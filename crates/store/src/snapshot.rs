//! A snapshot is a consistent copy of the database in `snapshot_dir`, of one of four kinds: one taken before migrating
//! (a build rolled back, or a migration gone wrong, has the database as it was before each of the last few migrations),
//! one the scheduler takes every day (`Store::snapshot_if_due`), so a bad day loses a day at most, the database a
//! restore replaced (`restore`), so putting back the wrong snapshot can itself be undone, and one taken before a new
//! release starts on it (`snapshot_before_update`, run by `remit update`), so a release that writes bad data has the
//! database as it was to go back to.

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use model::Time;
use rusqlite::{Connection, OpenFlags};
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::{PrimitiveDateTime, UtcOffset};

use crate::migrations::MIGRATIONS;
use crate::store::{Error, Result, Store};

/// How many snapshots of each kind stay: the newest. Each kind is pruned apart, so a day of migrations does not push out
/// the daily copies, nor a week of daily copies those taken before migrating.
const KEPT_SNAPSHOTS: usize = 5;

/// `20260102T150405.000Z`: a time that sorts as it reads.
const SNAPSHOT_TIME: &[BorrowedFormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second].[subsecond digits:3]Z");

const PREMIGRATE: &str = "pre-migrate";
pub(crate) const SCHEDULED: &str = "scheduled";
pub(crate) const PRERESTORE: &str = "pre-restore";
const PREUPDATE: &str = "pre-update";

/// Marks a copy being written: it is named as a snapshot only once whole, so a copy cut short by a crash is never taken
/// for one, and the next prune removes it.
pub(crate) const PART_SUFFIX: &str = ".part";

/// One snapshot of a database, as its file's name tells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Its path.
    pub file: PathBuf,
    /// pre-migrate, scheduled, pre-restore or pre-update.
    pub kind: String,
    /// The schema version of the database it holds (a pre-migrate snapshot holds the version it was migrated from).
    pub version: i64,
    /// When it was taken.
    pub at: Time,
    /// A copy that was never finished.
    part: bool,
}

/// Where the snapshots of the database at path are kept: backups, beside it.
pub fn snapshot_dir(path: &Path) -> PathBuf {
    path.parent().unwrap_or(Path::new(".")).join("backups")
}

fn stamp(at: Time) -> String {
    at.0.to_offset(UtcOffset::UTC).format(SNAPSHOT_TIME).unwrap_or_default()
}

pub(crate) fn kind_name(kind: &str, version: i64, at: Time) -> String {
    format!("{kind}-v{version}-{}.db", stamp(at))
}

fn premigrate_name(from: i64, to: i64, at: Time) -> String {
    format!("{PREMIGRATE}-v{from}-to-v{to}-{}.db", stamp(at))
}

/// The snapshot a file's name tells, None for a file named otherwise, which is not one and is never deleted:
/// `pre-migrate-v<from>-to-v<to>-<time>.db`, `scheduled-v<n>-<time>.db`, `pre-restore-v<n>-<time>.db` or
/// `pre-update-v<n>-<time>.db`, `.part` after
/// it while it is being written.
fn parse_name(name: &str) -> Option<Snapshot> {
    let (rest, part) = match name.strip_suffix(PART_SUFFIX) {
        Some(rest) => (rest, true),
        None => (name, false),
    };
    let rest = rest.strip_suffix(".db")?;
    let (head, at) = rest.rsplit_once('-')?;
    let at = PrimitiveDateTime::parse(at, SNAPSHOT_TIME).ok()?.assume_utc();
    let digits = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<i64>().ok())
            .flatten()
    };
    let (kind, version) = if let Some(versions) = head.strip_prefix("pre-migrate-v") {
        let (from, to) = versions.split_once("-to-v")?;
        digits(to)?;
        (PREMIGRATE, digits(from)?)
    } else if let Some(v) = head.strip_prefix("scheduled-v") {
        (SCHEDULED, digits(v)?)
    } else if let Some(v) = head.strip_prefix("pre-update-v") {
        (PREUPDATE, digits(v)?)
    } else {
        (PRERESTORE, digits(head.strip_prefix("pre-restore-v")?)?)
    };
    Some(Snapshot {
        file: PathBuf::from(name),
        kind: kind.to_owned(),
        version,
        at: Time(at),
        part,
    })
}

/// The snapshots among names, the newest first, each file its name alone.
fn snapshots_in(names: &[String]) -> Vec<Snapshot> {
    let mut snaps: Vec<Snapshot> = names.iter().filter_map(|n| parse_name(n)).collect();
    snaps.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.file.cmp(&a.file)));
    snaps
}

/// The snapshots among names that are older than the newest keep of their kind, and the copies never finished.
fn stale_snapshots(names: &[String], keep: usize) -> Vec<PathBuf> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut stale = Vec::new();
    for s in snapshots_in(names) {
        if s.part {
            stale.push(s.file);
            continue;
        }
        let n = seen.entry(s.kind).or_default();
        *n = n.saturating_add(1);
        if *n > keep {
            stale.push(s.file);
        }
    }
    stale
}

/// Whether a scheduled snapshot is due at now: none among names was taken less than every before.
fn scheduled_due(names: &[String], now: Time, every: time::Duration) -> bool {
    snapshots_in(names)
        .into_iter()
        .find(|s| s.kind == SCHEDULED && !s.part)
        .is_none_or(|s| s.at.0.checked_add(every).is_none_or(|due| now.0 >= due))
}

/// The snapshots of the database at path, the newest first; none when it has no backups directory.
pub fn snapshots(path: &Path) -> Result<Vec<Snapshot>> {
    let dir = snapshot_dir(path);
    let names = match file_names(&dir) {
        Err(Error::Io(e)) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        other => other?,
    };
    Ok(snapshots_in(&names)
        .into_iter()
        .filter(|s| !s.part)
        .map(|s| Snapshot {
            file: dir.join(&s.file),
            ..s
        })
        .collect())
}

impl Store {
    /// Copies the database into `snapshot_dir` when its newest scheduled snapshot was taken every or longer before now,
    /// or there is none, and keeps the newest of each kind. It returns the file it wrote, None when none was due. The
    /// copy is read on a read-only connection of its own, never the writer's: VACUUM INTO holds a read transaction for
    /// as long as the copy takes, which holds back the WAL's checkpoint and no write. One runs at a time: a caller that
    /// comes while one runs waits for it, and then finds none due.
    pub async fn snapshot_if_due(&self, now: Time, every: time::Duration) -> Result<Option<PathBuf>> {
        let _running = self.snapshotting.lock().await;
        let path = self.path().to_path_buf();
        tokio::task::spawn_blocking(move || {
            let dir = snapshot_dir(&path);
            let names = match file_names(&dir) {
                Err(Error::Io(e)) if e.kind() == ErrorKind::NotFound => Vec::new(),
                other => other?,
            };
            if !scheduled_due(&names, now, every) {
                return Ok(None);
            }
            make_dir(&dir)?;
            #[expect(
                clippy::disallowed_methods,
                reason = "the store owns the database: this is one of its own opens"
            )]
            let ro = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            ro.busy_timeout(std::time::Duration::from_millis(5000))?;
            let known = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
            let file = dir.join(kind_name(SCHEDULED, known, now));
            vacuum_into(&ro, &file)?;
            prune_snapshots(&dir)?;
            Ok(Some(file))
        })
        .await
        .map_err(|_| Error::Closed)?
    }
}

/// Copies the database at path into `snapshot_dir` before a new release starts on it, and keeps the newest of each kind;
/// the copy's path. It reads on a read-only connection of its own, so it runs beside the server that writes the
/// database, holding back no write.
pub fn snapshot_before_update(path: &Path, now: Time) -> Result<PathBuf> {
    let ro = open_read_only(path)?;
    let version: i64 = ro.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let dir = snapshot_dir(path);
    make_dir(&dir)?;
    let file = dir.join(kind_name(PREUPDATE, version, now));
    vacuum_into(&ro, &file)?;
    prune_snapshots(&dir)?;
    Ok(file)
}

/// The schema version of the database at path (the migrations it ran), read as a server that writes it beside sees
/// it, write-ahead log included: `remit update` compares it before and after a release starts to tell whether the
/// release migrated it.
pub fn schema_version(path: &Path) -> Result<i64> {
    Ok(open_read_only(path)?.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// A read-only connection of its own on the database at path, which holds back no write of the server beside it.
fn open_read_only(path: &Path) -> Result<Connection> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the store owns the database: this is one of its own opens"
    )]
    let ro = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    ro.busy_timeout(std::time::Duration::from_millis(5000))?;
    Ok(ro)
}

/// Copies c into dir when an earlier build wrote it and this one has migrations to apply: a migration that goes wrong, or
/// a build rolled back (which refuses a database migrated past what it knows), has the database as it was to go back
/// to. A new database has nothing to lose and one with nothing pending will not change, so neither is copied. It reads
/// the version outside the migration's transaction because SQLite runs VACUUM in none: the migration reads it again.
pub(crate) fn before_migrating(c: &Connection, dir: &Path, known: usize, now: Time) -> Result<()> {
    let version: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let known = i64::try_from(known).unwrap_or(i64::MAX);
    if version == 0 || version >= known {
        return Ok(());
    }
    make_dir(dir)?;
    vacuum_into(c, &dir.join(premigrate_name(version, known, now)))?;
    prune_snapshots(dir)
}

pub(crate) fn make_dir(dir: &Path) -> Result<()> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    Ok(())
}

use std::os::unix::fs::DirBuilderExt;

/// with_suffix is path with suffix appended to its name.
pub(crate) fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Writes a consistent copy of c to file, which must not exist yet: under file+PART_SUFFIX, renamed to file once whole.
/// The copy is made owner-only before SQLite fills it, because SQLite would create it readable by anyone the umask lets
/// in.
fn vacuum_into(c: &Connection, file: &Path) -> Result<()> {
    let part = with_suffix(file, PART_SUFFIX);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&part)?;
    let done = (|| {
        c.execute("VACUUM INTO ?", [part.to_string_lossy()])?;
        if file.exists() {
            return Err(Error::Refused(format!("snapshot {} exists already", file.display())));
        }
        std::fs::rename(&part, file)?;
        Ok(())
    })();
    if done.is_err() {
        std::fs::remove_file(&part).ok();
    }
    done
}

pub(crate) fn prune_snapshots(dir: &Path) -> Result<()> {
    for name in stale_snapshots(&file_names(dir)?, KEPT_SNAPSHOTS) {
        std::fs::remove_file(dir.join(name))?;
    }
    Ok(())
}

fn file_names(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for e in std::fs::read_dir(dir)? {
        names.push(e?.file_name().to_string_lossy().into_owned());
    }
    Ok(names)
}
