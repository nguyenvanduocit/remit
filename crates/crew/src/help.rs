//! A mentor's help on one card (ADR-0062). The Chair brings a mentor onto a card of its team for one problem
//! (bring_mentor); the mentor reads the offer on their device and accepts or declines it; once accepted, their messages
//! steer that card's own session, headed with their name and recorded as theirs (`by_mentor`), and the card's report stays
//! its AI's. A help is written only through commit (`Records::helps`), which places its deadline's timer in the same
//! transaction (`put_helps`) and ends every open help of a card whose work ends (`Records::ended`: the Chair stopped it,
//! or handed its work over) or whose chain finishes: the help ends with the card's work, on one path. A card's session
//! closing between turns (its chain waits, the server restarts) ends no help.

use std::sync::Arc;
use std::time::Duration;

use model::{Card, DecisionKind, Help, HelpState, Mentor, MentorStatus, Time, by_agent, by_mentor};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Crew;
use chains::apply::Records;
use chains::chain_core::{first_line, new_id};
use chains::health::after;
use chains::outbox::{by_remit, new_message};
use chains::step::Decided;
use chains::turn_end::mentor_joined_by;
use chains::wait::clock;
use chains::watch::instant_within;
use chains::{Error, Result};
use pipeline::processing::Delivery;

/// How long an offer of help stays open when the Chair gives no deadline.
const HELP_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

/// The Chair's call for a mentor's help on one card.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BringMentorInput {
    /// the id of the card of your team the mentor helps
    pub card: String,
    /// the id of the mentor, as list_mentors shows it
    pub mentor: String,
    /// what the agent is stuck on and what you want of the mentor, self-contained: the mentor reads this and the card's
    /// session, nothing else of the chain
    pub problem: String,
    /// when an offer not answered by then ends and you are told: RFC 3339 with its UTC offset, between 1 minute and 7
    /// days from now; 24 hours from now when left out
    #[serde(default)]
    pub deadline: String,
}

/// Refuses a mentor an act on a help that is not theirs, in the same words as one that does not exist.
pub fn help_not_yours(id: &str) -> Error {
    Error::refused(format!("help {id} is not yours"))
}

/// Heads a mentor's message to the session they steer.
pub fn mentor_header(label: &str) -> String {
    format!("[from {label}, your mentor]")
}

/// What the card's agent is told when mentor m accepted help h. Pure.
fn mentor_joined(m: &Mentor, h: &Help) -> String {
    let strong = if m.strengths.is_empty() {
        String::new()
    } else {
        format!(", strong in {},", m.strengths)
    };
    format!(
        "[from the Chair] {}{strong} is now your mentor on this card, for: {}\nThey read this session as you write it, and their messages reach you headed {}: take their direction as the Chair's. Go on with what does not wait on their answer, then end this turn with what you need from them, as questions they can answer: its last words go to {} alone. Tell the Chair with report when their help moves the work.",
        m.label,
        h.problem,
        mentor_header(&m.label),
        m.label
    )
}

/// The line that says what mentor m last said of when they can help, and when they said it: empty when they said
/// nothing. Their words, never a state computed from them. Pure.
fn availability_words(m: &Mentor, now: Time) -> String {
    match &m.availability {
        Some(a) if !a.note.is_empty() => format!("\n{} said {}: {}", m.label, clock(a.at, now), a.note),
        _ => String::new(),
    }
}

impl Crew {
    /// Offers mentor i.mentor a help on card i.card of chair's team, for i.problem. A card that stopped, a mentor removed
    /// and a mentor already open on the card are refused.
    pub(crate) async fn bring_mentor(&self, chair: &Card, i: BringMentorInput) -> Result<Help> {
        let problem = i.problem.trim();
        if problem.is_empty() {
            return Err(Error::refused(
                "say what the agent is stuck on and what you want of the mentor (problem)",
            ));
        }
        let c = self.team_card(&chair.home, &i.card).await?;
        let id = i.mentor.clone();
        let m = match self.store.read(move |r| r.mentor(&id)).await {
            Ok(m) if m.status == MentorStatus::Active => m,
            Ok(_) | Err(store::Error::NotFound) => {
                return Err(Error::refused(format!(
                    "no mentor {:?} in the pool: list_mentors shows who can help",
                    i.mentor
                )));
            }
            Err(err) => return Err(err.into()),
        };
        let now = self.now();
        let deadline = if i.deadline.is_empty() {
            after(now, HELP_WINDOW)
        } else {
            instant_within("deadline", &i.deadline, now)?
        };
        let card_id = c.id.clone();
        let helps = self.store.read(move |r| r.card_helps(&card_id)).await?;
        if let Some(h) = helps.iter().find(|h| h.mentor == m.id && h.open()) {
            return Err(Error::refused(format!(
                "{} is already on card {} (help {}, {})",
                m.label, c.id, h.id, h.state
            )));
        }
        let h = Help {
            id: new_id("help_"),
            chain: c.home.clone(),
            card: c.id.clone(),
            mentor: m.id.clone(),
            problem: problem.to_owned(),
            state: HelpState::Offered,
            offered_at: now,
            deadline,
            ..Help::default()
        };
        let w = Records {
            helps: vec![h.clone()],
            decisions: vec![Decided {
                kind: DecisionKind::Help,
                text: format!("Asked {} to help {}: {}", m.label, c.name, first_line(problem, 120)),
                why: String::new(),
                card_id: c.id.clone(),
                by: by_agent(&chair.id),
            }],
            ..Records::default()
        };
        let a = self.commit(None, None, w).await?;
        a.helps
            .into_iter()
            .find(|s| s.id == h.id)
            .ok_or_else(|| Error::refused("the help was not stored"))
    }

    /// The mentor taking on the help req names: the card's agent is told who its mentor is and for what, and the Chair
    /// that they took it on.
    pub async fn accept_help(self: &Arc<Self>, req: &WorkerRequest) -> Result<Help> {
        self.worker_act(req, "AcceptHelp", &(), |o, m, mut h| async move {
            if h.state != HelpState::Offered {
                return Err(Error::refused(format!(
                    "this help is {}: only an open offer is accepted",
                    h.state
                )));
            }
            let id = h.card.clone();
            let c = o.store.read(move |r| r.card(&id)).await?;
            (h.state, h.answered_at) = (HelpState::Accepted, Some(o.now()));
            let mut joined = new_message(
                &c.id,
                &format!("{} is your mentor", m.label),
                &mentor_joined(&m, &h),
                mentor_joined_by(),
            );
            c.home.clone_into(&mut joined.home);
            let w = Records {
                helps: vec![h.clone()],
                decisions: vec![Decided {
                    kind: DecisionKind::Help,
                    text: format!("{} accepted to help {}.", m.label, c.name),
                    why: String::new(),
                    card_id: c.id.clone(),
                    by: by_mentor(&h.id),
                }],
                messages: vec![joined],
                ..Records::default()
            };
            let text = format!(
                "[Remit: {} accepted] {} took on helping {} (card {}): their messages now steer its session.",
                m.label, m.label, c.name, c.id
            );
            o.message_chair(&c.home, &text, &format!("{} accepted to help", m.label), w)
                .await?;
            Ok(o.store.read(move |r| r.help(&h.id)).await?)
        })
        .await
    }

    /// The mentor saying no to the help req names, for why: the Chair is told, with their words.
    pub async fn decline_help(self: &Arc<Self>, req: &WorkerRequest, why: &str) -> Result<Help> {
        let why = why.trim().to_owned();
        self.worker_act(req, "DeclineHelp", &why.clone(), move |o, m, mut h| async move {
            if h.state != HelpState::Offered {
                return Err(Error::refused(format!(
                    "this help is {}: only an open offer is declined",
                    h.state
                )));
            }
            let id = h.card.clone();
            let c = o.store.read(move |r| r.card(&id)).await?;
            let said = if why.is_empty() { String::new() } else { format!(": {why}") };
            (h.state, h.answered_at, h.why) = (HelpState::Declined, Some(o.now()), why);
            let w = Records {
                helps: vec![h.clone()],
                decisions: vec![Decided {
                    kind: DecisionKind::Help,
                    text: format!("{} declined to help {}{said}.", m.label, c.name),
                    why: String::new(),
                    card_id: c.id.clone(),
                    by: by_mentor(&h.id),
                }],
                ..Records::default()
            };
            let text = format!(
                "[Remit: {} declined] {} will not help {} (card {}){said}.\nBring another mentor (list_mentors), or go on without one.",
                m.label, m.label, c.name, c.id
            );
            o.message_chair(&c.home, &text, &format!("{} declined to help", m.label), w)
                .await?;
            Ok(o.store.read(move |r| r.help(&h.id)).await?)
        })
        .await
    }

    /// Sends the Chair of chain chain_id a message from Remit, title, text, with w in one change.
    pub(crate) async fn message_chair(&self, chain_id: &str, text: &str, title: &str, w: Records) -> Result<()> {
        let id = chain_id.to_owned();
        let chair = self
            .store
            .read(move |r| -> store::Result<Card> { r.card(&r.chain(&id)?.chair_id) })
            .await?;
        self.send(&chair, new_message(&chair.id, title, text, by_remit()), w)
            .await?;
        Ok(())
    }

    /// The effect of a help's deadline: an offer still not answered ends, and the Chair is told, once, with the firing
    /// and a deadline decision. A help that is gone or was answered ends the subscription; one not due yet waits.
    pub async fn fire_deadline(&self, d: &Delivery) -> Result<()> {
        let (sub, now) = (&d.sub, d.now);
        // the mentor's acts wait: an accept decides before or after this
        let _held = self.help_locks.lock(&sub.action.subject).await;
        let id = sub.action.subject.clone();
        let mut h = match self.store.read(move |r| r.help(&id)).await {
            Ok(h) if h.state == HelpState::Offered => h,
            Ok(_) | Err(store::Error::NotFound) => return self.drop_subscription(&sub.id).await,
            Err(err) => return Err(err.into()),
        };
        if now < h.deadline {
            return Err(Error::NotAdmitted(format!(
                "the help's deadline is {}",
                clock(h.deadline, now)
            )));
        }
        let (mentor, card) = (h.mentor.clone(), h.card.clone());
        let (m, c) = self
            .store
            .read(move |r| -> store::Result<_> { Ok((r.mentor(&mentor)?, r.card(&card)?)) })
            .await?;
        (h.state, h.ended_at, h.why) = (HelpState::Ended, Some(now), "not answered by its deadline".to_owned());
        let fired = d.done(
            &format!("told the Chair: {} did not answer", m.label),
            &h.chain,
            &h.card,
        );
        let text = format!(
            "[Remit: no answer] {} has not answered your call to help {} (card {}), made at {}; its deadline was {}, so it ended.{}\nBring another mentor (list_mentors), or go on without one.",
            m.label,
            c.name,
            c.id,
            clock(h.offered_at, now),
            clock(h.deadline, now),
            availability_words(&m, now)
        );
        let w = Records {
            decisions: vec![Decided {
                kind: DecisionKind::Deadline,
                text: format!(
                    "{} did not answer the call to help {} by {}.",
                    m.label,
                    c.name,
                    clock(h.deadline, now)
                ),
                why: String::new(),
                card_id: c.id.clone(),
                by: by_remit(),
            }],
            fired: Some(fired),
            helps: vec![h.clone()],
            ..Records::default()
        };
        self.message_chair(&h.chain, &text, &format!("{} did not answer", m.label), w)
            .await
    }
}

/// One act of a mentor: the request id their page gave it, the key they signed in with, and the help the act is on.
#[derive(Debug, Clone, Default)]
pub struct WorkerRequest {
    pub id: String,
    pub key_id: String,
    pub help_id: String,
}

/// Whether help h's mentor reads its card's session: once they accepted it, and after it ended what they saw while
/// helping. An offer shows its problem and its card's brief, not the work. Pure.
pub fn mentor_reads(h: &Help) -> bool {
    h.state == HelpState::Accepted || h.state == HelpState::Ended && h.answered_at.is_some()
}
