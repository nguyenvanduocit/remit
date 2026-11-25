//! What a firing does in another domain (ADR-0096): the pipeline delivers it, the application does it. `orch`
//! implements `Actions` once over the crates that do each, and the pipeline's processors and shipped jobs call it where
//! they did the work themselves before the split.

use async_trait::async_trait;

use chains::Result;

use crate::automation::DigestPeriod;
use crate::processing::Delivery;

#[async_trait]
pub trait Actions: Send + Sync {
    /// A hire firing: the agent its fact names is hired on its chain (hire.rs in shipping).
    async fn fire_hire(&self, d: &Delivery) -> Result<()>;

    /// An end firing: the chain its subscription names is ended (hire.rs in shipping).
    async fn fire_end(&self, d: &Delivery) -> Result<()>;

    /// A help's deadline came (help.rs in crew).
    async fn fire_deadline(&self, d: &Delivery) -> Result<()>;

    /// An inbox item the Assistant decides for the owner (assist.rs in orch).
    async fn fire_assist(&self, d: &Delivery) -> Result<()>;

    /// The forge's checks on a landing recorded on it, and a breach reverted (landing_checks.rs in shipping).
    async fn fire_record_landing_checks(&self, d: &Delivery) -> Result<()>;

    /// A teaching an agent's memory learns from, recorded for the librarian (teach_memory.rs in crew).
    async fn fire_teach(&self, d: &Delivery) -> Result<()>;

    /// Embeds the memories with no vector (librarian.rs in crew).
    async fn embed_memories(&self);
    /// Asks the librarian to review what memory holds (consolidate.rs in crew).
    async fn review_memories(&self);

    /// The owner's digest of period (digest.rs in orch).
    async fn fire_digest(&self, d: &Delivery, period: DigestPeriod) -> Result<()>;

    /// Runs every queue with a landing ready, and keeps the landing sense of every chain in step (landing_checks.rs in
    /// shipping).
    async fn sync_landings(&self);

    /// Has the forge's checks on what chain id landed read now (landing_checks.rs in shipping).
    async fn follow_chain_landings(&self, id: &str);
}
