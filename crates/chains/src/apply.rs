//! A chain's lifecycle changes in one place: `apply` runs a command through step and stores the change, under the chain's
//! lock, so changes are made one at a time, stored and seen in the order they were made, and none is lost.

use std::sync::Arc;

use model::{Card, Chain, ChainStatus, Edge, Employee, Event, Help, InboxItem};
use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::admit::admit;
use crate::chain_core::Core;
use crate::ingest::Door;
use crate::ingest::Write;
use crate::outbox::Message;
use crate::step::{CardEdit, ChainCmd, Decided, Effect, Step, Transition, step};
use crate::wait::clock;
use crate::{Error, Result};
use model::Control;
use model::Cut;
use model::FiringKey;
use model::FiringStatus;
use model::OwnerKind;
use model::Subscription;
use store::Tx;

/// A chain as its one writer holds it. The lock is held across a change: step, the transaction that stores it, and the
/// assignment and publishing of what it stored.
pub struct ChainState {
    pub held: Mutex<Held>,
    /// Serialises changes to the chain's runs of a script (`lock_runs`): each is read, advanced and stored under it, so
    /// two changes of one run never interleave. Taken before `held`.
    pub runs: Arc<Mutex<()>>,
}

pub struct Held {
    /// As stored: assigned only once a write of it has committed.
    pub chain: Chain,
    /// Deleted: any change still waiting on the lock is refused.
    pub gone: bool,
    /// True unless the chain is paused: a session waits on it before it takes its next message. Only `set` keeps it
    /// true to the status.
    pub unpaused: watch::Sender<bool>,
    /// The lifetime of the chain's work in the background (a submit; `chain_work`): it ends when the chain
    /// pauses, finishes or is deleted, and a new one begins when it runs again.
    pub work: CancellationToken,
}

impl Held {
    fn new(ch: Chain) -> Self {
        let mut h = Self {
            chain: Chain::default(),
            gone: false,
            unpaused: watch::Sender::new(true),
            work: CancellationToken::new(),
        };
        h.set(ch);
        h
    }

    /// Makes ch the chain held.
    pub fn set(&mut self, ch: Chain) {
        let status = *ch.status();
        let held = status == ChainStatus::Paused || status == ChainStatus::Done;
        if !held && self.work.is_cancelled() {
            self.work = CancellationToken::new();
        }
        if held {
            self.work.cancel();
        }
        self.unpaused.send_replace(status != ChainStatus::Paused);
        self.chain = ch;
    }
}

/// Drops the messages queued for a card from priority min up.
#[derive(Debug, Clone)]
pub struct Drop {
    pub card_id: String,
    pub min: i64,
}

/// A change to an open inbox item stored with a change, made on the item's latest copy in its transaction, as a
/// `CardEdit` is on a card's: an item answered, decided or closed since its writer read it is left as it is.
#[derive(Clone)]
pub struct InboxEdit {
    pub id: String,
    pub edit: Arc<dyn Fn(&mut InboxItem) + Send + Sync>,
}

impl InboxEdit {
    pub fn new(id: &str, edit: impl Fn(&mut InboxItem) + Send + Sync + 'static) -> Self {
        Self {
            id: id.to_owned(),
            edit: Arc::new(edit),
        }
    }
}

/// A rule a change must still pass as the store holds it (`Records::checks`): its refusal refuses the change.
pub type Check = Arc<dyn Fn(&Tx<'_>) -> Result<()> + Send + Sync>;

/// What is stored in the transaction of a change, beside the chain.
#[derive(Clone, Default)]
pub struct Records {
    /// Rules run first in the change's transaction: a rule its writer checked on an earlier read holds against what was
    /// stored since (a card contested twice at once is refused once).
    pub checks: Vec<Check>,
    /// The employees the change hires or retires: an employee never exists without the card that works for it, and a
    /// card never without its employee. Each is written over the revision it carries.
    pub employees: Vec<Employee>,
    /// New cards.
    pub created: Vec<Card>,
    /// Changes to existing cards, made on their latest copy.
    pub cards: Vec<CardEdit>,
    /// New edges.
    pub edges: Vec<Edge>,
    pub decisions: Vec<Decided>,
    /// The verdicts it records (gate.rs, goal.rs): a chain's verdicts are records of their own.
    pub approvals: Vec<model::Approval>,
    /// The duels it opens or judges (duel.rs), each written over the revision it carries.
    pub duels: Vec<model::Duel>,
    /// The second opinions it opens (second_opinion.rs), each written over the revision it carries.
    pub second_opinions: Vec<model::SecondOpinion>,
    /// The rounds of competing attempts it opens, judges or ends (ADR-0115), each written over the revision it carries.
    pub rounds: Vec<model::Round>,
    /// Items it writes, each over the revision it carries.
    pub inbox: Vec<InboxItem>,
    /// Changes to items that are open, made on their latest copy (an item no longer open is left as it is).
    pub inbox_edits: Vec<InboxEdit>,
    /// Items it asks the owner to decide (inbox.rs's `put_asked`): each stored with the fact owner.asked, which puts it to
    /// the Assistant (ADR-0087).
    pub asked: Vec<InboxItem>,
    pub drops: Vec<Drop>,
    /// The helps it offers or answers (help.rs), each written over the revision it carries.
    pub helps: Vec<model::Help>,
    /// Cards whose work the change ends (stopped for good, handed over): their open helps end with it (help.rs).
    pub ended: Vec<String>,
    /// Messages it sends, each queued for its card in the card's home, after the drops.
    pub messages: Vec<Message>,
    /// Events the change records itself (a job's start or end): one with no id is added, one with an id rewritten.
    pub events: Vec<Event>,
    /// The firing of a subscription the change is the effect of, stored done in the step that does it, so a firing is
    /// done once (signals.rs's `put_signals`).
    pub fired: Option<Delivered>,
    /// The readings of signals the change is about (its facts, the records of live.rs): ingested with it, and refused
    /// with it.
    pub readings: Vec<Write>,
    /// The owner's drafts the change sends, ended in its transaction as they were read.
    pub drafts: Vec<model::Draft>,
    /// The runs of a script the change writes, each without its steps (workflow_run.rs's `put_runs`), and the steps.
    pub runs: Vec<model::Workflow>,
    pub steps: Vec<model::WorkflowStep>,
    /// The subscriptions it makes (a run's wait) and, by id, those it ends.
    pub subs: Vec<model::Subscription>,
    pub end_subs: Vec<String>,
    /// The releases it writes (ADR-0116), each over the revision it carries, before the landings: each landing a release
    /// carries is told again with it.
    pub releases: Vec<model::Release>,
    /// The landings it writes (ADR-0114), each over the revision it carries and with who moved it: its transaction
    /// writes the reading of each whose word moved (live.rs `landing_moved`), against the landing and the verdicts as
    /// stored, so no other writer comes between.
    pub landings: Vec<crate::live::LandingWrite>,
    /// The release pauses it pulls or lifts (ADR-0116).
    pub release_pauses: Vec<model::ReleasePause>,
    /// What the change proves of the picks of judgments (ADR-0118), stored with it so a crash cannot lose one.
    pub proofs: Vec<Proof>,
    /// The judgments the change shows to an agent (ADR-0118: a watchdog's note in the report it sends): stored as held
    /// until it commits, so an answer whose note never went out is not recorded as shown.
    pub shown: Vec<String>,
}

/// What a change proves of the pick of one judgment (`Records::proofs`).
#[derive(Debug, Clone)]
pub struct Proof {
    pub judgment: String,
    pub proved: model::Proved,
    pub source: model::OutcomeSource,
    /// Only when the judgment's asking has no outcome yet: what is merely known (its card went before it showed) must
    /// not replace what an earlier event proved.
    pub settle: bool,
}

/// What a change stored.
#[derive(Debug, Clone, Default)]
pub struct Applied {
    pub chain: Chain,
    /// False: the chain already was what the command asked, and nothing was written.
    pub changed: bool,
    /// The messages it queued: the change's records' own, then step's in the order step sent them.
    pub sent: Vec<Event>,
    /// The messages `sent` are of, in the same order.
    pub queued: Vec<Message>,
    /// The cards it created, then the cards it changed, as stored.
    pub cards: Vec<Card>,
    /// The helps it wrote or ended, as stored.
    pub helps: Vec<Help>,
}

impl Core {
    /// The chain state of ch, made when the orchestrator holds none.
    pub fn chain_state(&self, ch: Chain) -> Arc<ChainState> {
        let mut sh = self.shared();
        Arc::clone(sh.chains.entry(ch.id.clone()).or_insert_with(|| {
            Arc::new(ChainState {
                held: Mutex::new(Held::new(ch)),
                runs: Arc::new(Mutex::new(())),
            })
        }))
    }

    /// The chain state of chain id, read from the store the first time.
    pub async fn state(&self, id: &str) -> Result<Arc<ChainState>> {
        if let Some(cs) = self.shared().chains.get(id) {
            return Ok(Arc::clone(cs));
        }
        let key = id.to_owned();
        let ch = self.store.read(move |r| r.chain(&key)).await?;
        Ok(self.chain_state(ch))
    }

    /// The chain as last stored.
    pub async fn chain(&self, id: &str) -> Result<Chain> {
        let cs = self.state(id).await?;
        let held = cs.held.lock().await;
        Ok(held.chain.clone())
    }

    /// Runs cmd on the chain and stores the change with w, in one transaction; it is the only way a chain's lifecycle
    /// changes. A refused command returns step's reason and changes nothing; so does a write the store refuses, and a
    /// command the chain already satisfies (w included: it goes with the change, and there is none). Once stored, the
    /// change is published, the sessions it ends are closed while the lock is still held (so no message can be handed to
    /// one of them in between), and then the messages it sent are handed to their cards.
    pub async fn apply(&self, id: &str, cmd: ChainCmd, w: Records) -> Result<Applied> {
        use ::telemetry::attr;
        let span = tracing::info_span!(attr::STEP_SPAN, { attr::CHAIN_ID } = id, { attr::STEP } = %cmd);
        // boxed: every change of a chain runs through here, under many callers, and an unboxed step would inline its
        // whole state (commit, ending sessions, the hand-over) into each of their futures and debug-build stack frames
        Box::pin(self.apply_step(id, cmd, w).instrument(span)).await
    }

    /// `apply`, in the span of its step.
    async fn apply_step(&self, id: &str, cmd: ChainCmd, w: Records) -> Result<Applied> {
        let cs = self.state(id).await?;
        let mut held = cs.held.lock().await;
        let mut done = self.apply_locked(&mut held, &cmd, w.clone()).await;
        if matches!(&done, Err(e) if e.is_conflict()) {
            // commit has read the chain as stored: decide again on that
            done = self.apply_locked(&mut held, &cmd, w).await;
        }
        let (t, a) = done?;
        if a.changed {
            self.end_sessions(id, &t).await;
        }
        drop(held);
        if a.changed {
            self.after_change(id, &t, &a).await;
        }
        Ok(a)
    }

    /// One attempt of `apply`, under the chain's lock: step on the chain held, admission, and the commit.
    async fn apply_locked(&self, held: &mut Held, cmd: &ChainCmd, mut w: Records) -> Result<(Transition, Applied)> {
        if held.gone {
            return Err(Error::ChainDeleted);
        }
        let unchanged = Applied {
            chain: held.chain.clone(),
            ..Applied::default()
        };
        let waits = self.waits_of(&held.chain.id).await?;
        let cmd = self.observe(cmd.clone(), &held.chain, &waits).await?;
        let t = match step(&held.chain, &waits, &cmd, self.now())? {
            Step::Unchanged => return Ok((Transition::default(), unchanged)),
            Step::Changed(t) => *t,
        };
        if *t.next.status() == ChainStatus::Running && *held.chain.status() != ChainStatus::Running {
            let room = self.room(&held.chain.id).await?;
            admit(&held.chain, &cmd, &room)?;
        }
        if let Some(by) = &t.closed_out_by {
            w.readings.push(crate::fact::finished_fact(&t.next, by.clone())?);
        }
        let mut decisions = t.decisions.clone();
        decisions.append(&mut w.decisions);
        w.decisions = decisions;
        let mut cards = t.cards.clone();
        cards.append(&mut w.cards);
        w.cards = cards;
        for m in &t.messages {
            let mut m = m.clone();
            held.chain.id.clone_into(&mut m.home);
            w.messages.push(m);
        }
        let read = !w.readings.is_empty();
        let a = self.commit(Some(held), Some(&t), w).await?;
        if read {
            self.kick(); // the subscriptions to what the change says judge it on the next pass
        }
        Ok((t, a))
    }

    /// cmd with what it depends on besides the chain and its waits observed, under the chain's lock, so nothing it reads
    /// can change before the change is stored. What it cannot read refuses the command.
    async fn observe(&self, cmd: ChainCmd, ch: &Chain, w: &crate::wait::WakeSubs) -> Result<ChainCmd> {
        Ok(match cmd {
            ChainCmd::TurnEnded(c) => ChainCmd::TurnEnded(self.observe_turn_ended(c, ch, w).await),
            ChainCmd::Wake(c) => ChainCmd::Wake(self.observe_wake(c, ch, w).await),
            ChainCmd::EndGoalCheck(c) => match self.domains() {
                Ok(d) => ChainCmd::EndGoalCheck(d.observe_goal_check(c, ch).await),
                Err(_) => ChainCmd::EndGoalCheck(c),
            },
            ChainCmd::MarkLive(c) => ChainCmd::MarkLive(self.observe_mark_live(c, ch).await?),
            ChainCmd::Workflow(c) => ChainCmd::Workflow(self.observe_workflow(c, ch).await),
            other => other,
        })
    }

    /// Changes a card on its latest copy, in the store, then tells the browsers.
    pub async fn update_card(&self, id: &str, edit: impl Fn(&mut Card) + Send + Sync + 'static) -> Result<Card> {
        let a = self
            .commit(
                None,
                None,
                Records {
                    cards: vec![CardEdit::new(id, edit)],
                    ..Records::default()
                },
            )
            .await?;
        a.cards
            .into_iter()
            .next()
            .ok_or_else(|| Error::refused(format!("card {id} was not stored")))
    }

    /// Closes the sessions a stored change ends, while the chain's lock is still held: a message sent after the change
    /// finds them closed and opens a new session, and none is handed to a session about to close.
    async fn end_sessions(&self, id: &str, t: &Transition) {
        for e in &t.effects {
            match e {
                Effect::SleepSessions => {
                    let activity = if let Some(until) = t.sleep_until {
                        format!("waiting until {}", clock(until, self.now()))
                    } else {
                        "waiting".to_owned()
                    };
                    self.close_sessions(self.chain_runs(id), &activity, &Cut::Slept).await;
                }
                Effect::StopBackground => self.stop_background(id, &Cut::Paused).await,
                Effect::EndSessions => self.close_sessions(self.chain_runs(id), "", &Cut::ChainFinished).await,
                Effect::DropSessions => self.drop_sessions(id, &Cut::ChainDeleted),
                // the rest is after_change's, once the lock is released
                Effect::CancelTurns
                | Effect::FreeSlot
                | Effect::ResetNudges
                | Effect::Reschedule
                | Effect::RestoreWork => {}
            }
        }
    }

    /// Does what a stored change asks once the chain's lock is released: it ends turns, frees the slot, brings a reopened
    /// chain's worktrees back, and hands the messages it sent to their cards.
    async fn after_change(&self, id: &str, t: &Transition, a: &Applied) {
        for e in &t.effects {
            match e {
                Effect::CancelTurns => self.cut_turns(id, &Cut::Paused).await,
                Effect::SleepSessions => self.redispatch(id).await,
                Effect::FreeSlot | Effect::Reschedule => self.kick(),
                Effect::ResetNudges => {
                    self.shared().nudges.remove(id);
                }
                Effect::RestoreWork => self.restore_worktrees(&a.chain).await,
                // closed under the lock (end_sessions)
                Effect::StopBackground | Effect::EndSessions | Effect::DropSessions => {}
            }
        }
        self.hand_over(a).await;
    }

    /// Hands the messages a stored change sent to their cards: into the turn a card is in when its sender needs it read
    /// now, else to its session's next turn, opening a session when it has none.
    pub async fn hand_over(&self, a: &Applied) {
        let mut handed: Vec<&str> = Vec::new();
        for (e, m) in a.sent.iter().zip(&a.queued) {
            if m.steer && self.steer(e).await {
                continue;
            }
            if !handed.contains(&e.card_id.as_str()) {
                handed.push(&e.card_id);
                self.dispatch(&e.card_id).await;
            }
        }
    }
}

/// A firing done as its processor stores it with its effect: what it did, in words, for the board, and the chain and card
/// it acted on. Its processor stores it in the transaction that makes the effect (`store`, through `store_done` or the
/// change's records), so the effect and the firing done are stored together or not at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delivered {
    pub outcome: String,
    pub chain: String,
    pub card: String,
    firing: FiringKey,
}

impl Delivered {
    pub fn new(firing: FiringKey, outcome: &str, chain: &str, card: &str) -> Self {
        Self {
            outcome: outcome.to_owned(),
            chain: chain.to_owned(),
            card: card.to_owned(),
            firing,
        }
    }

    /// Records the firing done in tx, the transaction of its effect, and counts it on its subscription; a subscription
    /// that is over with nothing left to deliver is deleted then, unless it is Remit's (`keep_when_ended`). A firing done
    /// or given up already is refused (`Error::FiredAlready`), so its effect is not stored twice. One cancelled while it
    /// was delivered (its subscription deleted meanwhile, by its own effect, as a goal check's deadline ends the check) is
    /// stored done all the same: the effect is done.
    pub fn store(&self, tx: &Tx<'_>) -> Result<()> {
        let mut f = tx.firing(&self.firing)?;
        if f.status != FiringStatus::Pending && f.status != FiringStatus::Cancelled {
            return Err(Error::FiredAlready);
        }
        f.status = FiringStatus::Done;
        f.attempts = f.attempts.saturating_add(1);
        f.next_attempt = None;
        f.waiting = String::new();
        self.outcome.trim().clone_into(&mut f.outcome);
        f.chain.clone_from(&self.chain);
        f.card.clone_from(&self.card);
        tx.update_firing(&f)?;
        let mut sub = match tx.subscription(&f.subscription_id) {
            Ok(sub) => sub,
            Err(store::Error::NotFound) => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        sub.tally.fired = sub.tally.fired.saturating_add(1);
        if sub.control == Control::Ended && !keep_when_ended(&sub) && tx.pending_firings(&sub.id)?.is_empty() {
            tx.delete_subscription(&sub.id)?;
            return Ok(());
        }
        tx.put_subscription(sub)?;
        Ok(())
    }
}

impl Core {
    /// Stores dd and the effect write makes, in one transaction, and answers what write made: a processor's firing whose
    /// effect is the store's alone.
    pub async fn store_done<T, F>(&self, dd: Delivered, write: F) -> Result<T>
    where
        F: FnOnce(&Tx<'_>, &Door) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let door = self.door();
        self.store
            .update(move |tx| {
                dd.store(tx)?;
                write(tx, &door)
            })
            .await
    }
}

/// Says a subscription that is over stays in the store: Remit's own, which its owner reads (a routine that ran its last
/// time) and deletes by his word.
pub fn keep_when_ended(s: &Subscription) -> bool {
    s.owner.kind == OwnerKind::Remit
}
