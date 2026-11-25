//! Grants (ADR-0018)
//!
//! A chain acts on a connection through the grants it holds, never through a credential, and so does the release of a
//! repository, which runs as no chain (ADR-0116). Beyond the read a chain's capabilities imply (derived, not stored:
//! orch), a grant is a row: the strongest effect given on one connection of one source to one holder, and who gave it. A
//! connection the owner marked protected is read only through a grant.

use model::{Grant, GrantHolder};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, from_ms, ms};

impl Tx<'_> {
    /// Keeps g as the grant of its holder on its connection, replacing the one before: the row holds the effect that
    /// stands, and whether a new grant is stronger is the orchestrator's to say.
    pub fn put_grant(&self, g: &Grant) -> Result<()> {
        self.exec(
            "INSERT INTO grants(holder_kind, holder, source, connection, effect, granted_by, granted_at) VALUES(?,?,?,?,?,?,?)
ON CONFLICT(holder_kind, holder, source, connection) DO UPDATE SET effect=excluded.effect, granted_by=excluded.granted_by, granted_at=excluded.granted_at",
            params![
                g.holder.kind.as_str(),
                g.holder.id,
                g.source,
                g.connection,
                g.effect.as_str(),
                doc(&g.granted_by)?,
                ms(g.granted_at)
            ],
        )?;
        Ok(())
    }

    /// Marks a connection of a source protected, or clears the mark. The default connection has no row until it is
    /// marked: marking makes one, with no description. It changes nothing else of the connection.
    pub fn set_connection_protected(&self, source: &str, connection: &str, protected: bool) -> Result<()> {
        self.exec(
            "INSERT INTO source_connections(source, name, description, protected) VALUES(?,?,'',?) ON CONFLICT(source, name) DO UPDATE SET protected=excluded.protected",
            params![source, connection, protected],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The grants of one holder, by source then connection; none for a holder that holds none.
    pub fn grants(&self, holder: &GrantHolder) -> Result<Vec<Grant>> {
        let mut stmt = self.c.prepare_cached(
            "SELECT source, connection, effect, granted_by, granted_at FROM grants WHERE holder_kind=? AND holder=? ORDER BY source, connection",
        )?;
        let rows = stmt.query_map(params![holder.kind.as_str(), holder.id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (source, connection, effect, by, at) = row?;
            out.push(Grant {
                holder: holder.clone(),
                effect,
                granted_by: serde_json::from_str(&by)?,
                granted_at: from_ms(at),
                source,
                connection,
            });
        }
        Ok(out)
    }

    /// The names of a source's connections the owner marked protected, by name.
    pub fn protected_connections(&self, source: &str) -> Result<Vec<String>> {
        self.column(
            "SELECT name FROM source_connections WHERE source=? AND protected=1 ORDER BY name",
            [source],
        )
    }
}
