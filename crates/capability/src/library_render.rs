//! The library's skills reach Claude Code as one plugin directory, rendered from what is stored. It is derived and never
//! edited: `<library_dir>/<hash>/plugin`, where the hash is of the rendered files, so equal skills are the same directory
//! and are written once. An older rendering stays while a session may still read it, and goes at the next start.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use model::{Card, Origin, Skill};
use sha2::{Digest, Sha256};

use crate::Capabilities;
use chains::cap_names::CAP_OKR;
use chains::chain_core::optional;
use chains::{Error, Result};

/// The hex digits of the hash a rendering is named by.
const HASH_LEN: usize = 12;
/// Names the directory a rendering is written in before it is renamed to its hash: the hash only ever names a whole
/// rendering, so a crash in the middle of one is never mistaken for it.
const STAGING_SUFFIX: &str = ".partial";

/// The built-in skills a capability brings to every agent of a chain that lists it.
const CAPABILITY_SKILLS: [(&str, &[&str]); 1] = [(CAP_OKR, &["okr"])];

/// The built-in skill every Chair holds: how to write the scripts its run_workflow runs (ADR-0073).
pub(crate) const CHAIR_SKILL: &str = "workflows";

/// Why a library could not be laid out.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LayOutError {
    #[error("no library directory is configured")]
    NoDirectory,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Write(#[from] skills::WriteError),
}

impl Capabilities {
    /// Renders the stored skills and the skills that ship with Remit at start, and removes every earlier rendering.
    /// Where no library directory is configured the shipped skills are not laid out, and the cards that would hold them
    /// start without.
    pub async fn seed_library(&self) -> Result<()> {
        let _order = self.library.change_mu.lock().await;
        let held = self.store.read(|r| r.skills()).await?;
        let dir = self
            .lay_out_library(skills::PLUGIN_NAME, &held)
            .await
            .map_err(|e| Error::refused(format!("the library's skills cannot be laid out for sessions: {e}")))?;
        let (mut shipped, mut shipped_dir) = (Vec::new(), String::new());
        if !self.cfg.library_dir.as_os_str().is_empty() {
            shipped = builtin::for_assistant()
                .map_err(|e| Error::refused(format!("the skills that ship with Remit cannot be read: {e}")))?;
            shipped_dir = self
                .lay_out_library(skills::BUILTIN_PLUGIN_NAME, &shipped)
                .await
                .map_err(|e| {
                    Error::refused(format!(
                        "the skills that ship with Remit cannot be laid out for sessions: {e}"
                    ))
                })?;
        }
        {
            let mut laid = self.library.laid_out();
            laid.plugin_dir.clone_from(&dir);
            laid.builtin = shipped;
            laid.builtin_dir.clone_from(&shipped_dir);
        }
        let root = self.cfg.library_dir.clone();
        tokio::task::spawn_blocking(move || remove_other_renderings(&root, &[&dir, &shipped_dir]))
            .await
            .map_err(|e| Error::refused(format!("the library's old renderings: {e}")))?
            .map_err(|e| Error::refused(format!("the library's old renderings cannot be removed: {e}")))
    }

    /// The skills that ship with Remit for the Assistant: none where they were not laid out.
    pub fn builtin_for(&self) -> Vec<Skill> {
        let laid = self.library.laid_out();
        if laid.builtin_dir.is_empty() {
            return Vec::new();
        }
        laid.builtin.clone()
    }

    /// Where the plugin `library::access` names is laid out; empty where it is not.
    pub fn plugin_dir(&self, plugin: &str) -> String {
        let laid = self.library.laid_out();
        match plugin {
            ::library::PLUGIN_NAME => laid.plugin_dir.clone(),
            ::library::BUILTIN_PLUGIN_NAME => laid.builtin_dir.clone(),
            _ => String::new(),
        }
    }

    /// The built-in skills card c holds in its chain, in name order: a Chair's own (`CHAIR_SKILL`), and those of a
    /// capability its chain lists. A card of no chain (the Dispatcher) holds none.
    pub async fn builtin_of_chain(&self, c: &Card) -> Result<Vec<Skill>> {
        if c.home.is_empty() {
            return Ok(Vec::new());
        }
        let home = c.home.clone();
        let Some(ch) = optional(self.store.read(move |r| r.chain(&home)).await)? else {
            return Ok(Vec::new());
        };
        let chair = (c.origin == Origin::Chair).then_some(CHAIR_SKILL);
        let names: Vec<&str> = chair
            .into_iter()
            .chain(ch.capabilities.iter().flat_map(|cap| {
                CAPABILITY_SKILLS
                    .iter()
                    .filter(move |(c, _)| c == cap)
                    .flat_map(|(_, skills)| skills.iter().copied())
            }))
            .collect();
        Ok(self.builtin_named(&names))
    }

    /// The shipped skills called names, none where they were not laid out.
    fn builtin_named(&self, names: &[&str]) -> Vec<Skill> {
        let mut out = self.builtin_for();
        out.retain(|s| names.contains(&s.name.as_str()));
        out
    }

    /// Makes sure the plugin directory of list, a plugin called plugin, exists, and returns it; empty for no skill. An
    /// existing one is not written again: a session reading it is never disturbed by a save that changed nothing. The
    /// hash covers the plugin's manifest, so two plugins never share a directory.
    pub(crate) async fn lay_out_library(&self, plugin: &str, list: &[Skill]) -> Result<String, LayOutError> {
        if list.is_empty() {
            return Ok(String::new());
        }
        if self.cfg.library_dir.as_os_str().is_empty() {
            return Err(LayOutError::NoDirectory);
        }
        let files = skills::render(plugin, list);
        let root = self.cfg.library_dir.clone();
        tokio::task::spawn_blocking(move || lay_out_in(&root, &files))
            .await
            .map_err(io::Error::other)?
    }
}

/// Writes the rendering files under root, unless it is there already, and returns its plugin directory.
fn lay_out_in(root: &Path, files: &BTreeMap<String, String>) -> Result<String, LayOutError> {
    let dir = root.join(rendering_name(files));
    let plugin_dir = dir.join("plugin");
    match std::fs::metadata(&plugin_dir) {
        Ok(_) => return Ok(plugin_dir.to_string_lossy().into_owned()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let staging = PathBuf::from(format!("{}{STAGING_SUFFIX}", dir.display()));
    remove_all(&staging)?; // what a crash left
    if let Err(e) = skills::write_tree(&staging.join("plugin"), files) {
        remove_all(&staging)?;
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&staging, &dir) {
        remove_all(&staging)?;
        return Err(e.into());
    }
    Ok(plugin_dir.to_string_lossy().into_owned())
}

/// The first `HASH_LEN` hex digits of a hash over every path and content, in path order. Each part is prefixed with its
/// length, so no two sets of files give the same bytes.
fn rendering_name(files: &BTreeMap<String, String>) -> String {
    let mut sum = Sha256::new();
    for (path, content) in files {
        sum.update(format!("{}:{path}{}:{content}", path.len(), content.len()));
    }
    let hex: String = sum.finalize().iter().map(|b| format!("{b:02x}")).collect();
    hex.chars().take(HASH_LEN).collect()
}

/// Removes everything in root except the renderings that hold the plugins (an empty one: none).
fn remove_other_renderings(root: &Path, plugins: &[&str]) -> io::Result<()> {
    if root.as_os_str().is_empty() {
        return Ok(());
    }
    let found = match std::fs::read_dir(root) {
        Ok(found) => found,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let keep: Vec<_> = plugins
        .iter()
        .filter(|p| !p.is_empty())
        .filter_map(|p| Path::new(p).parent().and_then(Path::file_name).map(ToOwned::to_owned))
        .collect();
    for e in found {
        let e = e?;
        if keep.contains(&e.file_name()) {
            continue;
        }
        remove_all(&e.path())?;
    }
    Ok(())
}

/// Removes path and all it holds; nothing there is no error.
fn remove_all(path: &Path) -> io::Result<()> {
    let removed = match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(e) => Err(e),
    };
    match removed {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
