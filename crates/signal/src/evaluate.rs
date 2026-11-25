//! Given one subscription, the readings it has not judged and the time, `evaluate` says what the subscription does, as a
//! closed set of effects (docs/adr/0014, docs/adr/0064).

use model::{
    ChainStatus, Control, Firing, FiringCause, FlowPolicy, HoldReason, Overlap, OwnerKind, Reading, SignalKey,
    SkipReason, Subscription, Time, schedule_key,
};

use crate::condition::{Conditions, empty, facts_of};
use crate::instant::{self, earliest};
use crate::schedule::schedule_next;
use crate::screen::{Screen, screen_fires};

/// Everything one evaluation of one subscription reads.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    pub sub: &'a Subscription,
    /// The stored readings `sub.match` lets through past `sub.seen_seq`. Any other passed is left out, so the caller may
    /// pass more; any order.
    pub readings: &'a [Reading],
    /// The facts of the chain that owns `sub`; `None` for none (a subscription of Remit, or a chain that is gone).
    pub chain: Option<&'a ChainFacts>,
    /// How many firings of `sub` are recorded and not yet done; `running` says a chain `sub` started is not done. Both
    /// are read by the overlap policy only.
    pub pending: usize,
    pub running: bool,
    /// What a screen made of the firings the subscription would make (`asks`): empty for a subscription with none.
    pub screens: &'a [Screen],
    pub now: Time,
}

/// What a condition reads of the chain that owns its subscription.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChainFacts {
    pub marks: Vec<String>,
    pub status: ChainStatus,
}

/// One thing an evaluation says of a subscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Fire(Box<Fire>),
    Skip(Skip),
    Hold(Hold),
    /// The next instant the subscription acts with no new reading: its schedule's next instant, a flow's release or its
    /// deadline. None says it is due no more; it is given only to a subscription that was due.
    Due(Option<Time>),
    /// Moves the subscription's cursor to the seq: every reading up to it is judged.
    Advance(i64),
    /// The subscription is over: its deadline came, or its schedule acted at its last instant. Applied, it is ended.
    End,
}

/// A firing to record and do: its key (subscription, key, rev) is the same on every retry, so a firing recorded twice is
/// recorded once. `readings` are the ones it covers, ascending; none for an expiry. Owner, action and outcome are the
/// shell's to fill in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fire {
    pub firing: Firing,
    pub readings: Vec<Reading>,
}

/// A reading judged that fires nothing, and why: recorded as a near-miss, and counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    pub seq: i64,
    pub key: SignalKey,
    pub reason: SkipReason,
    /// For condition_error, the error; for screened, how sure the decider was.
    pub detail: String,
    /// For screened, the judgment that screened the reading out; empty for any other.
    pub judgment: String,
}

/// A firing that failed for good holding its subscription in place (`settle`): it does nothing, and its cursor stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hold {
    pub reason: HoldReason,
    pub detail: String,
}

/// `evaluate` says what `input.sub` does at `input.now` about `input.readings`. It takes its steps in this order, and the
/// first that settles a reading settles it:
///
///  1. control: a paused or ended subscription does nothing;
///  2. health: a held one does nothing, and its cursor stays, until its owner retries or skips the firing it is held on;
///  3. key match and 4. cursor: only the readings its pattern lets through past its `seen_seq` are judged;
///  5. own chain: a reading the owning chain caused is skipped (own_chain), so a chain's records never wake it;
///  6. condition: a reading its condition is false on, or cannot be read on, is skipped (condition_false,
///     condition_error);
///  7. flow: the readings that hold fire as the flow policy lets them now; the first one it holds back, and every reading
///     after it whatever it is, waits unjudged, and `Due` says when the flow lets it go;
///  8. overlap: under buffer_one, a firing past the one that may wait is skipped (overlap);
///  9. fire.
///
/// A subscription that keeps a schedule has no readings: in place of steps 3 to 7 its instant (`due_at`) comes or has
/// not come, and one that came longer ago than its catch-up window is skipped (missed_catchup); then overlap, then fire,
/// caused scheduled, under its schedule key at the instant's rev in Unix milliseconds. It is then due at its next instant
/// counted from now, so one firing catches up whatever the server missed, or it ends.
///
/// A deadline (`until`) is not spent by a firing on a reading. When it comes the subscription ends, the readings the flow
/// holds go at once, and when nothing else fires it fires once, caused by the expiry, at the deadline's instant (its rev,
/// in Unix milliseconds). The effects come in the order of the log, each fire at the place of its newest reading, then
/// advance, end and due: applied in order (`apply`) they move the subscription as recording each firing moves it.
pub fn evaluate(input: &Input<'_>) -> Vec<Effect> {
    let s = input.sub;
    if s.control != Control::Active || s.hold.is_some() {
        return Vec::new();
    }
    if s.schedule.is_some() {
        return scheduled(s, input);
    }
    let Heard {
        readings,
        mut placed,
        flow: f,
        deadline,
    } = heard_and_flowed(s, input);
    let Flowed {
        fires,
        collapsed,
        cut: flow_cut,
        due,
    } = f;
    // A screen judges the firings before overlap places them: one it drops frees its place for the next.
    let screened = screen_fires(fires, input.screens);
    placed.extend(screened.skips);
    let cut = [flow_cut, screened.cut]
        .into_iter()
        .filter(|c| *c > 0)
        .min()
        .unwrap_or_default();
    for fire in overlap(s, screened.fires, input, &mut placed) {
        let at = fire.firing.seqs.last().copied().unwrap_or_default();
        placed.push((at, Effect::Fire(Box::new(fire))));
    }
    placed.extend(collapsed);
    if cut > 0 {
        placed.retain(|(at, _)| *at < cut);
    }
    placed.sort_by_key(|(at, _)| *at);

    let fired = placed.iter().any(|(_, e)| matches!(e, Effect::Fire(_)));
    let mut out: Vec<Effect> = placed.into_iter().map(|(_, e)| e).collect();
    let to = judged_to(&readings, cut);
    if to > s.seen_seq {
        out.push(Effect::Advance(to));
    }
    // a deadline waits for the firings a screen deferred: they are asked next pass, and it ends the subscription then
    if let Some(until) = deadline.filter(|_| screened.cut == 0) {
        if !fired {
            // The deadline is the instant, in Unix milliseconds: it is the same on every retry and no reading's revision
            // comes near it.
            out.push(Effect::Fire(Box::new(Fire {
                firing: Firing {
                    subscription_id: s.id.clone(),
                    key: SignalKey(s.r#match.0.clone()),
                    rev: instant::unix_milli(until),
                    at: input.now,
                    cause: FiringCause::Expired,
                    ..Firing::default()
                },
                readings: Vec::new(),
            })));
        }
        out.push(Effect::End);
    }
    let at = earliest(due, until_ahead(s, input.now));
    if at.is_some() || s.due_at.is_some() && deadline.is_none() {
        out.push(Effect::Due(at));
    }
    out
}

/// The readings `s` judges, those among them it hears, the skips of the rest, what its flow makes of what it hears at
/// `input.now`, and the deadline that has come, if one has.
struct Heard {
    readings: Vec<Reading>,
    placed: Vec<(i64, Effect)>,
    flow: Flowed,
    deadline: Option<Time>,
}

fn heard_and_flowed(s: &Subscription, input: &Input<'_>) -> Heard {
    let readings = unjudged(s, input.readings);
    let mut placed: Vec<(i64, Effect)> = Vec::new();
    let mut heard = Vec::new();
    let mut conds = Conditions::default();
    for r in &readings {
        match hears(s, r, input, &mut conds) {
            Some(skip) => placed.push((r.seq, Effect::Skip(skip))),
            None => heard.push(r.clone()),
        }
    }
    let deadline = s.until.filter(|until| input.now >= *until);
    let f = flow_of(s, &heard, input.now, deadline.is_some());
    Heard {
        readings,
        placed,
        flow: f,
        deadline,
    }
}

/// The reading each firing the subscription would make is on, before a screen and overlap, in the order they fire: what
/// a screen is asked about. A firing of an expiry covers none, and a subscription that keeps a schedule has none.
pub fn asks(input: &Input<'_>) -> Vec<Reading> {
    let s = input.sub;
    if s.control != Control::Active || s.hold.is_some() || s.schedule.is_some() {
        return Vec::new();
    }
    let heard = heard_and_flowed(s, input);
    heard
        .flow
        .fires
        .into_iter()
        .filter_map(|fire| fire.readings.last().cloned())
        .collect()
}

/// What `s`, which keeps a schedule, does at `input.now`: before its instant, nothing but say when it comes; at or after
/// it, the instant fires, or is skipped when it came longer ago than the catch-up window (missed_catchup) or when overlap
/// lets no more firings wait (overlap); then `s` is due at its next instant, counted from now, or it ends.
fn scheduled(s: &Subscription, input: &Input<'_>) -> Vec<Effect> {
    let (Some(sch), Some(instant)) = (&s.schedule, s.due_at) else {
        return Vec::new();
    };
    if input.now < instant {
        return vec![Effect::Due(Some(instant))];
    }
    let key = schedule_key(&s.id);
    let late = instant::since(input.now, instant);
    let mut out = Vec::new();
    if sch.catchup_window.0 > 0 && late > sch.catchup_window {
        out.push(Effect::Skip(Skip {
            seq: 0,
            key,
            reason: SkipReason::MissedCatchup,
            detail: format!(
                "the instant {} came {} late, past the catch-up window of {}",
                instant::rfc3339(instant),
                instant::span_text(instant::round_second(late)),
                instant::span_text(sch.catchup_window)
            ),
            judgment: String::new(),
        }));
    } else if s.overlap == Overlap::BufferOne && buffer_room(input) == 0 {
        out.push(Effect::Skip(Skip {
            seq: 0,
            key,
            reason: SkipReason::Overlap,
            detail: String::new(),
            judgment: String::new(),
        }));
    } else {
        out.push(Effect::Fire(Box::new(Fire {
            firing: Firing {
                subscription_id: s.id.clone(),
                key,
                rev: instant::unix_milli(instant),
                at: input.now,
                cause: FiringCause::Scheduled,
                ..Firing::default()
            },
            readings: Vec::new(),
        })));
    }
    out.push(schedule_next(sch, input.now).map_or(Effect::End, |at| Effect::Due(Some(at))));
    out
}

/// `s` moved by `effects`, in order: the one place a subscription's state moves. A fire judges its readings and is its
/// last firing, and its schedule's when the instant fired it; a skip judges up to its place and is counted; an advance
/// judges up to its place; due is its next due time; end ends it and clears its deadline and its due time. Hold moves
/// nothing it stores: a hold is settled (`settle`).
pub fn apply(mut s: Subscription, effects: &[Effect]) -> Subscription {
    for e in effects {
        match e {
            Effect::Fire(fire) => {
                s.last_fired = Some(fire.firing.at);
                if let Some(last) = fire.firing.seqs.last() {
                    s.seen_seq = s.seen_seq.max(*last);
                }
                if fire.firing.cause == FiringCause::Scheduled
                    && let Some(sch) = &mut s.schedule
                {
                    sch.last = Some(fire.firing.at);
                }
            }
            Effect::Skip(skip) => {
                s.seen_seq = s.seen_seq.max(skip.seq);
                s.tally.count(&skip.reason);
            }
            Effect::Advance(seq) => s.seen_seq = s.seen_seq.max(*seq),
            Effect::Due(at) => s.due_at = *at,
            Effect::End => {
                s.control = Control::Ended;
                s.until = None;
                s.due_at = None;
            }
            Effect::Hold(_) => {}
        }
    }
    s
}

/// The readings `s` judges: the ones its pattern lets through past its cursor, ascending by seq.
pub(crate) fn unjudged(s: &Subscription, rs: &[Reading]) -> Vec<Reading> {
    let mut out: Vec<Reading> = rs
        .iter()
        .filter(|r| r.seq > s.seen_seq && s.r#match.matches(&r.key))
        .cloned()
        .collect();
    out.sort_by_key(|r| r.seq);
    out
}

/// `None` when `s` takes `r` as its own to judge: not what its own chain caused (the reinforcing loop ADR-0014 damps), and
/// its condition holds. Otherwise the skip that judges `r`.
fn hears(s: &Subscription, r: &Reading, input: &Input<'_>, conds: &mut Conditions) -> Option<Skip> {
    let chain_id = if s.owner.kind == OwnerKind::Chain {
        s.owner.id.as_str()
    } else {
        ""
    };
    let skip = |reason, detail| {
        Some(Skip {
            seq: r.seq,
            key: r.key.clone(),
            reason,
            detail,
            judgment: String::new(),
        })
    };
    if !chain_id.is_empty() && r.cause.chain_id == chain_id {
        return skip(SkipReason::OwnChain, String::new());
    }
    if empty(&s.expr) {
        return None;
    }
    let chain = input.chain.cloned().unwrap_or_default();
    match conds.holds(&s.expr, &facts_of(r).activation(chain_id, &chain, input.now)) {
        Err(e) => skip(SkipReason::ConditionError, e.to_string()),
        Ok(false) => skip(SkipReason::ConditionFalse, String::new()),
        Ok(true) => None,
    }
}

/// What the flow policy makes of the readings heard at an instant.
#[derive(Default)]
struct Flowed {
    fires: Vec<Fire>,
    /// The readings flow last fired a newer one in place of.
    collapsed: Vec<(i64, Effect)>,
    /// The seq of the first reading held back: it and every reading after it wait unjudged; zero when none.
    cut: i64,
    /// When what is held back goes; none when nothing is.
    due: Option<Time>,
}

impl Flowed {
    fn hold(mut self, r: &Reading, at: Time) -> Flowed {
        self.cut = r.seq;
        self.due = Some(at);
        self
    }
}

/// Fires the readings heard as `s`'s flow lets them go at `now`. `forced` is a deadline that has come: it lets everything
/// go.
fn flow_of(s: &Subscription, heard: &[Reading], now: Time, forced: bool) -> Flowed {
    let mut f = Flowed::default();
    let (Some(first), Some(newest)) = (heard.first(), heard.last()) else {
        return f;
    };
    let period = s.flow.period;
    match s.flow.policy {
        FlowPolicy::Delay => {
            let mut last = s.last_fired;
            for r in heard {
                if let Some(ready) = last.map(|last| instant::add(last, period))
                    && !forced
                    && now < ready
                {
                    return f.hold(r, ready);
                }
                f.fires.push(fire_of(s, std::slice::from_ref(r), now));
                last = Some(now);
            }
        }
        FlowPolicy::Batch => {
            let mut release = first.at;
            if let Some(after_last) = s.last_fired.map(|last| instant::add(last, period))
                && after_last > release
            {
                release = after_last;
            }
            if !forced && now < release {
                return f.hold(first, release);
            }
            let groups = group_of(heard);
            match groups.as_slice() {
                [first_group, second, ..] if !forced => {
                    // One release a period: the first key and subject goes, with its readings before the first of any
                    // other, which waits.
                    let Some(next) = second.first() else { return f };
                    let before: Vec<Reading> = first_group.iter().filter(|r| r.seq < next.seq).cloned().collect();
                    f.fires.push(fire_of(s, &before, now));
                    return f.hold(next, instant::add(now, period));
                }
                _ => f.fires.extend(groups.iter().map(|g| fire_of(s, g, now))),
            }
        }
        FlowPolicy::Last => {
            let longest = if s.flow.max_wait.0 <= 0 {
                period
            } else {
                s.flow.max_wait
            };
            let release = instant::add(newest.at, period).min(instant::add(first.at, longest));
            if !forced && now < release {
                return f.hold(first, release);
            }
            for g in group_of(heard) {
                let Some((latest, older)) = g.split_last() else {
                    continue;
                };
                for r in older {
                    f.collapsed.push((
                        r.seq,
                        Effect::Skip(Skip {
                            seq: r.seq,
                            key: r.key.clone(),
                            reason: SkipReason::Collapsed,
                            detail: String::new(),
                            judgment: String::new(),
                        }),
                    ));
                }
                f.fires.push(fire_of(s, std::slice::from_ref(latest), now));
            }
        }
        _ => f
            .fires
            .extend(heard.iter().map(|r| fire_of(s, std::slice::from_ref(r), now))),
    }
    f
}

/// The fires `s`'s overlap policy lets through, with a skip placed for each reading of the ones it does not. Under
/// buffer_one two firings may be at once, the one at work and one that waits: what is at work or waits already takes its
/// place.
fn overlap(s: &Subscription, mut fires: Vec<Fire>, input: &Input<'_>, placed: &mut Vec<(i64, Effect)>) -> Vec<Fire> {
    if s.overlap != Overlap::BufferOne {
        return fires;
    }
    let room = buffer_room(input);
    if fires.len() <= room {
        return fires;
    }
    for fire in fires.split_off(room) {
        for r in fire.readings {
            let skip = Skip {
                seq: r.seq,
                key: r.key,
                reason: SkipReason::Overlap,
                detail: String::new(),
                judgment: String::new(),
            };
            placed.push((r.seq, Effect::Skip(skip)));
        }
    }
    fires
}

/// How many firings buffer_one lets be recorded now: two may be at once, the one at work and one that waits, and what is
/// at work or waits already takes its place.
fn buffer_room(input: &Input<'_>) -> usize {
    2usize
        .saturating_sub(input.pending)
        .saturating_sub(usize::from(input.running))
}

/// `rs` split into the readings of each key and subject, each ascending, the groups in the order of their first reading.
fn group_of(rs: &[Reading]) -> Vec<Vec<Reading>> {
    let mut groups: Vec<Vec<Reading>> = Vec::new();
    for r in rs {
        match groups
            .iter_mut()
            .find(|g| g.first().is_some_and(|x| x.key == r.key && x.subject == r.subject))
        {
            Some(g) => g.push(r.clone()),
            None => groups.push(vec![r.clone()]),
        }
    }
    groups
}

/// The firing of `rs`, readings of one key ascending: its revision is the newest's, and it covers each of their places.
/// Every caller passes at least one reading.
fn fire_of(s: &Subscription, rs: &[Reading], now: Time) -> Fire {
    let (key, rev) = rs.last().map(|n| (n.key.clone(), n.rev)).unwrap_or_default();
    Fire {
        firing: Firing {
            subscription_id: s.id.clone(),
            key,
            rev,
            seqs: rs.iter().map(|r| r.seq).collect(),
            at: now,
            cause: FiringCause::Reading,
            ..Firing::default()
        },
        readings: rs.to_vec(),
    }
}

/// The seq of the last of `readings` before `cut` (all of them when `cut` is zero); zero when there is none.
fn judged_to(readings: &[Reading], cut: i64) -> i64 {
    readings
        .iter()
        .take_while(|r| cut == 0 || r.seq < cut)
        .last()
        .map_or(0, |r| r.seq)
}

/// `s`'s deadline while it has not come at `now`; none otherwise.
fn until_ahead(s: &Subscription, now: Time) -> Option<Time> {
    s.until.filter(|until| now < *until)
}
