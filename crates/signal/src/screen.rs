//! The screen of a subscription that starts a chain (ADR-0118, point 2), pure. `asks` says which readings the
//! subscription would fire on (flow applied, overlap not); the shell puts each to a decider and brings back a `Screen`
//! for it; `evaluate` applies them between flow and overlap, so a firing screened out frees its place in the overlap's
//! buffer for the next. A screened-out firing is skipped and nothing takes its place: the readings it covered are consumed
//! as if it had fired. A firing nobody answered for fires as it would with no screen, and one the shell did not get to
//! ask about this pass (`Verdict::Defer`) is left, with the cursor held before it, for the next pass to make again.

use model::SkipReason;

use crate::evaluate::{Effect, Fire, Skip};

/// What a decider's answer says of one firing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The firing goes on: the reading needs a chain, the decider was unsure, or the reading is in the audit.
    Keep,
    /// A decider sure of it says the reading needs no chain: the firing is dropped.
    Out,
    /// Not asked this pass: the firing and everything after it wait unjudged.
    Defer,
}

/// What the screen made of the firing whose reading is at `seq`.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub seq: i64,
    pub verdict: Verdict,
    /// The judgment it is recorded as, which what the chain the firing starts does is scored on; empty for `Defer`.
    pub judgment: String,
    /// How sure the decider was of its pick, from 0 to 1.
    pub p: f64,
}

/// The firings a screen leaves: those that go on, the skips of those dropped, and the seq of the first firing deferred
/// (zero when none), from which nothing is applied.
pub(crate) struct Screened {
    pub fires: Vec<Fire>,
    pub skips: Vec<(i64, Effect)>,
    pub cut: i64,
}

/// fires as screens leave them. One whose reading is screened out becomes a skip (screened), one kept goes on and carries
/// its judgment, one with no screen goes on as it was.
pub(crate) fn screen_fires(fires: Vec<Fire>, screens: &[Screen]) -> Screened {
    let of = |fire: &Fire| {
        let seq = fire.firing.seqs.last()?;
        screens.iter().find(|s| s.seq == *seq)
    };
    let cut = fires
        .iter()
        .filter(|f| of(f).is_some_and(|s| s.verdict == Verdict::Defer))
        .filter_map(|f| f.firing.seqs.first().copied())
        .min()
        .unwrap_or_default();
    let mut out = Screened {
        fires: Vec::with_capacity(fires.len()),
        skips: Vec::new(),
        cut,
    };
    for mut fire in fires {
        let screen = of(&fire).cloned();
        match screen {
            Some(s) if s.verdict == Verdict::Out => {
                let seq = fire.firing.seqs.last().copied().unwrap_or_default();
                out.skips.push((
                    seq,
                    Effect::Skip(Skip {
                        seq,
                        key: fire.firing.key,
                        reason: SkipReason::Screened,
                        detail: format!("{:.0}% sure it needs no chain", s.p * 100.0),
                        judgment: s.judgment,
                    }),
                ));
            }
            Some(s) if s.verdict == Verdict::Keep => {
                fire.firing.screen = s.judgment;
                out.fires.push(fire);
            }
            _ => out.fires.push(fire),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluate::{Input, evaluate};
    use model::{Control, Overlap, Reading, SignalKey, SignalPattern, Subscription, Time};

    fn sub(overlap: Overlap) -> Subscription {
        Subscription {
            id: "sub_a".to_owned(),
            r#match: SignalPattern("event:a".to_owned()),
            control: Control::Active,
            overlap,
            ..Subscription::default()
        }
    }

    fn reading(seq: i64) -> Reading {
        Reading {
            key: SignalKey("event:a".to_owned()),
            rev: seq,
            seq,
            ..Reading::default()
        }
    }

    fn screen(seq: i64, verdict: Verdict) -> Screen {
        Screen {
            seq,
            verdict,
            judgment: format!("ju_{seq}"),
            p: 0.96,
        }
    }

    fn run(s: &Subscription, readings: &[Reading], screens: &[Screen], running: bool) -> Vec<Effect> {
        evaluate(&Input {
            sub: s,
            readings,
            chain: None,
            pending: 0,
            running,
            screens,
            now: Time::default(),
        })
    }

    #[test]
    fn a_firing_screened_out_frees_its_place_so_the_real_one_behind_it_starts() {
        // room for one firing: a chain is at work and one may wait
        let s = sub(Overlap::BufferOne);
        let readings = [reading(1), reading(2)];
        let out = run(
            &s,
            &readings,
            &[screen(1, Verdict::Out), screen(2, Verdict::Keep)],
            true,
        );
        assert!(
            out.iter()
                .any(|e| matches!(e, Effect::Skip(k) if k.seq == 1 && k.reason == SkipReason::Screened)),
            "{out:?}"
        );
        assert!(
            out.iter()
                .any(|e| matches!(e, Effect::Fire(f) if f.firing.seqs == [2] && f.firing.screen == "ju_2")),
            "{out:?}"
        );
        assert!(
            !out.iter()
                .any(|e| matches!(e, Effect::Skip(k) if k.reason == SkipReason::Overlap)),
            "{out:?}"
        );
    }

    #[test]
    fn a_deferred_firing_holds_the_cursor_before_it_and_everything_after_it_waits() {
        let s = sub(Overlap::Queue);
        let readings = [reading(1), reading(2), reading(3)];
        let out = run(
            &s,
            &readings,
            &[
                screen(1, Verdict::Keep),
                screen(2, Verdict::Defer),
                screen(3, Verdict::Defer),
            ],
            false,
        );
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(matches!(&out[0], Effect::Fire(f) if f.firing.seqs == [1]));
        assert_eq!(
            out[1],
            Effect::Advance(1),
            "the cursor stops before the first deferred firing"
        );
    }

    #[test]
    fn asks_names_the_reading_of_each_firing_before_overlap() {
        let s = sub(Overlap::BufferOne);
        let readings = [reading(1), reading(2), reading(3)];
        let asked: Vec<i64> = crate::evaluate::asks(&Input {
            sub: &s,
            readings: &readings,
            chain: None,
            pending: 0,
            running: true,
            screens: &[],
            now: Time::default(),
        })
        .iter()
        .map(|r| r.seq)
        .collect();
        assert_eq!(asked, [1, 2, 3], "overlap has not yet dropped any");
    }
}
