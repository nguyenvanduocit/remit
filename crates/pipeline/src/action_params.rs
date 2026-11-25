//! What a tell, a start and a run read of the reading that fires them is a set of parameters, each an expression in the
//! condition's language (`signal::check`), by name in `SubscriptionAction::with`. The kind says which names it takes; a
//! name outside them, or an expression that does not compile or gives the wrong kind of value, is refused when the
//! subscription is made. One that runs and gives nothing on a reading (a field the event lacks) gives nothing, and the
//! effect says so on its firing.

use std::collections::BTreeMap;

use model::{ActionKind, Reading, SubscriptionAction, Time};

use crate::automation::{JOB_KINDS, job_takes_chain};
use chains::{Error, Result};

/// The chain the reading is about: marks (a list of text) or a chain's id.
pub(crate) const PARAM_CHAIN: &str = "chain";
/// What happened, in a line.
pub(crate) const PARAM_SUMMARY: &str = "summary";
/// Words of outsiders the reading carries: evidence, quoted.
pub(crate) const PARAM_QUOTE: &str = "quote";
/// Who wrote the quote.
pub(crate) const PARAM_AUTHOR: &str = "author";

/// The parameters kind takes.
fn params_of_kind(kind: &ActionKind) -> &'static [&'static str] {
    match kind {
        ActionKind::Tell => &[PARAM_CHAIN, PARAM_SUMMARY, PARAM_QUOTE, PARAM_AUTHOR],
        ActionKind::Start => &[PARAM_SUMMARY, PARAM_QUOTE, PARAM_AUTHOR],
        ActionKind::Run => &[PARAM_CHAIN, PARAM_SUMMARY],
        _ => &[],
    }
}

/// Whether a can be stored as the action of a subscription: it takes only the parameters its kind takes, each compiles,
/// a tell and a run of a job that acts on a chain say which chain, and a run names a job of the closed list.
pub(crate) fn check_action(a: &SubscriptionAction) -> Result<()> {
    let takes = params_of_kind(&a.kind);
    for (name, expr) in &a.with {
        if !takes.contains(&name.as_str()) {
            return Err(Error::refused(format!(
                "a {} does not take the parameter {name:?}",
                a.kind
            )));
        }
        let checked = if name == PARAM_CHAIN {
            signal::check_texts(expr)
        } else {
            signal::check_text(expr)
        };
        checked.map_err(|e| Error::refused(format!("{name}: {e}")))?;
    }
    let names_chain = a.with.get(PARAM_CHAIN).is_some_and(|e| !e.is_empty());
    if a.kind == ActionKind::Tell && !names_chain {
        return Err(Error::refused(
            "a tell needs a chain: say which chain the reading is about",
        ));
    }
    if a.kind == ActionKind::Run && !JOB_KINDS.contains(&a.job.as_str()) {
        return Err(Error::refused(format!(
            "a run does one of Remit's jobs ({JOB_KINDS:?}), not {:?}",
            a.job
        )));
    }
    if a.kind == ActionKind::Run && job_takes_chain(&a.job) && !names_chain {
        return Err(Error::refused(format!(
            "the job {} acts on a chain: say which chain the reading is about",
            a.job
        )));
    }
    Ok(())
}

/// What an action reads of one reading.
pub(crate) struct Params<'a> {
    with: &'a BTreeMap<String, String>,
    reading: &'a Reading,
    now: Time,
}

/// The parameters of action a, read of reading r at now.
pub(crate) fn params_of<'a>(a: &'a SubscriptionAction, r: &'a Reading, now: Time) -> Params<'a> {
    Params {
        with: &a.with,
        reading: r,
        now,
    }
}

impl Params<'_> {
    /// What parameter name gives as text; none when the action sets none or its expression gave nothing.
    pub(crate) fn text(&self, name: &str) -> Option<String> {
        signal::eval_text(self.with.get(name)?, self.reading, self.now)
    }

    /// What parameter name gives as a list of text; none when the action sets none or its expression gave nothing.
    pub(crate) fn marks(&self, name: &str) -> Option<Vec<String>> {
        signal::eval_texts(self.with.get(name)?, self.reading, self.now)
    }
}
