//! Samples the OS tree below the child that Claude's connection owns (including any jail supervisor).
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use wire::app::SessionProcess;

use crate::Error;
use crate::process_targets::Targets;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct ProcessSampler {
    system: Option<System>,
    targets: Targets,
    latest: Option<(Instant, Vec<SessionProcess>)>,
}

pub(crate) fn sample(conn: &claudecode::Conn, sampler: &Mutex<ProcessSampler>) -> Result<Vec<SessionProcess>, Error> {
    let mut held = sampler.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(pid) = conn.child_pid() else {
        return Ok(Vec::new());
    };
    let rows = held.read(Pid::from_u32(pid))?;
    // The PID cannot be reused until the connection reaps it, which clears it before waiting.
    if conn.child_pid() != Some(pid) {
        return Ok(Vec::new());
    }
    Ok(rows)
}

pub(crate) fn kill(conn: &claudecode::Conn, sampler: &Mutex<ProcessSampler>, token: &str) -> Result<(), Error> {
    let mut held = sampler.lock().unwrap_or_else(PoisonError::into_inner);
    let pid = conn
        .child_pid()
        .ok_or_else(|| Error::TurnFailed("the session has ended".to_owned()))?;
    held.kill(pid, token, || conn.child_pid() == Some(pid))
}

impl ProcessSampler {
    fn read(&mut self, root: Pid) -> Result<Vec<SessionProcess>, Error> {
        if let Some((at, rows)) = &self.latest
            && at.elapsed() < SAMPLE_INTERVAL
        {
            return Ok(rows.clone());
        }
        let system = match &mut self.system {
            Some(system) => system,
            empty => empty.insert(System::new().map_err(|e| Error::TurnFailed(format!("read process table: {e}")))?),
        };
        self.targets.pin_descendants(root.as_u32());
        let previous: HashMap<_, _> = system
            .processes()
            .iter()
            .map(|(&pid, p)| (pid, p.start_time()))
            .collect();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );
        let mut rows = tree(system, root, &previous);
        let pids: Vec<_> = rows.iter().map(|row| Pid::from_u32(row.pid)).collect();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        self.targets.mark(&mut rows, system);
        self.latest = Some((Instant::now(), rows.clone()));
        Ok(rows)
    }

    fn kill(&mut self, root: u32, token: &str, still_open: impl FnOnce() -> bool) -> Result<(), Error> {
        let Some((_, rows)) = self.latest.as_ref() else {
            return Err(expired_selection());
        };
        let selected = rows
            .iter()
            .find(|row| row.kill_token.as_deref() == Some(token))
            .ok_or_else(expired_selection)?;
        let pid = selected.pid;
        self.targets.kill(root, pid, still_open)?;
        self.latest = None;
        Ok(())
    }
}

fn expired_selection() -> Error {
    Error::TurnFailed("the process selection has expired".to_owned())
}

fn tree(system: &System, root: Pid, previous: &HashMap<Pid, u64>) -> Vec<SessionProcess> {
    let mut children = HashMap::<_, Vec<_>>::new();
    for (&pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut rows = Vec::new();
    let mut pending = vec![(root, 0_u32)];
    let mut seen = HashSet::new();
    while let Some((pid, depth)) = pending.pop() {
        let Some(process) = system.process(pid).filter(|_| seen.insert(pid)) else {
            continue;
        };
        let cpu = process.cpu_usage();
        rows.push(SessionProcess {
            pid: pid.as_u32(),
            depth,
            command: process.name().to_string_lossy().into_owned(),
            state: process.status().to_string(),
            cpu_percent: (previous.get(&pid) == Some(&process.start_time()) && cpu.is_finite()).then_some(cpu),
            resident_bytes: process.memory(),
            elapsed_seconds: process.run_time(),
            kill_token: None,
        });
        if let Some(mut next) = children.remove(&pid) {
            next.sort_unstable();
            pending.extend(next.into_iter().rev().map(|pid| (pid, depth.saturating_add(1))));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;

    use super::{Duration, Pid, ProcessSampler};

    #[tokio::test]
    async fn force_kill_is_bound_to_a_displayed_child_of_this_session() {
        let root = std::process::id();
        let mut selected = procgroup::command("/bin/sleep", ["30"]).spawn().unwrap();
        let mut other = procgroup::command("/bin/sleep", ["30"]).spawn().unwrap();
        let (pid, other_pid) = (selected.id().unwrap(), other.id().unwrap());
        let mut sampler = ProcessSampler::default();
        let rows = sampler.read(Pid::from_u32(root)).unwrap();
        let token = rows
            .iter()
            .find(|row| row.pid == pid)
            .and_then(|row| row.kill_token.as_deref())
            .expect("the session child has a force-kill action");
        assert!(rows.iter().any(|row| row.pid == root && row.kill_token.is_none()));
        assert!(sampler.kill(other_pid, token, || true).is_err());
        assert!(sampler.kill(root, token, || false).is_err());
        assert!(selected.try_wait().unwrap().is_none());

        sampler.kill(root, token, || true).unwrap();
        let status = tokio::time::timeout(Duration::from_secs(3), selected.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        assert!(sampler.kill(root, token, || true).is_err());
        assert!(other.try_wait().unwrap().is_none());
    }
}
