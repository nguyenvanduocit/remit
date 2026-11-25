//! An agent's background work: tasks its own process runs past the turn that started them (Claude Code's Monitor, a
//! shell run in the background), and the turns the agent takes on its own when one of them has news for it. Remit does
//! not run them; it shows them, and holds them to the chain's rails. A task is a task event of the card, running until
//! its process says it ended or the session closes under it. A turn of its own is a turn of the card like any other: the
//! card runs, the rails that stop a turn stop it, and its words go where any turn's go (a worker's report to the Chair);
//! only what opens it differs, the agent going to work with no message, and a turn in which it says nothing reports
//! nothing. A chain that waits closes all its sessions, and the tasks end with them: nothing an agent watches wakes a
//! waiting chain, which subscribes to a time instead (wait.rs).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use model::{
    Card, CardStatus, ChainStatus, Event, EventKind, EventStatus, PromptStatus, StopReason, SystemNote, TaskStatus,
    Time,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::chain_core::Core;
use crate::health::elapsed;
use crate::homes::standalone_card;
use crate::inbox::{Cause, Failed};
use crate::outbox::actor_words;
use crate::request::Service;
use crate::request::{Done, Request};
use crate::session::CANCEL_WAIT;
use crate::session::{Run, ended_by_silence, took_data};
use crate::turn_end::TurnEnd;
use crate::{Error, Result};
use model::Cut;

/// A turn the agent takes on its own, on news from its background work: its process starts it, not a message.
pub struct OwnTurn {
    pub started: Time,
    state: Mutex<OwnState>,
    ended: Notify,
}

#[derive(Default)]
struct OwnState {
    /// What the stream shows opened it.
    opening: Event,
    /// The agent went idle, or its session closed.
    ended: bool,
    /// What the agent said in it, once it ended.
    output: String,
    /// Why it was cut short (`cancel_turn`), once it ended; none when it was not.
    cut: Option<Cut>,
    /// The liveness rail cut it short: its end is a failure.
    silenced: bool,
}

impl OwnTurn {
    fn new(started: Time) -> Self {
        Self {
            started,
            state: Mutex::default(),
            ended: Notify::new(),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, OwnState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Ends the turn, once.
    pub fn end(&self) {
        self.state().ended = true;
        self.ended.notify_waiters();
        self.ended.notify_one();
    }

    /// Marks it cut short by the liveness rail.
    pub fn silence(&self) {
        self.state().silenced = true;
    }

    /// The id of the event that opened it.
    pub fn opening_id(&self) -> i64 {
        self.state().opening.id
    }

    async fn wait(&self) {
        loop {
            let notified = self.ended.notified();
            if self.state().ended {
                return;
            }
            notified.await;
        }
    }
}

/// What a task event records beside what it shows.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct TaskData {
    task_id: String,
    kind: String,
    /// The tool call that started it.
    #[serde(skip_serializing_if = "String::is_empty")]
    tool_call_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    output_file: String,
    /// The name of that tool call: Monitor, Bash, ...
    #[serde(skip_serializing_if = "String::is_empty")]
    tool: String,
    /// What it runs, as that call said: a command, or the address a Monitor listens to.
    #[serde(skip_serializing_if = "String::is_empty")]
    command: String,
    /// What it writes can be read while it runs (a background shell's or a Monitor's: `task_output`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    readable: bool,
    /// The owner can end it from its card (`stop_task`); a workflow run is stopped as a run (workflow_claude.rs).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stoppable: bool,
    /// The end of what it wrote, kept once it ended.
    #[serde(skip_serializing_if = "String::is_empty")]
    output: String,
}

/// The task type of a background shell and of a Monitor watching a command: the tasks Claude Code keeps output of.
const TASK_SHELL: &str = "local_bash";

/// What a Bash or Monitor call runs, from its tool event's data: its command, or the address a Monitor listens to.
fn command_of(data: Option<&model::ForeignJson>) -> String {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Call {
        input: Input,
    }
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Input {
        command: String,
        ws: Ws,
    }
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Ws {
        url: String,
    }
    let call: Call = data
        .and_then(|d| serde_json::from_value(d.0.clone()).ok())
        .unwrap_or_default();
    if call.input.command.is_empty() {
        call.input.ws.url
    } else {
        call.input.command
    }
}

/// What task event e records; an event that records nothing readable records nothing.
fn task_data_of(e: &Event) -> TaskData {
    e.data
        .as_ref()
        .and_then(|d| serde_json::from_value(d.0.clone()).ok())
        .unwrap_or_default()
}

/// A background task of a card, as the owner reaches it: by its event.
struct CardTask {
    run: Option<Arc<Run>>,
    event: Event,
    data: TaskData,
}

/// What a turn of the agent's own is about, wherever it shows.
pub const OWN_TURN_TITLE: &str = "News from its background work";

/// Whether prompt event p opened a turn the agent took on its own.
pub fn is_own_turn(p: &Event) -> bool {
    p.kind() == EventKind::Prompt && p.by.as_ref() == Some(&model::by_agent(&p.card_id)) && p.title == OWN_TURN_TITLE
}

impl Core {
    /// What the task event of t records, over what it recorded before (empty for a new one): the tool call that started
    /// it, once known, says what it runs, and the output kept at its end stays.
    async fn describe_task(&self, card_id: &str, t: &wire::runtime::Task, before: TaskData) -> TaskData {
        let mut d = TaskData {
            task_id: t.id.clone(),
            kind: t.kind.clone(),
            tool_call_id: t.tool_call_id.clone(),
            output_file: t.output_file.clone(),
            readable: t.kind == TASK_SHELL,
            stoppable: t.kind != wire::runtime::TASK_WORKFLOW,
            ..before
        };
        if d.tool.is_empty() && !t.tool_call_id.is_empty() {
            let (card, call) = (card_id.to_owned(), t.tool_call_id.clone());
            if let Ok(e) = self.store.read(move |s| s.tool_event(&card, &call)).await {
                d.command = command_of(e.data.as_ref());
                d.tool = e.tool_name;
            }
        }
        d
    }

    /// Records what the agent's process says of one of its background tasks: its event, added when it starts, then kept
    /// as it is known. A task whose output can be read has the end of it kept when it ends, so it stays to be read once
    /// the process is gone.
    pub async fn on_task(self: &Arc<Self>, r: &Arc<Run>, t: wire::runtime::Task) {
        let raw = |d: &TaskData| {
            serde_json::to_string(d)
                .ok()
                .and_then(|d| model::ForeignJson::parse(&d).ok())
        };
        let (known, home, card_id) = {
            let mut s = r.state();
            if t.status != TaskStatus::Running && !t.summary.is_empty() {
                s.news.push(t.summary.clone());
            }
            (s.tasks.get(&t.id).copied(), s.card.home.clone(), s.card.id.clone())
        };
        let Some(id) = known else {
            let title = t.title.clone();
            let data = raw(&self.describe_task(&card_id, &t, TaskData::default()).await);
            let added = self
                .add_event(Event {
                    home,
                    card_id,
                    status: EventStatus::Task(t.status),
                    title: t.title,
                    text: t.summary,
                    data,
                    ..Event::default()
                })
                .await;
            let e = match added {
                Ok(e) => e,
                Err(err) => return tracing::error!(task = %t.id, %err, "task event"),
            };
            r.state().tasks.insert(t.id, e.id);
            self.show(r, move |c| c.activity = format!("watching: {title}"));
            return;
        };
        let mut e = match self.store.read(move |s| s.event(id)).await {
            Ok(e) => e,
            Err(err) => return tracing::error!(home = %home, card = %card_id, task = %t.id, %err, "task event"),
        };
        let d = self.describe_task(&card_id, &t, task_data_of(&e)).await;
        let keep = t.status != TaskStatus::Running && d.readable && d.output.is_empty();
        (e.status, e.title, e.text, e.data) = (EventStatus::Task(t.status), t.title, t.summary, raw(&d));
        self.save_event(e).await;
        if keep {
            let (o, r) = (Arc::clone(self), Arc::clone(r));
            tokio::spawn(async move { o.keep_task_output(&r, id, t.id).await });
        }
    }

    /// Keeps on task event id the end of what task `task_id` of r's process wrote, read from the process, which keeps it
    /// until it exits.
    async fn keep_task_output(&self, r: &Run, id: i64, task_id: String) {
        let out = match read_task(r, task_id.clone()).await {
            Ok(out) => out,
            Err(err) => return tracing::warn!(card = %r.card_id(), task = %task_id, %err, "keep a task's output"),
        };
        let mut e = match self.store.read(move |s| s.event(id)).await {
            Ok(e) => e,
            Err(err) => return tracing::error!(card = %r.card_id(), task = %task_id, %err, "task event"),
        };
        let mut d = task_data_of(&e);
        d.output = out.text;
        e.data = serde_json::to_string(&d)
            .ok()
            .and_then(|d| model::ForeignJson::parse(&d).ok());
        self.save_event(e).await;
    }

    /// The background task whose event is `event_id`, in its card's process.
    async fn card_task(&self, event_id: i64) -> Result<CardTask> {
        let event = self.store.read(move |s| s.event(event_id)).await?;
        if event.kind() != EventKind::Task {
            return Err(Error::refused(format!("event {event_id} is not a background task")));
        }
        let data = task_data_of(&event);
        Ok(CardTask {
            run: self.run_of(&event.card_id),
            event,
            data,
        })
    }

    /// The end of what a background shell or Monitor has written: read from the agent's process while it runs, and as
    /// kept at its end once it ended.
    pub async fn task_output(&self, event_id: i64) -> Result<wire::runtime::TaskOutput> {
        let t = self.card_task(event_id).await?;
        if !t.data.readable {
            return Err(Error::refused(format!(
                "{} writes no output Remit can read",
                t.event.title
            )));
        }
        match t.run {
            Some(r) if t.event.status == EventStatus::Task(TaskStatus::Running) => read_task(&r, t.data.task_id)
                .await
                .map_err(|e| Error::refused(format!("read {}: {e}", t.event.title))),
            _ => Ok(wire::runtime::TaskOutput {
                total_bytes: u64::try_from(t.data.output.len()).unwrap_or(u64::MAX),
                text: t.data.output,
                truncated: false,
            }),
        }
    }

    /// Ends one background task of a card's agent, and the agent goes on: Claude Code tells it the task was stopped, its
    /// end comes as the task's own (`on_task`), and the card's stream says who stopped it.
    pub async fn stop_task(self: &Arc<Self>, req: Request, event_id: i64) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "StopTask", move |o| async move {
            let t = o.card_task(event_id).await?;
            if !t.data.stoppable {
                return Err(Error::refused(format!(
                    "{} is stopped as a workflow run, from the run",
                    t.event.title
                )));
            }
            let Some(r) = t
                .run
                .filter(|_| t.event.status == EventStatus::Task(TaskStatus::Running))
            else {
                return Ok(()); // ended already: what was asked holds
            };
            let sess = live_session(&r).map_err(Error::refused)?;
            match tokio::time::timeout(CANCEL_WAIT, sess.stop_task(t.data.task_id)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(Error::refused(format!("stop {}: {e}", t.event.title))),
                Err(_) => {
                    return Err(Error::refused(format!(
                        "stop {}: the agent's process did not answer",
                        t.event.title
                    )));
                }
            }
            o.add_event(Event {
                home: t.event.home,
                card_id: t.event.card_id,
                status: EventStatus::System(SystemNote::Plain),
                by: Some(by.clone()),
                text: format!("Background task stopped by {}: {}.", actor_words(&by), t.event.title),
                ..Event::default()
            })
            .await?;
            Ok(())
        })
        .await
    }

    /// Follows the agent from idle to working and back. Working with no message open is a turn of its own: its opening is
    /// stored now, ahead of anything the agent streams in it, its end and what it said are taken when the agent goes
    /// idle, and the card's loop records it as the card's turn (`run_own_turn`). Working on a message (its prompt open)
    /// is that message's turn, whose end is its prompt's answer.
    pub async fn on_working(self: &Arc<Self>, r: &Arc<Run>, working: bool) {
        if !working {
            let t = {
                let mut s = r.state();
                let t = s.own.take();
                if let Some(t) = &t {
                    let mut ts = t.state();
                    ts.output = s.turn.trim().to_owned();
                    ts.cut.clone_from(&s.cut);
                }
                t
            };
            if let Some(t) = t {
                t.end();
            }
            return;
        }
        let now = self.now();
        let (t, news, home, card_id) = {
            let mut s = r.state();
            if s.prompt != 0 || s.own.is_some() || s.sess.is_none() || r.mailbox.is_closed() {
                return;
            }
            let t = Arc::new(OwnTurn::new(now));
            s.own = Some(Arc::clone(&t));
            s.cut = None;
            let me = model::by_agent(&s.card.id);
            s.begin_turn(now, me);
            s.turn.clear();
            let news = s.news.join("\n");
            s.news.clear();
            (t, news, s.card.home.clone(), s.card.id.clone())
        };
        let news = if news.is_empty() {
            format!("What it watches: {}", self.watching(r).await.join("; "))
        } else {
            news
        };
        self.close_stream(r).await;
        let opening = self
            .add_event(Event {
                home,
                by: Some(model::by_agent(&card_id)),
                card_id,
                status: EventStatus::Prompt(PromptStatus::Delivered),
                title: OWN_TURN_TITLE.to_owned(),
                text: news,
                ..Event::default()
            })
            .await;
        // a turn runs from its stored opening: with none stored, it is not taken
        let opening = match opening {
            Ok(e) => e,
            Err(err) => return tracing::error!(card = %r.card_id(), %err, "own turn's opening"),
        };
        t.state().opening = opening;
        r.state().own_turns.push_back(t);
        r.mailbox.ring();
    }

    /// Runs a turn the agent took on its own as the card's turn: the card runs, and the turn ends as any turn does once
    /// the agent is idle. A paused or waiting chain runs none.
    pub async fn run_own_turn(self: &Arc<Self>, r: &Arc<Run>, t: &OwnTurn) {
        let c = match self.start_own_turn(r).await {
            Ok(Some(c)) => c,
            Ok(None) => return,
            Err(err) => {
                tracing::error!(home = %r.home, card = %r.card_id(), %err, "start a turn of its own");
                return;
            }
        };
        t.wait().await;
        if r.mailbox.is_closed() {
            return; // closed under it: its end is not recorded
        }
        let (opening, output, cut, silenced) = {
            let s = t.state();
            (s.opening.clone(), s.output.clone(), s.cut.clone(), s.silenced)
        };
        // the agent went idle once cancelled
        let stop = if cut.is_none() {
            StopReason::EndTurn
        } else {
            StopReason::Cancelled
        };
        if ended_by_silence(silenced, stop) {
            let why = self.silence();
            self.end_open_calls(r, &opening, Some(&why)).await;
            self.end_failed_turn(r, &opening, &Failed::new(Cause::Silent, why), None)
                .await;
            return;
        }
        self.close_stream(r).await;
        if cut.is_some() {
            // the tool calls it left open end with it; a turn that ended on its own left none
            self.end_open_calls(r, &opening, None).await;
        }
        self.note(Event {
            home: c.home.clone(),
            card_id: c.id.clone(),
            status: EventStatus::Stop(stop),
            data: took_data(elapsed(t.started, self.now())),
            ..Event::default()
        })
        .await;
        self.end_turn(
            r,
            &opening,
            TurnEnd {
                output,
                stop: stop.as_str().to_owned(),
                ..TurnEnd::default()
            },
        )
        .await;
    }

    /// Makes run r's card run the turn its agent took on its own, unless its chain is paused or waits (none): then its
    /// session closes, as the pause closed those of agents at work in the background and the wait closes all of them, and
    /// the card runs no turn. It looks and starts under the chain's lock: a pause stored after it finds the card running
    /// and cuts the turn, and a resume stored before it leaves the session open.
    async fn start_own_turn(self: &Arc<Self>, r: &Arc<Run>) -> Result<Option<Card>> {
        let c = r.card_now();
        let cs;
        let _held = if standalone_card(&c) {
            None
        } else {
            cs = self.state(&c.home).await?;
            let held = cs.held.lock().await;
            match held.chain.status() {
                ChainStatus::Paused => {
                    self.close_sessions(vec![Arc::clone(r)], "paused", &Cut::Paused).await;
                    return Ok(None);
                }
                ChainStatus::Waiting => {
                    self.close_sessions(vec![Arc::clone(r)], "", &Cut::Slept).await;
                    return Ok(None);
                }
                _ => Some(held),
            }
        };
        let now = self.now();
        let c = self
            .update_card(&c.id, move |c| {
                c.set_status(CardStatus::Running, now);
                c.turns = c.turns.saturating_add(1);
                c.error.clear();
                "news from its background work".clone_into(&mut c.activity);
            })
            .await?;
        Ok(Some(c))
    }

    /// What r's agent watches: the titles of its background tasks still running.
    pub async fn watching(&self, r: &Run) -> Vec<String> {
        let mut ids: Vec<i64> = r.state().tasks.values().copied().collect();
        ids.sort_unstable();
        let tasks = self
            .store
            .read(move |s| Ok::<_, store::Error>(ids.iter().filter_map(|id| s.event(*id).ok()).collect::<Vec<_>>()))
            .await
            .unwrap_or_default();
        tasks
            .into_iter()
            .filter(|e| e.status == EventStatus::Task(TaskStatus::Running))
            .map(|e| e.title)
            .collect()
    }

    /// Ends the background tasks of run r still running, because its session closed for why: its process, which ran
    /// them, is gone.
    pub async fn end_tasks(&self, r: &Run, why: &Cut) {
        let ids: Vec<i64> = r.state().tasks.values().copied().collect();
        for id in ids {
            let Ok(mut e) = self.store.read(move |s| s.event(id)).await else {
                continue;
            };
            if e.status != EventStatus::Task(TaskStatus::Running) {
                continue;
            }
            why.stop_task(&mut e);
            self.save_event(e).await;
        }
    }

    /// The cards of a chain whose agents have background tasks running, with the tasks' titles.
    pub async fn watching_cards(&self, chain_id: &str) -> Result<HashMap<String, Vec<String>>> {
        let id = chain_id.to_owned();
        let tasks = self.store.read(move |s| s.running_tasks(&id)).await?;
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for e in tasks {
            out.entry(e.card_id).or_default().push(e.title);
        }
        Ok(out)
    }

    /// Names a chain's background tasks still running, each with its agent ("watch the deploy (R)"); none when it cannot
    /// read them.
    pub async fn watch_list(&self, chain_id: &str) -> Vec<String> {
        let id = chain_id.to_owned();
        let read = self
            .store
            .read(move |s| Ok::<_, store::Error>((s.running_tasks(&id)?, s.cards(&id)?)))
            .await;
        let Ok((tasks, cards)) = read else { return Vec::new() };
        let names: HashMap<String, String> = cards.into_iter().map(|c| (c.id, c.name)).collect();
        tasks
            .into_iter()
            .map(|e| format!("{} ({})", e.title, names.get(&e.card_id).map_or("", String::as_str)))
            .collect()
    }
}

/// The session of r's process, while it is open.
fn live_session(r: &Run) -> std::result::Result<Arc<dyn agent::Session>, String> {
    r.state()
        .sess
        .clone()
        .filter(|_| r.open())
        .ok_or_else(|| "the agent's process has ended, and its background work with it".to_owned())
}

/// Reads the end of what task `task_id` of r's process has written, giving the process `CANCEL_WAIT` to answer.
async fn read_task(r: &Run, task_id: String) -> std::result::Result<wire::runtime::TaskOutput, String> {
    let sess = live_session(r)?;
    match tokio::time::timeout(CANCEL_WAIT, sess.task_output(task_id)).await {
        Ok(out) => out.map_err(|e| e.to_string()),
        Err(_) => Err("the agent's process did not answer".to_owned()),
    }
}
