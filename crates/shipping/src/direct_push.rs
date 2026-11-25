//! The one `Lander` built (ADR-0114): a plain `git push` of the batch's top to the base, from Remit's clone, as the account
//! the forge's CLI is logged in as. It never forces: git refuses a push that is not a fast-forward, which is how the base
//! only moves forward. The forge's word is the last one: after the push the base must stand at the top there, and a
//! refused push is told apart from a base that moved by asking the forge where the base stands.

use std::sync::Arc;

use async_trait::async_trait;
use forge::Repo;
use model::QueueKey;

use crate::landing::{Lander, Pushed};
use chains::forges::{FORGE_TIMEOUT, Forge};
use chains::git::git;

/// How long one push may take.
const PUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Lands by pushing to the base; f is the forge the base is hosted on, asked where the base stands.
pub struct DirectPush {
    pub forge: Arc<dyn Forge>,
}

impl DirectPush {
    async fn base_at(&self, q: &QueueKey) -> Result<String, String> {
        let r = Repo {
            host: q.host.clone(),
            project: q.project.clone(),
        };
        match tokio::time::timeout(FORGE_TIMEOUT, self.forge.branch_head(&r, &q.base)).await {
            Ok(Ok(at)) => Ok(at),
            Ok(Err(err)) => Err(format!(
                "{} did not say where {} stands: {err}",
                self.forge.name(),
                q.base
            )),
            Err(_) => Err(format!(
                "{} did not say where {} stands in time",
                self.forge.name(),
                q.base
            )),
        }
    }
}

#[async_trait]
impl Lander for DirectPush {
    async fn land(&self, clone: &str, q: &QueueKey, onto: &str, top: &str) -> Result<Pushed, String> {
        let dest = format!("{top}:refs/heads/{}", q.base);
        let pushed = tokio::time::timeout(PUSH_TIMEOUT, git(clone, &["push", "-q", "origin", &dest]))
            .await
            .unwrap_or_else(|_| Err(format!("the push took longer than {PUSH_TIMEOUT:?}")));
        let at = self.base_at(q).await?;
        match pushed {
            Ok(_) if at == top => Ok(Pushed::Landed),
            Ok(_) => Err(format!(
                "the push of {top} to {} went through, but {} says it stands at {at}",
                q.base,
                self.forge.name()
            )),
            // a push that reached the forge and only its answer was lost
            Err(_) if at == top => Ok(Pushed::Landed),
            Err(_) if at != onto => Ok(Pushed::Moved),
            Err(err) => Ok(Pushed::Refused(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chains::forges::fake::FakeForge;

    use super::*;

    async fn sh(dir: &Path, args: &[&str]) -> String {
        let mut argv = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
        argv.extend_from_slice(args);
        git(&dir.to_string_lossy(), &argv).await.unwrap()
    }

    async fn commit(dir: &Path, file: &str) -> String {
        std::fs::write(dir.join(file), file).unwrap();
        sh(dir, &["add", "-A"]).await;
        sh(dir, &["commit", "-q", "-m", file]).await;
        sh(dir, &["rev-parse", "HEAD"]).await
    }

    /// The base only moves forward: a top that does not build on where the base stands is never pushed over it, and a
    /// base that moved is told apart from a refusal.
    #[tokio::test]
    async fn the_base_only_moves_forward() {
        let d = tempfile::tempdir().unwrap();
        let origin = d.path().join("origin.git");
        sh(
            d.path(),
            &["init", "-q", "--bare", "-b", "main", &origin.to_string_lossy()],
        )
        .await;
        sh(d.path(), &["clone", "-q", &origin.to_string_lossy(), "clone"]).await;
        let clone = d.path().join("clone");
        let base = commit(&clone, "a").await;
        sh(&clone, &["push", "-q", "origin", "HEAD:main"]).await;
        let forge = Arc::new(FakeForge::default());
        forge.set_branch("main", &base);
        let lander = DirectPush { forge: forge.clone() };
        let q = QueueKey {
            host: "h".to_owned(),
            project: "p".to_owned(),
            base: "main".to_owned(),
        };
        let dir = clone.to_string_lossy().into_owned();

        let top = commit(&clone, "b").await;
        forge.set_branch("main", &top); // the forge sees what was pushed
        assert_eq!(lander.land(&dir, &q, &base, &top).await.unwrap(), Pushed::Landed);
        assert_eq!(sh(&origin, &["rev-parse", "main"]).await, top);

        // a top built on the old base is not a fast-forward: the base stays, and the queue combines again
        sh(&clone, &["checkout", "-q", "--detach", &base]).await;
        let stale = commit(&clone, "c").await;
        assert_eq!(lander.land(&dir, &q, &base, &stale).await.unwrap(), Pushed::Moved);
        assert_eq!(sh(&origin, &["rev-parse", "main"]).await, top, "nothing forced");

        // the same push where the base stands where the batch was combined is the forge's refusal
        assert!(matches!(
            lander.land(&dir, &q, &top, &stale).await.unwrap(),
            Pushed::Refused(_)
        ));
        assert_eq!(sh(&origin, &["rev-parse", "main"]).await, top);
    }
}
