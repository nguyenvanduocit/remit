//! Samples the OS tree below the child that Claude's connection owns (including any jail supervisor).
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use wire::app::SessionProcess;

use crate::Error;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct ProcessSampler {
    system: Option<System>,
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
        let rows = tree(system, root, &previous);
        self.latest = Some((Instant::now(), rows.clone()));
        Ok(rows)
    }
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
        });
        if let Some(mut next) = children.remove(&pid) {
            next.sort_unstable();
            pending.extend(next.into_iter().rev().map(|pid| (pid, depth.saturating_add(1))));
        }
    }
    rows
}
