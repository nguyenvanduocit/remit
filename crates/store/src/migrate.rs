//! The migrator: `PRAGMA user_version` counts the migrations a database ran, and
//! `schema_migrations` holds the SHA-256 of each one's text.

use std::collections::HashMap;

use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::store::{Error, Result};

/// Applies the migrations c has not had, all in one transaction: a database is at one version or the next, never
/// between. A database from a newer build is refused rather than written by code that does not know its schema, and so
/// is one that ran a migration whose text has changed since: what it holds is what the old text did, and the new text
/// would be taken as applied without ever running. Each migration's checksum is recorded when it runs; a database
/// migrated before checksums were recorded gets them at its first open here, as its migrations read now.
pub(crate) fn migrate(c: &mut Connection, ms: &[&str]) -> Result<()> {
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let known = i64::try_from(ms.len()).map_err(|_| Error::Refused("too many migrations".into()))?;
    if version > known {
        return Err(Error::Refused(format!(
            "the database is at version {version} and this build knows {known}: it was written by a newer Remit"
        )));
    }
    let recorded = recorded_checksums(&tx)?;
    let changed = changed_migrations(&recorded, ms, version);
    if !changed.is_empty() {
        let numbers: Vec<String> = changed.iter().map(i64::to_string).collect();
        return Err(Error::Refused(format!(
            "migration {} changed after this database ran it: a migration is never edited once it has run; put its text back as it was and append a new migration for the change",
            numbers.join(", ")
        )));
    }
    tx.create_scalar_function("unpack_event", 1, FunctionFlags::SQLITE_DETERMINISTIC, |ctx| {
        crate::pack::unpack(ctx.get_raw(0).as_blob()?).map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))
    })?;
    for (v, m) in (1_i64..).zip(ms) {
        if v > version {
            tx.execute_batch(m)
                .map_err(|e| Error::Refused(format!("migration {v}: {e}")))?;
        }
    }
    for (v, m) in (1_i64..).zip(ms) {
        tx.execute(
            "INSERT OR IGNORE INTO schema_migrations(version, sum) VALUES(?, ?)",
            params![v, checksum(m)],
        )?;
    }
    tx.pragma_update(None, "user_version", known)?;
    tx.commit()?;
    c.remove_function("unpack_event", 1)?;
    Ok(())
}

/// The checksums of the migrations the database ran, by number. The table is the migrator's own, outside the numbered
/// migrations it guards.
fn recorded_checksums(c: &Connection) -> Result<HashMap<i64, String>> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, sum TEXT NOT NULL)")?;
    let mut stmt = c.prepare("SELECT version, sum FROM schema_migrations")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The numbers of the migrations among the first version of ms whose recorded checksum is not their text's. One with no
/// record ran before checksums were recorded, and is taken as it reads now.
fn changed_migrations(recorded: &HashMap<i64, String>, ms: &[&str], version: i64) -> Vec<i64> {
    (1_i64..)
        .zip(ms)
        .take_while(|(v, _)| *v <= version)
        .filter(|(v, m)| recorded.get(v).is_some_and(|sum| *sum != checksum(m)))
        .map(|(v, _)| v)
        .collect()
}

/// The SHA-256 of a migration's text, byte for byte: a comment or a space changed is a change.
pub(crate) fn checksum(migration: &str) -> String {
    Sha256::digest(migration.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
