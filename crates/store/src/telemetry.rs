//! The telemetry database (ADR-0085): a SQLite file of its own beside Remit's, one table of the agents' wide events, kept
//! out of Remit's records, its snapshots and its backups. Its calls run off the async runtime's threads, one at a time.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use telemetry::{Record, SUMMARIZED};

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("telemetry database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("telemetry database: a call did not finish: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// The schema, created if it is not there. A record names the card and home its token belongs to.
const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
CREATE TABLE IF NOT EXISTS records (
    id INTEGER PRIMARY KEY,
    at_ns INTEGER NOT NULL,
    card_id TEXT NOT NULL,
    home TEXT NOT NULL,
    session_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    value REAL,
    trace_id TEXT NOT NULL,
    span_id TEXT NOT NULL,
    parent_id TEXT NOT NULL,
    attrs TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS records_card ON records(card_id, kind, name, at_ns);
CREATE INDEX IF NOT EXISTS records_at ON records(at_ns);
CREATE INDEX IF NOT EXISTS records_span ON records(trace_id, span_id);
";

/// The telemetry database.
#[derive(Clone)]
pub struct Telemetry {
    conn: Arc<Mutex<Connection>>,
}

impl Telemetry {
    /// Opens the database at path, making it if it is not there.
    pub fn open(path: &Path) -> Result<Self, TelemetryError> {
        #[expect(clippy::disallowed_methods, reason = "the store owns the telemetry database too")]
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Keeps records, of card `card_id` working in `home`, in one transaction.
    pub async fn write(&self, card_id: String, home: String, records: Vec<Record>) -> Result<(), TelemetryError> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            insert(&tx, &card_id, &home, records)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Keeps records that all name one trace and span, unless records of that span are kept already: a report sent
    /// again is kept once (ADR-0095 decision 12). The check and the write are one transaction, so two sends of a report
    /// at once keep it once. Says whether it kept them.
    pub async fn write_once(&self, records: Vec<Record>) -> Result<bool, TelemetryError> {
        let Some(first) = records.first() else {
            return Ok(false);
        };
        let (trace, span) = (first.trace_id.clone(), first.span_id.clone());
        self.run(move |conn| {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let kept = tx
                .prepare_cached("SELECT 1 FROM records WHERE trace_id = ? AND span_id = ? LIMIT 1")?
                .exists(params![trace, span])?;
            if kept {
                return Ok(false);
            }
            insert(&tx, "", "", records)?;
            tx.commit()?;
            Ok(true)
        })
        .await
    }

    /// Card `card_id`'s events a summary reads (`SUMMARIZED`), each as its name, when and its attributes.
    pub async fn card_events(&self, card_id: String) -> Result<Vec<(String, i64, Map<String, Value>)>, TelemetryError> {
        self.run(move |conn| {
            let mut q = conn.prepare_cached(
                "SELECT name, at_ns, attrs FROM records WHERE card_id = ? AND kind = 'event' AND name IN (?, ?, ?)",
            )?;
            let [a, b, c] = SUMMARIZED;
            let rows = q.query_map(params![card_id, a, b, c], |row| {
                let attrs: String = row.get(2)?;
                let attrs = serde_json::from_str::<Map<String, Value>>(&attrs).unwrap_or_default();
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, attrs))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// The events named `name` from `since_ns` on, each as its name, when and its attributes, as `telemetry` reads them.
    pub async fn events_since(
        &self,
        name: &'static str,
        since_ns: i64,
    ) -> Result<Vec<(String, i64, Map<String, Value>)>, TelemetryError> {
        self.run(move |conn| {
            let mut q = conn.prepare_cached(
                "SELECT name, at_ns, attrs FROM records WHERE at_ns >= ? AND kind = 'event' AND name = ?",
            )?;
            let rows = q.query_map(params![since_ns, name], |row| {
                let attrs: String = row.get(2)?;
                let attrs = serde_json::from_str::<Map<String, Value>>(&attrs).unwrap_or_default();
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, attrs))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// Removes the records from before `before_ns`; says how many.
    pub async fn forget_before(&self, before_ns: i64) -> Result<usize, TelemetryError> {
        self.run(move |conn| Ok(conn.execute("DELETE FROM records WHERE at_ns < ?", [before_ns])?))
            .await
    }

    async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, TelemetryError> + Send + 'static,
    ) -> Result<T, TelemetryError> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || f(&mut conn.lock().unwrap_or_else(PoisonError::into_inner))).await?
    }
}

/// Inserts records, of card `card_id` working in `home`, in tx.
fn insert(
    tx: &rusqlite::Transaction<'_>,
    card_id: &str,
    home: &str,
    records: Vec<Record>,
) -> Result<(), TelemetryError> {
    let mut put = tx.prepare_cached(
        "INSERT INTO records(at_ns, card_id, home, session_id, kind, name, value, trace_id, span_id, parent_id, attrs)
         VALUES(?,?,?,?,?,?,?,?,?,?,?)",
    )?;
    for r in records {
        let session = match r.attrs.get("session.id") {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        let attrs = Value::Object(r.attrs).to_string();
        put.execute(params![
            r.at_ns,
            card_id,
            home,
            session,
            r.kind.as_str(),
            r.name,
            r.value,
            r.trace_id,
            r.span_id,
            r.parent_id,
            attrs
        ])?;
    }
    Ok(())
}
