use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::store::{Result, Store};

/// How far a ping got: what a caller that stopped waiting for it reads to say what it waited on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingStage {
    /// No blocking thread had run it yet: the host was slow to schedule it.
    Queued,
    /// It waited for a reader: every one was held by other reads.
    WaitingForReader,
    /// It held a reader and SQLite had not answered: SQLite or the host was slow.
    Querying,
}

/// Where a ping is, set by the ping as it moves.
#[derive(Debug, Default)]
pub struct PingProgress(AtomicU8);

impl PingProgress {
    pub fn stage(&self) -> PingStage {
        match self.0.load(Ordering::Acquire) {
            0 => PingStage::Queued,
            1 => PingStage::WaitingForReader,
            _ => PingStage::Querying,
        }
    }

    fn reach(&self, stage: PingStage) {
        let n = match stage {
            PingStage::Queued => 0,
            PingStage::WaitingForReader => 1,
            PingStage::Querying => 2,
        };
        self.0.store(n, Ordering::Release);
    }
}

impl Store {
    /// Asks whether the database answers a read, marking progress at each stage. It leaves the write connection alone:
    /// there is only one, so a caller of ping (an unauthenticated health probe) would queue behind writers and make them
    /// wait in turn. A writer that is stuck shows in the scheduler, which writes on every Tick.
    pub async fn ping(&self, progress: Arc<PingProgress>) -> Result<()> {
        let waiting = Arc::clone(&progress);
        self.read_once_running(
            move || waiting.reach(PingStage::WaitingForReader),
            move |r| {
                progress.reach(PingStage::Querying);
                r.value::<i64>("SELECT 1", []).map(drop)
            },
        )
        .await
    }
}
