//! Ending every process of a Seatbelt sandbox.
//!
//! A sandbox is an instance: every process started under it stays in it, whatever process group or session it moves to
//! and whoever it is reparented to, and the profile's `(allow signal (target same-sandbox))` lets a member signal only
//! the members of its own instance. That is how the helper finds what to end without a list of descendants macOS cannot
//! give: it sends SIGKILL to every process of the user started after itself, and the kernel lets through only those of
//! its sandbox (ADR-0069).

use std::time::Duration;

use libproc::bsd_info::BSDInfo;
use libproc::proc_pid::pidinfo;
use libproc::processes::{ProcFilter, pids_by_type};
use rustix::io::Errno;
use rustix::process::{Pid, Signal, getppid, getuid, kill_process, test_kill_process};

/// When a process started, as the kernel keeps it: seconds and microseconds.
pub(crate) type StartTime = (u64, u64);

/// Bounds how many passes a sweep makes before it gives up on processes that keep appearing.
const MAX_SWEEPS: usize = 200;

/// How long a pass that ended members waits before the next looks: a killed process takes a moment to leave the table.
const SWEEP_PAUSE: Duration = Duration::from_millis(5);

/// The state of a process that has ended and waits for its parent to collect it: there is nothing left to end.
const SZOMB: u32 = 5;

/// Checks that the sandbox refuses this process a signal to a process outside it, the parent that started it, and
/// returns when this process started: the sweep ends nothing older. It runs before the program starts, while the parent
/// is surely outside the sandbox and alive.
pub(crate) fn signals_held_in() -> Result<StartTime, String> {
    let parent = getppid().ok_or("the helper has no parent")?;
    match test_kill_process(parent) {
        Err(Errno::PERM) => {}
        other => {
            return Err(format!(
                "the sandbox lets this process signal its parent outside it ({other:?}): the end of the program could not tell its own processes from the rest"
            ));
        }
    }
    start_time(std::process::id().cast_signed()).map_err(|e| format!("when the helper started: {e}"))
}

fn start_time(pid: i32) -> Result<StartTime, String> {
    let info = pidinfo::<BSDInfo>(pid, 0)?;
    Ok((info.pbi_start_tvsec, info.pbi_start_tvusec))
}

/// Ends every process of the sandbox but this one: SIGKILL to every process of the user started since, the kernel
/// refusing those outside the sandbox, until a pass ends none.
pub(crate) fn sweep(since: StartTime) -> Result<(), String> {
    let me = std::process::id();
    for _ in 0..MAX_SWEEPS {
        let pids = pids_by_type(ProcFilter::ByUID { uid: getuid().as_raw() })
            .map_err(|e| format!("list the processes to end: {e}"))?;
        let mut ended: u32 = 0;
        for pid in pids {
            if pid <= 1 || pid == me {
                continue;
            }
            let Ok(info) = pidinfo::<BSDInfo>(pid.cast_signed(), 0) else {
                continue; // gone since the list was made
            };
            if info.pbi_status == SZOMB || (info.pbi_start_tvsec, info.pbi_start_tvusec) < since {
                continue;
            }
            // EPERM is a process outside the sandbox, ESRCH one already gone: neither is this sandbox's to end.
            if let Some(p) = Pid::from_raw(pid.cast_signed())
                && kill_process(p, Signal::KILL).is_ok()
            {
                ended = ended.saturating_add(1);
            }
        }
        if ended == 0 {
            return Ok(());
        }
        std::thread::sleep(SWEEP_PAUSE);
    }
    Err(format!(
        "processes of the sandbox still appear after {MAX_SWEEPS} passes"
    ))
}
