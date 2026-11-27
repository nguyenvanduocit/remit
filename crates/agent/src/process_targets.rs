//! The signal boundary. A menu token refers to one opened OS process in one session.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use cosca::identity::{Liveness, ProcessId, Recursive, Resolved};
use cosca::process::Process;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use uuid::Uuid;
use wire::app::SessionProcess;

use crate::Error;

#[derive(Default)]
pub(crate) struct Targets {
    live: HashMap<u32, Target>,
}

struct Target {
    token: Uuid,
    process: Process,
}

impl Targets {
    /// Opens before the metrics sample so even a recycled PID during the sample cannot acquire the old row's action.
    pub(crate) fn pin_descendants(&mut self, root: u32) {
        let mut previous = std::mem::take(&mut self.live);
        let Resolved::Found(root) = ProcessId::of(root) else {
            return;
        };
        let Ok(children) = root.children(Recursive::Yes) else {
            return;
        };
        self.live = children
            .into_iter()
            .filter_map(|id| {
                let target = match previous
                    .remove(&id.pid())
                    .filter(|t| t.process.id() == id && alive(&t.process))
                {
                    Some(t) => t,
                    None => Target {
                        token: Uuid::new_v4(),
                        process: Process::open(id).ok()??,
                    },
                };
                Some((id.pid(), target))
            })
            .collect();
    }

    /// The main session child and any process of Remit's own binary belong to the existing whole-session stop path.
    pub(crate) fn mark(&mut self, rows: &mut [SessionProcess], system: &System) {
        let remit = remit_executable();
        let mut shown = HashSet::new();
        for row in rows {
            if let Some(target) = self.live.get(&row.pid)
                && alive(&target.process)
                && program(system, row.pid, remit.as_deref())
            {
                row.kill_token = Some(target.token.to_string());
                shown.insert(row.pid);
            }
        }
        self.live.retain(|pid, _| shown.contains(pid));
    }

    pub(crate) fn kill(&mut self, root: u32, pid: u32, still_open: impl FnOnce() -> bool) -> Result<(), Error> {
        let target = self.live.get(&pid).ok_or_else(expired)?;
        let Resolved::Found(root) = ProcessId::of(root) else {
            return Err(expired());
        };
        let mut current = target.process.id();
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current) {
                return Err(expired());
            }
            match current.parent().map_err(failed)? {
                Some(parent) if parent == root => break,
                Some(parent) => current = parent,
                None => return Err(expired()),
            }
        }

        let mut system = System::new().map_err(failed)?;
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        if !program(&system, pid, remit_executable().as_deref()) || !alive(&target.process) || !still_open() {
            return Err(expired());
        }
        target.process.kill().map_err(failed)?;
        self.live.remove(&pid);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn alive(process: &Process) -> bool {
    // The stable identity query avoids arming a kqueue exit watch for each row.
    process.id().is_alive() == Liveness::Alive
}

#[cfg(not(target_os = "macos"))]
fn alive(process: &Process) -> bool {
    // The held pidfd also distinguishes two instances that started in the same clock tick.
    process.is_alive() == Liveness::Alive
}

fn remit_executable() -> Option<PathBuf> {
    std::env::current_exe().ok()?.canonicalize().ok()
}

fn program(system: &System, pid: u32, remit: Option<&Path>) -> bool {
    let exe = system
        .process(Pid::from_u32(pid))
        .and_then(sysinfo::Process::exe)
        .and_then(|p| p.canonicalize().ok());
    matches!((exe, remit), (Some(exe), Some(remit)) if exe != remit)
}

fn expired() -> Error {
    Error::TurnFailed("the process is no longer an actionable member of this session".to_owned())
}

fn failed(error: impl std::fmt::Display) -> Error {
    Error::TurnFailed(format!("inspect or signal the process: {error}"))
}
