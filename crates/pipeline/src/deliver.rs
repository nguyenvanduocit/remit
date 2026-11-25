//! The deliver stage of the signal pipeline (docs/adr/0064, Record, then deliver). The record stage stores each firing
//! pending; here a worker of its own for each subscription hands them to their processor, one at a time in the order they
//! were decided, outside `tick_mu`, each cut at its processor's deadline: a processor that hangs holds its own
//! subscription and no other, and the scheduler's Tick waits for none. A restart finds the pending firings and delivers
//! them, so a firing recorded is delivered at least once; its key is the request id the processor keys an outside effect
//! by, and its database effect is stored with the firing done in one transaction (Delivered), so that effect happens once.
//! What a processor answers is handled here and nowhere else: done, nothing to act on, waiting without failing (the reason
//! stored on the firing, `waiting`, for the Subscriptions tab), or failed, tried again after a backoff and then failed for
//! good, holding its subscription in place (`signal::settle`) with fact:subscription.failed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use model::{Control, Firing, FiringStatus, Subscription, Time};

use crate::Pipeline;
use crate::processing::Delivery;
use crate::record::{put_failed_fact, raise_held};
use chains::admit::Room;
use chains::{Error, Result};

/// The delivery workers at work, one per subscription, by its id. In memory: a worker is a task of this process, and
/// what it delivers is stored.
#[derive(Default)]
pub struct Deliveries(Mutex<HashMap<String, Delivering>>);

/// One subscription's worker: `again` says a pass asked for it while it worked, so it looks once more before it ends, at
/// the latest time a pass gave it.
struct Delivering {
    again: bool,
    now: Time,
}

impl Deliveries {
    fn work(&self) -> std::sync::MutexGuard<'_, HashMap<String, Delivering>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Pipeline {
    /// Stores the firing of d done, saying outcome: what a firing that did nothing but say why does.
    pub async fn done_saying(&self, d: &Delivery, outcome: &str) -> Result<()> {
        self.store_done(d.done(outcome, "", ""), |_, _| Ok(())).await
    }

    /// Starts a worker for each subscription with a pending firing due by now: one that has not failed an attempt, or
    /// whose backoff has passed. The worker itself passes over a subscription that is paused or held. Called by the Tick,
    /// after the record pass.
    pub(crate) async fn deliver_due(&self, now: Time) {
        let pending = match self.store.read(|r| r.pending_firings("")).await {
            Ok(p) => p,
            Err(err) => {
                tracing::error!(%err, "deliver: read the pending firings");
                return;
            }
        };
        let mut last = "";
        for f in &pending {
            if f.subscription_id != last && f.next_attempt.is_none_or(|at| now >= at) {
                self.start_delivering(&f.subscription_id, now);
            }
            last = &f.subscription_id;
        }
    }

    /// Has subscription id's worker deliver what is due by now: a new one, or the one at work, which looks again once it
    /// is done. Once Shutdown has begun none starts, and what is pending is delivered after the restart.
    fn start_delivering(&self, id: &str, now: Time) {
        let Some(me) = self.me() else { return };
        {
            let mut work = self.signals.deliveries.work();
            if let Some(w) = work.get_mut(id) {
                w.again = true;
                w.now = w.now.max(now);
                return;
            }
            work.insert(id.to_owned(), Delivering { again: false, now });
        }
        let key = id.to_owned();
        let started = self.background(move |_| async move { me.deliver_all(&key).await });
        if !started {
            self.signals.deliveries.work().remove(id);
        }
    }

    /// Subscription id's worker: it delivers what is due, and looks again while a pass asked it to meanwhile.
    async fn deliver_all(self: Arc<Self>, id: &str) {
        loop {
            let now = {
                let mut work = self.signals.deliveries.work();
                let Some(w) = work.get_mut(id) else { return };
                w.again = false;
                w.now
            };
            self.deliver_due_of(id, now).await;
            let mut work = self.signals.deliveries.work();
            if !work.get(id).is_some_and(|w| w.again) {
                work.remove(id);
                return;
            }
        }
    }

    /// Delivers the pending firings of subscription id, one at a time in their order, while the next is due by now. It
    /// stops at one that waits out its backoff, waits without failing or failed: a subscription's firings are delivered in
    /// order, so the ones after it wait behind it. A paused or held subscription is delivered nothing.
    async fn deliver_due_of(&self, id: &str, now: Time) {
        while !self.work_ends.is_cancelled() {
            let key = id.to_owned();
            let read = self
                .store
                .read(move |r| Ok::<_, store::Error>((r.subscription(&key)?, r.pending_firings(&key)?)))
                .await;
            let (sub, pending) = match read {
                Ok(read) => read,
                Err(store::Error::NotFound) => return,
                Err(err) => {
                    tracing::error!(subscription = %id, %err, "deliver: read the subscription");
                    return;
                }
            };
            if sub.control == Control::Paused || sub.hold.is_some() {
                return;
            }
            let Some(next) = pending.into_iter().next() else { return };
            if next.next_attempt.is_some_and(|at| now < at) || !self.deliver_one(sub, next, now).await {
                return;
            }
        }
    }

    /// Hands firing f of sub to its processor at now and settles what it answers. It says whether f is settled, so that
    /// the next firing may go.
    async fn deliver_one(&self, sub: Subscription, f: Firing, now: Time) -> bool {
        let (key, seqs) = (f.key.clone(), f.seqs.clone());
        let readings = match self.store.read(move |r| r.readings_of(&key, &seqs)).await {
            Ok(rs) => rs,
            Err(err) => {
                tracing::error!(subscription = %sub.id, key = %f.key, rev = f.rev, %err, "deliver: read the firing's readings");
                return false;
            }
        };
        let d = Delivery {
            request_id: f.id().to_string(),
            room: Room {
                no_agents: self.no_agents(),
                no_headroom: self.no_headroom(),
                ..Room::default()
            },
            sub,
            firing: f,
            readings,
            now,
        };
        let answer = self.process(&d).await;
        if self.work_ends.is_cancelled() {
            return false; // the server is stopping: the firing stays pending, and the next start delivers it
        }
        let f = &d.firing;
        match answer {
            Ok(()) | Err(Error::FiredAlready) => {
                self.events_fired(&d.readings).await;
                self.settled(f).await
            }
            Err(Error::NothingToDo) => {
                match self
                    .done_saying(&d, "nothing to act on: what it names is gone, or done already")
                    .await
                {
                    Ok(()) | Err(Error::FiredAlready) => true,
                    Err(err) => {
                        tracing::error!(subscription = %d.sub.id, key = %f.key, rev = f.rev, %err, "deliver: store a firing with nothing to do");
                        false
                    }
                }
            }
            Err(Error::NotAdmitted(why)) => {
                self.store_waiting(f, why).await;
                false
            }
            Err(Error::RoutineAtWork(_)) => {
                self.store_waiting(
                    f,
                    "its last run is still at work: the next starts once it is done".to_owned(),
                )
                .await;
                false
            }
            Err(err) => {
                // its backoff counts from when it failed, a deadline after it began
                self.record_failed_attempt(f, &err, now.max(self.now())).await;
                false
            }
        }
    }

    /// Whether firing f is no longer pending, after its processor answered that it was done: one that is still pending
    /// was not stored by its processor, and its worker stops rather than hand it over again at once; the next pass
    /// delivers it.
    async fn settled(&self, f: &Firing) -> bool {
        let key = f.id();
        match self.store.read(move |r| r.firing(&key)).await {
            Ok(cur) if cur.status == FiringStatus::Pending => {
                tracing::error!(subscription = %f.subscription_id, action = %f.action, key = %f.key, rev = f.rev, "deliver: a processor answered done and did not store the firing done");
                false
            }
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Stores why pending firing f waits without failing, as the Subscriptions tab says; no attempt is counted, and the
    /// next pass tries it again.
    async fn store_waiting(&self, f: &Firing, why: String) {
        let key = f.id();
        let stored = self
            .store
            .update(move |tx| {
                let mut cur = tx.firing(&key)?;
                if cur.status != FiringStatus::Pending || cur.waiting == why {
                    return Ok(());
                }
                cur.waiting = why;
                tx.update_firing(&cur)
            })
            .await;
        if let Err(err) = stored {
            tracing::error!(subscription = %f.subscription_id, key = %f.key, rev = f.rev, %err, "deliver: store why a firing waits");
        }
    }

    /// Records an attempt of firing f that failed with cause at now, as `signal::settle` says: the firing pending, tried
    /// again after its backoff, or failed for good with its subscription held on it in place and fact:subscription.failed
    /// ingested, in one transaction. A firing no longer pending (its subscription deleted meanwhile) is left as it is.
    async fn record_failed_attempt(&self, f: &Firing, cause: &Error, now: Time) {
        let err = match cause {
            Error::Permanent(reason, why) => signal::permanent(*reason, why),
            other => signal::DeliveryError::new(other),
        };
        let (key, sub_id, door) = (f.id(), f.subscription_id.clone(), self.door());
        let stored = self
            .store
            .update(move |tx| {
                let cur = tx.firing(&key)?;
                if cur.status != FiringStatus::Pending {
                    return Ok::<_, Error>(None);
                }
                let sub = tx.subscription(&sub_id)?;
                let (next, mut failed, effects) = signal::settle(sub, cur, &err, now);
                failed.waiting = String::new();
                tx.update_firing(&failed)?;
                if effects.is_empty() {
                    return Ok(Some((failed, None)));
                }
                let next = tx.put_subscription(next)?;
                let item = raise_held(tx, &door, &next, now)?;
                put_failed_fact(tx, &door, &next, &failed)?;
                Ok(Some((failed, Some(item))))
            })
            .await;
        match stored {
            Err(err) => {
                tracing::error!(subscription = %f.subscription_id, key = %f.key, rev = f.rev, %err, "record a failed firing");
            }
            Ok(Some((settled, Some(item)))) => {
                tracing::warn!(subscription = %f.subscription_id, key = %f.key, rev = f.rev, attempts = settled.attempts, err = %settled.last_error, "subscription held: a firing failed for good");
                self.tell_inbox(&[item]);
                self.kick(); // the facts are judged on the next Tick: the failure, and the owner's item put to the Assistant
            }
            Ok(Some((settled, None))) if settled.status == FiringStatus::Pending => {
                tracing::warn!(subscription = %f.subscription_id, key = %f.key, rev = f.rev, attempt = settled.attempts, err = %settled.last_error, "a firing failed: tried again later");
            }
            Ok(_) => {}
        }
    }
}
