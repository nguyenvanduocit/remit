use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::validate::{PathError, parents, valid_path};

const DIR_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;

/// Why a tree was not written.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("write {root}: {source}", root = root.display())]
    Path { root: PathBuf, source: PathError },
    #[error("write {root}: {source}", root = root.display())]
    Io { root: PathBuf, source: io::Error },
    #[error("write {root}: {source}; and what was written could not be removed: {cleanup}", root = root.display())]
    LeftBehind {
        root: PathBuf,
        source: io::Error,
        cleanup: io::Error,
    },
}

/// Writes `files` (path to content, as `render` returns) under `root`, which must not exist. It leaves nothing behind when
/// it fails: a half-written plugin directory would be loaded as it is.
pub fn write_tree(root: &Path, files: &BTreeMap<String, String>) -> Result<(), WriteError> {
    for p in files.keys() {
        valid_path(p).map_err(|source| WriteError::Path {
            root: root.to_owned(),
            source,
        })?;
    }
    let io_err = |source| WriteError::Io {
        root: root.to_owned(),
        source,
    };
    if let Some(parent) = root.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    // root exists or cannot be made: nothing of ours to remove.
    mkdir(root).map_err(io_err)?;
    write_files(root, files).map_err(|source| match fs::remove_dir_all(root) {
        Ok(()) => WriteError::Io {
            root: root.to_owned(),
            source,
        },
        Err(cleanup) => WriteError::LeftBehind {
            root: root.to_owned(),
            source,
            cleanup,
        },
    })
}

fn write_files(root: &Path, files: &BTreeMap<String, String>) -> io::Result<()> {
    // A directory sorts before what it holds, so each parent exists when its child is made.
    let dirs: BTreeSet<&str> = files.keys().flat_map(|p| parents(p)).collect();
    for dir in dirs {
        mkdir(&root.join(dir))?;
    }
    for (p, content) in files {
        let name = root.join(p);
        fs::write(&name, content)?;
        // The umask only ever narrows the mode a file is made with.
        fs::set_permissions(&name, Permissions::from_mode(FILE_MODE))?;
    }
    Ok(())
}

/// Makes one directory with `DIR_MODE` whatever the umask is.
fn mkdir(dir: &Path) -> io::Result<()> {
    fs::create_dir(dir)?;
    fs::set_permissions(dir, Permissions::from_mode(DIR_MODE))
}
