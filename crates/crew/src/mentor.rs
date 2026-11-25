//! The pool of mentors (ADR-0062): the people who help the AI employees. The owner has the Assistant add and remove them
//! (add_mentor, remove_mentor) and make the codes they work from (worker_key_tools.rs); a Chair reads the pool
//! (list_mentors) and brings one onto a card of its team (bring_mentor, help.rs). Nobody fills in a form for it.

use std::sync::Arc;

use std::collections::HashMap;

use model::{Actor, Availability, Decision, DecisionKind, HelpState, Mentor, MentorStatus, Time};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Crew;
use crate::help::BringMentorInput;
use crate::memory::MEMORY_EVIDENCE;
use chains::apply::Records;
use chains::chain_core::{first_line, new_id};
use chains::owner_mentor::OWNER_MENTOR_ID;
use chains::tools::{Effect, NoInput, SelfCard, Tools, as_json};
use chains::wait::clock;
use chains::{Error, Result};

/// The longest name a mentor takes.
const MENTOR_LABEL_MAX: usize = 60;

/// How many of a mentor's helps, the newest first, list_mentors gives.
const LAST_HELPS_SHOWN: usize = 5;

/// How many of what a mentor taught list_mentors gives the gist of.
const TAUGHT_SHOWN: usize = 3;

/// A mentor as an agent reads them in list_mentors: who they are, what they are strong in, what they last said of when
/// they can help, whether they are in the app now, how many helps of theirs are open, and their last helps with how each
/// card's work was judged, and the gist of what they taught, as their memory keeps it. Codes counts the codes they work
/// from, for the Assistant.
#[derive(Debug, Serialize)]
struct MentorRow {
    id: String,
    label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    strengths: String,
    status: MentorStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    availability: Option<Availability>,
    presence: String,
    open_helps: usize,
    last_helps: Vec<HelpLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    taught: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codes: Option<usize>,
}

/// One of a mentor's helps as list_mentors gives it.
#[derive(Debug, Serialize)]
struct HelpLine {
    help: String,
    chain: String,
    card: String,
    /// The card's name, or its deliverable when it has none.
    work: String,
    problem: String,
    state: HelpState,
    #[serde(skip_serializing_if = "String::is_empty")]
    why: String,
    /// The card's newest verdict: accepted or sent back.
    #[serde(skip_serializing_if = "String::is_empty")]
    judged: String,
}

/// Each card's newest verdict among a chain's decisions ds: accepted or sent back. Pure.
fn newest_verdicts(ds: &[Decision]) -> HashMap<String, String> {
    let mut out: HashMap<String, (Time, &str)> = HashMap::new();
    for d in ds {
        let word = match d.kind {
            DecisionKind::Accept => "accepted",
            DecisionKind::Rework => "sent back",
            _ => continue,
        };
        if out.get(&d.card_id).is_some_and(|(at, _)| d.created_at < *at) {
            continue;
        }
        out.insert(d.card_id.clone(), (d.created_at, word));
    }
    out.into_iter()
        .map(|(card, (_, word))| (card, word.to_owned()))
        .collect()
}

const LIST_MENTORS_HELP: &str = "The people who can help your agents, as mentors: each with what they are strong in, what they last said of when they can help, whether they are in the app now, how many helps of theirs are open, their last helps with how each card's work was judged, and the gist of what they taught (taught). Choose by strengths and record. ";

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddMentorInput {
    /// their name, as the owner says it: Lan
    label: String,
    /// what they are good at, in the owner's words: visual design, the payments domain
    #[serde(default)]
    strengths: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MentorIdInput {
    /// the id of the mentor, as list_mentors shows it
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListMentorsInput {
    /// true lists removed mentors too
    #[serde(default)]
    all: bool,
}

impl Crew {
    /// Puts label into the pool, strong in strengths, by who asked. A name an active mentor has already is refused: the
    /// Chair and the owner tell mentors apart by name.
    async fn add_mentor(&self, label: &str, strengths: &str, by: &Actor) -> Result<Mentor> {
        let (label, strengths) = (label.trim(), strengths.trim());
        match label.chars().count() {
            0 => return Err(Error::refused("a mentor needs a name")),
            n if n > MENTOR_LABEL_MAX => {
                return Err(Error::refused(format!(
                    "a mentor's name is at most {MENTOR_LABEL_MAX} characters"
                )));
            }
            _ => {}
        }
        let m = Mentor {
            id: new_id("m_"),
            label: label.to_owned(),
            strengths: strengths.to_owned(),
            status: MentorStatus::Active,
            created_at: self.now(),
            ..Mentor::default()
        };
        let stored = m.clone();
        let saved = self
            .store
            .update(move |tx| -> Result<Mentor> {
                let lower = stored.label.to_lowercase();
                if let Some(had) = tx
                    .mentors()?
                    .into_iter()
                    .find(|x| x.status == MentorStatus::Active && x.label.to_lowercase() == lower)
                {
                    return Err(Error::refused(format!(
                        "{} is a mentor already ({})",
                        had.label, had.id
                    )));
                }
                tx.put_mentor(stored.clone())?;
                Ok(tx.mentor(&stored.id)?)
            })
            .await?;
        tracing::info!(mentor = %saved.id, by = %by.kind, who = %by.who, "mentor added");
        self.notify.mentor_updated(&saved);
        Ok(saved)
    }

    /// Changes mentor id by edit, on the copy stored now, and tells the channels.
    pub(crate) async fn update_mentor(&self, id: &str, edit: impl FnOnce(&mut Mentor) + Send + 'static) -> Result<()> {
        let id = id.to_owned();
        let saved = self
            .store
            .update(move |tx| -> store::Result<Mentor> {
                let mut m = tx.mentor(&id)?;
                edit(&mut m);
                tx.put_mentor(m)?;
                tx.mentor(&id)
            })
            .await?;
        self.notify.mentor_updated(&saved);
        Ok(())
    }

    /// Takes mentor id off the pool, by who asked: every code of theirs stops, and every help of theirs still open ends,
    /// its Chair told. Removed twice, it changes nothing.
    async fn remove_mentor(&self, id: &str, by: &Actor) -> Result<Mentor> {
        if id == OWNER_MENTOR_ID {
            return Err(Error::refused(
                "the owner is a mentor for good: his codes are taken back with revoke_worker_key",
            ));
        }
        let key = id.to_owned();
        let mut m = match self.store.read(move |r| r.mentor(&key)).await {
            Ok(m) => m,
            Err(store::Error::NotFound) => {
                return Err(Error::refused(format!("no mentor {id:?}: list_mentors shows the ids")));
            }
            Err(err) => return Err(err.into()),
        };
        if m.status != MentorStatus::Removed {
            self.update_mentor(id, |m| m.status = MentorStatus::Removed).await?;
        }
        let key = id.to_owned();
        let (keys, helps) = self
            .store
            .read(move |r| -> store::Result<_> { Ok((r.worker_keys(&key)?, r.mentor_helps(&key)?)) })
            .await?;
        for k in keys {
            self.keys.revoke_worker_key(&k.id).await?;
        }
        for h in helps {
            self.end_help_of_removed(&m, &h.id).await?;
        }
        tracing::info!(mentor = id, by = %by.kind, who = %by.who, "mentor removed");
        m.status = MentorStatus::Removed;
        Ok(m)
    }

    /// Ends help help_id of mentor m, whom the owner removed, when it is still open, and tells its Chair.
    async fn end_help_of_removed(&self, m: &Mentor, help_id: &str) -> Result<()> {
        let _held = self.help_locks.lock(help_id).await;
        let id = help_id.to_owned();
        let mut h = self.store.read(move |r| r.help(&id)).await?;
        if !h.open() {
            return Ok(());
        }
        let card = h.card.clone();
        let c = self.store.read(move |r| r.card(&card)).await?;
        (h.state, h.ended_at, h.why) = (
            HelpState::Ended,
            Some(self.now()),
            format!("the owner took {} off the mentors", m.label),
        );
        let text = format!(
            "[Remit: {0} is no mentor any more] The owner took {0} off the mentors, so their help on {1} (card {2}) ended.\nBring another mentor (list_mentors), or go on without one.",
            m.label, c.name, c.id
        );
        let chain = h.chain.clone();
        let w = Records {
            helps: vec![h],
            ..Records::default()
        };
        self.message_chair(&chain, &text, &format!("{}'s help ended", m.label), w)
            .await
    }

    /// Every mentor of the pool as list_mentors gives them, the oldest first: active ones alone unless all, with their
    /// codes counted when codes.
    async fn mentor_rows(&self, all: bool, codes: bool) -> Result<Vec<MentorRow>> {
        let read = self
            .store
            .read(move |r| -> store::Result<_> {
                let mut verdicts: HashMap<String, HashMap<String, String>> = HashMap::new();
                let mut out = Vec::new();
                for m in r.mentors()? {
                    if !all && m.status != MentorStatus::Active {
                        continue;
                    }
                    let keys = r.worker_keys(&m.id)?;
                    let helps = r.mentor_helps(&m.id)?;
                    let open = helps.iter().filter(|h| h.open()).count();
                    let mut last = Vec::new();
                    for h in helps.into_iter().take(LAST_HELPS_SHOWN) {
                        let c = r.card(&h.card)?;
                        if !verdicts.contains_key(&h.chain) {
                            verdicts.insert(h.chain.clone(), newest_verdicts(&r.decisions(&h.chain)?));
                        }
                        let judged = verdicts
                            .get(&h.chain)
                            .and_then(|v| v.get(&h.card))
                            .cloned()
                            .unwrap_or_default();
                        last.push(HelpLine {
                            work: if c.name.is_empty() { c.deliverable } else { c.name },
                            problem: first_line(&h.problem, 120),
                            help: h.id,
                            chain: h.chain,
                            card: h.card,
                            state: h.state,
                            why: h.why,
                            judged,
                        });
                    }
                    out.push((m, keys, open, last));
                }
                Ok(out)
            })
            .await?;
        let now = self.now();
        let mut rows = Vec::with_capacity(read.len());
        for (m, keys, open, last) in read {
            rows.push(MentorRow {
                presence: self.presence_now(&m.id, &keys).words(now),
                codes: codes.then_some(keys.len()),
                taught: self.mentor_gist(&m.id, TAUGHT_SHOWN).await,
                id: m.id,
                label: m.label,
                strengths: m.strengths,
                status: m.status,
                availability: m.availability,
                open_helps: open,
                last_helps: last,
            });
        }
        Ok(rows)
    }

    /// Gives the Assistant, acting as by, the mentor pool: add a person, list who is in it, take one off.
    pub fn add_mentor_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        let add_by = by.clone();
        t.serving(self).add(
            "add_mentor", Effect::Writes,
            "Add a person to the mentors when the owner asks: a person the Chairs may bring onto a card when an agent is stuck on what needs them (taste, a customer, deep context). Give their name and what they are strong in. Then make them a code with create_worker_key: they sign in to Remit with it and help from there.",
            move |o, i: AddMentorInput| {
                let by = add_by.clone();
                async move {
                    let m = o.add_mentor(&i.label, &i.strengths, &by).await?;
                    Ok(format!(
                        "Added {} as mentor {}. Make them a code with create_worker_key (mentor {}).",
                        m.label, m.id, m.id
                    ))
                }
            },
        );
        t.serving(self).add(
            "list_mentors",
            Effect::Reads,
            format!("{LIST_MENTORS_HELP}Each with the number of codes they work from."),
            |o, i: ListMentorsInput| async move { as_json(&o.mentor_rows(i.all, true).await?) },
        );
        t.serving(self).add(
            "read_mentor_memory", Effect::Reads,
            format!(
                "What a mentor taught, as their own memory keeps it: each lesson, preference or principle the librarian learned from their messages to the cards they helped (the owner's too: he is the mentor m_owner, and his messages to you and his overturns teach it), with its id, kind, title, body, the teaching it was learned from (source), whether it is still recalled and when it last changed. {MEMORY_EVIDENCE}"
            ),
            |o, i: MentorIdInput| async move { as_json(&o.mentor_memory(&i.id).await?) },
        );
        let remove_by = by.clone();
        t.serving(self).add(
            "remove_mentor", Effect::Writes,
            "Take a person off the mentors when the owner asks: every code of theirs stops at once, and every help of theirs still open ends, its Chair told.",
            move |o, i: MentorIdInput| {
                let by = remove_by.clone();
                async move {
                    let m = o.remove_mentor(&i.id, &by).await?;
                    Ok(format!("Removed {} from the mentors.", m.label))
                }
            },
        );
    }

    /// Gives a Chair, whose card me reads, the mentor pool to read and bring_mentor.
    pub fn add_bring_mentor_tools(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        t.serving(self).add(
            "list_mentors",
            Effect::Reads,
            LIST_MENTORS_HELP,
            |o, _: NoInput| async move { as_json(&o.mentor_rows(false, false).await?) },
        );
        let m = me.clone();
        t.serving(self).add(
            "bring_mentor", Effect::Writes,
            "Bring a mentor onto a card of your team, for one problem: when its agent is stuck on what needs a person (taste, a customer, deep context), or keeps falling short after a send-back. The mentor reads the problem and the card's brief, and accepts or declines; you are told either way, and when they do not answer by the deadline. Once they accept, their messages steer that card's session; the card's report stays its agent's, judged with accept_work. The help ends with the card.",
            move |o, i: BringMentorInput| {
                let m = m.clone();
                async move {
                    let me = m.read(&o).await?;
                    let h = o.bring_mentor(&me, i).await?;
                    Ok(format!(
                        "Offered help {}; it waits for their answer until {}. You are told when they answer.",
                        h.id,
                        clock(h.deadline, o.now())
                    ))
                }
            },
        );
    }
}
