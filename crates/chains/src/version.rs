//! A version of a chain's change is made by the employees whose work the Chair accepted in its worktree: a card is tied to
//! a repository by its cwd, the worktree or a directory inside it, as the Chair hires it. land names them on the landing
//! (`Landing::by`) and as Co-authored-by trailers of the commit it lands as.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use model::{Card, Decision, DecisionKind, Employee, RepoWork, Signs, Time};

use crate::chain_core::Core;
use crate::{Error, Result};

/// Who made a version: `by`, the employees' ids the landing records; `made_by`, the labels it shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Version {
    pub by: Vec<String>,
    pub made_by: Vec<String>,
}

impl Core {
    /// Who made the version of rw a land submits: the workers `version_by` finds.
    pub async fn version_of(&self, chain_id: &str, rw: &RepoWork) -> Result<Version> {
        let id = chain_id.to_owned();
        let (cards, ds, landings) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.cards(&id)?, r.decisions(&id)?, r.chain_landings(&id)?)))
            .await?;
        let since = landings
            .iter()
            .filter(|l| l.repo == rw.repo && l.on_base())
            .filter_map(|l| l.landed_at)
            .max();
        let by = version_by(&cards, &ds, rw, since);
        let mut staff: HashMap<String, Employee> = HashMap::new();
        let mut made_by = Vec::with_capacity(by.len());
        for id in &by {
            let e = match staff.get(id) {
                Some(e) => e.clone(),
                None => {
                    let key = id.clone();
                    let e = self
                        .store
                        .read(move |r| r.employee(&key))
                        .await
                        .map_err(|err| Error::refused(format!("the employee {id} of this version: {err}")))?;
                    staff.insert(id.clone(), e.clone());
                    e
                }
            };
            made_by.push(if e.label.is_empty() { e.shown() } else { e.label });
        }
        Ok(Version { by, made_by })
    }
}

/// The employee ids of the workers (cards that sign nothing) the Chair accepted after since whose cwd is rw's worktree or
/// inside it, in the order first accepted. since is when the chain's change to the repository last landed (none when it
/// has not), so the list is every worker of what has not landed yet, and none of what landed before.
pub fn version_by(cards: &[Card], ds: &[Decision], rw: &RepoWork, since: Option<Time>) -> Vec<String> {
    let by_id: HashMap<&str, &Card> = cards.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut ds: Vec<&Decision> = ds.iter().collect();
    ds.sort_by_key(|d| d.created_at); // stable: decisions made at once keep their order
    let mut out: Vec<String> = Vec::new();
    for d in ds {
        let Some(c) = by_id.get(d.card_id.as_str()) else {
            continue;
        };
        if d.kind != DecisionKind::Accept
            || since.is_some_and(|since| d.created_at <= since)
            || c.employee.is_empty()
            || c.signs != Signs::None
            || !cwd_in_worktree(&c.cwd, &rw.path)
        {
            continue;
        }
        if !out.contains(&c.employee) {
            out.push(c.employee.clone());
        }
    }
    out
}

/// Whether cwd is worktree or a directory inside it.
pub fn cwd_in_worktree(cwd: &str, worktree: &str) -> bool {
    if cwd.is_empty() || worktree.is_empty() {
        return false;
    }
    clean(cwd).starts_with(clean(worktree))
}

/// p with its . and .. elements resolved lexically.
fn clean(p: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for c in Path::new(p).components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}
