//! The dry run of a subscription over the readings it would have judged (docs/adr/0064).

use model::{Control, Reading, Subscription, Time};

use crate::evaluate::{ChainFacts, Effect, Input, apply, evaluate, unjudged};
use crate::instant;

/// One evaluation of a replay: the time it ran at and what `evaluate` said then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub at: Time,
    pub effects: Vec<Effect>,
}

/// What `s` would have done over `readings` had it judged each as it came, up to `until`: `evaluate` at the time of each
/// reading and at each instant `s` said it was due (its schedule, a flow's release, its deadline), each evaluation's
/// effects applied before the next, at most `most` evaluations. It answers the evaluations that said anything and `s`
/// where they leave it. Nothing is done, so a firing counts as done at once and overlap holds none back (none waits, no
/// run is at work), and no hold comes. `s` judges as if it were active and healthy, from its own cursor and due time;
/// `chain` is its chain's facts as the caller can read them, now.
pub fn replay(
    mut s: Subscription,
    readings: &[Reading],
    chain: Option<&ChainFacts>,
    until: Time,
    most: usize,
) -> (Vec<Step>, Subscription) {
    s.control = Control::Active;
    s.hold = None;
    let readings = unjudged(&s, readings);
    let mut steps = Vec::new();
    // readings[from..arrived] are the ones come and not judged.
    let (mut from, mut arrived) = (0usize, 0usize);
    let mut last = None;
    for _ in 0..most {
        let Some(at) = next_instant(&s, &readings, arrived, last, until) else {
            break;
        };
        while readings.get(arrived).is_some_and(|r| r.at <= at) {
            arrived = arrived.saturating_add(1);
        }
        let effects = evaluate(&Input {
            sub: &s,
            readings: readings.get(from..arrived).unwrap_or_default(),
            chain,
            pending: 0,
            running: false,
            screens: &[],
            now: at,
        });
        s = apply(s, &effects);
        if !effects.is_empty() {
            steps.push(Step { at, effects });
        }
        while from < arrived && readings.get(from).is_some_and(|r| r.seq <= s.seen_seq) {
            from = from.saturating_add(1);
        }
        if s.control == Control::Ended {
            break;
        }
        last = Some(at);
    }
    (steps, s)
}

/// The instant after `last` that `s` next judges at, by `until`: the time the next of `readings` comes (`arrived` is the
/// first not come), or `s`'s due time, whichever is first. A reading stored with a time before `last` comes at `last`.
/// `None` when nothing comes by `until`, or nothing new would be judged: a due time that has passed and no reading to come.
fn next_instant(
    s: &Subscription,
    readings: &[Reading],
    arrived: usize,
    last: Option<Time>,
    until: Time,
) -> Option<Time> {
    let comes = readings.get(arrived).map(|r| last.map_or(r.at, |last| r.at.max(last)));
    let due = s.due().filter(|due| last.is_none_or(|last| *due > last));
    instant::earliest(comes, due).filter(|at| *at <= until)
}
