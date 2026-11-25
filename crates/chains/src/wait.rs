//! A Chair whose result can only be judged later calls subscribe: it subscribes its chain to a time, or to a signal with a
//! condition and a deadline (a watch, watch.rs; ADR-0014). The chain waits without a slot or a session, and when the time
//! comes or a reading meets the condition the subscription fires and the Chair's own session resumes. The subscriptions
//! are stored with the chain, so a restart loses nothing.

use std::collections::HashMap;
use std::sync::Arc;

use model::{
    ActionKind, Actor, ActorKind, Card, CardStatus, Chain, ChainStatus, OwnerKind, Subscription, SubscriptionOwner,
    Time,
};
use time::macros::format_description;
use wire::app::{ChainRead, CheckBack, Watch};

use crate::Result;
use crate::apply::Records;
use crate::chain_core::Core;
use crate::request::Service;
use crate::request::{Done, Request};
use crate::schedule::at_work;
use crate::step::{ChainCmd, CheckNowCmd, Need, TurnEndedCmd, WakeCmd};
use crate::submit::submitting;
use crate::watch::watch_wake;
use model::Cut;
use model::Help;

/// A wake subscription with the instant it comes due (`due_of`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WakeSub {
    pub sub: Subscription,
    /// When it comes due; none for one with no time, which comes after every one with a time.
    pub at: Option<Time>,
}

/// The wake subscriptions a chain holds: what it waits for. apply reads them under the chain's lock and hands them to
/// step, so a chain waits while it holds one, or with none while only its mentors' helps are open, whose messages wake it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WakeSubs(pub Vec<WakeSub>);

impl WakeSubs {
    /// The wake subscription whose time or deadline comes first.
    pub fn earliest(&self) -> Option<WakeSub> {
        self.0.iter().min_by_key(|s| (s.at.is_none(), s.at)).cloned()
    }

    /// The wakes at a time: a Chair's check-backs.
    pub fn check_backs(&self) -> WakeSubs {
        self.only(|s| s.sub.schedule.is_some())
    }

    /// The wake subscription with this id.
    pub fn by_id(&self, id: &str) -> Option<WakeSub> {
        self.0.iter().find(|s| s.sub.id == id).cloned()
    }

    pub fn only(&self, keep: impl Fn(&WakeSub) -> bool) -> WakeSubs {
        WakeSubs(self.0.iter().filter(|s| keep(s)).cloned().collect())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, WakeSub> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// When s comes due with no new reading, as the owner reads it: the next instant of its schedule, or its deadline; none
/// when it has neither.
pub fn due_of(s: &Subscription) -> Option<Time> {
    if s.schedule.is_some() { s.due_at } else { s.until }
}

/// The owner of the subscriptions of chain id.
pub fn wait_owner(id: &str) -> SubscriptionOwner {
    SubscriptionOwner {
        kind: OwnerKind::Chain,
        id: id.to_owned(),
    }
}

/// The wake subscriptions among subs, each with when it comes due.
pub fn wakes_in(subs: Vec<Subscription>) -> WakeSubs {
    WakeSubs(
        subs.into_iter()
            .filter(|s| s.action.kind == ActionKind::Wake)
            .map(|sub| WakeSub { at: due_of(&sub), sub })
            .collect(),
    )
}

/// When the Chair of a chain that holds the wake subscriptions w comes back, as the owner's pages read it: its earliest
/// wake subscription, a time or the deadline of a watch, and the watch that ends first when it holds any. The one place
/// the owner's pages learn when a chain comes back and what it watches.
pub fn wake_of(w: &WakeSubs) -> Option<CheckBack> {
    let sub = w.earliest()?;
    let watch = w.watches().earliest().and_then(|first| {
        Some(Watch {
            key: first.sub.r#match.as_str().to_owned(),
            condition: Some(first.sub.expr.clone()).filter(|e| !e.is_empty()),
            until: first.sub.until?,
            why: first.sub.action.why.clone(),
        })
    });
    Some(CheckBack {
        at: sub.at?,
        why: sub.sub.action.why.clone(),
        set_at: sub.sub.created_at,
        watch,
    })
}

/// When the Chair of each chain that holds wake subscriptions comes back, by chain id, from subs of any owner.
pub fn wakes_by(subs: Vec<Subscription>) -> HashMap<String, CheckBack> {
    let mut by: HashMap<String, WakeSubs> = HashMap::new();
    for s in subs {
        if s.owner.kind == OwnerKind::Chain && s.action.kind == ActionKind::Wake {
            by.entry(s.owner.id.clone())
                .or_default()
                .0
                .push(WakeSub { at: due_of(&s), sub: s });
        }
    }
    by.into_iter()
        .filter_map(|(id, w)| wake_of(&w).map(|wake| (id, wake)))
        .collect()
}

/// Chain c as a channel reads it, with when its Chair comes back from wakes.
pub fn chain_read(c: Chain, wakes: &HashMap<String, CheckBack>) -> ChainRead {
    let check_back = wakes.get(&c.id).cloned();
    ChainRead { chain: c, check_back }
}

/// The title and the text the Chair reads when its wake subscription sub fires; ended are the background tasks that ended
/// when the chain waited.
pub fn wake_message(sub: &WakeSub, cmd: &WakeCmd, ended: &[String], now: Time) -> (String, String) {
    if sub.sub.watch() {
        let (title, text) = watch_wake(&sub.sub, cmd, now);
        return (title, format!("{text}{}", ended_note(ended)));
    }
    (
        "Check back as planned".to_owned(),
        format!(
            "[Remit: checking back as you planned at {}] {}\nCheck what you were waiting for, then continue: route work, subscribe again, or finish_chain.{}",
            clock(sub.at.unwrap_or(now), now),
            sub.sub.action.why,
            ended_note(ended)
        ),
    )
}

/// Tells the Chair which background tasks of its agents ended with the sessions the wait closed; empty when none did.
fn ended_note(ended: &[String]) -> String {
    if ended.is_empty() {
        return String::new();
    }
    format!(
        "\nWhile the chain waited its sessions closed, and what your agents ran in the background ended with them: {}. Nothing watches for you meanwhile: subscribe to what you wait for, with a time to look again.",
        ended.join("; ")
    )
}

/// When t is, in the server's time zone (the offset the clock gives now in): the time of day, with the date when it is not
/// today.
pub fn clock(t: Time, now: Time) -> String {
    let t = t.0.to_offset(now.0.offset());
    let format = if t.date() == now.0.date() {
        t.format(format_description!("[hour]:[minute]"))
    } else {
        t.format(format_description!("[day]/[month] [hour]:[minute]"))
    };
    format.unwrap_or_default()
}

/// t in RFC 3339: seconds, no fraction, Z for UTC.
pub fn rfc3339(t: Time) -> String {
    let base =
        t.0.format(format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]"))
            .unwrap_or_default();
    let offset = t.0.offset();
    if offset.is_utc() {
        return format!("{base}Z");
    }
    let zone = offset
        .format(format_description!("[offset_hour sign:mandatory]:[offset_minute]"))
        .unwrap_or_default();
    format!("{base}{zone}")
}

impl Core {
    /// The wake subscriptions chain id holds.
    pub async fn waits_of(&self, id: &str) -> Result<WakeSubs> {
        let owner = wait_owner(id);
        let subs = self.store.read(move |r| r.subscriptions_of(&owner)).await?;
        Ok(wakes_in(subs))
    }

    /// The owner waking a waiting chain's Chair before its wake subscription is due. Admission does not hold it to a slot,
    /// like any wake the owner asks for.
    pub fn wake_now(
        self: &Arc<Self>,
        req: Request,
        chain_id: String,
    ) -> impl std::future::Future<Output = Result<Done>> + Send + use<> {
        let by = req.by.clone();
        self.act_on_chain(req, "WakeNow", chain_id, move |o, id| async move {
            o.apply(&id, ChainCmd::CheckNow(CheckNowCmd { by }), Records::default())
                .await
                .map(|_| ())
        })
    }

    /// Records the end of card's turn (edit) and settles the wait the Chair planned, together: the chain waits, its
    /// sessions closed and its slot free, or, when something still needs the Chair, the wait is cancelled and the Chair
    /// told why. When the wait cannot be settled, the turn's end is recorded alone: the card is never left in a turn
    /// that is over.
    pub async fn settle_wait(&self, card: &Card, w: Records) {
        let cmd = ChainCmd::TurnEnded(TurnEndedCmd {
            card: card.id.clone(),
            ..TurnEndedCmd::default()
        });
        match self.apply(&card.home, cmd, w.clone()).await {
            Ok(a) if a.changed => return,
            Ok(_) => {} // no wait to settle
            Err(err) => {
                tracing::warn!(chain = %card.home, card = %card.id, %err, "wait not settled: the turn's end is recorded alone");
            }
        }
        match self.commit(None, None, w).await {
            Ok(a) => self.hand_over(&a).await,
            Err(err) => tracing::error!(chain = %card.home, card = %card.id, %err, "turn end"),
        }
    }

    /// What still needs the Chair, under the chain's lock: a message or a report sent after it is stored finds the chain
    /// waiting and wakes it. With nothing to wait for, a chain whose mentors are helping it waits on them.
    pub async fn observe_turn_ended(&self, mut c: TurnEndedCmd, ch: &Chain, w: &WakeSubs) -> TurnEndedCmd {
        if *ch.status() != ChainStatus::Running {
            return c;
        }
        let chair = ch.chair_id.clone();
        match self.store.read(move |r| r.queue(&chair)).await {
            Ok(queue) if queue.is_empty() => {}
            _ => {
                c.need = Need::MessageWaits;
                return c;
            }
        }
        let (busy, mentors) = self.others_busy(ch, &c.card).await;
        if busy {
            c.need = Need::OthersWork;
        } else if w.is_empty() {
            c.mentors = mentors;
            (c.runs_rest, c.rest_until) = self.runs_resting(&ch.id, None, None).await;
        }
        c
    }

    /// Subscribes the Chair's chain to a time or a watch (input), in place of any wake it held: the chain keeps running
    /// until the Chair's turn ends, then waits. Gives when it is due.
    pub async fn subscribe(
        &self,
        chair: &model::Card,
        input: &crate::watch::SubscribeInput,
        now: Time,
    ) -> Result<Time> {
        if chair.origin != model::Origin::Chair {
            return Err(crate::Error::refused("only the Chair can subscribe"));
        }
        if input.at.is_empty() == input.key.is_empty() {
            return Err(crate::Error::refused(
                "give either at, a time to look again, or key, a signal to watch with until: not both and not neither",
            ));
        }
        let ch = self.chain(&chair.home).await?;
        let sub = self.subscription_of(&ch, chair, input, now).await?;
        if *ch.status() != model::ChainStatus::Running {
            return Err(crate::Error::refused(format!("the chain is {}", ch.status())));
        }
        if self.others_busy(&ch, "").await.0 {
            return Err(crate::Error::refused(
                "other agents are still working or a submit is running: wait for their reports or stop them, then subscribe",
            ));
        }
        let due =
            due_of(&sub).ok_or_else(|| crate::Error::refused("the subscription names no time to come back at"))?;
        self.apply(
            &ch.id,
            ChainCmd::Subscribe(crate::step::SubscribeCmd { sub }),
            Records::default(),
        )
        .await?;
        Ok(due)
    }

    /// Whether anything in the chain besides the Chair and card (whose turn is ending, or "") is still at work: an agent
    /// in a turn or starting, a message queued for one, or a submit running. A failed agent is not at work: its messages
    /// wait for the owner's retry. Nor is a mentor: mentors names instead the mentors of the chain's open helps (ADR-0062).
    /// Unread, the chain is busy.
    pub async fn others_busy(&self, ch: &Chain, card: &str) -> (bool, Vec<String>) {
        if submitting(ch) {
            return (true, Vec::new());
        }
        let id = ch.id.clone();
        // The queue is read before the cards: an agent's message leaves the queue in the transaction that sets it running,
        // so one read before the other cannot miss it between the two.
        let Ok((queued, cards)) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.queued_prompts()?, r.cards(&id)?)))
            .await
        else {
            return (true, Vec::new());
        };
        let mut failed = Vec::new();
        for c in cards.iter().filter(|c| c.id != ch.chair_id && c.id != card) {
            if c.status() == &CardStatus::Failed {
                failed.push(c.id.clone());
            } else if at_work(c) {
                return (true, Vec::new());
            }
        }
        if queued
            .iter()
            .any(|e| e.home == ch.id && e.card_id != ch.chair_id && !failed.contains(&e.card_id))
        {
            return (true, Vec::new());
        }
        match self.helping_now(&ch.id).await {
            Ok(mentors) => (false, mentors),
            Err(_) => (true, Vec::new()),
        }
    }

    /// The background tasks that ended when the chain waited, for the wake message, under the chain's lock.
    pub async fn observe_wake(&self, mut c: WakeCmd, ch: &Chain, w: &WakeSubs) -> WakeCmd {
        if let Some(held) = w.by_id(&c.sub.id).filter(|_| *ch.status() == ChainStatus::Waiting) {
            c.ended = self.ended_while_waiting(&ch.id, held.sub.created_at).await;
        }
        c
    }

    /// The background tasks of chain id's agents that ended with the sessions closed for the wait that began at since,
    /// each with its agent.
    async fn ended_while_waiting(&self, chain_id: &str, since: Time) -> Vec<String> {
        let id = chain_id.to_owned();
        let Ok((tasks, cards)) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.stopped_tasks(&id, since)?, r.cards(&id)?)))
            .await
        else {
            return Vec::new();
        };
        tasks
            .iter()
            .filter(|e| Cut::of(e.data.as_ref()) == Some(Cut::Slept))
            .map(|e| {
                let name = cards
                    .iter()
                    .find(|c| c.id == e.card_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or_default();
                format!("{} ({name})", e.title)
            })
            .collect()
    }

    /// Who woke a chain early, for the owner.
    pub async fn sender(&self, by: &Actor, to: &Card) -> String {
        match by.kind {
            ActorKind::Owner => return format!("you messaged {}", to.name),
            ActorKind::Assistant => return format!("the Assistant messaged {} for you", to.name),
            ActorKind::Client => {
                return format!("{} messaged {}", self.client_key_label(&by.who).await, to.name);
            }
            ActorKind::Remit => return "Remit".to_owned(),
            ActorKind::Mentor => return format!("a mentor messaged {}", to.name),
            _ => {}
        }
        let who = by.who.clone();
        match self.store.read(move |r| r.card(&who)).await {
            Ok(c) => c.name,
            Err(_) => by.who.clone(),
        }
    }
}

impl Core {
    /// The mentors whose help on chain chain_id is open, each once, the oldest offer first: a chain with nothing else at
    /// work waits on them, holding no slot, until a message of theirs or an answer wakes it.
    pub async fn helping_now(&self, chain_id: &str) -> Result<Vec<String>> {
        let id = chain_id.to_owned();
        Ok(self
            .store
            .read(move |r| -> store::Result<Vec<String>> {
                let mut names: Vec<String> = Vec::new();
                for h in r.chain_helps(&id)?.into_iter().filter(Help::open) {
                    let label = r.mentor(&h.mentor)?.label;
                    if !names.contains(&label) {
                        names.push(label);
                    }
                }
                Ok(names)
            })
            .await?)
    }
}
