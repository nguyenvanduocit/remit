//! The one rule set of a safe read of MongoDB, whichever way the agent wrote it: the Mongo source's JSON (mongo/query.rs) and
//! a mongosh statement sent through Bytebase (bytebase/mongoshell.rs) both decode into a `serde_json::Value` and are checked
//! here. A read is find, count, distinct and aggregate. An aggregation can also write (`$out`, `$merge`) or run JavaScript on
//! the server (`$where`, `$function`, `$accumulator`), and it can nest those inside `$lookup`, `$facet` or `$unionWith`, so
//! every key of a filter or a pipeline, at any depth, is checked; a pipeline's own stages must be ones that only shape the
//! read, and a join stays in the database of the call. A field that holds a credential may be tested for and projected (the
//! answer removes its value) but never compared, copied or ordered by. It is pure.

use serde_json::Value;

use crate::error::{Result, fail};
use crate::redact::sensitive_key;

/// The operators that write, run JavaScript, or show the server's own work.
const MONGO_FORBIDDEN: &[&str] = &[
    "$out",
    "$merge",
    "$where",
    "$function",
    "$accumulator",
    "$currentOp",
    "$listSessions",
    "$listLocalSessions",
    "$planCacheStats",
    "$collStats",
    "$indexStats",
];

/// The stages a pipeline may start a step with.
pub(crate) const MONGO_STAGES: &[&str] = &[
    "$match",
    "$project",
    "$group",
    "$sort",
    "$limit",
    "$skip",
    "$unwind",
    "$lookup",
    "$count",
    "$addFields",
    "$set",
    "$unset",
    "$facet",
    "$bucket",
    "$bucketAuto",
    "$sortByCount",
    "$replaceRoot",
    "$replaceWith",
    "$sample",
    "$unionWith",
    "$graphLookup",
    "$geoNear",
    "$redact",
    "$densify",
    "$fill",
    "$setWindowFields",
];

/// Ok when a filter or a projection (decoded from JSON) uses no forbidden operator at any depth and reads no credential.
pub fn check_mongo_filter(v: &Value) -> Result<()> {
    check_mongo_keys(v)?;
    check_no_credential_fields(v)
}

/// Ok when `pipeline` is a list of stages that only read: each stage one of the stages that shape a read, no forbidden
/// operator at any depth, no credential read, and no join out of the call's database.
pub fn check_mongo_pipeline(pipeline: &[Value]) -> Result<()> {
    if pipeline.is_empty() {
        return fail!("the pipeline is empty");
    }
    for (i, stage) in pipeline.iter().enumerate() {
        let Some(m) = stage.as_object().filter(|m| m.len() == 1) else {
            return fail!("stage {i} must be an object with one stage operator, such as {{\"$match\": {{...}}}}");
        };
        for name in m.keys() {
            if !MONGO_STAGES.contains(&name.as_str()) {
                return fail!(
                    "stage {i}: {name} is not a stage this tool runs; it reads with [{}]",
                    MONGO_STAGES.join(" ")
                );
            }
        }
    }
    let all = Value::Array(pipeline.to_vec());
    check_mongo_keys(&all)?;
    check_no_credential_fields(&all)?;
    check_collection_refs(&all)
}

/// Ok when the field `path` may order or list a read's values (a sort key, a distinct's field): it holds no credential, whose
/// values the order or the list would reveal.
pub(crate) fn check_ordered_field(path: &str) -> Result<()> {
    if sensitive_path(path) {
        return fail!("{path:?} holds a credential; the order or the list of its values would reveal them");
    }
    Ok(())
}

/// The operators that build a field name at run time, which would slip past a check on names.
const MONGO_DYNAMIC_READERS: &[&str] = &["$getField", "$objectToArray"];

/// Refuses a filter or a pipeline that filters on, sorts by, groups by or copies a field that holds a credential. The answer
/// drops the value of such a field by its name, so copying it under another name ({"$project": {"x": "$accessToken"}}) or
/// probing it ({"accessToken": {"$regex": "^a"}}) would read what the answer hides. Testing that a credential exists and
/// projecting the field (the answer removes its value) stay allowed: a missing token is something an investigation needs to
/// see.
fn check_no_credential_fields(v: &Value) -> Result<()> {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                if MONGO_DYNAMIC_READERS.contains(&k.as_str()) {
                    return fail!(
                        "{k} reads a field by a computed name, which could reach a credential; name the field"
                    );
                }
                if (k == "$sort" || k == "sortBy") && sorts_by_credential(val) {
                    return fail!(
                        "{k} names a field that holds a credential; the order of its values would reveal them"
                    );
                }
                if !k.starts_with('$') && sensitive_path(k) && !value_free(val) {
                    return fail!(
                        "the field {k:?} holds a credential: a query may test that it exists ({{\"$exists\": true}}) but not read, compare or sort by it"
                    );
                }
                check_no_credential_fields(val)?;
            }
        }
        Value::Array(a) => {
            for val in a {
                check_no_credential_fields(val)?;
            }
        }
        Value::String(s) if s.starts_with('$') && sensitive_path(s.trim_start_matches('$')) => {
            return fail!("{s:?} refers to a field that holds a credential; this tool does not copy or compare one");
        }
        _ => {}
    }
    Ok(())
}

/// Whether a sort specification orders by a credential field.
fn sorts_by_credential(spec: &Value) -> bool {
    spec.as_object().is_some_and(|m| m.keys().any(|f| sensitive_path(f)))
}

/// Whether any segment of a dotted field path names a credential.
fn sensitive_path(path: &str) -> bool {
    path.split('.').any(sensitive_key)
}

/// A value a credential field may be given in a query without revealing it: nothing, a flag or number (a projection's 1 or
/// 0), or {"$exists": ...}.
fn value_free(v: &Value) -> bool {
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) => true,
        Value::Object(m) => m.len() == 1 && m.get("$exists").is_some_and(Value::is_boolean),
        _ => false,
    }
}

/// The stages that read another collection.
const MONGO_JOINS: &[&str] = &["$lookup", "$unionWith", "$graphLookup"];

/// Refuses a join that reaches into another database or a system collection (users' hashes, the oplog).
fn check_collection_refs(v: &Value) -> Result<()> {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                if MONGO_JOINS.contains(&k.as_str()) {
                    check_join(val).map_err(|e| e.context(k))?;
                }
                check_collection_refs(val)?;
            }
        }
        Value::Array(a) => {
            for val in a {
                check_collection_refs(val)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_join(spec: &Value) -> Result<()> {
    match spec {
        Value::String(s) => check_collection_name(s),
        Value::Object(s) => {
            if s.contains_key("db") {
                return fail!("it may read only the database of the call");
            }
            for field in ["from", "coll"] {
                match s.get(field) {
                    Some(Value::String(t)) => check_collection_name(t)?,
                    Some(Value::Object(_)) => return fail!("it may read only the database of the call"),
                    _ => {}
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The databases that hold the accounts, the oplog and the cluster's own settings: none of it is the product's data.
pub(crate) const MONGO_SYSTEM_DATABASES: &[&str] = &["admin", "local", "config"];

/// Ok when `db` and `coll` name a collection of the product's data.
pub(crate) fn check_namespace(db: &str, coll: &str) -> Result<()> {
    check_database_name(db)?;
    check_collection_name(coll)
}

pub(crate) fn check_database_name(db: &str) -> Result<()> {
    if db.is_empty()
        || db.len() > 64
        || db.contains(['/', '\\', '.', ' ', '"', '$', '*', '<', '>', ':', '|', '?', '\0'])
    {
        return fail!("{db:?} is not a database name; mongo_collections lists them");
    }
    if MONGO_SYSTEM_DATABASES.contains(&db) {
        return fail!("the database {db:?} holds the cluster's own accounts and settings, not data");
    }
    Ok(())
}

pub(crate) fn check_collection_name(coll: &str) -> Result<()> {
    if coll.is_empty() || coll.len() > 255 || coll.contains(['$', '\0']) {
        return fail!("{coll:?} is not a collection name; mongo_collections lists them");
    }
    if coll.starts_with("system.") {
        return fail!("{coll:?} is a system collection, not data");
    }
    Ok(())
}

fn check_mongo_keys(v: &Value) -> Result<()> {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                if MONGO_FORBIDDEN.contains(&k.as_str()) {
                    return fail!("{k} is not allowed: this tool only reads");
                }
                check_mongo_keys(val)?;
            }
            Ok(())
        }
        Value::Array(a) => a.iter().try_for_each(check_mongo_keys),
        _ => Ok(()),
    }
}
