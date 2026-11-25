//! The teaching record (docs/research-agent-learning/design.md D1, D3): each time a person or a verdict teaches an
//! employee, Remit records a fact fact:teach.<kind> naming the employee taught and its card, the teacher as the reading's
//! cause. The facts are the durable record any later learner reads again; what the librarian is let learn from each
//! is the firing of a subscription that ships with Remit (teach_memory.rs), so the coach's own subscriptions to the same
//! keys come beside it.
//!
//! The step that teaches stores its fact in its own transaction: the Chair's verdict on a card (accept_work), the goal
//! judge's verdict on the Chair's (goal.rs), the coach's retrospective of a finished chain, taught to its Chair (coach.rs),
//! the owner's overturn of his Assistant's answer. A mentor's message, the owner's among them (he is the mentor m_owner, ADR-0062), is recorded in the transaction
//! that queues it (`put_steer_fact` in chains' fact.rs), so a message sent again is the same teaching: the fact names the
//! message's event, whose id the store gives only as the message is written, and the mentor who said it, whose own memory
//! learns what they teach (teach_memory.rs). A person marking an agent's reply
//! (helpful, not helpful, corrected, ADR-0093) is a teaching of the card that replied, by the owner or a mentor of it; the
//! reply shows each person's mark as it stands (`remarked`), which their new mark replaces, so the reply and its fact
//! change in one transaction.

use std::sync::Arc;

use model::{Actor, ActorKind, Card, Decision, Event, EventKind, ForeignJson, Time, by_mentor};
use wire::app::{ReplyMark, TurnMark};

use crate::Crew;
use crate::help::{WorkerRequest, mentor_reads};
use chains::activity::marks_of;
use chains::fact::{FactValue, fact_key, fact_write};
use chains::ingest::Write;
use chains::owner_mentor::OWNER_MENTOR_ID;
use chains::request::Service;
use chains::request::{Done, Request};
use chains::wait::rfc3339;
use chains::{Error, Result};

/// The fact that the coach's retrospective retro of chain c teaches c's Chair, chair, by the coach's card coach; once per
/// retrospective, so writing it again teaches nothing twice. Pure.
pub(crate) fn taught_retrospective(c: &model::Chain, chair: &Card, retro: &str, coach: &Actor) -> Result<Write> {
    let v = FactValue {
        chain: c.id.clone(),
        card: chair.id.clone(),
        employee: chair.employee.clone(),
        retrospective: retro.to_owned(),
        ..FactValue::default()
    };
    let mut w = fact_write(fact_key("teach", "retrospective"), &v, coach.clone(), &c.id)?;
    w.dedupes = vec![format!("retrospective:{retro}")];
    Ok(w)
}

/// Reply e of card c as it shows by's mark on it, with the fact of that teaching; none when the mark stands as it is
/// already (the same mark with the same words counts once). by marks as mentor (the owner as `m_owner`): one person's mark
/// on a reply is one record, and their new mark replaces it, a new teaching of the reply's next revision. Only the card's
/// own agent's reply is marked, a message its session wrote: one Remit posted on the card (a digest) names its writer.
/// A correction says the correct reply. Pure.
pub(crate) fn remarked(
    e: &Event,
    c: &Card,
    mark: TurnMark,
    words: &str,
    by: &Actor,
    mentor: String,
    at: Time,
) -> Result<Option<(Event, Write)>> {
    if e.kind() != EventKind::Message || e.by.is_some() {
        return Err(Error::refused(format!(
            "event {} is no reply of the agent's own: only what its session wrote is marked",
            e.id
        )));
    }
    if mentor.is_empty() {
        return Err(Error::refused("only the owner or a mentor marks a reply"));
    }
    let words = words.trim();
    if mark == TurnMark::Correct && words.is_empty() {
        return Err(Error::refused("say the correct reply"));
    }
    let mut marks = marks_of(e);
    if marks
        .iter()
        .any(|m| m.mentor == mentor && m.mark == mark && m.words == words)
    {
        return Ok(None);
    }
    marks.retain(|m| m.mentor != mentor);
    marks.push(ReplyMark {
        mentor: mentor.clone(),
        by: by.clone(),
        mark,
        words: words.to_owned(),
        at: rfc3339(at),
    });
    let mut shown = e.clone();
    shown.data = Some(ForeignJson(serde_json::json!({ "marks": marks })));
    shown.updated_at = at;
    let v = FactValue {
        chain: e.home.clone(),
        card: c.id.clone(),
        employee: c.employee.clone(),
        event: e.id,
        mentor,
        mark: Some(mark),
        why: words.to_owned(),
        ..FactValue::default()
    };
    let mut w = fact_write(fact_key("teach", "reply"), &v, by.clone(), &e.home)?;
    w.dedupes = vec![format!("reply:{}:{}", e.id, e.rev)]; // each change of the reply's marks is one teaching
    Ok(Some((shown, w)))
}

impl Crew {
    /// The fact that the owner's overturn ov of decision d taught the card that decided d: the Assistant's.
    pub async fn taught_overturn(&self, ov: &Decision, d: &Decision) -> Result<Write> {
        let Some(by) = d.by.as_ref() else {
            return Err(Error::refused(format!("decision {} names no one who decided it", d.id)));
        };
        let id = by.who.clone();
        let c = self
            .store
            .read(move |r| r.card(&id))
            .await
            .map_err(|e: store::Error| Error::refused(format!("the card that decided {}: {e}", d.id)))?;
        let v = FactValue {
            chain: ov.chain_id.clone(),
            decision: ov.id.clone(),
            item: d.item.clone(), // the kind of what it decided, for the coach's overturn_rates
            card: c.id,
            employee: c.employee,
            mentor: OWNER_MENTOR_ID.to_owned(),
            ..FactValue::default()
        };
        let teacher = chains::lifecycle::or_default_actor(ov.by.as_ref());
        fact_write(fact_key("teach", "overturn"), &v, teacher, &ov.chain_id)
    }

    /// The mentor by is: m_owner for the owner, the mentor of the help a mentor's message comes through; none for anyone
    /// else, or a help no longer on record (logged).
    pub(crate) async fn mentor_of(&self, by: &Actor) -> String {
        match by.kind {
            ActorKind::Owner => OWNER_MENTOR_ID.to_owned(),
            ActorKind::Mentor => {
                let help = by.who.clone();
                match self.store.read(move |r| r.help(&help)).await {
                    Ok(h) => h.mentor,
                    Err(err) => {
                        tracing::warn!(help = %by.who, %err, "teach: the help a mentor's message came through is not on record");
                        String::new()
                    }
                }
            }
            _ => String::new(),
        }
    }

    /// The owner marks the agent's reply event_id with mark, saying words (ADR-0093).
    pub async fn teach_turn(
        self: &Arc<Self>,
        req: Request,
        event_id: i64,
        mark: TurnMark,
        words: String,
    ) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "TeachTurn", move |o| async move {
            o.mark_reply(event_id, mark, &words, by).await
        })
        .await
    }

    /// Key key_id's mentor marks the agent's reply event_id with mark, saying words: a reply on a card they help, once
    /// they read its session.
    pub async fn mentor_teach_turn(
        self: &Arc<Self>,
        request_id: &str,
        key_id: &str,
        event_id: i64,
        mark: TurnMark,
        words: String,
    ) -> Result<Done> {
        // one refusal for a reply that is not there and one on a card they do not help: neither names another's card
        let not_yours = || Error::refused(format!("you help no agent that wrote reply {event_id}"));
        let e = match self.store.read(move |r| r.event(event_id)).await {
            Ok(e) => e,
            Err(store::Error::NotFound) => return Err(not_yours()),
            Err(err) => return Err(err.into()),
        };
        let (k, h) = self.help_on_card(key_id, &e.card_id).await.map_err(|err| match err {
            Error::Refused(_) => not_yours(),
            err => err,
        })?;
        let req = WorkerRequest {
            id: request_id.to_owned(),
            key_id: k.id,
            help_id: h.id,
        };
        let args = (event_id, mark, words.clone());
        self.worker_act(&req, "MentorTeachTurn", &args, move |o, _, h| async move {
            if !mentor_reads(&h) {
                return Err(Error::refused(format!(
                    "this help is {}: you mark the agent's replies once you accepted to help it",
                    h.state
                )));
            }
            o.mark_reply(event_id, mark, &words, by_mentor(&h.id)).await?;
            Ok(Done {})
        })
        .await
    }

    /// Shows by's mark on reply event_id on the reply and records it as the teaching `teach.reply`, in one change, then
    /// kicks the scheduler to judge it. A reply its agent is still writing is refused: the stream writes over it.
    async fn mark_reply(&self, event_id: i64, mark: TurnMark, words: &str, by: Actor) -> Result<()> {
        let e = self
            .store
            .read(move |r| r.event(event_id))
            .await
            .map_err(|err: store::Error| Error::refused(format!("reply {event_id}: {err}")))?;
        let writing = self
            .run_of(&e.card_id)
            .is_some_and(|r| r.state().stream.as_ref().is_some_and(|s| s.id == event_id));
        if writing {
            return Err(Error::refused(format!(
                "the agent is still writing reply {event_id}: mark it once it is done"
            )));
        }
        let mentor = self.mentor_of(&by).await;
        let (at, door, words) = (self.now(), self.door(), words.to_owned());
        let shown = self
            .in_home(&e.home, || async {
                self.store
                    .update(move |tx| -> Result<Option<Event>> {
                        let e = tx.event(event_id)?;
                        let c = tx.card(&e.card_id)?;
                        let Some((shown, w)) = remarked(&e, &c, mark, &words, &by, mentor, at)? else {
                            return Ok(None);
                        };
                        let shown = tx.update_event(shown)?;
                        door.ingest(tx, w)?;
                        Ok(Some(shown))
                    })
                    .await
            })
            .await?;
        if let Some(e) = shown {
            self.notify.event_updated(&e);
            self.kick();
        }
        Ok(())
    }
}
