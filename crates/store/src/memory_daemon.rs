//! The database of the memory daemon older Remits ran (ADR-0117), read once so its memories come into the store. Only its
//! `memories` table is read, as that daemon wrote it; nothing is written to it.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::store::Result;

/// One row of the daemon's `memories` table, as it was written: `project` the holder it named, `source` its `ref`, the
/// times unix seconds. A text column the daemon left NULL is none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaemonMemory {
    pub id: i64,
    pub project: String,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub status: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub evidence: Option<String>,
    pub teacher: Option<String>,
    pub source: Option<String>,
    pub retaught: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The memories of the daemon's database at path that a project holds, in the order they were written. It blocks on
/// the file: an async caller runs it on a blocking thread.
pub fn read_memory_daemon(path: &Path) -> Result<Vec<DaemonMemory>> {
    #[expect(
        clippy::disallowed_methods,
        reason = "another program's database, read once and never written: the store's own connection would migrate it"
    )]
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt = c.prepare(
        "SELECT id, project, kind, scope, state, status, title, body, evidence, teacher, ref, retaught,
                coalesce(created_at, 0), coalesce(updated_at, created_at, 0)
         FROM memories WHERE project IS NOT NULL AND project != '' ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(DaemonMemory {
            id: r.get(0)?,
            project: r.get(1)?,
            kind: r.get(2)?,
            scope: r.get(3)?,
            state: r.get(4)?,
            status: r.get(5)?,
            title: r.get(6)?,
            body: r.get(7)?,
            evidence: r.get(8)?,
            teacher: r.get(9)?,
            source: r.get(10)?,
            retaught: r.get(11)?,
            created_at: r.get(12)?,
            updated_at: r.get(13)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
