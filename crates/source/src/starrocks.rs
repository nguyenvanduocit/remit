//! StarRocks is the OLAP warehouse (orders, revenue, fees, ad spend). It speaks the MySQL protocol. Credentials:
//! `STARROCKS_HOST` and `STARROCKS_USER`; `STARROCKS_PORT` (default 9030), `STARROCKS_PASSWORD` (an account may have none) and
//! `STARROCKS_DB` (the default database) are optional.
//!
//! What keeps it a reader is the guard (`read_only_sql`), run on every statement this file sends, the schema ones included. The
//! session is not put in read-only mode: StarRocks accepts `SET TRANSACTION READ ONLY` but a write still goes on to be analysed
//! (confirmed on staging), so it would only look like a rail.

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use mysql_async::prelude::Queryable;
use mysql_async::{Conn, Opts, OptsBuilder, Value};
use regex::Regex;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::api::CALL_TIMEOUT;
use crate::env::{Env, missing};
use crate::error::{BoxFuture, Error, Result, err, fail};
use crate::limit::Limit;
use crate::redact::{REMOVED, mask_emails, sensitive_key};
use crate::source::{Provider, Shared, Tool, Var, new_tool};
use crate::sqlguard::read_only_sql;
use crate::text::{cut_bytes, shorten, table};
use crate::when::duration_text;

// The whole answer of a statement is bounded three ways: the server returns at most SERVER_ROW_LIMIT rows and stops a statement
// after SERVER_QUERY_TIMEOUT seconds (a cancelled client does not stop a query by itself), and the agent sees at most MAX_ROWS
// rows of CELL_MAX bytes.
const DEFAULT_PORT: &str = "9030";
const ROWS: Limit = Limit::new(100, 500);
const CELL_MAX: usize = 300;
const SERVER_ROW_LIMIT: usize = 5000;
/// Seconds, less than the gate's deadline so the server gives up first and says why.
const SERVER_QUERY_TIMEOUT: u32 = 25;
const SCHEMA_TABLES: usize = 30;
const SCHEMA_COLUMNS: usize = 60;
const COMMENT_MAX: usize = 80;
const CONNECT_WAIT: Duration = Duration::from_secs(5);
/// How long this source waits for a statement: a second short of the gate's deadline, so the agent is told how to narrow the
/// statement rather than only that it ran out of time.
const STATEMENT_WAIT: Duration = Duration::from_secs(29);

/// The schemas that hold no business data: the schema tool leaves them out.
const SYSTEM_SCHEMAS: &str = "'information_schema','sys','_statistics_'";

/// Starts a statement in `database` ("" for the account's default) and reads its rows into a `ResultSet` that keeps `keep`
/// of them. A test serves rows without a server.
pub(crate) trait RowsProvider: Send + Sync {
    fn rows<'a>(&'a self, database: &'a str, stmt: &'a str, keep: usize) -> BoxFuture<'a, Result<ResultSet>>;
}

pub(crate) struct Starrocks {
    env: Env,
    rows: Arc<dyn RowsProvider>,
}

pub(crate) fn new(env: Env, _: Shared) -> Arc<dyn Provider> {
    let server = Server {
        host: env.get("STARROCKS_HOST"),
        port: env.get("STARROCKS_PORT"),
        user: env.get("STARROCKS_USER"),
        password: env.get("STARROCKS_PASSWORD"),
        db_name: env.get("STARROCKS_DB"),
    };
    Arc::new(Starrocks::on(env, Arc::new(server)))
}

impl Starrocks {
    /// The source on `rows`: the server in `new`, a fake in a test.
    pub(crate) fn on(env: Env, rows: Arc<dyn RowsProvider>) -> Self {
        Self { env, rows }
    }
}

impl Provider for Starrocks {
    fn name(&self) -> String {
        "StarRocks".to_owned()
    }

    fn missing(&self) -> Vec<String> {
        missing(&self.env, &["STARROCKS_HOST", "STARROCKS_USER"])
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            Var::plain("STARROCKS_HOST", "the host of the StarRocks frontend").address(),
            Var::plain("STARROCKS_PORT", "its MySQL port, when it is not the default").optional(),
            Var::plain("STARROCKS_USER", "the user that reads"),
            Var::plain("STARROCKS_PASSWORD", "that user's password, when it has one")
                .secret()
                .optional(),
            Var::plain("STARROCKS_DB", "the database the queries start in").optional(),
        ]
    }

    fn tools(self: Arc<Self>) -> Vec<Tool> {
        let q = Arc::clone(&self);
        let s = self;
        vec![
            new_tool(
                "starrocks_query",
                "Run one read-only SQL statement (SELECT, WITH, SHOW, DESCRIBE or EXPLAIN, MySQL dialect) on StarRocks, the warehouse of orders, revenue, transaction fees and ad spend. Answers a table of at most `limit` rows (default 100, at most 500) and says how many more there were; long cells are cut, a column named like a password or token comes back as [removed], customer email addresses are masked. Table and column names differ from MongoDB's (`x_shopify_shop_id`, not `shop_id`): read them with starrocks_schema first. `database` is the default database for tables written without one.",
                move |input: QueryInput| {
                    let q = Arc::clone(&q);
                    async move { q.query(input).await }
                },
            ),
            new_tool(
                "starrocks_schema",
                "Learn the names StarRocks uses. With no input it lists the databases and how many tables each holds. With `database` it lists that database's tables with their columns and types; with `name` (one word) it keeps the tables whose name or a column's name contains the word, from every database when `database` is empty. Use it before starrocks_query to find the table and column names, for example `name: x_shopify_shop_id`.",
                move |input: SchemaInput| {
                    let s = Arc::clone(&s);
                    async move { s.schema(input).await }
                },
            ),
        ]
    }
}

#[derive(Deserialize, JsonSchema)]
struct QueryInput {
    /// one statement that reads; no semicolon needed
    sql: String,
    /// default database for tables written without one; empty: the configured default
    #[serde(default)]
    database: String,
    /// most rows to show, 1 to 500; default 100
    #[serde(default)]
    limit: i64,
}

#[derive(Deserialize, JsonSchema)]
struct SchemaInput {
    /// a database name; empty: all databases
    #[serde(default)]
    database: String,
    /// one word that a table name or a column name contains, such as shop_id or ad_spend
    #[serde(default)]
    name: String,
}

impl Starrocks {
    async fn query(&self, input: QueryInput) -> Result<String> {
        let limit = ROWS.of("limit", input.limit)?;
        if !input.database.is_empty() {
            identifier("database", &input.database)?;
        }
        let res = self.read(&input.database, &input.sql, limit).await?;
        Ok(mask_emails(&res.render()))
    }

    /// The one way a statement leaves this source: checked as a reader, run, and cut to `keep` rows.
    async fn read(&self, database: &str, stmt: &str, keep: usize) -> Result<ResultSet> {
        read_only_sql(stmt)?;
        match tokio::time::timeout(STATEMENT_WAIT, self.rows.rows(database, stmt, keep)).await {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(e)) => Err(err!("starrocks: {e}")),
            Err(_) => fail!(
                "starrocks: the statement did not finish in {}: narrow it (a shorter period, a filter on x_shopify_shop_id, fewer columns)",
                duration_text(CALL_TIMEOUT)
            ),
        }
    }

    async fn schema(&self, input: SchemaInput) -> Result<String> {
        let stmt = schema_statement(&input.database, &input.name)?;
        if input.database.is_empty() && input.name.is_empty() {
            return Ok(self.read("", &stmt, ROWS.max).await?.render());
        }
        let res = self.read("", &stmt, SERVER_ROW_LIMIT).await?;
        Ok(layout_schema(&res.rows, &input.name.to_lowercase()))
    }
}

// --- the connection ---

/// The StarRocks server of a connection. It opens a connection for each statement and closes it after: a default database
/// never leaks into another call, and nothing stays open between calls.
struct Server {
    host: String,
    port: String,
    user: String,
    password: String,
    db_name: String,
}

impl RowsProvider for Server {
    fn rows<'a>(&'a self, database: &'a str, stmt: &'a str, keep: usize) -> BoxFuture<'a, Result<ResultSet>> {
        Box::pin(async move {
            let db = if database.is_empty() {
                self.db_name.as_str()
            } else {
                database
            };
            let opts = starrocks_opts(&self.host, &self.port, &self.user, &self.password, db)?;
            let mut conn = match tokio::time::timeout(CONNECT_WAIT, Conn::new(opts)).await {
                Ok(c) => c.map_err(|e| Error::msg(e.to_string()))?,
                Err(_) => {
                    return fail!(
                        "no answer from {}:{} within {}",
                        self.host,
                        self.port,
                        duration_text(CONNECT_WAIT)
                    );
                }
            };
            let res = read_all(&mut conn, stmt, keep).await;
            // The rows are read: closing the connection cannot lose data, and a failed close is the server's to clean up.
            conn.disconnect().await.unwrap_or_default();
            res
        })
    }
}

async fn read_all(conn: &mut Conn, stmt: &str, keep: usize) -> Result<ResultSet> {
    let mut q = conn.query_iter(stmt).await.map_err(|e| Error::msg(e.to_string()))?;
    let cols = q.columns_ref().iter().map(|c| c.name_str().into_owned()).collect();
    let mut set = ResultSet::reading(cols, keep);
    while let Some(row) = q.next().await.map_err(|e| Error::msg(e.to_string()))? {
        set.add(row.unwrap_raw().into_iter().map(|v| v.and_then(raw_bytes)).collect());
    }
    q.drop_result().await.map_err(|e| Error::msg(e.to_string()))?;
    Ok(set)
}

/// The bytes of a value as the server sent them. The text protocol hands only bytes and NULL; the other kinds are written as
/// SQL would write them.
fn raw_bytes(v: Value) -> Option<Vec<u8>> {
    match v {
        Value::NULL => None,
        Value::Bytes(b) => Some(b),
        other => Some(other.as_sql(true).into_bytes()),
    }
}

/// The options of a connection to a server. A password with `@` or `/` in it needs no escaping, values come back as text (the
/// text protocol), and no local file is ever sent (no infile handler is set). The session is bounded by the server's own
/// statement timeout and row limit.
pub(crate) fn starrocks_opts(host: &str, port: &str, user: &str, password: &str, database: &str) -> Result<Opts> {
    let port = if port.is_empty() { DEFAULT_PORT } else { port };
    let Some(n) = port.parse::<u16>().ok().filter(|n| *n >= 1) else {
        return fail!("STARROCKS_PORT {port:?} is not a port");
    };
    let builder = OptsBuilder::default()
        .ip_or_hostname(host)
        .tcp_port(n)
        .user(Some(user))
        .pass((!password.is_empty()).then_some(password))
        .db_name((!database.is_empty()).then_some(database))
        .prefer_socket(false)
        .init(vec![
            format!("SET query_timeout = {SERVER_QUERY_TIMEOUT}"),
            format!("SET sql_select_limit = {SERVER_ROW_LIMIT}"),
        ]);
    Ok(builder.into())
}

static IDENTIFIER: LazyLock<Regex> = LazyLock::new(|| {
    #[expect(clippy::expect_used, reason = "a constant pattern that is tested")]
    Regex::new(r"^[A-Za-z0-9_$-]{1,64}$").expect("identifier pattern")
});

/// Ok when `s` can be a name; the schema tool puts it inside a statement, so nothing else gets through.
fn identifier(label: &str, s: &str) -> Result<()> {
    if IDENTIFIER.is_match(s) {
        return Ok(());
    }
    fail!("{label} {s:?} is not a name: use letters, digits, _ , $ and - (at most 64)")
}

// --- the rows ---

/// The rows of a statement as the agent reads them: the first `keep`, and a count of the rest. A cell is cut at `CELL_MAX`
/// bytes, and the value of a column whose name says it holds a credential is replaced; a column is judged by its name, so an
/// alias is judged by the alias.
pub(crate) struct ResultSet {
    cols: Vec<String>,
    secret: Vec<bool>,
    rows: Vec<Vec<String>>,
    /// rows read but not kept
    unseen: usize,
    keep: usize,
}

impl ResultSet {
    pub(crate) fn reading(cols: Vec<String>, keep: usize) -> Self {
        let secret = cols.iter().map(|c| sensitive_key(c)).collect();
        Self {
            cols,
            secret,
            rows: Vec::new(),
            unseen: 0,
            keep,
        }
    }

    /// Takes one row of raw values (`None` is NULL).
    pub(crate) fn add(&mut self, row: Vec<Option<Vec<u8>>>) {
        if self.rows.len() >= self.keep {
            self.unseen = self.unseen.saturating_add(1);
            return;
        }
        let cells = row
            .into_iter()
            .enumerate()
            .map(|(i, v)| cell(v.as_deref(), self.secret.get(i).copied().unwrap_or(false)))
            .collect();
        self.rows.push(cells);
    }

    fn render(&self) -> String {
        let mut out = table(&self.cols, &self.rows, self.rows.len());
        if self.unseen > 0 {
            out.push_str(&format!(
                "\n[{} more rows not shown; add a LIMIT, a filter or fewer columns]",
                self.unseen
            ));
        }
        if self.rows.len().saturating_add(self.unseen) >= SERVER_ROW_LIMIT {
            out.push_str(&format!(
                "\n[the server returns at most {SERVER_ROW_LIMIT} rows to a statement, so there may be more]"
            ));
        }
        out
    }
}

/// One value as the agent reads it. NULL stays visible, an empty value stays empty (that a token is unset is something an
/// investigation needs to see), a binary value is named by its size.
fn cell(v: Option<&[u8]>, secret: bool) -> String {
    let Some(v) = v else {
        return "NULL".to_owned();
    };
    if v.is_empty() {
        return String::new();
    }
    if secret {
        return REMOVED.to_owned();
    }
    let Ok(s) = std::str::from_utf8(v) else {
        return format!("[binary, {} bytes]", v.len());
    };
    if s.len() > CELL_MAX {
        return format!("{}…[{} bytes]", cut_bytes(s, CELL_MAX), s.len());
    }
    s.to_owned()
}

// --- the schema ---

/// The statement that answers a schema question. Its only inputs are names that passed `identifier`, so it can be written as
/// text; the word is compared in lower case with instr, which has no wildcard to escape.
fn schema_statement(database: &str, word: &str) -> Result<String> {
    if !database.is_empty() {
        identifier("database", database)?;
    }
    if !word.is_empty() {
        identifier("name", word)?;
    }
    if database.is_empty() && word.is_empty() {
        return Ok(format!(
            "SELECT table_schema AS `database`, COUNT(*) AS tables FROM information_schema.tables WHERE table_schema NOT IN ({SYSTEM_SCHEMAS}) GROUP BY table_schema ORDER BY table_schema"
        ));
    }
    let mut filter = format!("table_schema NOT IN ({SYSTEM_SCHEMAS})");
    if !database.is_empty() {
        filter.push_str(&format!(" AND table_schema = '{database}'"));
    }
    if !word.is_empty() {
        let w = word.to_lowercase();
        filter.push_str(&format!(
            " AND (instr(lower(table_name), '{w}') > 0 OR instr(lower(column_name), '{w}') > 0)"
        ));
    }
    Ok(format!(
        "SELECT table_schema, table_name, column_name, column_type, column_comment FROM information_schema.columns WHERE {filter} ORDER BY table_schema, table_name, ordinal_position"
    ))
}

/// The rows of `schema_statement` (database, table, column, type, comment) as one block per table. A table whose own name holds
/// the word shows all its columns; one that matched through a column shows the columns that matched, and says so.
fn layout_schema(rows: &[Vec<String>], word: &str) -> String {
    let mut tables: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut index: BTreeMap<(String, String), usize> = BTreeMap::new();
    for r in rows {
        let [db, name, column, typ, comment, ..] = r.as_slice() else {
            continue;
        };
        let key = (db.clone(), name.clone());
        let i = *index.entry(key).or_insert_with(|| {
            tables.push((db.clone(), name.clone(), Vec::new()));
            tables.len().saturating_sub(1)
        });
        let mut line = format!("  {column}  {typ}");
        if !comment.is_empty() && comment != "NULL" {
            line.push_str(&format!("  -- {}", shorten(comment, COMMENT_MAX)));
        }
        if let Some(t) = tables.get_mut(i) {
            t.2.push(line);
        }
    }
    if tables.is_empty() {
        return "[no table or column matches]".to_owned();
    }
    let mut b = String::new();
    for (i, (db, name, cols)) in tables.iter().enumerate() {
        if i == SCHEMA_TABLES {
            b.push_str(&format!(
                "[{} more tables not shown; name a database or a longer word]\n",
                tables.len().saturating_sub(SCHEMA_TABLES)
            ));
            break;
        }
        b.push_str(&format!("{db}.{name}"));
        if !word.is_empty() && !name.to_lowercase().contains(word) {
            b.push_str(" (columns that match only)");
        }
        b.push('\n');
        let mut shown: Vec<String> = cols.iter().take(SCHEMA_COLUMNS).cloned().collect();
        if cols.len() > SCHEMA_COLUMNS {
            shown.push(format!(
                "  [{} more columns not shown]",
                cols.len().saturating_sub(SCHEMA_COLUMNS)
            ));
        }
        b.push_str(&shown.join("\n"));
        b.push('\n');
    }
    if rows.len() >= SERVER_ROW_LIMIT {
        b.push_str(&format!(
            "[the server returns at most {SERVER_ROW_LIMIT} columns to a statement: name a database or a longer word]\n"
        ));
    }
    b.trim_end_matches('\n').to_owned()
}
