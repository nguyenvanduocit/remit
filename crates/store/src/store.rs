//! The store: one writer connection on a thread of its own, a fixed set of `query_only` readers, and the helpers every
//! entity's reads and writes share. Chains, cards, edges, decisions, approvals, landings, drafts, inbox items,
//! session inits and playbooks are stored as JSON documents keyed by id (plus the columns queries filter on); events are
//! rows, because streamed chunks are appended to them in place.

use std::collections::HashSet;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::ops::Deref;
use std::os::unix::fs::OpenOptionsExt;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use model::{
    Approval, Assistant, Card, Chain, Cleanup, ContextCount, Decision, DecisionKind, Draft, Edge, EventKind,
    ForeignJson, InboxItem, InitReport, OwnerKind, Playbook, PromptStatus, Request, SessionLaunch, StoredChain,
    SubscriptionOwner, Time,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Params, ToSql, TransactionBehavior, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{lock, migrate, migrations, snapshot};

/// What a read or write of the store fails with.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found")]
    NotFound,
    /// A record written over a revision that is no longer the stored one: the writer missed a change.
    #[error("it changed since it was read: read it again")]
    Conflict,
    /// A save of a draft that was sent or deleted: a save arriving late must not bring it back.
    #[error("this draft was sent or deleted already")]
    DraftEnded,
    /// Ending a draft at a revision that is no longer the stored one: its newer words were not sent.
    #[error("a draft changed while it was being sent: send it again")]
    DraftChanged,
    /// The database another process holds: a server runs on it, or a restore is putting a snapshot back.
    #[error("{0}: another process holds the database: a Remit server runs on it, or a restore does")]
    InUse(PathBuf),
    #[error("the default account stays: it is the machine's own login")]
    DefaultAccount,
    #[error("a plugin's account is not stored: it is the plugin's connection")]
    PluginAccount,
    #[error("a client key with this name exists already")]
    ClientKeyName,
    #[error("an event signal with this name exists already")]
    EventSignalName,
    #[error("a sensor with this name exists already")]
    SensorName,
    #[error("a project with this name exists already")]
    ProjectName,
    /// The store was closed, or its writer thread is gone.
    #[error("the store is closed")]
    Closed,
    /// A read or a write panicked: its transaction was rolled back, and the connection it ran on serves the next one.
    #[error("a store transaction panicked: {0}")]
    Panicked(String),
    /// The one record asked for is stored in a shape this build cannot read: a newer build wrote it, and this one, rolled
    /// back to, leaves it as it is (ADR-0107).
    #[error("{record} {id} was written by a newer Remit and this one cannot read it: {why}")]
    Unreadable {
        record: &'static str,
        id: String,
        why: String,
    },
    /// A refusal with its reason: a database too new, a migration edited, a snapshot that cannot be restored.
    #[error("{0}")]
    Refused(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// The writer: the one connection, owned by its thread, which runs the transactions sent to it one after the other.
/// SQLite has one writer at a time, so a second connection would only wait on its lock.
struct Writer {
    jobs: mpsc::Sender<Job>,
    thread: JoinHandle<()>,
}

/// The `query_only` connections; a read takes one and gives it back.
struct Pool {
    free: Mutex<Vec<Connection>>,
    returned: Condvar,
}

/// Reads and writes on separate connections, so a write never waits for a long read: SQLite in WAL mode lets readers
/// run beside the one writer.
pub struct Store {
    writer: Option<Writer>,
    readers: Arc<Pool>,
    unreadable: Arc<Unreadable>,
    path: PathBuf,
    pub(crate) snapshotting: tokio::sync::Mutex<()>,
    /// Held until the store is dropped, after its connections: no other server, and no restore, takes the database
    /// meanwhile (`lock::hold`).
    _lock: File,
}

/// The reads, on a reader's connection or inside a write transaction.
pub struct Reader<'c> {
    pub(crate) c: &'c Connection,
    unreadable: &'c Unreadable,
}

/// The records this process could not read, each once: a build rolled back to finds records a newer one wrote in a shape
/// it does not name (ADR-0107). A read leaves such a record out of what it returns, so the rest reads; this build never
/// writes it, as it never read it, and a build that reads it finds it as it was.
#[derive(Default)]
pub(crate) struct Unreadable(Mutex<HashSet<(&'static str, String)>>);

impl Unreadable {
    /// Notes the record of kind record and id as one this build cannot read, for why, and logs it the first time.
    fn note(&self, record: &'static str, id: &str, why: &dyn fmt::Display) {
        let mut seen = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if seen.insert((record, id.to_owned())) {
            tracing::warn!(record, id, %why, "a record this build cannot read is left out");
        }
    }

    fn count(&self) -> u32 {
        let seen = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        u32::try_from(seen.len()).unwrap_or(u32::MAX)
    }
}

/// One write transaction: what is written through it is stored together, or not at all, and what is read through it
/// (every `Reader` method) is what the transaction sees.
pub struct Tx<'c> {
    pub(crate) r: Reader<'c>,
}

impl<'c> Deref for Tx<'c> {
    type Target = Reader<'c>;
    fn deref(&self) -> &Reader<'c> {
        &self.r
    }
}

const BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

impl Store {
    /// Opens the database at path, creating it if it is new, and migrates it to this build's version after copying it
    /// into `snapshot_dir` (see `snapshot::before_migrating`). It holds the database until dropped, and refuses with
    /// `Error::InUse` one that another process holds.
    pub fn open(path: impl AsRef<Path>) -> Result<Store> {
        let path = path.as_ref().to_path_buf();
        let lock = lock::hold(&path)?;
        // The database holds the owner's records, so a new one is created owner-only; SQLite gives its -wal and -shm
        // files the database's mode.
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)?;
        let mut write = open_writer(&path)?;
        let ms = migrations::MIGRATIONS;
        snapshot::before_migrating(
            &write,
            &snapshot::snapshot_dir(&path),
            ms.len(),
            Time::from(time::OffsetDateTime::now_utc()),
        )
        .map_err(|e| Error::Refused(format!("snapshot {} before migrating it: {e}", path.display())))?;
        migrate::migrate(&mut write, ms).map_err(|e| match e {
            Error::Refused(why) => Error::Refused(format!("migrate {}: {why}", path.display())),
            other => other,
        })?;
        // A read is CPU-bound: more than one per core would only take turns.
        let n = std::thread::available_parallelism().map_or(1, usize::from);
        let mut free = Vec::with_capacity(n);
        for _ in 0..n {
            free.push(open_reader(&path)?);
        }
        let (jobs, queue) = mpsc::channel::<Job>();
        let thread = std::thread::Builder::new().name("store-writer".into()).spawn(move || {
            for job in queue {
                job(&mut write);
            }
        })?;
        Ok(Store {
            writer: Some(Writer { jobs, thread }),
            readers: Arc::new(Pool {
                free: Mutex::new(free),
                returned: Condvar::new(),
            }),
            unreadable: Arc::default(),
            path,
            snapshotting: tokio::sync::Mutex::new(()),
            _lock: lock,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many records this process found it cannot read and left out (ADR-0107): none but after a rollback.
    pub fn unreadable(&self) -> u32 {
        self.unreadable.count()
    }

    /// Runs do in one write transaction (`BEGIN IMMEDIATE`), committed when do returns Ok
    /// and rolled back when it returns an error. do is synchronous: nothing awaits inside a transaction.
    pub async fn update<T, E, F>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce(&Tx<'_>) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        let writer = self.writer.as_ref().ok_or(Error::Closed)?;
        let (answer, answered) = tokio::sync::oneshot::channel();
        let unreadable = Arc::clone(&self.unreadable);
        let job: Job = Box::new(move |c| {
            // The caller may have stopped waiting; the transaction ran to its end all the same. A panic in f is answered
            // as an error, its transaction rolled back as it unwinds, so the one writer thread lives on.
            let done = std::panic::catch_unwind(AssertUnwindSafe(|| run_update(c, &unreadable, f)))
                .unwrap_or_else(|p| Err(Error::Panicked(panic_words(p.as_ref())).into()));
            answer.send(done).ok();
        });
        writer.jobs.send(job).map_err(|_| Error::Closed)?;
        answered.await.map_err(|_| Error::Closed)?
    }

    /// Runs do in one read transaction on a reader, on a blocking thread: every part it reads is as it was at one
    /// moment, so a write landing between two of its queries cannot show half of a change.
    pub async fn read<T, E, F>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce(&Reader<'_>) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        self.read_once_running(|| {}, f).await
    }

    /// `read`, calling running on its blocking thread before it waits for a reader: what tells a read that waited for a
    /// thread from one that waited for a reader.
    pub(crate) async fn read_once_running<T, E, F>(&self, running: impl FnOnce() + Send + 'static, f: F) -> Result<T, E>
    where
        F: FnOnce(&Reader<'_>) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        let pool = Arc::clone(&self.readers);
        let unreadable = Arc::clone(&self.unreadable);
        tokio::task::spawn_blocking(move || {
            running();
            let mut c = pool.take()?;
            // A panic in f is answered as an error and the connection, its read rolled back, goes back to the pool: lost,
            // the pool would shrink to nothing and every read wait for good.
            let out = std::panic::catch_unwind(AssertUnwindSafe(|| run_read(&mut c, &unreadable, f)))
                .unwrap_or_else(|p| Err(Error::Panicked(panic_words(p.as_ref())).into()));
            pool.give(c);
            out
        })
        .await
        .map_err(|_| Error::Closed)?
    }

    /// View loads a whole chain for the browser, in one read transaction.
    pub async fn view(&self, chain_id: String) -> Result<StoredChain> {
        self.read(move |r| r.view(&chain_id)).await
    }

    /// Closes the database: the writer finishes what was sent to it and closes, then the readers, then the lock lets
    /// go, so another process may take it only once nothing here writes to it.
    pub fn close(mut self) -> Result<()> {
        self.stop_writer()
    }

    fn stop_writer(&mut self) -> Result<()> {
        if let Some(Writer { jobs, thread }) = self.writer.take() {
            drop(jobs);
            thread.join().map_err(|_| Error::Closed)?;
        }
        if let Ok(mut free) = self.readers.free.lock() {
            free.clear();
        }
        Ok(())
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        // close reports a failure; a store only dropped has no one to report it to.
        self.stop_writer().ok();
    }
}

impl Pool {
    fn take(&self) -> Result<Connection> {
        let mut free = self.free.lock().map_err(|_| Error::Closed)?;
        loop {
            if let Some(c) = free.pop() {
                return Ok(c);
            }
            free = self.returned.wait(free).map_err(|_| Error::Closed)?;
        }
    }

    fn give(&self, c: Connection) {
        if let Ok(mut free) = self.free.lock() {
            free.push(c);
            self.returned.notify_one();
        }
    }
}

/// What a panic said, as its message.
fn panic_words(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "no message".to_owned())
}

fn run_update<T, E, F>(c: &mut Connection, unreadable: &Unreadable, f: F) -> Result<T, E>
where
    F: FnOnce(&Tx<'_>) -> Result<T, E>,
    E: From<Error>,
{
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(Error::from)?;
    let out = f(&Tx {
        r: Reader { c: &tx, unreadable },
    })?;
    tx.commit().map_err(Error::from)?;
    Ok(out)
}

fn run_read<T, E, F>(c: &mut Connection, unreadable: &Unreadable, f: F) -> Result<T, E>
where
    F: FnOnce(&Reader<'_>) -> Result<T, E>,
    E: From<Error>,
{
    // Dropped unfinished, the transaction rolls back: it wrote nothing.
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(Error::from)?;
    f(&Reader { c: &tx, unreadable })
}

/// The writer: WAL, busy_timeout 5000, foreign keys on, synchronous FULL (set here so it does not rest on how SQLite was
/// built).
pub(crate) fn open_writer(path: &Path) -> Result<Connection> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the store owns the database: this is one of its own opens"
    )]
    let c = Connection::open(path)?;
    let mode: String = c.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
    if mode != "wal" {
        return Err(Error::Refused(format!(
            "{}: journal_mode is {mode}, not wal",
            path.display()
        )));
    }
    c.busy_timeout(BUSY_TIMEOUT)?;
    c.pragma_update(None, "foreign_keys", "on")?;
    c.pragma_update(None, "synchronous", "FULL")?;
    Ok(c)
}

/// A reader: busy_timeout 5000, query_only.
fn open_reader(path: &Path) -> Result<Connection> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the store owns the database: this is one of its own opens"
    )]
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    c.busy_timeout(BUSY_TIMEOUT)?;
    c.pragma_update(None, "query_only", 1)?;
    Ok(c)
}

/// A time as a column holds it: Unix milliseconds, rounded down.
pub(crate) fn ms(t: Time) -> i64 {
    t.0.unix_timestamp()
        .saturating_mul(1000)
        .saturating_add(i64::from(t.0.millisecond()))
}

/// A column's Unix milliseconds as a time, in UTC. Every column `ms` wrote is one; a number past the years a time holds
/// reads as the default time.
pub(crate) fn from_ms(ms: i64) -> Time {
    let nanos = i128::from(ms).saturating_mul(1_000_000);
    time::OffsetDateTime::from_unix_timestamp_nanos(nanos).map_or_else(|_| Time::default(), Time)
}

/// A record as its document.
pub(crate) fn doc<T: Serialize + ?Sized>(v: &T) -> Result<String> {
    Ok(serde_json::to_string(v)?)
}

/// The name of the record T, as the log of one left out names it: `Chain` for `model::Chain`.
fn record_name<T>() -> &'static str {
    let path = std::any::type_name::<T>();
    path.rsplit("::").next().unwrap_or(path)
}

/// The id of a stored document as its `id` names it; a document with none (or that is no JSON) is named by its text.
fn doc_id(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|d| d.get("id").and_then(serde_json::Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| text.chars().take(120).collect())
}

/// rusqlite's no-row answer as `Error::NotFound`.
pub(crate) fn found<T>(r: rusqlite::Result<T>) -> Result<T> {
    r.optional()?.ok_or(Error::NotFound)
}

/// Whether e is a UNIQUE constraint on what (`table.column`, or `table.a, table.b`) refusing a write.
pub(crate) fn unique_failed(e: &rusqlite::Error, what: &str) -> bool {
    e.to_string().contains(&format!("UNIQUE constraint failed: {what}"))
}

// Revisions
//
// Every record a channel is pushed carries its revision, which counts its stored changes: a copy pushed or answered
// earlier has a lower one, so a channel keeps the newer of two copies whatever order they reach it in. The store counts
// it, at the one place every write of the record passes: a record is written over the revision it was read at (0 for a
// new one) and stored at the next. A write over any other revision changes nothing and fails with `Error::Conflict`: its
// writer missed a change it would otherwise undo. A write returns the record at the revision stored; no caller counts it.

/// The guard of an upsert of table (`Tx::put_doc`): the stored document is at the revision before the one the document
/// written brings, so a write over a stale read, or a new record (revision 1) on a taken id, updates nothing.
macro_rules! over_read_rev {
    ($table:literal) => {
        concat!(
            " WHERE json_extract(",
            $table,
            ".doc, '$.rev') = json_extract(excluded.doc, '$.rev') - 1"
        )
    };
}
/// The count of the messages waiting for the card match names, its session's opening (priority 3, `PRIORITY_OPENING`)
/// left out: nobody sent that one to a session already open. A query that reads it binds `queued_params`, from which
/// SQLite sees that it reads the partial index events_queued.
macro_rules! queued_of {
    ($match:literal) => {
        concat!(
            "(SELECT COUNT(*) FROM events WHERE kind = :prompt AND status = :queued AND card_id = ",
            $match,
            " AND priority != 3)"
        )
    };
}
/// A card is read with what is filled in from elsewhere (its queued count, `queued_of!`) and stored without it.
macro_rules! card_doc {
    () => {
        concat!("json_set(doc, '$.queued', ", $crate::store::queued_of!("cards.id"), ")")
    };
}
pub(crate) use {card_doc, over_read_rev, queued_of};

const _: () = assert!(model::PRIORITY_OPENING == 3, "queued_of! spells the opening's priority");

const PROMPT: &str = EventKind::Prompt.as_str();
const QUEUED: &str = PromptStatus::Queued.as_str();

/// The named parameters of a query that reads `queued_of!` (or `card_doc!`): own, with the kind and the status of a
/// queued message.
pub(crate) fn queued_params<'a>(own: &[(&'a str, &'a dyn ToSql)]) -> Vec<(&'a str, &'a dyn ToSql)> {
    let mut all: Vec<(&str, &dyn ToSql)> = vec![(":prompt", &PROMPT), (":queued", &QUEUED)];
    all.extend_from_slice(own);
    all
}

impl Reader<'_> {
    /// The documents query reads, decoded: one this build cannot decode is left out (`Unreadable`).
    pub(crate) fn list<T: DeserializeOwned>(&self, query: &str, params: impl Params) -> Result<Vec<T>> {
        let mut stmt = self.c.prepare_cached(query)?;
        let mut rows = stmt.query(params)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let text: String = row.get(0)?;
            match serde_json::from_str(&text) {
                Ok(record) => out.push(record),
                Err(why) => self.unreadable.note(record_name::<T>(), &doc_id(&text), &why),
            }
        }
        Ok(out)
    }

    /// The one document query reads, decoded; `Error::NotFound` when it reads none, `Error::Unreadable` when this build
    /// cannot decode it.
    pub(crate) fn one<T: DeserializeOwned>(&self, query: &str, params: impl Params) -> Result<T> {
        let text: String = found(self.c.prepare_cached(query)?.query_row(params, |r| r.get(0)))?;
        serde_json::from_str(&text).map_err(|why| self.left_out(record_name::<T>(), &doc_id(&text), &why))
    }

    /// Notes the record of kind record and id as one this build cannot read, for why: the error a read of it alone
    /// answers, while a list leaves it out.
    pub(crate) fn left_out(&self, record: &'static str, id: &str, why: &dyn fmt::Display) -> Error {
        self.unreadable.note(record, id, why);
        Error::Unreadable {
            record,
            id: id.to_owned(),
            why: why.to_string(),
        }
    }

    /// The one value query reads.
    pub(crate) fn value<T: rusqlite::types::FromSql>(&self, query: &str, params: impl Params) -> Result<T> {
        found(self.c.prepare_cached(query)?.query_row(params, |r| r.get(0)))
    }

    /// The values of query's first column.
    pub(crate) fn column<T: rusqlite::types::FromSql>(&self, query: &str, params: impl Params) -> Result<Vec<T>> {
        let mut stmt = self.c.prepare_cached(query)?;
        let rows = stmt.query_map(params, |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

impl Tx<'_> {
    /// Runs one statement and says how many rows it changed.
    pub(crate) fn exec(&self, query: &str, params: impl Params) -> Result<usize> {
        Ok(self.c.prepare_cached(query)?.execute(params)?)
    }

    /// Writes a record's document, brought at the revision after the one it was read at, with query: an upsert guarded by
    /// `over_read_rev!`, or an UPDATE on the revision read, either RETURNING what it returns (the revision stored, or the
    /// document when the store fills a part of it in). `Error::Conflict` when it wrote no row: the stored record is at
    /// another revision, or a new one's id is taken.
    pub(crate) fn put_doc<T: rusqlite::types::FromSql>(&self, query: &str, params: impl Params) -> Result<T> {
        match self.value(query, params) {
            Err(Error::NotFound) => Err(Error::Conflict),
            other => other,
        }
    }
}

// Chains

impl Tx<'_> {
    /// Writes c over the revision it was read at, c.rev (0 for a new chain), and returns it at the revision stored.
    /// `Error::Conflict` when the stored chain is at another revision: a change made since would otherwise be lost.
    pub fn put_chain(&self, mut c: Chain) -> Result<Chain> {
        c.rev = c.rev.saturating_add(1);
        c.rev = self.put_doc(
            concat!(
                "INSERT INTO chains(id, created_at, doc) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("chains"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![c.id, ms(c.created_at), doc(&c)?],
        )?;
        Ok(c)
    }

    /// Removes a chain and everything recorded under it (`delete_home`). Part of a Tx: a chain is deleted with the
    /// check that it may be.
    pub fn delete_chain(&self, id: &str) -> Result<()> {
        self.delete_home(id)?;
        // What a chain subscribed to ends with it: nothing is left to wake a chain that is gone.
        for kind in [OwnerKind::Chain, OwnerKind::Remit] {
            self.delete_subscriptions_of(&SubscriptionOwner {
                kind,
                id: id.to_owned(),
            })?;
        }
        self.exec("DELETE FROM chains WHERE id=?", [id])?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn chain(&self, id: &str) -> Result<Chain> {
        self.one("SELECT doc FROM chains WHERE id=?", [id])
    }

    pub fn chains(&self) -> Result<Vec<Chain>> {
        self.list("SELECT doc FROM chains ORDER BY created_at DESC", [])
    }
}

// Cleanups

impl Tx<'_> {
    /// Records the files a deleted chain leaves to remove. Part of a Tx: it is stored with the deletion.
    pub fn put_cleanup(&self, c: &Cleanup) -> Result<()> {
        self.exec(
            "INSERT INTO cleanups(chain_id, doc) VALUES(?,?) ON CONFLICT(chain_id) DO UPDATE SET doc=excluded.doc",
            params![c.chain_id, doc(c)?],
        )?;
        Ok(())
    }

    /// Says a deleted chain's files have been dealt with.
    pub fn delete_cleanup(&self, chain_id: &str) -> Result<()> {
        self.exec("DELETE FROM cleanups WHERE chain_id=?", [chain_id])?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The files of deleted chains not yet removed.
    pub fn cleanups(&self) -> Result<Vec<Cleanup>> {
        self.list("SELECT doc FROM cleanups ORDER BY chain_id", [])
    }
}

// Cards

impl Tx<'_> {
    /// Writes c over the revision it was read at, c.rev (0 for a new card), and returns it at the revision stored, with
    /// its queued count as it is now. A transaction that changes the card's queue after writes the card after it, so the
    /// card it returns is the card as stored. `Error::Conflict` when the stored card is at another revision.
    pub fn put_card(&self, mut c: Card) -> Result<Card> {
        c.rev = c.rev.saturating_add(1);
        c.rev = self.put_doc(
            concat!(
                "INSERT INTO cards(id, home, created_at, doc) VALUES(?,?,?,json_remove(?, '$.queued')) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("cards"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![c.id, c.home, ms(c.created_at), doc(&c)?],
        )?;
        c.queued = self.card_queued(&c.id)?;
        Ok(c)
    }
}

impl Reader<'_> {
    /// How many messages wait for a card, as a read of the card gives it.
    fn card_queued(&self, card_id: &str) -> Result<i64> {
        self.value(
            concat!("SELECT ", queued_of!(":card")),
            &*queued_params(&[(":card", &card_id)]),
        )
    }

    pub fn card(&self, id: &str) -> Result<Card> {
        self.one(
            concat!("SELECT ", card_doc!(), " FROM cards WHERE id = :id"),
            &*queued_params(&[(":id", &id)]),
        )
    }

    pub fn cards(&self, chain_id: &str) -> Result<Vec<Card>> {
        self.list(
            concat!(
                "SELECT ",
                card_doc!(),
                " FROM cards WHERE home = :home ORDER BY created_at"
            ),
            &*queued_params(&[(":home", &chain_id)]),
        )
    }
}

// Edges

impl Tx<'_> {
    pub fn put_edge(&self, e: &Edge) -> Result<()> {
        self.exec(
            "INSERT INTO edges(id, chain_id, created_at, doc) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
            params![e.id, e.chain_id, ms(e.created_at), doc(e)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn edges(&self, chain_id: &str) -> Result<Vec<Edge>> {
        self.list("SELECT doc FROM edges WHERE chain_id=? ORDER BY created_at", [chain_id])
    }
}

// Decisions

impl Tx<'_> {
    pub fn put_decision(&self, d: &Decision) -> Result<()> {
        self.exec(
            "INSERT INTO decisions(id, chain_id, created_at, doc) VALUES(?,?,?,?)",
            params![d.id, d.chain_id, ms(d.created_at), doc(d)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn decisions(&self, chain_id: &str) -> Result<Vec<Decision>> {
        self.list(
            "SELECT doc FROM decisions WHERE chain_id=? ORDER BY created_at",
            [chain_id],
        )
    }

    pub fn decision(&self, id: &str) -> Result<Decision> {
        self.one("SELECT doc FROM decisions WHERE id=?", [id])
    }

    /// A chain's decisions of one kind, oldest first.
    pub fn decisions_of_kind(&self, chain_id: &str, kind: &DecisionKind) -> Result<Vec<Decision>> {
        self.list(
            "SELECT doc FROM decisions WHERE chain_id=? AND kind=? ORDER BY created_at",
            [chain_id, kind.as_str()],
        )
    }

    /// The newest decisions of one kind across chains, newest first, at most limit. A decision's kind is a column
    /// generated from its document, so a query on a kind reads it from its index.
    pub fn recent_decisions(&self, kind: &DecisionKind, limit: i64) -> Result<Vec<Decision>> {
        self.list(
            "SELECT doc FROM decisions WHERE kind=? ORDER BY created_at DESC, rowid DESC LIMIT ?",
            params![kind.as_str(), limit],
        )
    }

    /// The decisions of kind made from `from` up to `to`, oldest first.
    pub fn decisions_between(&self, kind: &DecisionKind, from: Time, to: Time) -> Result<Vec<Decision>> {
        self.list(
            "SELECT doc FROM decisions WHERE kind=? AND created_at>=? AND created_at<? ORDER BY created_at, rowid",
            params![kind.as_str(), ms(from), ms(to)],
        )
    }
}

// Approvals

impl Tx<'_> {
    /// Keeps a reviewer's or QC's verdict. Part of a Tx: it is stored with the decision the owner reads.
    pub fn put_approval(&self, a: &Approval) -> Result<()> {
        self.exec(
            "INSERT INTO approvals(id, chain_id, at, doc) VALUES(?,?,?,?)",
            params![a.id, a.chain_id, ms(a.at), doc(a)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    /// A chain's verdicts, oldest first.
    pub fn approvals(&self, chain_id: &str) -> Result<Vec<Approval>> {
        self.list(
            "SELECT doc FROM approvals WHERE chain_id=? ORDER BY at, rowid",
            [chain_id],
        )
    }
}

// Drafts
//
// A draft lives until it is sent or deleted, which ends it: an ended draft is no longer the chain's, and stays as a row
// only so that a save of it arriving late is refused, until forget_drafts.

impl Tx<'_> {
    /// Saves d: a new draft at revision 1, or new words and attachments on the stored one at its next revision (what a draft is about is
    /// kept as first saved). Unlike every other record, a save is not refused over a stale revision: a draft is the
    /// owner's words as he types them, and the newest save is what he sees; its revision guards its end (`end_drafts`).
    /// `Error::DraftEnded` when d was ended.
    pub fn put_draft(&self, d: &Draft) -> Result<Draft> {
        let saved = self.one(
            "INSERT INTO drafts(id, home, at, doc) VALUES(?,?,?,json_set(?, '$.rev', 1))
ON CONFLICT(id) DO UPDATE SET doc=json_set(drafts.doc, '$.text', json_extract(excluded.doc, '$.text'), '$.attachments', excluded.doc -> '$.attachments', '$.updatedAt', json_extract(excluded.doc, '$.updatedAt'),
  '$.rev', json_extract(drafts.doc, '$.rev') + 1) WHERE drafts.ended IS NULL
RETURNING doc",
            params![d.id, d.home, ms(d.updated_at), doc(d)?],
        );
        match saved {
            // The row is there, ended: the update did not happen.
            Err(Error::NotFound) => Err(Error::DraftEnded),
            other => other,
        }
    }

    /// Ends drafts as they were read: `Error::DraftChanged`, and none ended, when one is no longer at the revision read.
    /// Part of a Tx: drafts end with the send that sends them.
    pub fn end_drafts(&self, ds: &[Draft], at: Time) -> Result<()> {
        for d in ds {
            let n = self.exec(
                "UPDATE drafts SET ended=? WHERE id=? AND ended IS NULL AND json_extract(doc, '$.rev')=?",
                params![ms(at), d.id, d.rev],
            )?;
            if n == 0 {
                return Err(Error::DraftChanged);
            }
        }
        Ok(())
    }

    /// Removes the drafts ended before before.
    pub fn forget_drafts(&self, before: Time) -> Result<()> {
        self.exec("DELETE FROM drafts WHERE ended < ?", [ms(before)])?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The draft with id, unless it was ended: `Error::NotFound` then, as for one never saved.
    pub fn draft(&self, id: &str) -> Result<Draft> {
        self.one("SELECT doc FROM drafts WHERE id=? AND ended IS NULL", [id])
    }

    /// A chain's drafts, not ended, oldest first.
    pub fn drafts(&self, chain_id: &str) -> Result<Vec<Draft>> {
        self.list(
            "SELECT doc FROM drafts WHERE home=? AND ended IS NULL ORDER BY at, rowid",
            [chain_id],
        )
    }

    /// Whether the draft was sent or deleted; false for one never saved.
    pub fn draft_ended(&self, id: &str) -> Result<bool> {
        match self.value("SELECT ended IS NOT NULL FROM drafts WHERE id=?", [id]) {
            Err(Error::NotFound) => Ok(false),
            other => other,
        }
    }
}

// Session inits

impl Tx<'_> {
    /// Keeps `init` as card `card_id`'s (of `home`) session init, replacing the one before.
    pub fn put_session_init(&self, card_id: &str, home: &str, init: &InitReport) -> Result<()> {
        self.exec(
            "INSERT INTO session_inits(card_id, home, doc) VALUES(?,?,?) ON CONFLICT(card_id) DO UPDATE SET doc=excluded.doc",
            params![card_id, home, doc(init)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn session_init(&self, card_id: &str) -> Result<InitReport> {
        self.one("SELECT doc FROM session_inits WHERE card_id=?", [card_id])
    }
}

// Session contexts

impl Tx<'_> {
    /// Keeps `c` as card `card_id`'s (of `home`) session context, replacing the one before.
    pub fn put_session_context(&self, card_id: &str, home: &str, c: &ContextCount) -> Result<()> {
        self.exec(
            "INSERT INTO session_contexts(card_id, home, doc) VALUES(?,?,?) ON CONFLICT(card_id) DO UPDATE SET doc=excluded.doc",
            params![card_id, home, doc(c)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn session_context(&self, card_id: &str) -> Result<ContextCount> {
        self.one("SELECT doc FROM session_contexts WHERE card_id=?", [card_id])
    }
}

// Session launches

impl Tx<'_> {
    pub fn put_session_launch(&self, l: &SessionLaunch) -> Result<()> {
        self.exec(
            "INSERT INTO session_launches(card_id, home, doc) VALUES(?,?,?) ON CONFLICT(card_id) DO UPDATE SET doc=excluded.doc",
            params![l.card_id, l.home, doc(l)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn session_launch(&self, card_id: &str) -> Result<SessionLaunch> {
        self.one("SELECT doc FROM session_launches WHERE card_id=?", [card_id])
    }
}

// Inbox

impl Tx<'_> {
    /// Writes it over the revision it was read at, it.rev (0 for a new item), and returns it at the revision stored. A
    /// change item is stored with its kind beside the change, which no channel is told. `Error::Conflict` when the stored
    /// item is at another revision: it was answered, cancelled or decided since it was read.
    pub fn put_inbox(&self, mut it: InboxItem) -> Result<InboxItem> {
        if let Some(change) = &it.change {
            it.change_kind = Some(change.kind);
        }
        it.rev = it.rev.saturating_add(1);
        it.rev = self.put_doc(
            concat!(
                "INSERT INTO inbox(id, home, status, created_at, doc) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status, doc=excluded.doc",
                over_read_rev!("inbox"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![it.id, it.home, it.status.as_str(), ms(it.created_at), doc(&it)?],
        )?;
        Ok(it)
    }
}

impl Reader<'_> {
    pub fn inbox_item(&self, id: &str) -> Result<InboxItem> {
        self.one("SELECT doc FROM inbox WHERE id=?", [id])
    }

    pub fn open_inbox(&self) -> Result<Vec<InboxItem>> {
        self.list("SELECT doc FROM inbox WHERE status='open' ORDER BY created_at", [])
    }

    /// The items the Assistant decided whose answer waits out the overturn window (ADR-0101), the oldest first.
    pub fn decided_inbox(&self) -> Result<Vec<InboxItem>> {
        self.list("SELECT doc FROM inbox WHERE status='decided' ORDER BY created_at", [])
    }

    /// How many failures were put to the owner since t (error items, open or answered): what `fact:remit.health` counts.
    pub fn errors_since(&self, t: Time) -> Result<i64> {
        self.value(
            "SELECT COUNT(*) FROM inbox WHERE created_at>=? AND json_extract(doc, '$.kind')='error'",
            params![ms(t)],
        )
    }
}

// The Assistant

impl Tx<'_> {
    pub fn put_assistant(&self, a: &Assistant) -> Result<()> {
        self.exec(
            "INSERT INTO assistant(id, doc) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
            params![a.id, doc(a)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The Assistant; `Error::NotFound` before it is made.
    pub fn assistant(&self) -> Result<Assistant> {
        self.one("SELECT doc FROM assistant", [])
    }
}

// Playbooks

impl Tx<'_> {
    /// Writes p over the revision it was read at, p.rev (0 for a new playbook), and returns it at the revision stored.
    /// `Error::Conflict` when the stored playbook is at another revision, or a new one's id is taken.
    pub fn put_playbook(&self, mut p: Playbook) -> Result<Playbook> {
        p.rev = p.rev.saturating_add(1);
        p.rev = self.put_doc(
            concat!(
                "INSERT INTO playbooks(id, doc) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("playbooks"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![p.id, doc(&p)?],
        )?;
        Ok(p)
    }

    pub fn delete_playbook(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM playbooks WHERE id=?", [id])?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn playbook(&self, id: &str) -> Result<Playbook> {
        self.one("SELECT doc FROM playbooks WHERE id=?", [id])
    }

    pub fn playbooks(&self) -> Result<Vec<Playbook>> {
        self.list("SELECT doc FROM playbooks ORDER BY id", [])
    }
}

// Requests
//
// An action a channel asked for is kept with its first answer, so that the same action asked again (the same request
// id) is answered the same and done once.

impl Tx<'_> {
    /// Records that an action has begun; false when a request with its id is recorded already.
    pub fn start_request(&self, r: &Request) -> Result<bool> {
        let n = self.exec(
            "INSERT INTO requests(id, method, by, at) VALUES(?,?,?,?) ON CONFLICT(id) DO NOTHING",
            params![r.id, r.method, doc(&r.by)?, ms(r.at)],
        )?;
        Ok(n == 1)
    }

    /// Keeps an action's answer: what it returned, or why it failed (err_text), the kind of that failure (err_kind) and
    /// what a channel acts on beyond the kind (err_detail).
    pub fn finish_request(
        &self,
        id: &str,
        answer: Option<&ForeignJson>,
        err_text: &str,
        err_kind: &str,
        err_detail: Option<&ForeignJson>,
    ) -> Result<()> {
        self.exec(
            "UPDATE requests SET done=1, answer=?, error=?, error_kind=?, error_detail=? WHERE id=?",
            params![
                answer.map(ForeignJson::text),
                err_text,
                err_kind,
                err_detail.map(ForeignJson::text),
                id
            ],
        )?;
        Ok(())
    }

    /// Records every action still under way as ended with err_text: the process doing it has gone.
    pub fn end_requests(&self, err_text: &str) -> Result<()> {
        self.exec("UPDATE requests SET done=1, error=? WHERE done=0", [err_text])?;
        Ok(())
    }

    /// Drops the actions recorded before t: past the time any channel would ask again.
    pub fn forget_requests(&self, t: Time) -> Result<()> {
        self.exec("DELETE FROM requests WHERE at < ?", [ms(t)])?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The action recorded under id.
    pub fn request(&self, id: &str) -> Result<Request> {
        type Row = (
            String,
            String,
            String,
            bool,
            Option<String>,
            String,
            String,
            Option<String>,
            i64,
        );
        let (id, method, by, done, answer, error, error_kind, error_detail, at): Row = found(
            self.c
                .prepare_cached(
                    "SELECT id, method, by, done, answer, error, error_kind, error_detail, at FROM requests WHERE id=?",
                )?
                .query_row([id], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?,
                    ))
                }),
        )?;
        Ok(Request {
            id,
            method,
            by: serde_json::from_str(&by)?,
            done,
            answer: answer.as_deref().map(ForeignJson::parse).transpose()?,
            error,
            error_kind,
            error_detail: error_detail.as_deref().map(ForeignJson::parse).transpose()?,
            at: from_ms(at),
        })
    }

    pub(crate) fn view(&self, chain_id: &str) -> Result<StoredChain> {
        let chain = self.chain(chain_id)?;
        let cards = self.cards(chain_id)?;
        let edges = self.edges(chain_id)?;
        Ok(StoredChain {
            chain,
            cards,
            edges: model::relationships(&edges).into_iter().cloned().collect(),
            decisions: self.decisions(chain_id)?,
            drafts: self.drafts(chain_id)?,
            workflows: self.workflows_of(chain_id)?,
            retrospectives: self.retrospectives_of(chain_id)?,
        })
    }
}
