//! What an agent reads of a source never carries a credential. Two nets: a structured answer drops the value of every field
//! whose name says it holds one (`redact_value`), and any text is scanned for the shape of a token (`redact_text`). The first
//! knows the field; the second catches what a free-text field, a log line or an error carries.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Number, Value};

use crate::error::{Error, Result};

/// What a removed credential reads as.
pub const REMOVED: &str = "[removed]";

/// The ends of a field name (lower-cased, without separators) that holds a credential.
const CREDENTIAL_SUFFIXES: &[&str] = &[
    "token",
    "password",
    "passwd",
    "passphrase",
    "pwd",
    "secret",
    "apikey",
    "accesskey",
    "secretkey",
    "privatekey",
    "authorization",
    "cookie",
    "credential",
    "credentials",
    "passwordhash",
    "salt",
];

/// Whether a field of that name holds a credential. `tokenExpiresAt` does not: it ends with something else.
#[must_use]
pub fn sensitive_key(name: &str) -> bool {
    let n: String = name
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '_' | '-' | '.' | ' '))
        .collect();
    CREDENTIAL_SUFFIXES.iter().any(|s| n.ends_with(s))
}

/// `v` with the value of every credential field replaced. An empty value stays empty: that a token is missing is something an
/// investigation needs to see.
#[must_use]
pub fn redact_value(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, val)| {
                    if sensitive_key(k) && !is_empty(val) {
                        (k.clone(), Value::String(REMOVED.to_owned()))
                    } else {
                        (k.clone(), redact_value(val))
                    }
                })
                .collect::<Map<_, _>>(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}

/// `s` redacted (`redact_text`) and its email addresses masked.
#[must_use]
pub fn clean_text(s: &str) -> String {
    mask_emails(&redact_text(s))
}

/// `v` with each text cleaned (`clean_text`), the names of its fields too: a key is text an agent chose, and a token fits in
/// one. Each text is cleaned on its own, so a replacement never crosses the structure of the JSON around it.
pub fn clean_texts(v: Value) -> Value {
    match v {
        Value::String(s) => Value::String(clean_text(&s)),
        Value::Object(m) => {
            let mut out = Map::new();
            for (k, val) in m {
                out.insert(free_key(&out, clean_text(&k)), clean_texts(val));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(clean_texts).collect()),
        other => other,
    }
}

/// `key` as a name `taken` holds no entry under: two names that clean to the same text (two masked addresses) are both
/// kept, the later ones with a numbered suffix, so cleaning never drops a field.
fn free_key(taken: &Map<String, Value>, key: String) -> String {
    if !taken.contains_key(&key) {
        return key;
    }
    (2_u32..)
        .map(|n| format!("{key}#{n}"))
        .find(|candidate| !taken.contains_key(candidate))
        .unwrap_or(key)
}

fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

/// `v` as compact JSON with its credential fields removed. A struct is read through its JSON, so the names the agent sees
/// are the ones that are checked; keys come out sorted.
pub fn render(v: &impl Serialize) -> Result<String> {
    let generic = serde_json::to_value(v).map_err(|e| Error::msg(e.to_string()))?;
    render_value(&generic)
}

/// `render` for a document already decoded.
pub fn render_value(v: &Value) -> Result<String> {
    serde_json::to_string(&integral(redact_value(v))).map_err(|e| Error::msg(e.to_string()))
}

/// `v` with every whole float written as the integer it is (`1`, not `1.0`).
fn integral(v: Value) -> Value {
    const EXACT: f64 = 9_007_199_254_740_992.0; // 2^53: every whole float below is an exact integer
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if !n.is_i64() && !n.is_u64() && f.fract() == 0.0 && f.abs() < EXACT => {
                Value::Number(whole(f).map_or(n, Number::from))
            }
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(integral).collect()),
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, v)| (k, integral(v))).collect()),
        other => other,
    }
}

/// The integer a whole float below 2^53 is.
fn whole(f: f64) -> Option<i64> {
    // Formatting a whole float below 2^53 with no fraction digits and reading it back is exact, and needs no cast.
    format!("{f:.0}").parse().ok()
}

struct Shape {
    re: Regex,
    with: &'static str,
}

#[expect(clippy::expect_used, reason = "the patterns are constants a test compiles")]
static TOKEN_SHAPES: LazyLock<Vec<Shape>> = LazyLock::new(|| {
    [
        (r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----", "[private key removed]"),
        (r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*", "[token removed]"),
        (r"(?-u:\b)sh(?:pat|pca|ppa|pss)_[A-Za-z0-9]{16,}", "[token removed]"),
        (r"(?-u:\b)glpat-[A-Za-z0-9_-]{16,}", "[token removed]"),
        (r"(?-u:\b)gh[opsur]_[A-Za-z0-9]{20,}", "[token removed]"),
        (r"(?-u:\b)github_pat_[A-Za-z0-9_]{20,}", "[token removed]"),
        (r"(?-u:\b)xox[abprs]-[A-Za-z0-9-]{10,}", "[token removed]"),
        (r"(?-u:\b)(?:AKIA|ASIA)[0-9A-Z]{16}(?-u:\b)", "[key removed]"),
        (r"(?i)(?-u:\b)bearer\s+[A-Za-z0-9._~+/=-]{8,}", "Bearer [token removed]"),
        (r"(?i)(?-u:\b)([a-z][a-z0-9+.-]*://)[^\s:/@]+:[^\s@/]+@", "${1}[credential removed]@"),
        (
            r#"(?i)(?-u:\b)(password|passwd|pwd|secret|api[_-]?key|access[_-]?token|refresh[_-]?token)(?-u:\b)(["']?\s*[:=]\s*["']?)[^\s"',;})]+"#,
            "${1}${2}[removed]",
        ),
    ]
    .into_iter()
    .map(|(re, with)| Shape { re: Regex::new(re).expect("a token shape compiles"), with })
    .collect()
});

/// `s` with every token-shaped string, connection-string login and `password=...` pair replaced.
#[must_use]
pub fn redact_text(s: &str) -> String {
    let mut out = s.to_owned();
    for t in TOKEN_SHAPES.iter() {
        out = t.re.replace_all(&out, t.with).into_owned();
    }
    out
}

#[expect(clippy::expect_used, reason = "the pattern is a constant a test compiles")]
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([A-Za-z0-9._%+-])[A-Za-z0-9._%+-]*@([A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+)")
        .expect("the email shape compiles")
});

/// Every address with its first letter and its domain kept: enough to tell customers apart and to find the shop, not enough
/// to write to anyone.
#[must_use]
pub fn mask_emails(s: &str) -> String {
    EMAIL.replace_all(s, "${1}***@${2}").into_owned()
}

/// `s` without any of the secret values. A hostile or confused program may print what it was given.
pub(crate) fn scrub(s: &str, secrets: &[String]) -> String {
    let mut out = s.to_owned();
    for v in secrets {
        if !v.is_empty() {
            out = out.replace(v.as_str(), REMOVED);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn two_names_that_clean_to_the_same_text_are_both_kept() {
        let cleaned = clean_texts(json!({"ann@example.com": 1, "alice@example.com": 2, "other": 3}));
        let object = cleaned.as_object().unwrap();
        let mut values: Vec<i64> = object.values().filter_map(Value::as_i64).collect();
        values.sort_unstable();
        assert_eq!(values, vec![1, 2, 3], "no value is dropped: {cleaned}");
        assert!(object.contains_key("a***@example.com"), "{cleaned}");
        assert!(object.contains_key("a***@example.com#2"), "{cleaned}");
    }
}
