//! A routine is the owner's routine: words he gave once and the time, or the rhythm, at which Remit starts a chain from
//! them. It is a subscription of Remit whose action is to start a chain (ADR-0014) and that keeps a schedule
//! (docs/adr/0064): the time is its Schedule and `due_at`, the words are its StartSpec, and the Tick fires it when its
//! instant comes. He never fills in a form for one: he tells the Assistant, whose tools are the actions here. A routine
//! runs one chain at a time (overlap buffer_one): while the last chain it started is at work, one next run waits, the
//! Subscriptions tab says so, and each time after it is skipped and counted. A routine's chain is one the owner asked for
//! in advance, so its Chair chooses nothing: its title, goal and kind are the routine's.

use std::sync::Arc;

use model::{
    ActionKind, Actor, Chain, ChainKind, ChainOrigin, ChainStatus, Control, Overlap, OwnerKind, Schedule, Start,
    StartFrom, StartSpec, Subscription, SubscriptionAction, SubscriptionOwner, Time,
};
use serde::Deserialize;
use store::Tx;

use crate::Pipeline;
use crate::signals::queue_start;
use chains::chain_core::{first_line, new_id};
use chains::request::Service;
use chains::request::{Done, Request};
use chains::subscribe::subscribe;
use chains::timewait::{counted_from, once_at, wait_for};
use chains::wait::rfc3339;
use chains::{Error, Result};

/// The fastest a routine may repeat. A chain takes agents' work and the owner's review: this refuses a typo such as
/// `* * * * *` before it fills the board.
const MIN_ROUTINE_EVERY: time::Duration = time::Duration::minutes(5);

/// A routine as the Assistant's tools take it. It is what update_routine replaces a routine with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct RoutineInput {
    /// Short name in the owner's language; it heads every chain the routine starts.
    pub title: String,
    /// What each chain is to do, in the owner's words, self-contained.
    pub goal: String,
    /// The kind of chain it starts (ADR-0113): work unless the owner says chore or objective.
    pub kind: ChainKind,
    /// To repeat: five fields in the server's time zone, or `@every 6h`; never more often than every 5 minutes.
    pub cron: String,
    /// To run once: the time, RFC 3339 with its UTC offset; it must be in the future.
    pub at: String,
    /// A workflow script each chain begins with (ADR-0073): its Chair takes its first turn when the run ends, with its
    /// result. Empty: the Chair starts from the goal.
    pub script: String,
    /// What the script reads as args: a JSON object; only with a script.
    pub args: Option<serde_json::Map<String, serde_json::Value>>,
}

impl RoutineInput {
    /// The routine's subscription with its words cleaned up and no time yet: Remit's ownership, the start it makes, and
    /// one run at a time. It has no id, author or time of its own yet. Its kind is checked by the pipeline
    /// (`check_start_kind`). The script is evaluated once: one that fails before its first call would fail every run. The engine runs
    /// JavaScript for up to two seconds: call this off the async workers.
    fn words(&self) -> Result<Subscription> {
        let mut start = StartSpec {
            title: first_line(self.title.trim(), 80),
            goal: self.goal.trim().to_owned(),
            kind: self.kind,
            origin: ChainOrigin::Routine,
            script: self.script.trim().to_owned(),
            args: None,
        };
        if start.title.is_empty() {
            return Err(Error::refused("a routine needs a title"));
        }
        if start.goal.is_empty() {
            return Err(Error::refused(
                "a routine needs a goal: what each chain is to do, in the owner's words",
            ));
        }
        if let Some(args) = &self.args {
            if start.script.is_empty() {
                return Err(Error::refused(
                    "args are what a script reads: give a script with them, or no args",
                ));
            }
            start.args = Some(model::ForeignJson(serde_json::to_value(args)?));
        }
        if !start.script.is_empty() {
            let first = workflow::advance(&start.script, start.args.as_ref(), &[]);
            if first.state == workflow::ScriptState::Failed {
                return Err(Error::refused(format!(
                    "the script fails before its first call, so every run would: {}",
                    first.why()
                )));
            }
        }
        Ok(Subscription {
            owner: SubscriptionOwner {
                kind: OwnerKind::Remit,
                id: String::new(),
            },
            overlap: Overlap::BufferOne,
            action: SubscriptionAction {
                kind: ActionKind::Start,
                start: Some(start),
                ..SubscriptionAction::default()
            },
            ..Subscription::default()
        })
    }

    /// The routine in checked: its words, and when it next starts a chain, counted from now.
    fn routine_at(&self, now: Time) -> Result<Subscription> {
        let r = self.words()?;
        let (cron, at) = (self.cron.trim(), self.at.trim());
        if cron.is_empty() == at.is_empty() {
            return Err(Error::refused(
                "give cron or at, one of the two: cron to repeat, at to run once",
            ));
        }
        if !at.is_empty() {
            let t = time::OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).map_err(|_| {
                Error::refused(format!(
                    "at {at:?} is not an RFC 3339 time with its UTC offset, e.g. 2026-10-27T15:00:00+07:00"
                ))
            })?;
            if t <= now.0 {
                return Err(Error::refused(format!(
                    "at {at} is not in the future (it is now {})",
                    rfc3339(now)
                )));
            }
            return Ok(once_at(r, Time(t)));
        }
        let r = counted_from(
            Subscription {
                schedule: Some(Schedule {
                    cron: cron.to_owned(),
                    ..Schedule::default()
                }),
                ..r
            },
            now,
        )?;
        // counted_from gives the first instant: the second is counted from it
        let first = r.due_at.unwrap_or(now);
        match signal::cron_next(cron, first) {
            Ok(second)
                if first
                    .0
                    .checked_add(MIN_ROUTINE_EVERY)
                    .is_some_and(|soonest| second.0 >= soonest) =>
            {
                Ok(r)
            }
            _ => Err(Error::refused(format!(
                "cron {cron:?} repeats more often than every 5 minutes, the fastest a routine may"
            ))),
        }
    }
}

/// Says s is a routine: a subscription of Remit that keeps a schedule and starts a chain.
pub fn is_routine(s: &Subscription) -> bool {
    s.owner.kind == OwnerKind::Remit
        && s.schedule.is_some()
        && s.action.kind == ActionKind::Start
        && s.action.start.is_some()
}

/// The chain a start subscription queues.
pub fn start_spec_of(s: &Subscription) -> StartSpec {
    s.action.start.clone().unwrap_or_default()
}

/// `RoutineInput::routine_at` off the async workers, where its script's evaluation runs.
async fn routine_off_workers(input: &RoutineInput, now: Time) -> Result<Subscription> {
    let input = input.clone();
    tokio::task::spawn_blocking(move || input.routine_at(now))
        .await
        .map_err(|e| Error::refused(format!("the routine's script's evaluation did not finish: {e}")))?
}

/// r with the words and time of next, a routine checked from RoutineInput: its owner's replacement. What the owner does
/// not give stays: its origin, pause and standing, its catch-up window and when it last ran. One that was over has a run
/// left again.
fn with_words(mut r: Subscription, next: Subscription) -> Subscription {
    let mut start = start_spec_of(&next);
    start.origin = start_spec_of(&r).origin;
    let mut sch = next.schedule.unwrap_or_default();
    if let Some(was) = &r.schedule {
        sch.catchup_window = was.catchup_window;
        sch.last = was.last;
    }
    r.schedule = Some(sch);
    r.due_at = next.due_at;
    r.action.start = Some(start);
    if r.control == Control::Ended {
        r.control = Control::Active;
    }
    r
}

/// r once its chain was started at now by the owner's word: a routine that repeats counts its next time from now so the
/// times it missed are not run, and one that runs once is done. A run its schedule starts is moved on by the Tick
/// (`signal::apply`).
fn after_run(mut r: Subscription, now: Time) -> Subscription {
    let mut sch = r.schedule.clone().unwrap_or_default();
    sch.last = Some(now);
    let next = signal::schedule_next(&sch, now);
    r.last_fired = Some(now);
    r.schedule = Some(sch);
    r.due_at = next;
    if next.is_none() {
        r.control = Control::Ended;
    }
    r
}

/// r no longer paused. One that repeats counts its next time from now, so the days it was paused are not run at once; one
/// that runs once keeps its time, and if that has passed it runs at once; one whose one run ran (no time left) is over;
/// one that keeps no schedule hears what comes.
pub fn resumed(mut r: Subscription, now: Time) -> Result<Subscription> {
    r.control = Control::Active;
    match &r.schedule {
        None => Ok(r),
        Some(s) if s.once() => {
            if r.due_at.is_none() {
                r.control = Control::Ended;
            }
            Ok(r)
        }
        Some(_) => counted_from(r, now),
    }
}

/// The chain a routine started that is not done, if there is one.
pub(crate) fn unfinished_run<'a>(chains: &'a [Chain], routine_id: &str) -> Option<&'a Chain> {
    chains
        .iter()
        .find(|c| c.routine_id == routine_id && *c.status() != ChainStatus::Done)
}

/// How a routine repeats: "once" or "cron 0 9 * * *".
pub(crate) fn cadence_label(cron: &str) -> String {
    if cron.is_empty() {
        "once".to_owned()
    } else {
        format!("cron {cron}")
    }
}

fn no_routine(id: &str) -> Error {
    Error::refused(format!("no routine {id:?}: list_routines shows the ids"))
}

/// The part of Remit that starts a due routine's chain.
pub fn routine_by() -> Actor {
    model::by_remit("routine")
}

/// Routine id as tx holds it: refused when there is none.
fn routine_in(tx: &Tx<'_>, id: &str) -> Result<Subscription> {
    match tx.subscription(id) {
        Ok(sub) if is_routine(&sub) => Ok(sub),
        Ok(_) | Err(store::Error::NotFound) => Err(no_routine(id)),
        Err(err) => Err(err.into()),
    }
}

impl Pipeline {
    /// Refuses a start of s whose kind its starter may not name (`Domains::kind_refusal`): checked before a routine or a
    /// subscription that starts chains is stored, so what fires later starts what may be started.
    pub(crate) fn check_start_kind(&self, s: &Subscription) -> Result<()> {
        let Some(start) = &s.action.start else {
            return Ok(());
        };
        match self.domains()?.kind_refusal(&start.origin, start.kind) {
            Some(why) => Err(Error::refused(format!("kind {}: {why}", start.kind))),
            None => Ok(()),
        }
    }

    /// The owner's routines, by id.
    pub(crate) async fn routines(&self) -> Result<Vec<Subscription>> {
        let owner = SubscriptionOwner {
            kind: OwnerKind::Remit,
            id: String::new(),
        };
        let mut subs = self.store.read(move |r| r.subscriptions_of(&owner)).await?;
        subs.retain(is_routine);
        Ok(subs)
    }

    /// Makes a routine from the owner's words, by who asked.
    pub(crate) async fn create_routine(&self, input: &RoutineInput, by: Actor) -> Result<Subscription> {
        let now = self.now();
        let mut r = routine_off_workers(input, now).await?;
        self.check_start_kind(&r)?;
        r.id = new_id("rt_");
        r.by = by;
        r.created_at = now;
        r.updated_at = now;
        let r = self
            .store
            .update(move |tx| {
                subscribe(
                    tx,
                    r,
                    &Start {
                        from: StartFrom::Now,
                        ..Start::default()
                    },
                    now,
                )
            })
            .await?;
        self.kick();
        Ok(r)
    }

    /// Replaces routine id with input, keeping whether it is paused; its next time is counted from now.
    pub(crate) async fn update_routine(&self, id: &str, input: &RoutineInput, by: Actor) -> Result<Subscription> {
        let next = routine_off_workers(input, self.now()).await?;
        self.check_start_kind(&next)?;
        self.change_routine(id, by, move |_, cur| Ok(with_words(cur, next)))
            .await
    }

    /// Stops a routine from starting chains until it resumes; a run already at work goes on.
    pub(crate) async fn pause_routine(&self, id: &str, by: Actor) -> Result<Subscription> {
        self.change_routine(id, by, |_, mut r| {
            r.control = Control::Paused;
            Ok(r)
        })
        .await
    }

    /// Lets a paused routine start chains again.
    pub(crate) async fn resume_routine(&self, id: &str, by: Actor) -> Result<Subscription> {
        let now = self.now();
        self.change_routine(id, by, move |_, r| resumed(r, now)).await
    }

    /// Stores what change makes of routine id, read and written in one transaction so no change is lost, as by, and
    /// shows it on the Subscriptions tab.
    async fn change_routine<F>(&self, id: &str, by: Actor, change: F) -> Result<Subscription>
    where
        F: FnOnce(&Tx<'_>, Subscription) -> Result<Subscription> + Send + 'static,
    {
        let (key, now) = (id.to_owned(), self.now());
        let saved = self
            .store
            .update(move |tx| {
                let cur = routine_in(tx, &key)?;
                let mut next = change(tx, cur)?;
                next.by = by;
                next.updated_at = now;
                Ok::<_, Error>(wait_for(tx, next)?)
            })
            .await?;
        self.kick();
        Ok(saved)
    }

    /// Removes a routine. The chains it started stay.
    pub(crate) async fn delete_routine(&self, id: &str) -> Result<()> {
        let key = id.to_owned();
        self.store
            .update(move |tx| {
                routine_in(tx, &key)?;
                Ok::<_, Error>(tx.delete_subscription(&key)?)
            })
            .await?;
        self.kick();
        Ok(())
    }

    /// The owner starting a routine's chain now.
    pub async fn run_routine_now(self: &Arc<Self>, req: Request, id: String) -> Result<Chain> {
        let by = req.by.clone();
        self.act(req, "RunRoutine", move |o| async move { o.run_routine(&id, by).await })
            .await
    }

    /// The owner stopping a routine from starting chains until he resumes it.
    pub async fn pause_routine_now(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "PauseRoutine", move |o| async move {
            o.pause_routine(&id, by).await.map(|_| ())
        })
        .await
    }

    /// The owner letting a paused routine start chains again.
    pub async fn resume_routine_now(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "ResumeRoutine", move |o| async move {
            o.resume_routine(&id, by).await.map(|_| ())
        })
        .await
    }

    /// Starts a chain from routine id now, instead of at its time: the owner's (or the Assistant's, for him). The chain
    /// and the routine's next time are stored in one transaction: a crash leaves both or neither, so a run is never lost
    /// nor made twice. Refused while its last chain is at work.
    pub(crate) async fn run_routine(&self, id: &str, by: Actor) -> Result<Chain> {
        let key = id.to_owned();
        let r = self
            .store
            .read(move |r| r.subscription(&key))
            .await
            .map_err(|_| no_routine(id))?;
        if !is_routine(&r) {
            return Err(no_routine(id));
        }
        let now = self.now();
        let ch = self.new_start_chain(&r, "", by, now, "").await?;
        let run = self.start_run(&r, &ch, now).await?;
        let (door, key, quota) = (self.door(), id.to_owned(), self.cfg.event_chains_per_day);
        let q = self
            .store
            .update(move |tx| {
                let r = routine_in(tx, &key)?;
                let q = queue_start(tx, &door, &r, ch, run, quota)?;
                wait_for(tx, after_run(r, now))?;
                Ok::<_, Error>(q)
            })
            .await?;
        self.start_queued(&q);
        Ok(q.chain)
    }
}
