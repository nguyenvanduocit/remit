//! A call's canonical input and its hash (ADR-0075, decision 6): `workflow_steps` hold each call's hash, and a run
//! replays against them, so the spelling is fixed byte for byte: object keys sorted by their bytes, the last of a
//! repeated key kept, numbers as written, no insignificant space, and `<`, `>`, `&`, U+2028 and U+2029 as `\u` escapes,
//! a lone surrogate as U+FFFD. serde_json writes none of that (`1e21` comes out `1e+21`, `1.50` comes out `1.5`), so
//! the value is read and written here.

use std::collections::BTreeMap;

use model::WorkflowVerb;
use sha2::{Digest, Sha256};

/// How deep an input may nest. A call's input that nests past this is no prompt, and the reader
/// recurses once a level on a thread QuickJS also runs on.
const MAX_DEPTH: usize = 1000;

/// A JSON value as the canonical spelling reads it: a number keeps its text.
enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

/// A JSON value in one spelling: two inputs equal as values hash the same however the script built them.
pub(crate) fn canonical(raw: &str) -> Result<String, String> {
    let mut r = Reader { rest: raw };
    let v = r
        .value(0)
        .and_then(|v| {
            r.space();
            if r.rest.is_empty() {
                Ok(v)
            } else {
                Err("text after the value".to_owned())
            }
        })
        .map_err(|e| format!("read call input: {e}"))?;
    let mut out = String::with_capacity(raw.len());
    write_value(&mut out, &v);
    Ok(out)
}

/// Names a call by its verb and canonical input, so a replay can tell whether the call it reached is the one the journal
/// recorded at that step: `sha256(verb ‖ 0x00 ‖ input)` in lowercase hex.
pub(crate) fn input_hash(verb: &WorkflowVerb, input: &str) -> String {
    let mut h = Sha256::new();
    h.update(verb.as_str().as_bytes());
    h.update([0u8]);
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

/// Reads one JSON value from the front of `rest`.
struct Reader<'a> {
    rest: &'a str,
}

impl Reader<'_> {
    fn space(&mut self) {
        self.rest = self.rest.trim_start_matches([' ', '\t', '\n', '\r']);
    }

    fn eat(&mut self, token: char) -> bool {
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn expect(&mut self, token: char) -> Result<(), String> {
        self.space();
        if self.eat(token) {
            Ok(())
        } else {
            Err(format!("expected {token:?}"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(format!("nested deeper than {MAX_DEPTH} levels"));
        }
        self.space();
        let inner = depth.saturating_add(1);
        match self.rest.chars().next() {
            Some('{') => self.object(inner),
            Some('[') => self.array(inner),
            Some('"') => self.string().map(Value::String),
            Some('-' | '0'..='9') => self.number().map(Value::Number),
            _ => {
                for (word, v) in [
                    ("null", Value::Null),
                    ("true", Value::Bool(true)),
                    ("false", Value::Bool(false)),
                ] {
                    if let Some(rest) = self.rest.strip_prefix(word) {
                        self.rest = rest;
                        return Ok(v);
                    }
                }
                Err("not the beginning of a value".to_owned())
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.expect('{')?;
        let mut out = BTreeMap::new();
        self.space();
        if self.eat('}') {
            return Ok(Value::Object(out));
        }
        loop {
            self.space();
            let key = self.string()?;
            self.expect(':')?;
            out.insert(key, self.value(depth)?);
            self.space();
            if self.eat('}') {
                return Ok(Value::Object(out));
            }
            self.expect(',')?;
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.expect('[')?;
        let mut out = Vec::new();
        self.space();
        if self.eat(']') {
            return Ok(Value::Array(out));
        }
        loop {
            out.push(self.value(depth)?);
            self.space();
            if self.eat(']') {
                return Ok(Value::Array(out));
            }
            self.expect(',')?;
        }
    }

    /// A number's text, checked against JSON's grammar and kept as written.
    fn number(&mut self) -> Result<String, String> {
        let text = self.rest;
        self.eat('-');
        if !self.eat('0') && self.digits() == 0 {
            return Err("a number needs a digit".to_owned());
        }
        if self.eat('.') && self.digits() == 0 {
            return Err("a number needs a digit after its point".to_owned());
        }
        if self.eat('e') || self.eat('E') {
            if !self.eat('+') {
                self.eat('-');
            }
            if self.digits() == 0 {
                return Err("a number needs a digit in its exponent".to_owned());
            }
        }
        let read = text.len().saturating_sub(self.rest.len());
        text.get(..read)
            .map(str::to_owned)
            .ok_or_else(|| "a number out of place".to_owned())
    }

    fn digits(&mut self) -> usize {
        let before = self.rest.len();
        self.rest = self.rest.trim_start_matches(|c: char| c.is_ascii_digit());
        before.saturating_sub(self.rest.len())
    }

    /// A string's value, its escapes read: a surrogate that is not half of a pair is U+FFFD.
    fn string(&mut self) -> Result<String, String> {
        if !self.eat('"') {
            return Err("expected a string".to_owned());
        }
        let mut out = String::new();
        loop {
            let mut chars = self.rest.chars();
            let c = chars.next().ok_or("a string is not closed")?;
            self.rest = chars.as_str();
            match c {
                '"' => return Ok(out),
                '\\' => self.escape(&mut out)?,
                c if c < ' ' => return Err("a control character in a string".to_owned()),
                c => out.push(c),
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> Result<(), String> {
        let mut chars = self.rest.chars();
        let c = chars.next().ok_or("a string is not closed")?;
        self.rest = chars.as_str();
        let plain = match c {
            '"' => '"',
            '\\' => '\\',
            '/' => '/',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                let unit = self.hex4()?;
                if !(0xD800..0xE000).contains(&unit) {
                    char::from_u32(unit).ok_or("an escape out of range")?
                } else if let Some(pair) = self.low_half(unit) {
                    pair
                } else {
                    char::REPLACEMENT_CHARACTER
                }
            }
            _ => return Err(format!("an unknown escape \\{c}")),
        };
        out.push(plain);
        Ok(())
    }

    /// The character a high surrogate makes with the `\uXXXX` low surrogate after it, which is then read; none when
    /// `high` is not a high surrogate or no low one follows, and nothing is read.
    fn low_half(&mut self, high: u32) -> Option<char> {
        if !(0xD800..0xDC00).contains(&high) {
            return None;
        }
        let digits = self.rest.strip_prefix("\\u")?.get(..4)?;
        let low = u32::from_str_radix(digits, 16).ok()?;
        if !(0xDC00..0xE000).contains(&low) {
            return None;
        }
        let c = high
            .checked_sub(0xD800)?
            .checked_shl(10)?
            .checked_add(low.checked_sub(0xDC00)?)?
            .checked_add(0x10000)
            .and_then(char::from_u32)?;
        self.rest = self.rest.get(6..)?;
        Some(c)
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.rest.get(..4).filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()));
        let digits = digits.ok_or("a \\u escape needs four hex digits")?;
        self.rest = self.rest.get(4..).unwrap_or_default();
        u32::from_str_radix(digits, 16).map_err(|e| e.to_string())
    }
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(n),
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
            out.push('{');
            for (i, (k, item)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, k);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

/// A string as the canonical spelling writes it: HTML characters, U+2028 and U+2029 escaped.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' || matches!(c, '<' | '>' | '&' | '\u{2028}' | '\u{2029}') => {
                out.push_str(&format!("\\u{:04x}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
