//! Installing a release on this server (ADR-0087, ADR-0116): the one door that restarts it. A release of the repository
//! this server runs (`-update-repo`) runs it as one more step after its procedure, to the version the procedure
//! reported, and a release taken back there runs it to `previous`. It starts `remit update` apart from the server and
//! returns at once; the update backs up the database, installs, restarts and goes back by itself when the release is not
//! healthy, and says what it did as `fact:remit.updated`. A one-way release (one that migrated) is not gone back over:
//! `remit update` refuses, and the way back is the release forward of the reverts.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::Shipping;

/// How this server runs `remit update` to install a release on itself (-update-restart): the program and its arguments
/// but `-to`, which `install_release` adds, and the files the update holds its lock in and appends its output to. The
/// server's own crate fills it, as it alone knows `remit update`'s flags; none: nothing installs a release here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Updater {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The file whose lock says an update is under way.
    pub lock: PathBuf,
    /// Where the update's output is appended.
    pub log: PathBuf,
    /// The names of the server's environment the update takes besides those every child takes (`childenv::of`): its
    /// login to the forge and its supervisor's home, which `childenv` keeps from every other child.
    pub inherits: Vec<String>,
}

/// What `install_release` takes for "the release before the one installed": the link `remit update` keeps in the
/// releases directory.
pub const PREVIOUS: &str = "previous";
/// The link to the release installed now.
const CURRENT: &str = "current";

/// Why install_release starts no update while one runs.
const UNDER_WAY: &str = "an update is under way on this server: wait for fact:remit.updated";

/// The tag `to` names: a release's tag as given, or the one the `previous` link points at. Refused when it names
/// nothing, or could be read as a flag of `remit update`. Pure.
fn target_of(to: &str, previous: Option<&str>) -> Result<String, String> {
    let to = to.trim();
    let tag = if to == PREVIOUS {
        previous.ok_or("there is no previous release to go back to on this server")?
    } else {
        to
    };
    if tag.is_empty() || tag.starts_with('-') || tag.chars().any(|c| c.is_whitespace() || c == '/') {
        return Err(format!(
            "{tag:?} is not a release's tag: the procedure reports the tag of the release it waited for \
             (<yyyymmdd>-<commit>), or previous"
        ));
    }
    Ok(tag.to_owned())
}

/// The release the link name in the releases directory points at.
fn linked(releases: &Path, name: &str) -> Option<String> {
    let to = std::fs::read_link(releases.join(name)).ok()?;
    Some(to.file_name()?.to_string_lossy().into_owned())
}

/// The entries of parent (`NAME=value`) whose names are in names. Pure.
fn update_env(parent: &[String], names: &[String]) -> Vec<String> {
    parent
        .iter()
        .filter(|kv| {
            kv.split_once('=')
                .is_some_and(|(name, _)| names.iter().any(|n| n == name))
        })
        .cloned()
        .collect()
}

/// Says no update holds the lock at path; the lock is let go at once, as the update takes it itself.
fn no_update_under_way(path: &Path) -> Result<(), String> {
    let f: File = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(format!("{}: {err}", path.display())),
    };
    match f.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(UNDER_WAY.to_owned()),
        Err(TryLockError::Error(err)) => Err(format!("{}: {err}", path.display())),
    }
}

impl Shipping {
    /// Starts `remit update` to tag `to` (or `previous`) apart from the server, and says what it started: the update
    /// restarts this server. Refused on a server not started with -update-restart, and while an update is under way.
    pub(crate) async fn install_release(&self, to: &str) -> Result<String, String> {
        if self.is_stopping() {
            return Err("the server is stopping".to_owned());
        }
        let Some(up) = self.updater.as_ref() else {
            return Err(
                "this server was not started with -update-restart (it does not run from releases, as a dev instance \
                 runs from a working copy): nothing is installed here"
                    .to_owned(),
            );
        };
        let releases = &self.cfg.releases_dir;
        let tag = target_of(to, linked(releases, PREVIOUS).as_deref())?;
        let from = linked(releases, CURRENT).unwrap_or_default();
        // Held until the update is started and its pid kept: the update takes its lock only once it runs, so the pid
        // stands for it until then.
        let mut started = self.installing.lock().await;
        if started.is_some_and(procgroup::running) {
            return Err(UNDER_WAY.to_owned());
        }
        no_update_under_way(&up.lock)?;
        let parent: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
        let env = childenv::of(&parent, &update_env(&parent, &up.inherits));
        let mut args = up.args.clone();
        args.extend(["-to".to_owned(), tag.clone()]);
        let pid = procgroup::detach(&up.program, &args, &env, &up.log)
            .await
            .map_err(|err| format!("remit update did not start: {err}"))?;
        *started = Some(pid);
        Ok(if from.is_empty() {
            format!("Installing release {tag}: remit update runs as pid {pid}; this server restarts.")
        } else {
            format!("Installing release {tag} over {from}: remit update runs as pid {pid}; this server restarts.")
        })
    }
}
