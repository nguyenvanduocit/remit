//! The first stage of the Linux helper: the namespaces and the one user mapping the kernel allows without privilege.

use std::os::unix::process::CommandExt;
use std::process::Command;

use nix::sched::{CloneFlags, unshare};
use rustix::process::{Pid, Signal, getgid, getuid, kill_process};
use rustix::thread::{CapabilitySet, CapabilitySets, capabilities, clear_ambient_capability_set, set_capabilities};
use signal_hook::consts::{SIGCHLD, SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::init_linux::NAMESPACED_NAME;
use crate::plan_linux::Plan;
use crate::status::Report;
use crate::{HELPER_FAILED, run::exit_code};

/// The capabilities the second stage needs while it sets the namespaces up: mount and pivot_root, bringing the loopback
/// up, and dropping the bounding set. It drops all of them before the program starts.
fn needed() -> CapabilitySet {
    CapabilitySet::SETPCAP | CapabilitySet::NET_ADMIN | CapabilitySet::SYS_ADMIN
}

/// Unshares the namespaces, maps the caller's ids to themselves (files keep their owner, so a session still reads and
/// writes what it did as this user), passes the capabilities on to the second stage as ambient ones, starts it, gives up
/// its own, forwards the parent's signals to it and exits with its status. It runs as the first thing of the binary,
/// single-threaded, as unshare(CLONE_NEWUSER) requires.
pub(crate) fn first_stage(body: &str, status: &str) -> i32 {
    // Registered before the second stage starts, so neither its end nor a signal is missed. The first stage creates no
    // thread: after unshare(CLONE_NEWPID) the kernel refuses one (clone answers EINVAL), so it waits on its own thread.
    let signals = Signals::new([SIGTERM, SIGINT, SIGHUP, SIGQUIT, SIGCHLD]);
    let (mut signals, mut second) = match signals
        .map_err(|e| format!("signals: {e}"))
        .and_then(|s| unshare_and_start(body, status).map(|c| (s, c)))
    {
        Ok(started) => started,
        Err(why) => {
            match Report::connect(status) {
                Ok(r) => r.failed(&why),
                Err(e) => eprintln!("remit jail: {why} (status socket: {e})"),
            }
            return HELPER_FAILED;
        }
    };
    let pid = Pid::from_raw(second.id().cast_signed());
    loop {
        match second.try_wait() {
            Ok(Some(s)) => return exit_code(s),
            Ok(None) => {}
            Err(_) => return HELPER_FAILED,
        }
        for sig in signals.wait() {
            if let (false, Some(pid), Some(sig)) = (sig == SIGCHLD, pid, Signal::from_named_raw(sig)) {
                // The second stage may have just ended.
                kill_process(pid, sig).ok();
            }
        }
    }
}

fn unshare_and_start(body: &str, status: &str) -> Result<std::process::Child, String> {
    let plan: Plan = serde_json::from_str(body).map_err(|e| format!("read plan: {e}"))?;
    let (uid, gid) = (getuid().as_raw(), getgid().as_raw());
    // A jail gets its own network and pid namespaces too: without the first the program shares the host's loopback,
    // and without the second it can signal every process of the same user.
    let mut flags = CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS;
    if plan.jail.is_some() {
        flags |= CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_NEWPID;
    }
    unshare(flags).map_err(|e| format!("unshare the namespaces: {e}"))?;
    std::fs::write("/proc/self/uid_map", format!("{uid} {uid} 1")).map_err(|e| format!("map the user: {e}"))?;
    std::fs::write("/proc/self/setgroups", "deny").map_err(|e| format!("deny setgroups: {e}"))?;
    std::fs::write("/proc/self/gid_map", format!("{gid} {gid} 1")).map_err(|e| format!("map the group: {e}"))?;
    let mut sets = capabilities(None).map_err(|e| format!("read capabilities: {e}"))?;
    sets.inheritable |= needed();
    set_capabilities(None, sets).map_err(|e| format!("inheritable capabilities: {e}"))?;
    for cap in needed().iter() {
        rustix::thread::configure_capability_in_ambient_set(cap, true)
            .map_err(|e| format!("ambient capability {cap:?}: {e}"))?;
    }
    #[expect(
        clippy::disallowed_methods,
        reason = "the second stage starts inside the namespaces the first just unshared, with its argv[0] set: no other crate can"
    )]
    let second = Command::new("/proc/self/exe")
        .arg0(NAMESPACED_NAME)
        .args([body, status])
        .spawn()
        .map_err(|e| format!("start the helper in its namespaces: {e}"))?;
    // The first stage only waits from here: it keeps no capability.
    clear_ambient_capability_set().ok();
    let none = CapabilitySets {
        effective: CapabilitySet::empty(),
        permitted: CapabilitySet::empty(),
        inheritable: CapabilitySet::empty(),
    };
    set_capabilities(None, none).ok();
    Ok(second)
}
