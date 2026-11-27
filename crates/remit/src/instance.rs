//! What this server knows of itself, for the facts it records about itself (`orch::Instance`, ADR-0087): how it stands by
//! `/healthz`'s rule and the release it runs from.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use model::Time;
use store::{PingProgress, PingStage, Store};

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
    let progress = Arc::new(PingProgress::default());
    match tokio::time::timeout(PING_FOR, store.ping(Arc::clone(&progress))).await {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            tracing::error!(%err, "healthz: database does not answer");
            false
        }
        Err(_) => {
            // `err` is what the issue a chain starts from carries: the stage the ping stood at names what to look at.
            tracing::error!(err = waited_on(progress.stage()), "healthz: database does not answer in time");
            false
        }
    }
}

/// What a ping that ran out of time waited on, by the stage it reached.
fn waited_on(stage: PingStage) -> &'static str {
    match stage {
        PingStage::Queued => "no blocking thread ran the ping: the host was slow",
        PingStage::WaitingForReader => "no reader was free: the ping waited behind other reads",
        PingStage::Querying => "SELECT 1 did not return on its reader: SQLite or the host was slow",
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use store::{Error, Store};
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::db_answers;

    // A probe that times out while every reader is held says so in the issue the next investigation starts from.
    #[tokio::test]
    async fn a_ping_that_waited_behind_held_readers_names_them_in_its_issue() {
        let (issues, mut issued) = crate::errors::layer();
        let _log = tracing::subscriber::set_default(tracing_subscriber::registry().with(issues));
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(dir.path().join("remit.db")).unwrap());
        let n = std::thread::available_parallelism().map_or(1, usize::from);
        let (taken, release) = (Arc::new(Barrier::new(n + 1)), Arc::new(Barrier::new(n + 1)));
        let held: Vec<_> = (0..n)
            .map(|_| {
                let (store, taken, release) = (Arc::clone(&store), Arc::clone(&taken), Arc::clone(&release));
                tokio::spawn(async move {
                    store
                        .read(move |_| {
                            taken.wait();
                            release.wait();
                            Ok::<_, Error>(())
                        })
                        .await
                })
            })
            .collect();
        let t = Arc::clone(&taken);
        tokio::task::spawn_blocking(move || t.wait()).await.unwrap();

        let answered = db_answers(&store).await;
        // released before any assert, so a failing one does not leave the readers waiting for good
        let r = Arc::clone(&release);
        tokio::task::spawn_blocking(move || r.wait()).await.unwrap();
        for h in held {
            h.await.unwrap().unwrap();
        }

        assert!(!answered);
        let issue = issued.try_recv().unwrap();
        assert_eq!(issue.message, "healthz: database does not answer in time");
        assert_eq!(issue.error, "no reader was free: the ping waited behind other reads");
        assert!(db_answers(&store).await);
    }
}
