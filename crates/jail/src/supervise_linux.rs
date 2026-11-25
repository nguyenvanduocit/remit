//! The second stage's last work: giving up privileges, and running the program as the namespace's init.

use std::os::unix::process::CommandExt;
use std::process::Command;

use rustix::process::{Pid, Resource, Rlimit, Signal, WaitOptions, kill_process, prlimit, wait};
use rustix::thread::{
    CapabilitySet, CapabilitySets, clear_ambient_capability_set, remove_capability_from_bounding_set, set_capabilities,
    set_no_new_privs,
};
use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::HELPER_FAILED;
use crate::plan_linux::{JailPlan, Plan};
use crate::status::Report;

/// Ends everything the helper needed for the mounts: no ambient capability, an empty bounding set (which stops a program
/// that runs as root in its namespace from regaining them at exec), no_new_privs, and no capability of its own.
pub(crate) fn give_up_privileges() -> Result<(), String> {
    clear_ambient_capability_set().map_err(|e| format!("clear ambient capabilities: {e}"))?;
    for cap in CapabilitySet::all().iter() {
        if let Err(e) = remove_capability_from_bounding_set(cap)
            && e != rustix::io::Errno::INVAL
        {
            return Err(format!("drop capability {cap:?}: {e}"));
        }
    }
    set_no_new_privs(true).map_err(|e| format!("no_new_privs: {e}"))?;
    let none = CapabilitySets {
        effective: CapabilitySet::empty(),
        permitted: CapabilitySet::empty(),
        inheritable: CapabilitySet::empty(),
    };
    set_capabilities(None, none).map_err(|e| format!("drop the helper's capabilities: {e}"))
}

/// Runs the program as the namespace's init: it starts it, applies its limits, forwards the signals the parent sends (an
/// init receives no signal it has no handler for), reaps every orphan, and returns the program's exit code. When it
/// returns, the kernel ends whatever is left in a pid namespace.
pub(crate) fn supervise(p: &Plan, report: Report, relay: Option<std::net::TcpListener>) -> i32 {
    match start(p, relay) {
        Ok(pid) => {
            // The program runs under its limits: the parent may return from spawn.
            report.running();
            reap(pid)
        }
        Err(why) => {
            report.failed(&why);
            HELPER_FAILED
        }
    }
}

fn start(p: &Plan, relay: Option<std::net::TcpListener>) -> Result<Pid, String> {
    let path = if p.path.is_empty() {
        let env: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
        // Every path is visible: the helper looks once the jail's root is its own, which holds only what the jail shows.
        crate::lookpath::look_path(p.argv.first().map_or("", String::as_str), &env, |_| true)?
    } else {
        p.path.clone()
    };
    if let (Some(l), Some(socket)) = (relay, p.jail.as_ref().and_then(|j| j.socket.clone())) {
        start_relay(l, socket)?;
    }
    // Registered before the start: a signal in between is held, not lost.
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP, SIGQUIT]).map_err(|e| format!("signals: {e}"))?;
    #[expect(
        clippy::disallowed_methods,
        reason = "the helper starts the program in the namespaces and view it set up, as its supervisor: no other crate can"
    )]
    let mut program = Command::new(&path);
    if let Some((argv0, args)) = p.argv.split_first() {
        program.arg0(argv0).args(args);
    }
    let child = program.spawn().map_err(|e| format!("start {path}: {e}"))?;
    let pid = Pid::from_raw(child.id().cast_signed()).ok_or("the program has no pid")?;
    if let Some(j) = &p.jail
        && let Err(e) = limit(pid, j)
    {
        // The program is being refused: whether it was already gone changes nothing.
        kill_process(pid, Signal::KILL).ok();
        return Err(e);
    }
    std::thread::spawn(move || {
        for sig in signals.forever() {
            if let Some(sig) = Signal::from_named_raw(sig) {
                // The program may have just ended.
                kill_process(pid, sig).ok();
            }
        }
    });
    Ok(pid)
}

/// Listens on the jail's own loopback and copies every connection to the unix socket, on a thread of the helper, so the
/// relay is part of the jail's process tree and ends with it. It adds no way out: the program could connect to the
/// socket itself, the path being in its filesystem; it gives ordinary tooling a proxy address.
fn start_relay(l: std::net::TcpListener, socket: String) -> Result<(), String> {
    l.set_nonblocking(true).map_err(|e| format!("relay: {e}"))?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|e| format!("relay: {e}"))?;
    std::thread::spawn(move || {
        rt.block_on(async {
            if let Ok(l) = tokio::net::TcpListener::from_std(l) {
                crate::relay::serve_relay(l, socket).await;
            }
        });
    });
    Ok(())
}

/// Waits for every child of the process, and returns when pid is the one that ended.
fn reap(pid: Pid) -> i32 {
    loop {
        match wait(WaitOptions::empty()) {
            Ok(Some((got, status))) if got == pid => {
                return status
                    .terminating_signal()
                    .and_then(|s| s.checked_add(128))
                    .or(status.exit_status())
                    .unwrap_or(HELPER_FAILED);
            }
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(e) => {
                eprintln!("remit jail: wait: {e}");
                return HELPER_FAILED;
            }
        }
    }
}

/// Sets the program's rlimits from outside: set in the helper, they would bind the helper too.
fn limit(pid: Pid, j: &JailPlan) -> Result<(), String> {
    for (resource, name, value) in [
        (Resource::Data, "memory", j.memory_bytes),
        (Resource::Cpu, "cpu", j.cpu_seconds),
        // A core file would hold the program's memory, credentials included, in the work directory.
        (Resource::Core, "core", 0),
    ] {
        prlimit(
            Some(pid),
            resource,
            Rlimit {
                current: Some(value),
                maximum: Some(value),
            },
        )
        .map_err(|e| format!("limit {name}: {e}"))?;
    }
    Ok(())
}
