//! A change is stored in one transaction and, once it has committed, held in memory and published, all before the
//! chain's lock (held by the caller) is released: the store and the browsers see changes in the order they were made.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use model::{Card, Chain, ChainStatus, Cleanup, Decision, Event, InboxItem, InboxKind, InboxStatus, Time};
use store::Tx;

use crate::apply::{Applied, Held, Records};
use crate::apply_subs::put_waits;
use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::fact::put_steer_fact;
use crate::goal::owned_by;
use crate::inbox::put_asked;
use crate::ingest::Door;
use crate::outbox::by_remit;
use crate::outbox::queue_message;
use crate::step::{CardEdit, Transition};
use crate::subscribe::subscribe;
use crate::timewait::once_at;
use crate::wait::chain_read;
use crate::workflow_run::{put_runs, stop_runs_of};
use crate::{Error, Result};
use model::ActionKind;
use model::Employee;
use model::Help;
use model::HelpState;
use model::Start;
use model::StartFrom;
use model::Subscription;
use model::SubscriptionAction;
use wire::app::CheckBack;

/// One commit: what it writes and, once its transaction has run, what the store holds of it.
struct Change {
    t: Option<Transition>,
    w: Records,
    now: Time,
    /// What deleting the chain leaves on disk, when t deletes it.
    cleanup: Option<Cleanup>,
    /// The door the change's readings enter by (ingest.rs).
    door: Door,
    /// The chain the change is about: its own, or its first card's home.
    chain_id: String,
    decisions: Vec<Decision>,
    saved: Option<Chain>,
    /// What the chain waits for once the change is stored.
    wakes: HashMap<String, CheckBack>,
    added: Vec<Event>,
    rewritten: Vec<Event>,
    /// Queued messages dropped, and what finishing the chain dropped.
    dropped: Vec<Event>,
    /// `w.messages`, as queued.
    sent: Vec<Event>,
    /// Cards the change creates, as stored.
    created: Vec<Card>,
    /// Cards the change edits, as stored.
    edited: Vec<Card>,
    /// As stored: `w.inbox`, `w.asked`, then what finishing the chain cancelled.
    inbox: Vec<InboxItem>,
    /// What the sessions of the cards it edits hold of them now: a card a session runs is
    /// changed on the run's copy, so what it shows of the moment (stream.rs's `show`) is stored with it.
    live: HashMap<String, Live>,
    /// The runs the change wrote, each as last stored with its steps (workflow_run.rs's `put_runs`).
    runs: Vec<model::Workflow>,
    /// The helps the change wrote or ended, as stored.
    helps: Vec<model::Help>,
    /// The landings it wrote and those the releases it wrote carry, as stored, each with its release.
    landings: Vec<wire::app::LandingRead>,
    /// The repositories whose landings or releases it wrote.
    repos: BTreeSet<String>,
    /// A message it queued steers its card, and taught it (`put_steer_fact`).
    taught: bool,
}

/// What a run holds of its card that the store does not have yet, and whether the run has closed.
struct Live {
    activity: String,
    ctx_used: i64,
    ctx_size: i64,
    closed: bool,
}

impl Core {
    /// Stores a change in one transaction (the chain at t's next, when t is set, and w) and, once it has committed,
    /// holds what it stored in held and publishes it. A chain written over a revision that is no longer the stored one is
    /// refused and read again: something wrote it around its writer.
    pub async fn commit(&self, held: Option<&mut Held>, t: Option<&Transition>, w: Records) -> Result<Applied> {
        let cleanup = t.filter(|t| t.gone).map(|t| self.cleanup_of(&t.next));
        let door = self.door();
        let w_cards: Vec<String> = w.cards.iter().map(|e| e.id.clone()).collect();
        let c = Change {
            t: t.cloned(),
            w,
            now: door.now,
            cleanup,
            door,
            chain_id: t.map(|t| t.next.id.clone()).unwrap_or_default(),
            decisions: Vec::new(),
            saved: None,
            wakes: HashMap::new(),
            added: Vec::new(),
            rewritten: Vec::new(),
            dropped: Vec::new(),
            sent: Vec::new(),
            created: Vec::new(),
            edited: Vec::new(),
            inbox: Vec::new(),
            live: self.live_of(&w_cards),
            runs: Vec::new(),
            helps: Vec::new(),
            landings: Vec::new(),
            repos: BTreeSet::new(),
            taught: false,
        };
        // boxed: every stored change runs through here, and the change it carries would otherwise sit inline in the
        // future (and the debug-build stack frame) of each of the many callers
        let stored = Box::pin(self.store.update(move |tx| {
            let mut c = c;
            put(tx, &mut c)?;
            Ok::<_, Error>(c)
        }))
        .await;
        match stored {
            Ok(c) => Ok(self.publish(held, c)),
            Err(err) => {
                if let (true, Some(held)) = (err.is_conflict(), held) {
                    tracing::warn!(chain = %held.chain.id, "a chain was written around its writer: reading it again");
                    let id = held.chain.id.clone();
                    if let Ok(stored) = self.store.read(move |r| r.chain(&id)).await {
                        held.set(stored);
                    }
                }
                Err(err)
            }
        }
    }

    /// What the sessions of the cards ids hold of them now.
    fn live_of(&self, ids: &[String]) -> HashMap<String, Live> {
        ids.iter()
            .filter_map(|id| {
                let r = self.run_of(id)?;
                let closed = !r.open();
                let s = r.state();
                let l = Live {
                    activity: s.card.activity.clone(),
                    ctx_used: s.card.ctx_used,
                    ctx_size: s.card.ctx_size,
                    closed,
                };
                Some((id.clone(), l))
            })
            .collect()
    }

    /// Holds stored change c in held and tells the browsers, and gives what was applied.
    fn publish(&self, held: Option<&mut Held>, c: Change) -> Applied {
        let gone = c.t.as_ref().is_some_and(|t| t.gone);
        let mut a = Applied {
            sent: c.sent,
            ..Applied::default()
        };
        if !gone {
            // a deletion stores nothing else: no message was queued
            a.queued = c.w.messages;
        }
        match (c.t, held) {
            (Some(t), Some(held)) if t.gone => {
                held.gone = true;
                held.work.cancel();
                a.chain = t.next;
                a.changed = true;
                self.notify.chain_deleted(&c.chain_id);
            }
            (Some(_), Some(held)) => {
                if let Some(saved) = c.saved {
                    held.set(saved.clone());
                    self.notify.chain_updated(&chain_read(saved.clone(), &c.wakes));
                    a.chain = saved;
                    a.changed = true;
                }
            }
            _ => {}
        }
        for e in &c.w.employees {
            self.notify.employee_updated(e);
        }
        for card in &c.edited {
            if let Some(r) = self.run_of(&card.id) {
                r.state().card = card.clone();
            }
        }
        for d in &c.w.drafts {
            self.notify.draft_deleted(d);
        }
        for h in &c.helps {
            self.notify.help_updated(h);
        }
        a.helps = c.helps;
        for card in c.created.into_iter().chain(c.edited) {
            self.notify.card_updated(&card);
            a.cards.push(card);
        }
        for e in &c.w.edges {
            self.notify.edge_added(e);
        }
        for d in &c.decisions {
            self.notify.decision_added(d);
        }
        if !c.w.approvals.is_empty() {
            // a verdict moves its chain's verification and the chip of its board card
            self.notify.views_moved(&c.chain_id);
        }
        for e in &c.added {
            self.notify.event_added(e);
        }
        for e in &c.rewritten {
            self.notify.event_updated(e);
        }
        for (e, m) in a.sent.iter().zip(&a.queued) {
            if m.again == 0 {
                self.notify.event_added(e);
            } else {
                self.notify.event_updated(e);
            }
        }
        for e in &c.dropped {
            self.notify.event_updated(e);
        }
        for it in &c.inbox {
            self.notify.inbox_updated(it);
        }
        if c.taught {
            self.kick(); // the subscriptions to the teaching judge it on the next pass
        }
        if !c.w.asked.is_empty() {
            self.kick(); // the Assistant is told on the Tick
        }
        for run in &c.runs {
            self.notify.workflow_updated(run);
        }
        for l in &c.landings {
            self.notify.landing_updated(l);
        }
        for repo in &c.repos {
            self.notify.repo_moved(repo);
        }
        a
    }
}

/// Stores change c in transaction tx and records in c what the store holds of it.
fn put(tx: &Tx<'_>, c: &mut Change) -> Result<()> {
    for check in &c.w.checks {
        check(tx)?;
    }
    if let Some(cleanup) = &c.cleanup {
        // the files it leaves are stored with it, for Delete or, after a crash, Recover to remove
        for l in withdrawn(tx.chain_landings(&c.chain_id)?, c.now) {
            let l = tx.put_landing(l)?;
            c.repos.insert(l.repo.clone());
            c.landings.push(crate::landings::landing_read(tx, l)?);
        }
        tx.delete_chain(&c.chain_id)?;
        tx.put_cleanup(cleanup)?;
        return Ok(());
    }
    let edits = read_edits(tx, &c.w.cards, &c.live, c.now)?;
    if !c.w.drafts.is_empty() {
        tx.end_drafts(&c.w.drafts, c.now)?;
    }
    if c.chain_id.is_empty() {
        c.chain_id = edits
            .first()
            .map(|card| card.home.clone())
            .or_else(|| c.w.landings.first().map(|l| l.landing.chain_id.clone()))
            .or_else(|| c.w.approvals.first().map(|a| a.chain_id.clone()))
            .or_else(|| c.w.helps.first().map(|h| h.chain.clone()))
            .or_else(|| c.w.runs.first().map(|run| run.home.clone()))
            .unwrap_or_default();
    }
    for d in &c.w.decisions {
        c.decisions.push(Decision {
            id: new_id("d_"),
            chain_id: c.chain_id.clone(),
            kind: d.kind,
            by: Some(d.by.clone()),
            text: d.text.clone(),
            why: d.why.clone(),
            card_id: d.card_id.clone(),
            created_at: c.now,
            ..Decision::default()
        });
    }
    let moved = landings_moved(tx, &c.w)?;
    c.w.readings.extend(moved);
    put_chain_records(tx, c)?;
    put_landings(tx, c)?;
    put_messages(tx, c)?;
    put_inbox_and_cards(tx, c, edits)
}

/// The cards w edits, each read in tx and changed by its edits in order; a card is read once, and the cards are in id
/// order.
fn read_edits(tx: &Tx<'_>, edits: &[CardEdit], live: &HashMap<String, Live>, now: Time) -> Result<Vec<Card>> {
    let mut by_card: BTreeMap<&str, Vec<&CardEdit>> = BTreeMap::new();
    for e in edits {
        by_card.entry(e.id.as_str()).or_default().push(e);
    }
    let mut out = Vec::with_capacity(by_card.len());
    for (id, edits) in by_card {
        let mut card = tx.card(id)?;
        let run = live.get(id);
        if let Some(l) = run {
            card.activity.clone_from(&l.activity);
            (card.ctx_used, card.ctx_size) = (l.ctx_used, l.ctx_size);
        }
        let (prev, ended) = (*card.status(), card.ended_at());
        for e in edits {
            (e.edit)(&mut card);
        }
        // a closed card that has reached an end state stays there: a turn still unwinding must not revive it
        if run.is_some_and(|l| l.closed) && matches!(prev, model::CardStatus::Stopped | model::CardStatus::Failed) {
            card.restore_status(prev, ended);
        }
        card.updated_at = now;
        out.push(card);
    }
    Ok(out)
}

/// Stores the chain c moves, what it waits for, and the edges, decisions, approvals, duels, second opinions, rounds and
/// events c records.
fn put_chain_records(tx: &Tx<'_>, c: &mut Change) -> Result<()> {
    // The signals first: a firing is recorded over its subscription as it was evaluated, before the waits the change ends
    // delete it.
    put_signals(tx, &c.door, &c.w)?;
    if let Some(t) = &c.t {
        // what a screened start's chain proved is scored once, on the change that finishes it
        let was_done = tx
            .chain(&t.next.id)
            .is_ok_and(|prev| *prev.status() == ChainStatus::Done);
        let saved = tx.put_chain(t.next.clone())?;
        if !was_done {
            crate::screen::finished_start(tx, &saved, c.now);
        }
        c.saved = Some(saved);
        c.wakes = put_waits(tx, t, c.now)?;
    }
    for e in &c.w.edges {
        tx.put_edge(e)?;
    }
    for d in &c.decisions {
        tx.put_decision(d)?;
        crate::hire_choice::accepted_hire(tx, d, c.now)?;
        crate::card_watch::proved_by(tx, d, c.now)?;
    }
    for id in &c.w.shown {
        match tx.set_judgment_used(id, model::JudgmentUse::Informed) {
            Ok(()) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    for p in &c.w.proofs {
        let outcome = model::JudgmentOutcome {
            proved: p.proved,
            source: p.source,
            at: c.now,
        };
        let stored = if p.settle {
            tx.settle_judgment_outcome(&p.judgment, &outcome)
        } else {
            tx.set_judgment_outcome(&p.judgment, &outcome)
        };
        match stored {
            Ok(()) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    for a in &c.w.approvals {
        tx.put_approval(a)?;
        let ending = crate::review_depth::Ending::Judged {
            role: a.role,
            verdict: a.verdict,
        };
        for l in tx.chain_landings(&a.chain_id)? {
            if l.repo == a.repo && l.version.change == a.change {
                crate::review_depth::ended(tx, &l, ending, c.now)?;
            }
        }
    }
    for d in &c.w.duels {
        tx.put_duel(d.clone())?;
    }
    for s in &c.w.second_opinions {
        tx.put_second_opinion(s.clone())?;
    }
    for r in &c.w.rounds {
        tx.put_round(r.clone())?;
    }
    for e in &c.w.events {
        if e.id == 0 {
            c.added.push(tx.add_event(e.clone())?);
        } else {
            c.rewritten.push(tx.update_event(e.clone())?);
        }
    }
    put_runs(tx, &c.w, c.now, &mut c.runs)
}

/// Stores the releases, then the landings, then the release pauses c writes, and records in c each landing to tell, with
/// its release as stored now (those it wrote and every one a release it wrote carries, whose release moved), and each
/// repository whose view moved.
fn put_landings(tx: &Tx<'_>, c: &mut Change) -> Result<()> {
    let mut told: BTreeMap<String, model::Landing> = BTreeMap::new();
    for r in &c.w.releases {
        let r = tx.put_release(r.clone())?;
        for l in tx.release_landings(&r.id)? {
            if r.state == model::ReleaseState::Live {
                crate::review_depth::ended(tx, &l, crate::review_depth::Ending::Held, c.now)?;
            }
            told.insert(l.id.clone(), l);
        }
        c.repos.insert(r.repo);
    }
    for lw in &c.w.landings {
        let l = tx.put_landing(lw.landing.clone())?;
        if let model::LandingState::Reverted { cause, .. } = &l.state {
            crate::review_depth::ended(tx, &l, crate::review_depth::Ending::TakenBack(*cause), c.now)?;
        }
        c.repos.insert(l.repo.clone());
        told.insert(l.id.clone(), l);
    }
    for p in &c.w.release_pauses {
        tx.put_release_pause(p)?;
        c.repos.insert(p.repo.clone());
    }
    for l in told.into_values() {
        c.landings.push(crate::landings::landing_read(tx, l)?);
    }
    Ok(())
}

/// Drops and queues the messages c changes.
fn put_messages(tx: &Tx<'_>, c: &mut Change) -> Result<()> {
    for d in &c.w.drops {
        c.dropped.extend(tx.drop_queued(&d.card_id, d.min, c.now)?);
    }
    for m in &c.w.messages {
        // after the drops: what the change sends replaces what they drop
        let e = queue_message(tx, m, c.now)?;
        // a message sent again is the teaching it was
        if m.again == 0 && put_steer_fact(tx, &c.door, &e)? {
            c.taught = true;
        }
        c.sent.push(e);
    }
    Ok(())
}

/// Stores the inbox items, employees and cards c writes, closes out the chain c finishes, and stores the helps c writes
/// with those that end with the cards' work or the chain (help.rs).
fn put_inbox_and_cards(tx: &Tx<'_>, c: &mut Change, edits: Vec<Card>) -> Result<()> {
    for it in &c.w.inbox {
        c.inbox.push(tx.put_inbox(it.clone())?);
    }
    for e in &c.w.inbox_edits {
        let mut it = match tx.inbox_item(&e.id) {
            Ok(it) if it.status == InboxStatus::Open => it,
            Ok(_) | Err(store::Error::NotFound) => continue,
            Err(err) => return Err(err.into()),
        };
        (e.edit)(&mut it);
        c.inbox.push(tx.put_inbox(it)?);
    }
    for it in &c.w.asked {
        c.inbox.push(put_asked(tx, &c.door, it.clone())?);
    }
    if c.t.as_ref().is_some_and(|t| t.closed_out_by.is_some()) {
        let (dropped, cancelled) = close_out(tx, &c.chain_id, c.now)?;
        c.dropped.extend(dropped);
        c.inbox.extend(cancelled);
        stop_runs_of(tx, &c.chain_id, "the chain finished", c.now, &mut c.runs)?;
    }
    put_employees(tx, &mut c.w.employees)?;
    for card in &c.w.created {
        c.created.push(tx.put_card(card.clone())?);
    }
    // last: a card is stored with its queue as the change leaves it (its queued count)
    for card in edits {
        c.edited.push(tx.put_card(card)?);
    }
    let finished =
        c.t.as_ref()
            .is_some_and(|t| t.closed_out_by.is_some())
            .then_some(c.chain_id.as_str());
    c.helps = put_helps(tx, &c.w.helps, &c.w.ended, finished, c.now)?;
    Ok(())
}

/// Drops what a finished chain's cards had queued and cancels the questions and errors that wait on it, and returns
/// both as stored. Nothing waits on a finished chain; a permission request dies with its agent's session instead.
fn close_out(tx: &Tx<'_>, chain_id: &str, now: Time) -> Result<(Vec<Event>, Vec<InboxItem>)> {
    let dropped = tx.drop_chain_queue(chain_id, now)?;
    let mut cancelled = Vec::new();
    for mut it in tx.open_inbox()? {
        if it.home == chain_id && it.kind != InboxKind::Permission {
            it.status = InboxStatus::Cancelled;
            "the chain has finished".clone_into(&mut it.answer);
            cancelled.push(tx.put_inbox(it)?);
        }
    }
    Ok((dropped, cancelled))
}

/// Stores the employees of a change in tx, a new one taking from the store the number it gives in that write:
/// employees holds them as stored after (ADR-0061).
fn put_employees(tx: &Tx<'_>, employees: &mut [Employee]) -> Result<()> {
    for e in employees.iter_mut() {
        *e = tx.put_employee(e.clone())?;
    }
    Ok(())
}

/// Stores, in tx, the helps a change writes, each over the revision it carries, and the timer of each: placed while it
/// is offered, ended once it is not. Then it ends every open help of the cards whose work the change ends and, when the
/// change finishes chain finished, of that chain. The owner mentors his Assistant as long as it is, so that help stays.
/// It returns the helps as stored.
pub fn put_helps(
    tx: &Tx<'_>,
    written: &[Help],
    ended: &[String],
    finished: Option<&str>,
    now: Time,
) -> Result<Vec<Help>> {
    let mut stored = Vec::new();
    let mut put = |h: Help| -> Result<()> {
        let h = tx.put_help(h)?;
        if h.state == HelpState::Offered {
            if h.rev == 1 {
                // a new offer: its timer is placed with it
                let from = Start {
                    from: StartFrom::Now,
                    ..Start::default()
                };
                subscribe(tx, help_timer(&h, now), &from, now)?;
            }
        } else {
            match tx.delete_subscription(&help_deadline_id(&h.id)) {
                Ok(()) | Err(store::Error::NotFound) => {}
                Err(e) => return Err(e.into()),
            }
        }
        stored.push(h);
        Ok(())
    };
    for h in written {
        put(h.clone())?;
    }
    let open = match finished {
        Some(chain) => tx.chain_helps(chain)?,
        None => {
            let mut open = Vec::new();
            for id in ended {
                open.extend(tx.card_helps(id)?);
            }
            open
        }
    };
    for mut h in open {
        if !h.open() || h.id == OWNER_HELP_ID {
            continue;
        }
        (h.state, h.ended_at, h.why) = (HelpState::Ended, Some(now), "the card's work ended".to_owned());
        put(h)?;
    }
    Ok(stored)
}

/// Ingests the readings a change records and stores the firing it is the effect of done (`Delivered::store`). A firing
/// done already means its effect was done then (`Error::FiredAlready`), so nothing of this change is stored.
pub fn put_signals(tx: &Tx<'_>, door: &Door, w: &Records) -> Result<()> {
    for r in &w.readings {
        door.ingest(tx, r.clone())?;
    }
    if let Some(fired) = &w.fired {
        fired.store(tx)?;
    }
    Ok(())
}

/// Stores the review threads w writes, those the owner's words were carried on included (each naming sent's last
/// What of a deleted chain's landings, landings, is withdrawn at now: each still on its way, so it never lands. What
/// landed stays the repository's history, and a revert still on its way lands: taking a change back is how a landed
/// one goes (ADR-0114).
fn withdrawn(landings: Vec<model::Landing>, now: Time) -> impl Iterator<Item = model::Landing> {
    landings
        .into_iter()
        .filter(|l| l.state.open() && l.reverts.is_none())
        .map(move |l| l.returned(model::ReturnReason::Withdrawn, "its chain was deleted".to_owned(), now))
}

/// The readings of the landings w writes whose word moved (live.rs `landing_moved`), each against the landing as stored
/// and the chain's verdicts with those w adds, read in tx.
fn landings_moved(tx: &Tx<'_>, w: &Records) -> Result<Vec<crate::ingest::Write>> {
    let mut out = Vec::new();
    for lw in &w.landings {
        let l = &lw.landing;
        let mut approvals = tx.approvals(&l.chain_id)?;
        approvals.extend(w.approvals.iter().filter(|a| a.chain_id == l.chain_id).cloned());
        let was = match tx.landing(&l.id) {
            Ok(was) => Some(crate::live::landing_value_of(&was, &approvals)),
            Err(store::Error::NotFound) => None,
            Err(err) => return Err(err.into()),
        };
        let now = crate::live::landing_value_of(l, &approvals);
        out.extend(crate::live::landing_moved(l, was.as_ref(), &now, &lw.cause)?);
    }
    Ok(out)
}

/// The subscription that tells the Chair when offer h is not answered by its deadline, a schedule that acts once, then:
/// Remit's own, for h's chain, so it ends with the chain.
fn help_timer(h: &Help, now: Time) -> Subscription {
    let sub = Subscription {
        id: help_deadline_id(&h.id),
        owner: owned_by(&h.chain),
        action: SubscriptionAction {
            kind: ActionKind::Deadline,
            subject: h.id.clone(),
            why: "Tell the Chair when a mentor has not answered its call for help in time.".to_owned(),
            ..SubscriptionAction::default()
        },
        by: by_remit(),
        created_at: now,
        updated_at: now,
        ..Subscription::default()
    };
    once_at(sub, h.deadline)
}

/// The id of the deadline subscription of help id.
pub fn help_deadline_id(id: &str) -> String {
    format!("sub_deadline_{id}")
}

/// The help the owner gives his Assistant: it lasts as long as the Assistant does.
pub const OWNER_HELP_ID: &str = "help_owner";
