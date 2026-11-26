//! The end of a card's turn is recorded together with what it moves on, in one transaction. A turn answers whoever
//! opened it: a worker's turn the Chair (or Remit for it) opened ends in its report to the Chair; one a person opened (the
//! owner, the Assistant on his words, a mentor) ends in its reply to that person alone, and the worker tells the Chair
//! with the report tool when the work moves. A Chair's planned wait is settled. Before a worker's turn ends, it owes a
//! true plan, and after a person's turn that completed its work, a report (`owed_before_end`): Claude Code's Stop hook
//! keeps it going in the same turn until it has paid them.

use std::sync::Arc;

use model::{Actor, ActorKind, Card, CardRole, CardStatus, ChainStatus, Event, JudgeKind, PlanEntry, Retry};

use crate::apply::Records;
use crate::background::is_own_turn;
use crate::chain_core::{Core, new_id};
use crate::duel::duel_by;
use crate::duel::duel_note;
use crate::fact::{TurnEnding, turn_ended_fact};
use crate::gate::verdict_record;
use crate::handover::handover_by;
use crate::inbox::Failed;
use crate::ingest::Write;
use crate::outbox::{new_message, sent_by};
use crate::plan::{plan_open, plan_text};
use crate::session::Run;
use crate::step::{CardEdit, JudgeEnd};
use crate::stream::tail_line;
use crate::{Error, Result};

/// How a turn ended.
#[derive(Debug, Clone, Default)]
pub struct TurnEnd {
    pub output: String,
    pub stop: String,
    /// Why it failed; none when it did not.
    pub failed: Option<Failed>,
    /// What the owner's retry of a failed turn does.
    pub retry: Option<Retry>,
}

impl TurnEnd {
    pub fn failed(why: Failed, retry: Option<Retry>) -> Self {
        Self {
            failed: Some(why),
            retry,
            ..Self::default()
        }
    }

    fn is_failed(&self) -> bool {
        self.failed.is_some()
    }

    /// What its failure said, verbatim: what an agent reads of it.
    fn failed_text(&self) -> &str {
        self.failed.as_ref().map_or("", |f| f.text.as_str())
    }
}

/// A worker's report to its Chair: the end of a turn the Chair opened, or what it sent with the report tool.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkerReport {
    pub output: String,
    pub stop: String,
    /// Who opened the turn it came from.
    pub by: Actor,
}

/// The fact that card's turn on message p ended as end says (fact.rs's `turn_ended_fact`): its reply, or what failed.
fn turn_ended_of(card: &Card, p: &Event, end: &TurnEnd) -> Result<Write> {
    let (ended, text) = match &end.failed {
        Some(f) => (TurnEnding::Failed, f.text.as_str()),
        None if end.stop == model::StopReason::Cancelled.as_str() => (TurnEnding::Stopped, end.output.as_str()),
        None => (TurnEnding::Done, end.output.as_str()),
    };
    turn_ended_fact(card, p, ended, text)
}

/// Whether a turn by opened is answered to a person rather than reported: one the owner, the Assistant on his words or a
/// mentor opened, or Remit's word that a mentor joined.
pub fn answers_a_person(by: &Actor) -> bool {
    by.for_owner() || by.kind == ActorKind::Mentor || *by == mentor_joined_by()
}

/// What a worker owes before its turn ends, with plan its plan, written whether it wrote that plan in the turn, person
/// whether a person opened the turn, and reported whether it reported in it: a true plan, and the Chair told when a
/// person's turn completed its work. Empty when it owes nothing. The last message it wrote stands either way. Pure.
pub fn owed(plan: &[PlanEntry], written: bool, person: bool, reported: bool) -> String {
    const STANDS: &str = " Then end your turn without writing more: your last message stands as it is.";
    if plan_open(plan) && !written {
        let mut b = String::from("Before your turn ends, make your plan true. It still shows steps not completed:\n");
        for p in plan.iter().filter(|p| p.status != "completed") {
            b.push_str(&format!("- [{}] {}\n", p.status, p.content));
        }
        b.push_str(
            "Call update_plan so every step shows what really happened: completed, still pending, reworded or dropped.",
        );
        if person && !reported {
            b.push_str(" If your work is now done or blocked, tell the Chair with report.");
        }
        b.push_str(STANDS);
        return b;
    }
    if person && written && !reported && !plan.is_empty() && !plan_open(plan) {
        return format!("Your plan is complete, and the Chair has not heard: tell it your result with report.{STANDS}");
    }
    String::new()
}

impl Core {
    /// Records the end of the card's turn on message p together with what it moves on, in one transaction: a worker's
    /// report goes to the Chair, or its reply to the person who opened the turn stays theirs, and a Chair's planned wait
    /// is settled.
    pub async fn end_turn(self: &Arc<Self>, r: &Arc<Run>, p: &Event, end: TurnEnd) {
        let fact = turn_ended_of(&r.card_now(), p, &end);
        // boxed: every way a turn ends (done, failed, cut, silent) comes here, and its many branches would otherwise sit
        // inline in each caller's future and debug-build stack frame, deep under a session's loop
        Box::pin(self.record_turn_end(r, p, end)).await;
        self.put_turn_ended(fact).await;
    }

    /// Stores fact:turn.ended once the end is recorded, for what answers a card's reply (a plugin's processor). Each
    /// branch of `record_turn_end` stores the end in a change of its own (the Assistant's, a judge's, a report), so the
    /// fact is stored after it in its own transaction, as memory.recalled is after its recall: a restart that records
    /// the end again writes the same reading (`turn_ended_fact`'s dedupe). A fact that cannot be stored is logged; the
    /// turn's end stands.
    async fn put_turn_ended(&self, fact: Result<Write>) {
        let w = match fact {
            Ok(w) => w,
            Err(err) => {
                tracing::error!(%err, "fact:turn.ended not recorded");
                return;
            }
        };
        let door = self.door();
        match self.store.update(move |tx| door.ingest(tx, w).map(|_| ())).await {
            Ok(()) => self.kick(),
            Err(err) => tracing::error!(%err, "fact:turn.ended not recorded"),
        }
    }

    /// `end_turn`, unboxed.
    async fn record_turn_end(self: &Arc<Self>, r: &Arc<Run>, p: &Event, end: TurnEnd) {
        let card = r.card_now();
        let failed = end.failed.as_ref().map(|f| self.failure_title(&card, f));
        let output = end.output.clone();
        let now = self.now();
        let idle = move |c: &mut Card| {
            c.set_status(CardStatus::Idle, now);
            if let Some(failed) = &failed {
                c.error.clone_from(failed);
            } else if !output.is_empty() {
                c.output.clone_from(&output);
                c.activity = tail_line(&output, 90);
            }
        };
        let by = sent_by(p);
        let set_idle = move |c: &mut Card| c.set_status(CardStatus::Idle, now);
        let role = CardRole::of(&card);
        match role {
            CardRole::Assistant => {
                self.assistant_turn_ended(r, &card, CardEdit::new(&card.id, idle)).await;
                if let Some(f) = &end.failed {
                    self.report_error(&card, f, end.retry).await;
                }
                return;
            }
            CardRole::Judge(JudgeKind::Goal) => {
                // its verdict, not a report, moves the chain on, once its turn has ended
                let why = if end.is_failed() {
                    format!("the judge's turn failed: {}", end.failed_text())
                } else {
                    format!("the judge ended its turn without a verdict ({})", end.stop)
                };
                // Remit sends the check's messages; the owner's (or the Assistant's for him) are questions to the judge
                let end = if by.for_owner() {
                    JudgeEnd::Question
                } else {
                    JudgeEnd::Turn
                };
                if let Ok(d) = self.domains() {
                    d.judge_turn_ended(r, &card, &why, end, CardEdit::new(&card.id, idle))
                        .await;
                }
                return;
            }
            CardRole::Judge(JudgeKind::Duel) => {
                // its pick, not a report, moves the duel on; a turn that ended without one ends the duel
                let why = if end.is_failed() {
                    format!("the judge's turn failed: {}", end.failed_text())
                } else {
                    format!("the judge ended its turn without a pick ({})", end.stop)
                };
                self.display(&card.id, move |c| {
                    idle(c);
                    c.set_status(CardStatus::Stopped, now);
                })
                .await;
                if let Ok(d) = self.domains() {
                    d.duel_judge_ended(&card, &why).await;
                }
                self.close_run_obj(r, &model::Cut::DuelJudged);
                return;
            }
            CardRole::Judge(JudgeKind::Round) => {
                // its pick, not a report, moves the round on; a turn that ended without one ends the round
                let why = if end.is_failed() {
                    format!("the judge's turn failed: {}", end.failed_text())
                } else {
                    format!("the judge ended its turn without a pick ({})", end.stop)
                };
                self.display(&card.id, move |c| {
                    idle(c);
                    c.set_status(CardStatus::Stopped, now);
                })
                .await;
                if let Ok(d) = self.domains() {
                    d.round_judge_ended(&card, &why).await;
                }
                self.close_run_obj(r, &model::Cut::RoundJudged);
                return;
            }
            CardRole::Judge(JudgeKind::Second) => {
                // its comparison, not a report, is its work; a turn that ended without one ends its second opinion
                let why = if end.is_failed() {
                    format!("the judge's turn failed: {}", end.failed_text())
                } else {
                    format!("the judge ended its turn without comparing ({})", end.stop)
                };
                self.display(&card.id, move |c| {
                    idle(c);
                    c.set_status(CardStatus::Stopped, now);
                })
                .await;
                if let Ok(d) = self.domains() {
                    d.second_judge_ended(&card, &why).await;
                }
                self.close_run_obj(r, &model::Cut::SecondOpinionGiven);
                return;
            }
            // the team's turns: each ends by who opened it, below
            CardRole::Chair | CardRole::Worker | CardRole::Spawned => {}
        }
        if by == duel_by() {
            // the duel's pick, read by a side's employee: what it says goes to no one, and its report stays as it was
            self.end_turn_quietly(&card, CardEdit::new(&card.id, set_idle)).await;
        } else if by == handover_by() {
            // the handover note's turn: its words are the note; nothing goes to the Chair, the new card's brief carries it
            self.end_handover_turn(&card, &end).await;
        } else if card.handover.is_some() {
            // a turn the hand-over cut, or one that ended while the note waits: the work is moving, its end goes nowhere
            self.end_turn_quietly(&card, CardEdit::new(&card.id, idle)).await;
        } else if let Some(f) = &end.failed {
            let replies = self.replies_to_assistant(&card, &by, &format!("Its turn failed: {}", f.text));
            let w = Records {
                cards: vec![CardEdit::new(&card.id, idle)],
                messages: replies,
                ..Records::default()
            };
            if role == CardRole::Chair {
                self.settle_wait(&card, w).await;
            } else {
                match self.commit(None, None, w).await {
                    Ok(a) => self.hand_over(&a).await,
                    Err(err) => tracing::error!(home = %card.home, card = %card.id, %err, "turn end"),
                }
            }
            self.respond_to_failed_turn(&card, f, end.retry).await;
        } else if role == CardRole::Chair {
            let replies = self.replies_to_assistant(&card, &by, &end.output);
            let w = Records {
                cards: vec![CardEdit::new(&card.id, idle)],
                messages: replies,
                ..Records::default()
            };
            self.settle_wait(&card, w).await;
        } else if is_own_turn(p) && end.output.is_empty() {
            // news that needed nothing: the agent said nothing, so nothing goes to the Chair
            self.end_turn_quietly(&card, CardEdit::new(&card.id, idle)).await;
        } else if answers_a_person(&by) {
            // its reply is the person's: it stays in the session they read (the Assistant is sent it), and the card's
            // last report stays as it was; the Chair heard what moved through the report tool
            let messages = self.replies_to_assistant(&card, &by, &end.output);
            let reply = end.output;
            let w = Records {
                cards: vec![CardEdit::new(&card.id, move |c| {
                    c.set_status(CardStatus::Idle, now);
                    if !reply.is_empty() {
                        c.activity = tail_line(&reply, 90);
                    }
                })],
                messages,
                ..Records::default()
            };
            if let Err(err) = self.commit(None, None, w).await {
                tracing::error!(home = %card.home, card = %card.id, %err, "turn end");
            }
        } else {
            let report = WorkerReport {
                output: end.output,
                stop: end.stop,
                by,
            };
            self.report(&card, report, idle).await;
        }
    }

    /// What run r's agent owes before its turn ends (`owed`): none for a card whose turn's end moves nothing to a Chair
    /// (the Chair's own, the Assistant's, a judge's), for one whose work is being handed over, and for a turn the
    /// handover note or a duel's pick opened.
    pub fn owed_before_end(&self, r: &Run) -> Option<String> {
        let s = r.state();
        let c = &s.card;
        match CardRole::of(c) {
            CardRole::Chair | CardRole::Assistant | CardRole::Judge(_) => return None,
            CardRole::Worker | CardRole::Spawned => {}
        }
        if c.handover.is_some() || s.opener == handover_by() || s.opener == duel_by() {
            return None;
        }
        let o = owed(&c.plan, s.plan_written, answers_a_person(&s.opener), s.reported);
        (!o.is_empty()).then_some(o)
    }

    /// Notes that card_id's agent wrote its plan in the turn in progress.
    pub fn plan_written(&self, card_id: &str) {
        if let Some(r) = self.run_of(card_id) {
            r.state().plan_written = true;
        }
    }

    /// Sends card_id's report, text, to its Chair now, in a turn a person opened (the report tool): a turn the Chair
    /// opened ends in its report, so there it is refused.
    pub async fn report_now(&self, card_id: &str, text: &str) -> Result<String> {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return Err(Error::refused("a report needs text"));
        }
        let r = self
            .run_of(card_id)
            .ok_or_else(|| Error::refused("report works inside your turn"))?;
        let (opener, c) = {
            let s = r.state();
            (s.opener.clone(), s.card.clone())
        };
        if !answers_a_person(&opener) {
            return Err(Error::refused(
                "the Chair opened this turn: your final message is your report, so end your turn with it",
            ));
        }
        let report = WorkerReport {
            output: text.clone(),
            stop: crate::card_watch::REPORT_STOP.to_owned(),
            by: opener,
        };
        self.send_report(&c, report, move |c| c.output.clone_from(&text), Records::default())
            .await?;
        r.state().reported = true;
        Ok("Reported to the Chair.".to_owned())
    }

    /// Stores the end of card's turn, edit, and sends nothing on: the turn's end goes to no one.
    async fn end_turn_quietly(&self, card: &Card, edit: CardEdit) {
        if let Err(err) = self
            .commit(
                None,
                None,
                Records {
                    cards: vec![edit],
                    ..Records::default()
                },
            )
            .await
        {
            tracing::error!(home = %card.home, card = %card.id, %err, "turn end");
        }
    }

    /// Finishes card's hand-over with the note its turn wrote; a turn that failed or said nothing leaves the core's note.
    async fn end_handover_turn(&self, card: &Card, end: &TurnEnd) {
        let (note, from) = if end.is_failed() || end.output.trim().is_empty() {
            (self.core_note_of(card).await, "core")
        } else {
            (end.output.clone(), "session")
        };
        if let Err(err) = self.complete_handover(&card.id, &note, from).await {
            tracing::error!(home = %card.home, card = %card.id, %err, "finish a hand-over with its note");
        }
    }

    /// Sends a worker's report to its Chair, with the card's change: that event is what moves the chain on. A finished
    /// chain takes no report; the card's change is stored all the same. A report that cannot be stored leaves the card as
    /// it was, mid-turn: after a restart its session resumes and the turn's end is recorded then.
    pub async fn report(&self, c: &Card, h: WorkerReport, edit: impl Fn(&mut Card) + Clone + Send + Sync + 'static) {
        if let Err(err) = self.send_report(c, h, edit, Records::default()).await {
            tracing::error!(home = %c.home, card = %c.id, %err, "report not stored");
        }
    }

    /// Stores report h of card c to its Chair, with the card's change and w, in one change; a finished chain takes no
    /// report, and the card's change and w are stored all the same.
    pub async fn send_report(
        &self,
        c: &Card,
        h: WorkerReport,
        edit: impl Fn(&mut Card) + Clone + Send + Sync + 'static,
        mut w: Records,
    ) -> Result<()> {
        let ch = self.chain(&c.home).await?;
        if *ch.status() == ChainStatus::Done {
            w.cards.push(CardEdit::new(&c.id, edit));
            self.commit(None, None, w).await?;
            return Ok(());
        }
        // A card a run's step put to work reports to the run: its report is the step's result, and the run goes on with it.
        if self.report_to_run(c, &h, edit.clone(), w.clone()).await? {
            return Ok(());
        }
        let mut output = if h.output.is_empty() {
            "(no message)".to_owned()
        } else {
            h.output.clone()
        };
        // a reviewer's or QC's report is short by rule (prompts.rs GATE_REPORT): its evidence is the verdict it recorded
        if c.signs.gate().is_some() {
            let (home, card) = (c.home.clone(), c.id.clone());
            let approvals = self.store.read(move |r| r.approvals(&home)).await?;
            if let Some(a) = approvals.iter().rfind(|a| a.card_id == card) {
                output.push_str(&verdict_record(a));
            }
        }
        let mut head = format!("[report from {} · card {} · stop {}]", c.name, c.id, h.stop);
        if h.by == model::by_agent(&c.id) {
            // a turn it took on its own
            head = format!(
                "[report from {} · card {} · stop {} · on news from its background work]",
                c.name, c.id, h.stop
            );
        }
        let title = format!("Read the report from {}", c.name);
        let duel = self.held_by_duel(c).await?;
        let mut review = true;
        let mut watch = String::new();
        // whether the watchdog reads it: the report of a turn's work, not a failure's
        let watched = c.signs.gate().is_none() && crate::card_watch::watched(&h.stop);
        let report_id = new_id("rp_");
        if let Some(note) = duel.as_ref().map(|d| duel_note(d, c)) {
            // the judge picks between it and its sibling first
            head.push_str(&note);
            review = false;
        } else {
            head.push_str(" — accept it or send it back with accept_work");
            // the Chair's verdict on this report is what scores the watchdog's answers, so a gate signer's, whose evidence is
            // its verdict, is not watched
            if watched && let Some(said) = self.watch_report(c, &report_id, &h.output).await {
                watch = said.note;
                w.shown = said.shown;
            }
        }
        let chair_id = ch.chair_id.clone();
        let chair = self.store.read(move |s| s.card(&chair_id)).await?;
        w.cards.push(CardEdit::new(&c.id, move |c| {
            edit(c);
            if review {
                c.awaiting_review = true;
                // the verdict rules on this report, so a failure's too: it is the last the Chair is asked about, and the
                // answers about the one before it are left without an outcome
                c.report = Some(report_id.clone());
            }
        }));
        let text = format!("{head}\n{output}{}{watch}", plan_text(&c.plan));
        self.send(&chair, new_message(&chair.id, &title, &text, model::by_agent(&c.id)), w)
            .await?;
        match duel {
            Some(d) => self.domains()?.duel_reported(&d).await,
            None => Ok(()),
        }
    }
}

/// Who sends the message that tells a card's agent its mentor joined: Remit, for the mentor, so the turn it opens is
/// answered to the mentor (turn_end.rs's `answers_a_person`).
pub fn mentor_joined_by() -> Actor {
    model::by_remit("mentor_joined")
}
