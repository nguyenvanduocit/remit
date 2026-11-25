//! What a poller of Remit remembers of each thing it reads on a period (a sensor, sensor.rs): when the scheduler last had it
//! read, one read of it at a time, and how its reads have failed. A source that fails `SILENT_AFTER` reads in a row is
//! silent, and the Signals tab says so. It is in memory: a restart learns a silence again in a few periods.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use model::Time;

/// How many reads in a row must fail before a source is called silent.
const SILENT_AFTER: u32 = 3;

/// What a poller remembers of one thing it reads.
#[derive(Default)]
pub(crate) struct Poll {
    /// One read of it at a time, so two never store the same change twice.
    pub run: tokio::sync::Mutex<()>,
    state: Mutex<PollState>,
}

#[derive(Default)]
struct PollState {
    /// When the scheduler last had it read.
    polled: Option<Time>,
    /// Reads that failed in a row.
    failed: u32,
    /// The last failure.
    err: String,
}

/// How the reads of a silent thing failed: how many in a row, and the last error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailedReads {
    pub reads: u32,
    pub err: String,
}

/// The polls of the things a poller reads, by key.
#[derive(Default)]
pub struct Polls(Mutex<HashMap<String, Arc<Poll>>>);

impl Polls {
    /// The poll of key, made when it is the first.
    pub(crate) fn of(&self, key: &str) -> Arc<Poll> {
        let mut keys = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(keys.entry(key.to_owned()).or_default())
    }

    /// Whether key is to be read at now: it was never read, or last every ago. It marks it read, so the next Tick does not
    /// read it again while this read runs.
    pub fn claim(&self, key: &str, now: Time, every: time::Duration) -> bool {
        let poll = self.of(key);
        let mut st = poll.state.lock().unwrap_or_else(PoisonError::into_inner);
        if st
            .polled
            .is_some_and(|at| at.0.checked_add(every).is_some_and(|next| now.0 < next))
        {
            return false;
        }
        st.polled = Some(now);
        true
    }

    /// Forgets the polls of the keys not in keys: nothing reads them any more.
    pub(crate) fn keep_only(&self, keys: &[String]) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|k, _| keys.contains(k));
    }

    /// Records how a read of key went.
    pub fn result(&self, key: &str, err: Option<&str>) {
        let poll = self.of(key);
        let mut st = poll.state.lock().unwrap_or_else(PoisonError::into_inner);
        match err {
            None => (st.failed, st.err) = (0, String::new()),
            Some(err) => (st.failed, st.err) = (st.failed.saturating_add(1), err.to_owned()),
        }
    }

    /// The things whose last `SILENT_AFTER` reads or more all failed, by key.
    pub(crate) fn silent(&self) -> HashMap<String, FailedReads> {
        let keys = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        keys.iter()
            .filter_map(|(k, p)| {
                let st = p.state.lock().unwrap_or_else(PoisonError::into_inner);
                (st.failed >= SILENT_AFTER).then(|| {
                    (
                        k.clone(),
                        FailedReads {
                            reads: st.failed,
                            err: st.err.clone(),
                        },
                    )
                })
            })
            .collect()
    }
}
