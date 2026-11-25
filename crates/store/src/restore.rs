//! Restoring puts a snapshot back as the database, while no server runs on it (`lock::hold`), in an order a crash at any
//! step leaves either the database it replaces or the snapshot in its place, never a mix: the snapshot is copied beside
//! the database and checked there, the database is copied into `snapshot_dir` as a pre-restore snapshot, its write-ahead
//! files are removed, and the checked copy is renamed over it.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use model::Time;
use rusqlite::{Connection, OpenFlags};

use crate::lock;
use crate::migrations::MIGRATIONS;
use crate::snapshot::{PART_SUFFIX, PRERESTORE, kind_name, make_dir, prune_snapshots, snapshot_dir, with_suffix};
use crate::store::{Error, Result};

/// Names the copy of a snapshot being restored, beside the database. A copy a crash cut short is never the database, and
/// the next restore removes it.
const RESTORE_SUFFIX: &str = ".restore.part";

/// Begins every SQLite database file; the schema version a migration sets (PRAGMA user_version) is the four big-endian
/// bytes at USER_VERSION_AT of its header.
const SQLITE_HEADER: &[u8] = b"SQLite format 3\x00";
const USER_VERSION_AT: usize = 60;

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The snapshot put back.
    pub from: PathBuf,
    /// The schema version of the database it holds.
    pub version: i64,
    /// The schema version of this build, which the next open migrates the database to.
    pub build: i64,
    /// The pre-restore snapshot the replaced database was copied to; None when there was none.
    pub kept: Option<PathBuf>,
    /// The write-ahead log kept beside kept, when the replaced database could not take it in.
    pub kept_log: Option<PathBuf>,
}

/// Puts the snapshot from back as the database at path, keeping the database it replaces in `snapshot_dir`. It refuses
/// with `Error::InUse` while a server holds the database, and refuses, before the database is touched, a from that is not
/// a whole Remit database this build can migrate. A copy an earlier restore left half done is removed first.
pub fn restore(path: &Path, from: &Path, now: Time) -> Result<Restored> {
    let _lock = lock::hold(path)?;
    let part = with_suffix(path, RESTORE_SUFFIX);
    let part_files = [part.clone(), with_suffix(&part, "-wal"), with_suffix(&part, "-shm")];
    remove_all(&part_files).map_err(|e| Error::Refused(format!("remove the copy an earlier restore left: {e}")))?;
    check_source(path, from)?;
    let version = match copy_checked(from, &part) {
        Ok(v) => v,
        Err(e) => {
            remove_all(&part_files).ok();
            return Err(e);
        }
    };
    let build = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    let (kept, kept_log) = match keep_replaced(path, now) {
        Ok(k) => k,
        Err(e) => {
            remove_all(&part_files).ok();
            return Err(Error::Refused(format!("keep the database it replaces: {e}")));
        }
    };
    // The log of the replaced database goes before the copy takes its name: replayed onto the copy, it would bring back
    // the rows the restore takes away, or damage it.
    let moved = remove_all(&[with_suffix(path, "-wal"), with_suffix(path, "-shm")])
        .and_then(|()| Ok(std::fs::rename(&part, path)?));
    if let Err(e) = moved {
        remove_all(&part_files).ok();
        return Err(e);
    }
    File::open(path.parent().unwrap_or(Path::new(".")))?.sync_all()?;
    if kept.is_some() {
        prune_snapshots(&snapshot_dir(path))?;
    }
    Ok(Restored {
        from: from.to_path_buf(),
        version,
        build,
        kept,
        kept_log,
    })
}

/// Refuses a from that cannot be read whole as a file: one that is not a regular file, the database itself, or one with
/// a write-ahead log beside it, whose rows a copy of the file alone would leave out.
fn check_source(path: &Path, from: &Path) -> Result<()> {
    let info = std::fs::metadata(from)?;
    if !info.is_file() {
        return Err(Error::Refused(format!("{} is not a file", from.display())));
    }
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(from), std::fs::canonicalize(path))
        && a == b
    {
        return Err(Error::Refused(format!("{} is the database itself", from.display())));
    }
    if std::fs::metadata(with_suffix(from, "-wal")).is_ok_and(|l| l.len() > 0) {
        return Err(Error::Refused(format!(
            "{0} has a write-ahead log beside it whose rows a copy of the file would leave out: copy it whole first (sqlite3 {0} \".backup <file>\")",
            from.display()
        )));
    }
    Ok(())
}

/// Copies from to part, owner-only and on disk, and returns the schema version of the copy once it is a whole Remit
/// database this build migrates: SQLite's header, a version from 1 to this build's, and an integrity check that passes.
/// It checks the copy, not from, so what is checked is what is put back.
fn copy_checked(from: &Path, part: &Path) -> Result<i64> {
    copy_file(from, part)?;
    let version = header_version(part).map_err(|e| Error::Refused(format!("{}: {e}", from.display())))?;
    if version < 1 {
        return Err(Error::Refused(format!(
            "{} holds no Remit database: its schema version is {version}",
            from.display()
        )));
    }
    let known = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    if version > known {
        return Err(Error::Refused(format!(
            "{} is at schema version {version}, newer than this build's {known}: restore it with the build that wrote it",
            from.display()
        )));
    }
    integrity_check(part).map_err(|e| Error::Refused(format!("{}: {e}", from.display())))?;
    // Opening the copy may leave SQLite's files beside it, which would follow it as the database's.
    remove_all(&[with_suffix(part, "-wal"), with_suffix(part, "-shm")])?;
    Ok(version)
}

/// The schema version from the header of the database file at path, read without SQLite: it reads a database SQLite
/// cannot open, and refuses a file that is not one, which SQLite would take for an empty database when it is empty.
fn header_version(path: &Path) -> Result<i64> {
    let mut header = [0_u8; 64];
    let read = File::open(path)?.read_exact(&mut header);
    let version = header.get(USER_VERSION_AT..).and_then(|b| <[u8; 4]>::try_from(b).ok());
    match (read, version) {
        (Ok(()), Some(v)) if header.starts_with(SQLITE_HEADER) => Ok(i64::from(u32::from_be_bytes(v))),
        _ => Err(Error::Refused("not a SQLite database".into())),
    }
}

/// SQLite's full integrity check on the database at path, read-only. The full check, not quick_check: a restore is rare
/// and runs with no server waiting, and quick_check leaves out whether each index matches its table.
fn integrity_check(path: &Path) -> Result<()> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the store owns the database: this is one of its own opens"
    )]
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt = c.prepare("PRAGMA integrity_check")?;
    let lines = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let problems: Vec<String> = lines.into_iter().filter(|l| l != "ok").collect();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::Refused(format!("damaged: {}", problems.join("; "))))
    }
}

/// Copies the database at path into `snapshot_dir` as a pre-restore snapshot and returns its path, None when there is
/// no database. The write-ahead log is checkpointed into the file first, so the copy is whole; a database SQLite cannot
/// checkpoint (a damaged one) has its log copied beside the copy instead, and returned too.
fn keep_replaced(path: &Path, now: Time) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
    if !path.exists() {
        return Ok((None, None));
    }
    let checkpointed = checkpoint(path);
    // A damaged database is kept all the same, under the version it can no longer tell.
    let version = header_version(path).unwrap_or(0);
    let dir = snapshot_dir(path);
    make_dir(&dir)?;
    let kept = dir.join(kind_name(PRERESTORE, version, now));
    copy_as(path, &kept)?;
    let log = with_suffix(path, "-wal");
    if checkpointed.is_ok() || !std::fs::metadata(&log).is_ok_and(|l| l.len() > 0) {
        return Ok((Some(kept), None));
    }
    let kept_log = with_suffix(&kept, "-wal");
    if let Err(e) = copy_as(&log, &kept_log) {
        std::fs::remove_file(&kept).ok();
        return Err(e);
    }
    Ok((Some(kept), Some(kept_log)))
}

/// Moves every row in the write-ahead log of the database at path into the file and empties the log.
fn checkpoint(path: &Path) -> Result<()> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the store owns the database: this is one of its own opens"
    )]
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_millis(5000))?;
    let busy: i64 = c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
    if busy != 0 {
        return Err(Error::Refused(
            "the write-ahead log could not be checkpointed: another connection reads it".into(),
        ));
    }
    Ok(())
}

/// Copies src to dst, which must not exist yet, under dst+PART_SUFFIX renamed to dst once whole.
fn copy_as(src: &Path, dst: &Path) -> Result<()> {
    let part = with_suffix(dst, PART_SUFFIX);
    let done = copy_file(src, &part).and_then(|()| {
        if dst.exists() {
            return Err(Error::Refused(format!("{} exists already", dst.display())));
        }
        Ok(std::fs::rename(&part, dst)?)
    });
    if done.is_err() {
        remove_all(&[part]).ok();
    }
    done
}

/// Copies src to dst, which must not exist yet, owner-only, and syncs it to disk.
fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    let mut input = File::open(src)?;
    let mut out = OpenOptions::new().write(true).create_new(true).mode(0o600).open(dst)?;
    std::io::copy(&mut input, &mut out)?;
    out.sync_all()?;
    Ok(())
}

/// Removes the files that exist among files.
fn remove_all(files: &[PathBuf]) -> Result<()> {
    for f in files {
        match std::fs::remove_file(f) {
            Err(e) if e.kind() != ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}
