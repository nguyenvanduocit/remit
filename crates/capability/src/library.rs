//! The library's skills: what an agent definition can hold (agents.rs), kept and changed here. An agent changes them
//! through the tools of library_tools.rs, and what a session is given from them is decided by the `library` crate.

use std::future::Future;
use std::sync::{Mutex, PoisonError};

use model::{Actor, ChangeKind, Skill};
use store::Tx;
use wire::app::{ListedSkill, SkillFile, SkillFileText};

use crate::Capabilities;
use crate::over::{Named, Over};
use chains::chain_core::optional;
use chains::changes::{Authorship, record_change};
use chains::{Error, Result};

/// What the library keeps in memory.
#[derive(Default)]
pub struct LibraryState {
    /// Orders changes of the skills: a change lays the library out and stores the skill under it, so the directory
    /// sessions load skills from is always the one for what is stored. Held across the store write, so it is async.
    pub(crate) change_mu: tokio::sync::Mutex<()>,
    /// What is laid out, read at once by whoever starts a session.
    pub(crate) laid_out: Mutex<LaidOut>,
}

/// The renderings sessions load skills from.
#[derive(Default, Clone)]
pub(crate) struct LaidOut {
    /// The rendering of the stored skills; empty when there is none.
    pub(crate) plugin_dir: String,
    /// The skills that ship with Remit, and their rendering: set at start, empty where nothing can be laid out.
    pub(crate) builtin: Vec<Skill>,
    pub(crate) builtin_dir: String,
}

impl LibraryState {
    pub(crate) fn laid_out(&self) -> std::sync::MutexGuard<'_, LaidOut> {
        self.laid_out.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn listed_skill(s: &Skill) -> ListedSkill {
    ListedSkill {
        name: s.name.clone(),
        description: s.description.clone(),
        group: s.group.clone(),
        files: s
            .files
            .iter()
            .map(|(path, text)| SkillFile {
                path: path.clone(),
                size: text.len(),
            })
            .collect(),
        source: s.source.clone(),
        rev: s.rev,
    }
}

pub fn listed_skills(held: &[Skill]) -> Vec<ListedSkill> {
    held.iter().map(listed_skill).collect()
}

/// Stores skill s in the transaction tx, with its change, at at. The library is laid out for it by `change_skills`,
/// which the write runs under.
pub fn put_skill(tx: &Tx<'_>, w: &Authorship, at: model::Time, s: Skill) -> Result<Skill> {
    let cur = optional(tx.skill(&s.name))?;
    let s = tx.put_skill(s)?;
    record_change(tx, w, at, ChangeKind::Skill, &s.name, cur.as_ref(), Some(&s))?;
    Ok(s)
}

/// The edit of the stored skills that puts s in place of the skill of its name.
pub fn with_skill(s: Skill) -> impl FnOnce(Vec<Skill>) -> Vec<Skill> + Send {
    move |held| {
        let mut out = without_skill(held, &s.name);
        out.push(s);
        out
    }
}

/// Skill name as a refused save names it.
fn named_skill(name: &str) -> Named<'_> {
    Named {
        what: "skill",
        id: name,
        create: "create_skill",
        update: "update_skill",
        reread: "read_skill",
    }
}

fn without_skill(mut list: Vec<Skill>, name: &str) -> Vec<Skill> {
    list.retain(|s| s.name != name);
    list
}

impl Capabilities {
    /// Writes this version of a skill over what over names, decided in the transaction that writes it. The library is
    /// laid out for sessions before the skill is stored, so a skill that cannot be laid out is not stored either.
    pub(crate) async fn save_skill(&self, by: &Actor, s: Skill, over: Over) -> Result<Skill> {
        let s = self.prepared_skill(s).await?;
        let (w, at) = (
            Authorship {
                by: by.clone(),
                approved_by: None,
            },
            self.now(),
        );
        let mut stored = s.clone();
        let s = self
            .change_skills(with_skill(s), async move {
                self.store
                    .update(move |tx| {
                        let rev = optional(tx.skill(&stored.name))?.map(|c| c.rev);
                        stored.rev = over.rev(rev, &named_skill(&stored.name))?;
                        put_skill(tx, &w, at, stored)
                    })
                    .await
            })
            .await?;
        self.notify.skill_updated(&listed_skill(&s));
        Ok(s)
    }

    /// The text of the file at path in the skill name; not found when the skill has no such file.
    pub async fn skill_file(&self, name: &str, path: &str) -> Result<SkillFileText> {
        let key = name.to_owned();
        let s = self.store.read(move |r| r.skill(&key)).await?;
        match s.files.get(path) {
            Some(text) => Ok(SkillFileText { text: text.clone() }),
            None => Err(store::Error::NotFound.into()),
        }
    }

    /// s as it would be written, or why it cannot be.
    pub async fn prepared_skill(&self, mut s: Skill) -> Result<Skill> {
        s.name = s.name.trim().to_owned();
        skills::validate(&s).map_err(|e| Error::refused(e.to_string()))?;
        s.group = model::parse_group(&s.group).map_err(|e| Error::refused(e.to_string()))?;
        let held = self.store.read(|r| r.skills()).await?;
        ::library::check_skill_name(&s.name, &held).map_err(|e| Error::refused(e.to_string()))?;
        Ok(s)
    }

    /// Saves a skill that does not exist yet; a name already in use is refused, never overwritten. It is refused before
    /// an agent's write is held for the owner, and again in the write.
    pub(crate) async fn create_skill(&self, by: &Actor, s: Skill) -> Result<Skill> {
        self.stored_skill(&s.name, Over::New).await?;
        self.hold_skill(by, &s).await?;
        self.save_skill(by, s, Over::New).await
    }

    /// Replaces a skill that exists with s, made from a read of it at s.rev: a skill written since is not written over.
    /// It is refused before an agent's write is held for the owner, and again in the write. Where it first came from
    /// stays true of it, so it is kept.
    pub(crate) async fn update_skill(&self, by: &Actor, mut s: Skill) -> Result<Skill> {
        let over = Over::Read(s.rev);
        let cur = self.stored_skill(&s.name, over).await?;
        if s.source.is_none() {
            s.source = cur.and_then(|c| c.source);
        }
        self.hold_skill(by, &s).await?;
        self.save_skill(by, s, over).await
    }

    /// The skill name as stored, if a save over would not be refused now.
    pub(crate) async fn stored_skill(&self, name: &str, over: Over) -> Result<Option<Skill>> {
        let name = name.trim().to_owned();
        let key = name.clone();
        let cur = optional(self.store.read(move |r| r.skill(&key)).await)?;
        over.rev(cur.as_ref().map(|c| c.rev), &named_skill(&name))?;
        Ok(cur)
    }

    /// Changes one file of skill name by quoting the text to change: old, which the file holds exactly once, becomes
    /// new, and the rest of the skill stays as it is stored. Quoted text the file no longer holds was changed since it was
    /// read, so nothing is written; text held more than once is refused until the quote is long enough to be one place.
    pub(crate) async fn edit_skill_file(
        &self,
        by: &Actor,
        name: &str,
        path: &str,
        old: &str,
        new: &str,
    ) -> Result<Skill> {
        let key = name.to_owned();
        let Some(mut s) = optional(self.store.read(move |r| r.skill(&key)).await)? else {
            return Err(Error::refused(format!(
                "no skill {name:?}: list_library shows the names, create_skill makes a new one"
            )));
        };
        let Some(text) = s.files.get(path) else {
            let paths: Vec<&str> = s.files.keys().map(String::as_str).collect();
            return Err(Error::refused(format!(
                "skill {name} has no file {path:?}: its files are {}; a new file goes in with update_skill",
                paths.join(", ")
            )));
        };
        if old.is_empty() {
            return Err(Error::refused("quote the text to change in old: it cannot be empty"));
        }
        let text = match text.matches(old).count() {
            1 => text.replacen(old, new, 1),
            0 => {
                return Err(Error::refused(format!(
                    "{path} of skill {name} does not hold the text quoted in old: it changed since it was read, or the quote is not exact. read_skill shows it as it is now"
                )));
            }
            n => {
                return Err(Error::refused(format!(
                    "{path} of skill {name} holds the text quoted in old {n} times: quote more of the lines around the change, so it is one place"
                )));
            }
        };
        s.files.insert(path.to_owned(), text);
        self.update_skill(by, s).await
    }

    /// Removes a skill no agent holds.
    pub(crate) async fn delete_skill(&self, by: &Actor, name: &str) -> Result<()> {
        let key = name.to_owned();
        self.store
            .read(move |r| r.skill(&key))
            .await
            .map_err(|e| Error::refused(format!("skill {name:?}: {e}")))?;
        self.agents_holding_skill(name).await?;
        let (w, at, key) = (
            Authorship {
                by: by.clone(),
                approved_by: None,
            },
            self.now(),
            name.to_owned(),
        );
        let gone = name.to_owned();
        self.change_skills(move |held| without_skill(held, &gone), async move {
            self.store
                .update(move |tx| -> Result<()> {
                    let cur = tx.skill(&key)?;
                    tx.delete_skill(&key)?;
                    record_change::<Skill>(tx, &w, at, ChangeKind::Skill, &key, Some(&cur), None)
                })
                .await
        })
        .await?;
        self.notify.skill_deleted(name);
        Ok(())
    }

    /// Makes a change to the stored skills: edit says what the skills are afterwards, and the library is laid out for
    /// that before write stores it. Nothing is stored when the layout fails. Once write is done, the directory sessions
    /// load skills from is the rendering of what is stored: the one laid out, or the one before when write stored
    /// nothing (a change the owner let through whose record moved while it waited).
    pub async fn change_skills<T, F>(&self, edit: impl FnOnce(Vec<Skill>) -> Vec<Skill>, write: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let _order = self.library.change_mu.lock().await;
        let held = self.store.read(|r| r.skills()).await?;
        self.lay_out_library(skills::PLUGIN_NAME, &edit(held))
            .await
            .map_err(|e| Error::refused(format!("the library's skills cannot be laid out for sessions: {e}")))?;
        let done = write.await?;
        let held = self.store.read(|r| r.skills()).await?;
        // laid out already: equal skills are the same directory
        let dir = self
            .lay_out_library(skills::PLUGIN_NAME, &held)
            .await
            .map_err(|e| Error::refused(format!("the library's skills cannot be laid out for sessions: {e}")))?;
        self.library.laid_out().plugin_dir = dir;
        Ok(done)
    }
}
