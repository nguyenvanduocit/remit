//! Export
//!
//! The owner's right to take what the server holds of his work (docs/remit-design.md, K9): one JSON Lines file per
//! table, a row an object, the documents (doc, data) as the JSON they are, all read in one transaction so the files are
//! one moment of the database. What is sealed under the server's key is left out: the key is not exported, so a sealed
//! value would be a secret the owner cannot open and a stranger might. It opens the database read-only: it runs beside a
//! server that is writing, and never migrates.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
use serde_json::value::RawValue;

use crate::pack::unpack;
use crate::snapshot::make_dir;
use crate::store::{Result, doc};

/// The tables an export leaves out, each with why. Every other table is exported.
pub const NOT_EXPORTED: &[(&str, &str)] = &[
    (
        "schema_migrations",
        "the bookkeeping of this build's schema, not the owner's data",
    ),
    ("memory_text", "the words index of memories, made from them"),
    ("memory_text_config", "the words index of memories, made from them"),
    ("memory_text_content", "the words index of memories, made from them"),
    ("memory_text_data", "the words index of memories, made from them"),
    ("memory_text_docsize", "the words index of memories, made from them"),
    ("memory_text_idx", "the words index of memories, made from them"),
    (
        "memory_embeddings",
        "the meaning of memories, made from their text by the embedding model",
    ),
    (
        "memory_unembedded",
        "the memories the embedding model refused lately, which it is asked again",
    ),
    (
        "memory_edges",
        "the graph of memories, made from their links and subjects",
    ),
];

/// What an export wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Exported {
    /// The schema version of the database it was read from.
    pub version: i64,
    /// Rows written, by table.
    pub tables: BTreeMap<String, i64>,
}

/// A value of a row as it is written: text that is a JSON document in a document column is that JSON.
#[derive(Serialize)]
#[serde(untagged)]
enum Cell {
    Raw(Box<RawValue>),
    Value(Value),
}

/// Writes the database at path into dir, which it makes (mode 0700), and a manifest.json that says what it wrote.
pub fn export(path: &Path, dir: &Path) -> Result<Exported> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the store owns the database: this is one of its own opens"
    )]
    let mut c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    c.busy_timeout(std::time::Duration::from_millis(5000))?;
    let tx = c.transaction()?;
    make_dir(dir)?;
    let mut out = Exported {
        version: tx.query_row("PRAGMA user_version", [], |r| r.get(0))?,
        tables: BTreeMap::new(),
    };
    let tables: Vec<String> = tx
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for table in tables {
        if NOT_EXPORTED.iter().any(|(t, _)| *t == table) {
            continue;
        }
        let n = export_table(&tx, &table, &dir.join(format!("{table}.jsonl")))
            .map_err(|e| crate::Error::Refused(format!("export {table}: {e}")))?;
        out.tables.insert(table, n);
    }
    let mut manifest = serde_json::to_string_pretty(&out)?;
    manifest.push('\n');
    write_new(&dir.join("manifest.json"), manifest.as_bytes())?;
    Ok(out)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    Ok(())
}

/// Writes the rows of table to path, every column but those that hold what is sealed, and says how many.
fn export_table(c: &Connection, table: &str, path: &Path) -> Result<i64> {
    let cols: Vec<String> = c
        .prepare("SELECT name FROM pragma_table_xinfo(?) ORDER BY cid")?
        .query_map([table], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|name| !name.to_lowercase().contains("sealed"))
        .collect();
    let quoted: Vec<String> = cols.iter().map(|n| quote_ident(n)).collect();
    let mut stmt = c.prepare(&format!("SELECT {} FROM {}", quoted.join(", "), quote_ident(table)))?;
    let mut rows = stmt.query([])?;
    let f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    let mut w = BufWriter::new(f);
    let mut n = 0_i64;
    while let Some(row) = rows.next()? {
        let mut obj = BTreeMap::new();
        for (i, col) in cols.iter().enumerate() {
            obj.insert(col.as_str(), exported(col, row.get_ref(i)?)?);
        }
        w.write_all(doc(&obj)?.as_bytes())?;
        w.write_all(b"\n")?;
        n = n.saturating_add(1);
    }
    w.flush()?;
    Ok(n)
}

/// A table or column name as SQLite reads an identifier in quotes.
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A value of column as it is written: text that is a JSON document in a document column is that JSON. A blob is an
/// event's packed text or data (every other blob is sealed, and not exported), written as the text it packs.
fn exported(column: &str, v: ValueRef<'_>) -> Result<Cell> {
    Ok(match v {
        ValueRef::Null => Cell::Value(Value::Null),
        ValueRef::Integer(i) => Cell::Value(Value::from(i)),
        ValueRef::Real(f) => Cell::Value(Value::from(f)),
        ValueRef::Blob(b) => text_cell(column, unpack(b)?),
        ValueRef::Text(t) => text_cell(column, String::from_utf8_lossy(t).into_owned()),
    })
}

/// Text of column as it is written: a JSON document in a document column is that JSON.
fn text_cell(column: &str, text: String) -> Cell {
    if matches!(column, "doc" | "data")
        && let Ok(raw) = RawValue::from_string(text.clone())
    {
        return Cell::Raw(raw);
    }
    Cell::Value(Value::from(text))
}
