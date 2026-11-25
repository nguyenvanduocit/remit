//! One process owns the database at a time: a server, for as long as its Store is open, or a restore, for as long as it
//! takes. It holds an exclusive `flock` on `<db>.lock`, so a second process is refused.

use std::fs::{File, OpenOptions, TryLockError};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::store::{Error, Result};

/// The file whose exclusive lock says one process owns the database at path. The file stays; only its lock comes and
/// goes.
fn lock_file(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Takes the lock on the database at path without waiting, and refuses with `Error::InUse` when another holds it.
/// Dropping the returned file lets it go. The kernel holds the lock for the open file, so a process that dies, however
/// it dies, leaves none behind, and a child started after it does not inherit it (std opens files close-on-exec).
pub(crate) fn hold(path: &Path) -> Result<File> {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_file(path))?;
    match f.try_lock() {
        Ok(()) => Ok(f),
        Err(TryLockError::WouldBlock) => Err(Error::InUse(path.to_path_buf())),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}
