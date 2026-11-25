//! A subagent Claude Code spawns inside a card's process is a standing employee at work (ADR-0073): a child card of the
//! employee its type names, bound to that process by `Card::spawn` and hired by the card that spawned it. Its card has a
//! run, so what the subagent streams, the tools it calls and the permissions it asks land on it as on any card
//! (session.rs's `RunHandler`), but the run holds no session of its own: the process is its root's. So the rules that
//! assume a session of its own leave it alone (no account, no mailbox: a message to it is refused), it ends with its
//! report, its stop is a `stop_task` in its root's process, and it ends stopped when that process ends. A child is never
//! resumed. Pure: `spawn_card`, `spawn_end`, `cut_children`.

use std::collections::HashMap;
use std::sync::Arc;

use model::{
    Card, CardStatus, Edge, EdgeKind, Event, EventStatus, Loadout, Origin, Spawn, SystemNote, TaskStatus, Time,
};

use crate::apply::Records;
use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::schedule::at_work;
use crate::session::{CANCEL_WAIT, Run};
use crate::{Error, Result};
use model::Cut;

/// Whether card c is a subagent at work in another card's process: it holds no session of its own.
pub fn spawned(c: &Card) -> bool {
    c.spawn.is_some()
}

/// What a subagent task's report comes to, worked out as the process says it and done in the order of its reports.
pub enum SpawnStep {
    /// Records the child card of a spawn, its run made already (`Run::child_of`), which spawner spawned.
    Record { child: Arc<Run>, spawner: Card },
    /// Stops the spawn of task (titled title) by spawner in root's process, which is refused for why.
    Refuse {
        root: Arc<Run>,
        spawner: Card,
        task: String,
        title: String,
        why: String,
    },
    /// Ends child, whose task ended as task says.
    End { child: Arc<Run>, task: wire::runtime::Task },
}

/// Who a spawn is at work as: the employee, the agent it was hired from, and what its card holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spawnee {
    pub employee: String,
    pub agent: String,
    pub loadout: Option<Loadout>,
}

impl Run {
    /// The run whose process this run's agent runs in: its own, or its root's for a child card.
    pub fn process(self: &Arc<Self>) -> Arc<Run> {
        self.root.clone().unwrap_or_else(|| Arc::clone(self))
    }

    /// The run of the child card subagent agent_id of this run's process works in; none when it has none or it has
    /// ended.
    pub fn child(self: &Arc<Self>, agent_id: &str) -> Option<Arc<Run>> {
        let child = self.process().state().children.get(agent_id).cloned();
        child.filter(|c| c.open())
    }
}

impl Core {
    /// What subagent task t, which run r's agent spawned (r's own, or a child card's for a nested spawn), comes to: its
    /// start makes its card's run, at once, so the subagent's first word finds it; its end ends it. Called as the process
    /// reports t; what it returns is done in the order of the process's reports.
    pub fn spawn_task(&self, r: &Arc<Run>, t: wire::runtime::Task) -> Option<SpawnStep> {
        let root = r.process();
        let child = root.state().children.get(&t.id).cloned();
        let running = t.status == TaskStatus::Running;
        match child {
            Some(child) if !running => Some(SpawnStep::End { child, task: t }),
            None if running => {
                let spawner = r.card_now();
                let Some(spawnee) = self.spawnee_of(&root, &spawner, &t.agent_type) else {
                    let why = format!("its type {} is not one this session was offered", t.agent_type);
                    return Some(SpawnStep::Refuse {
                        root,
                        spawner,
                        task: t.id,
                        title: t.title,
                        why,
                    });
                };
                let c = spawn_card(&spawner, spawnee, &t, self.now());
                let child = Arc::new(Run::child_of(c, Arc::clone(&root)));
                {
                    // under the root's lock, as its close is: a process that ended takes no child, and one that ends
                    // after this finds it among its children
                    let mut s = root.state();
                    if root.mailbox.is_closed() {
                        return None;
                    }
                    s.children.insert(t.id, Arc::clone(&child));
                }
                Some(SpawnStep::Record { child, spawner })
            }
            _ => None,
        }
    }

    /// Does what a subagent task's report came to.
    pub async fn on_spawn_step(self: &Arc<Self>, step: SpawnStep) {
        match step {
            SpawnStep::Record { child, spawner } => self.record_spawn(&child, &spawner).await,
            SpawnStep::Refuse {
                root,
                spawner,
                task,
                title,
                why,
            } => self.refuse_spawn(&root, &spawner, &task, &title, &why).await,
            SpawnStep::End { child, task } => self.on_spawn_ended(&child, &task).await,
        }
    }

    /// Records the card of child, which spawner's agent spawned, in one change with the hire edge from its spawner, and
    /// makes its run the card's. A card Remit could not record has its spawn stopped.
    async fn record_spawn(self: &Arc<Self>, child: &Arc<Run>, spawner: &Card) {
        if !child.open() {
            return; // its parent's process ended before it was recorded: nothing of it was kept
        }
        let c = child.card_now();
        let task = c.spawn.clone().unwrap_or_default().task;
        let edge = Edge {
            id: new_id("e_"),
            chain_id: c.home.clone(),
            from: spawner.id.clone(),
            to: c.id.clone(),
            kind: EdgeKind::Hire,
            label: crate::tools::relation_label(&EdgeKind::Hire, None),
            created_at: c.created_at,
        };
        let w = Records {
            created: vec![c.clone()],
            edges: vec![edge],
            ..Records::default()
        };
        let stored = self.commit(None, None, w).await.and_then(|a| {
            a.cards
                .into_iter()
                .next()
                .ok_or_else(|| Error::refused(format!("card {} was not stored", c.id)))
        });
        let root = child.process();
        let stored = match stored {
            Ok(stored) => stored,
            Err(err) => {
                tracing::error!(home = %c.home, card = %spawner.id, task, %err, "make a subagent's card");
                root.state().children.remove(&task);
                child.close_box();
                let why = format!("Remit could not record its card: {err}");
                return self.refuse_spawn(&root, spawner, &task, &c.name, &why).await;
            }
        };
        child.state().card = stored.clone();
        self.shared().runs.insert(stored.id.clone(), Arc::clone(child));
        if !child.open() {
            // its parent's process ended while it was recorded, and found no card to stop
            let why = "its parent's process ended";
            let now = self.now();
            if let Err(err) = self
                .update_card(&stored.id, move |c| {
                    c.set_status(CardStatus::Stopped, now);
                    why.clone_into(&mut c.activity);
                })
                .await
            {
                tracing::error!(card = %stored.id, %err, "stop a subagent's card");
            }
        }
    }

    /// Stops subagent task task_id (titled title) in root's process and records on spawner's card why it was refused.
    /// The stop is asked in the background: the process answers it as it goes on.
    async fn refuse_spawn(&self, root: &Run, spawner: &Card, task_id: &str, title: &str, why: &str) {
        let shown = if title.is_empty() { task_id } else { title };
        self.note(Event {
            home: spawner.home.clone(),
            card_id: spawner.id.clone(),
            status: EventStatus::System(SystemNote::Plain),
            text: format!("Remit stopped the subagent {shown:?}: {why}."),
            ..Event::default()
        })
        .await;
        let Some(sess) = root.state().sess.clone() else { return };
        let (task, card) = (task_id.to_owned(), spawner.id.clone());
        self.background(move |_| async move {
            match tokio::time::timeout(CANCEL_WAIT, sess.stop_task(task.clone())).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => tracing::warn!(card, task, %err, "stop a refused subagent"),
                Err(_) => tracing::warn!(card, task, "stop a refused subagent: no answer in time"),
            }
        });
    }

    /// Ends child, whose subagent task t ended: with its report, failed or stopped. A child that ended already (stopped
    /// by the owner, or with its parent's process) stays as it ended.
    async fn on_spawn_ended(self: &Arc<Self>, child: &Arc<Run>, t: &wire::runtime::Task) {
        child.process().state().children.remove(&t.id);
        self.close_stream(child).await;
        let said = child.state().turn.trim().to_owned();
        let why = Cut::SubagentEnded { status: t.status };
        if !self.close_run_obj(child, &why) {
            return;
        }
        let card_id = child.card_id();
        if t.status != TaskStatus::Completed
            && let Err(err) = self.end_tool_calls(&card_id, 0, &why.to_string()).await
        {
            tracing::error!(card = %card_id, %err, "end a subagent's tool calls");
        }
        if let Err(err) = self.update_card(&card_id, spawn_end(t, &said, self.now())).await {
            tracing::error!(card = %card_id, %err, "end a subagent's card");
        }
    }

    /// Ends the child cards still at work in run r's process, which ended for why: they end stopped, saying so, and the
    /// tool calls they left open end with them.
    pub async fn end_children(&self, r: &Run, why: &Cut) {
        let children: Vec<Arc<Run>> = r.state().children.drain().map(|(_, c)| c).collect();
        if children.is_empty() {
            return;
        }
        let ids: Vec<String> = children.iter().map(|c| c.card_id()).collect();
        self.close_sessions(children, &format!("its parent's process ended: {why}"), why)
            .await;
        for id in ids {
            if let Err(err) = self.end_tool_calls(&id, 0, &why.to_string()).await {
                tracing::error!(card = %id, %err, "end a subagent's tool calls");
            }
        }
    }

    /// Ends the subagent task of child card c in its parent's process, which goes on; a child no process runs any more
    /// has none to end.
    pub async fn stop_spawn(&self, c: &Card) -> Result<()> {
        let Some(r) = self.run_of(&c.id).filter(|r| r.open()) else {
            return Ok(());
        };
        let (Some(root), Some(spawn)) = (&r.root, &c.spawn) else {
            return Ok(());
        };
        let Some(sess) = root.state().sess.clone() else {
            return Ok(());
        };
        match tokio::time::timeout(CANCEL_WAIT, sess.stop_task(spawn.task.clone())).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(Error::refused(format!(
                "stop the subagent in its parent's process: {err}"
            ))),
            Err(_) => Err(Error::refused(
                "stop the subagent in its parent's process: it did not answer in time",
            )),
        }
    }

    /// Refuses a message to child card c: it reads none, its parent does.
    pub async fn inside_parent(&self, c: &Card) -> Error {
        let parent = c.spawn.clone().unwrap_or_default().parent;
        match self.store.read(move |r| r.card(&parent)).await {
            Ok(p) => Error::refused(format!("{} runs inside {}: talk to {}", c.name, p.name, p.name)),
            Err(err) => Error::refused(format!("{} runs inside another card's process: {err}", c.name)),
        }
    }

    /// Names the child cards still at work in run r's process.
    pub fn children_at_work(&self, r: &Run) -> Vec<String> {
        let children: Vec<Arc<Run>> = r.state().children.values().cloned().collect();
        let mut names: Vec<String> = children
            .iter()
            .map(|c| c.card_now())
            .filter(at_work)
            .map(|c| cut_child_name(&c))
            .collect();
        names.sort();
        names
    }
}

/// The child card of subagent task t that spawner's agent spawned at now, at work as spawnee.
pub fn spawn_card(spawner: &Card, spawnee: Spawnee, t: &wire::runtime::Task, now: Time) -> Card {
    let name = if t.title.is_empty() { &t.agent_type } else { &t.title };
    let mut built = Card::default();
    built.id = new_id("c_");
    built.home = spawner.home.clone();
    built.employee = spawnee.employee;
    built.agent = spawnee.agent;
    built.loadout = spawnee.loadout;
    built.name = name.clone();
    built.origin = Origin::Spawned;
    built.brief = t.prompt.clone();
    built.model = spawner.model.clone();
    built.mode = spawner.mode.clone();
    built.cwd = spawner.cwd.clone();
    built.activity = format!("spawned by {}", spawner.name);
    built.spawn = Some(Spawn {
        parent: spawner.id.clone(),
        task: t.id.clone(),
        tool_call: t.tool_call_id.clone(),
    });
    built.created_at = now;
    built.updated_at = now;
    built.set_status(CardStatus::Running, now);
    built
}

/// The end of a child card whose task t ended, said being what its subagent wrote: done with its report (the task's
/// summary, else what it wrote), failed, or stopped.
pub fn spawn_end(t: &wire::runtime::Task, said: &str, now: Time) -> impl Fn(&mut Card) + Send + Sync + 'static {
    let summary = t.summary.trim().to_owned();
    let (status, said) = (t.status, said.to_owned());
    move |c: &mut Card| match status {
        TaskStatus::Completed => {
            c.set_status(CardStatus::Idle, now);
            c.output = if summary.is_empty() {
                said.clone()
            } else {
                summary.clone()
            };
            "done".clone_into(&mut c.activity);
        }
        TaskStatus::Failed => {
            c.set_status(CardStatus::Failed, now);
            c.error = if summary.is_empty() {
                "the subagent failed".to_owned()
            } else {
                summary.clone()
            };
        }
        _ => {
            c.set_status(CardStatus::Stopped, now);
            let how = if summary.is_empty() { status.as_str() } else { &summary };
            c.activity = format!("stopped: {how}");
        }
    }
}

/// How a child card cut with its parent's process is named to the parent when it resumes.
pub fn cut_child_name(c: &Card) -> String {
    format!("the subagent {} (card {})", c.name, c.id)
}

/// Adds to lost, under the card whose process ran them, the child cards among cards (one home's) still at work: a
/// restart cut them with that process.
pub fn cut_children(cards: &[Card], lost: &mut HashMap<String, Vec<String>>) {
    let by_id: HashMap<&str, &Card> = cards.iter().map(|c| (c.id.as_str(), c)).collect();
    for c in cards.iter().filter(|c| spawned(c) && at_work(c)) {
        let mut root = Some(c);
        // each step goes up one spawner; more steps than cards is a loop in what was recorded
        for _ in 0..=cards.len() {
            match root.and_then(|r| r.spawn.as_ref()) {
                Some(s) => root = by_id.get(s.parent.as_str()).copied(),
                None => break,
            }
        }
        if let Some(root) = root.filter(|r| !spawned(r)) {
            lost.entry(root.id.clone()).or_default().push(cut_child_name(c));
        }
    }
}
