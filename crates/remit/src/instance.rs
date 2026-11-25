//! What this server knows of itself, for the facts it records about itself (`orch::Instance`, ADR-0087): how it stands by
//! `/healthz`'s rule and the release it runs from.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use model::Time;
use store::Store;

use crate::health::{Build, PING_FOR, verdict};
use crate::ports::RuntimeLink;

/// This server, as `orch::Instance` asks of it.
pub struct ThisServer {
    pub store: Arc<Store>,
    pub launcher: runtimelink::Launcher<RuntimeLink>,
    pub clock: orch::Clock,
    pub build: Build,
    /// The release it runs from (`release_of`).
    pub tag: String,
    pub started_at: Time,
}

#[async_trait]
impl orch::Instance for ThisServer {
    async fn health(&self, last_tick: Option<Time>) -> orch::InstanceHealth {
        let body = verdict(
            db_answers(&self.store).await,
            last_tick,
            &self.launcher.state(),
            (self.clock)(),
            &self.build,
        );
        orch::InstanceHealth {
            revision: body.revision,
            tag: self.tag.clone(),
            started_at: self.started_at,
            healthy: body.ok,
            reason: body.reason,
        }
    }
}

/// Whether the database answers within `PING_FOR`, as `/healthz` asks it.
pub async fn db_answers(store: &Store) -> bool {
    match tokio::time::timeout(PING_FOR, store.ping()).await {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            tracing::error!(%err, "healthz: database does not answer");
            false
        }
        Err(_) => {
            tracing::error!("healthz: database does not answer in time");
            false
        }
    }
}

/// The release this binary runs from: the name of its directory when that is one of releases' (`<releases>/<tag>/remit`,
/// as `remit update` installs it); empty when it runs from elsewhere.
pub fn release_of(exe: &Path, releases: &Path) -> String {
    let (Ok(exe), Ok(releases)) = (exe.canonicalize(), releases.canonicalize()) else {
        return String::new();
    };
    match exe.parent() {
        Some(dir) if dir.parent() == Some(releases.as_path()) => dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        _ => String::new(),
    }
}
