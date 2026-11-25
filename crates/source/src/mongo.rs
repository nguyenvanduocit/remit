//! The MongoDB source reads a deployment (production or staging, whichever `MONGODB_URI` names). The connection string holds
//! the login, so it never leaves this file: an error is scrubbed of it, no result carries it, and the driver sits behind
//! `MongoClient`, which has no way to write (`driver.rs`). The queries an agent writes are Extended JSON, checked by the guards
//! (`mongoguard.rs`, `query.rs`) before the driver sees them.

mod driver;
mod query;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use bson::{Bson, Document, doc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

use crate::env::{Env, missing};
use crate::error::{BoxFuture, Error, Result};
use crate::mongoguard::{MONGO_SYSTEM_DATABASES, check_database_name, check_namespace};
use crate::redact::{redact_text, scrub};
use crate::source::{Provider, Shared, Tool, Var, new_tool};
use crate::when::duration_text;
use driver::{MONGO_CONNECT_WAIT, MONGO_OP_TIMEOUT, connect_mongo};
use query::{
    MONGO_COUNT_CAP, MONGO_LIMIT, MONGO_SAMPLE, default_database, infer_schema, mongo_schema_table, parse_filter,
    parse_pipeline, parse_projection, parse_sort, render_docs, to_json,
};

/// Why a call to MongoDB failed, as far as the answer to the agent depends on it.
#[derive(Debug)]
pub(crate) enum MongoError {
    /// No member of the deployment answered.
    Unreachable,
    /// The operation ran past its time.
    TimedOut,
    /// Any other failure, in the driver's or this source's words: scrubbed before an agent reads it.
    Other(String),
}

impl From<Error> for MongoError {
    fn from(e: Error) -> Self {
        Self::Other(e.to_string())
    }
}

pub(crate) type MongoResult<T> = std::result::Result<T, MongoError>;

/// The whole of what the MongoDB source may ask of a server: six reads. It has no insert, update, delete, drop, index or
/// command method, so no tool can write whatever an agent sends; the driver behind it is the only implementation, and a test
/// fake the other.
pub(crate) trait MongoClient: Send + Sync {
    fn databases(&self) -> BoxFuture<'_, MongoResult<Vec<MongoDatabase>>>;
    fn collections<'a>(&'a self, db: &'a str) -> BoxFuture<'a, MongoResult<Vec<MongoCollection>>>;
    fn estimated_count<'a>(&'a self, db: &'a str, coll: &'a str) -> BoxFuture<'a, MongoResult<i64>>;
    /// The number of documents matching `filter`, counting no further than `limit`.
    fn count<'a>(&'a self, db: &'a str, coll: &'a str, filter: Document, limit: i64)
    -> BoxFuture<'a, MongoResult<i64>>;
    fn find<'a>(&'a self, db: &'a str, coll: &'a str, q: MongoFind) -> BoxFuture<'a, MongoResult<Vec<Document>>>;
    fn aggregate<'a>(
        &'a self,
        db: &'a str,
        coll: &'a str,
        pipeline: Vec<Document>,
    ) -> BoxFuture<'a, MongoResult<Vec<Document>>>;
}

#[derive(Debug, Clone)]
pub(crate) struct MongoDatabase {
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct MongoCollection {
    pub name: String,
    pub view: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MongoFind {
    pub filter: Document,
    pub projection: Document,
    pub sort: Document,
    pub limit: i64,
}

/// Opens a client for a connection string.
pub(crate) type Opener = Arc<dyn Fn(String) -> BoxFuture<'static, MongoResult<Arc<dyn MongoClient>>> + Send + Sync>;

pub(crate) struct Mongo {
    env: Env,
    uri: String,
    open: Opener,
    /// The client, opened on first use and kept: opening is a TLS handshake and a topology scan. Dropping the source (its
    /// connection's values changed, or it was removed) frees it.
    client: Mutex<Option<Arc<dyn MongoClient>>>,
}

pub(crate) fn new(env: Env, _: Shared) -> Arc<dyn Provider> {
    Arc::new(Mongo::on(env, Arc::new(connect_mongo)))
}

impl Mongo {
    /// The source that opens its client with `open`: the driver in `new`, a fake in a test.
    pub(crate) fn on(env: Env, open: Opener) -> Self {
        Self {
            uri: env.get("MONGODB_URI"),
            env,
            open,
            client: Mutex::new(None),
        }
    }

    async fn connect(&self) -> MongoResult<Arc<dyn MongoClient>> {
        let mut held = self.client.lock().await;
        if let Some(c) = held.as_ref() {
            return Ok(Arc::clone(c));
        }
        let c = (self.open)(self.uri.clone()).await?;
        *held = Some(Arc::clone(&c));
        Ok(c)
    }

    /// `e` as an agent may read it: without the connection string or the login in it (a driver error can echo either), without
    /// the topology dump of a server-selection error (it lists every host), and with a timeout said plainly.
    fn scrub(&self, e: MongoError) -> Error {
        match e {
            MongoError::Unreachable => unreachable_error(),
            MongoError::Other(m) if m.contains("server selection error") => unreachable_error(),
            MongoError::TimedOut => Error::msg(format!(
                "MongoDB did not finish within {}: narrow the filter to an indexed field (_id, shop_id, a date range) or lower the limit",
                duration_text(MONGO_OP_TIMEOUT)
            )),
            MongoError::Other(m) => Error::msg(redact_text(&scrub(&m, &self.login_parts()))),
        }
    }

    /// The strings of the connection string that must not be printed: the whole of it, the user and the password, as written
    /// and as decoded.
    fn login_parts(&self) -> Vec<String> {
        let mut parts = vec![self.uri.clone()];
        if let Some((_, rest)) = self.uri.split_once("://")
            && let Some((login, _)) = rest.split_once('@')
        {
            let (user, pass) = login.split_once(':').unwrap_or((login, ""));
            parts.extend([login.to_owned(), pass.to_owned(), user.to_owned()]);
            parts.extend([user, pass].into_iter().filter_map(urlescape::unescape));
        }
        parts.retain(|s| s.len() >= 3);
        parts
    }

    /// The database and collection of a call, after the checks that keep a read to the product's data. A database left empty
    /// is the one the connection string names.
    fn target(&self, db: &str, coll: &str) -> Result<(String, String)> {
        let db = if db.is_empty() {
            default_database(&self.uri)
        } else {
            db.to_owned()
        };
        if db.is_empty() {
            return Err(Error::msg(
                "database is empty and MONGODB_URI names none: pass a database (mongo_collections lists them)",
            ));
        }
        check_namespace(&db, coll)?;
        Ok((db, coll.to_owned()))
    }
}

fn unreachable_error() -> Error {
    Error::msg(format!(
        "MongoDB is not reachable from this server (no member answered within {})",
        duration_text(MONGO_CONNECT_WAIT)
    ))
}

/// A tool whose failures are scrubbed of the connection string by `m`.
fn mongo_tool<In, F, Fut>(m: &Arc<Mongo>, name: &str, description: &str, run: F) -> Tool
where
    In: serde::de::DeserializeOwned + JsonSchema + Send + 'static,
    F: Fn(Arc<Mongo>, In) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MongoResult<String>> + Send + 'static,
{
    let m = Arc::clone(m);
    let run = Arc::new(run);
    new_tool(name, description, move |input: In| {
        let m = Arc::clone(&m);
        let run = Arc::clone(&run);
        async move { run(Arc::clone(&m), input).await.map_err(|e| m.scrub(e)) }
    })
}

impl Provider for Mongo {
    fn name(&self) -> String {
        "MongoDB".to_owned()
    }

    fn missing(&self) -> Vec<String> {
        missing(&self.env, &["MONGODB_URI"])
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            Var::plain(
                "MONGODB_URI",
                "the connection string of the MongoDB deployment, login included; the address inside it is where it connects",
            )
            .secret(),
        ]
    }

    fn tools(self: Arc<Self>) -> Vec<Tool> {
        vec![
            mongo_tool(
                &self,
                "mongo_collections",
                "List what MongoDB holds. With no database it lists the databases and names the default one. With a database it lists the collections with approximate document counts; the many collections that belong to one shop (orders_<shopID>, ad-insights_<shopID>) are counted as one line per kind, and `shop` lists just one shop's. Use `match` to narrow by a word of the name. Start here to learn the names for the other mongo_ tools.",
                |m, input: CollectionsIn| async move { m.collections(input).await },
            ),
            mongo_tool(
                &self,
                "mongo_find",
                "Find documents in a collection by an Extended JSON filter, e.g. {\"shop_id\": 66635923611, \"created_at\": {\"$gte\": {\"$date\": \"2026-10-01T00:00:00Z\"}}}; ObjectId is {\"$oid\": \"...\"}. Optional projection ({\"name\": 1}) and sort ({\"created_at\": -1}). At most `limit` documents (default 20, max 100); values of credential fields come back as [removed] and long texts and lists are cut. A filter that names no indexed field can time out: filter by _id, shop_id or a date range. Learn field names with mongo_schema first.",
                |m, input: FindIn| async move { m.find(input).await },
            ),
            mongo_tool(
                &self,
                "mongo_count",
                "Count the documents of a collection that match an Extended JSON filter. With no filter it returns the collection's quick estimate; with a filter it counts exactly, up to 1,000,000.",
                |m, input: CountIn| async move { m.count(input).await },
            ),
            mongo_tool(
                &self,
                "mongo_aggregate",
                "Run a read-only aggregation pipeline (a JSON array of stages: $match, $group, $project, $sort, $lookup and the others that only shape a read) for sums, group-bys and joins that find cannot do. $out, $merge, JavaScript operators and reaching a credential field are refused. The pipeline is cut at `limit` documents (default 20, max 100). Put $match first, on an indexed field (shop_id, _id, a date range).",
                |m, input: AggregateIn| async move { m.aggregate(input).await },
            ),
            mongo_tool(
                &self,
                "mongo_schema",
                "Learn the shape of a collection without reading its data: samples documents at random (default 100, max 500; optionally only those matching a filter) and lists every field path with the types seen and how many sampled documents have it. Arrays show as path[]. Use before writing a filter or a pipeline.",
                |m, input: SchemaIn| async move { m.schema(input).await },
            ),
        ]
    }
}

// --- mongo_collections ---

#[derive(Deserialize, JsonSchema)]
struct CollectionsIn {
    /// the database to list collections of; empty lists the databases
    #[serde(default)]
    database: String,
    /// keep collections whose name contains this word (case-insensitive)
    #[serde(default, rename = "match")]
    pattern: String,
    /// a shop id: list the collections of that one shop (names ending _<shopID>)
    #[serde(default)]
    shop: String,
}

#[derive(Serialize)]
struct DatabaseRow {
    name: String,
    #[serde(rename = "sizeMB")]
    size_mb: i64,
}

#[derive(Serialize)]
struct CollectionRow {
    name: String,
    #[serde(rename = "approxDocs", skip_serializing_if = "Option::is_none")]
    docs: Option<i64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    view: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ShopKind {
    /// orders_<shopID>
    collection: String,
    shops: usize,
}

#[derive(Serialize, Default)]
struct CollectionsOut {
    #[serde(rename = "defaultDatabase", skip_serializing_if = "String::is_empty")]
    default: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    databases: Vec<DatabaseRow>,
    #[serde(skip_serializing_if = "String::is_empty")]
    database: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    collections: Vec<CollectionRow>,
    #[serde(rename = "shopBound", skip_serializing_if = "Vec::is_empty")]
    shop_bound: Vec<ShopKind>,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
}

/// How many collections one listing counts: each count is a call to the server.
const MONGO_COLLECTION_ROWS: usize = 60;
/// How many counts run at once.
const MONGO_COUNTS_AT_ONCE: usize = 8;

impl Mongo {
    async fn collections(&self, input: CollectionsIn) -> MongoResult<String> {
        let c = self.connect().await?;
        if input.database.is_empty() {
            return self.databases(c.as_ref()).await;
        }
        check_database_name(&input.database)?;
        let all = c.collections(&input.database).await?;
        let (mut plain, kinds) = plan_collections(all, &input.pattern, &input.shop);
        let mut out = CollectionsOut {
            database: input.database.clone(),
            shop_bound: kinds,
            ..CollectionsOut::default()
        };
        if plain.len() > MONGO_COLLECTION_ROWS {
            out.note = format!(
                "{} more collections not shown; narrow with match",
                plain.len().saturating_sub(MONGO_COLLECTION_ROWS)
            );
            plain.truncate(MONGO_COLLECTION_ROWS);
        }
        out.collections = count_collections(&c, &input.database, plain).await;
        if out.collections.is_empty() && out.shop_bound.is_empty() {
            "no collection matches".clone_into(&mut out.note);
        } else if !out.shop_bound.is_empty() {
            out.note = format!("{} Pass shop=<shopID> to list one shop's collections.", out.note)
                .trim()
                .to_owned();
        }
        Ok(to_json(&out)?)
    }

    async fn databases(&self, c: &dyn MongoClient) -> MongoResult<String> {
        let dbs = c.databases().await?;
        let out = CollectionsOut {
            default: default_database(&self.uri),
            databases: dbs
                .into_iter()
                .filter(|d| !MONGO_SYSTEM_DATABASES.contains(&d.name.as_str()))
                .map(|d| DatabaseRow {
                    name: d.name,
                    size_mb: i64::try_from(d.size_bytes >> 20).unwrap_or(i64::MAX),
                })
                .collect(),
            ..CollectionsOut::default()
        };
        Ok(to_json(&out)?)
    }
}

/// Splits the collections of a database into those listed one by one and the kinds that repeat per shop (orders_<shopID>): a
/// deployment holds thousands of the second, and an agent needs the kind, not every shop. With `shop` set, the collections of
/// that shop are the plain ones.
fn plan_collections(all: Vec<MongoCollection>, pattern: &str, shop: &str) -> (Vec<MongoCollection>, Vec<ShopKind>) {
    let pattern = pattern.to_lowercase();
    let mut plain = Vec::new();
    let mut per_kind: BTreeMap<String, usize> = BTreeMap::new();
    for c in all {
        if c.name.starts_with("system.") {
            continue;
        }
        if !pattern.is_empty() && !c.name.to_lowercase().contains(&pattern) {
            continue;
        }
        match split_shop(&c.name) {
            Some((_, id)) if !shop.is_empty() => {
                if id == shop {
                    plain.push(c);
                }
            }
            None if !shop.is_empty() => {}
            Some((prefix, _)) => {
                let n = per_kind.entry(prefix.to_owned()).or_default();
                *n = n.saturating_add(1);
            }
            None => plain.push(c),
        }
    }
    plain.sort_by(|a, b| a.name.cmp(&b.name));
    let mut kinds: Vec<ShopKind> = per_kind
        .into_iter()
        .map(|(prefix, shops)| ShopKind {
            collection: format!("{prefix}_<shopID>"),
            shops,
        })
        .collect();
    kinds.sort_by(|a, b| a.collection.cmp(&b.collection));
    (plain, kinds)
}

/// A name such as orders_72516665583 read as a kind and a shop id: an underscore and at least six digits.
fn split_shop(name: &str) -> Option<(&str, &str)> {
    let (prefix, id) = name.rsplit_once('_')?;
    (!prefix.is_empty() && id.len() >= 6 && id.bytes().all(|b| b.is_ascii_digit())).then_some((prefix, id))
}

/// The collections with the quick estimate of their documents, a few counts at a time. A count that fails stays unknown (the
/// collection is still listed).
async fn count_collections(c: &Arc<dyn MongoClient>, db: &str, colls: Vec<MongoCollection>) -> Vec<CollectionRow> {
    let mut rows: Vec<CollectionRow> = colls
        .iter()
        .map(|coll| CollectionRow {
            name: coll.name.clone(),
            docs: None,
            view: coll.view,
        })
        .collect();
    let slots = Arc::new(Semaphore::new(MONGO_COUNTS_AT_ONCE));
    let mut counts = JoinSet::new();
    for (i, coll) in colls.into_iter().enumerate().filter(|(_, c)| !c.view) {
        let c = Arc::clone(c);
        let slots = Arc::clone(&slots);
        let db = db.to_owned();
        counts.spawn(async move {
            let _slot = slots.acquire_owned().await.ok()?;
            c.estimated_count(&db, &coll.name).await.ok().map(|n| (i, n))
        });
    }
    while let Some(done) = counts.join_next().await {
        if let Ok(Some((i, n))) = done
            && let Some(row) = rows.get_mut(i)
        {
            row.docs = Some(n);
        }
    }
    rows
}

// --- mongo_find, mongo_count, mongo_aggregate, mongo_schema ---

#[derive(Deserialize, JsonSchema)]
struct FindIn {
    /// database; empty is the one MONGODB_URI names
    #[serde(default)]
    database: String,
    /// collection name, e.g. products or orders_66635923611
    collection: String,
    /// Extended JSON object; empty matches every document
    #[serde(default)]
    filter: String,
    /// Extended JSON object of fields to return, e.g. {"name": 1, "_id": 0}
    #[serde(default)]
    projection: String,
    /// Extended JSON object of field to 1 or -1, e.g. {"created_at": -1}
    #[serde(default)]
    sort: String,
    /// most documents to return; default 20, max 100
    #[serde(default)]
    limit: i64,
}

#[derive(Deserialize, JsonSchema)]
struct CountIn {
    /// database; empty is the one MONGODB_URI names
    #[serde(default)]
    database: String,
    /// collection name
    collection: String,
    /// Extended JSON object; empty returns the collection's quick estimate
    #[serde(default)]
    filter: String,
}

#[derive(Serialize)]
struct CountOut {
    collection: String,
    count: i64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    estimated: bool,
    #[serde(rename = "atLeast", skip_serializing_if = "std::ops::Not::not")]
    at_least: bool,
}

#[derive(Deserialize, JsonSchema)]
struct AggregateIn {
    /// database; empty is the one MONGODB_URI names
    #[serde(default)]
    database: String,
    /// collection the pipeline starts from
    collection: String,
    /// Extended JSON array of stages, e.g. [{"$match": {"shop_id": 1}}, {"$group": {"_id": "$status", "n": {"$sum": 1}}}]
    pipeline: String,
    /// most documents to return; default 20, max 100
    #[serde(default)]
    limit: i64,
}

#[derive(Deserialize, JsonSchema)]
struct SchemaIn {
    /// database; empty is the one MONGODB_URI names
    #[serde(default)]
    database: String,
    /// collection to sample
    collection: String,
    /// Extended JSON object: sample only the documents that match (e.g. one shop's rows of a shared collection)
    #[serde(default)]
    filter: String,
    /// how many documents to sample; default 100, max 500
    #[serde(default)]
    sample: i64,
}

/// `n` documents as the driver counts them; every `n` is below the small ceiling of a `Limit`.
fn documents(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

impl Mongo {
    async fn find(&self, input: FindIn) -> MongoResult<String> {
        let (db, coll) = self.target(&input.database, &input.collection)?;
        let limit = MONGO_LIMIT.of("limit", input.limit)?;
        let q = MongoFind {
            filter: parse_filter(&input.filter)?,
            projection: parse_projection(&input.projection)?,
            sort: parse_sort(&input.sort)?,
            // one more than asked tells whether the answer is the whole result
            limit: documents(limit).saturating_add(1),
        };
        let docs = self.connect().await?.find(&db, &coll, q).await?;
        Ok(render_docs(&format!("{db}.{coll}"), docs, limit)?)
    }

    async fn count(&self, input: CountIn) -> MongoResult<String> {
        let (db, coll) = self.target(&input.database, &input.collection)?;
        let filter = parse_filter(&input.filter)?;
        let c = self.connect().await?;
        let mut out = CountOut {
            collection: format!("{db}.{coll}"),
            count: 0,
            estimated: false,
            at_least: false,
        };
        if filter.is_empty() {
            out.estimated = true;
            out.count = c.estimated_count(&db, &coll).await?;
        } else {
            out.count = c.count(&db, &coll, filter, MONGO_COUNT_CAP).await?;
            out.at_least = out.count >= MONGO_COUNT_CAP;
        }
        Ok(to_json(&out)?)
    }

    async fn aggregate(&self, input: AggregateIn) -> MongoResult<String> {
        let (db, coll) = self.target(&input.database, &input.collection)?;
        let mut pipeline = parse_pipeline(&input.pipeline)?;
        let limit = MONGO_LIMIT.of("limit", input.limit)?;
        // Whatever the pipeline ends with, the result is cut here, one past the limit so the answer can say it was cut.
        pipeline.push(doc! {"$limit": Bson::Int64(documents(limit).saturating_add(1))});
        let docs = self.connect().await?.aggregate(&db, &coll, pipeline).await?;
        Ok(render_docs(&format!("{db}.{coll}"), docs, limit)?)
    }

    async fn schema(&self, input: SchemaIn) -> MongoResult<String> {
        let (db, coll) = self.target(&input.database, &input.collection)?;
        let filter = parse_filter(&input.filter)?;
        let size = MONGO_SAMPLE.of("sample", input.sample)?;
        let mut pipeline = Vec::new();
        if !filter.is_empty() {
            pipeline.push(doc! {"$match": filter});
        }
        pipeline.push(doc! {"$sample": {"size": Bson::Int64(documents(size))}});
        let docs = self.connect().await?.aggregate(&db, &coll, pipeline).await?;
        if docs.is_empty() {
            return Ok(format!(
                "{db}.{coll}: no documents sampled (an empty collection, or nothing matches the filter; a shop's collections end _<shopID>)"
            ));
        }
        let fields = infer_schema(&docs);
        Ok(format!(
            "{db}.{coll}: {} documents sampled at random, {} field paths\n{}",
            docs.len(),
            fields.len(),
            mongo_schema_table(&fields, docs.len())
        ))
    }
}
