//! What the owner reads of a landing (ADR-0114): its diff, read from Remit's clone. The queues and releases are laid out
//! by orch's repo_view.rs.

use wire::app::{FileChange, LandingDiff};

use crate::Shipping;
use crate::queue::fork_point;
use chains::git::{RepoRemote, git, path_str};
use chains::{Error, Result};

impl Shipping {
    /// The files landing id changes, from the base it forked from to the version the gates approved (a revert: its
    /// target's change backwards), each with its diff.
    pub async fn landing_diff(&self, id: &str) -> Result<LandingDiff> {
        let key = id.to_owned();
        let l = self.store.read(move |r| r.landing(&key)).await?;
        let clone = path_str(&self.clone_dir(&RepoRemote {
            host: l.host.clone(),
            project: l.project.clone(),
        }));
        let (from, to) = match l.reverted_one() {
            None => (
                fork_point(&clone, &l).await.map_err(Error::Refused)?,
                l.version.from.clone(),
            ),
            Some(target) => {
                let key = target.to_owned();
                let t = self.store.read(move |r| r.landing(&key)).await?;
                let commit = t.commit.unwrap_or_default();
                (commit.clone(), format!("{commit}^"))
            }
        };
        let range = [from.as_str(), to.as_str()];
        let stat = git(&clone, &[&["diff", "--numstat"][..], &range[..]].concat())
            .await
            .map_err(Error::Refused)?;
        let mut files = Vec::new();
        for line in stat.lines() {
            let mut cols = line.splitn(3, '\t');
            let (Some(added), Some(removed), Some(path)) = (cols.next(), cols.next(), cols.next()) else {
                continue;
            };
            let diff = git(&clone, &[&["diff"][..], &range[..], &["--", path][..]].concat())
                .await
                .map_err(Error::Refused)?;
            files.push(FileChange {
                path: path.to_owned(),
                added: added.parse().unwrap_or_default(),
                removed: removed.parse().unwrap_or_default(),
                new: diff.contains("\nnew file mode").then_some(true),
                deleted: diff.contains("\ndeleted file mode").then_some(true),
                diff,
            });
        }
        Ok(LandingDiff {
            landing_id: l.id,
            from: to,
            files,
        })
    }
}
