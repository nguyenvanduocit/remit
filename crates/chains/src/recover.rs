//! session.rs enters `host_restarted` when a session's `agent::Handler::on_exit` (or a turn's error) carries
//! `agent::Exit::HostRestarted`: the session resumes, its work cut, not failed.
//!
//! What a restart leaves: the previous process's agents are gone, so Recover marks their cards stopped and their work
//! ended, and ResumeInterrupted carries on where it stopped.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use model::{
    CardStatus, Chain, ChainStatus, Event, EventKind, EventStatus, InboxKind, InboxStatus, JobStatus, PromptStatus,
    Retry, SystemNote,
};
use tokio_util::sync::CancellationToken;

use crate::apply::Records;
use crate::chain_core::Core;
use crate::inbox::{Cause, Failed, failure_words, who_of};
use crate::outbox::{Message, restart_by, sent_by};
use crate::recap::RECAP_EVENTS;
use crate::schedule::at_work;
use crate::submit::job_repo;
use crate::{Error, Result};
use model::Cut;

/// A card a restart cut off in a running chain: mid-turn, or with background tasks running (lost: their titles), which
/// its agent may still need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interruption {
    pub card_id: String,
    pub lost: Vec<String>,
}

/// What a card a restart cut off is told when its session resumes.
pub const SERVER_RESTARTED: &str =
    "[The server restarted while you were working. Your session is resumed; continue where you stopped.]";

/// What a card the AI runtime's restart cut off is told when its session resumes.
const RUNTIME_RESTARTED: &str =
    "[The AI runtime restarted while you were working. Your session is resumed; continue where you stopped.]";

/// How many times in a row restarts resume a card in the same turn before it is parked: a process that dies in the
/// same turn at every start (a crash loop under a supervisor) would otherwise spend on it at each.
const MAX_RESUMES: usize = 3;

impl Core {
    /// Marks what a previous process left running: its agent processes are gone, so their cards are stopped, their tool
    /// calls end failed, their background tasks end and their permission requests are cancelled. The cards it cut off
    /// mid-turn in running chains are noted for `resume_interrupted`; the messages it queued wait in the store, which
    /// `resume_interrupted` hands over. The files of chains it deleted but did not get to remove are removed; what cannot
    /// be is put in the owner's inbox.
    pub async fn recover(&self) -> Result<()> {
        // `new` is not async, so the first step of a start reads the accounts (accounts.rs)
        self.load_accounts().await;
        self.settle_requests().await?;
        for c in self.store.read(|r| r.cleanups()).await? {
            if let Err(err) = self.clean_up(&c).await {
                self.report_left(&c.chain_id, &err).await;
            }
        }
        let chains = self.store.read(|r| r.chains()).await?;
        for ch in &chains {
            self.end_lost_jobs(ch).await?;
            self.recover_cards(&ch.id, ch.status()).await?;
        }
        self.restore_facts_answers(&chains).await?;
        self.note_handovers(&chains).await?;
        match self.store.read(|r| r.assistant()).await {
            // in no chain, nothing holds it: it carries on as a running chain's agents do
            Ok(a) => self.recover_cards(&a.id, &ChainStatus::Running).await?,
            Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
        let now = self.now();
        let finished: HashMap<String, bool> = chains
            .iter()
            .map(|ch| (ch.id.clone(), *ch.status() == ChainStatus::Done))
            .collect();
        self.store
            .update(move |tx| {
                for (id, done) in &finished {
                    if *done {
                        // nothing waits on a finished chain: what an earlier build left queued for it is dropped
                        tx.drop_chain_queue(id, now)?;
                    }
                }
                for mut it in tx.open_inbox()? {
                    // Permission requests die with their agent process; anything else of a finished chain waits on
                    // nothing.
                    if it.kind == InboxKind::Permission || finished.get(&it.home).copied().unwrap_or_default() {
                        it.status = InboxStatus::Cancelled;
                        tx.put_inbox(it)?;
                    }
                }
                Ok::<_, store::Error>(())
            })
            .await?;
        Ok(())
    }

    /// Marks the cards of home (a chain in status, or the Assistant, which goes on as a running chain does) stopped,
    /// their tool calls failed and their background tasks ended, and notes those it cut off mid-turn or mid-task for
    /// `resume_interrupted` when the home goes on working (a running chain). A card stored stopped is not cut off:
    /// something stopped it on purpose, and a crash before its process and tasks were ended does not undo that.
    async fn recover_cards(&self, home: &str, status: &ChainStatus) -> Result<()> {
        let going_on = *status == ChainStatus::Running;
        let key = home.to_owned();
        let cards = self.store.read(move |r| r.cards(&key)).await?;
        let mut lost = self.end_lost_tasks(home).await?;
        crate::spawn::cut_children(&cards, &mut lost); // a child is never resumed: its parent is told it was cut
        for c in &cards {
            // Claude Code's workflow runs died with their processes: each card is told the call that resumes its own
            let cut = self.cut_claude_runs(c, &Cut::ServerRestarted).await;
            if !cut.is_empty() {
                lost.entry(c.id.clone()).or_default().extend(cut);
            }
        }
        let mut stopped = Vec::new();
        for mut c in cards {
            self.end_tool_calls(&c.id, 0, &Cut::ServerRestarted.to_string()).await?;
            let lost_here = lost.get(&c.id).cloned().unwrap_or_default();
            // a card being handed over is finished by note_handovers, not resumed
            let cut = at_work(&c) || !lost_here.is_empty() && c.status() != &CardStatus::Stopped;
            if going_on && c.handover.is_none() && cut && !c.session_id.is_empty() {
                self.shared().interrupted.push(Interruption {
                    card_id: c.id.clone(),
                    lost: lost_here,
                });
            }
            // No agent process outlives this server, so no card that had not ended is still reachable. A card waiting
            // for a provider had none: it waits on, and resume_interrupted opens it with the others whose messages are
            // queued.
            if !matches!(
                c.status(),
                CardStatus::Stopped | CardStatus::Failed | CardStatus::Waiting
            ) {
                c.set_status(CardStatus::Stopped, self.now());
                "server restarted".clone_into(&mut c.activity);
                c.updated_at = self.now();
                stopped.push(c);
            }
        }
        self.store
            .update(move |tx| {
                for c in stopped {
                    tx.put_card(c)?;
                }
                Ok::<_, store::Error>(())
            })
            .await?;
        Ok(())
    }

    /// Carries on where the previous process stopped. The sessions of cards it cut off mid-turn resume with a message
    /// saying so: the same turn goes on, so a wait the Chair planned in it stands. A Chair whose turn had ended before its
    /// wait was settled has it settled now. A submit it left goes on from the step it reached. Then every message still
    /// queued is handed to its card, opening its session, except a failed card's, which waits for the owner's retry.
    pub async fn resume_interrupted(&self) {
        let cuts = std::mem::take(&mut self.shared().interrupted);
        self.finish_handovers().await;
        for cut in &cuts {
            self.resume_after_restart(cut, SERVER_RESTARTED).await;
        }
        let chains = match self.store.read(|r| r.chains()).await {
            Ok(chains) => chains,
            Err(err) => return tracing::error!(%err, "resume after restart"),
        };
        for ch in &chains {
            if *ch.status() == ChainStatus::Running && !cuts.iter().any(|c| c.card_id == ch.chair_id) {
                let waits = self.waits_of(&ch.id).await.unwrap_or_default();
                let chair = ch.chair_id.clone();
                if !waits.is_empty()
                    && let Ok(chair) = self.store.read(move |r| r.card(&chair)).await
                {
                    self.settle_wait(&chair, Records::default()).await;
                }
            }
            if let Ok(d) = self.domains() {
                for rw in ch.repos.iter().filter(|rw| rw.submit.is_some()) {
                    d.carry_on_later(&ch.id, &rw.repo).await;
                }
            }
        }
        let queued = match self.store.read(|r| r.queued_prompts()).await {
            Ok(q) => q,
            Err(err) => return tracing::error!(%err, "deliver after restart"),
        };
        let mut handed: Vec<&str> = Vec::new();
        for e in &queued {
            if handed.contains(&e.card_id.as_str()) {
                continue;
            }
            handed.push(&e.card_id);
            let id = e.card_id.clone();
            if let Ok(c) = self.store.read(move |r| r.card(&id)).await
                && c.status() != &CardStatus::Failed
            {
                self.dispatch(&e.card_id).await;
            }
        }
    }

    /// Handles run r, whose AI runtime died under it: its run closes and its card stops, as a server restart stops it,
    /// and a turn it cut or background work it lost resumes at once, by the same step a server restart takes. An idle
    /// card waits: the message that reaches it next resumes its session. A run closed already (stopped, or handled by the
    /// other of its two callers, the exit and the turn) is left as it is. Whether a turn was cut is read here, from the
    /// run, so both callers decide the same: a message's turn stays open (its prompt) until its end is recorded, and the
    /// turn leaves it open when the runtime's end cut it; a turn of the agent's own is open while it is.
    pub async fn host_restarted(self: &Arc<Self>, r: &Arc<crate::session::Run>) {
        let mid_turn = {
            let s = r.state();
            s.prompt != 0 || s.own.is_some()
        };
        let mut lost = self.watching(r).await;
        lost.extend(self.children_at_work(r));
        if !self.close_run_obj(r, &Cut::RuntimeRestarted) {
            return;
        }
        let card_id = r.card_id();
        let now = self.now();
        let stopped = self
            .update_card(&card_id, move |c| {
                c.set_status(CardStatus::Stopped, now);
                "the AI runtime restarted".clone_into(&mut c.activity);
            })
            .await;
        if let Err(err) = stopped {
            return tracing::error!(card = %card_id, %err, "stop a card whose runtime restarted");
        }
        tracing::warn!(card = %card_id, mid_turn, lost_tasks = lost.len(), "the AI runtime restarted under a card");
        if mid_turn || !lost.is_empty() {
            self.resume_after_restart(&Interruption { card_id, lost }, RUNTIME_RESTARTED)
                .await;
        }
    }

    /// Whether the server has begun to stop: Drain, its first step, has begun. A supervisor stopping the server signals
    /// every process it started at once, so from then on an agent's turn that fails or its process that exits is the
    /// stop's doing, not the agent's.
    pub fn stop_begun(&self) -> bool {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .draining
            || self.is_stopping()
    }

    /// Whether Shutdown has begun.
    pub fn is_stopping(&self) -> bool {
        self.stopped.is_cancelled()
    }

    /// Resumes the card a restart cut off by sending it text (and what it ran in the background that ended), unless
    /// restarts resumed it too often in a row already (`park`).
    async fn resume_after_restart(&self, cut: &Interruption, text: &str) {
        if self.park(&cut.card_id).await {
            return;
        }
        let mut text = text.to_owned();
        if !cut.lost.is_empty() {
            text = format!(
                "{} What you ran in the background ended with the old process: {}. Start again what you still need.]",
                text.trim_end_matches(']'),
                cut.lost.join("; ")
            );
        }
        let id = cut.card_id.clone();
        let sent = match self.store.read(move |r| r.card(&id)).await {
            Ok(c) => {
                let m = Message {
                    title: "Resume after a restart".to_owned(),
                    text,
                    by: restart_by(),
                    ..Message::default()
                };
                self.send(&c, m, Records::default()).await.map(|_| ())
            }
            Err(err) => Err(err.into()),
        };
        if let Err(err) = sent {
            tracing::warn!(card = %cut.card_id, %err, "resume after restart");
        }
    }

    /// Keeps a restart from resuming card id again once restarts resumed it `MAX_RESUMES` times in a row: the card is
    /// failed, saying why, and put in front of the owner, whose retry resumes it. It says whether it parked the card.
    async fn park(&self, card_id: &str) -> bool {
        let id = card_id.to_owned();
        let events = match self.store.read(move |r| r.card_events(&id, RECAP_EVENTS)).await {
            Ok(events) => events,
            Err(err) => {
                tracing::warn!(card = %card_id, %err, "resume after restart");
                return false;
            }
        };
        let (n, last, queued) = resumes_in_a_row(&events);
        if n < MAX_RESUMES {
            return false;
        }
        let why = format!(
            "parked: the server restarted {n} times in a row while this agent was in the same turn, which never ended, so Remit does not resume it again on its own; retry when you want it to go on"
        );
        let parked = Failed::new(Cause::Restarts, why);
        let (f, now) = (parked.clone(), self.now());
        let c = match self
            .update_card(card_id, move |c| {
                c.set_status(CardStatus::Failed, now);
                c.error = failure_words(&who_of(c), &f, now).title;
                "parked after repeated restarts".clone_into(&mut c.activity);
            })
            .await
        {
            Ok(c) => c,
            Err(err) => {
                tracing::error!(card = %card_id, %err, "park a card");
                return false;
            }
        };
        tracing::warn!(chain = %c.home, card = %card_id, resumes = n, "card parked after repeated restarts");
        let retry = if queued {
            // the newest resume was never taken: opening the session reads it
            Retry {
                card_id: card_id.to_owned(),
                reopen: true,
                ..Retry::default()
            }
        } else {
            Retry {
                card_id: card_id.to_owned(),
                prompt_id: last,
                ..Retry::default()
            }
        };
        self.report_error(&c, &parked, Some(retry)).await;
        true
    }

    /// Runs act in the background (`background`) as work of the chain: its token is also cancelled when the chain
    /// pauses, finishes or is deleted, so pausing a chain holds everything it does.
    pub async fn chain_work<F, Fut>(self: &Arc<Self>, chain_id: &str, act: F) -> Result<()>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let cs = self.state(chain_id).await?;
        let held = cs.held.lock().await.work.clone();
        let started = self.background(move |work| async move {
            // cancelled with the work's lifetime, and with the chain's below
            let token = work.child_token();
            let chain_ends = token.clone();
            let act = act(token);
            tokio::pin!(act);
            tokio::select! {
                () = &mut act => return,
                () = held.cancelled() => chain_ends.cancel(),
            }
            act.await;
        });
        if started { Ok(()) } else { Err(Error::Stopping) }
    }

    /// Runs act on a task of its own as work Shutdown waits for, with the work's lifetime as its token. Once Shutdown
    /// has begun nothing new starts, and it says false.
    pub fn background<F, Fut>(&self, act: F) -> bool
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        // under the shared lock, as shutdown cancels `stopped` under it: all work `wait` sees has started before it
        let _sh = self.shared();
        if self.stopped.is_cancelled() {
            return false;
        }
        self.work.spawn(act(self.work_ends.clone()));
        true
    }

    /// Ends the work in the background before the process exits: nothing new starts, what is at work may finish until
    /// deadline is cancelled, then what is left is cut short and every session closes, and Shutdown returns once the work
    /// has stopped, so nothing it started outlives it. The cards keep the state Recover reads and unread messages stay
    /// queued, so the next start resumes the cards and delivers the messages. A turn in progress is cut short first (its
    /// end is not recorded: no report, no status change).
    pub async fn shutdown(&self, deadline: &CancellationToken) {
        let runs: Vec<_> = {
            let sh = self.shared();
            self.stopped.cancel();
            sh.runs.values().cloned().collect()
        };
        for r in &runs {
            let busy = r.busy();
            r.close_box();
            if busy {
                self.cut_turn_of(r, &Cut::ServerStopped).await;
            }
        }
        self.work.close();
        let finished = tokio::select! {
            () = self.work.wait() => true,
            () = deadline.cancelled() => false,
        };
        if !finished {
            tracing::warn!("shutdown: work still running at the deadline is cut short");
        }
        self.work_ends.cancel();
        // work still running may wait on a process (a prompt, a child's permission request): it ends with its session
        for r in &runs {
            let sess = r.state().sess.clone();
            if let Some(sess) = sess {
                sess.close();
            }
        }
        self.work.wait().await;
    }

    /// Ends the background tasks of home's cards still running: the agent processes that ran them are gone. It returns
    /// their titles by card.
    async fn end_lost_tasks(&self, home: &str) -> Result<HashMap<String, Vec<String>>> {
        let (key, now) = (home.to_owned(), self.now());
        let lost = self
            .store
            .update(move |tx| {
                let mut lost: HashMap<String, Vec<String>> = HashMap::new();
                for mut e in tx.running_tasks(&key)? {
                    Cut::ServerRestarted.stop_task(&mut e);
                    e.updated_at = now;
                    let e = tx.update_event(e)?;
                    lost.entry(e.card_id).or_default().push(e.title);
                }
                Ok::<_, store::Error>(lost)
            })
            .await?;
        Ok(lost)
    }

    /// Ends the chain's running jobs that no submit carries on: the process that ran them is gone. A job whose submit is
    /// recorded stays running: `resume_interrupted` carries the submit on, and its end ends the job.
    async fn end_lost_jobs(&self, ch: &Chain) -> Result<()> {
        let (id, now) = (ch.id.clone(), self.now());
        let submits: Vec<String> = ch
            .repos
            .iter()
            .filter(|rw| rw.submit.is_some())
            .map(|rw| rw.repo.clone())
            .collect();
        self.store
            .update(move |tx| {
                for mut j in tx.running_jobs(&id)? {
                    if submits.contains(&job_repo(&j)) {
                        continue;
                    }
                    j.status = EventStatus::Job(JobStatus::Failed);
                    j.text = model::cut_short(&Cut::ServerRestarted.to_string());
                    j.updated_at = now;
                    tx.update_event(j)?;
                }
                Ok::<_, store::Error>(())
            })
            .await?;
        Ok(())
    }
}

/// How many times in a row restarts resumed the card whose events (its newest, oldest first) these are, its turn never
/// ending since (no stop, no failure); and the newest of those resume messages, and whether it still waits in the queue.
/// Pure.
fn resumes_in_a_row(events: &[Event]) -> (usize, i64, bool) {
    let (mut n, mut last, mut queued) = (0_usize, 0_i64, false);
    for e in events.iter().rev() {
        if matches!(e.status, EventStatus::Stop(_) | EventStatus::System(SystemNote::Error)) {
            break;
        }
        if e.kind() == EventKind::Prompt && sent_by(e) == restart_by() {
            if n == 0 {
                last = e.id;
                queued = e.status == EventStatus::Prompt(PromptStatus::Queued);
            }
            n = n.saturating_add(1);
        }
    }
    (n, last, queued)
}
