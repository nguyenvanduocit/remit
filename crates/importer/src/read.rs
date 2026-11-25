//! The reads every import goes through: what is inside the configuration directory, and how much of a file is taken.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::Error;

/// Resolves a path under `config_dir`, following links, and refuses one that leaves the directory. It is the only place
/// that decides what is inside.
pub(crate) fn confine(config_dir: &Path, elems: &[&str]) -> Result<PathBuf, Error> {
    if config_dir.as_os_str().is_empty() {
        return Err(Error::NoConfigDir);
    }
    let root = config_dir.canonicalize().map_err(|e| Error::io(config_dir, e))?;
    let joined: PathBuf = elems.iter().collect();
    let unresolved = root.join(&joined);
    let target = unresolved.canonicalize().map_err(|e| Error::io(&unresolved, e))?;
    if !target.starts_with(&root) {
        return Err(Error::Outside {
            path: joined.display().to_string(),
            target: target.display().to_string(),
        });
    }
    Ok(target)
}

/// Resolves one entry of a kind's directory. The directory itself must be inside the configuration directory; the entry
/// may be a link to anywhere, because managed installs link their skills in from elsewhere. Only the entry is followed:
/// inside a skill nothing is (`read_skill`), so a link can never bring in a file the skill's directory does not hold.
pub(crate) fn entry_target(config_dir: &Path, kind: &str, name: &str) -> Result<PathBuf, Error> {
    let entry = confine(config_dir, &[kind])?.join(name);
    entry.canonicalize().map_err(|e| Error::io(&entry, e))
}

/// Refuses a name that is not one file name: with a separator or ".." it could reach a file outside the directory it is
/// looked up in.
pub(crate) fn check_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || name == "." || name.contains("..") || name.contains(['/', '\\', '\0']) {
        return Err(Error::BadName(name.to_owned()));
    }
    Ok(())
}

/// Reads a file of at most `limit` bytes. A larger one is refused, not truncated: half of an agent's instructions is
/// worse than none.
pub(crate) fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    let file = File::open(path).map_err(|e| Error::io(path, e))?;
    let cap = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut data = Vec::new();
    file.take(cap).read_to_end(&mut data).map_err(|e| Error::io(path, e))?;
    if data.len() > limit {
        return Err(Error::TooBig(limit));
    }
    Ok(data)
}
