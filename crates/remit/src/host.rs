//! The machine this server runs on, as the capacity reads it (`chains::ports::Host`, ADR-0115): the memory free, the
//! memory its own processes hold (the agents' sessions and what they run are its descendants), the last minute's load and
//! the cores.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use orch::chains::ports::{Host, HostReading};
use sysinfo::{MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// This host, read through sysinfo; the system is kept between looks, as sysinfo asks.
pub struct ThisHost {
    system: Mutex<Option<System>>,
}

impl ThisHost {
    pub fn new() -> Self {
        Self {
            system: Mutex::new(System::new().ok()),
        }
    }
}

impl Host for ThisHost {
    fn reading(&self) -> Option<HostReading> {
        let mut held = self.system.lock().unwrap_or_else(PoisonError::into_inner);
        let sys = held.as_mut()?;
        sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory(),
        );
        let load = System::load_average().ok()?;
        let cores = std::thread::available_parallelism().map_or(0, |n| u32::try_from(n.get()).unwrap_or(u32::MAX));
        Some(HostReading {
            free_memory: sys.available_memory(),
            sessions_memory: descendants_memory(sys, Pid::from_u32(std::process::id())),
            load: load.one,
            cores,
        })
    }
}

/// The bytes resident in the processes that descend from root, root not counted.
fn descendants_memory(sys: &System, root: Pid) -> u64 {
    let parents: HashMap<Pid, Pid> = sys
        .processes()
        .iter()
        .filter_map(|(pid, p)| p.parent().map(|parent| (*pid, parent)))
        .collect();
    let descends = |mut pid: Pid| {
        // a parent chain is a few links long; the bound stops a cycle a racing pid reuse could make
        for _ in 0..64 {
            match parents.get(&pid) {
                Some(&parent) if parent == root => return true,
                Some(&parent) => pid = parent,
                None => return false,
            }
        }
        false
    };
    sys.processes()
        .iter()
        .filter(|(pid, _)| descends(**pid))
        .map(|(_, p)| p.memory())
        .sum()
}
