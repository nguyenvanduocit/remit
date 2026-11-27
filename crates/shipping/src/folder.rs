//! A chain's work in place in the user's own folder (ADR-0137): opening it with a checkpoint, measuring a version of its
//! change between two checkpoints, the change a verdict covers, what a landing in place records, and taking a
//! checkpoint back. The folder has no forge, clone, branch, push or queue; the pure rules are `checkpoint.rs`, the git is
//! `shadow.rs`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use model::{
    Actor, Card, Chain, ChainStatus, DecisionKind, Gate, Landing, LandingState, RepoWork, Submit, WorkPlace, by_agent,
    by_remit,
};
use serde::Deserialize;

use crate::Shipping;
use crate::checkpoint::{Restore, measured_from, pick, unfit_folder};
use crate::folder_repo::FolderRepo;
use crate::gate::RAILS_FILE;
use crate::land::{Found, change_gates, last_element, path_rules};
use crate::shadow::{Restored, Shadow};
use chains::apply::Records;
use chains::chain_core::first_line;
use chains::fact::{FactValue, fact_write, keys};
use chains::git::{path_str, short};
use chains::review_depth::Said;
use chains::step::{Decided, replace_with};
use chains::{Error, Result};

/// How long a checkpoint of a folder may take: a first one of a large folder copies what it holds.
const CHECKPOINT_TIMEOUT: Duration = Duration::from_secs(180);

/// restore_checkpoint: the checkpoint to write the folder back to and why.
#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestoreInput {
    /// the chain's id; leave it empty when you are the chain's Chair
    #[serde(default)]
    pub chain: String,
    /// the folder as given to open_repo; empty when the chain has exactly one repository open
    #[serde(default)]
    pub repo: String,
    /// the commit id (at least its first 4 characters) of the checkpoint to write the folder back to; empty lists the
    /// folder's checkpoints, newest first, and writes nothing
    #[serde(default)]
    pub checkpoint: String,
    /// why the folder is taken back, in the owner's words: it is recorded
    #[serde(default)]
    pub why: String,
}

/// The chain among chains, other than chain_id's own, that holds dir in place and is not done.
fn held_by<'a>(chains: &'a [Chain], chain_id: &str, dir: &str) -> Option<&'a Chain> {
    chains
        .iter()
        .filter(|c| c.id != chain_id && *c.status() != ChainStatus::Done)
        .find(|c| c.repos.iter().any(|rw| rw.in_place() && rw.dir() == dir))
}

impl Shipping {
    /// The shadow Git directory of the folder rw works in place in.
    pub(crate) fn shadow_of(&self, rw: &RepoWork) -> Shadow {
        Shadow::new(&self.cfg.folders_dir, rw.dir())
    }

    /// Gives the chain the folder dir to work in place, with the checkpoint of the folder as it is: refused when another
    /// chain that is not done holds it, or it is no folder to work in.
    pub(crate) async fn open_folder(&self, chain: &Chain, chair: &Card, dir: &Path) -> Result<RepoWork> {
        let dir = tokio::fs::canonicalize(dir)
            .await
            .map_err(|e| Error::refused(format!("{}: {e}", dir.display())))?;
        if !dir.is_dir() {
            return Err(Error::refused(format!("{} is not a directory", dir.display())));
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        if let Some(why) = unfit_folder(&dir, home.as_deref()) {
            return Err(Error::refused(why));
        }
        let dir = path_str(&dir);
        let chains = self.store.read(|r| r.chains()).await?;
        if let Some(rw) = chain.repos.iter().find(|rw| rw.in_place() && rw.dir() == dir) {
            return Ok(rw.clone()); // already open, as it was named another way
        }
        if let Some(other) = held_by(&chains, &chain.id, &dir) {
            return Err(Error::refused(format!(
                "{dir} is held in place by chain {} ({}), which is not done: finish that chain, or work in another folder",
                other.id, other.title
            )));
        }
        let shadow = Shadow::new(&self.cfg.folders_dir, &dir);
        let opened = tokio::time::timeout(
            CHECKPOINT_TIMEOUT,
            shadow.checkpoint(&format!("open for chain {}", chain.id)),
        )
        .await
        .map_err(|_| {
            Error::refused(format!(
                "the first checkpoint of {dir} took longer than {CHECKPOINT_TIMEOUT:?}"
            ))
        })?
        .map_err(Error::refused)?;
        let rw = RepoWork {
            repo: dir.clone(),
            place: WorkPlace::Folder {
                path: dir.clone(),
                opened: opened.clone(),
            },
            submit: None,
        };
        let d = Decided {
            kind: DecisionKind::Repo,
            text: format!("Open {dir} in place (checkpoint {}).", short(&opened)),
            why: String::new(),
            card_id: chair.id.clone(),
            by: by_agent(&chair.id),
        };
        self.record_work(&chain.id, &rw.repo, replace_with(rw.clone()), vec![d])
            .await?;
        Ok(rw)
    }

    /// The version the folder rw holds, checkpointed and measured: its change from the checkpoint of the last version a gate
    /// approved (or of the folder as the chain opened it), the path rules it must pass and the gates it needs. The landing
    /// is returned unsaved, judging, with what the deciders did to its review (`Core::judge_change`).
    pub(crate) async fn version_of_folder(
        &self,
        chain: &Chain,
        rw: &RepoWork,
        s: &Submit,
    ) -> Result<(Landing, Vec<Said>), String> {
        let shadow = self.shadow_of(rw);
        let from = shadow
            .checkpoint(&format!("land: {}", first_line(&s.title, 100)))
            .await?;
        let id = chain.id.clone();
        let landings = self
            .store
            .read(move |r| r.chain_landings(&id))
            .await
            .map_err(|e| e.to_string())?;
        let forked = measured_from(rw, &landings).to_owned();
        let change = shadow.patch_id(&forked, &from).await?;
        if change.is_empty() {
            return Err(format!(
                "the folder changes nothing beyond its last approved checkpoint ({}): nothing to land",
                short(&forked)
            ));
        }
        let paths = shadow.changed_paths(&forked, &from).await?;
        path_rules(&paths)?;
        let rails = shadow.rails_at(&forked, RAILS_FILE).await;
        let names: Vec<&str> = paths.iter().map(String::as_str).collect();
        let gates: Vec<Gate> = crate::gate::required_gates(change_gates(s), &names, &rails);
        let found = Found {
            queue: None,
            from: from.clone(),
            forked: forked.clone(),
            change,
            gates,
            paths,
        };
        self.judged_version(chain, rw, s, found).await
    }

    /// Commits landing l, a version of a folder worked in place its gates just approved, on the branch of the folder's own
    /// Git repository (ADR-0139), and records the commit on l. A folder that is not the top of a Git repository has no
    /// history to write: its approval is the landing, and l keeps no commit. A landing with a queue is left as it is.
    pub(crate) async fn commit_in_folder(&self, chain: &Chain, l: &mut Landing) -> Result<(), String> {
        if l.queue.is_some() || l.state != LandingState::Landed {
            return Ok(());
        }
        let rw = chain
            .repos
            .iter()
            .find(|rw| rw.repo == l.repo && rw.in_place())
            .ok_or_else(|| format!("the chain no longer holds {} in place", l.repo))?;
        let Some(repo) = FolderRepo::at(rw.dir()).await else {
            return Ok(());
        };
        l.commit = repo
            .commit(l, &self.shadow_of(rw))
            .await
            .map_err(|e| format!("committing version {} in {}: {e}", l.version.number, l.repo))?;
        Ok(())
    }

    /// The command that prints the change landing l is, for the reviewers the Chair hires to read it, when l is a version of
    /// a folder worked in place: such a folder has no branch to diff, so the Chair is told the checkpoints to diff.
    pub(crate) fn read_command(&self, chain: &Chain, l: &Landing) -> Option<String> {
        let rw = chain.repos.iter().find(|rw| rw.repo == l.repo && rw.in_place())?;
        Some(self.shadow_of(rw).diff_command(&l.version.forked, &l.version.from))
    }

    /// The patch id of the change a verdict on the folder rw covers: its newest checkpoint measured from the last version
    /// approved. Refused while the folder holds an edit no checkpoint holds, so a verdict covers exactly what it judged.
    pub(crate) async fn folder_change(&self, chain_id: &str, rw: &RepoWork) -> Result<String, String> {
        let shadow = self.shadow_of(rw);
        let unsaved = shadow.unsaved().await?;
        if let Some(first) = unsaved.first() {
            return Err(format!(
                "the folder changed since its last checkpoint ({first}{}): land again so the verdict covers exactly what \
                 it judged",
                if unsaved.len() > 1 { " and others" } else { "" }
            ));
        }
        let id = chain_id.to_owned();
        let landings = self
            .store
            .read(move |r| r.chain_landings(&id))
            .await
            .map_err(|e| e.to_string())?;
        let forked = measured_from(rw, &landings);
        let head = shadow.head().await?;
        let change = shadow.patch_id(forked, &head).await?;
        if change.is_empty() {
            return Err(format!(
                "the folder changes nothing beyond its last approved checkpoint ({})",
                short(forked)
            ));
        }
        Ok(change)
    }

    /// Writes the folder the chain works in place in back to a checkpoint of it, on the word of by (the Chair, or the
    /// Assistant for the owner): it refuses, naming the file, when a file it would overwrite changed since the newest
    /// checkpoint. With no checkpoint named it lists them. Says what it did.
    pub async fn restore_checkpoint(&self, by: Actor, chain_id: &str, input: RestoreInput) -> Result<String> {
        let chain = self.chain(chain_id).await?;
        let i = crate::land::work_named(&chain.repos, &input.repo)
            .ok_or_else(|| Error::Refused(crate::land::not_open_text(&chain.repos, &input.repo)))?;
        let rw = chain
            .repos
            .get(i)
            .cloned()
            .ok_or_else(|| Error::refused("the repository is not open"))?;
        if !rw.in_place() {
            return Err(Error::refused(format!(
                "{} is a worktree of a clone, not a folder worked in place: its changes are taken back with revert_landing",
                rw.repo
            )));
        }
        let shadow = self.shadow_of(&rw);
        let have = shadow.checkpoints().await.map_err(Error::refused)?;
        if input.checkpoint.trim().is_empty() {
            let lines: Vec<String> = have
                .iter()
                .map(|c| format!("{} {}", short(&c.commit), first_line(&c.why, 100)))
                .collect();
            return Ok(format!(
                "Checkpoints of {}, newest first:\n{}",
                rw.repo,
                lines.join("\n")
            ));
        }
        if input.why.trim().is_empty() {
            return Err(Error::refused(
                "why is required: it is recorded, and the Chair reads it",
            ));
        }
        if rw.submit.is_some() {
            return Err(Error::refused(format!(
                "a land of {} is running, which checkpoints the folder: restore once its result arrives",
                rw.repo
            )));
        }
        let to = pick(&have, &input.checkpoint).map_err(Error::refused)?.clone();
        let restored = tokio::time::timeout(CHECKPOINT_TIMEOUT, shadow.restore(&to.commit))
            .await
            .map_err(|_| Error::refused(format!("restoring took longer than {CHECKPOINT_TIMEOUT:?}")))?
            .map_err(Error::refused)?;
        let Restore { write, remove } = match restored {
            Restored::Done(plan) => plan,
            Restored::Refused(r) => {
                return Err(Error::refused(format!(
                    "{} changed since the newest checkpoint, and restoring {} would overwrite it: nothing was written. Land \
                     the folder as it is first (a checkpoint keeps the file), or have its owner put the file aside",
                    r.path,
                    short(&to.commit)
                )));
            }
        };
        let text = format!(
            "Restored {} to checkpoint {}: {} files written back, {} removed.",
            rw.repo,
            short(&to.commit),
            write.len(),
            remove.len()
        );
        let d = Decided {
            kind: DecisionKind::Repo,
            text: text.clone(),
            why: input.why.trim().to_owned(),
            card_id: chain.chair_id.clone(),
            by: by.clone(),
        };
        // the Chair is woken for a restore it did not make itself
        let tell = if by.kind == model::ActorKind::Agent {
            String::new()
        } else {
            format!("[restore · {}]\n{text} Why: {}", rw.repo, input.why.trim())
        };
        let cmd = chains::step::ChainCmd::Work(chains::step::WorkCmd {
            repo: rw.repo.clone(),
            edit: chains::step::keep_record(),
            tell,
        });
        self.apply(
            &chain.id,
            cmd,
            Records {
                decisions: vec![d],
                ..Records::default()
            },
        )
        .await?;
        Ok(text)
    }
}

/// What a version of a folder worked in place records when its gates approved it and it landed (`approve`): the fact that
/// the chain's change landed and the decision, as a queue's landing records them when it reaches its base. A landing with
/// a queue records nothing here.
pub(crate) fn landed_in_place(chain: &Chain, l: &Landing, rec: &mut Records) -> Result<()> {
    if l.queue.is_some() || l.state != LandingState::Landed {
        return Ok(());
    }
    let v = FactValue {
        chain: chain.id.clone(),
        landing: l.id.clone(),
        repo: l.repo.clone(),
        commit: l.version.from.clone(),
        paths: l.version.paths.clone(),
        ..FactValue::default()
    };
    rec.readings
        .push(fact_write(keys::chain_landed(), &v, by_remit(""), &chain.id)?);
    rec.decisions.push(Decided {
        kind: DecisionKind::Landing,
        text: match &l.commit {
            Some(commit) => format!(
                "Committed {} version {} on the folder's branch as {}: {}.",
                last_element(&l.repo),
                l.version.number,
                short(commit),
                l.version.title
            ),
            None => format!(
                "Approved {} version {} at checkpoint {}: {}.",
                last_element(&l.repo),
                l.version.number,
                short(&l.version.from),
                l.version.title
            ),
        },
        why: l.version.description.clone(),
        card_id: chain.chair_id.clone(),
        by: by_remit(""),
    });
    Ok(())
}
