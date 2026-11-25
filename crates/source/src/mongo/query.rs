//! The pure half of the MongoDB source: what an agent typed becomes a query the driver may send, and what the driver returned
//! becomes an answer an agent can read. Nothing here touches the network, so every rule is tested with plain values.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bson::{Bson, Document};
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use crate::error::{Error, Result, err, fail};
use crate::limit::Limit;
use crate::mongoguard::{check_mongo_filter, check_mongo_pipeline, check_ordered_field};
use crate::redact::redact_value;
use crate::text::{cut_bytes, table};

pub(crate) const MONGO_LIMIT: Limit = Limit::new(20, 100);
pub(crate) const MONGO_SAMPLE: Limit = Limit::new(100, 500);
/// Bounds a filtered count: counting a filter that matches millions of documents is a scan the owner pays for.
pub(crate) const MONGO_COUNT_CAP: i64 = 1_000_000;

/// A JSON value as the agent wrote it: an object keeps the order of its keys, which a sort and a pipeline stage depend on.
#[derive(Debug, Clone)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        d.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> std::result::Result<Json, E> {
        Ok(Json::Null)
    }
    fn visit_bool<E>(self, v: bool) -> std::result::Result<Json, E> {
        Ok(Json::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> std::result::Result<Json, E> {
        Ok(Json::Number(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> std::result::Result<Json, E> {
        Ok(Json::Number(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Json, E> {
        Number::from_f64(v)
            .map(Json::Number)
            .ok_or_else(|| E::custom("a number JSON cannot hold"))
    }
    fn visit_str<E>(self, v: &str) -> std::result::Result<Json, E> {
        Ok(Json::String(v.to_owned()))
    }
    fn visit_string<E>(self, v: String) -> std::result::Result<Json, E> {
        Ok(Json::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Json, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = seq.next_element()? {
            out.push(v);
        }
        Ok(Json::Array(out))
    }
    // An object that repeats a key is refused: the guards read the decoded value, which keeps one of two equal keys, while the
    // server reads every key of the text it is sent.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Json, A::Error> {
        let mut out: Vec<(String, Json)> = Vec::new();
        let mut seen = BTreeSet::new();
        while let Some(k) = map.next_key::<String>()? {
            if !seen.insert(k.clone()) {
                return Err(de::Error::custom(format!(
                    "the key {k:?} appears twice in one object; the server would apply both"
                )));
            }
            let v = map.next_value()?;
            out.push((k, v));
        }
        Ok(Json::Object(out))
    }
}

impl Json {
    /// The value the guards read; key order does not matter to them.
    fn to_value(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(b) => Value::Bool(*b),
            Self::Number(n) => Value::Number(n.clone()),
            Self::String(s) => Value::String(s.clone()),
            Self::Array(a) => Value::Array(a.iter().map(Self::to_value).collect()),
            Self::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), v.to_value())).collect::<Map<_, _>>()),
        }
    }

    /// The value as BSON, read as relaxed Extended JSON ({"$oid": ...}, {"$date": ...}, {"$numberLong": ...}), the keys of
    /// every document in the order written.
    fn to_bson(&self) -> Result<Bson> {
        match self {
            Self::Array(a) => Ok(Bson::Array(a.iter().map(Self::to_bson).collect::<Result<_>>()?)),
            Self::Object(o) if o.iter().any(|(k, _)| EXTJSON_KEYS.contains(&k.as_str())) => {
                Bson::try_from(self.to_value()).map_err(|e| Error::msg(e.to_string()))
            }
            Self::Object(o) => {
                let mut d = Document::new();
                for (k, v) in o {
                    d.insert(k.clone(), v.to_bson()?);
                }
                Ok(Bson::Document(d))
            }
            other => Bson::try_from(other.to_value()).map_err(|e| Error::msg(e.to_string())),
        }
    }
}

/// The keys that make an object one value of Extended JSON rather than a document.
const EXTJSON_KEYS: &[&str] = &[
    "$oid",
    "$symbol",
    "$regularExpression",
    "$numberInt",
    "$numberLong",
    "$numberDouble",
    "$numberDecimal",
    "$binary",
    "$uuid",
    "$code",
    "$timestamp",
    "$date",
    "$minKey",
    "$maxKey",
    "$dbPointer",
    "$undefined",
];

/// `text` decoded as the agent's JSON: one value, no key twice in one object, numbers with their digits.
pub(crate) fn parse_json(text: &str) -> Result<Json> {
    serde_json::from_str(text).map_err(|e| {
        let m = e.to_string();
        if m.contains("appears twice") {
            Error::msg(m)
        } else {
            err!("not JSON: {m}")
        }
    })
}

type Guard = fn(&Value) -> Result<()>;

/// The Extended JSON object in `text` after the checks that guard it; empty text is the empty document. `label` names the
/// argument in an error.
fn parse_object(label: &str, text: &str, guards: &[Guard]) -> Result<Document> {
    if text.trim().is_empty() {
        return Ok(Document::new());
    }
    let j = parse_json(text).map_err(|e| e.context(label))?;
    if !matches!(j, Json::Object(_)) {
        return fail!("{label} must be a JSON object such as {{\"field\": value}}");
    }
    let v = j.to_value();
    for g in guards {
        g(&v).map_err(|e| e.context(label))?;
    }
    match j.to_bson().map_err(|e| e.context(label))? {
        Bson::Document(d) => Ok(d),
        _ => fail!("{label} must be a JSON object such as {{\"field\": value}}"),
    }
}

/// The Extended JSON array of stages in `text` after the guards, as the driver takes it.
pub(crate) fn parse_pipeline(text: &str) -> Result<Vec<Document>> {
    let j = parse_json(text).map_err(|e| e.context("pipeline"))?;
    let Json::Array(stages) = &j else {
        return fail!("pipeline must be a JSON array of stages such as [{{\"$match\": {{...}}}}]");
    };
    let values: Vec<Value> = stages.iter().map(Json::to_value).collect();
    check_mongo_pipeline(&values).map_err(|e| e.context("pipeline"))?;
    stages
        .iter()
        .map(|s| match s.to_bson().map_err(|e| e.context("pipeline"))? {
            Bson::Document(d) => Ok(d),
            _ => fail!("pipeline: a stage is not an object"),
        })
        .collect()
}

/// A filter (find, count, the $match of a sample) after the guard.
pub(crate) fn parse_filter(text: &str) -> Result<Document> {
    parse_object("filter", text, &[check_mongo_filter])
}

/// A projection; it may name a credential field (the answer removes its value) but not copy one.
pub(crate) fn parse_projection(text: &str) -> Result<Document> {
    parse_object("projection", text, &[check_mongo_filter])
}

/// A sort such as {"created_at": -1}: field names with 1 or -1, in the order written.
pub(crate) fn parse_sort(text: &str) -> Result<Document> {
    let d = parse_object("sort", text, &[])?;
    for (k, v) in &d {
        if !matches!(sort_direction(v), Some(1 | -1)) {
            return fail!("sort: {k:?} must be 1 or -1");
        }
        check_ordered_field(k).map_err(|e| e.context("sort"))?;
    }
    Ok(d)
}

fn sort_direction(v: &Bson) -> Option<i64> {
    match v {
        Bson::Int32(n) => Some(i64::from(*n)),
        Bson::Int64(n) => Some(*n),
        Bson::Double(f) if *f == 1.0 => Some(1),
        Bson::Double(f) if *f == -1.0 => Some(-1),
        _ => None,
    }
}

/// The database named in the path of a connection string, "" for none. The path starts at the first "/" after the scheme: a
/// "/" in a login or a host is percent-encoded in a connection string.
pub(crate) fn default_database(uri: &str) -> String {
    let Some((_, rest)) = uri.split_once("://") else {
        return String::new();
    };
    let Some((_, path)) = rest.split_once('/') else {
        return String::new();
    };
    path.split('?').next().unwrap_or_default().to_owned()
}

// Bound one document: a long text, a long list or a deep tree is cut, because an agent reads the shape of a document before
// it reads all of one.
const SHRINK_STRING: usize = 600;
const SHRINK_ITEMS: usize = 20;
const SHRINK_DEPTH: usize = 10;
/// How many bytes of documents one answer carries, below `OUTPUT_MAX` so the answer stays valid JSON.
const DOC_BUDGET: usize = 20000;
const NESTED: &str = "[nested deeper: project the fields you need]";

/// `v` with long strings, long lists and deep nesting cut and a note where it was cut.
fn shrink(v: Value, depth: usize) -> Value {
    match v {
        Value::Object(_) | Value::Array(_) if depth >= SHRINK_DEPTH => Value::String(NESTED.to_owned()),
        Value::Object(m) => Value::Object(
            m.into_iter()
                .map(|(k, val)| (k, shrink(val, depth.saturating_add(1))))
                .collect(),
        ),
        Value::Array(a) => {
            let n = a.len();
            let mut out: Vec<Value> = a
                .into_iter()
                .take(SHRINK_ITEMS)
                .map(|val| shrink(val, depth.saturating_add(1)))
                .collect();
            if n > SHRINK_ITEMS {
                out.push(Value::String(format!(
                    "[{} more items not shown]",
                    n.saturating_sub(SHRINK_ITEMS)
                )));
            }
            Value::Array(out)
        }
        Value::String(s) if s.len() > SHRINK_STRING => {
            let head = cut_bytes(&s, SHRINK_STRING);
            Value::String(format!("{head}…[{} more bytes]", s.len().saturating_sub(head.len())))
        }
        other => other,
    }
}

/// One document as relaxed Extended JSON (numbers stay numbers; {"$oid"}, {"$date"} and {"$numberLong"} keep what JSON cannot
/// say) with the value of every credential field removed and long values cut.
pub(crate) fn doc_json(doc: Document) -> Value {
    shrink(redact_value(&Bson::Document(doc).into_relaxed_extjson()), 0)
}

#[derive(Serialize)]
struct MongoDocs {
    collection: String,
    returned: usize,
    documents: Vec<Value>,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
}

/// The answer to a find or an aggregate that asked for at most `limit` documents and was given up to `limit`+1: the documents
/// that fit the budget, and a note when some are left out.
pub(crate) fn render_docs(collection: &str, mut docs: Vec<Document>, limit: usize) -> Result<String> {
    let more = docs.len() > limit;
    docs.truncate(limit);
    let given = docs.len();
    let mut out = MongoDocs {
        collection: collection.to_owned(),
        returned: 0,
        documents: Vec::new(),
        note: String::new(),
    };
    let mut used: usize = 0;
    for d in docs {
        let j = doc_json(d);
        let size = to_json(&j)?.len();
        if used.saturating_add(size) > DOC_BUDGET && !out.documents.is_empty() {
            break;
        }
        used = used.saturating_add(size);
        out.documents.push(j);
    }
    out.returned = out.documents.len();
    if out.returned < given {
        out.note = format!(
            "{} of {given} documents fit this answer: filter more, project fewer fields or lower limit",
            out.returned
        );
    } else if more {
        out.note = format!(
            "more documents match than the limit of {limit}: narrow the filter, or sort by _id and filter on the last _id seen to page"
        );
    } else if out.returned == 0 {
        "no documents matched".clone_into(&mut out.note);
    }
    to_json(&out)
}

/// `v` as compact JSON; `<`, `>` and `&` stay as they are, so an agent reads "orders_<shopID>".
pub(crate) fn to_json(v: &impl Serialize) -> Result<String> {
    serde_json::to_string(v).map_err(|e| err!("the answer could not be written as JSON: {e}"))
}

/// How many field paths a schema answer lists.
const SCHEMA_MAX_PATHS: usize = 300;
/// How deep a document is walked: below it, a path is a type and its children are not listed.
const SCHEMA_MAX_DEPTH: usize = 8;

#[derive(Debug, Default)]
pub(crate) struct SchemaField {
    pub path: String,
    /// documents of the sample that have the path
    pub seen: usize,
    /// documents of the sample, per type seen at the path
    pub types: BTreeMap<String, usize>,
}

/// The name of a BSON type in an agent's words: the ones its Extended JSON filters write.
fn type_name(v: &Bson) -> &'static str {
    match v {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Document(_) => "object",
        Bson::Array(_) => "array",
        Bson::Binary(_) => "binary",
        Bson::ObjectId(_) => "objectId",
        Bson::Boolean(_) => "bool",
        Bson::DateTime(_) => "date",
        Bson::Null => "null",
        Bson::Int32(_) => "int",
        Bson::Int64(_) => "long",
        Bson::Decimal128(_) => "decimal",
        Bson::Timestamp(_) => "timestamp",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "code with scope",
        Bson::Symbol(_) => "symbol",
        Bson::Undefined => "undefined",
        Bson::MaxKey => "max key",
        Bson::MinKey => "min key",
        Bson::DbPointer(_) => "dbPointer",
    }
}

/// Every field path of `docs`, the types it holds and in how many documents. An array's elements are the path with "[]" after
/// it; values are never reported, so the shape is learnt without reading data.
pub(crate) fn infer_schema(docs: &[Document]) -> Vec<SchemaField> {
    let mut by_path: BTreeMap<String, SchemaField> = BTreeMap::new();
    for doc in docs {
        let mut here: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
        walk_document(doc, "", 0, &mut here);
        for (path, types) in here {
            let f = by_path.entry(path.clone()).or_insert_with(|| SchemaField {
                path,
                ..SchemaField::default()
            });
            f.seen = f.seen.saturating_add(1);
            for t in types {
                let n = f.types.entry(t.to_owned()).or_default();
                *n = n.saturating_add(1);
            }
        }
    }
    by_path.into_values().collect()
}

fn walk_document(doc: &Document, prefix: &str, depth: usize, note: &mut BTreeMap<String, BTreeSet<&'static str>>) {
    for (k, v) in doc {
        walk_value(v, &format!("{prefix}{k}"), depth, note);
    }
}

fn walk_value(v: &Bson, path: &str, depth: usize, note: &mut BTreeMap<String, BTreeSet<&'static str>>) {
    note.entry(path.to_owned()).or_default().insert(type_name(v));
    if depth >= SCHEMA_MAX_DEPTH {
        return;
    }
    match v {
        Bson::Document(d) => walk_document(d, &format!("{path}."), depth.saturating_add(1), note),
        Bson::Array(items) => {
            let p = format!("{path}[]");
            for item in items {
                walk_value(item, &p, depth.saturating_add(1), note);
            }
        }
        _ => {}
    }
}

/// The fields laid out one line each: the path, its types by how many documents hold them, and the share of the sample that
/// has the path.
pub(crate) fn mongo_schema_table(fields: &[SchemaField], sampled: usize) -> String {
    let rows: Vec<Vec<String>> = fields
        .iter()
        .map(|f| {
            let mut types: Vec<(&String, &usize)> = f.types.iter().collect();
            types.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            let types: Vec<String> = types.iter().map(|(t, n)| format!("{t}:{n}")).collect();
            vec![f.path.clone(), types.join(" "), format!("{}/{sampled}", f.seen)]
        })
        .collect();
    let cols = ["path", "types (documents)", "present in"].map(String::from);
    table(&cols, &rows, SCHEMA_MAX_PATHS)
}
