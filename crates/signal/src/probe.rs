//! The dry run of a measure's condition (ADR-0045).

use std::fmt;

use cel::Value;
use model::{Reading, Time};

use crate::ChainFacts;
use crate::check::Kind;
use crate::condition::{Activation, ExprError, empty, facts_of, parse, shown};
use crate::instant;

/// Why a measure's condition cannot be dry-run, or what it failed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    NoCondition,
    /// Too long or not compiling.
    Condition(ExprError),
    NotBool {
        gives: String,
    },
    ReadsNothing,
    Unreadable {
        made: String,
        error: ExprError,
    },
    Gave(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::NoCondition => f.write_str("a measure needs a condition: what the signal's value must satisfy"),
            ProbeError::Condition(e) => e.fmt(f),
            ProbeError::NotBool { gives } => write!(f, "condition gives {gives}, and a condition gives true or false"),
            ProbeError::ReadsNothing => f.write_str(
                "condition reads nothing from the signal: it must use its value or body, or it holds whatever the signal says",
            ),
            ProbeError::Unreadable { made, error } => {
                write!(f, "condition cannot be read on the reading made {made}: {error}")
            }
            ProbeError::Gave(v) => write!(f, "condition gave {v} on the reading, and a condition gives true or false"),
        }
    }
}

impl std::error::Error for ProbeError {}

/// `probe` runs `expr` on `r`, a reading the signal really made, and says whether it holds. `evaluate` answers false for a
/// condition that fails to run, so that one subscription's mistake stops no other; a measure is the opposite case, where a
/// mistyped field must not pass for a measure that is not met, so `probe` returns the reason. The condition must be
/// present, compile, give true or false, and read the signal's value or body: a condition that reads only the clock or a
/// constant measures nothing.
pub fn probe(expr: &str, r: &Reading, now: Time) -> Result<bool, ProbeError> {
    probe_on(expr, &facts_of(r).activation("", &ChainFacts::default(), now), r.at)
}

pub(crate) fn probe_on(expr: &str, act: &Activation, made: Time) -> Result<bool, ProbeError> {
    if empty(expr) {
        return Err(ProbeError::NoCondition);
    }
    let prg = parse(expr, "condition").map_err(ProbeError::Condition)?;
    if !matches!(prg.gives.kind(), Kind::Dyn | Kind::Bool) {
        return Err(ProbeError::NotBool {
            gives: prg.gives.to_string(),
        });
    }
    if !prg.reads_signal {
        return Err(ProbeError::ReadsNothing);
    }
    match prg.run(act) {
        Ok(Value::Bool(held)) => Ok(held),
        Ok(other) => Err(ProbeError::Gave(shown(&other))),
        Err(error) => Err(ProbeError::Unreadable {
            made: instant::rfc3339(made),
            error,
        }),
    }
}
