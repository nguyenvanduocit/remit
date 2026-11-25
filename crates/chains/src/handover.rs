//! A Chair hands a card's work to another employee (hand_over, ADR-0059): a standing employee, or an agent hired anew for
//! it, on a new card in the same worktree. The leaving session writes a handover note in one more turn, within
//! `HANDOVER_NOTE_WAIT`; a session that cannot (none is open, its turn failed, it said nothing, the limit passed) leaves
//! the core's note (`core_note`). The move is one change of the chain: the leaving card stops, the new card opens with the
//! note. `Card::handover` holds the move from the Chair's call until then, so a restart in between finishes it
//! (`note_handovers`, `finish_handovers`). It is how the Chair reassigns an AI employee's work that keeps falling short
//! (ADR-0062).

use std::collections::HashSet;
use std::sync::Arc;

use model::{
    Actor, Card, CardStatus, Chain, ChainStatus, Chosen, Climb, ClimbCause, DecisionKind, Edge, EdgeKind, Event,
    EventKind, EventStatus, ForeignJson, Handover, Origin, OutcomeSource, Proved, Signs, SystemNote, by_agent,
};
use serde::{Deserialize, Serialize};

use crate::apply::{Drop, Proof, Records};
use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::failover::model_label;
use crate::hire_input::HireInput;
use crate::hire_input::HireInputRef;
use crate::outbox::{Message, PRIORITY_OPENING, new_message};
use crate::prompts::{Given, opening_title, worker_prompt};
use crate::step::{CardEdit, Decided};
use crate::{Error, Result};

/// The Chair's hand-over of a card's work (hand_over).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HandOverInput {
    /// the card whose work moves
    pub card_id: String,
    /// an active standing employee, from list_agents' standing
    #[serde(skip_serializing_if = "String::is_empty")]
    pub employee: String,
    /// or an agent to hire anew for it, as hire_agent's agent
    #[serde(skip_serializing_if = "String::is_empty")]
    pub agent: String,
    /// why the work moves: where the leaving employee fell short or is stuck; the owner reads it
    pub why: String,
}

/// Who sends the leaving session the request for its handover note: the turn on it ends the hand-over (turn_end.rs).
pub fn handover_by() -> Actor {
    model::by_remit("handover")
}

/// How long the leaving session has to write its note before the core's note is used.
const HANDOVER_NOTE_WAIT: time::Duration = time::Duration::minutes(3);

/// Labels the handoff edge from the leaving card to the card that takes its work on.
pub const HANDOVER_EDGE: &str = "handover";

/// Asks the leaving session for its note.
const HANDOVER_NOTE_PROMPT: &str = "Your work on this task moves to someone else, who carries on in this worktree. Write them a handover note as your whole answer: what you did, what is left, where you are in the worktree (files, branch, what is uncommitted), and what you would do next. Change nothing more.";

/// The priority of a message an agent is blocked on until it is read.
const PRIORITY_BLOCKING: i64 = 1;

/// The lowest priority, every queued message's floor.
const PRIORITY_OTHER: i64 = 0;

/// The data of a handed_over event.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HandedOverData {
    pub to: String,
    pub employee: String,
    /// Who wrote the note: "session" or "core".
    pub note: String,
}

/// The hire that takes card old's work on for hand-over ho: the same task, worktree and role, with the reports old was
/// handed. It never runs below the model old ran on when code chose that model: a climb names the tier above it, and a
/// Chair's hand-over keeps it, so only a climb changes the tier, and only upward. Pure.
pub fn hire_of_handover(old: &Card, ho: &Handover, inputs: Vec<HireInputRef>) -> HireInput {
    let (model, effort) = match &ho.climb {
        Some(c) => (c.model.clone(), old.effort.clone()),
        None if old.chosen.is_some() => (old.model.clone(), String::new()),
        None => (String::new(), String::new()),
    };
    HireInput {
        employee: ho.employee.clone(),
        agent: ho.agent.clone(),
        name: old.name.clone(),
        brief: old.brief.clone(),
        why: ho.why.clone(),
        inputs,
        deliverable: old.deliverable.clone(),
        criteria: old.criteria.clone(),
        cwd: old.cwd.clone(),
        model,
        effort,
        // a new hire's role is its agent's (errRoleWithoutEmployee)
        role: if ho.employee.is_empty() {
            String::new()
        } else {
            role_of_signs(&old.signs).to_owned()
        },
        ..HireInput::default()
    }
}

/// The role an employee put to work closes gate s in: chair_hire.rs's `signs_of_role`'s inverse. Pure.
pub fn role_of_signs(s: &Signs) -> &'static str {
    match s {
        Signs::Review => "reviewer",
        Signs::Qc => "qc",
        Signs::Security => "security",
        _ => "worker",
    }
}

/// What the agent was writing in a turn that did not end, from its newest events (oldest first): its messages after the
/// last turn's end, "" when the last turn ended. Pure.
pub fn draft_of(events: &[Event]) -> String {
    let mut parts: Vec<&str> = events
        .iter()
        .rev()
        .take_while(|e| e.kind() != EventKind::Stop)
        .filter(|e| e.kind() == EventKind::Message && !e.text.trim().is_empty())
        .map(|e| e.text.trim())
        .collect();
    parts.reverse();
    parts.join("\n\n")
}

/// The handover note the core writes for card c when its session wrote none: its plan, the draft it was writing, its
/// last report and its worktree's git status. It names no card, chain or Chair: a person may read it. Pure.
pub fn core_note(c: &Card, draft: &str, git_status: &str) -> String {
    let mut b = String::from("(Written by the system: the one who had this work left no note.)\n");
    if !c.plan.is_empty() {
        b.push_str("\nTHEIR PLAN:\n");
        for p in &c.plan {
            b.push_str(&format!("- [{}] {}\n", p.status, p.content));
        }
    }
    let draft = draft.trim();
    if !draft.is_empty() {
        b.push_str(&format!("\nWHAT THEY WERE WRITING WHEN THEY STOPPED:\n{draft}\n"));
    }
    let out = c.output.trim();
    if !out.is_empty() {
        b.push_str(&format!("\nTHEIR LAST REPORT:\n{out}\n"));
    }
    let status = git_status.trim_end_matches('\n');
    if status.trim().is_empty() {
        b.push_str("\nGIT STATUS OF THE WORKTREE: clean, or not a git worktree.\n");
    } else {
        b.push_str(&format!(
            "\nGIT STATUS OF THE WORKTREE (git status --short):\n{status}\n"
        ));
    }
    b
}

/// The brief a new card c opens with: the worker brief with the decisions people made for the chain (decided); a
/// hand-over note (note, "" for none) closes it, so whoever takes the work on starts where the leaving employee stopped.
/// Pure.
pub fn handover_brief(
    chain: &Chain,
    c: &Card,
    reports: &[Given],
    decided: &str,
    note: &str,
    principles: &str,
    lang: &str,
) -> String {
    let mut brief = worker_prompt(chain, c, principles, lang, reports) + decided;
    if !note.is_empty() {
        brief.push_str("\n\nHANDOVER NOTE (from the employee who had this work before you, in this worktree: what they did, what is left, where they were):\n");
        brief.push_str(note);
    }
    brief
}

impl Core {
    /// chair handing the work of a card of its team to another employee (i): checked against every hiring rule,
    /// recorded as the Chair's decision at once, then finished with the leaving session's note when it has one open, or
    /// at once with the core's.
    pub async fn hand_over_card(self: &Arc<Self>, chair: &Card, i: HandOverInput) -> Result<()> {
        if chair.origin != Origin::Chair {
            return Err(Error::refused("only the Chair hands work over"));
        }
        self.hand_over_by(chair, i, by_agent(&chair.id), None).await
    }

    /// `hand_over_card` as by makes it: the Chair, or Remit when it is the climb (`climb_card`), whose card runs at the
    /// model and effort climb names.
    pub(crate) async fn hand_over_by(
        self: &Arc<Self>,
        chair: &Card,
        mut i: HandOverInput,
        by: Actor,
        climb: Option<Climb>,
    ) -> Result<()> {
        for s in [&mut i.employee, &mut i.agent, &mut i.why] {
            *s = s.trim().to_owned();
        }
        if i.employee.is_empty() == i.agent.is_empty() {
            return Err(Error::refused(
                "give exactly one of employee (a standing employee) and agent (one to hire anew)",
            ));
        }
        if i.why.is_empty() {
            return Err(Error::refused("why is required: where the work fell short or is stuck"));
        }
        let failed = climb.as_ref().is_some_and(|c| c.cause == ClimbCause::Failed);
        let old = {
            // the check and the write of the hand-over are one step: two calls move it once
            let _one = self.handovers.lock(&i.card_id).await;
            let old = self.team_card(&chair.home, &i.card_id).await?;
            let now = self.now();
            let ho = Handover {
                employee: i.employee.clone(),
                agent: i.agent.clone(),
                why: i.why.clone(),
                by: by.clone(),
                until: model::Time(now.0.checked_add(HANDOVER_NOTE_WAIT).unwrap_or(now.0)),
                climb,
            };
            self.may_hand_over(chair, &old, &ho).await?;
            let what = match &ho.climb {
                Some(c) => format!(
                    "Raise {} from {} to {}.",
                    old.name,
                    model_label(&old.model, &old.effort),
                    model_label(&c.model, &old.effort)
                ),
                None => format!("Hand {} to {}.", old.name, self.handover_label(&ho).await?),
            };
            // the Chair is told in the same change, so it does not hire a duplicate of what Remit is rehiring
            let told = ho.climb.as_ref().map(|c| {
                let text = format!(
                    "[from Remit · {} (card {}) fell short: {}. Remit is rehiring it on {}, one tier up, on a new card in the same worktree: do not hire another.]",
                    old.name,
                    old.id,
                    i.why,
                    model_label(&c.model, &old.effort)
                );
                let mut m = new_message(&chair.id, "Remit is raising an agent", &text, by.clone());
                m.home.clone_from(&chair.home);
                m
            });
            let w = Records {
                cards: vec![CardEdit::new(&old.id, move |c| c.handover = Some(ho.clone()))],
                decisions: vec![Decided {
                    kind: DecisionKind::Handover,
                    text: what,
                    why: i.why.clone(),
                    card_id: old.id.clone(),
                    by,
                }],
                messages: told.into_iter().collect(),
                ..Records::default()
            };
            self.commit(None, None, w).await?;
            old
        };
        // a card whose turn failed has no note to wait for: the core's is used at once
        if failed || !self.session_open(&old.id) {
            return self
                .complete_handover(&old.id, &self.core_note_of(&old).await, "core")
                .await;
        }
        if self.run_of(&old.id).is_some_and(|r| r.busy()) {
            self.cut_turn(&old.id, &model::Cut::HandedOver).await;
        }
        let m = Message {
            title: "Write a handover note".to_owned(),
            text: HANDOVER_NOTE_PROMPT.to_owned(),
            by: handover_by(),
            priority: PRIORITY_BLOCKING,
            ..Message::default()
        };
        if self.send(&old, m, Records::default()).await.is_err() {
            return self
                .complete_handover(&old.id, &self.core_note_of(&old).await, "core")
                .await;
        }
        let o = Arc::clone(self);
        let wait = std::time::Duration::try_from(HANDOVER_NOTE_WAIT).unwrap_or_default();
        self.background(move |work| async move {
            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                () = work.cancelled() => return, // a restart finishes it (note_handovers)
            }
            let id = old.id.clone();
            let c = match o.store.read(move |r| r.card(&id)).await {
                Ok(c) if c.handover.is_some() => c,
                _ => return, // finished by its note turn
            };
            if let Err(err) = o.complete_handover(&c.id, &o.core_note_of(&c).await, "core").await {
                tracing::error!(chain = %c.home, card = %c.id, %err, "finish a hand-over at its time limit");
            }
        });
        Ok(())
    }

    /// Refuses chair's hand-over i of card old when it breaks a rule: a card handed over or being handed over, or a hire
    /// the hiring rules refuse (checked now, nothing stored).
    async fn may_hand_over(&self, chair: &Card, old: &Card, ho: &Handover) -> Result<()> {
        if old.handover.is_some() {
            return Err(Error::refused("this card's work is being handed over already"));
        }
        if self.handed_over(old).await? {
            return Err(Error::refused(
                "this card's work was handed over already: hand over the card that took it on",
            ));
        }
        let inputs = self.handed_inputs(old).await?;
        self.domains()?
            .hiring_of(chair, hire_of_handover(old, ho, inputs))
            .await
            .map(|_| ())
    }

    /// Whether card c's work was handed over already: a hand-over edge leaves it.
    pub async fn handed_over(&self, c: &Card) -> Result<bool> {
        let home = c.home.clone();
        let edges = self.store.read(move |r| r.edges(&home)).await?;
        Ok(edges
            .iter()
            .any(|e| e.from == c.id && e.kind == EdgeKind::Handoff && e.label == HANDOVER_EDGE))
    }

    /// The reports card c was handed when it was hired (its handoff edges in, a hand-over's own left out): the card that
    /// takes its work on is handed them too, so a reviewer's or QC's work keeps what it judges.
    async fn handed_inputs(&self, c: &Card) -> Result<Vec<HireInputRef>> {
        let home = c.home.clone();
        let edges = self.store.read(move |r| r.edges(&home)).await?;
        Ok(edges
            .into_iter()
            .filter(|e| e.to == c.id && e.kind == EdgeKind::Handoff && e.label != HANDOVER_EDGE)
            .map(|e| HireInputRef {
                card_id: e.from,
                label: e.label,
            })
            .collect())
    }

    /// The handoff edges that bring card c's work to the cards that sign a gate on it (its reviewer, QC, security): what
    /// the card that takes its work on is given as well.
    async fn judged_by(&self, c: &Card) -> Result<Vec<Edge>> {
        let home = c.home.clone();
        let (edges, cards) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.edges(&home)?, r.cards(&home)?)))
            .await?;
        let gates: HashSet<&str> = cards
            .iter()
            .filter(|k| k.signs != Signs::None)
            .map(|k| k.id.as_str())
            .collect();
        Ok(edges
            .into_iter()
            .filter(|e| {
                e.from == c.id
                    && e.kind == EdgeKind::Handoff
                    && e.label != HANDOVER_EDGE
                    && gates.contains(e.to.as_str())
            })
            .collect())
    }

    /// Who takes hand-over ho's work on, for the owner: the kept employee as shown, or the agent hired.
    async fn handover_label(&self, ho: &Handover) -> Result<String> {
        if ho.employee.is_empty() {
            return Ok(ho.agent.clone());
        }
        let id = ho.employee.clone();
        let emp = self
            .store
            .read(move |r| r.employee(&id))
            .await
            .map_err(|err: store::Error| Error::refused(format!("employee {:?}: {err}", ho.employee)))?;
        Ok(emp.shown())
    }

    /// Finishes card's pending hand-over with note, written by from ("session" or "core"), in one change of the chain:
    /// the leaving card stopped, its queue dropped, its hand-over cleared, a handed_over event on it; the new card hired,
    /// its handoff edge from the leaving card and its brief, which closes with the note. The leaving card's run is closed
    /// first. Idempotent: a card whose hand-over is finished, or was never asked, is left as it is. A hand-over the
    /// hiring rules now refuse (the employee retired meanwhile) is cleared, and the Chair is told.
    pub async fn complete_handover(&self, card: &str, note: &str, from: &str) -> Result<()> {
        let _one = self.handovers.lock(card).await;
        let id = card.to_owned();
        let old = self.store.read(move |r| r.card(&id)).await?;
        let Some(ho) = old.handover.clone() else {
            return Ok(());
        };
        let ch = self.chain(&old.home).await?;
        let chair_id = ch.chair_id.clone();
        let chair = self.store.read(move |r| r.card(&chair_id)).await?;
        let inputs = self.handed_inputs(&old).await?;
        let climbs = ho.climb.is_some();
        let hired = match self.domains() {
            Ok(d) => d.hiring_of(&chair, hire_of_handover(&old, &ho, inputs)).await,
            Err(err) => Err(err),
        };
        let mut h = match hired {
            Ok(h) => h,
            Err(err) => return self.handover_refused(&old, &chair, &err.to_string(), climbs).await,
        };
        let label = match self.handover_label(&ho).await {
            Ok(label) => label,
            Err(err) => return self.handover_refused(&old, &chair, &err.to_string(), climbs).await,
        };
        let (mut c, now) = (h.card.clone(), self.now());
        // A card that takes on the work of one whose model code chose is chosen by code too, so it climbs when it fails;
        // its judgment was scored on the card it replaces. A climb's hire is Remit's decision, not the Chair's.
        if old.chosen.is_some() {
            c.chosen = Some(Chosen::default());
        }
        if climbs {
            h.decision.by = ho.by.clone();
        }
        // What the card that leaves proves of its model, stored with its leaving: only a climb its gate caused proves it
        // too low. A failed turn says little of the model, and a Chair's hand-over proves nothing; both only settle what
        // no event has proved yet.
        let wrong = ho.climb.as_ref().is_some_and(|c| c.cause == ClimbCause::SentBack);
        let proofs: Vec<Proof> = old
            .chosen
            .as_ref()
            .and_then(|c| c.judgment.clone())
            .map(|judgment| Proof {
                judgment,
                proved: if wrong { Proved::Wrong } else { Proved::Unknown },
                source: OutcomeSource::Card,
                settle: !wrong,
            })
            .into_iter()
            .collect();
        let mut edges = h.edges.clone();
        if c.chosen.is_some() {
            // the gates that judged the work judge the card that carries it on, so its verdicts count as its own rounds
            for e in self.judged_by(&old).await? {
                edges.push(Edge {
                    id: new_id("e_"),
                    from: c.id.clone(),
                    created_at: now,
                    ..e
                });
            }
        }
        edges.push(Edge {
            id: new_id("e_"),
            chain_id: ch.id.clone(),
            from: old.id.clone(),
            to: c.id.clone(),
            kind: EdgeKind::Handoff,
            label: HANDOVER_EDGE.to_owned(),
            created_at: now,
        });
        let data = serde_json::to_string(&HandedOverData {
            to: c.id.clone(),
            employee: c.employee.clone(),
            note: from.to_owned(),
        })?;
        let moved = Event {
            home: old.home.clone(),
            card_id: old.id.clone(),
            status: EventStatus::System(SystemNote::HandedOver),
            by: Some(ho.by.clone()),
            text: format!("The work was handed over to {label}."),
            data: Some(ForeignJson::parse(&data)?),
            created_at: now,
            updated_at: now,
            ..Event::default()
        };
        let stopped = format!("handed over to {label}");
        let stop = move |k: &mut Card| {
            k.set_status(CardStatus::Stopped, now);
            k.activity.clone_from(&stopped);
            k.handover = None;
            k.chosen = None; // what it proved is recorded; a card that stopped is not accepted or raised again
        };
        let brief = Message {
            title: opening_title(&c),
            text: handover_brief(
                &h.chain,
                &c,
                &h.reports,
                &h.decided,
                note,
                &self.cfg.principles,
                &self.cfg.language,
            ),
            by: ho.by.clone(),
            priority: PRIORITY_OPENING,
            ..Message::default()
        };
        // first: the leaving session runs no tool once the new one may start
        if let Some(o) = self.me() {
            o.close_run(&old.id, &model::Cut::HandedOver);
        }
        let w = Records {
            employees: h.employees,
            created: vec![c.clone()],
            edges,
            decisions: vec![h.decision],
            cards: vec![CardEdit::new(&old.id, stop)],
            drops: vec![Drop {
                card_id: old.id.clone(),
                min: PRIORITY_OTHER,
            }],
            events: vec![moved],
            proofs,
            ..Records::default()
        };
        if let Err(err) = self.send(&c, brief, w).await {
            if let Some(chosen) = &c.chosen {
                self.hire_unused(chosen).await;
            }
            // Nothing was stored: the hand-over is cleared, so the card's later turns report again and the Chair may
            // hand it over anew.
            if let Err(also) = self.handover_refused(&old, &chair, &err.to_string(), climbs).await {
                tracing::error!(card = %old.id, err = %also, "a hand-over not made: the Chair not told");
            }
            return Err(err);
        }
        Ok(())
    }

    /// Clears card old's hand-over, which could not be made (the hiring rules now refuse it, or the new card could not
    /// be stored) for why, and tells chair: the card stays as it is, its work not moved.
    async fn handover_refused(&self, old: &Card, chair: &Card, why: &str, climbed: bool) -> Result<()> {
        let w = Records {
            cards: vec![CardEdit::new(&old.id, move |c| {
                c.handover = None;
                if climbed {
                    c.chosen = None; // Remit does not raise it again: its model is the Chair's to move now
                }
            })],
            // what it chose is then proved by nothing more
            proofs: old
                .chosen
                .iter()
                .filter(|_| climbed)
                .filter_map(|c| c.judgment.clone())
                .map(|judgment| Proof {
                    judgment,
                    proved: Proved::Unknown,
                    source: OutcomeSource::Card,
                    settle: true,
                })
                .collect(),
            ..Records::default()
        };
        self.commit(None, None, w).await?;
        let text = if climbed {
            format!(
                "[from Remit · raising {} (card {}) one tier was not possible]\nIts work stays on it, and its model is yours to move now: {why}. Hand it over yourself if it needs a stronger model.",
                old.name, old.id
            )
        } else {
            format!(
                "[from Remit · hand-over of {} (card {}) not made]\nIts work stays on it: {why}. Hand it over again, to someone the rules allow.",
                old.name, old.id
            )
        };
        let m = Message {
            title: "Hand-over not made".to_owned(),
            text,
            by: handover_by(),
            priority: PRIORITY_BLOCKING,
            ..Message::default()
        };
        self.send(chair, m, Records::default()).await.map(|_| ())
    }

    /// The core's handover note for card c: its plan, what it was writing, its last report and its worktree's git status,
    /// read now.
    pub async fn core_note_of(&self, c: &Card) -> String {
        let id = c.id.clone();
        let events = self.store.read(move |r| r.card_events(&id, 50)).await.unwrap_or_else(|err: store::Error| {
            tracing::warn!(chain = %c.home, card = %c.id, %err, "hand-over: the card's last events could not be read");
            Vec::new()
        });
        let status = if c.cwd.is_empty() {
            String::new()
        } else {
            crate::git::git(&c.cwd, &["status", "--short"])
                .await
                .unwrap_or_default()
        };
        core_note(c, &draft_of(&events), &status)
    }

    /// A worker's first message when its session is restarted: its brief as at hire, with the reports it was handed and
    /// the decisions people made for the chain.
    pub async fn restart_brief(&self, chain: &Chain, c: &Card) -> Result<String> {
        let reports = self.given_to(c).await?;
        let decided = self.chain_decided(&chain.id).await?;
        Ok(handover_brief(
            chain,
            c,
            &reports,
            &decided,
            "",
            &self.cfg.principles,
            &self.cfg.language,
        ))
    }

    /// Notes for `resume_interrupted` the cards whose hand-over a restart cut between the Chair's call and the new card:
    /// they are finished once the AI runtime is up, as the new card's session may open at once. In a finished chain no
    /// card is hired, so the hand-over is cleared and the card stays as the finish left it.
    pub async fn note_handovers(&self, chains: &[Chain]) -> Result<()> {
        let mut ids = Vec::new();
        for ch in chains {
            let key = ch.id.clone();
            let cards = self.store.read(move |r| r.cards(&key)).await?;
            for mut c in cards.into_iter().filter(|c| c.handover.is_some()) {
                if *ch.status() == ChainStatus::Done {
                    c.handover = None;
                    c.updated_at = self.now();
                    self.store.update(move |tx| tx.put_card(c).map(|_| ())).await?;
                } else {
                    ids.push(c.id);
                }
            }
        }
        self.shared().handing_over.extend(ids);
        Ok(())
    }

    /// Finishes the hand-overs `note_handovers` noted, each with the core's note: the leaving session is gone with the
    /// process that ran it.
    pub async fn finish_handovers(&self) {
        let cards = std::mem::take(&mut self.shared().handing_over);
        for id in cards {
            let key = id.clone();
            let done = match self.store.read(move |r| r.card(&key)).await {
                Ok(c) => self.complete_handover(&id, &self.core_note_of(&c).await, "core").await,
                Err(err) => Err(err.into()),
            };
            if let Err(err) = done {
                tracing::error!(card = %id, %err, "finish a hand-over after restart");
            }
        }
    }
}
