//! The Assistant's routine tools: how the owner sets up work that repeats, or that waits for a time, by telling it. Each
//! is the orchestrator's own action (routine.rs), taken as the Assistant for him, so the owner sees the routine on the
//! Subscriptions tab at once and who made it. A routine starts chains; it lifts no rail, so the Assistant may make, change,
//! pause and delete them. A routine is a subscription of Remit that starts a chain at a time (routine.rs); these tools
//! read and write it as a routine.

use std::sync::Arc;

use model::{Actor, Chain, ChainKind, Control, ForeignJson, Subscription, Time};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Pipeline;
use crate::routine::{RoutineInput, start_spec_of, unfinished_run};
use chains::tools::{Effect, NoInput, Tools, as_json};
use chains::wait::rfc3339;

/// A routine as create_routine takes it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateRoutineInput {
    /// short name in the owner's language, e.g. 'Weekly dependency audit'; it heads every chain the routine starts
    title: String,
    /// what each chain is to do, in the owner's words, self-contained: the chain's Chair reads only this, so name the repository, the question and what it should deliver
    goal: String,
    /// the kind of chain it starts: work (the default), chore (a small, well-understood change whose changes pass QC
    /// alone) or objective (a review or a check-in of the owner's objectives; the owner waits on it). A playbook that fits
    /// is named in goal
    #[serde(default)]
    kind: ChainKind,
    /// to repeat: five fields (minute hour day-of-month month day-of-week) in the server's time zone, e.g. '0 9 * * 1-5' for weekdays at 09:00, or '@every 6h'; never more often than every 5 minutes. Give cron or at, not both
    #[serde(default)]
    cron: String,
    /// to run once: the time, RFC 3339 with its UTC offset, e.g. '2026-10-27T15:00:00+07:00'; it must be in the future. Give cron or at, not both
    #[serde(default)]
    at: String,
    /// a workflow script each chain begins with, when the shape of the work is known before it starts (the workflows skill: agent, parallel, wait, askOwner...); the chain's Chair takes its first turn when the run ends, with its result. Empty: the Chair starts from the goal
    #[serde(default)]
    script: String,
    /// what the script reads as args: a JSON object; only with a script
    #[serde(default)]
    args: Option<serde_json::Map<String, serde_json::Value>>,
}

impl From<CreateRoutineInput> for RoutineInput {
    fn from(i: CreateRoutineInput) -> Self {
        RoutineInput {
            title: i.title,
            goal: i.goal,
            kind: i.kind,
            cron: i.cron,
            at: i.at,
            script: i.script,
            args: i.args,
        }
    }
}

/// A routine as update_routine replaces one with: create_routine's fields and the routine's id.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateRoutineInput {
    /// routine id (rt_…), as list_routines shows it
    id: String,
    /// short name in the owner's language, e.g. 'Weekly dependency audit'; it heads every chain the routine starts
    title: String,
    /// what each chain is to do, in the owner's words, self-contained: the chain's Chair reads only this, so name the repository, the question and what it should deliver
    goal: String,
    /// the kind of chain it starts: work (the default), chore (a small, well-understood change whose changes pass QC
    /// alone) or objective (a review or a check-in of the owner's objectives; the owner waits on it). A playbook that fits
    /// is named in goal
    #[serde(default)]
    kind: ChainKind,
    /// to repeat: five fields (minute hour day-of-month month day-of-week) in the server's time zone, e.g. '0 9 * * 1-5' for weekdays at 09:00, or '@every 6h'; never more often than every 5 minutes. Give cron or at, not both
    #[serde(default)]
    cron: String,
    /// to run once: the time, RFC 3339 with its UTC offset, e.g. '2026-10-27T15:00:00+07:00'; it must be in the future. Give cron or at, not both
    #[serde(default)]
    at: String,
    /// a workflow script each chain begins with, when the shape of the work is known before it starts (the workflows skill: agent, parallel, wait, askOwner...); the chain's Chair takes its first turn when the run ends, with its result. Empty: the Chair starts from the goal
    #[serde(default)]
    script: String,
    /// what the script reads as args: a JSON object; only with a script
    #[serde(default)]
    args: Option<serde_json::Map<String, serde_json::Value>>,
}

impl UpdateRoutineInput {
    /// The routine's id and what replaces it.
    fn split(self) -> (String, RoutineInput) {
        let routine = RoutineInput {
            title: self.title,
            goal: self.goal,
            kind: self.kind,
            cron: self.cron,
            at: self.at,
            script: self.script,
            args: self.args,
        };
        (self.id, routine)
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RoutineIdInput {
    /// routine id (rt_…), as list_routines shows it
    id: String,
}

/// A routine as the Assistant reads it: in the shape update_routine takes, plus where it stands.
#[derive(Debug, Serialize)]
struct RoutineRecord {
    id: String,
    title: String,
    goal: String,
    kind: ChainKind,
    /// The workflow script each chain begins with.
    #[serde(skip_serializing_if = "String::is_empty")]
    script: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<ForeignJson>,
    #[serde(skip_serializing_if = "String::is_empty")]
    cron: String,
    /// RFC 3339 in the server's zone; none once a routine that runs once has run.
    #[serde(skip_serializing_if = "String::is_empty")]
    next_at: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    paused: bool,
    /// The chain its last run started, while that is at work.
    #[serde(skip_serializing_if = "String::is_empty")]
    running_chain: String,
    /// Who made or last changed it: owner, assistant, agent or remit.
    by: String,
}

/// What list_routines answers: the server's time with its zone, and the routines.
#[derive(Serialize)]
struct RoutineList {
    now: String,
    routines: Vec<RoutineRecord>,
}

/// t in the server's zone, which is the zone of the clock's now.
fn local(t: Time, now: Time) -> Time {
    Time(t.0.to_offset(now.0.offset()))
}

/// The one place a routine's subscription is read as the routine the Assistant knows.
fn routine_record_of(r: &Subscription, chains: &[Chain], now: Time) -> RoutineRecord {
    let start = start_spec_of(r);
    RoutineRecord {
        id: r.id.clone(),
        title: start.title,
        goal: start.goal,
        kind: start.kind,
        script: start.script,
        args: start.args,
        cron: r.schedule.as_ref().map(|s| s.cron.clone()).unwrap_or_default(),
        next_at: r.due_at.map(|at| rfc3339(local(at, now))).unwrap_or_default(),
        paused: r.control == Control::Paused,
        running_chain: unfinished_run(chains, &r.id).map(|c| c.id.clone()).unwrap_or_default(),
        by: r.by.kind.to_string(),
    }
}

/// A routine's next time in words, for the owner and the Assistant to read back.
fn next_run_text(r: &Subscription, now: Time) -> String {
    let Some(due) = r.due_at else {
        return "no run is left".to_owned();
    };
    let at = local(due, now).0;
    let day = at
        .format(time::macros::format_description!(
            "[weekday repr:short] [year]-[month]-[day] [hour]:[minute] [offset_hour sign:mandatory]:[offset_minute]"
        ))
        .unwrap_or_default();
    format!("next run {day}")
}

/// The server's time now, with the day and the zone the hours of a routine are in.
fn now_text(now: Time) -> String {
    let day = now
        .0
        .format(time::macros::format_description!("[weekday]"))
        .unwrap_or_default();
    format!("{day} {}", rfc3339(now))
}

impl Pipeline {
    /// Gives the Assistant, acting as by, the tools that make, read, change, run and delete the owner's routines.
    pub fn add_routine_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        t.serving(self).add(
            "list_routines", Effect::Reads,
            "The owner's routines, each in the shape update_routine takes, with when it next starts a chain, whether it is paused and the chain of its run at work. Also the server's time now and its zone: the hours of a routine are in that zone, so read it before you make one.",
            |o, _: NoInput| async move {
                let rs = o.routines().await?;
                let chains = o.store.read(|r| r.chains()).await?;
                let now = o.now();
                let routines = rs.iter().map(|r| routine_record_of(r, &chains, now)).collect();
                as_json(&RoutineList { now: now_text(now), routines })
            },
        );
        let create_by = by.clone();
        t.serving(self).add(
            "create_routine", Effect::Writes,
            "Make a routine for the owner: Remit starts a chain from its goal at the times he gave, on a rhythm (cron) or once (at), without anyone asking, one run at a time. With a script, each chain begins by running it (the workflows skill), and its Chair takes over with the run's result. It shows on the Subscriptions tab at once. Make one only when the owner asked for it.",
            move |o, i: CreateRoutineInput| {
                let by = create_by.clone();
                async move {
                    let r = o.create_routine(&i.into(), by).await?;
                    Ok(format!(
                        "Made routine {}, {:?}: {}.",
                        r.id,
                        start_spec_of(&r).title,
                        next_run_text(&r, o.now())
                    ))
                }
            },
        );
        let update_by = by.clone();
        t.serving(self).add(
            "update_routine", Effect::Writes,
            "Replace a routine with this version: when it runs, what it does or its kind. Every field is replaced, so start from list_routines and keep what the owner did not ask to change. A paused routine stays paused; its next run is counted from now.",
            move |o, i: UpdateRoutineInput| {
                let by = update_by.clone();
                async move {
                    let (id, input) = i.split();
                    let r = o.update_routine(&id, &input, by).await?;
                    Ok(format!(
                        "Updated routine {}, {:?}: {}.",
                        r.id,
                        start_spec_of(&r).title,
                        next_run_text(&r, o.now())
                    ))
                }
            },
        );
        let pause_by = by.clone();
        t.serving(self).add(
            "pause_routine",
            Effect::Writes,
            "Pause a routine for the owner: it starts no chain until it resumes. A run at work goes on.",
            move |o, i: RoutineIdInput| {
                let by = pause_by.clone();
                async move {
                    let r = o.pause_routine(&i.id, by).await?;
                    Ok(format!("Paused routine {}, {:?}.", r.id, start_spec_of(&r).title))
                }
            },
        );
        let resume_by = by.clone();
        t.serving(self).add(
            "resume_routine", Effect::Writes,
            "Resume a paused routine for the owner. A routine that repeats waits for its next time: the times it missed are not run.",
            move |o, i: RoutineIdInput| {
                let by = resume_by.clone();
                async move {
                    let r = o.resume_routine(&i.id, by).await?;
                    Ok(format!(
                        "Resumed routine {}, {:?}: {}.",
                        r.id,
                        start_spec_of(&r).title,
                        next_run_text(&r, o.now())
                    ))
                }
            },
        );
        let run_by = by.clone();
        t.serving(self).add(
            "run_routine", Effect::Writes,
            "Start a routine's chain now instead of at its time, for the owner. One run at a time: refused while its last run is at work. A routine that repeats then waits for its next time counted from now; one that runs once has run.",
            move |o, i: RoutineIdInput| {
                let by = run_by.clone();
                async move {
                    let ch = o.run_routine(&i.id, by).await?;
                    Ok(format!("Started chain {} from routine {}.", ch.id, i.id))
                }
            },
        );
        t.serving(self).add(
            "delete_routine",
            Effect::Writes,
            "Delete a routine the owner named: it never starts a chain again. Chains it started are not affected.",
            |o, i: RoutineIdInput| async move {
                o.delete_routine(&i.id).await?;
                Ok(format!("Deleted routine {}.", i.id))
            },
        );
    }
}
