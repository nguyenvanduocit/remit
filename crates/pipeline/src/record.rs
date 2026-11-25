//! The record stage of the signal pipeline (docs/adr/0064): what `signal::evaluate` says of a subscription is recorded
//! here, under `tick_mu`, in one short transaction per subscription per pass: each fire stored as a firing pending, each
//! skip as a near-miss and its count, and the subscription where the effects leave it (`signal::apply`), its cursor past
//! what it judged. A subscription changed since the evaluation read it is left for the next pass. What the firings do is
//! the deliver stage's (deliver.rs), outside `tick_mu`: the cursor moves when a firing is durably recorded, not when its
//! effect is done, so a crash between the two leaves it pending, and the next start delivers it. The owner's agent lifts a
//! hold a failed firing put a subscription on: it retries the firing (resume_subscription) or gives it up (skip_firing),
//! `lift_hold`.

use model::{
    Actor, Control, Firing, FiringStatus, Hold, HoldReason, InboxItem, InboxKind, InboxOption, InboxStatus,
    JudgmentUse, NearMiss, OwnerKind, SkipReason, Subscription, Time,
};
use signal::Effect;
use store::{FiringFilter, Tx};

use crate::Pipeline;
use chains::apply::keep_when_ended;
use chains::chain_core::new_id;
use chains::fact::{FactValue, fact_write, keys, put_asked_fact};
use chains::inbox::OPTION_RETRY;
use chains::ingest::Door;
use chains::{Error, Result};

/// How many near-misses of one subscription are kept, the newest: enough to answer why it has not fired of late.
pub(crate) const NEAR_MISSES_KEPT: i64 = 20;

/// How many of a subscription's newest firings `redo_firing` looks through for the one a skip gave up.
const REDO_LOOKBACK: i64 = 100;

pub const OPTION_SKIP: &str = "skip";

/// Who records what becomes of a firing.
fn pipeline_by() -> Actor {
    model::by_remit("signals")
}

/// One evaluation of a subscription as the record stage stores it: the firings it decided, the subscription as it was
/// evaluated, where the evaluation leaves it, and the near-misses it leaves.
#[derive(Debug, Clone)]
pub(crate) struct RecordStep {
    pub firings: Vec<Firing>,
    /// As evaluated: the step is recorded only while the subscription is stored at that revision.
    pub sub: Subscription,
    /// The subscription once the step is recorded (`signal::apply`).
    pub next: Subscription,
    /// The step ends the subscription.
    pub ended: bool,
    pub misses: Vec<NearMiss>,
}

/// The effects of one evaluation of sub at now, as recorded: the firings, each pending and saying whose it was and what
/// it does, where it leaves sub, whether it ends it, and a near-miss for each reading it skipped but the ones the
/// subscription's own chain caused, which are only counted. An instant a schedule skipped is only counted too: it is no
/// reading the owner could open.
pub(crate) fn step_of(sub: &Subscription, effects: &[Effect], now: Time) -> RecordStep {
    let mut r = RecordStep {
        firings: Vec::new(),
        sub: sub.clone(),
        next: signal::apply(sub.clone(), effects),
        ended: false,
        misses: Vec::new(),
    };
    for e in effects {
        match e {
            Effect::Fire(fire) => {
                // A chain's subscription is deleted once it is over: the firing keeps whose it was, and what it does.
                let mut f = fire.firing.clone();
                f.owner = sub.owner.clone();
                f.action = sub.action.kind.clone();
                f.status = FiringStatus::Pending;
                r.firings.push(f);
            }
            Effect::Skip(s) if s.reason != SkipReason::OwnChain && sub.schedule.is_none() => {
                r.misses.push(NearMiss {
                    subscription_id: sub.id.clone(),
                    seq: s.seq,
                    key: s.key.clone(),
                    reason: s.reason,
                    detail: s.detail.clone(),
                    judgment: s.judgment.clone(),
                    at: now,
                });
            }
            Effect::End => r.ended = true,
            _ => {}
        }
    }
    r
}

impl RecordStep {
    /// Whether the step stores anything: a firing, or the subscription moved on (past a reading, or to its next instant)
    /// or ended.
    pub(crate) fn moves(&self) -> bool {
        !self.firings.is_empty()
            || self.ended
            || self.next.seen_seq != self.sub.seen_seq
            || self.next.due_at != self.sub.due_at
    }

    /// Stores the step in tx: each firing pending, a firing stored under its key already counted as a duplicate, and the
    /// subscription where the step leaves it. `Ok(false)` when the subscription changed since it was evaluated: nothing is
    /// stored, and the next pass evaluates it as it is.
    fn store(mut self, tx: &Tx<'_>) -> Result<bool> {
        match tx.subscription(&self.sub.id) {
            Err(store::Error::NotFound) if self.ended => return Ok(true), // deleted while the step was ending it anyway
            Err(store::Error::NotFound) => return Ok(false),
            Err(err) => return Err(err.into()),
            // its owner paused, changed or ended it, or a delivery counted a firing: each moves its revision
            Ok(cur) if cur.rev != self.sub.rev => return Ok(false),
            Ok(_) => {}
        }
        for f in &self.firings {
            if !tx.add_firing(f)? {
                self.next.tally.duplicate = self.next.tally.duplicate.saturating_add(1);
            }
        }
        self.move_on(tx)?;
        Ok(true)
    }

    /// Stores the step's subscription where the step leaves it, with its near-misses. One that is over with nothing left
    /// to deliver is deleted, unless it is Remit's (`keep_when_ended`); one with a firing still to deliver is kept, ended,
    /// and its last delivery deletes it (`Delivered::store`).
    fn move_on(self, tx: &Tx<'_>) -> Result<()> {
        if self.ended && !keep_when_ended(&self.sub) && tx.pending_firings(&self.sub.id)?.is_empty() {
            tx.delete_subscription(&self.sub.id)?;
            return Ok(());
        }
        // the answer a drop acted on is recorded as an act with the drop, in the one transaction that stores it
        for m in self.misses.iter().filter(|m| m.reason == SkipReason::Screened) {
            match tx.set_judgment_used(&m.judgment, JudgmentUse::Acted) {
                Ok(()) | Err(store::Error::NotFound) => {}
                Err(err) => return Err(err.into()),
            }
        }
        tx.add_near_misses(&self.sub.id, &self.misses, NEAR_MISSES_KEPT)?;
        tx.put_subscription(self.next)?;
        Ok(())
    }
}

/// How the owner's agent lifts a hold: by trying the failed firing again, or by giving it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiftHow {
    Retry,
    Skip,
}

impl LiftHow {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Retry => OPTION_RETRY,
            Self::Skip => OPTION_SKIP,
        }
    }
}

/// Who may act on a subscription: refuses one the asker may not lift, inspect or redo.
pub(crate) type May = dyn Fn(&Subscription) -> Result<()> + Send + Sync;

/// What anyone may do: the Assistant and the owner reach every subscription.
pub(crate) fn any_subscription(_: &Subscription) -> Result<()> {
    Ok(())
}

/// Hold h in the owner's words, with what lifts it.
pub(crate) fn hold_words(h: &Hold) -> String {
    let what = if h.reason == HoldReason::ProcessorMissing {
        "no processor does its action"
    } else {
        "a firing failed for good"
    };
    format!(
        "{what} ({}, on {}): it waits in place until it is retried (resume_subscription) or the firing is given up (skip_firing)",
        h.detail, h.firing.key
    )
}

/// Ingests fact:subscription.failed in tx: subscription sub is held on firing f, which failed with its last error.
pub(crate) fn put_failed_fact(tx: &Tx<'_>, door: &Door, sub: &Subscription, f: &Firing) -> Result<()> {
    let mut v = FactValue {
        subscription: sub.id.clone(),
        firing: Some(f.id()),
        error: f.last_error.clone(),
        ..FactValue::default()
    };
    if sub.owner.kind == OwnerKind::Chain {
        v.chain.clone_from(&sub.owner.id);
    }
    door.ingest(tx, fact_write(keys::subscription_failed(), &v, pipeline_by(), "")?)?;
    Ok(())
}

/// Ingests fact:subscription.resumed in tx: by lifted (or redid) the firing f of subscription sub, how and why.
fn put_resumed_fact(
    tx: &Tx<'_>,
    door: &Door,
    sub: &Subscription,
    f: &Firing,
    how: &str,
    why: &str,
    by: &Actor,
) -> Result<()> {
    let mut v = FactValue {
        subscription: sub.id.clone(),
        firing: Some(f.id()),
        how: how.to_owned(),
        why: why.to_owned(),
        ..FactValue::default()
    };
    let mut chain_id = "";
    if sub.owner.kind == OwnerKind::Chain {
        v.chain.clone_from(&sub.owner.id);
        chain_id = &sub.owner.id;
    }
    door.ingest(tx, fact_write(keys::subscription_resumed(), &v, by.clone(), chain_id)?)?;
    Ok(())
}

/// Puts in tx, with the hold, the owner's item that subscription sub is held on the firing its hold names: what waits for
/// him, which the Assistant is put first (ADR-0062, through the fact owner.asked). A chain's subscription's item is the
/// chain's; any other is Remit's, on no chain. It answers the item, for the caller to tell the channels once tx commits.
pub(crate) fn raise_held(tx: &Tx<'_>, door: &Door, sub: &Subscription, now: Time) -> Result<InboxItem> {
    let hold = sub.hold.as_ref().map(hold_words).unwrap_or_default();
    let note = sub.note.trim_end_matches('.');
    let it = InboxItem {
        id: new_id("i_"),
        kind: InboxKind::Held,
        subscription: sub.id.clone(),
        status: InboxStatus::Open,
        created_at: now,
        title: format!(
            "Held: {}",
            if note.is_empty() {
                format!("subscription {}", sub.id)
            } else {
                note.to_owned()
            }
        ),
        detail: format!(
            "Subscription {} is held: {hold}.\n\nRetry tries the firing again once what failed is fixed, then the ones after it; skip gives the firing up and goes on with the readings after it. Say why: it is recorded. describe_subscription and explain_subscription say where it stands.",
            sub.id
        ),
        options: vec![
            InboxOption {
                id: OPTION_RETRY.to_owned(),
                name: "Retry".to_owned(),
                ..InboxOption::default()
            },
            InboxOption {
                id: OPTION_SKIP.to_owned(),
                name: "Skip".to_owned(),
                ..InboxOption::default()
            },
        ],
        home: if sub.owner.kind == OwnerKind::Chain {
            sub.owner.id.clone()
        } else {
            String::new()
        },
        ..InboxItem::default()
    };
    let it = tx.put_inbox(it)?;
    put_asked_fact(tx, door, &it)?;
    Ok(it)
}

/// Answers in tx the open item that subscription id is held, as by lifted the hold how and why; none when there is none.
/// It answers the items closed, for the caller to tell the channels once tx commits.
fn close_held(tx: &Tx<'_>, id: &str, how: LiftHow, why: &str, by: &Actor, now: Time) -> Result<Vec<InboxItem>> {
    let mut closed = Vec::new();
    for mut it in tx.open_inbox()? {
        if it.kind != InboxKind::Held || it.subscription != id {
            continue;
        }
        it.status = InboxStatus::Answered;
        it.answer = format!("{}: {why}", how.as_str());
        it.answered_at = Some(now);
        it.answered_by = Some(by.clone());
        closed.push(tx.put_inbox(it)?);
    }
    Ok(closed)
}

impl Pipeline {
    /// Stores what `signal::evaluate` said of sub at now (effects), in one transaction: each firing pending, a firing
    /// stored under its key already counted as a duplicate, and the subscription where the step leaves it. A
    /// subscription changed since it was evaluated is left as it is: the next pass evaluates it again.
    pub(crate) async fn record(&self, sub: &Subscription, effects: &[Effect], now: Time) {
        let r = step_of(sub, effects, now);
        if !r.moves() {
            return;
        }
        if let Err(err) = self.store.update(move |tx| r.store(tx)).await {
            tracing::error!(subscription = %sub.id, %err, "record what a subscription does");
        }
    }

    /// Lifts the hold that held item `it` says, as the answer `option_id` names, why being its text, by by. A subscription
    /// no longer held, or gone, has nothing to lift: the item closes saying so.
    pub async fn answer_held(&self, mut it: InboxItem, option_id: &str, text: &str, by: Actor) -> Result<()> {
        let how = match option_id {
            OPTION_RETRY => LiftHow::Retry,
            OPTION_SKIP => LiftHow::Skip,
            _ => {
                return Err(Error::refused(format!(
                    "a held subscription is answered {OPTION_RETRY} or {OPTION_SKIP}, not {option_id:?}"
                )));
            }
        };
        let why = if text.trim().is_empty() {
            format!("answered {option_id} in the inbox")
        } else {
            text.trim().to_owned()
        };
        match self.lift_hold(&it.subscription, how, &why, by, &any_subscription).await {
            Err(Error::Refused(e)) if e.ends_with(NOT_HELD) => {}
            Err(Error::Store(store::Error::NotFound)) => {}
            Err(err) => return Err(err),
            Ok(_) => return Ok(()),
        }
        it.status = InboxStatus::Cancelled;
        "the subscription is no longer held".clone_into(&mut it.answer);
        let it = self.store.update(move |tx| tx.put_inbox(it)).await?;
        self.tell_inbox(&[it]);
        Ok(())
    }

    /// Does again the newest firing of subscription id that a skip gave up, as by says why (`signal::redo`), with
    /// fact:subscription.resumed, in one transaction. may refuses a subscription by may not act on.
    pub(crate) async fn redo_firing(&self, id: &str, why: &str, by: Actor, may: &May) -> Result<Firing> {
        let why = why.trim().to_owned();
        if why.is_empty() {
            return Err(Error::refused(
                "say why the given-up firing is done again; it is recorded",
            ));
        }
        let key = id.to_owned();
        let sub = self.store.read(move |r| r.subscription(&key)).await?;
        may(&sub)?;
        let (door, key) = (self.door(), id.to_owned());
        let redone = self
            .store
            .update(move |tx| {
                let sub = tx.subscription(&key)?;
                let filter = FiringFilter { subscription: key.clone(), ..FiringFilter::default() };
                let fs = tx.firings(&filter, REDO_LOOKBACK)?;
                let Some(given_up) = fs.into_iter().find(|f| f.status == FiringStatus::Skipped) else {
                    return Err(Error::refused(format!(
                        "subscription {key} has no given-up firing among its newest {REDO_LOOKBACK}: a held one is resumed with resume_subscription"
                    )));
                };
                let redone = signal::redo(given_up);
                tx.update_firing(&redone)?;
                put_resumed_fact(tx, &door, &sub, &redone, "redo", &why, &by)?;
                Ok(redone)
            })
            .await?;
        self.kick(); // the firing done again is delivered
        Ok(redone)
    }

    /// Lifts the hold of subscription id as by says how and why, in one transaction (`signal::retry`,
    /// `signal::skip_firing`), with fact:subscription.resumed and the owner's item that it was held answered. may refuses
    /// a subscription by may not lift. A subscription a skip ends is deleted when it is a chain's.
    pub(crate) async fn lift_hold(
        &self,
        id: &str,
        how: LiftHow,
        why: &str,
        by: Actor,
        may: &May,
    ) -> Result<Subscription> {
        let why = why.trim().to_owned();
        if why.is_empty() {
            return Err(Error::refused(
                "say why: what was fixed, or why the firing is given up; it is recorded with the hold it lifts",
            ));
        }
        let key = id.to_owned();
        let cur = self.store.read(move |r| r.subscription(&key)).await?;
        may(&cur)?;
        let (door, key) = (self.door(), id.to_owned());
        let (saved, closed) = self
            .store
            .update(move |tx| {
                let cur = tx.subscription(&key)?;
                let Some(hold) = cur.hold.clone() else {
                    return Err(Error::refused(format!("subscription {key} {NOT_HELD}")));
                };
                let closed = close_held(tx, &key, how, &why, &by, door.now)?;
                let f = tx
                    .firing(&hold.firing)
                    .map_err(|e| Error::refused(format!("the firing {key} is held on: {e}")))?;
                let (mut next, lifted) = match how {
                    LiftHow::Retry => signal::retry(cur, f.clone()),
                    LiftHow::Skip => signal::skip_firing(cur, f.clone()),
                };
                tx.update_firing(&lifted)?;
                put_resumed_fact(tx, &door, &next, &f, how.as_str(), &why, &by)?;
                if next.control == Control::Ended && !keep_when_ended(&next) {
                    tx.delete_subscription(&key)?;
                    return Ok((next, closed));
                }
                next.by = by;
                next.updated_at = door.now;
                Ok((tx.put_subscription(next)?, closed))
            })
            .await?;
        self.tell_inbox(&closed);
        self.kick(); // the firing retried is delivered, or the next goes
        Ok(saved)
    }
}

/// Says a subscription has no hold to lift.
pub(crate) const NOT_HELD: &str = "is not held: no firing of it failed for good";
