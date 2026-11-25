//! The owner's lifecycle actions on a chain: create, pause, resume, finish, restart a card. Each takes a Request and is
//! done once per request id (request.rs); each changes the chain through `apply` alone.

use std::sync::Arc;

use model::{
    Actor, Card, CardRole, CardStatus, Chain, ChainKind, ChainOrigin, ChainStatus, DecisionKind, Event, JudgeKind,
    KeyResult, Objective, ObjectiveStatus, Serves, Unjudged,
};

use crate::apply::{Drop, Records};
use crate::chain_core::Core;
use crate::chain_core::{first_line, new_id, or_default};
use crate::fact::put_queued_fact;
use crate::outbox::{Message, PRIORITY_OPENING, by_remit};
use crate::project::{Assignment, assign_in};
use crate::prompts::opening_title;
use crate::request::Service;
use crate::request::{Done, Request};
use crate::schedule::at_work;
use crate::step::{CardEdit, ChainCmd, Decided, FinishCmd, PauseCmd, ResumeCmd, finish_why, queued};
use crate::{Error, Result};

/// A chain to queue: what it is for, its kind (ADR-0113), and who asked for it. The fields other groups queue chains with (proposed and replay chains) come with their groups.
#[derive(Debug, Clone, Default)]
pub struct NewChain {
    /// The chain's id when the firing that starts it has recorded it (signals.rs); empty: a new one.
    pub id: String,
    /// The subscription that started it: a routine, or a start on an event.
    pub routine_id: String,
    pub title: String,
    pub goal: String,
    /// What code branches on for it; its capabilities start as the kind's.
    pub kind: ChainKind,
    pub origin: ChainOrigin,
    /// The chain that proposed it (schedule.rs `propose`); empty for any other.
    pub proposed_by: String,
    pub by: Actor,
    /// The key result of an active objective it works toward; none: its own task objective.
    pub serves: Option<Serves>,
    /// The round it is an attempt of (ADR-0115); none for a chain that competes with none.
    pub attempt: Option<String>,
    /// Who answers for a task objective's key result; empty: the owner.
    pub owner: model::Accountable,
    /// The capabilities Remit gives a chain it opens for its own job (a verification chain), in place of its kind's.
    pub capabilities: Vec<String>,
    /// The project it is about, named by whoever asked for it (ADR-0108): set on its objective as it is stored
    /// (`project::assign_in`); none leaves its objective's as it is.
    pub project: Option<String>,
}

/// What a restarted session's first message ends with.
const RESTART_NOTE: &str = "\n\n[This session was restarted after an interruption; your earlier conversation is lost.]";

impl Core {
    /// Records a chain waiting for a slot; `start_chain` gives it its Chair. An owner's chain is titled by the first line of its goal until its Chair titles it in its first turn (`set_title`).
    pub async fn queue_chain(&self, input: NewChain) -> Result<Chain> {
        self.queue_chain_if(input, |_| Ok(())).await
    }

    /// Queues a new chain as `queue_chain` does, if admit, run in the transaction that stores it, lets it through: a
    /// rule on the chains stored (a title taken, a cap reached) holds against a chain queued at the same time.
    pub async fn queue_chain_if(
        &self,
        input: NewChain,
        admit: impl FnOnce(&store::Tx<'_>) -> Result<()> + Send + 'static,
    ) -> Result<Chain> {
        let ch = self.new_queued_chain(&input).await?;
        let (door, owner) = (self.door(), input.owner);
        let named = input.project.map(|id| Assignment {
            project: Some(id),
            by: input.by,
            why: "Named when the chain was asked for.".to_owned(),
            card_id: String::new(),
        });
        let (ch, decided) = self
            .store
            .update(move |tx| {
                admit(tx)?;
                let ch = with_task_objective(tx, ch, owner)?;
                let decided = match &named {
                    Some(a) => assign_in(tx, &ch, a, door.now)?,
                    None => None,
                };
                let ch = tx.put_chain(ch)?;
                put_queued_fact(tx, &door, &ch)?;
                Ok::<_, Error>((ch, decided))
            })
            .await?;
        self.queued(&ch);
        if let Some(d) = decided {
            self.told_assigned(&d);
        }
        Ok(ch)
    }

    /// The key result s names, when an active objective holds it: a draft has no chains, a closed objective needs none,
    /// and the one key result of a task objective belongs to its own chain, though what refines it is served by the
    /// chains its Chair opens (ADR-0115).
    async fn served_key_result(&self, s: &Serves) -> Result<KeyResult> {
        let (id, kr_id) = (s.objective.clone(), s.key_result.clone());
        let (obj, kr) = self
            .store
            .read(move |r| {
                Ok::<_, Error>((
                    r.objective(&id)?,
                    crate::chain_core::optional(r.key_result(&id, &kr_id))?,
                ))
            })
            .await
            .map_err(|e| Error::refused(format!("serves: objective {}: {e}", s.objective)))?;
        let Some(kr) = kr else {
            return Err(Error::refused(format!(
                "serves: objective {} has no key result {}",
                s.objective, s.key_result
            )));
        };
        if obj.status != ObjectiveStatus::Active || (obj.task && kr.parent.is_none()) {
            return Err(Error::refused(format!(
                "serves: objective {} is not an active objective of the owner's",
                s.objective
            )));
        }
        Ok(kr)
    }

    /// The chain input asks for, as it is queued, with its directory made.
    pub async fn new_queued_chain(&self, input: &NewChain) -> Result<Chain> {
        if input.goal.trim().is_empty() {
            return Err(Error::refused("goal is required"));
        }
        let now = self.now();
        let id = if input.id.is_empty() {
            new_id("ch_")
        } else {
            input.id.clone()
        };
        let mut goal = input.goal.clone();
        if let Some(s) = &input.serves {
            let kr = self.served_key_result(s).await?;
            goal = format!("KEY RESULT {}: {}\n\n{}", kr.id, kr.text, input.goal);
        }
        let mut ch = queued(Chain::default());
        ch.title = or_default(&input.title, &first_line(&input.goal, 80));
        ch.goal = goal;
        ch.serves.clone_from(&input.serves);
        ch.attempt.clone_from(&input.attempt);
        ch.origin = input.origin;
        ch.routine_id.clone_from(&input.routine_id);
        ch.proposed_by.clone_from(&input.proposed_by);
        ch.by = Some(input.by.clone());
        ch.workdir = self.workdir_of(&id);
        ch.id = id;
        ch.created_at = now;
        ch.updated_at = now;
        ch.kind = input.kind;
        ch.capabilities = if input.capabilities.is_empty() {
            self.domains()?.kind_capabilities(input.kind)
        } else {
            // a chain Remit opens for a job of its own
            input.capabilities.clone()
        };
        tokio::fs::create_dir_all(&ch.workdir)
            .await
            .map_err(|e| Error::refused(format!("make {}: {e}", ch.workdir)))?;
        Ok(ch)
    }

    /// Tells the orchestrator and its channels of a chain just stored as queued.
    pub fn queued(&self, ch: &Chain) {
        self.chain_state(ch.clone());
        self.notify.chain_updated(&wire::app::ChainRead {
            chain: ch.clone(),
            check_back: None,
        });
    }

    /// The directory Remit makes for a chain, and the only one it removes when the chain is deleted.
    pub fn workdir_of(&self, chain_id: &str) -> String {
        self.cfg.work_root.join(chain_id).to_string_lossy().into_owned()
    }

    /// Starts a new session for a card whose session is gone (stopped, failed, or lost in a restart), with the card's
    /// original prompt and a note that its earlier conversation is lost. The new prompt replaces an opening message still
    /// waiting from the session that never started; what others sent it meanwhile stays queued after it.
    pub async fn restart(self: &Arc<Self>, req: Request, card_id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "Restart", move |o| async move {
            o.restart_card(&card_id, "", by).await
        })
        .await
    }

    /// Restarts card card_id's session as by (the owner, or the Assistant for him), for why.
    pub async fn restart_card(self: &Arc<Self>, card_id: &str, why: &str, by: Actor) -> Result<()> {
        let id = card_id.to_owned();
        let c = self.store.read(move |r| r.card(&id)).await?;
        let chair = match CardRole::of(&c) {
            // the owner starting the conversation over, in a new session
            CardRole::Assistant => return self.start_over(&c, &by).await,
            CardRole::Judge(_) => {
                return Err(Error::refused(
                    "a judge is not restarted: the owner can still ask the one that answered, and a new finish is checked by a new judge",
                ));
            }
            CardRole::Chair => true,
            CardRole::Worker | CardRole::Spawned => false,
        };
        if self.session_open(card_id) {
            return Err(Error::refused(format!("card {card_id} is still running")));
        }
        let chain = self.chain(&c.home).await?;
        if *chain.status() == ChainStatus::Done {
            return Err(Error::refused("the chain is finished"));
        }
        let first = if chair {
            format!(
                "{}{RESTART_NOTE} Call list_agents to see what the chain already has before hiring.",
                self.chair_brief(&chain).await?
            )
        } else {
            format!("{}{RESTART_NOTE}", self.restart_brief(&chain, &c).await?)
        };
        let m = Message {
            title: opening_title(&c),
            text: first,
            by: by_remit(),
            priority: PRIORITY_OPENING,
            ..Message::default()
        };
        let now = self.now();
        let w = Records {
            cards: vec![CardEdit::new(&c.id, move |c| {
                c.session_id.clear();
                c.set_status(CardStatus::Starting, now);
                c.error.clear();
                "restarting".clone_into(&mut c.activity);
            })],
            decisions: vec![Decided {
                kind: DecisionKind::Restart,
                text: format!("Restart {}.", c.name),
                why: why.to_owned(),
                card_id: c.id.clone(),
                by,
            }],
            drops: vec![Drop {
                card_id: c.id.clone(),
                min: PRIORITY_OPENING,
            }],
            ..Records::default()
        };
        self.send(&c, m, w).await.map(|_| ())
    }

    /// Holds the chain: turns in progress end, no agent takes a message until it resumes, and it gives up its slot. A
    /// waiting chain keeps its check-back.
    pub fn pause(
        self: &Arc<Self>,
        req: Request,
        chain_id: String,
        why: String,
    ) -> impl std::future::Future<Output = Result<Done>> + Send + use<> {
        let by = req.by.clone();
        self.act_on_chain(req, "Pause", chain_id, move |o, id| async move {
            o.pause_as(&id, &or_default(&why, "paused by the owner"), by).await
        })
    }

    /// Pauses the chain for why, as by (the owner, or his Assistant on his words).
    pub async fn pause_as(&self, chain_id: &str, why: &str, by: Actor) -> Result<()> {
        let _runs = self.lock_runs(chain_id).await?;
        // its runs pause with it: no call is issued until it resumes
        let w = self.paused_runs(chain_id).await?;
        let why = why.to_owned();
        self.apply(chain_id, ChainCmd::Pause(PauseCmd { why, by }), w)
            .await
            .map(|_| ())
    }

    /// Runs a paused chain again and tells its Chair; one paused while waiting waits again for its check-back.
    pub fn resume(
        self: &Arc<Self>,
        req: Request,
        chain_id: String,
        why: String,
    ) -> impl std::future::Future<Output = Result<Done>> + Send + use<> {
        let by = req.by.clone();
        self.act_on_chain(req, "Resume", chain_id, move |o, id| async move {
            o.resume_as(&id, &or_default(&why, "resumed by the owner"), by).await
        })
    }

    /// Resumes the paused chain for why, as by (the owner, or his Assistant on his words).
    pub async fn resume_as(&self, chain_id: &str, why: &str, by: Actor) -> Result<()> {
        let why = why.to_owned();
        let a = self
            .apply(chain_id, ChainCmd::Resume(ResumeCmd { why, by }), Records::default())
            .await?;
        if !a.changed {
            return Ok(());
        }
        // each paused run issues what came due while the chain was paused
        self.resume_runs(chain_id).await
    }

    /// The owner ending an agent's session for good, saying why when he gives a reason (empty: none).
    pub async fn stop_card(self: &Arc<Self>, req: Request, card_id: String, why: String) -> Result<Card> {
        let by = req.by.clone();
        self.act(req, "StopCard", move |o| async move {
            o.stop_for_owner(&card_id, &why, &by).await
        })
        .await
    }

    /// The owner saying, after he stopped an agent, why he stopped it: its Chair is told, with what it may not do on its
    /// own, and the agent's stream says it.
    pub async fn explain_stop(self: &Arc<Self>, req: Request, card_id: String, why: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "ExplainStop", move |o| async move {
            let why = why.trim().to_owned();
            if why.is_empty() {
                return Err(Error::refused("say why the agent was stopped"));
            }
            let id = card_id.clone();
            let c = o.store.read(move |r| r.card(&id)).await?;
            if c.status() != &CardStatus::Stopped {
                return Err(Error::refused(format!("{} is not stopped", c.name)));
            }
            let activity = format!("stopped: {why}");
            let w = Records {
                cards: vec![CardEdit::new(&c.id, move |k| k.activity.clone_from(&activity))],
                ..Records::default()
            };
            let told = Told {
                who: o.who_acted(&by).await,
                why,
                after: true,
            };
            o.store_told(&c, w, Some(&told), &by).await
        })
        .await
    }

    /// Stops card_id for the owner (by him, or by the Assistant on his words), for why: as `stop`, and its Chair is told in
    /// the same change, with the why and that it may not restart the agent on its own; the agent's stream says who stopped
    /// it and that the Chair was told.
    pub async fn stop_for_owner(self: &Arc<Self>, card_id: &str, why: &str, by: &Actor) -> Result<Card> {
        let told = Told {
            who: self.who_acted(by).await,
            why: why.trim().to_owned(),
            after: false,
        };
        let recorded = or_default(
            &told.why,
            &format!("stopped by {}", told.who.trim_end_matches(',').to_lowercase()),
        );
        self.stop_with(card_id, &recorded, by, Some(&told)).await
    }

    /// Ends a card's session for good, and the messages still waiting for it with it; by stopped it, for why. A
    /// judge stopped ends what it judges. The card's lock is held from the close to the store, so no session opens
    /// for the card in between: one a dispatch opens comes before the stop, which closes it, or after it, which reads
    /// the card stopped and, with nothing queued, opens none.
    pub async fn stop(self: &Arc<Self>, card_id: &str, why: &str, by: &Actor) -> Result<Card> {
        self.stop_with(card_id, why, by, None).await
    }

    /// `stop`, telling what told says in the same change.
    async fn stop_with(self: &Arc<Self>, card_id: &str, why: &str, by: &Actor, told: Option<&Told>) -> Result<Card> {
        let id = card_id.to_owned();
        if let Ok(c) = self.store.read(move |r| r.card(&id)).await
            && crate::spawn::spawned(&c)
        {
            self.stop_spawn(&c).await?; // a child's task ends in its parent's process, which goes on
        }
        let c = {
            let _card = self.closing(card_id, &model::Cut::AgentStopped).await;
            let id = card_id.to_owned();
            let c = self.store.read(move |r| r.card(&id)).await?;
            let mut w = stopped(card_id, why, self.now());
            w.decisions = vec![Decided {
                kind: model::DecisionKind::Stop,
                text: format!("Stop {}.", c.name),
                why: why.to_owned(),
                card_id: card_id.to_owned(),
                by: by.clone(),
            }];
            self.store_told(&c, w, told, by).await?;
            let id = card_id.to_owned();
            self.store.read(move |r| r.card(&id)).await?
        };
        // a judge stopped ends what it judges, without its answer
        match CardRole::of(&c) {
            CardRole::Judge(JudgeKind::Goal) => {
                let why = format!("the judge was stopped before it gave a verdict: {why}");
                self.domains()?
                    .end_goal_check(&c.home, card_id, &why, false, crate::apply::Records::default())
                    .await?;
            }
            CardRole::Judge(JudgeKind::Duel) => {
                let why = format!("the judge was stopped before it picked: {why}");
                self.domains()?.duel_judge_ended(&c, &why).await;
            }
            CardRole::Judge(JudgeKind::Round) => {
                let why = format!("the judge was stopped before it picked: {why}");
                self.domains()?.round_judge_ended(&c, &why).await;
            }
            CardRole::Judge(JudgeKind::Second) => {
                let why = format!("the judge was stopped before it compared: {why}");
                self.domains()?.second_judge_ended(&c, &why).await;
            }
            CardRole::Chair | CardRole::Assistant | CardRole::Worker | CardRole::Spawned => {}
        }
        Ok(c)
    }

    /// Stores w, a change of stopped card c, with what told says of the stop: its Chair's message, sent with w when c is
    /// an agent of a chain not done and not the Chair itself, and the line in c's stream that says who stopped it and
    /// whether the Chair was told. The stop is stored whatever becomes of telling: when the Chair cannot be read or told,
    /// w is stored alone and the line says so. None tells nothing.
    async fn store_told(&self, c: &Card, mut w: Records, told: Option<&Told>, by: &Actor) -> Result<()> {
        let Some(told) = told else {
            self.commit(None, None, w).await?;
            return Ok(());
        };
        let line = |reached: Reached| Event {
            home: c.home.clone(),
            card_id: c.id.clone(),
            status: model::EventStatus::System(model::SystemNote::Plain),
            by: Some(by.clone()),
            text: stop_line(told, &c.name, &reached),
            ..Event::default()
        };
        let chair = match self.chair_told_of_stop(c).await {
            Ok(Some(chair)) => chair,
            Ok(None) => {
                w.events.push(line(Reached::NotAsked));
                self.commit(None, None, w).await?;
                return Ok(());
            }
            Err(err) => {
                tracing::warn!(card = %c.id, %err, "the Chair of a stopped card is not read");
                w.events.push(line(Reached::Failed(err.to_string())));
                self.commit(None, None, w).await?;
                return Ok(());
            }
        };
        let mut sent = w.clone();
        sent.events.push(line(Reached::Told));
        let title = format!("{} was stopped", c.name);
        let m = crate::outbox::owner_message(&chair.id, &title, &stop_note(&c.name, &c.id, &told.why), by.clone());
        if let Err(err) = self.send(&chair, m, sent).await {
            tracing::warn!(card = %c.id, chair = %chair.id, %err, "the Chair of a stopped card is not told");
            w.events.push(line(Reached::Failed(err.to_string())));
            self.commit(None, None, w).await?;
        }
        Ok(())
    }

    /// The Chair told when card c stops (`stop_tells_chair`), when there is one.
    async fn chair_told_of_stop(&self, c: &Card) -> Result<Option<Card>> {
        if crate::homes::standalone_card(c) {
            return Ok(None);
        }
        let ch = self.chain(&c.home).await?;
        if !stop_tells_chair(c, Some(&ch)) {
            return Ok(None);
        }
        let id = ch.chair_id.clone();
        Ok(Some(self.store.read(move |r| r.card(&id)).await?))
    }

    /// Who acted, as an agent's stream names them: the owner, the Assistant for him, or an outside agent by its key.
    pub async fn who_acted(&self, by: &Actor) -> String {
        if by.kind == model::ActorKind::Client {
            format!("An outside agent ({})", self.client_key_label(&by.who).await)
        } else if by.kind == model::ActorKind::Assistant {
            "The Assistant, for the owner,".to_owned()
        } else {
            "The owner".to_owned()
        }
    }

    /// Closes card's run for why and holds its session lock until the caller has stored the card's stop (`stopped`) and
    /// drops the guard: the run closes first, so no reply is recorded after it and the card read next says whether one
    /// came.
    pub async fn closing(self: &Arc<Self>, card_id: &str, why: &model::Cut) -> crate::keyed_locks::KeyGuard {
        let guard = self.sessions.lock(card_id).await;
        self.close_run(card_id, why);
        guard
    }

    /// The owner interrupting a card's current turn; the card stays available for more prompts.
    pub async fn cancel(self: &Arc<Self>, req: Request, card_id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "Cancel", move |o| async move {
            let r = o
                .run_of(&card_id)
                .ok_or_else(|| Error::refused(format!("card {card_id} is not running")))?;
            o.interrupt(&r, &by).await
        })
        .await
    }

    /// Ends the turn r's agent is in, and its stream says who ended it.
    pub async fn interrupt(&self, r: &crate::session::Run, by: &Actor) -> Result<()> {
        let who = self.who_acted(by).await;
        r.cancel_turn(&model::Cut::Interrupted { who: who.clone() }).await?;
        let c = r.card_now();
        self.add_event(Event {
            home: c.home,
            card_id: c.id,
            status: model::EventStatus::System(model::SystemNote::Plain),
            by: Some(by.clone()),
            text: format!("{who} interrupted the turn."),
            ..Event::default()
        })
        .await?;
        Ok(())
    }

    /// Ends the turn agent c is in for someone the owner trusts with it (his Assistant, a client key's holder), its session
    /// kept, and records on its chain that by ended it and why, as a decision of kind.
    pub async fn cancel_turn_as(&self, c: &Card, why: &str, kind: DecisionKind, by: Actor) -> Result<()> {
        let r = self
            .run_of(&c.id)
            .ok_or_else(|| Error::refused(format!("agent {} is not in a turn", c.id)))?;
        self.interrupt(&r, &by).await?;
        let text = format!("Ended {}'s turn.", c.name);
        self.decide(&c.home, kind, &text, why, &c.id, by).await;
        Ok(())
    }

    /// Blocks while card c may take no message: while its chain is paused. The Assistant is in no chain, so only a
    /// shutdown stops it.
    pub async fn wait_open(&self, c: &Card) -> Result<()> {
        if !crate::homes::standalone_card(c) {
            return self.wait_unpaused(&c.home).await;
        }
        if self.is_stopping() {
            Err(Error::Stopping)
        } else {
            Ok(())
        }
    }

    /// Blocks while the chain is paused; it errors when the chain is gone or the server is shutting down. It returns
    /// through the chain's lock: the change that ended the pause has closed the sessions it ends by then, so a session
    /// loop does not take a message from under it.
    pub async fn wait_unpaused(&self, chain_id: &str) -> Result<()> {
        let cs = self.state(chain_id).await?;
        loop {
            let mut gate = {
                let held = cs.held.lock().await;
                if held.gone {
                    return Err(Error::ChainDeleted);
                }
                held.unpaused.subscribe()
            };
            tokio::select! {
                open = gate.wait_for(|open| *open) => { open.map_err(|_| Error::ChainDeleted)?; }
                () = self.stopped.cancelled() => return Err(Error::Stopping),
            }
            if *cs.held.lock().await.chain.status() != ChainStatus::Paused {
                return Ok(());
            }
        }
    }

    /// The owner finishing a chain with a summary.
    pub fn finish(
        self: &Arc<Self>,
        req: Request,
        chain_id: String,
        summary: String,
    ) -> impl std::future::Future<Output = Result<Done>> + Send + use<> {
        let by = req.by.clone();
        self.act_on_chain(req, "Finish", chain_id, move |o, chain_id| async move {
            let summary = or_default(&summary, "finished by the owner");
            let cmd = FinishCmd {
                text: "Finish chain.".to_owned(),
                why: finish_why(&summary),
                summary,
                by,
                unjudged: Some(Unjudged::Owner),
                ..FinishCmd::default()
            };
            o.finish_chain(&chain_id, cmd, Records::default()).await
        })
    }

    /// Closes every session of the chain, and its open questions and errors: nothing waits on a finished chain. A chain
    /// finishes once: the goal check and the stall rail may both try, and the second is refused.
    pub async fn finish_chain(&self, chain_id: &str, cmd: FinishCmd, w: Records) -> Result<()> {
        let summary = cmd.summary.clone();
        let a = self.apply(chain_id, ChainCmd::Finish(cmd), w).await?;
        if *a.chain.status() == ChainStatus::Done {
            self.domains()?
                .tell_judge_what_verification_found(&a.chain, &summary)
                .await;
        }
        Ok(())
    }
}

impl Core {
    /// The active agents in staff that hold a card open in any chain (`holds_open`): a reuse leaves them out.
    pub async fn busy_of(&self, staff: &[model::Employee]) -> Result<std::collections::HashSet<String>> {
        let active: Vec<String> = staff
            .iter()
            .filter(|e| e.status == model::EmployeeStatus::Active)
            .map(|e| e.id.clone())
            .collect();
        self.store
            .read(move |r| {
                let mut busy = std::collections::HashSet::new();
                for id in active {
                    if r.employee_cards(&id)?.iter().any(crate::lifecycle::holds_open) {
                        busy.insert(id);
                    }
                }
                Ok(busy)
            })
            .await
    }
}

/// by, or Remit when a chain from before Remit recorded who asked for it has none.
pub fn or_default_actor(by: Option<&Actor>) -> Actor {
    by.cloned().unwrap_or_else(by_remit)
}

/// ch with the task objective of its own request when it serves no key result of an objective the owner approved,
/// stored in tx: the caller stores the chain in the same transaction, so it never exists without the key result it
/// points at. owner answers for its key result (empty: the owner).
pub fn with_task_objective(tx: &store::Tx<'_>, mut ch: Chain, owner: model::Accountable) -> Result<Chain> {
    if ch.serves.is_some() {
        return Ok(ch);
    }
    ch.serves = Some(Serves {
        objective: task_objective_id(&ch.id),
        key_result: task_key_result_id(&ch.id),
    });
    tx.put_objective(task_objective_of(&ch, owner))?;
    Ok(ch)
}

/// The id of the objective of a direct request.
pub fn task_objective_id(chain_id: &str) -> String {
    format!("o_{chain_id}")
}

/// The id of the one key result of a direct request.
pub fn task_key_result_id(chain_id: &str) -> String {
    format!("k_{chain_id}")
}

/// The objective of one direct request (ADR-0031): no scope, so the objective list leaves it out; the owner answers for
/// it and judges it; its one key result is his words, with no check until the Chair declares one in its first turn, and
/// the chain does not finish without it (ADR-0068). A chain Remit opens for an employee's turn names that employee as
/// owner, so the employee is who works it.
fn task_objective_of(ch: &Chain, owner: model::Accountable) -> Objective {
    let owner = if owner.0.is_empty() {
        model::Accountable::owner()
    } else {
        owner
    };
    Objective {
        id: task_objective_id(&ch.id),
        task: true,
        status: ObjectiveStatus::Active,
        outcome: ch.goal.clone(),
        owner: owner.clone(),
        key_results: vec![KeyResult {
            id: task_key_result_id(&ch.id),
            text: ch.goal.clone(),
            owner,
            judge: model::Accountable::owner(),
            ..KeyResult::default()
        }],
        created_at: ch.created_at,
        ..Objective::default()
    }
}

/// What the Chair reads when the owner stops its agent name (card id), why being his reason (empty: none given).
fn stop_note(name: &str, id: &str, why: &str) -> String {
    let reason = if why.is_empty() {
        "He gave no reason.".to_owned()
    } else {
        format!("Why: {why}")
    };
    format!(
        "I stopped {name} ({id}). {reason}\nDo not restart it, resume it or route its work to it again on your own. If its work still has to be done, plan around the stop or ask me first."
    )
}

/// What a stop tells of itself: who stopped the agent (`who_acted`), why (empty: no reason given), and whether the why
/// comes after the stop.
struct Told {
    who: String,
    why: String,
    after: bool,
}

/// The line in a stopped agent's stream: who stopped it and why, or why they had, and whether its Chair was told.
fn stop_line(t: &Told, name: &str, reached: &Reached) -> String {
    let who = t.who.trim_end_matches(',');
    let line = if t.after {
        format!("{who} said why {name} was stopped: {}", t.why)
    } else if t.why.is_empty() {
        format!("{who} stopped {name}")
    } else {
        format!("{who} stopped {name}: {}", t.why)
    };
    match reached {
        Reached::Told => format!("{line}. The Chair was told."),
        Reached::NotAsked => format!("{line}."),
        Reached::Failed(why) => format!("{line}. The Chair could not be told: {why}."),
    }
}

/// Whether stopping card c, of chain ch (none: a card in no chain), tells a Chair: only an agent of a chain not done
/// that is not the Chair itself has one to tell.
pub fn stop_tells_chair(c: &Card, ch: Option<&Chain>) -> bool {
    ch.is_some_and(|ch| *ch.status() != ChainStatus::Done && ch.chair_id != c.id)
}

/// What stopping card c, of chain ch, does, in the server's words: what the owner reads before he stops it.
pub fn stop_says(c: &Card, ch: Option<&Chain>) -> String {
    const ENDS: &str = "Its session ends for good and the messages waiting for it are dropped.";
    let who = match ch {
        _ if stop_tells_chair(c, ch) => {
            return format!("{ENDS} The Chair reads your why and is told not to restart it on its own.");
        }
        Some(ch) if ch.chair_id == c.id => "It is the Chair, so no one is told",
        Some(_) => "Its chain is finished, so no Chair is told",
        None => "No Chair is told",
    };
    format!("{ENDS} {who}: your why stays in its stream.")
}

/// What became of telling the Chair of a stop.
enum Reached {
    Told,
    /// No Chair is told: the card is the Chair, in no chain, or in a chain that is done.
    NotAsked,
    Failed(String),
}

/// The change that stops card card_id for why: its status, and the messages waiting for it dropped.
pub fn stopped(card_id: &str, why: &str, now: model::Time) -> Records {
    let activity = if why.is_empty() {
        String::new()
    } else {
        format!("stopped: {why}")
    };
    Records {
        cards: vec![CardEdit::new(card_id, move |c| {
            c.set_status(CardStatus::Stopped, now);
            if !activity.is_empty() {
                c.activity.clone_from(&activity);
            }
        })],
        drops: vec![Drop {
            card_id: card_id.to_owned(),
            min: crate::outbox::PRIORITY_OTHER,
        }],
        ended: vec![card_id.to_owned()],
        ..Records::default()
    }
}

impl Core {
    /// Names key id for the owner who reads what it did: by its name, or as deleted once it is gone.
    pub async fn client_key_label(&self, id: &str) -> String {
        let key = id.to_owned();
        match self.store.read(move |r| r.client_key(&key)).await {
            Ok(k) => format!("client key {}", k.name),
            Err(_) => format!("deleted client key {id}"),
        }
    }
}

/// Whether the employee at work in card c holds it open: it is in a turn, opening one or waiting for a provider to run
/// it, or its session has opened and its brief waits to be taken (idle before its first turn).
pub fn holds_open(c: &Card) -> bool {
    at_work(c) || c.status() == &CardStatus::Waiting || (c.status() == &CardStatus::Idle && c.turns == 0)
}
