//! The agents' memory at start: what older Remits kept elsewhere comes into the store once (ADR-0117). The memory the
//! employees once kept as files goes to the backups; the database of the memory daemon Remit once ran is imported, each of
//! its memories under the holder its project named, then moved to the backups. Each step finds nothing the second time,
//! and the files are read and moved on blocking threads, so the server's runtime is never held by the disk.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use model::{Card, Memory, MemoryEnd, MemoryKind, MemoryScope, MemoryTrust, Time};
use sha2::{Digest, Sha256};
use store::{DaemonMemory, Store};

/// Brings what older Remits kept of memory into the store before any session starts: the employees' memory files go to
/// the backups beside database, and the daemon's database in old_dir is imported, then moved there too.
pub async fn start(
    store: &Store,
    data_dir: &Path,
    old_dir: &Path,
    database: &Path,
    now: time::OffsetDateTime,
) -> Result<(), String> {
    let (data, db) = (data_dir.to_path_buf(), database.to_path_buf());
    let n = blocking(move || archive_file_memory(&data, &db, now).map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("archive the employees' file memory: {e}"))?;
    if n > 0 {
        tracing::info!(
            files = n,
            "memory: the employees' memory files are archived in the backups and removed"
        );
    }
    let n = import_daemon_memory(store, old_dir, database, now)
        .await
        .map_err(|e| format!("import the memory daemon's database {}: {e}", old_dir.display()))?;
    if n > 0 {
        tracing::info!(memories = n, "memory: the memory daemon's memories are imported");
    }
    Ok(())
}

/// What work returns, run on a blocking thread.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(work).await.map_err(|e| e.to_string())?
}

/// The card a teaching's source names (`card:<id> ...`), if it names one. Pure.
fn card_of(source: &str) -> Option<&str> {
    source.strip_prefix("card:")?.split_whitespace().next()
}

/// source in words, about card (its name), taught by teacher, as a teaching recorded today says it (teach_memory.rs): an
/// accepted card, one sent back, a steer, a message; any other source as it is. Pure.
fn source_label(source: &str, card: Option<&Card>, teacher: &str) -> String {
    let Some(c) = card else {
        return source.to_owned();
    };
    let rest = source
        .strip_prefix("card:")
        .and_then(|s| s.split_once(' '))
        .map_or("", |(_, rest)| rest);
    if rest == "accepted" {
        format!("an accepted card: {}", c.name)
    } else if rest == "sent back" && !teacher.is_empty() {
        format!("a card {teacher} sent back: {}", c.name)
    } else if rest == "sent back" {
        format!("a card sent back: {}", c.name)
    } else if let Some(by) = rest.strip_prefix("steered by ").and_then(|s| s.split(" (event").next()) {
        format!("{by} steered {}", c.name)
    } else if !teacher.is_empty() {
        format!("what {teacher} told {}", c.name)
    } else {
        format!("what was said to {}", c.name)
    }
}

/// m as a memory of the store, its holder hired from template, taught on card when the store still holds it: what the
/// daemon held under project is held by project (an employee, a mentor or `assistant`); its scope project (the holder's)
/// is own, stack (the template's) lineage, global everyone; its state confirmed is a person's; rejected, done and
/// abandoned are retracted. A field the daemon left empty reads as empty. Its id is derived from the daemon's, so an import
/// run again names the same memories. Pure.
fn imported(m: &DaemonMemory, template: &str, card: Option<&Card>) -> Memory {
    let at =
        |secs: i64| Time(time::OffsetDateTime::from_unix_timestamp(secs).unwrap_or(time::OffsetDateTime::UNIX_EPOCH));
    let text = |s: &Option<String>| s.as_deref().unwrap_or_default().trim().to_owned();
    let digest = Sha256::digest(format!("memoryd:{}", m.id).as_bytes());
    let (state, status) = (
        m.state.as_deref().unwrap_or_default(),
        m.status.as_deref().unwrap_or("active"),
    );
    let ended = at(m.updated_at);
    let end = match (state, status) {
        ("rejected", _) => MemoryEnd::Retracted {
            at: ended,
            why: "rejected in the memory daemon".to_owned(),
        },
        (_, "active") => MemoryEnd::Active,
        // its successor is the daemon's, which no id here names
        (_, "superseded") => MemoryEnd::Superseded {
            by: String::new(),
            at: ended,
            why: "superseded in the memory daemon".to_owned(),
        },
        (_, other) => MemoryEnd::Retracted {
            at: ended,
            why: format!("{other} in the memory daemon"),
        },
    };
    let source = text(&m.source);
    Memory {
        id: format!("mem_{}", hex::encode(digest.get(..6).unwrap_or_default())),
        holder: m.project.clone(),
        template: template.to_owned(),
        chain: card.map(|c| c.home.clone()).unwrap_or_default(),
        card: card.map(|c| c.id.clone()).unwrap_or_default(),
        source_label: source_label(&source, card, &text(&m.teacher)),
        kind: text(&m.kind).parse::<MemoryKind>().unwrap_or_default(),
        scope: match text(&m.scope).as_str() {
            "stack" => MemoryScope::Lineage,
            "global" => MemoryScope::Everyone,
            _ => MemoryScope::Own,
        },
        trust: if state == "confirmed" {
            MemoryTrust::Confirmed
        } else {
            MemoryTrust::Auto
        },
        end,
        title: text(&m.title),
        body: text(&m.body),
        evidence: text(&m.evidence),
        source,
        teacher: text(&m.teacher),
        retaught: serde_json::from_str(m.retaught.as_deref().unwrap_or("[]")).unwrap_or_default(),
        created_at: at(m.created_at),
        updated_at: ended,
        ..Memory::default()
    }
}

/// Imports the daemon's database in old_dir into store, then moves old_dir into the backups beside database: each memory
/// not stored yet, so one the store already holds (a run cut between the import and the move) keeps what was learned of it
/// since. Nothing to do when there is no database there. Returns how many memories it wrote.
async fn import_daemon_memory(
    store: &Store,
    old_dir: &Path,
    database: &Path,
    now: time::OffsetDateTime,
) -> Result<usize, String> {
    let db = old_dir.join("memory.db");
    if !db.is_file() {
        return Ok(0);
    }
    let old = blocking(move || store::read_memory_daemon(&db).map_err(|e| e.to_string())).await?;
    let n = store
        .update(move |tx| -> store::Result<usize> {
            let mut templates: HashMap<String, String> = HashMap::new();
            let mut cards: HashMap<String, Option<Card>> = HashMap::new();
            let ids: Vec<String> = old.iter().map(|m| imported(m, "", None).id).collect();
            let stored: HashSet<String> = tx.memories_named(&ids)?.into_iter().map(|m| m.id).collect();
            let mut n = 0_usize;
            for (m, id) in old.iter().zip(&ids) {
                if stored.contains(id) {
                    continue;
                }
                let template = match templates.get(&m.project) {
                    Some(t) => t.clone(),
                    None => {
                        let t = match tx.employee(&m.project) {
                            Ok(e) => e.template,
                            Err(store::Error::NotFound) => String::new(),
                            Err(err) => return Err(err),
                        };
                        templates.insert(m.project.clone(), t.clone());
                        t
                    }
                };
                let card = match card_of(m.source.as_deref().unwrap_or_default()) {
                    Some(id) => match cards.get(id) {
                        Some(c) => c.clone(),
                        None => {
                            let c = match tx.card(id) {
                                Ok(c) => Some(c),
                                Err(store::Error::NotFound) => None,
                                Err(err) => return Err(err),
                            };
                            cards.insert(id.to_owned(), c.clone());
                            c
                        }
                    },
                    None => None,
                };
                tx.put_memory(&imported(m, &template, card.as_ref()))?;
                n = n.saturating_add(1);
            }
            Ok(n)
        })
        .await
        .map_err(|e| e.to_string())?;
    let (old_dir, database) = (old_dir.to_path_buf(), database.to_path_buf());
    blocking(move || {
        let backups = store::snapshot_dir(&database);
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&backups)
            .map_err(|e| e.to_string())?;
        let to = backups.join(format!("memory-daemon-{}", stamp(now)?));
        std::fs::rename(&old_dir, &to).map_err(|e| format!("move it to {}: {e}", to.display()))
    })
    .await?;
    Ok(n)
}

/// now as a backup's name carries it.
fn stamp(now: time::OffsetDateTime) -> Result<String, String> {
    now.to_offset(time::UtcOffset::UTC)
        .format(time::macros::format_description!(
            "[year][month][day]T[hour][minute][second]Z"
        ))
        .map_err(|e| e.to_string())
}

/// Moves the memory employees kept as Claude Code's auto memory, `<data>/employees/<id>/memory`, into one tarball in the
/// backups beside the database, then removes the directories. With none left it does nothing, so every start after the
/// first passes through. The tarball is checked to hold every file before anything is removed.
fn archive_file_memory(data_dir: &Path, database: &Path, now: time::OffsetDateTime) -> std::io::Result<usize> {
    let dirs = memory_dirs(data_dir)?;
    if dirs.is_empty() {
        return Ok(0);
    }
    let backups = store::snapshot_dir(database);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&backups)?;
    let path = backups.join(format!("memory-{}.tar.gz", stamp(now).map_err(std::io::Error::other)?));
    let written = write_tarball(&path, data_dir, &dirs)?;
    let found = dirs
        .iter()
        .map(|d| regular_files(d).map(|f| f.len()))
        .sum::<std::io::Result<usize>>()?;
    if found != written {
        return Err(std::io::Error::other(format!(
            "the memory tarball {} holds {written} files of {found}: nothing is removed",
            path.display()
        )));
    }
    for d in &dirs {
        std::fs::remove_dir_all(d)?;
    }
    Ok(written)
}

/// `<data>/employees/*/memory` that are directories.
fn memory_dirs(data_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let employees = data_dir.join("employees");
    let entries = match std::fs::read_dir(&employees) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut dirs = Vec::new();
    for e in entries {
        let m = e?.path().join("memory");
        if m.is_dir() {
            dirs.push(m);
        }
    }
    dirs.sort();
    Ok(dirs)
}

/// The regular files under dir, at any depth.
fn regular_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d)? {
            let e = e?;
            let t = e.file_type()?;
            if t.is_dir() {
                todo.push(e.path());
            } else if t.is_file() {
                out.push(e.path());
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Writes the regular files under dirs into a gzipped tarball at path, named relative to root, and says how many. It is
/// written beside path and renamed onto it, so a tarball at path is whole.
fn write_tarball(path: &Path, root: &Path, dirs: &[PathBuf]) -> std::io::Result<usize> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".partial");
    let tmp = PathBuf::from(tmp);
    let written = (|| {
        let out = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)?;
        let mut tw = tar::Builder::new(flate2::write::GzEncoder::new(out, flate2::Compression::default()));
        let mut n = 0_usize;
        for d in dirs {
            for f in regular_files(d)? {
                let name = f.strip_prefix(root).map_err(std::io::Error::other)?;
                tw.append_file(name, &mut File::open(&f)?)?;
                n = n.saturating_add(1);
            }
        }
        let out = tw.into_inner()?.finish()?;
        out.sync_all()?;
        Ok(n)
    })();
    match written {
        Ok(n) => std::fs::rename(&tmp, path).map(|()| n),
        Err(e) => {
            std::fs::remove_file(&tmp).ok();
            Err(e)
        }
    }
}
