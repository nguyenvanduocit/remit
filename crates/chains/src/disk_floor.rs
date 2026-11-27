//! The free-space floor (ADR-0140): a worker's or a judge's session does not start while the volume that holds the work
//! root, where every chain's worktree and what its agents build are written, has less free than `WORK_FLOOR`. The agents'
//! builds filled that volume once and the server's own writes failed with it; the floor keeps room for the sessions
//! already at work and for Remit's own database. The Chair and the Assistant start below it: they build nothing, and
//! they are who decides a refused card's retry and frees the space.

use model::{Card, CardRole};

use crate::admit::bytes_text;
use crate::chain_core::Core;

/// The free bytes below which no worker's or judge's session starts: room for two Rust builds at once (about 2 GB each)
/// and for the server's own writes.
pub const WORK_FLOOR: u64 = 5 << 30;

/// A session refused because the work root's volume is below the floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "the volume of the work root has {} free, below the floor of {} Remit keeps so the agents at work and its own \
     writes do not fill it; free space on it and retry",
    bytes_text(*free),
    bytes_text(*floor)
)]
pub struct BelowFloor {
    pub free: u64,
    pub floor: u64,
}

/// Whether card c is held by the floor: every seat that works in a worktree. The Chair and the Assistant decide and
/// recover, and build nothing. Pure.
pub fn held_by_floor(c: &Card) -> bool {
    match CardRole::of(c) {
        CardRole::Chair | CardRole::Assistant => false,
        CardRole::Worker | CardRole::Spawned | CardRole::Judge(_) => true,
    }
}

/// Whether free bytes are at or above the floor. Pure.
pub fn above_floor(free: u64, floor: u64) -> Result<(), BelowFloor> {
    if free < floor {
        return Err(BelowFloor { free, floor });
    }
    Ok(())
}

impl Core {
    /// Whether the work root's volume has room for card c's session to start (`WORK_FLOOR`). A seat the floor does not
    /// hold, a volume that cannot be read, or no host to read it, refuses nothing.
    pub async fn work_room(&self, c: &Card) -> Result<(), BelowFloor> {
        if !held_by_floor(c) {
            return Ok(());
        }
        let Some(host) = self.cfg.host.clone() else {
            return Ok(());
        };
        let root = self.cfg.work_root.clone();
        let free = match tokio::task::spawn_blocking(move || host.free_space(&root)).await {
            Ok(free) => free, // none: the host said why when it could not read it
            Err(err) => {
                tracing::warn!(%err, "read the work root's free space; the session starts unchecked");
                None
            }
        };
        match free {
            Some(free) => above_floor(free, WORK_FLOOR),
            None => Ok(()),
        }
    }
}
