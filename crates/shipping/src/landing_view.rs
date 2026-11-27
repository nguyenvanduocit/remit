//! What the owner reads of a landing (ADR-0114): its diff, read from Remit's clone, or, for a version of a folder worked in
//! place (ADR-0137), between its two checkpoints. The queues and releases are laid out by orch's repo_view.rs.

use wire::app::{FileChange, LandingDiff};

use crate::Shipping;
use crate::queue::fork_point;
use crate::shadow::Shadow;
use chains::git::{RepoRemote, git, path_str};
use chains::{Error, Result};

/// Where a landing's commits are read from.
enum Commits {
    /// Remit's clone, at this path.
    Clone(String),
    /// The shadow Git directory of a folder worked in place.
    Folder(Shadow),
}

impl Commits {
    async fn git(&self, args: &[&str]) -> Result<String> {
        match self {
            Self::Clone(dir) => git(dir, args).await,
            Self::Folder(shadow) => shadow.read(args).await,
        }
        .map_err(Error::Refused)
    }
}

impl Shipping {
    /// The files landing id changes, from the base it forked from to the version the gates approved (a revert: its
    /// target's change backwards), each with its diff.
    pub async fn landing_diff(&self, id: &str) -> Result<LandingDiff> {
        let key = id.to_owned();
        let l = self.store.read(move |r| r.landing(&key)).await?;
        let commits = if l.queue.is_none() {
            let chain = self.chain(&l.chain_id).await?;
            let rw = chain
                .repos
                .iter()
                .find(|rw| rw.repo == l.repo && rw.in_place())
                .ok_or_else(|| Error::refused(format!("the chain no longer holds {} in place", l.repo)))?;
            Commits::Folder(self.shadow_of(rw))
        } else {
            Commits::Clone(path_str(&self.clone_dir(&RepoRemote {
                host: l.host().to_owned(),
                project: l.project().to_owned(),
            })))
        };
        let (from, to) = match l.reverted_one() {
            None => (
                match &commits {
                    Commits::Clone(clone) => fork_point(clone, &l).await.map_err(Error::Refused)?,
                    Commits::Folder(_) => l.version.forked.clone(),
                },
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
        let stat = commits.git(&[&["diff", "--numstat"][..], &range[..]].concat()).await?;
        let mut files = Vec::new();
        for line in stat.lines() {
            let mut cols = line.splitn(3, '\t');
            let (Some(added), Some(removed), Some(path)) = (cols.next(), cols.next(), cols.next()) else {
                continue;
            };
            let diff = commits
                .git(&[&["diff"][..], &range[..], &["--", path][..]].concat())
                .await?;
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
