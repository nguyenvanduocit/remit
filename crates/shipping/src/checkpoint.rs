//! What a folder worked in place decides about its checkpoints (ADR-0137), as pure functions of values: which folder is
//! fit to open, what a checkpoint is called, where a version of its change is measured from, and what a restore writes
//! or refuses. The git that takes and reads checkpoints is `shadow.rs`.

use std::collections::BTreeSet;
use std::path::Path;

use model::{Landing, LandingState, RepoWork, WorkPlace};
use sha2::{Digest, Sha256};

/// One checkpoint: the commit of the folder's shadow Git directory, when it was taken (seconds since the epoch) and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub commit: String,
    pub at: i64,
    pub why: String,
}

/// What a file differs by between a checkpoint and the folder now, as a restore sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Differs {
    /// The checkpoint has it and the folder has not, or has other content: the restore writes the checkpoint's.
    Write,
    /// The folder has it and the checkpoint has not: the restore removes it.
    Remove,
}

/// What restoring a checkpoint does to the folder, each path relative to it, sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Restore {
    pub write: Vec<String>,
    pub remove: Vec<String>,
}

/// Why a restore does not happen: a file it would overwrite holds an edit no checkpoint holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub path: String,
}

/// The name of the shadow Git directory of the folder at dir: its last element and a digest of the whole path, so two
/// folders of one name never share one and the same folder always finds its own. Pure.
pub fn shadow_name(dir: &str) -> String {
    let digest = Sha256::digest(dir.as_bytes());
    let name: String = dir
        .rsplit('/')
        .find(|e| !e.is_empty())
        .unwrap_or("folder")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    format!("{name}-{}.git", hex::encode(digest.get(..6).unwrap_or_default()))
}

/// Why dir is no folder to work in place, or none: the machine's root and the user's whole home hold more than any chain's
/// work, and a checkpoint copies what the folder holds. Pure.
pub fn unfit_folder(dir: &Path, home: Option<&Path>) -> Option<&'static str> {
    if dir.parent().is_none() {
        return Some("the machine's root is not a folder to work in place: name the project's own folder");
    }
    if home.is_some_and(|h| h == dir) {
        return Some("a home directory is not a folder to work in place: name the project's own folder inside it");
    }
    None
}

/// Where the open change of the folder rw is measured from: the commit its newest landing that is on the folder
/// (`Landed`) was checkpointed as, else the checkpoint taken when the chain opened it. A version a gate sent back is not
/// a base, so the next version holds everything since the last one approved. Pure.
pub fn measured_from<'a>(rw: &'a RepoWork, landings: &'a [Landing]) -> &'a str {
    landings
        .iter()
        .rfind(|l| l.repo == rw.repo && l.queue.is_none() && l.reverts.is_none() && l.state == LandingState::Landed)
        .map(|l| l.version.from.as_str())
        .or(match &rw.place {
            WorkPlace::Folder { opened, .. } => Some(opened.as_str()),
            WorkPlace::Worktree(_) => None,
        })
        .unwrap_or_default()
}

/// The files `git diff --name-status -z` lists (`M\0path\0`, `A\0path\0`, `D\0path\0`), as a restore sees them: the
/// folder has a file the checkpoint has not (`A`) and it goes, any other difference writes the checkpoint's file back.
/// Pure.
pub fn differing(name_status: &str) -> Vec<(Differs, String)> {
    let mut fields = name_status.split('\0').filter(|f| !f.is_empty());
    let mut out = Vec::new();
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        let how = if status.starts_with('A') {
            Differs::Remove
        } else {
            Differs::Write
        };
        out.push((how, path.to_owned()));
    }
    out
}

/// What restoring a checkpoint does, given the files that differ between it and the folder now, and the files that differ
/// between the newest checkpoint and the folder now (`unsaved`: the user's own edits, which no checkpoint holds): it
/// refuses, naming the first, when a file it would overwrite or remove is among them. Pure.
pub fn plan_restore(differs: &[(Differs, String)], unsaved: &[String]) -> Result<Restore, Refusal> {
    let unsaved: BTreeSet<&str> = unsaved.iter().map(String::as_str).collect();
    if let Some(path) = differs
        .iter()
        .map(|(_, p)| p.as_str())
        .filter(|p| unsaved.contains(p))
        .min()
    {
        return Err(Refusal { path: path.to_owned() });
    }
    let mut plan = Restore::default();
    for (how, path) in differs {
        match how {
            Differs::Write => plan.write.push(path.clone()),
            Differs::Remove => plan.remove.push(path.clone()),
        }
    }
    plan.write.sort();
    plan.remove.sort();
    Ok(plan)
}

/// The checkpoints `git log --format=%H%x1f%ct%x1f%s%x1e` lists, newest first. Pure.
pub fn parse_log(log: &str) -> Vec<Checkpoint> {
    log.split('\u{1e}')
        .filter_map(|rec| {
            let mut f = rec.trim().splitn(3, '\u{1f}');
            Some(Checkpoint {
                commit: f.next().filter(|c| !c.is_empty())?.to_owned(),
                at: f.next()?.parse().ok()?,
                why: f.next().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// The checkpoint a name picks: a commit id, or its first characters when only one starts so. Pure.
pub fn pick<'a>(have: &'a [Checkpoint], name: &str) -> Result<&'a Checkpoint, String> {
    let name = name.trim();
    if name.len() < 4 {
        return Err(format!(
            "{name:?} is too short to name a checkpoint: give at least 4 characters of its id"
        ));
    }
    let hits: Vec<&Checkpoint> = have.iter().filter(|c| c.commit.starts_with(name)).collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no checkpoint starts with {name:?}")),
        _ => Err(format!("{name:?} starts several checkpoints: give more of its id")),
    }
}
