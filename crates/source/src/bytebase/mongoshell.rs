//! A MongoDB statement is parsed completely, checked, and sent as the text this parser wrote, so what was checked is what
//! runs.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::guard::{BYTEBASE_FETCH_ROWS, StatementPlan};
use crate::error::{Error, Result, err, fail};
use crate::mongoguard::{check_collection_name, check_mongo_filter, check_mongo_pipeline, check_ordered_field};

/// One value of a statement as it was written. A document keeps the order its keys were written in, which a sort and a
/// pipeline depend on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Shell {
    Object(Vec<(String, Self)>),
    Array(Vec<Self>),
    Str(String),
    /// A number as it was written.
    Number(String),
    Bool(bool),
    Null,
    /// A value constructor such as `ObjectId("...")` or `ISODate("...")`; its argument is a string or a number.
    Ctor(String, Box<Self>),
    Regex {
        body: String,
        flags: String,
    },
}

/// The constructors a filter may use to write a typed value.
const MONGO_CTORS: &[&str] = &["ObjectId", "ISODate", "NumberLong", "NumberInt", "NumberDecimal"];

const MONGO_MAX_DEPTH: usize = 24;
const MONGO_MAX_STATEMENT: usize = 20000;

#[expect(clippy::expect_used, reason = "a constant pattern: a test runs it")]
static MONGO_NUMBER_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?").expect("number pattern"));
#[expect(clippy::expect_used, reason = "a constant pattern: a test runs it")]
static MONGO_REGEX_FLAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[gimsuy]*").expect("flags pattern"));

/// Whether `c` may be part of a name in a statement: a letter, a digit, `_`, or any character past ASCII.
pub(crate) const fn is_name_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric() || !c.is_ascii()
}

/// Reads the JavaScript subset of a mongosh statement: calls, documents, arrays, strings, numbers, regular expressions and
/// the typed-value constructors. Everything else (a function, an arrow, an operator, a variable) is a syntax error, so there
/// is no way to write code in it.
struct Parser {
    rs: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn fail(&self, what: &str) -> Error {
        err!("mongosh statement, character {}: {what}", self.pos.saturating_add(1))
    }

    fn at(&self, i: usize) -> Option<char> {
        self.rs.get(i).copied()
    }

    fn advance(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n);
    }

    fn rest(&self) -> String {
        self.rs.get(self.pos..).unwrap_or_default().iter().collect()
    }

    fn space(&mut self) {
        while self.at(self.pos).is_some_and(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
            self.advance(1);
        }
    }

    /// The next character after spaces, '\0' at the end.
    fn peek(&mut self) -> char {
        self.space();
        self.at(self.pos).unwrap_or('\0')
    }

    fn expect(&mut self, c: char) -> Result<()> {
        if self.peek() != c {
            return Err(self.fail(&format!("expected {:?}", c.to_string())));
        }
        self.advance(1);
        Ok(())
    }

    fn ident(&mut self) -> Result<String> {
        self.space();
        let start = self.pos;
        while self
            .at(self.pos)
            .is_some_and(|c| c.is_ascii() && (is_name_char(c) || c == '$'))
        {
            self.advance(1);
        }
        let first = self.at(start);
        if self.pos == start || first.is_some_and(|c| c.is_ascii_digit()) {
            return Err(self.fail("expected a name"));
        }
        Ok(self.rs.get(start..self.pos).unwrap_or_default().iter().collect())
    }

    fn value(&mut self) -> Result<Shell> {
        self.depth = self.depth.saturating_add(1);
        let v = if self.depth > MONGO_MAX_DEPTH {
            Err(self.fail(&format!("nested deeper than {MONGO_MAX_DEPTH} levels")))
        } else {
            match self.peek() {
                '{' => self.object(),
                '[' => self.array(),
                '\'' | '"' => self.str().map(Shell::Str),
                '/' => self.regex(),
                c if c == '-' || c.is_ascii_digit() => self.number(),
                '\0' => Err(self.fail("the statement ends where a value is expected")),
                _ => self.word(),
            }
        };
        self.depth = self.depth.saturating_sub(1);
        v
    }

    /// true, false, null, or a constructor call; any other name is code and is refused.
    fn word(&mut self) -> Result<Shell> {
        let name = self.ident()?;
        match name.as_str() {
            "true" => return Ok(Shell::Bool(true)),
            "false" => return Ok(Shell::Bool(false)),
            "null" => return Ok(Shell::Null),
            "new" => {
                return Err(self.fail("`new` is not accepted: write ISODate(\"2026-10-30T00:00:00Z\") for a date"));
            }
            _ => {}
        }
        if self.peek() != '(' {
            return Err(self.fail(&format!(
                "{name:?} is not a value: write a document, array, string, number, true, false or null"
            )));
        }
        if !MONGO_CTORS.contains(&name.as_str()) {
            return Err(self.fail(&format!("the call {name}(...) is not accepted: a statement is data, and only ObjectId, ISODate, NumberLong, NumberInt and NumberDecimal build a value")));
        }
        self.advance(1);
        let arg = self.value()?;
        if !matches!(arg, Shell::Str(_) | Shell::Number(_)) {
            return Err(self.fail(&format!("{name} takes one string or number")));
        }
        self.expect(')')?;
        Ok(Shell::Ctor(name, Box::new(arg)))
    }

    fn number(&mut self) -> Result<Shell> {
        let rest = self.rest();
        let Some(m) = MONGO_NUMBER_TEXT.find(&rest) else {
            return Err(self.fail("not a number"));
        };
        let text = m.as_str().to_owned();
        self.advance(text.chars().count());
        Ok(Shell::Number(text))
    }

    fn regex(&mut self) -> Result<Shell> {
        self.advance(1); // the opening slash
        let start = self.pos;
        while let Some(c) = self.at(self.pos) {
            match c {
                '\\' => self.advance(1),
                '\n' | '\r' => return Err(self.fail("a regular expression ends on its line")),
                '/' => {
                    let body: String = self.rs.get(start..self.pos).unwrap_or_default().iter().collect();
                    self.advance(1);
                    let rest = self.rest();
                    let flags = MONGO_REGEX_FLAGS
                        .find(&rest)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default();
                    self.advance(flags.chars().count());
                    if body.is_empty() || body.contains(['"', '\'', '`']) {
                        return Err(self.fail(
                            "a regular expression is not empty and holds no quote: write a quote as \\x22 or \\x27",
                        ));
                    }
                    return Ok(Shell::Regex { body, flags });
                }
                _ => {}
            }
            self.advance(1);
        }
        Err(self.fail("a regular expression is not closed"))
    }

    fn str(&mut self) -> Result<String> {
        let q = self.at(self.pos).unwrap_or('\0');
        self.advance(1);
        let mut b = String::new();
        while let Some(c) = self.at(self.pos) {
            self.advance(1);
            match c {
                c if c == q => return Ok(b),
                '\n' | '\r' => return Err(self.fail("a string ends on its line")),
                '\\' => b.push(self.escape()?),
                c => b.push(c),
            }
        }
        Err(self.fail("a string is not closed"))
    }

    fn escape(&mut self) -> Result<char> {
        let Some(c) = self.at(self.pos) else {
            return Err(self.fail("a string is not closed"));
        };
        self.advance(1);
        match c {
            'n' => Ok('\n'),
            't' => Ok('\t'),
            'r' => Ok('\r'),
            'b' => Ok('\u{8}'),
            'f' => Ok('\u{c}'),
            '\\' | '\'' | '"' | '/' => Ok(c),
            'u' => {
                let r = self.hex4()?;
                if (0xD800..0xE000).contains(&r)
                    && self.at(self.pos) == Some('\\')
                    && self.at(self.pos.saturating_add(1)) == Some('u')
                {
                    self.advance(2);
                    let low = self.hex4()?;
                    return Ok(decode_surrogates(r, low));
                }
                Ok(char::from_u32(r).unwrap_or('\u{FFFD}'))
            }
            _ => Err(self.fail(&format!("the escape \\{c} is not accepted"))),
        }
    }

    fn hex4(&mut self) -> Result<u32> {
        let digits: String = self
            .rs
            .get(self.pos..self.pos.saturating_add(4))
            .unwrap_or_default()
            .iter()
            .collect();
        let v = (digits.len() == 4 && digits.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(&digits, 16).ok())
            .flatten();
        let Some(v) = v else {
            return Err(self.fail("a \\u escape needs 4 hexadecimal digits"));
        };
        self.advance(4);
        Ok(v)
    }

    fn object(&mut self) -> Result<Shell> {
        self.advance(1); // {
        let mut obj: Vec<(String, Shell)> = Vec::new();
        while self.peek() != '}' {
            if !obj.is_empty() {
                self.expect(',')?;
            }
            let key = self.key()?;
            if obj.iter().any(|(k, _)| *k == key) {
                return Err(self.fail(&format!("the key {key:?} is written twice")));
            }
            self.expect(':')?;
            let val = self.value()?;
            obj.push((key, val));
        }
        self.advance(1); // }
        Ok(Shell::Object(obj))
    }

    fn key(&mut self) -> Result<String> {
        match self.peek() {
            '\'' | '"' => self.str(),
            _ => self.ident(),
        }
    }

    fn array(&mut self) -> Result<Shell> {
        self.advance(1); // [
        let mut arr = Vec::new();
        while self.peek() != ']' {
            if !arr.is_empty() {
                self.expect(',')?;
            }
            arr.push(self.value()?);
        }
        self.advance(1); // ]
        Ok(Shell::Array(arr))
    }

    /// Reads `( value, value )`.
    fn args(&mut self) -> Result<Vec<Shell>> {
        self.expect('(')?;
        let mut out = Vec::new();
        while self.peek() != ')' {
            if !out.is_empty() {
                self.expect(',')?;
            }
            out.push(self.value()?);
        }
        self.advance(1);
        Ok(out)
    }

    /// What follows `db`: the collection, as db.name, db["name"] or db.getCollection("name"), and the first call on it. A
    /// name with dots (db.a.b.find) is the collection "a.b", as mongosh reads it.
    fn target(&mut self) -> Result<(String, Call)> {
        let mut segments: Vec<String> = Vec::new();
        if self.peek() == '[' {
            self.advance(1);
            if !matches!(self.peek(), '\'' | '"') {
                return Err(self.fail("expected a string"));
            }
            segments = vec![self.str()?];
            self.expect(']')?;
        }
        loop {
            self.expect('.')?;
            let name = self.ident()?;
            if self.peek() != '(' {
                segments.push(name);
                continue;
            }
            let args = self.args()?;
            if name != "getCollection" || !segments.is_empty() {
                if segments.is_empty() {
                    return fail!(
                        "name a collection: db.<collection>.find(...); to list collections use bytebase_schema"
                    );
                }
                return Ok((segments.join("."), Call { name, args }));
            }
            match args.as_slice() {
                [Shell::Str(coll)] => segments = vec![coll.clone()],
                _ => return fail!("getCollection takes the collection's name as a string"),
            }
        }
    }
}

/// The character of a UTF-16 surrogate pair, U+FFFD when the two are not one.
fn decode_surrogates(high: u32, low: u32) -> char {
    if (0xD800..0xDC00).contains(&high) && (0xDC00..0xE000).contains(&low) {
        let c = high
            .saturating_sub(0xD800)
            .checked_shl(10)
            .map(|h| h | low.saturating_sub(0xDC00))
            .map(|v| v.saturating_add(0x10000));
        return c.and_then(char::from_u32).unwrap_or('\u{FFFD}');
    }
    '\u{FFFD}'
}

#[derive(Debug, Clone)]
struct Call {
    name: String,
    args: Vec<Shell>,
}

/// `db.<collection>.<method>(args)` and the calls chained after it.
struct Statement {
    collection: String,
    method: Call,
    chain: Vec<Call>,
}

/// `stmt` read as one mongosh call on a collection, with an optional chain and a closing semicolon.
fn parse_mongo(stmt: &str) -> Result<Statement> {
    if stmt.len() > MONGO_MAX_STATEMENT {
        return fail!("the statement is longer than {MONGO_MAX_STATEMENT} characters");
    }
    let mut p = Parser {
        rs: stmt.chars().collect(),
        pos: 0,
        depth: 0,
    };
    if p.ident().ok().as_deref() != Some("db") {
        return fail!("a mongosh statement starts with db.<collection>, for example db.orders.find({{...}}).limit(10)");
    }
    let (collection, method) = p.target()?;
    let mut chain = Vec::new();
    while p.peek() == '.' {
        p.advance(1);
        let name = p.ident()?;
        let args = p.args()?;
        chain.push(Call { name, args });
    }
    if p.peek() == ';' {
        p.advance(1);
    }
    if p.peek() != '\0' || p.pos < p.rs.len() {
        return Err(p.fail("nothing may follow the statement: one call at a time"));
    }
    Ok(Statement {
        collection,
        method,
        chain,
    })
}

/// The argument `i`, none when it is absent or written as null.
fn present(args: &[Shell], i: usize) -> Option<&Shell> {
    args.get(i).filter(|a| **a != Shell::Null)
}

/// The statement to send for `stmt`, or why it is refused. It reads only: find, findOne, aggregate, countDocuments, distinct
/// and estimatedDocumentCount, with the cursor calls limit, sort, skip, project and toArray; what it reads passes the rules
/// every Mongo read passes (mongoguard.rs). The text returned is written from
/// what was parsed, with the row cap in it: a `.limit(0)` or a pipeline without one cannot ask for the whole collection.
pub(crate) fn plan_mongo(stmt: &str) -> Result<StatementPlan> {
    let st = parse_mongo(stmt)?;
    check_collection_name(&st.collection)?;
    let mut b = format!("db.getCollection({}).", mongo_json(&st.collection));
    let mut count = false;
    match st.method.name.as_str() {
        "find" | "findOne" => plan_find(&mut b, &st)?,
        "aggregate" => plan_aggregate(&mut b, &st)?,
        "countDocuments" => {
            count = true;
            plan_count(&mut b, &st)?;
        }
        "distinct" => plan_distinct(&mut b, &st)?,
        "estimatedDocumentCount" => {
            if !st.method.args.is_empty() || !st.chain.is_empty() {
                return fail!("{} takes no arguments and no chained call", st.method.name);
            }
            b.push_str("estimatedDocumentCount()");
        }
        other => {
            return fail!(
                "{other} is not a call this tool makes: it reads with find, findOne, aggregate, countDocuments, distinct and estimatedDocumentCount"
            );
        }
    }
    Ok(StatementPlan { text: b, count })
}

/// `arg` as a document that passes the filter check, or why not; an absent arg is the empty document.
fn mongo_document(arg: Option<&Shell>, what: &str) -> Result<Shell> {
    match arg {
        None => Ok(Shell::Object(Vec::new())),
        Some(obj @ Shell::Object(_)) => {
            check_mongo_filter(&to_generic(obj))?;
            Ok(obj.clone())
        }
        Some(_) => fail!("{what} must be a document such as {{\"field\": value}}"),
    }
}

fn plan_find(b: &mut String, st: &Statement) -> Result<()> {
    let m = &st.method;
    if m.args.len() > 2 {
        return fail!("{} takes a filter and a projection", m.name);
    }
    let filter = mongo_document(present(&m.args, 0), "the filter")?;
    let mut projection = present(&m.args, 1);
    let (mut sort_by, mut limit, mut skip) = (None, None, None);
    let mut seen: Vec<&str> = Vec::new();
    for c in &st.chain {
        if seen.contains(&c.name.as_str()) {
            return fail!(".{} is chained twice", c.name);
        }
        seen.push(&c.name);
        if m.name == "findOne" && c.name != "toArray" {
            return fail!("findOne takes no chained call except toArray");
        }
        match c.name.as_str() {
            "toArray" => {
                if !c.args.is_empty() {
                    return fail!("toArray takes no arguments");
                }
            }
            "project" => {
                if m.args.len() == 2 || c.args.len() != 1 {
                    return fail!("give the projection once, as find's second argument or .project(...)");
                }
                projection = present(&c.args, 0);
            }
            "sort" => {
                sort_by = present(&c.args, 0);
                if c.args.len() != 1 {
                    return fail!("sort takes one document such as {{\"created_at\": -1}}");
                }
            }
            "limit" => {
                limit = present(&c.args, 0);
                if c.args.len() != 1 {
                    return fail!("limit takes one number");
                }
            }
            "skip" => {
                skip = present(&c.args, 0);
                if c.args.len() != 1 {
                    return fail!("skip takes one number");
                }
            }
            other => {
                return fail!(
                    ".{other} is not accepted after {}: use limit, sort, skip, project or toArray",
                    m.name
                );
            }
        }
    }
    let proj = mongo_document(projection, "the projection")?;
    b.push_str(&m.name);
    b.push('(');
    render_mongo(b, &filter);
    if projection.is_some() {
        b.push_str(", ");
        render_mongo(b, &proj);
    }
    b.push(')');
    if m.name == "findOne" {
        return Ok(());
    }
    write_cursor(b, sort_by, skip, limit)
}

/// Adds .sort, .skip and .limit; the limit is never more than the rows this source fetches.
fn write_cursor(b: &mut String, sort_by: Option<&Shell>, skip: Option<&Shell>, limit: Option<&Shell>) -> Result<()> {
    if let Some(s) = sort_by {
        let Shell::Object(keys) = s else {
            return fail!("sort takes a document such as {{\"created_at\": -1}}");
        };
        for (k, v) in keys {
            if !matches!(v, Shell::Number(n) if n == "1" || n == "-1") {
                return fail!("sort {k:?}: use 1 for ascending or -1 for descending");
            }
            check_ordered_field(k).map_err(|e| e.context("sort"))?;
        }
        b.push_str(".sort(");
        render_mongo(b, s);
        b.push(')');
    }
    if let Some(s) = skip {
        let n = whole_number(s, "skip")?;
        b.push_str(&format!(".skip({n})"));
    }
    let mut n = match limit {
        Some(l) => whole_number(l, "limit")?,
        None => 0,
    };
    if n == 0 || n > BYTEBASE_FETCH_ROWS {
        n = BYTEBASE_FETCH_ROWS;
    }
    b.push_str(&format!(".limit({n})"));
    Ok(())
}

fn whole_number(v: &Shell, what: &str) -> Result<usize> {
    let Shell::Number(n) = v else {
        return fail!("{what} takes a whole number");
    };
    match n.parse::<i64>() {
        Ok(i) => usize::try_from(i).map_err(|_| err!("{what} takes a whole number of 0 or more")),
        Err(_) => fail!("{what} takes a whole number of 0 or more"),
    }
}

fn plan_aggregate(b: &mut String, st: &Statement) -> Result<()> {
    let m = &st.method;
    let pipeline = match m.args.as_slice() {
        [Shell::Array(stages)] => stages,
        _ => return fail!("aggregate takes one pipeline, an array of stages, and no options"),
    };
    let generic: Vec<Value> = pipeline.iter().map(to_generic).collect();
    check_mongo_pipeline(&generic)?;
    for c in &st.chain {
        if c.name != "toArray" || !c.args.is_empty() {
            return fail!(".{} is not accepted after aggregate: only toArray", c.name);
        }
    }
    // A trailing $limit bounds the answer however the pipeline ends; the stage checks above have already run.
    let mut capped = pipeline.clone();
    capped.push(Shell::Object(vec![(
        "$limit".to_owned(),
        Shell::Number(BYTEBASE_FETCH_ROWS.to_string()),
    )]));
    b.push_str("aggregate(");
    render_mongo(b, &Shell::Array(capped));
    b.push(')');
    Ok(())
}

/// A count written as an aggregation: through Bytebase, countDocuments answers at most a small default (live: 25 for 3573
/// documents), while $match with $count counts them all.
fn plan_count(b: &mut String, st: &Statement) -> Result<()> {
    if st.method.args.len() > 1 || !st.chain.is_empty() {
        return fail!("countDocuments takes one filter and no chained call");
    }
    let filter = mongo_document(present(&st.method.args, 0), "the filter")?;
    b.push_str("aggregate(");
    render_mongo(
        b,
        &Shell::Array(vec![
            Shell::Object(vec![("$match".to_owned(), filter)]),
            Shell::Object(vec![("$count".to_owned(), Shell::Str("n".to_owned()))]),
        ]),
    );
    b.push(')');
    Ok(())
}

fn plan_distinct(b: &mut String, st: &Statement) -> Result<()> {
    let m = &st.method;
    let field = match m.args.first() {
        Some(Shell::Str(f)) if m.args.len() <= 2 && st.chain.is_empty() => f,
        _ => return fail!("distinct takes the field's name as a string and optionally a filter"),
    };
    check_ordered_field(field).map_err(|e| e.context("distinct"))?;
    b.push_str(&format!("distinct({}", mongo_json(field)));
    if m.args.len() == 2 {
        let filter = mongo_document(present(&m.args, 1), "the filter")?;
        b.push_str(", ");
        render_mongo(b, &filter);
    }
    b.push(')');
    Ok(())
}

/// `v` as the decoded-JSON shapes the filter and pipeline checks read: they look at keys and arrays only.
fn to_generic(v: &Shell) -> Value {
    match v {
        Shell::Object(members) => Value::Object(
            members
                .iter()
                .map(|(k, v)| (k.clone(), to_generic(v)))
                .collect::<Map<_, _>>(),
        ),
        Shell::Array(items) => Value::Array(items.iter().map(to_generic).collect()),
        Shell::Str(s) | Shell::Number(s) => Value::String(s.clone()),
        Shell::Bool(b) => Value::Bool(*b),
        Shell::Null | Shell::Ctor(..) | Shell::Regex { .. } => Value::Null,
    }
}

/// `s` as a JSON string with HTML left alone and U+2028 and U+2029 escaped, so a JavaScript reader cannot end a line
/// inside it.
fn mongo_json(s: &str) -> String {
    serde_json::to_string(s)
        .unwrap_or_else(|_| "\"\"".to_owned())
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// `v` written as mongosh reads it: the shape that was parsed, nothing added.
fn render_mongo(b: &mut String, v: &Shell) {
    match v {
        Shell::Object(members) => {
            b.push('{');
            for (i, (k, val)) in members.iter().enumerate() {
                if i > 0 {
                    b.push_str(", ");
                }
                b.push_str(&mongo_json(k));
                b.push_str(": ");
                render_mongo(b, val);
            }
            b.push('}');
        }
        Shell::Array(items) => {
            b.push('[');
            for (i, e) in items.iter().enumerate() {
                if i > 0 {
                    b.push_str(", ");
                }
                render_mongo(b, e);
            }
            b.push(']');
        }
        Shell::Str(s) => b.push_str(&mongo_json(s)),
        Shell::Number(n) => b.push_str(n),
        Shell::Bool(x) => b.push_str(if *x { "true" } else { "false" }),
        Shell::Null => b.push_str("null"),
        Shell::Ctor(name, arg) => {
            b.push_str(name);
            b.push('(');
            render_mongo(b, arg);
            b.push(')');
        }
        Shell::Regex { body, flags } => {
            b.push('/');
            b.push_str(body);
            b.push('/');
            b.push_str(flags);
        }
    }
}
