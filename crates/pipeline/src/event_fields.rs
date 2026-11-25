//! What the Assistant is shown of an event to write a `when` from: the names of its fields as the expressions spell them,
//! and the kind of value each holds. What a writer wrote is never the owner's, so words stay out: a value is shown only
//! when it is short and made of the characters of an identifier or an event name (`failed`, `Merge Request Hook`), which
//! is what a condition compares to, and every other text is shown as its kind alone. That keeps a commit message or a
//! comment, where an instruction could hide, out of the Assistant's context.

use model::InboundEvent;
use serde_json::Value;

/// Lines shown: an event may have thousands of fields.
const FIELDS_MAX: usize = 200;
const FIELD_DEPTH_MAX: usize = 8;
const FIELD_VALUE_MAX: usize = 24;
/// How many items of a list are shown: the first stands for the rest.
const FIELDS_ITEMS_MAX: usize = 1;

/// Whether s is a name or a token, not a sentence: one or two words of `[A-Za-z0-9_.:/@-]`.
fn is_plain(s: &str) -> bool {
    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || "_.:/@-".contains(c));
    match s.split_once(' ') {
        None => word(s),
        Some((a, b)) => word(a) && word(b),
    }
}

/// Whether k can follow a dot in an expression.
fn is_identifier(k: &str) -> bool {
    let mut cs = k.chars();
    cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The fields of ev, one per line: `header['X-Gitlab-Event'] = "Pipeline Hook"`, `body.object_attributes.status =
/// "failed"`, `body.user.name: text`. It says so when it was cut or when the body is none that can be read.
pub fn fields_of(ev: &InboundEvent) -> String {
    let mut lines: Vec<String> = ev
        .header
        .iter()
        .map(|(k, v)| format!("header['{k}']{}", field_value(&Value::String(v.clone()))))
        .collect();
    if ev.body_cut {
        lines.push("body: cut (longer than what is kept), so no `when` can read it".to_owned());
    } else {
        match serde_json::from_str::<Value>(&ev.body) {
            Ok(body) => walk_fields(&mut lines, "body", &body, 0),
            Err(_) => lines.push("body: not JSON, so no `when` can read it".to_owned()),
        }
    }
    if lines.len() > FIELDS_MAX {
        let more = lines.len().saturating_sub(FIELDS_MAX);
        lines.truncate(FIELDS_MAX);
        lines.push(format!("… {more} more fields not shown"));
    }
    lines.join("\n")
}

/// What a line says of a value that is not an object or a list.
fn field_value(v: &Value) -> String {
    match v {
        Value::String(s) if s.chars().count() <= FIELD_VALUE_MAX && is_plain(s) => format!(" = {s:?}"),
        Value::String(_) => ": text".to_owned(),
        Value::Number(_) => ": number".to_owned(),
        Value::Bool(b) => format!(" = {b}"),
        Value::Null => ": null".to_owned(),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

fn walk_fields(lines: &mut Vec<String>, path: &str, v: &Value, depth: usize) {
    match v {
        Value::Object(m) => {
            if depth >= FIELD_DEPTH_MAX {
                lines.push(format!("{path}: object, deeper than is shown"));
                return;
            }
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            for k in keys {
                let child = if is_identifier(k) {
                    format!("{path}.{k}")
                } else {
                    format!("{path}['{}']", k.replace('\'', ""))
                };
                if let Some(x) = m.get(k) {
                    walk_fields(lines, &child, x, depth.saturating_add(1));
                }
            }
        }
        Value::Array(xs) => {
            lines.push(format!("{path}: list of {}", xs.len()));
            for (i, x) in xs.iter().take(FIELDS_ITEMS_MAX).enumerate() {
                walk_fields(lines, &format!("{path}[{i}]"), x, depth.saturating_add(1));
            }
        }
        other => lines.push(format!("{path}{}", field_value(other))),
    }
}
