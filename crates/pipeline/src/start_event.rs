//! A start on an event (ADR-0036) begins a chain from the owner's words, as a routine's start does, and gives the chain
//! what the event says as evidence: after the goal, in a fence the sender's words cannot close. The title and the goal
//! are only what the owner wrote; what the sender wrote (summary, quote, author) never reaches them. A subscription starts
//! one chain at a time (`unfinished_run`), and at most `Config::event_chains_per_day` a day.

use model::{Chain, Reading, SignalKey, Subscription, Time};

use crate::action_params::{PARAM_AUTHOR, PARAM_QUOTE, PARAM_SUMMARY, params_of};
use crate::tell::EVIDENCE_NOTE;
use chains::quote::fenced;
use chains::schedule::local_day_start;
use chains::signal_sources::signal_key_of;

/// What follows the goal of a chain a start on an event begins: what happened and, when the subscription gives the
/// outsiders' words, those after it, in a fence. Empty for a start on a schedule, whose readings are none.
pub(crate) fn start_evidence(sub: &Subscription, readings: &[Reading], now: Time) -> String {
    let Some(r) = readings.first().filter(|_| sub.schedule.is_none()) else {
        return String::new();
    };
    let prm = params_of(&sub.action, r, now);
    let summary = prm.text(PARAM_SUMMARY).unwrap_or_default();
    let quote = prm.text(PARAM_QUOTE).unwrap_or_default();
    let author = prm.text(PARAM_AUTHOR).unwrap_or_default();
    if summary.is_empty() && quote.is_empty() {
        return String::new();
    }
    let mut body = summary;
    if !author.is_empty() {
        body.push_str(&format!(" (by {author})"));
    }
    if !quote.is_empty() {
        body.push_str(&format!(":\n{quote}"));
    }
    let key = SignalKey(sub.r#match.0.clone());
    let name = signal_key_of(&key).map(|k| k.name).unwrap_or_default();
    format!(
        "\n\n[event] This chain began from an event of {name:?}.{EVIDENCE_NOTE}\n\n{}",
        fenced(&body)
    )
}

/// Why subscription sub may start no more chains today: it has started that many among chains, and quota is the most it
/// may a day (0: no limit). None while it may. Whoever writes a signal cannot turn a burst of events into a burst of
/// chains: what is past the quota waits, kept rather than dropped, until tomorrow.
pub(crate) fn start_quota(sub: &Subscription, chains: &[Chain], now: Time, quota: usize) -> Option<String> {
    if quota == 0 {
        return None;
    }
    let today = local_day_start(now);
    let started = chains
        .iter()
        .filter(|c| c.routine_id == sub.id && local_day_start(Time(c.created_at.0.to_offset(now.0.offset()))) == today)
        .count();
    (started >= quota).then(|| {
        format!(
            "the subscription {} has started {started} today, the most it may: its events wait for tomorrow",
            sub.id
        )
    })
}
