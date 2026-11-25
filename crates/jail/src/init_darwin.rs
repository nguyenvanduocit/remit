//! The macOS helper, run inside the sandbox, and its sweeper.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;

use rustix::process::{Pid, Resource, Rlimit, Signal, kill_process, setrlimit};
use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::plan_darwin::{Limits, Plan};
use crate::sandbox_darwin::{real_path, within};
use crate::status::Report;
use crate::sweep_darwin::{StartTime, signals_held_in, sweep};
use crate::{HELPER_FAILED, HELPER_NAME};

/// argv\[1\] of the sweeper: the helper's companion in the sandbox, in a process group of its own, that ends the sandbox
/// once the helper is gone, so that the end of the command's process group (dropping its `procgroup::Child`) ends the
/// processes that left that group too.
const SWEEPER_NAME: &str = "remit-jail-sweep";

pub(crate) fn init() {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, name, plan, status] if name == HELPER_NAME => std::process::exit(helper(plan, status)),
        [_, name, sec, usec] if name == SWEEPER_NAME => std::process::exit(sweeper(sec, usec)),
        _ => {}
    }
}

/// The helper: it reports on the status socket and exits with the program's status.
fn helper(plan: &str, status: &str) -> i32 {
    let report = match Report::connect(status) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("remit jail: status socket: {e}");
            return HELPER_FAILED;
        }
    };
    match prepare(plan) {
        Ok((plan, path, since)) => supervise(&path, &plan, report, since),
        Err(why) => {
            report.failed(&why);
            HELPER_FAILED
        }
    }
}

/// Reads the plan, checks the sandbox holds signals in, finds the program and applies the limits.
fn prepare(body: &str) -> Result<(Plan, String, StartTime), String> {
    let plan: Plan = serde_json::from_str(body).map_err(|e| format!("read plan: {e}"))?;
    let Some(argv0) = plan.argv.first() else {
        return Err("read plan: no program".into());
    };
    // Before anything starts: the end of the program signals every process of the user, which is safe only while the
    // sandbox refuses every signal to a process outside it.
    let since = signals_held_in()?;
    let path = if plan.path.is_empty() {
        let env: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
        crate::lookpath::look_path(argv0, &env, |p| shows(&plan, p))?
    } else {
        plan.path.clone()
    };
    if let Some(l) = &plan.limits {
        apply_limits(l)?;
    }
    Ok((plan, path, since))
}

/// Sets the jail's rlimits on the helper, which the program inherits when it starts.
fn apply_limits(l: &Limits) -> Result<(), String> {
    for (resource, name, value) in [
        (Resource::Cpu, "cpu", l.cpu_seconds),
        // A core file would hold the program's memory, credentials included, in the work directory.
        (Resource::Core, "core", 0),
    ] {
        setrlimit(
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

enum Event {
    Ended(i32),
    Signal(i32),
}

/// Starts the program and stays beside it in the sandbox: it forwards the signals the parent sends but SIGTERM, and
/// once the program has ended, or on SIGTERM (the parent's stop), it ends every process of the sandbox. It returns the
/// program's exit code.
fn supervise(path: &str, plan: &Plan, report: Report, since: StartTime) -> i32 {
    // Registered before the start: a signal in between is held, not lost.
    let mut signals = match Signals::new([SIGTERM, SIGINT, SIGHUP, SIGQUIT]) {
        Ok(s) => s,
        Err(e) => {
            report.failed(&format!("signals: {e}"));
            return HELPER_FAILED;
        }
    };
    // The sweeper reads its stdin to the end, which comes when this helper is gone however it ended.
    let (alive, alive_end) = match std::io::pipe() {
        Ok((r, w)) => (w, r),
        Err(e) => {
            report.failed(&format!("sweeper pipe: {e}"));
            return HELPER_FAILED;
        }
    };
    let sweeper = std::env::current_exe().and_then(|exe| {
        #[expect(
            clippy::disallowed_methods,
            reason = "the sweeper starts inside the sandbox, as a member of it, from the helper: no other crate can"
        )]
        let mut sweeper = Command::new(exe);
        sweeper
            .args([SWEEPER_NAME, &since.0.to_string(), &since.1.to_string()])
            .stdin(alive_end)
            .stdout(Stdio::null())
            .process_group(0)
            .spawn()
    });
    if let Err(e) = sweeper {
        report.failed(&format!("start the sweeper: {e}"));
        return HELPER_FAILED;
    }
    #[expect(
        clippy::disallowed_methods,
        reason = "the helper starts the jail's program inside the sandbox it is a member of: no other crate can"
    )]
    let mut program = Command::new(path);
    if let Some((argv0, args)) = plan.argv.split_first() {
        program.arg0(argv0).args(args);
    }
    let mut child = match program.spawn() {
        Ok(c) => c,
        Err(e) => {
            report.failed(&format!("start {path}: {e}"));
            return HELPER_FAILED;
        }
    };
    // The program runs: the parent may return from spawn.
    report.running();
    let pid = Pid::from_raw(child.id().cast_signed());
    let (tx, events) = mpsc::channel();
    let ended = tx.clone();
    std::thread::spawn(move || {
        let code = child.wait().map_or(-1, crate::run::exit_code);
        ended.send(Event::Ended(code)).ok();
    });
    std::thread::spawn(move || {
        for sig in signals.forever() {
            if tx.send(Event::Signal(sig)).is_err() {
                return;
            }
        }
    });
    let code = loop {
        match events.recv() {
            Ok(Event::Ended(code)) => break code,
            Ok(Event::Signal(SIGTERM)) => {
                if let Err(e) = sweep(since) {
                    eprintln!("remit jail: {e}");
                }
                // The program is among the swept: its end comes next.
            }
            Ok(Event::Signal(sig)) => {
                if let (Some(pid), Some(sig)) = (pid, Signal::from_named_raw(sig)) {
                    // The program may have just ended.
                    kill_process(pid, sig).ok();
                }
            }
            Err(_) => break HELPER_FAILED,
        }
    };
    if let Err(e) = sweep(since) {
        eprintln!("remit jail: {e}");
    }
    drop(alive);
    code
}

/// Waits for the helper's end, then ends the sandbox: whatever is left of it once the helper was killed with its group.
fn sweeper(sec: &str, usec: &str) -> i32 {
    let (Ok(sec), Ok(usec)) = (sec.parse(), usec.parse()) else {
        return HELPER_FAILED;
    };
    let mut rest = Vec::new();
    // The end of stdin, or an error reading it, both mean the helper is gone.
    std::io::stdin().read_to_end(&mut rest).ok();
    match sweep((sec, usec)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("remit jail: {e}");
            HELPER_FAILED
        }
    }
}

/// Says whether the profile lets the program read path: under a readable path and under no hidden one.
fn shows(plan: &Plan, path: &str) -> bool {
    let resolved = real_path(path);
    plan.readable.iter().any(|r| within(&resolved, r)) && !plan.hidden.iter().any(|h| within(&resolved, h))
}
