//! The command line's flags, as every service file and script passes them (`-addr 127.0.0.1:4320 -data /var/lib/remit`,
//! `-autonomous=false`, `-turn-idle 30m`): one or two dashes, `-name value` or `-name=value`, a bool set by its name
//! alone, parsing stopped at the first word that is not a flag or at `--`. Pure.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

/// What a flag holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Str,
    Bool,
    Int,
    Duration,
}

/// One flag: its name without the dash, what it holds, its default as the usage prints it, and what it is for.
#[derive(Debug, Clone)]
pub struct Flag {
    pub name: &'static str,
    pub kind: Kind,
    pub default: String,
    pub usage: &'static str,
}

/// A flag's value, read as its kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Bool(bool),
    Int(i64),
    Duration(Duration),
}

/// Why a command line was not read.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// `-h` or `-help`: the caller prints the usage and exits 0.
    Help,
    /// What is wrong, printed before the usage.
    Bad(String),
}

/// The values of every flag of a set, the given ones over the defaults, and the words after the flags.
#[derive(Debug, Clone)]
pub struct Parsed {
    values: BTreeMap<&'static str, Value>,
    pub rest: Vec<String>,
}

impl Parsed {
    pub fn str(&self, name: &str) -> String {
        match self.values.get(name) {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        }
    }

    pub fn bool(&self, name: &str) -> bool {
        matches!(self.values.get(name), Some(Value::Bool(true)))
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.values.get(name) {
            Some(Value::Int(n)) => *n,
            _ => 0,
        }
    }

    pub fn duration(&self, name: &str) -> Duration {
        match self.values.get(name) {
            Some(Value::Duration(d)) => *d,
            _ => Duration::ZERO,
        }
    }
}

/// Reads args (the words after the program's name) against the flags defs.
pub fn parse(defs: &[Flag], args: &[String]) -> Result<Parsed, Error> {
    let mut values = BTreeMap::new();
    for d in defs {
        let v = value_of(d, &d.default).map_err(|e| Error::Bad(format!("default of -{}: {e}", d.name)))?;
        values.insert(d.name, v);
    }
    let mut i = 0_usize;
    while let Some(arg) = args.get(i) {
        let Some(body) = arg.strip_prefix('-').filter(|b| !b.is_empty()) else {
            break; // a word that is not a flag ends them
        };
        i = i.saturating_add(1);
        let body = match body.strip_prefix('-') {
            Some("") => break, // "--" ends the flags and is consumed
            Some(b) => b,
            None => body,
        };
        if body.starts_with('-') || body.starts_with('=') {
            return Err(Error::Bad(format!("bad flag syntax: {arg}")));
        }
        let (name, inline) = match body.split_once('=') {
            Some((n, v)) => (n, Some(v.to_owned())),
            None => (body, None),
        };
        let Some(def) = defs.iter().find(|d| d.name == name) else {
            if name == "h" || name == "help" {
                return Err(Error::Help);
            }
            return Err(Error::Bad(format!("flag provided but not defined: -{name}")));
        };
        let text = match (inline, def.kind) {
            (Some(v), _) => v,
            (None, Kind::Bool) => "true".to_owned(),
            (None, _) => {
                let Some(next) = args.get(i) else {
                    return Err(Error::Bad(format!("flag needs an argument: -{name}")));
                };
                i = i.saturating_add(1);
                next.clone()
            }
        };
        let v =
            value_of(def, &text).map_err(|e| Error::Bad(format!("invalid value {text:?} for flag -{name}: {e}")))?;
        values.insert(def.name, v);
    }
    Ok(Parsed {
        values,
        rest: args.get(i..).unwrap_or_default().to_vec(),
    })
}

fn value_of(d: &Flag, text: &str) -> Result<Value, String> {
    Ok(match d.kind {
        Kind::Str => Value::Str(text.to_owned()),
        Kind::Bool => Value::Bool(parse_bool(text)?),
        Kind::Int => Value::Int(text.parse::<i64>().map_err(|_| "parse error".to_owned())?),
        Kind::Duration => Value::Duration(parse_duration(text)?),
    })
}

/// A bool: `1`, `t`, `true` and their capitals are true; `0`, `f`, `false` and theirs are false.
fn parse_bool(s: &str) -> Result<bool, String> {
    match s {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Ok(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Ok(false),
        _ => Err("parse error".to_owned()),
    }
}

/// A duration as decimals with units (`1h30m`, `1.5s`, `250ms`; `ns`, `us`, `ms`, `s`, `m`, `h`), or `0`; no negative
/// duration (nothing in Remit waits a negative time).
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let bad = || format!("time: invalid duration {s:?}");
    let s = s.strip_prefix('+').unwrap_or(s);
    if s == "0" {
        return Ok(Duration::ZERO);
    }
    if s.is_empty() || s.starts_with('-') {
        return Err(bad());
    }
    let mut total = Duration::ZERO;
    let mut rest = s;
    while !rest.is_empty() {
        let num_len = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .ok_or_else(|| format!("time: missing unit in duration {s:?}"))?;
        let (num, after) = rest.split_at(num_len);
        let unit_len = after
            .find(|c: char| c.is_ascii_digit() || c == '.')
            .unwrap_or(after.len());
        let (unit, next) = after.split_at(unit_len);
        let nanos: f64 = match unit {
            "ns" => 1.0,
            "us" | "µs" | "μs" => 1e3,
            "ms" => 1e6,
            "s" => 1e9,
            "m" => 60e9,
            "h" => 3600e9,
            "" => return Err(format!("time: missing unit in duration {s:?}")),
            _ => return Err(format!("time: unknown unit {unit:?} in duration {s:?}")),
        };
        if num.is_empty() || num == "." {
            return Err(bad());
        }
        let n: f64 = num.parse().map_err(|_| bad())?;
        let part = Duration::try_from_secs_f64(n * nanos / 1e9).map_err(|_| bad())?;
        total = total.checked_add(part).ok_or_else(bad)?;
        rest = next;
    }
    Ok(total)
}

/// The usage, flag by flag in name order.
pub fn usage(defs: &[Flag]) -> String {
    let mut sorted: Vec<&Flag> = defs.iter().collect();
    sorted.sort_by_key(|d| d.name);
    let mut out = String::new();
    for d in sorted {
        let kind = match d.kind {
            Kind::Str => " string",
            Kind::Bool => "",
            Kind::Int => " int",
            Kind::Duration => " duration",
        };
        let zero = matches!(d.default.as_str(), "" | "0" | "false" | "0s");
        let default = if zero {
            String::new()
        } else if d.kind == Kind::Str {
            format!(" (default {:?})", d.default)
        } else {
            format!(" (default {})", d.default)
        };
        writeln!(out, "  -{}{kind}\n    \t{}{default}", d.name, d.usage).ok();
    }
    out
}

/// The whole seconds a default holds, written as a duration: "30m0s", "168h0m0s", "0s".
pub fn duration_text(d: Duration) -> String {
    let secs = d.as_secs();
    if secs == 0 {
        return "0s".to_owned();
    }
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h{m}m{s}s")
    } else if m > 0 {
        format!("{m}m{s}s")
    } else {
        format!("{s}s")
    }
}
