//! Cutting and laying out answers, and writing values as plain text.

use serde_json::Value;

/// The most an answer carries, in bytes: about what an agent reads without losing its place.
pub const OUTPUT_MAX: usize = 24000;

/// `s` cut at `n` bytes (on a character), with a note that says how much was left out and that the call should be narrowed.
/// A shorter `s` is returned as it is.
#[must_use]
pub fn limit(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_owned();
    }
    let head = cut_bytes(s, n);
    format!(
        "{head}\n[cut: {} of {} bytes shown; narrow the query (a shorter period, a filter, fewer columns) to see the rest]",
        head.len(),
        s.len()
    )
}

/// The longest start of `s` that is at most `n` bytes and ends on a character.
#[must_use]
pub fn cut_bytes(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    s.get(..s.floor_char_boundary(n)).unwrap_or_default()
}

/// The end of `s` from byte `from` on, moved forward to a character.
pub(crate) fn tail_from(s: &str, from: usize) -> &str {
    s.get(s.ceil_char_boundary(from)..).unwrap_or_default()
}

/// `rows` under `cols` as tab-separated lines, at most `max_rows` of them, saying when rows were left out. A cell's tabs and
/// newlines become spaces so a row stays one line.
#[must_use]
pub fn table(cols: &[String], rows: &[Vec<String>], max_rows: usize) -> String {
    let mut b = cols.join("\t");
    for (i, row) in rows.iter().enumerate() {
        if i == max_rows {
            b.push_str(&format!(
                "\n[{} more rows not shown; narrow the query or add a LIMIT]",
                rows.len().saturating_sub(max_rows)
            ));
            break;
        }
        b.push('\n');
        for (j, cell) in row.iter().enumerate() {
            if j > 0 {
                b.push('\t');
            }
            b.push_str(&cell.replace(['\t', '\n', '\r'], " "));
        }
    }
    if rows.is_empty() {
        b.push_str("\n[no rows]");
    }
    b
}

/// `s` cut to `n` characters, with an ellipsis when it was longer.
pub fn shorten(s: &str, n: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(n).collect();
    if chars.next().is_some() { head + "…" } else { head }
}

/// A decoded JSON value as plain text: text as it is, a number as a float64, null as `<nil>`, a list as `[a b]` and an
/// object as `map[k:v]` (keys sorted).
pub fn value_text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), |f| float_prec(f, 21)),
        Value::Null => "<nil>".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => format!("[{}]", items.iter().map(value_text_of).collect::<Vec<_>>().join(" ")),
        Value::Object(m) => {
            let mut pairs: Vec<(&String, &Value)> = m.iter().collect();
            pairs.sort_by(|a, b| a.0.cmp(b.0));
            let parts: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}:{}", value_text_of(v))).collect();
            format!("map[{}]", parts.join(" "))
        }
    }
}

/// `s` quoted: in double quotes, with `\"`, `\\` and the C escapes, a control byte as `\xNN`, and a character that does
/// not print (a C1 control, a space other than U+0020) as `\uNNNN`.
pub fn quote_escaped(s: &str) -> String {
    let mut out = String::with_capacity(s.len().saturating_add(2));
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            c if c.is_ascii_control() => out.push_str(&format!("\\x{:02x}", u32::from(c))),
            c if c.is_control() || (c.is_whitespace() && c != ' ') => {
                let n = u32::from(c);
                if n > 0xFFFF {
                    out.push_str(&format!("\\U{n:08x}"));
                } else {
                    out.push_str(&format!("\\u{n:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `f` in the general format: the shortest digits, in exponent form below 1e-4 or from 1e6.
pub(crate) fn float_short(f: f64) -> String {
    float_prec(f, 6)
}

/// `f` with the shortest digits that read back as it, in exponent form when its exponent is below -4 or at least `eprec`.
pub(crate) fn float_prec(f: f64, eprec: i32) -> String {
    if f.is_nan() {
        return "NaN".to_owned();
    }
    if f.is_infinite() {
        return if f > 0.0 { "+Inf" } else { "-Inf" }.to_owned();
    }
    let sci = format!("{f:e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if exp < -4 || exp >= eprec {
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mantissa}e{sign}{:02}", exp.unsigned_abs());
    }
    format!("{f}")
}
