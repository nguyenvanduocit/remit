//! What a mentor reads and does from the web app (ADR-0089), signed in with a worker key: the chains of their helps on a
//! board of their own, the cards they help with their sessions, their own message drafts, a message to a card they help,
//! and their acts on a help (help.rs). Every read and act is checked against the key: a mentor reads and acts on their
//! own helps, and nothing else.

use std::collections::BTreeMap;
use std::sync::Arc;

use model::{Draft, Event, Help, HelpState, Mentor, WorkerKey, by_mentor};
use wire::app::{
    AgentStanding, AgentTone, Ask, Board, BoardColumn, ChainStanding, ChainTone, Column, Delivery, DoingTone,
};

use crate::orch::Orchestrator;
use chains::outbox::new_message;
use chains::{Error, Result};
use crew::help::mentor_header;
use crew::help::{WorkerRequest, mentor_reads};
use wire::app::{ChainView, EventPage, Snapshot, Who};

/// The board's columns for a mentor, in order, with what they are called.
const MENTOR_COLUMNS: [(Column, &str, &str); 3] = [
    (
        Column::Asked,
        "Asked",
        "A Chair that asks for your help puts its chain here until you accept or decline.",
    ),
    (
        Column::Helping,
        "Helping",
        "A chain stands here while you help it, from the help you accepted until it ends.",
    ),
    (
        Column::Finished,
        "Finished",
        "A help lands here once it ends or you decline it.",
    ),
];

/// The column help h stands in on its mentor's board. Pure.
fn column_of(h: &Help) -> Column {
    match h.state {
        HelpState::Offered => Column::Asked,
        HelpState::Accepted => Column::Helping,
        _ => Column::Finished,
    }
}

/// Of each chain, the help that shows it on its mentor's board: the one asking most of them (asked, then helping, then
/// done), the newest offer among equals. Pure.
fn shown_helps(helps: &[Help]) -> BTreeMap<&str, &Help> {
    let mut by_chain: BTreeMap<&str, &Help> = BTreeMap::new();
    for h in helps {
        let first = by_chain.get(h.chain.as_str()).is_none_or(|held| {
            (column_of(h), std::cmp::Reverse(h.offered_at)) < (column_of(held), std::cmp::Reverse(held.offered_at))
        });
        if first {
            by_chain.insert(&h.chain, h);
        }
    }
    by_chain
}

/// A mentor's board from their helps: the chains they are asked to help, help, and helped, each column the newest offer
/// first. `needs_you` counts what is asked of them; `rev` grows with every change of a help, so a later board replaces an
/// earlier one. Pure.
pub fn mentor_board(helps: &[Help]) -> Board {
    let mut shown: Vec<&Help> = shown_helps(helps).into_values().collect();
    shown.sort_by_key(|h| std::cmp::Reverse(h.offered_at));
    let columns: Vec<BoardColumn> = MENTOR_COLUMNS
        .iter()
        .map(|(key, label, empty)| BoardColumn {
            key: *key,
            label: (*label).to_owned(),
            empty: (*empty).to_owned(),
            chains: shown
                .iter()
                .filter(|h| column_of(h) == *key)
                .map(|h| h.chain.clone())
                .collect(),
            items: Vec::new(),
        })
        .collect();
    Board {
        needs_you: shown.iter().filter(|h| column_of(h) == Column::Asked).count(),
        columns,
        rev: helps.iter().map(|h| h.rev.max(1)).fold(0_i64, i64::saturating_add),
        ..Board::default()
    }
}

/// Where chain `base` stands for the mentor of h, the help that shows it on their board: what is asked of them, as the
/// owner's words of the chain say nothing to them. Pure.
fn mentor_standing(h: &Help, base: ChainStanding) -> ChainStanding {
    let (tone, word) = match h.state {
        HelpState::Offered => (ChainTone::Needs, "Asked"),
        HelpState::Accepted => (ChainTone::Running, "Helping"),
        HelpState::Declined => (ChainTone::Done, "Declined"),
        _ => (ChainTone::Done, "Ended"),
    };
    ChainStanding {
        chain_id: base.chain_id,
        text: format!("{word} · {}", h.problem),
        tone,
        working: h.state == HelpState::Accepted && base.working,
        rev: base.rev,
        ..ChainStanding::default()
    }
}

/// Agent a as the mentor of help h on its card reads it: while the help is accepted they may ask it to read a message, and
/// it stands as for the owner; otherwise they may ask nothing of it, and where it stands is said of the help. Pure.
fn for_mentor(a: &mut AgentStanding, h: Option<&Help>) {
    let state = h.map(|h| &h.state);
    let accepted = state == Some(&HelpState::Accepted);
    a.asks
        .retain(|ask| accepted && matches!(ask, Ask::Message | Ask::Steer | Ask::Resume));
    a.remembers = false;
    if accepted {
        return;
    }
    let (label, tone, doing, doing_tone) = match state {
        Some(HelpState::Offered) => (
            "Asks your help",
            AgentTone::Needs,
            "Accept to read its session and write to it",
            DoingTone::Needs,
        ),
        Some(HelpState::Declined) => (
            "You declined",
            AgentTone::Done,
            "You declined this help",
            DoingTone::Done,
        ),
        _ => ("Help ended", AgentTone::Done, "This help has ended", DoingTone::Done),
    };
    a.label = label.to_owned();
    a.tone = tone;
    a.doing = doing.to_owned();
    a.doing_tone = doing_tone;
    a.waiting.clear();
    a.on_message = None;
    a.on_send = None;
}

/// Chain view v as the mentor whose helps on it are helps reads it: the cards they help, the decisions about those, their
/// helps and drafts, and where those agents stand. Pure.
fn mentor_view(mut v: ChainView, helps: Vec<Help>, drafts: Vec<Draft>) -> ChainView {
    let help_of = |card: &str| help_on(&helps, card);
    v.cards.retain(|c| help_of(&c.id).is_some());
    v.decisions.retain(|d| help_of(&d.decision.card_id).is_some());
    v.agents.retain(|a| help_of(&a.card_id).is_some());
    for a in &mut v.agents {
        for_mentor(a, help_of(&a.card_id));
    }
    if let Some(h) = shown_helps(&helps).into_values().next() {
        v.standing = mentor_standing(h, v.standing);
    }
    v.verification.gates.clear();
    v.verification.judges.clear();
    ChainView {
        edges: Vec::new(),
        workflows: Vec::new(),
        away: None,
        retrospectives: Vec::new(),
        landings: Vec::new(),
        datadog: Vec::new(),
        drafts,
        helps,
        ..v
    }
}

/// The newest of helps on card card_id. Pure.
fn help_on<'a>(helps: &'a [Help], card_id: &str) -> Option<&'a Help> {
    helps.iter().filter(|h| h.card == card_id).max_by_key(|h| h.offered_at)
}

/// Where agent a stands for the mentor whose helps are helps (`for_mentor`); none when they help its card in none.
/// Pure.
pub fn mentor_agent_standing(mut a: AgentStanding, helps: &[Help]) -> Option<AgentStanding> {
    let h = help_on(helps, &a.card_id)?;
    for_mentor(&mut a, Some(h));
    Some(a)
}

/// Where chain base stands for the mentor whose helps are helps (`mentor_standing`); none when they help it in none.
/// Pure.
pub fn mentor_chain_standing(helps: &[Help], base: ChainStanding) -> Option<ChainStanding> {
    let h = *shown_helps(helps).get(base.chain_id.as_str())?;
    Some(mentor_standing(h, base))
}

/// Refuses a mentor the session of help h's card until they read it (`mentor_reads`). Pure.
fn may_read(h: &Help) -> Result<()> {
    if mentor_reads(h) {
        return Ok(());
    }
    Err(Error::refused(format!(
        "this help is {}: you read the card's session once you accepted to help it",
        h.state
    )))
}

impl Orchestrator {
    /// The helps of key key_id's mentor that are on a chain, the newest first: the owner's help on his Assistant is his
    /// panel's, not a chain's.
    pub async fn mentor_helps(&self, key_id: &str) -> Result<Vec<Help>> {
        let k = self.crew.device_key(key_id).await?;
        let mentor = k.mentor.clone();
        let mut helps = self.store.read(move |r| r.mentor_helps(&mentor)).await?;
        helps.retain(|h| !self.is_assistant_home(&h.chain));
        Ok(helps)
    }

    /// Who key key_id is: the key and its mentor.
    pub async fn worker_me(&self, key_id: &str) -> Result<(WorkerKey, Mentor)> {
        let k = self.crew.device_key(key_id).await?;
        let id = k.mentor.clone();
        let m = self.store.read(move |r| r.mentor(&id)).await?;
        Ok((k, m))
    }

    /// Everything key key_id's mentor sees, as it is now: who they are, the chains of their helps, where each stands
    /// for them, and their board.
    pub async fn mentor_snapshot(&self, key_id: &str) -> Result<Snapshot> {
        let (k, m) = self.worker_me(key_id).await?;
        let helps = self.mentor_helps(key_id).await?;
        let mut chains = Vec::new();
        let mut standings = Vec::new();
        for (id, h) in shown_helps(&helps) {
            let v = match self.chain_of(id).await {
                Ok(v) => v,
                Err(Error::Store(store::Error::NotFound) | Error::ChainDeleted) => continue,
                Err(err) => return Err(err),
            };
            if v.chain.chain.archived_at.is_some() {
                continue;
            }
            standings.push(mentor_standing(h, v.standing));
            chains.push(v.chain);
        }
        Ok(Snapshot {
            who: Who::Mentor {
                mentor_id: m.id.clone(),
                name: m.label.clone(),
                key_id: k.id,
            },
            chains,
            board: mentor_board(&helps),
            mentors: vec![m],
            standings,
            ..Snapshot::default()
        })
    }

    /// Chain chain_id as key key_id's mentor reads it (`mentor_view`): refused when they help none of its cards.
    pub async fn mentor_chain(&self, key_id: &str, chain_id: String) -> Result<ChainView> {
        let k = self.crew.device_key(key_id).await?;
        let (id, mentor) = (chain_id.clone(), k.mentor.clone());
        let (helps, drafts) = self
            .store
            .read(move |r| -> store::Result<_> {
                let helps: Vec<Help> = r.chain_helps(&id)?.into_iter().filter(|h| h.mentor == mentor).collect();
                let drafts: Vec<Draft> = r.drafts(&id)?.into_iter().filter(|d| d.mentor == mentor).collect();
                Ok((helps, drafts))
            })
            .await?;
        if helps.is_empty() {
            return Err(Error::refused(format!("you help no agent of chain {chain_id}")));
        }
        Ok(mentor_view(self.view(chain_id).await?, helps, drafts))
    }

    /// The newest events of chain chain_id (`chain_events`) of the cards whose sessions key key_id's mentor reads
    /// (`mentor_reads`).
    pub async fn mentor_chain_events(&self, key_id: &str, chain_id: String) -> Result<EventPage> {
        let k = self.crew.device_key(key_id).await?;
        let id = chain_id.clone();
        let helps = self.store.read(move |r| r.chain_helps(&id)).await?;
        let reads: Vec<String> = helps
            .into_iter()
            .filter(|h| h.mentor == k.mentor && mentor_reads(h))
            .map(|h| h.card)
            .collect();
        let mut page = self.chain_events(chain_id).await?;
        page.events.retain(|e| reads.contains(&e.event.card_id));
        Ok(page)
    }

    /// A page of the session of card card_id (`card_events`), once key key_id's mentor accepted to help it.
    pub async fn mentor_card_events(&self, key_id: &str, card_id: String, before: i64) -> Result<EventPage> {
        let (_, h) = self.crew.help_on_card(key_id, &card_id).await?;
        may_read(&h)?;
        self.card_events(card_id, before).await
    }

    /// Keeps the words key key_id's mentor types to the agent of a card they help, as `keep_draft` keeps the owner's:
    /// their own message draft, made on base_rev.
    pub async fn mentor_save_draft(
        self: &Arc<Self>,
        request_id: &str,
        key_id: &str,
        mut d: Draft,
        base_rev: i64,
    ) -> Result<Draft> {
        let (k, h) = self.crew.help_on_card(key_id, &d.card_id).await?;
        may_read(&h)?;
        d.mentor.clone_from(&k.mentor);
        let args = (d.id.clone(), d.text.clone(), base_rev);
        crew::mentor_act::mentor_once(
            self,
            &k,
            request_id,
            by_mentor(&h.id),
            "SaveDraft",
            &args,
            move |o| async move { o.keep_draft(d, base_rev).await },
        )
        .await
    }

    /// Ends a draft of key key_id's mentor's, as the mentor of the help on its card. One already ended is done.
    pub async fn mentor_delete_draft(self: &Arc<Self>, request_id: &str, key_id: &str, id: String) -> Result<()> {
        let key = id.clone();
        let d = match self.store.read(move |r| r.draft(&key)).await {
            Err(store::Error::NotFound) => return Ok(()),
            other => other?,
        };
        let (k, h) = self.crew.help_on_card(key_id, &d.card_id).await?;
        let writer = k.mentor.clone();
        crew::mentor_act::mentor_once(
            self,
            &k,
            request_id,
            by_mentor(&h.id),
            "DeleteDraft",
            &id.clone(),
            move |o| async move { o.end_draft(id, writer).await.map(|()| chains::request::Done {}) },
        )
        .await
        .map(|_| ())
    }

    /// Sends key key_id's mentor's message draft to the agent of the card they help, headed as theirs, as delivery says
    /// (`owner_says`), ending the draft with the send. Refused while their help is not accepted.
    pub async fn mentor_prompt(
        self: &Arc<Self>,
        request_id: &str,
        key_id: &str,
        draft_id: String,
        delivery: Delivery,
    ) -> Result<Event> {
        let k = self.crew.device_key(key_id).await?;
        let d = self.draft_to_send(&draft_id, &k.mentor).await?;
        let (k, h) = self.crew.help_on_card(key_id, &d.card_id).await?;
        let req = WorkerRequest {
            id: request_id.to_owned(),
            key_id: k.id,
            help_id: h.id,
        };
        self.crew
            .worker_act(&req, "MentorPrompt", &(draft_id, delivery), move |o, m, h| async move {
                if h.state != HelpState::Accepted {
                    return Err(Error::refused(format!(
                        "this help is {}: you message the agent once you accepted to help it, until it ends",
                        h.state
                    )));
                }
                let id = d.card_id.clone();
                let to = o.store.read(move |r| r.card(&id)).await?;
                let text = format!("{}\n{}", mentor_header(&m.label), d.text.trim());
                let msg = new_message(&to.id, &format!("{}'s message", m.label), &text, by_mentor(&h.id));
                o.core
                    .owner_says(
                        &to,
                        msg,
                        delivery,
                        chains::apply::Records {
                            drafts: vec![d],
                            ..Default::default()
                        },
                    )
                    .await
            })
            .await
    }
}
