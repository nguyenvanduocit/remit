//! The rule a chain's landing passes before it reaches users (ADR-0050, ADR-0114): a change goes live in stages, and a
//! stage lands only when the one before it on its repository has baked and what guards it holds. It is pure: it stores
//! nothing, lands nothing and reads no signal, so the queue hands it the previous stage and the answer of a probe, and
//! tests run it without a mock. The plan is the landing's (`model::StagePlan`).

use model::{Duration, SignalKey, StagePlan, Time};
use time::format_description::FormatItem;
use time::macros::format_description;

/// The signal that says a stage hurts and the condition on its reading that says so (the breach), watched while the stage
/// bakes. A guardrail no reading proves is none (ADR-0045).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guardrail {
    pub key: SignalKey,
    pub breach: String,
}

impl Guardrail {
    /// The guardrail plan declares.
    pub fn of(plan: &StagePlan) -> Self {
        Self {
            key: plan.guardrail.clone(),
            breach: plan.breach.clone(),
        }
    }
}

/// A plan that landed: what the next landing of a stage on its repository is judged against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stage {
    /// The chain whose change it carries, whose Chair watches its guardrail.
    pub by: String,
    pub exposure: String,
    pub guardrail: Guardrail,
    pub rollback: String,
    pub undo: bool,
    /// The landing it is.
    pub landing: String,
    pub at: Time,
    pub bake_until: Time,
}

impl Stage {
    /// The stage plan became when the landing `landing` of chain `by` landed at `at`.
    pub fn landed(plan: &StagePlan, by: &str, landing: &str, at: Time) -> Self {
        Self {
            by: by.to_owned(),
            exposure: plan.exposure.clone(),
            guardrail: Guardrail::of(plan),
            rollback: plan.rollback.clone(),
            undo: plan.undo,
            landing: landing.to_owned(),
            at,
            bake_until: bake_end(plan.bake, at),
        }
    }
}

/// When a stage that landed at `at` and bakes for `bake` ends its bake.
pub fn bake_end(bake: Duration, at: Time) -> Time {
    Time(at.0.saturating_add(time::Duration::nanoseconds(bake.0)))
}

/// Why a plan may not land now, written for the Chair that has to act on it. `E` is why a guardrail's reading could not
/// be had.
#[derive(Debug, thiserror::Error)]
pub enum Refusal<E: std::error::Error + 'static> {
    #[error(
        "say who this landing reaches (exposure): \"0%, dark behind flag X\" for the first stage of a change, then the share it grows to"
    )]
    NoExposure,
    #[error("say how this stage is taken back (rollback): the flag value to set, or the revert")]
    NoRollback,
    #[error("name the guardrail: a signal key and the condition on its reading that means this stage hurts")]
    NoGuardrail,
    #[error("say how long this stage bakes before the next may land (bake_minutes)")]
    NoBake,
    #[error(
        "the stage landed at {} ({exposure}) is still baking until {}: watch it, then land the next",
        rfc3339(at),
        rfc3339(until)
    )]
    Baking { at: Time, exposure: String, until: Time },
    #[error(
        "undo takes back the stage that is baking or whose guardrail is breached, and no stage on this repository is: \
         land it as a stage of its own, with its exposure, guardrail, bake and rollback"
    )]
    NothingToUndo,
    #[error("guardrail {} cannot be read, so it guards nothing: {source}", key.as_str())]
    Unreadable { key: SignalKey, source: E },
    #[error(
        "guardrail {} is breached now ({breach}): take the stage back with a landing that sets undo, or revert_landing, or fix the cause, before going on",
        key.as_str()
    )]
    Breached { key: SignalKey, breach: String },
}

/// Whether `p` may land now: `Ok`, or the reason. `last` is the stage landed before on the repository, none for the first.
/// `breached` says whether a guardrail's newest reading meets its breach condition now, or why that cannot be said. A way
/// back lands past the bake and the guardrails, as taking a stage back is what they are for, but only while there is one
/// to take back: the last stage, not itself a way back, still baking or breached. Else undo would be a door past both.
pub fn vet<E, F>(p: &StagePlan, last: Option<&Stage>, now: Time, mut breached: F) -> Result<(), Refusal<E>>
where
    E: std::error::Error + 'static,
    F: FnMut(&Guardrail) -> Result<bool, E>,
{
    if p.exposure.trim().is_empty() {
        return Err(Refusal::NoExposure);
    }
    if p.undo {
        let Some(last) = last.filter(|l| !l.undo) else {
            return Err(Refusal::NothingToUndo);
        };
        if now < last.bake_until {
            return Ok(());
        }
        return match breached(&last.guardrail) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Refusal::NothingToUndo),
            Err(source) => Err(Refusal::Unreadable {
                key: last.guardrail.key.clone(),
                source,
            }),
        };
    }
    if p.rollback.trim().is_empty() {
        return Err(Refusal::NoRollback);
    }
    if p.guardrail.as_str().is_empty() || p.breach.trim().is_empty() {
        return Err(Refusal::NoGuardrail);
    }
    if p.bake <= Duration::ZERO {
        return Err(Refusal::NoBake);
    }
    let own = Guardrail::of(p);
    let mut guards = vec![&own];
    if let Some(last) = last.filter(|l| !l.undo) {
        if now < last.bake_until {
            return Err(Refusal::Baking {
                at: last.at,
                exposure: last.exposure.clone(),
                until: last.bake_until,
            });
        }
        if last.guardrail != own {
            guards.push(&last.guardrail);
        }
    }
    for g in guards {
        match breached(g) {
            Err(source) => {
                return Err(Refusal::Unreadable {
                    key: g.key.clone(),
                    source,
                });
            }
            Ok(true) => {
                return Err(Refusal::Breached {
                    key: g.key.clone(),
                    breach: g.breach.clone(),
                });
            }
            Ok(false) => {}
        }
    }
    Ok(())
}

const SECONDS: &[FormatItem<'_>] = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
const OFFSET: &[FormatItem<'_>] = format_description!("[offset_hour sign:mandatory]:[offset_minute]");

/// `t` as RFC 3339 to the second: `Z` for UTC and the offset otherwise.
fn rfc3339(t: &Time) -> String {
    let offset = if t.0.offset().is_utc() {
        Ok("Z".to_owned())
    } else {
        t.0.format(OFFSET)
    };
    match (t.0.format(SECONDS), offset) {
        (Ok(seconds), Ok(offset)) => seconds + &offset,
        // Only a year past four digits fails to format: say the instant some way rather than hide it.
        _ => t.0.to_string(),
    }
}
