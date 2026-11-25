//! Bytebase runs a statement in whatever language the database speaks, and a language is only safe to forward once it has
//! been read by something that agrees with the server on where a string, a comment and a statement end. SQL engines go
//! through `read_only_sql` (the MySQL dialect) and, where their dialect reads a character differently, through a further
//! check here. A MongoDB statement is parsed completely, checked, and sent as the text the parser wrote, so what was checked
//! is what runs (`mongoshell.rs`).

use super::mongoshell::{is_name_char, plan_mongo};
use crate::error::{Result, fail};
use crate::sqlguard::read_only_sql;

/// A statement once it has been checked: the text to send and what the answer needs to be read correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatementPlan {
    pub text: String,
    /// Marks a count rewritten as an aggregation, which answers no document when the count is zero.
    pub count: bool,
}

/// The most rows a statement asks Bytebase for; what an agent is shown is less (`bytebase.rs`).
pub(crate) const BYTEBASE_FETCH_ROWS: usize = 500;

/// How a statement for one engine is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    Mongo,
    Postgres,
    BigQuery,
    MySql,
}

/// An engine a statement may be sent to: how an agent writes for it and how its statement is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EngineSpec {
    pub language: &'static str,
    check: Check,
}

const MYSQL_DIALECT: &str = "MySQL-dialect SQL: SELECT, WITH, SHOW, DESCRIBE or EXPLAIN";

/// The engines a statement may be sent to, by the name Bytebase gives them, sorted.
pub(crate) const ENGINE_NAMES: &[&str] = &[
    "BIGQUERY",
    "DORIS",
    "MARIADB",
    "MONGODB",
    "MYSQL",
    "POSTGRES",
    "STARROCKS",
    "TIDB",
];

/// The engine Bytebase names `name`, none when this source does not query it.
pub(crate) const fn engine(name: &str) -> Option<EngineSpec> {
    let (language, check) = match name.as_bytes() {
        b"MONGODB" => (
            "mongosh, one call: db.<collection>.find({...}, {projection}).sort({...}).skip(n).limit(n), findOne, aggregate([...]), countDocuments, distinct(\"field\"), estimatedDocumentCount()",
            Check::Mongo,
        ),
        b"POSTGRES" => (
            "PostgreSQL SQL: SELECT, WITH, SHOW or EXPLAIN, without backslashes, $ quotes, # or backticks",
            Check::Postgres,
        ),
        b"BIGQUERY" => ("GoogleSQL: SELECT or WITH; name a table dataset.table", Check::BigQuery),
        b"STARROCKS" | b"DORIS" | b"TIDB" => (MYSQL_DIALECT, Check::MySql),
        b"MYSQL" => ("MySQL SQL: SELECT, WITH, SHOW, DESCRIBE or EXPLAIN", Check::MySql),
        b"MARIADB" => ("MariaDB SQL: SELECT, WITH, SHOW, DESCRIBE or EXPLAIN", Check::MySql),
        _ => return None,
    };
    Some(EngineSpec { language, check })
}

impl EngineSpec {
    /// The statement to send for `stmt`, or why it is refused.
    pub(crate) fn plan(self, stmt: &str) -> Result<StatementPlan> {
        match self.check {
            Check::Mongo => return plan_mongo(stmt),
            Check::Postgres => read_only_postgres(stmt)?,
            Check::BigQuery => read_only_bigquery(stmt)?,
            Check::MySql => read_only_sql(stmt)?,
        }
        Ok(StatementPlan {
            text: stmt.to_owned(),
            count: false,
        })
    }
}

// ---- SQL ----

/// `read_only_sql` and what GoogleSQL reads differently: a triple-quoted string, which the MySQL reading of quotes would split
/// in the wrong place.
pub(crate) fn read_only_bigquery(stmt: &str) -> Result<()> {
    if stmt.contains(r#"""""#) || stmt.contains("'''") {
        return fail!("a triple-quoted string is not accepted: use a single-quoted one");
    }
    read_only_sql(stmt)
}

/// The words a PostgreSQL statement may start with (`read_only_sql` also lets DESCRIBE through, which PostgreSQL lacks).
const PG_STARTS: &[&str] = &["select", "with", "show", "explain"];

/// The functions and words that change state, wait or reach outside the database though they sit in a SELECT.
const PG_DENIED: &[&str] = &[
    "into",
    "copy",
    "nextval",
    "setval",
    "lastval",
    "set_config",
    "pg_sleep",
    "pg_notify",
    "vacuum",
    "refresh",
    "listen",
    "notify",
];

/// The starts of the names of such functions.
const PG_DENIED_PREFIXES: &[&str] = &[
    "lo_",
    "dblink",
    "pg_advisory",
    "pg_try_advisory",
    "pg_read_",
    "pg_ls_",
    "pg_stat_reset",
    "pg_logical_",
    "pg_replication_",
    "pg_create_",
    "pg_drop_",
    "pg_terminate",
    "pg_cancel",
    "pg_reload",
    "pg_rotate",
    "pg_switch",
    "pg_promote",
    "query_to_xml",
    "database_to_xml",
    "schema_to_xml",
    "table_to_xml",
    "cursor_to_xml",
];

const NO_BACKSLASH: &str = "a backslash is not accepted in a PostgreSQL statement: write the character with chr()";

/// Ok when `stmt` reads under `read_only_sql` and under PostgreSQL's own reading of it. `read_only_sql` reads `#` as a
/// comment, a backslash as an escape, a backtick as a quote and sees no dollar-quoted string, where PostgreSQL has an
/// operator, a plain character, an operator again and a string: a statement can hide its second half from one reader and not
/// from the other. Those four characters are refused outside the places both readers agree on; the words of a quoted name
/// are read too, because `"nextval"('s')` calls the function.
pub(crate) fn read_only_postgres(stmt: &str) -> Result<()> {
    read_only_sql(stmt)?;
    let words = postgres_words(stmt)?;
    if !words.first().is_some_and(|w| PG_STARTS.contains(&w.as_str())) {
        return fail!("a PostgreSQL statement may start with SELECT, WITH, SHOW or EXPLAIN");
    }
    for w in &words {
        if PG_DENIED.contains(&w.as_str()) {
            return fail!("the word {:?} is not allowed: this tool only reads", w.to_uppercase());
        }
        if PG_DENIED_PREFIXES.iter().any(|p| w.starts_with(p)) {
            return fail!("{w:?} is not allowed: this tool only reads");
        }
    }
    Ok(())
}

/// The lower-cased words of `stmt` outside comments and single-quoted strings, those inside a quoted name included.
fn postgres_words(stmt: &str) -> Result<Vec<String>> {
    let rs: Vec<char> = stmt.chars().collect();
    let at = |i: usize| rs.get(i).copied();
    let mut words = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, words: &mut Vec<String>| {
        if !cur.is_empty() {
            words.push(cur.to_lowercase());
            cur.clear();
        }
    };
    let mut i = 0usize;
    while let Some(r) = at(i) {
        let next = at(i.saturating_add(1));
        match r {
            '\\' => return fail!("{NO_BACKSLASH}"),
            '$' | '#' | '`' => {
                return fail!(
                    "{:?} is not accepted in a PostgreSQL statement outside a quoted string",
                    r.to_string()
                );
            }
            '\'' => {
                flush(&mut cur, &mut words);
                i = pg_skip_string(&rs, i)?;
            }
            '"' => {
                flush(&mut cur, &mut words);
                let end = pg_skip_string(&rs, i)?;
                let name: String = rs.get(i.saturating_add(1)..end).unwrap_or_default().iter().collect();
                words.extend(pg_name_words(&name));
                i = end;
            }
            '-' if next == Some('-') => {
                flush(&mut cur, &mut words);
                while at(i).is_some_and(|c| c != '\n') {
                    i = i.saturating_add(1);
                }
            }
            '/' if next == Some('*') => {
                flush(&mut cur, &mut words);
                i = pg_skip_comment(&rs, i)?;
            }
            r if is_name_char(r) => cur.push(r),
            _ => flush(&mut cur, &mut words),
        }
        i = i.saturating_add(1);
    }
    flush(&mut cur, &mut words);
    Ok(words)
}

/// The words of a quoted name.
fn pg_name_words(name: &str) -> Vec<String> {
    name.to_lowercase()
        .split(|c: char| !is_name_char(c))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The index of the quote that closes the one at `rs[open]`; PostgreSQL doubles a quote to carry it.
fn pg_skip_string(rs: &[char], open: usize) -> Result<usize> {
    let q = rs.get(open).copied();
    let mut i = open.saturating_add(1);
    while let Some(c) = rs.get(i).copied() {
        if c == '\\' {
            return fail!("{NO_BACKSLASH}");
        }
        if Some(c) == q {
            if rs.get(i.saturating_add(1)).copied() == q {
                i = i.saturating_add(1);
            } else {
                return Ok(i);
            }
        }
        i = i.saturating_add(1);
    }
    fail!("a quote is not closed")
}

/// The index of the last character of the comment opened at `rs[open]`; PostgreSQL nests /* */.
fn pg_skip_comment(rs: &[char], open: usize) -> Result<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while let (Some(a), Some(b)) = (rs.get(i).copied(), rs.get(i.saturating_add(1)).copied()) {
        if a == '/' && b == '*' {
            depth = depth.saturating_add(1);
            i = i.saturating_add(1);
        } else if a == '*' && b == '/' {
            depth = depth.saturating_sub(1);
            i = i.saturating_add(1);
            if depth == 0 {
                return Ok(i);
            }
        }
        i = i.saturating_add(1);
    }
    fail!("a comment is not closed")
}
