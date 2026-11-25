//! Bytebase is the gate to the databases only its service account can reach (MongoDB, PostgreSQL, StarRocks, BigQuery). It
//! is read through Bytebase's v1 REST API with the service account's login: `BYTEBASE_URL`, `BYTEBASE_SA_EMAIL` and
//! `BYTEBASE_SA_KEY`. The token the login returns lives an hour; it is minted when first needed, kept until shortly before
//! it ends, and minted again when Bytebase answers 401.
//!
//! Three calls are made and no others: GET on the instance, database and metadata listings, POST /v1/auth/login, and POST
//! of one checked statement to the database's `:query` endpoint, which Bytebase offers for reading. Nothing here proposes a
//! change, calls an arbitrary endpoint or sends an agent's path: the instance and database an agent names are looked up in
//! Bytebase's own listings, and every path is built from them and fixed segments.

mod guard;
mod mongoshell;

use std::sync::{Arc, LazyLock};

use base64::Engine as _;
use jiff::{SignedDuration, Timestamp};
use regex::Regex;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::Mutex;

use self::guard::{BYTEBASE_FETCH_ROWS, ENGINE_NAMES, StatementPlan, engine};
use crate::api::{Api, Http, Query};
use crate::env::{Clock, Env, missing};
use crate::error::{Error, Result, err, fail};
use crate::redact::{REMOVED, redact_value, render, sensitive_key};
use crate::source::{Provider, Shared, Tool, Var, new_tool};
use crate::text::{cut_bytes, table};

const NAME: &str = "Bytebase";

/// How long before a token ends it is replaced, and how long a token without an `exp` claim is used.
const TOKEN_MARGIN: SignedDuration = SignedDuration::from_mins(5);
const TOKEN_FALLBACK: SignedDuration = SignedDuration::from_mins(50);

const BYTEBASE_PAGES: usize = 20;
const BYTEBASE_PER_PAGE: usize = 500;

/// How many databases one answer names.
const MAX_LISTED_DATABASES: usize = 150;

/// The tables one answer names; `DETAIL_TABLES` is how many matches get their columns.
const MAX_SCHEMA_TABLES: usize = 100;
const DETAIL_TABLES: usize = 10;
const MAX_COLUMNS: usize = 120;
const MAX_INDEXES: usize = 12;

/// How much of a result an agent reads; `BYTEBASE_FETCH_ROWS` is how much is fetched, so the count of what was left out is
/// true.
const SHOWN_ROWS: usize = 100;
const SHOWN_DOCUMENTS: usize = 40;
const MAX_CELL: usize = 1000;
const MAX_DOCUMENT: usize = 4000;

/// What an instance id or a database name may be to be put in a path: no slash, colon or other character that could make
/// Bytebase read the path as another route.
#[expect(clippy::expect_used, reason = "a constant pattern: a test runs it")]
static BYTEBASE_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_.$-]+$").expect("name pattern"));

/// The token the login gave and when to stop using it.
#[derive(Default)]
struct Login {
    token: String,
    expires: Option<Timestamp>,
}

struct Bytebase {
    env: Env,
    now: Clock,
    http: Http,
    url: String,
    email: String,
    key: String,
    /// Held across the login, so two calls that both need a token mint one.
    login: Mutex<Login>,
}

pub(crate) fn new(env: Env, shared: Shared) -> Arc<dyn Provider> {
    Arc::new(Bytebase {
        url: env.get("BYTEBASE_URL"),
        email: env.get("BYTEBASE_SA_EMAIL"),
        key: env.get("BYTEBASE_SA_KEY"),
        env,
        now: shared.now,
        http: shared.http,
        login: Mutex::new(Login::default()),
    })
}

impl Provider for Bytebase {
    fn name(&self) -> String {
        NAME.to_owned()
    }

    fn missing(&self) -> Vec<String> {
        missing(&self.env, &["BYTEBASE_URL", "BYTEBASE_SA_EMAIL", "BYTEBASE_SA_KEY"])
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            Var::plain("BYTEBASE_URL", "the address of the Bytebase server").address(),
            Var::plain("BYTEBASE_SA_EMAIL", "the email of a service account that may only read"),
            Var::plain("BYTEBASE_SA_KEY", "the service account's key").secret(),
        ]
    }

    fn tools(self: Arc<Self>) -> Vec<Tool> {
        let (d, s, q) = (Arc::clone(&self), Arc::clone(&self), self);
        vec![
            new_tool(
                "bytebase_databases",
                "List the database instances Bytebase reaches and the databases in each: instance id, engine (MONGODB, POSTGRES, STARROCKS, BIGQUERY), environment, project, and the language bytebase_query speaks for that engine. Start here to learn what exists; narrow with a word of an instance, engine, project or database name.",
                move |input: DatabasesInput| {
                    let b = Arc::clone(&d);
                    async move { b.databases(input).await }
                },
            ),
            new_tool(
                "bytebase_schema",
                "A database's tables (collections for MongoDB) with row counts, and for a few matching tables their columns and indexes, as Bytebase last synced them. Name the instance and the database as bytebase_databases lists them; `table` narrows by a word of the name. A MongoDB collection has no fixed columns: read a few documents with bytebase_query to see its fields.",
                move |input: SchemaInput| {
                    let b = Arc::clone(&s);
                    async move { b.schema(input).await }
                },
            ),
            new_tool(
                "bytebase_query",
                "Read from a production or staging database through Bytebase: one statement, read-only. SQL engines take SELECT, WITH, SHOW, DESCRIBE or EXPLAIN; MongoDB takes one mongosh call such as db.orders.find({status: \"paid\"}).sort({created_at: -1}).limit(10), findOne, aggregate([...]), countDocuments, distinct or estimatedDocumentCount. Rows are capped and credential-named fields are removed; Bytebase may mask data further. Check the engine with bytebase_databases first: it decides the syntax.",
                move |input: QueryInput| {
                    let b = Arc::clone(&q);
                    async move { b.query(input).await }
                },
            ),
        ]
    }
}

// ---- the login ----

#[derive(Serialize)]
struct LoginBody<'a> {
    email: &'a str,
    password: &'a str,
    web: bool,
}

/// The body of a query: the statement and the most rows to answer. Nothing else is sent.
#[derive(Serialize)]
struct QueryBody<'a> {
    statement: &'a str,
    limit: usize,
}

impl Bytebase {
    /// A token that is valid now. `stale` is a token Bytebase just refused: it is never returned again.
    async fn bearer(&self, stale: &str) -> Result<String> {
        let mut login = self.login.lock().await;
        let now = self.now.now();
        if !login.token.is_empty() && login.token != stale && login.expires.is_some_and(|e| now < e) {
            return Ok(login.token.clone());
        }
        let api = Api {
            name: NAME.to_owned(),
            base: self.url.clone(),
            headers: Vec::new(),
            http: self.http.clone(),
        };
        // The one POST that is not a query: it exchanges the service account's key for a token and changes nothing in
        // Bytebase.
        let body = LoginBody {
            email: &self.email,
            password: &self.key,
            web: false,
        };
        let raw = api
            .post_json("/v1/auth/login", &body)
            .await
            .map_err(|e| e.context("bytebase login failed"))?;
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            token: String,
        }
        let token = serde_json::from_slice::<Answer>(&raw)
            .ok()
            .map(|a| a.token)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| Error::msg("bytebase login answered no token"))?;
        login.expires = Some(token_expiry(&token, now));
        login.token.clone_from(&token);
        Ok(token)
    }

    /// The answer to GET `path`, or to a POST of `body` there, with a token, and once more with a new one when Bytebase
    /// refuses it.
    async fn call(&self, path: &str, query: &Query, body: Option<&QueryBody<'_>>) -> Result<Vec<u8>> {
        let token = self.bearer("").await?;
        match self.send(&token, path, query, body).await {
            Err(e) if e.status().is_some_and(|s| s.code == 401) => {
                let token = self.bearer(&token).await?;
                self.send(&token, path, query, body).await
            }
            other => other,
        }
    }

    async fn send(&self, token: &str, path: &str, query: &Query, body: Option<&QueryBody<'_>>) -> Result<Vec<u8>> {
        let api = Api {
            name: NAME.to_owned(),
            base: self.url.clone(),
            headers: vec![("Authorization".to_owned(), format!("Bearer {token}"))],
            http: self.http.clone(),
        };
        match body {
            Some(b) => api.post_json(path, b).await,
            None => api.get(path, query).await,
        }
    }

    async fn get(&self, path: &str, query: &Query) -> Result<Vec<u8>> {
        self.call(path, query, None).await
    }

    /// Sends `statement`, already checked for its engine, to `database` of `instance`. `:query` is Bytebase's read endpoint
    /// (the SQL editor's); the POST carries the statement in a body because it can be long.
    async fn run_query(&self, instance: &str, database: &str, statement: &str) -> Result<Vec<u8>> {
        let path = format!("{}:query", database_path(instance, database)?);
        let body = QueryBody {
            statement,
            limit: BYTEBASE_FETCH_ROWS,
        };
        self.call(&path, &Query::new(), Some(&body)).await
    }
}

/// When to stop using `token`: its own `exp` claim less a margin, else a fixed time under the hour it lives.
fn token_expiry(token: &str, now: Timestamp) -> Timestamp {
    #[derive(Deserialize)]
    struct Claims {
        #[serde(default)]
        exp: i64,
    }
    let parts: Vec<&str> = token.split('.').collect();
    if let [_, payload, _] = parts.as_slice()
        && let Ok(raw) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)
        && let Ok(claims) = serde_json::from_slice::<Claims>(&raw)
        && claims.exp > 0
        && let Some(at) = Timestamp::from_second(claims.exp)
            .ok()
            .and_then(|t| t.checked_sub(TOKEN_MARGIN).ok())
    {
        return at;
    }
    now.checked_add(TOKEN_FALLBACK).unwrap_or(now)
}

/// The path of a database, refused when a name could make Bytebase read it as another route. A name that passes holds no
/// character a path segment escapes.
fn database_path(instance: &str, database: &str) -> Result<String> {
    if !BYTEBASE_NAME.is_match(instance) || !BYTEBASE_NAME.is_match(database) {
        return fail!("bytebase: {database:?} on {instance:?} cannot be addressed");
    }
    Ok(format!("/v1/instances/{instance}/databases/{database}"))
}

// ---- the listings ----

impl Bytebase {
    /// Every item of `field` in the pages of `path`.
    async fn list<T: DeserializeOwned>(&self, path: &str, field: &str) -> Result<Vec<T>> {
        let mut all = Vec::new();
        let mut q: Query = vec![("pageSize".to_owned(), BYTEBASE_PER_PAGE.to_string())];
        for _ in 0..BYTEBASE_PAGES {
            let raw = self.get(path, &q).await?;
            let page: Map<String, Value> =
                serde_json::from_slice(&raw).map_err(|e| err!("bytebase: the listing {path} is not JSON: {e}"))?;
            match page.get(field) {
                None | Some(Value::Null) => {}
                Some(v) => all.extend(
                    Vec::<T>::deserialize(v)
                        .map_err(|e| err!("bytebase: the listing {path} has an unexpected shape: {e}"))?,
                ),
            }
            // An absent token is the end of the listing.
            let next = page.get("nextPageToken").and_then(Value::as_str).unwrap_or_default();
            if next.is_empty() {
                return Ok(all);
            }
            q.retain(|(k, _)| k != "pageToken");
            q.push(("pageToken".to_owned(), next.to_owned()));
        }
        fail!("bytebase: {path} lists more than {BYTEBASE_PAGES} pages")
    }

    async fn instances(&self) -> Result<Vec<Instance>> {
        let list: Vec<Instance> = self.list("/v1/instances", "instances").await?;
        Ok(list
            .into_iter()
            .filter(|i| i.state != "DELETED" && BYTEBASE_NAME.is_match(i.id()))
            .collect())
    }

    async fn databases_of(&self, i: &Instance) -> Result<Vec<DatabaseRecord>> {
        // The id passed the name pattern in `instances`, so it needs no escaping.
        let list: Vec<DatabaseRecord> = self
            .list(&format!("/v1/instances/{}/databases", i.id()), "databases")
            .await?;
        Ok(list
            .into_iter()
            .filter(|d| d.state != "DELETED" && !d.short().is_empty())
            .collect())
    }

    /// The instance and the database an agent named, as Bytebase knows them.
    async fn resolve(&self, instance: &str, database: &str) -> Result<(Instance, String)> {
        let list = self.instances().await?;
        let inst = pick_instance(&list, instance)?.clone();
        let names: Vec<String> = self
            .databases_of(&inst)
            .await?
            .iter()
            .map(|d| d.short().to_owned())
            .collect();
        if names.iter().any(|n| n == database) {
            return Ok((inst, database.to_owned()));
        }
        fail!(
            "instance {} has no database {database:?}; it has {}",
            inst.id(),
            first_names(&names, 20)
        )
    }
}

/// The fields of an instance this source reads. It leaves out the data sources, which carry hosts and logins, so they cannot
/// reach an answer.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Instance {
    name: String,
    title: String,
    engine: String,
    environment: String,
    state: String,
}

impl Instance {
    fn id(&self) -> &str {
        self.name.strip_prefix("instances/").unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct DatabaseRecord {
    name: String,
    state: String,
    project: String,
}

impl DatabaseRecord {
    fn short(&self) -> &str {
        self.name.split_once("/databases/").map(|(_, n)| n).unwrap_or_default()
    }
}

/// The instance `reference` names: its id (with or without "instances/"), its title, or a word that only one instance's id
/// or title contains.
fn pick_instance<'a>(list: &'a [Instance], reference: &str) -> Result<&'a Instance> {
    let trimmed = reference.trim();
    let r = trimmed.strip_prefix("instances/").unwrap_or(trimmed).to_lowercase();
    if r.is_empty() {
        return fail!("instance is empty: bytebase_databases lists the instances");
    }
    let mut partial = Vec::new();
    for i in list {
        if i.id().to_lowercase() == r || i.title.to_lowercase() == r {
            return Ok(i);
        }
        if i.id().to_lowercase().contains(&r) || i.title.to_lowercase().contains(&r) {
            partial.push(i);
        }
    }
    match partial.as_slice() {
        [one] => Ok(one),
        [] => fail!("no instance {r:?}: bytebase_databases lists them"),
        many => {
            let ids: Vec<&str> = many.iter().map(|p| p.id()).collect();
            fail!(
                "{r:?} matches {} instances ({}): give one id",
                many.len(),
                ids.join(", ")
            )
        }
    }
}

/// The first `most` of `names`, joined, with the count of those left out.
fn first_names(names: &[String], most: usize) -> String {
    match names.get(..most) {
        Some(head) if names.len() > most => {
            format!("{} and {} more", head.join(", "), names.len().saturating_sub(most))
        }
        _ => names.join(", "),
    }
}

/// The part of a resource name after its first slash: "environments/prod" is "prod".
fn resource_id(resource: &str) -> &str {
    resource.split_once('/').map_or(resource, |(_, n)| n)
}

// ---- bytebase_databases ----

#[derive(Deserialize, JsonSchema)]
struct DatabasesInput {
    /// a word of an instance id or title, an engine (mongodb, postgres, starrocks, bigquery), an environment, a project or a
    /// database name; empty lists everything
    #[serde(default)]
    filter: String,
}

#[derive(Serialize)]
struct ListedDatabase {
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    project: String,
}

#[derive(Serialize)]
struct ListedInstance {
    instance: String,
    title: String,
    engine: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    environment: String,
    language: String,
    databases: Vec<ListedDatabase>,
}

/// Whether any of `values` holds `word`, ignoring case.
fn has_word(word: &str, values: &[&str]) -> bool {
    values.iter().any(|v| v.to_lowercase().contains(word))
}

impl Bytebase {
    async fn databases(&self, input: DatabasesInput) -> Result<String> {
        let list = self.instances().await?;
        let word = input.filter.trim().to_lowercase();
        let mut out = Vec::new();
        let (mut shown, mut hidden) = (0usize, 0usize);
        for inst in &list {
            let dbs = self.databases_of(inst).await?;
            let mut entry = ListedInstance {
                instance: inst.id().to_owned(),
                title: inst.title.clone(),
                engine: inst.engine.clone(),
                environment: resource_id(&inst.environment).to_owned(),
                language: engine(&inst.engine)
                    .map_or("not queried by this tool", |s| s.language)
                    .to_owned(),
                databases: Vec::new(),
            };
            let instance_hit = word.is_empty()
                || has_word(
                    &word,
                    &[&entry.instance, &entry.title, &entry.engine, &entry.environment],
                );
            for d in &dbs {
                let project = resource_id(&d.project);
                if !instance_hit && !has_word(&word, &[d.short(), project]) {
                    continue;
                }
                if shown >= MAX_LISTED_DATABASES {
                    hidden = hidden.saturating_add(1);
                    continue;
                }
                shown = shown.saturating_add(1);
                entry.databases.push(ListedDatabase {
                    name: d.short().to_owned(),
                    project: project.to_owned(),
                });
            }
            if !entry.databases.is_empty() {
                out.push(entry);
            }
        }
        if out.is_empty() {
            return fail!(
                "nothing matches {:?}: leave filter empty to list everything",
                input.filter
            );
        }
        let mut text = render(&out)?;
        if hidden > 0 {
            text.push_str(&format!("\n[{hidden} more databases not shown; narrow by filter]"));
        }
        Ok(text)
    }
}

// ---- bytebase_schema ----

#[derive(Deserialize, JsonSchema)]
struct SchemaInput {
    /// the instance id as bytebase_databases lists it, such as cloud-db1-prod-bsop
    instance: String,
    /// the database name within that instance
    database: String,
    /// a word of the table or collection name; columns and indexes are given when at most 10 match
    #[serde(default)]
    table: String,
}

#[derive(Debug, Default, Deserialize)]
struct Metadata {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    schemas: Vec<Schema>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Schema {
    name: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    tables: Vec<Table>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    views: Vec<Table>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    materialized_views: Vec<Table>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Table {
    name: String,
    row_count: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    columns: Vec<Column>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    indexes: Vec<Index>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Column {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    nullable: bool,
    comment: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Index {
    name: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    expressions: Vec<String>,
    unique: bool,
}

#[derive(Debug, Serialize)]
struct SchemaTable {
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    rows: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    columns: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    indexes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SchemaAnswer {
    instance: String,
    database: String,
    engine: String,
    matching: usize,
    tables: Vec<SchemaTable>,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
}

impl Bytebase {
    async fn schema(&self, input: SchemaInput) -> Result<String> {
        let (inst, database) = self.resolve(&input.instance, &input.database).await?;
        let path = format!("{}/metadata", database_path(inst.id(), &database)?);
        let raw = self.get(&path, &Query::new()).await?;
        let md: Metadata = serde_json::from_slice(&raw).map_err(|e| err!("bytebase: the metadata is not JSON: {e}"))?;
        render(&summarise_schema(
            &inst,
            &database,
            md,
            &input.table.trim().to_lowercase(),
        ))
    }
}

/// The tables of `md` that hold `word`, named and counted, with columns and indexes when few match.
fn summarise_schema(inst: &Instance, database: &str, md: Metadata, word: &str) -> SchemaAnswer {
    let mut all: Vec<(SchemaTable, Table)> = Vec::new();
    for s in md.schemas {
        for (kind, group) in [
            ("", s.tables),
            ("view", s.views),
            ("materialized view", s.materialized_views),
        ] {
            for mut t in group {
                if !s.name.is_empty() {
                    t.name = format!("{}.{}", s.name, t.name);
                }
                if !word.is_empty() && !t.name.to_lowercase().contains(word) {
                    continue;
                }
                let row = SchemaTable {
                    name: t.name.clone(),
                    kind: kind.to_owned(),
                    rows: t.row_count.clone(),
                    columns: Vec::new(),
                    indexes: Vec::new(),
                };
                all.push((row, t));
            }
        }
    }
    all.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    let matching = all.len();
    let mut note = String::new();
    if matching > DETAIL_TABLES {
        note = "give `table` a word of the name to see columns and indexes".to_owned();
    }
    if matching > MAX_SCHEMA_TABLES {
        note = format!(
            "{} more not shown; give `table` a word of the name",
            matching.saturating_sub(MAX_SCHEMA_TABLES)
        );
        all.truncate(MAX_SCHEMA_TABLES);
    }
    let detail = all.len() <= DETAIL_TABLES;
    let tables = all
        .into_iter()
        .map(|(mut row, t)| {
            if detail {
                (row.columns, row.indexes) = describe_table(&t);
            }
            row
        })
        .collect::<Vec<_>>();
    if tables.is_empty() {
        note = "no table or collection matches".to_owned();
    }
    SchemaAnswer {
        instance: inst.id().to_owned(),
        database: database.to_owned(),
        engine: inst.engine.clone(),
        matching,
        tables,
        note,
    }
}

fn describe_table(t: &Table) -> (Vec<String>, Vec<String>) {
    let mut columns: Vec<String> = t
        .columns
        .iter()
        .take(MAX_COLUMNS)
        .map(|c| {
            let mut col = format!("{} {}", c.name, c.kind);
            if c.nullable {
                col.push_str(" NULL");
            }
            if !c.comment.is_empty() {
                col.push_str(" -- ");
                col.push_str(&c.comment);
            }
            col
        })
        .collect();
    if t.columns.len() > MAX_COLUMNS {
        columns.push(format!(
            "[{} more columns]",
            t.columns.len().saturating_sub(MAX_COLUMNS)
        ));
    }
    let mut indexes: Vec<String> = t
        .indexes
        .iter()
        .take(MAX_INDEXES)
        .map(|ix| {
            let mut desc = format!("{}: {}", ix.name, ix.expressions.join(", "));
            if ix.unique {
                desc.push_str(" (unique)");
            }
            desc
        })
        .collect();
    if t.indexes.len() > MAX_INDEXES {
        indexes.push(format!(
            "[{} more indexes]",
            t.indexes.len().saturating_sub(MAX_INDEXES)
        ));
    }
    (columns, indexes)
}

// ---- bytebase_query ----

#[derive(Deserialize, JsonSchema)]
struct QueryInput {
    /// the instance id as bytebase_databases lists it, such as cloud-db1-prod-bsop
    instance: String,
    /// the database name within that instance
    database: String,
    /// one read-only statement in the engine's language: SQL for POSTGRES, STARROCKS and BIGQUERY, a single mongosh call
    /// for MONGODB
    statement: String,
}

#[derive(Debug, Default, Deserialize)]
struct Results {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    results: Vec<ResultSet>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ResultSet {
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    column_names: Vec<String>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    rows: Vec<Row>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    masked: Vec<Value>,
    error: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Row {
    #[serde(deserialize_with = "crate::null_json::null_as_zero")]
    values: Vec<Map<String, Value>>,
}

impl Bytebase {
    async fn query(&self, input: QueryInput) -> Result<String> {
        let (inst, database) = self.resolve(&input.instance, &input.database).await?;
        let Some(spec) = engine(&inst.engine) else {
            return fail!(
                "instance {} is {}: this tool queries {}",
                inst.id(),
                inst.engine,
                ENGINE_NAMES.join(", ")
            );
        };
        let plan = spec
            .plan(&input.statement)
            .map_err(|e| err!("refused, {} reads {}: {e}", inst.id(), spec.language))?;
        let raw = self.run_query(inst.id(), &database, &plan.text).await?;
        let res: Results =
            serde_json::from_slice(&raw).map_err(|e| err!("bytebase: the query answer is not JSON: {e}"))?;
        render_results(&inst, &database, &plan, res)
    }
}

fn render_results(inst: &Instance, database: &str, plan: &StatementPlan, res: Results) -> Result<String> {
    let Some(r) = res.results.into_iter().next() else {
        return fail!("bytebase answered no result");
    };
    if !r.error.is_empty() {
        return fail!("{}/{database} answered an error: {}", inst.id(), r.error);
    }
    let mut rows: Vec<Vec<String>> = r.rows.iter().map(|row| row.values.iter().map(cell).collect()).collect();
    let head = format!("{}/{database} ({})\n", inst.id(), inst.engine);
    let mut body = if inst.engine == "MONGODB" {
        mongo_documents(&rows, plan.count)
    } else {
        sql_table(&r.column_names, &mut rows)
    };
    if rows.len() >= BYTEBASE_FETCH_ROWS {
        body.push_str(&format!(
            "\n[Bytebase was asked for {BYTEBASE_FETCH_ROWS} rows at most; the result may hold more]"
        ));
    }
    let masked = masked_columns(&r.column_names, &r.masked);
    if !masked.is_empty() {
        body.push_str(&format!("\n[masked by Bytebase: {}]", masked.join(", ")));
    }
    Ok(head + &body)
}

/// The columns Bytebase masked: it answers `{}` for a column it left as it is.
fn masked_columns(cols: &[String], masked: &[Value]) -> Vec<String> {
    cols.iter()
        .zip(masked)
        .filter(|(_, m)| !matches!(m, Value::Null) && !matches!(m, Value::Object(o) if o.is_empty()))
        .map(|(c, _)| c.clone())
        .collect()
}

/// The rows laid out, with the cells of a credential-named column removed.
fn sql_table(cols: &[String], rows: &mut [Vec<String>]) -> String {
    for row in rows.iter_mut() {
        for (j, c) in row.iter_mut().enumerate() {
            let credential = cols.get(j).is_some_and(|name| sensitive_key(name));
            if credential && c != "NULL" && !c.is_empty() {
                REMOVED.clone_into(c);
            } else {
                *c = cut_text(c, MAX_CELL);
            }
        }
    }
    table(cols, rows, SHOWN_ROWS)
}

/// One document per line: each row of a MongoDB answer is one value as extended JSON. A document is read back so its
/// credential fields can be removed; a value that is not JSON stays as Bytebase wrote it.
fn mongo_documents(rows: &[Vec<String>], counted: bool) -> String {
    if rows.is_empty() {
        return if counted { r#"{"n":0}"# } else { "[no documents]" }.to_owned();
    }
    let mut lines: Vec<String> = rows
        .iter()
        .take(SHOWN_DOCUMENTS)
        .map(|row| cut_text(&redact_document(&row.join(" ")), MAX_DOCUMENT))
        .collect();
    if rows.len() > SHOWN_DOCUMENTS {
        lines.push(format!(
            "[{} more documents not shown; add a filter, a projection or a smaller limit]",
            rows.len().saturating_sub(SHOWN_DOCUMENTS)
        ));
    }
    lines.join("\n")
}

/// `s` cut at `n` bytes on a character, marked as cut.
fn cut_text(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_owned();
    }
    format!("{}...[cut]", cut_bytes(s, n))
}

/// The first JSON value of `text` with its credential fields removed; any text after that value is left out.
fn redact_document(text: &str) -> String {
    match serde_json::Deserializer::from_str(text).into_iter::<Value>().next() {
        Some(Ok(v)) => serde_json::to_string(&redact_value(&v)).unwrap_or_else(|_| text.to_owned()),
        _ => text.to_owned(),
    }
}

/// One value of a row. Bytebase writes a value as an object with the one key of its type.
fn cell(v: &Map<String, Value>) -> String {
    let Some((kind, raw)) = v.iter().next() else {
        return String::new();
    };
    let text = || match raw {
        Value::String(s) => s.trim().to_owned(),
        other => other.to_string(),
    };
    match kind.as_str() {
        "nullValue" => "NULL".to_owned(),
        "stringValue" | "int32Value" | "int64Value" | "uint32Value" | "uint64Value" | "doubleValue" | "floatValue"
        | "boolValue" => match raw {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        },
        "timestampValue" | "timestampTzValue" => raw
            .get("googleTimestamp")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map_or_else(|| raw.to_string(), str::to_owned),
        "bytesValue" => format!("<bytes, base64 {}>", text()),
        _ => raw.to_string(),
    }
}
