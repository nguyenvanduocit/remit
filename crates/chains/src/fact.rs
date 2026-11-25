//! A fact is what Remit records when its own state changes: a reading of fact:<noun>.<past-tense verb> (ADR-0035). The
//! step that changes the state ingests the reading in its own transaction, so a fact exists exactly when the state does,
//! and kicks the scheduler after the commit; the subscriptions that answer it (the shipped ones in automation.rs, and any
//! the owner tells the Assistant to make) fire from it on the Tick, as they fire from any reading. One key serves every
//! chain, with the chain in the value, so each subscription keeps one cursor over one key.

use model::{
    Actor, Cause, Chain, ChainStatus, FACT_KEY_PREFIX, FiringKey, InboxItem, Reading, SignalKey, Start, StartFrom,
    Writer, WriterKind,
};
use serde::{Deserialize, Serialize};
use store::Tx;

use crate::chain_core::Core;
use crate::goal::{goal_deadline, goal_deadline_id};
use crate::ingest::{Door, Write};
use crate::lifecycle::or_default_actor;
use crate::signal_sources::fact_noun_of;
use crate::subscribe::subscribe;
use crate::{Error, Result};
use model::{Card, ReportVerdict};

/// The key of the fact noun.verb: fact:chain.queued.
pub fn fact_key(noun: &str, verb: &str) -> SignalKey {
    SignalKey(format!("{FACT_KEY_PREFIX}{noun}.{verb}"))
}

/// The facts Remit records.
pub mod keys {
    use model::SignalKey;

    use super::fact_key;

    pub fn chain_queued() -> SignalKey {
        fact_key("chain", "queued")
    }
    pub fn finish_requested() -> SignalKey {
        fact_key("chain", "finish-requested")
    }
    pub fn check_stated() -> SignalKey {
        fact_key("check", "stated")
    }
    /// A chain's change landed on its base (ADR-0114): the landing, its repository, base, commit and paths.
    pub fn chain_landed() -> SignalKey {
        fact_key("chain", "landed")
    }
    /// A repository's base moved: what landed there, by which chain, and the paths it changed (ADR-0114), so a chain
    /// whose own paths overlap is told.
    pub fn repo_landed() -> SignalKey {
        fact_key("repo", "landed")
    }
    /// A repository's release published, went live or was taken back (ADR-0116): shipping's `ReleaseFact`.
    pub fn repo_release() -> SignalKey {
        fact_key("repo", "release")
    }
    /// The chain finished, by whoever finished it (ADR-0082).
    pub fn chain_finished() -> SignalKey {
        fact_key("chain", "finished")
    }
    /// A session recalled memories of its employee (ADR-0082): which, for which card.
    pub fn memory_recalled() -> SignalKey {
        fact_key("memory", "recalled")
    }
    /// A session said a memory it was given helped or misled it (ADR-0117): which, with what it found.
    pub fn memory_used() -> SignalKey {
        fact_key("memory", "used")
    }
    /// How well memory is recalled (ADR-0117): what the daily review measured of the week's recalls and of the frozen
    /// questions.
    pub fn memory_recall_quality() -> SignalKey {
        fact_key("memory", "recall-quality")
    }
    pub fn owner_asked() -> SignalKey {
        fact_key("owner", "asked")
    }
    pub fn chain_marked_live() -> SignalKey {
        fact_key("chain", "marked-live")
    }
    /// A firing failed for good and holds its subscription.
    pub fn subscription_failed() -> SignalKey {
        fact_key("subscription", "failed")
    }
    /// Its owner's agent lifted the hold.
    pub fn subscription_resumed() -> SignalKey {
        fact_key("subscription", "resumed")
    }
    /// How this server stands (ADR-0087): remit_facts.rs's `RemitHealth`.
    pub fn remit_health() -> SignalKey {
        fact_key("remit", "health")
    }
    /// A new issue among this server's errors, one per issue however often it is logged (ADR-0119): remit_facts.rs's
    /// `RemitError`.
    pub fn remit_error() -> SignalKey {
        fact_key("remit", "error")
    }
    /// What a run of `remit update` did: `RemitUpdated`.
    pub fn remit_updated() -> SignalKey {
        fact_key("remit", "updated")
    }
    /// A card's turn ended: whose, who opened it, how it ended and what it answered (`turn_ended_fact`).
    pub fn turn_ended() -> SignalKey {
        fact_key("turn", "ended")
    }
}

/// What a reading of a fact says: the ids of what it is about, never words of the work, but for turn.ended's reply: a
/// plugin's processor reads only the reading it is fired on, so the reply it is to process travels in it, cut.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FactValue {
    pub chain: String,
    /// check.stated: the key result whose check was stated.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub key_result: String,
    /// owner.asked: the inbox item that asks him.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub item: String,
    /// teach.*: the employee taught and its card; what taught it is the reading's cause. memory.recalled: the employee
    /// whose session recalled, and its card.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub employee: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub card: String,
    /// memory.recalled: the ids of the memories the recall returned, best first. memory.used: the memory rated.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<String>,
    /// memory.recalled: the session searched for them (search_memory); otherwise they were given it unasked.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub searched: bool,
    /// memory.used: helped or misled, and what the agent found.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub memory_use: String,
    /// teach.verdict: the Chair's ruling on the card's report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<ReportVerdict>,
    /// teach.reply: the person's mark on the reply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<wire::app::TurnMark>,
    /// teach.steer: the mentor's message. teach.reply: the reply marked.
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub event: i64,
    /// teach.steer, teach.overturn, teach.reply: the mentor who taught, whose own memory learns what they teach.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mentor: String,
    /// teach.retrospective: the coach's retrospective of the chain, which teaches its Chair.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub retrospective: String,
    /// teach.overturn, teach.duel: the owner's overturn.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub decision: String,
    /// chain.landed, repo.landed: the landing, its repository and base, the commit it landed as and the paths it changed.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub landing: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub repo: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub base: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub commit: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// teach.duel: the duel, the card whose report won as it stands, and the other side's card and employee.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub duel: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub winner: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rival: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rival_employee: String,
    /// subscription.failed, subscription.resumed: the subscription, the firing it is held on, and the error it failed
    /// with; for a resume how its hold was lifted (retry or skip) and why.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subscription: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firing: Option<FiringKey>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub how: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub why: String,
    /// turn.ended: what the card is (`role_name`), the kind of who opened the turn (owner, assistant, mentor, agent,
    /// remit), how it ended, and its reply (or for a failed turn what failed), cut at `TURN_TEXT_MAX` bytes, cut saying
    /// so. event is the message the turn answered, card and employee whose turn it was.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub role: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub opener: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<TurnEnding>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cut: bool,
}

/// How a card's turn ended: its agent ended it, it was cancelled, or it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnEnding {
    Done,
    Stopped,
    Failed,
}

/// The most bytes of a turn's reply fact:turn.ended carries. A turn's last message is most often shorter, and a reading
/// is kept for the 7 days a fact is (forget.rs's `STREAM_WINDOW`), so the log grows by at most this for each turn of
/// that week; the whole reply stays in the card's events.
pub const TURN_TEXT_MAX: usize = 8000;

/// How card c's role reads in fact:turn.ended.
fn role_name(c: &Card) -> &'static str {
    match model::CardRole::of(c) {
        model::CardRole::Chair => "chair",
        model::CardRole::Assistant => "assistant",
        model::CardRole::Worker => "worker",
        model::CardRole::Spawned => "spawned",
        model::CardRole::Judge(model::JudgeKind::Goal) => "goal-judge",
        model::CardRole::Judge(model::JudgeKind::Duel) => "duel-judge",
        model::CardRole::Judge(model::JudgeKind::Second) => "second-judge",
        model::CardRole::Judge(model::JudgeKind::Round) => "round-judge",
    }
}

/// The write of the fact that card c's turn on message answered ended as ended, with text its reply or what failed. Its
/// agent caused it. One message ended one way is one fact: a write of it again (a restart that records the end again) is
/// the same reading.
pub fn turn_ended_fact(c: &Card, answered: &model::Event, ended: TurnEnding, text: &str) -> Result<Write> {
    let kept = source::cut_bytes(text, TURN_TEXT_MAX);
    let v = FactValue {
        chain: c.home.clone(),
        card: c.id.clone(),
        employee: c.employee.clone(),
        event: answered.id,
        role: role_name(c).to_owned(),
        opener: crate::outbox::sent_by(answered).kind.as_str().to_owned(),
        ended: Some(ended),
        text: kept.to_owned(),
        cut: kept.len() < text.len(),
        ..FactValue::default()
    };
    let mut w = fact_write(keys::turn_ended(), &v, model::by_agent(&c.id), &c.home)?;
    w.dedupes = vec![format!("turn:{}:{}:{ended:?}", c.id, answered.id)];
    Ok(w)
}

fn is_zero_i64(n: &i64) -> bool {
    *n == 0
}

/// The write of the fact key that says v, made by `by`: its writer is the fact's noun, what changed.
pub fn fact_write(key: SignalKey, v: &impl Serialize, by: Actor, chain_id: &str) -> Result<Write> {
    let Some(noun) = fact_noun_of(&key).map(str::to_owned) else {
        return Err(Error::refused(format!("{key} is no fact key, fact:<noun>.<verb>")));
    };
    let value = model::ForeignJson(serde_json::to_value(v)?);
    Ok(Write {
        key,
        value: Some(value),
        cause: Cause {
            actor: by,
            chain_id: chain_id.to_owned(),
        },
        writer: Writer {
            kind: WriterKind::Fact,
            id: noun,
        },
        ..Write::default()
    })
}

/// What reading r says.
pub fn fact_value_of(r: &Reading) -> Result<FactValue> {
    let value = r.value.as_ref().map_or(serde_json::Value::Null, |v| v.0.clone());
    Ok(serde_json::from_value::<Option<FactValue>>(value)?.unwrap_or_default())
}

/// Stores in tx the fact that ch was queued, with the chain.
pub fn put_queued_fact(tx: &Tx<'_>, door: &Door, ch: &Chain) -> Result<()> {
    let v = FactValue {
        chain: ch.id.clone(),
        ..FactValue::default()
    };
    let w = fact_write(keys::chain_queued(), &v, or_default_actor(ch.by.as_ref()), &ch.id)?;
    door.ingest(tx, w)?;
    Ok(())
}

/// Whether message e steers the card it is sent to: a mentor's, the owner's among them (he is the mentor m_owner,
/// ADR-0062). What steers teaches (teach.rs in crew).
pub fn steers(e: &model::Event) -> bool {
    e.kind() == model::EventKind::Prompt
        && e.by
            .as_ref()
            .is_some_and(|b| matches!(b.kind, model::ActorKind::Mentor | model::ActorKind::Owner))
}

/// Stores in tx, the transaction that queues message e, the fact fact:teach.steer when e steers (`steers`): the card it
/// steers is taught by the mentor who said it, whose own memory learns what they teach. The fact names e, whose id the
/// store has just given, so it exists exactly when the message does. True when it was stored.
pub fn put_steer_fact(tx: &Tx<'_>, door: &Door, e: &model::Event) -> Result<bool> {
    let Some(by) = e.by.clone().filter(|_| steers(e)) else {
        return Ok(false);
    };
    let Some(card) = crate::chain_core::optional(tx.card(&e.card_id))? else {
        return Ok(false);
    };
    let mentor = match by.kind {
        model::ActorKind::Mentor => match crate::chain_core::optional(tx.help(&by.who))? {
            Some(h) => h.mentor,
            None => {
                tracing::warn!(help = %by.who, "teach: the help a mentor's message came through is not on record");
                String::new()
            }
        },
        _ => crate::owner_mentor::OWNER_MENTOR_ID.to_owned(),
    };
    let v = FactValue {
        chain: e.home.clone(),
        event: e.id,
        card: card.id,
        employee: card.employee,
        mentor,
        ..FactValue::default()
    };
    let mut w = fact_write(fact_key("teach", "steer"), &v, by, &e.home)?;
    w.dedupes = vec![format!("event:{}", e.id)];
    door.ingest(tx, w)?;
    Ok(true)
}

/// The write of the fact that ch finished, by by: stored in the step that finishes it.
pub fn finished_fact(ch: &Chain, by: Actor) -> Result<Write> {
    let v = FactValue {
        chain: ch.id.clone(),
        ..FactValue::default()
    };
    fact_write(keys::chain_finished(), &v, by, &ch.id)
}

/// Stores in tx the fact that the owner is asked item it: the subscription that answers it puts the item to the Assistant
/// first (ADR-0062). The agent that raised it caused it; Remit, when no agent did.
pub fn put_asked_fact(tx: &Tx<'_>, door: &Door, it: &InboxItem) -> Result<()> {
    let by = if it.card_id.is_empty() {
        crate::outbox::by_remit()
    } else {
        model::by_agent(&it.card_id)
    };
    let v = FactValue {
        chain: it.home.clone(),
        item: it.id.clone(),
        ..FactValue::default()
    };
    door.ingest(tx, fact_write(keys::owner_asked(), &v, by, &it.home)?)?;
    Ok(())
}

impl Core {
    /// Gives back what a restart cut off of the answers to facts (ADR-0035): a chain finishing whose goal check lost its
    /// deadline gets it back.
    pub async fn restore_facts_answers(&self, chains: &[Chain]) -> Result<()> {
        for ch in chains {
            if let (Some(f), ChainStatus::Running) = (&ch.finishing, ch.status()) {
                let (id, at, now) = (ch.id.clone(), f.at, self.now());
                self.store
                    .update(move |tx| {
                        match tx.subscription(&goal_deadline_id(&id)) {
                            Err(store::Error::NotFound) => {}
                            other => return other.map(|_| ()).map_err(Error::from),
                        }
                        let from_now = Start {
                            from: StartFrom::Now,
                            ..Start::default()
                        };
                        subscribe(tx, goal_deadline(&id, at), &from_now, now)?;
                        Ok(())
                    })
                    .await?;
            }
        }
        Ok(())
    }
}

/// The beginning of every key of these facts.
pub fn remit_key_prefix() -> String {
    fact_key("remit", "").0
}

/// The fact that card c was accepted or sent back (verdict) by the Chair chair. Pure.
pub fn taught_verdict(c: &Card, verdict: ReportVerdict, chair: &Actor) -> Result<Write> {
    let v = FactValue {
        chain: c.home.clone(),
        card: c.id.clone(),
        employee: c.employee.clone(),
        verdict: Some(verdict),
        ..FactValue::default()
    };
    fact_write(fact_key("teach", "verdict"), &v, chair.clone(), &c.home)
}
