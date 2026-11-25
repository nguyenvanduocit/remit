use std::collections::{BTreeMap, BTreeSet};

use model::ChangedField;
use serde_json::{Map, Value};

/// How many characters of a field's value a change keeps in its list of fields. The change keeps both records whole, so
/// the list is for reading, and a long skill file or instruction need not be there twice.
const MAX_FIELD_VALUE: usize = 600;

/// Ends a value that was cut.
const CLIP_MARK: &str = "…";

/// The store's counter of writes, not part of what a record says.
const REVISION_FIELD: &str = "rev";

/// The fields that differ between two records written as JSON, by name. A record of the library or of the playbooks is
/// one JSON object; `before` is empty for a record that did not exist and `after` for one that is gone. A field that holds
/// an object (a skill's files) is compared key by key and named "field.key", so the list says which file changed. The
/// fields are in the order of their names, each value is compact JSON clipped to `MAX_FIELD_VALUE` characters, and a
/// field that is not there is empty. What is not a record gives no fields: a change keeps the records whole.
pub fn diff_fields(before: &str, after: &str) -> Vec<ChangedField> {
    let (Some(b), Some(a)) = (fields_of(before), fields_of(after)) else {
        return Vec::new();
    };
    union(&b, &a)
        .into_iter()
        .filter(|name| *name != REVISION_FIELD)
        .flat_map(|name| diff_field(name, b.get(name), a.get(name)))
        .collect()
}

/// The difference of one field: itself, or its keys when both sides hold an object.
fn diff_field(name: &str, before: Option<&Value>, after: Option<&Value>) -> Vec<ChangedField> {
    if let (Some(Value::Object(bo)), Some(Value::Object(ao))) = (before, after) {
        return union(bo, ao)
            .into_iter()
            .filter_map(|key| differ(format!("{name}.{key}"), bo.get(key), ao.get(key)))
            .collect();
    }
    differ(name.to_owned(), before, after).into_iter().collect()
}

/// The field as one change if its two values differ, and none if they are the same.
fn differ(name: String, before: Option<&Value>, after: Option<&Value>) -> Option<ChangedField> {
    let (b, a) = (render(before), render(after));
    (b != a).then(|| ChangedField {
        name,
        before: clip(&b),
        after: clip(&a),
    })
}

/// The fields of a record written as JSON; an empty text is a record with none, and what is not one JSON object is none.
fn fields_of(record: &str) -> Option<Map<String, Value>> {
    if record.is_empty() {
        return Some(Map::new());
    }
    match serde_json::from_str(record) {
        Ok(Value::Object(fields)) => Some(fields),
        _ => None,
    }
}

/// The keys of both objects, sorted, each once.
fn union<'a>(a: &'a Map<String, Value>, b: &'a Map<String, Value>) -> BTreeSet<&'a str> {
    a.keys().chain(b.keys()).map(String::as_str).collect()
}

/// A value as `json_text` writes it, read with every number a float64, so two values that mean the same read the
/// same; empty for a value that is not there.
fn render(value: Option<&Value>) -> String {
    value.map(|v| json_text(&numbers_as_floats(v))).unwrap_or_default()
}

/// `v` with every number read as a float64.
fn numbers_as_floats(v: &Value) -> Value {
    match v {
        Value::Number(n) => n.as_f64().map_or_else(|| v.clone(), Value::from),
        Value::Array(items) => Value::Array(items.iter().map(numbers_as_floats).collect()),
        Value::Object(fields) => Value::Object(fields.iter().map(|(k, v)| (k.clone(), numbers_as_floats(v))).collect()),
        Value::Null | Value::Bool(_) | Value::String(_) => v.clone(),
    }
}

/// `v` as compact JSON with the keys of its objects sorted and nothing HTML-escaped: the one way a change names a value,
/// so what a rail check says and what a diff says read the same, before and after an upgrade. A float is written as
/// `float` writes it, and U+2028 and U+2029 are escaped.
pub fn json_text(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v);
    out
}

/// `f` as the shortest decimal that reads back the same, an integral value with no `.0`, the exponent form only below
/// 1e-6 or from 1e21, a positive exponent signed: the form every change named a float in since Remit began.
fn float(f: f64) -> String {
    let abs = f.abs();
    if abs == 0.0 || (1e-6..1e21).contains(&abs) {
        return format!("{f}");
    }
    let exponent = format!("{f:e}");
    match exponent.split_once('e') {
        Some((mantissa, exp)) if !exp.starts_with('-') => format!("{mantissa}e+{exp}"),
        _ => exponent,
    }
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => out.push_str(&i.to_string()),
            (None, Some(u), _) => out.push_str(&u.to_string()),
            (None, None, Some(f)) => out.push_str(&float(f)),
            (None, None, None) => out.push_str(&n.to_string()),
        },
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(fields) => {
            // Sorted here whatever order the map keeps.
            let sorted: BTreeMap<&String, &Value> = fields.iter().collect();
            out.push('{');
            for (i, (k, v)) in sorted.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, k);
                out.push(':');
                write_value(out, v);
            }
            out.push('}');
        }
    }
}

/// A string as JSON with HTML left unescaped and U+2028 and U+2029 escaped.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < '\u{20}' || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `s` cut to `MAX_FIELD_VALUE` characters, with `CLIP_MARK` where it was cut.
fn clip(s: &str) -> String {
    match s.char_indices().nth(MAX_FIELD_VALUE) {
        Some((cut, _)) => format!("{}{CLIP_MARK}", s.get(..cut).unwrap_or(s)),
        None => s.to_owned(),
    }
}
