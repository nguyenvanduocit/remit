//! What the owner gives a source: connections, each with a name and a line that says what it reaches, and the values of
//! the variables a connection reads. A plain variable (an address, a user) is kept as typed; a secret is kept sealed,
//! bytes the store cannot read, with the id of the key that sealed it. Only the orchestrator opens them.

use std::collections::HashMap;

use model::SourceConnection;
use rusqlite::{Row, params};
use secrets::Sealed;

use crate::store::{Reader, Result, Tx, found};

/// One stored variable: the text of a plain one, or the sealed bytes of a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceValue {
    Plain(String),
    Secret(Sealed),
}

/// The value of a row of (plain, sealed, key_id) from column i on: a secret when it has sealed bytes.
fn value_at(r: &Row<'_>, i: usize) -> rusqlite::Result<SourceValue> {
    let plain: String = r.get(i)?;
    let sealed: Option<Vec<u8>> = r.get(i.saturating_add(1))?;
    Ok(match sealed {
        Some(b) => SourceValue::Secret(Sealed {
            r#box: b,
            key_id: r.get(i.saturating_add(2))?,
        }),
        None => SourceValue::Plain(plain),
    })
}

impl Tx<'_> {
    /// Makes a connection of a source, or changes the line that describes it.
    pub fn put_source_connection(&self, c: &SourceConnection) -> Result<()> {
        self.exec(
            "INSERT INTO source_connections(source, name, description) VALUES(?,?,?) ON CONFLICT(source, name) DO UPDATE SET description=excluded.description",
            params![c.source, c.name, c.description],
        )?;
        Ok(())
    }

    /// Removes a connection, its values and the grants on it, together: no value or grant outlives its connection, so a
    /// connection made later under the same name starts with none.
    pub fn delete_source_connection(&self, source: &str, connection: &str) -> Result<()> {
        self.exec(
            "DELETE FROM source_values WHERE source=? AND connection=?",
            [source, connection],
        )?;
        self.exec(
            "DELETE FROM grants WHERE source=? AND connection=?",
            [source, connection],
        )?;
        self.exec(
            "DELETE FROM source_connections WHERE source=? AND name=?",
            [source, connection],
        )?;
        Ok(())
    }

    /// Keeps v as the value of a variable of a connection, replacing the one before.
    pub fn put_source_value(&self, source: &str, connection: &str, name: &str, v: &SourceValue) -> Result<()> {
        match v {
            SourceValue::Secret(sealed) => self.exec(
                "INSERT INTO source_values(source, connection, name, plain, sealed, key_id) VALUES(?,?,?,'',?,?) ON CONFLICT(source, connection, name) DO UPDATE SET plain='', sealed=excluded.sealed, key_id=excluded.key_id",
                params![source, connection, name, sealed.r#box, sealed.key_id],
            )?,
            SourceValue::Plain(plain) => self.exec(
                "INSERT INTO source_values(source, connection, name, plain, sealed, key_id) VALUES(?,?,?,?,NULL,'') ON CONFLICT(source, connection, name) DO UPDATE SET plain=excluded.plain, sealed=NULL, key_id=''",
                params![source, connection, name, plain],
            )?,
        };
        Ok(())
    }

    /// Removes the value of one variable of a connection. A variable with no value is not an error.
    pub fn delete_source_value(&self, source: &str, connection: &str, name: &str) -> Result<()> {
        self.exec(
            "DELETE FROM source_values WHERE source=? AND connection=? AND name=?",
            [source, connection, name],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The connections the owner made for a source, by name.
    pub fn source_connections(&self, source: &str) -> Result<Vec<SourceConnection>> {
        let mut stmt = self
            .c
            .prepare_cached("SELECT name, description FROM source_connections WHERE source=? ORDER BY name")?;
        let rows = stmt.query_map([source], |r| {
            Ok(SourceConnection {
                source: source.to_owned(),
                name: r.get(0)?,
                description: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The stored values of a connection, by variable name; none for a connection that has none.
    pub fn source_values(&self, source: &str, connection: &str) -> Result<HashMap<String, SourceValue>> {
        let mut stmt = self
            .c
            .prepare_cached("SELECT name, plain, sealed, key_id FROM source_values WHERE source=? AND connection=?")?;
        let rows = stmt.query_map([source, connection], |r| Ok((r.get(0)?, value_at(r, 1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// One stored variable; `Error::NotFound` until the owner has entered it.
    pub fn source_value(&self, source: &str, connection: &str, name: &str) -> Result<SourceValue> {
        found(
            self.c
                .prepare_cached(
                    "SELECT plain, sealed, key_id FROM source_values WHERE source=? AND connection=? AND name=?",
                )?
                .query_row([source, connection, name], |r| value_at(r, 0)),
        )
    }
}
