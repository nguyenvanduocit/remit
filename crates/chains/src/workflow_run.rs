//! The workflow shell (ADR-0073, decision 3): Remit runs a Chair's script durably. crates/workflow is the pure engine, a
//! function of (script, args, journal) that runs JavaScript for up to two seconds, so the shell calls it off the async
//! workers (`evaluate`). This module is the shell around it. A run is advanced in the change that records any of its
//! steps' results (a card's report, a wait's firing, the owner's answer): the shell replays the script against the journal
//! of its steps and applies each call that has no step yet through the path the Chair's own tool takes (`hiring_of` and the
//! brief, a subscription as subscribe makes it, the inbox question, accept_work's verdict), all in that change. Runs of a
//! chain change one at a time under its runs lock (`lock_runs`), taken before the chain's own lock.

use std::collections::HashSet;
use std::sync::Arc;

use model::{
    Actor, ActorKind, Card, Chain, ChainStatus, DecisionKind, Employee, ForeignJson, InboxItem, InboxKind, InboxStatus,
    ReportVerdict, StepStatus, Subscription, SubscriptionAction, Time, Workflow, WorkflowEngine, WorkflowRef,
    WorkflowStatus, WorkflowStep, WorkflowVerb,
};
use serde::Deserialize;
use store::Tx;
use tokio::sync::OwnedMutexGuard;
use workflow::{Entry, Outcome, ScriptError, ScriptState};

use crate::apply::Delivered;
use crate::apply::{InboxEdit, Records};
use crate::chain_core::Core;
use crate::chain_core::{first_line, new_id};
use crate::fact::{FactValue, fact_write, keys};
use crate::handover::handover_brief;
use crate::hire_input::HireInput;
use crate::lifecycle::stopped;
use crate::outbox::{Message, PRIORITY_OPENING, by_remit, new_message};
use crate::prompts::opening_title;
use crate::request::Done;
use crate::request::Service;
use crate::step::{CardEdit, ChainCmd, Decided, WorkflowCmd};
use crate::subscribe::subscribe;
use crate::turn_end::WorkerReport;
use crate::wait::{due_of, wait_owner};
use crate::watch::{SubscribeInput, wake_start};
use crate::{Error, Request, Result};

/// Evaluates script against journal off the async workers: the engine runs JavaScript for up to two seconds.
pub async fn evaluate(script: &str, args: Option<&ForeignJson>, journal: Vec<Entry>) -> Result<Outcome> {
    let (script, args) = (script.to_owned(), args.cloned());
    tokio::task::spawn_blocking(move || workflow::advance(&script, args.as_ref(), &journal))
        .await
        .map_err(|e| Error::refused(format!("the script's evaluation did not finish: {e}")))
}

/// Starts a run of a script: what the Chair's run_workflow takes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct WorkflowInput {
    /// What the owner reads the run as; empty uses the script's meta.name.
    pub name: String,
    /// The script: plain JavaScript beginning with export const meta = {name, description}, then the body that awaits
    /// agent(), parallel(), wait(), askOwner() and returns the run's result.
    pub script: String,
    /// What the script reads as args: a JSON object, the place for times and choices the script cannot make itself.
    #[schemars(with = "Option<serde_json::Map<String, serde_json::Value>>")]
    #[serde(deserialize_with = "args_object")]
    pub args: Option<ForeignJson>,
    /// The id of an earlier run of this chain whose recorded steps this one reuses up to the first that differs.
    pub resume_from: String,
    /// One or two sentences for the owner: what the run is for.
    pub why: String,
}

/// Reads run_workflow's args: a JSON object, none when left out or null. Anything else is refused at the boundary.
fn args_object<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<ForeignJson>, D::Error> {
    use serde::de::Error as _;
    match Option::<serde_json::Value>::deserialize(d)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(v @ serde_json::Value::Object(_)) => Ok(Some(ForeignJson(v))),
        Some(_) => Err(D::Error::custom("args must be a JSON object")),
    }
}

/// Bounds the evaluations of one change: each pass that settles a call at once (a log, an accept, a refused hire) replays
/// the script again, and a run makes at most a thousand calls.
const MAX_PASSES: usize = 1000;

/// The subscription a run's wait at step seq holds: one per step, so a stop or a finish finds it.
pub fn step_sub_id(run: &str, seq: i64) -> String {
    format!("sub_wf_{run}_{seq}")
}

/// A run being changed under its chain's runs lock: the run with its steps, and the steps the change writes.
#[derive(Debug, Clone, Default)]
pub struct RunChange {
    pub run: Workflow,
    pub dirty: Vec<i64>,
    pub now: Time,
    /// The employees this change put to work: not idle any more, though the store does not know it yet.
    pub taken: HashSet<String>,
}

impl RunChange {
    fn new(run: Workflow, now: Time) -> Self {
        Self {
            run,
            now,
            ..Self::default()
        }
    }

    fn step(&self, seq: i64) -> Option<WorkflowStep> {
        self.run.steps.iter().find(|s| s.seq == seq).cloned()
    }

    /// Writes s into the run, in seq order.
    fn put(&mut self, s: WorkflowStep) {
        let seq = s.seq;
        match self.run.steps.iter_mut().find(|x| x.seq == seq) {
            Some(x) => *x = s,
            None => {
                self.run.steps.push(s);
                self.run.steps.sort_by_key(|x| x.seq);
            }
        }
        if !self.dirty.contains(&seq) {
            self.dirty.push(seq);
        }
    }

    /// Gives step s its result, or its failure when why is set: it is the next result to arrive.
    fn settle(&mut self, mut s: WorkflowStep, result: Option<ForeignJson>, why: &str) {
        (s.status, s.result, s.error, s.ended_at) = (StepStatus::Done, result, String::new(), Some(self.now));
        if !why.is_empty() {
            (s.status, s.result, s.error) = (StepStatus::Failed, None, why.to_owned());
        }
        s.arrived = self
            .run
            .steps
            .iter()
            .map(|x| x.arrived.saturating_add(1))
            .fold(1, i64::max);
        self.put(s);
    }

    /// Ends the run as status, with result or why.
    fn end(&mut self, status: WorkflowStatus, result: Option<ForeignJson>, why: &str) {
        (self.run.status, self.run.result, self.run.error, self.run.ended_at) =
            (status, result, why.to_owned(), Some(self.now));
    }

    /// The run's steps that wait for a result.
    fn pending(&self) -> Vec<WorkflowStep> {
        self.run
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Pending)
            .cloned()
            .collect()
    }

    /// Adds the run and the steps the change writes to w.
    fn add_to(&self, w: &mut Records) {
        let mut run = self.run.clone();
        run.steps = Vec::new();
        w.runs.push(run);
        w.steps.extend(self.dirty.iter().filter_map(|seq| self.step(*seq)));
    }
}

/// The journal of a run's steps: each that has its result, in the order the results arrived.
pub fn journal_of(steps: &[WorkflowStep]) -> Vec<Entry> {
    let mut settled: Vec<&WorkflowStep> = steps.iter().filter(|s| s.status != StepStatus::Pending).collect();
    settled.sort_by_key(|s| s.arrived);
    settled
        .into_iter()
        .map(|s| Entry {
            seq: s.seq,
            verb: s.verb,
            hash: s.hash.clone(),
            result: s.result.clone(),
            err: if s.status == StepStatus::Failed {
                or(&s.error, "failed")
            } else {
                String::new()
            },
        })
        .collect()
}

/// Where an open run stands by its pending steps: waiting when it waits only on a time, a signal or the owner.
pub fn run_status_of(steps: &[WorkflowStep]) -> WorkflowStatus {
    let working = steps
        .iter()
        .any(|s| s.status == StepStatus::Pending && s.verb != WorkflowVerb::Wait && s.verb != WorkflowVerb::AskOwner);
    if working {
        WorkflowStatus::Running
    } else {
        WorkflowStatus::Waiting
    }
}

/// What a change of a run comes to besides its records: whether it puts a card to work, and what its Chair is told.
#[derive(Debug, Clone, Default)]
struct Moved {
    issues: bool,
    tell: Option<Message>,
    /// The soonest time a wait the change made comes due; none when none has a time.
    until: Option<Time>,
}

/// a, or b when a is empty.
fn or(a: &str, b: &str) -> String {
    if a.is_empty() { b.to_owned() } else { a.to_owned() }
}

/// Why a call of run at step seq was made, for the owner's record.
pub fn step_why(run: &Workflow, seq: i64) -> String {
    format!("Workflow {} ({}), step {seq}.", run.name, run.id)
}

fn decided(kind: DecisionKind, text: String, why: String, card_id: &str, by: Actor) -> Decided {
    Decided {
        kind,
        text,
        why,
        card_id: card_id.to_owned(),
        by,
    }
}

/// JSON of v.
fn raw_of<T: serde::Serialize + ?Sized>(v: &T) -> Result<ForeignJson> {
    Ok(ForeignJson(serde_json::to_value(v)?))
}

/// Why the hire a step s of run asks for was made: its step, and the phase it runs in.
pub fn hire_why(run: &Workflow, s: &WorkflowStep) -> String {
    let why = step_why(run, s.seq);
    if s.phase.is_empty() {
        return why;
    }
    format!("{}, phase {}.", why.trim_end_matches('.'), s.phase)
}

/// What agent() takes (the engine refuses isolation). Its model and effort are the hire's.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct AgentCall {
    prompt: String,
    label: String,
    schema: Option<Box<serde_json::value::RawValue>>,
    model: String,
    effort: String,
    agent_type: String,
}

impl AgentCall {
    fn of(input: Option<&ForeignJson>) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_value(input.map_or(serde_json::Value::Null, |j| j.0.clone())).map(Option::unwrap_or_default)
    }

    /// The schema the call names, when it names one (`null` names none).
    fn schema(&self) -> Option<&str> {
        self.schema
            .as_deref()
            .map(serde_json::value::RawValue::get)
            .filter(|s| *s != "null")
    }
}

/// What agent() returns for report: the report as a string, or, when the call names a schema, its last JSON value checked
/// against the schema. The error says what is wrong with it.
pub fn report_value(report: &str, input: Option<&ForeignJson>) -> std::result::Result<ForeignJson, String> {
    let call = AgentCall::of(input).unwrap_or_default();
    let Some(schema) = call.schema() else {
        return raw_of(report).map_err(|e| e.to_string());
    };
    let schema: serde_json::Value =
        serde_json::from_str(schema).map_err(|e| format!("the call's schema is no JSON schema: {e}"))?;
    let validator =
        jsonschema::validator_for(&schema).map_err(|e| format!("the call's schema is no JSON schema: {e}"))?;
    let raw = last_json(report).ok_or_else(|| "it holds no JSON value".to_owned())?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    validator
        .validate(&value)
        .map_err(|e| format!("its last JSON value does not match the schema: {e}"))?;
    ForeignJson::parse(&raw).map_err(|e| e.to_string())
}

/// The last whole JSON object or array in s, compacted.
pub fn last_json(s: &str) -> Option<String> {
    let mut last = None;
    let mut from = 0;
    while let Some(i) = s
        .get(from..)
        .and_then(|rest| rest.find(['{', '[']))
        .map(|i| from.saturating_add(i))
    {
        let rest = s.get(i..).unwrap_or_default();
        let mut values = serde_json::Deserializer::from_str(rest).into_iter::<serde_json::Value>();
        match values.next() {
            Some(Ok(v)) => {
                last = Some(v.to_string());
                from = i.saturating_add(values.byte_offset().max(1));
            }
            _ => from = i.saturating_add(1),
        }
    }
    last
}

/// What the step `seq` of an agent() or hire() names of the call, for the hire.
fn hire_of_input(s: &WorkflowStep) -> Result<HireInput> {
    serde_json::from_value::<Option<HireInput>>(s.input.as_ref().map_or(serde_json::Value::Null, |j| j.0.clone()))
        .map(Option::unwrap_or_default)
        .map_err(|e| Error::refused(format!("hire()'s input is not hire_agent's: {e}")))
}

/// Names who stopped a run, for its Chair.
fn stopper_of(by: &Actor, chair: &Card) -> String {
    match by.kind {
        ActorKind::Owner => "the owner".to_owned(),
        ActorKind::Assistant => "the Assistant, for the owner".to_owned(),
        ActorKind::Remit => "Remit".to_owned(),
        _ if by.who == chair.id => chair.name.clone(),
        _ => by.who.clone(),
    }
}

/// Stores, in tx, the runs and steps w writes, and the subscriptions it makes and ends, and keeps each run as last stored
/// in runs.
pub fn put_runs(tx: &Tx<'_>, w: &Records, now: Time, runs: &mut Vec<Workflow>) -> Result<()> {
    for id in &w.end_subs {
        match tx.delete_subscription(id) {
            Ok(()) | Err(store::Error::NotFound) => {}
            Err(e) => return Err(e.into()),
        }
    }
    for s in &w.subs {
        subscribe(tx, s.clone(), &wake_start(s), now)?;
    }
    let mut keep = |run: Workflow| match runs.iter_mut().find(|r| r.id == run.id) {
        Some(r) => *r = run,
        None => runs.push(run),
    };
    for run in &w.runs {
        keep(tx.put_workflow(run.clone())?);
    }
    for s in &w.steps {
        keep(tx.put_step(s)?);
    }
    Ok(())
}

/// Stops, in tx, every run of chain chain_id that has not ended, for why, as the chain closes out: their waits end with
/// them (the questions are cancelled with the chain's, and its sessions close).
pub fn stop_runs_of(tx: &Tx<'_>, chain_id: &str, why: &str, now: Time, runs: &mut Vec<Workflow>) -> Result<()> {
    for mut run in tx.workflows_of(chain_id)? {
        if run.status.ended() {
            continue;
        }
        for mut s in run
            .steps
            .clone()
            .into_iter()
            .filter(|s| s.status == StepStatus::Pending)
        {
            if s.verb == WorkflowVerb::Wait {
                match tx.delete_subscription(&step_sub_id(&run.id, s.seq)) {
                    Ok(()) | Err(store::Error::NotFound) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            (s.status, s.error, s.ended_at) = (StepStatus::Failed, why.to_owned(), Some(now));
            tx.put_step(&s)?;
        }
        (run.status, run.error, run.ended_at) = (WorkflowStatus::Stopped, why.to_owned(), Some(now));
        let stored = tx.put_workflow(run)?;
        runs.retain(|r| r.id != stored.id);
        runs.push(stored);
    }
    Ok(())
}

impl Core {
    /// Serialises the changes of chain id's runs until the guard is dropped.
    pub async fn lock_runs(&self, id: &str) -> Result<OwnedMutexGuard<()>> {
        let cs = self.state(id).await?;
        Ok(Arc::clone(&cs.runs).lock_owned().await)
    }

    /// Starts a run of input's script for chair's chain (run_workflow): the run, its first calls and what they put to work,
    /// in one change. A script that cannot be read, or that fails before its first call, is refused and nothing is stored.
    pub async fn start_workflow(&self, chair: &Card, input: WorkflowInput) -> Result<Workflow> {
        if chair.origin != model::Origin::Chair {
            return Err(Error::refused("only the Chair runs a workflow"));
        }
        if input.why.trim().is_empty() {
            return Err(Error::refused("why is required: what the run is for, for the owner"));
        }
        let _runs = self.lock_runs(&chair.home).await?;
        let ch = self.chain(&chair.home).await?;
        if *ch.status() != ChainStatus::Running {
            return Err(Error::refused(format!("the chain is {}", ch.status())));
        }
        let mut run = self
            .new_script_run(&ch, &input.script, input.args.clone(), self.now())
            .await?;
        run.card.clone_from(&chair.id);
        if !input.name.trim().is_empty() {
            input.name.trim().clone_into(&mut run.name);
        }
        if !input.resume_from.is_empty() {
            run.steps = self.reused_steps(&ch.id, &run, &input.resume_from).await?;
            run.resumed_from.clone_from(&input.resume_from);
        }
        let mut rc = RunChange::new(run.clone(), self.now());
        for s in run.steps.clone() {
            rc.put(s);
        }
        let mut w = Records {
            decisions: vec![decided(
                DecisionKind::Workflow,
                format!("Run workflow {} ({}).", run.name, run.id),
                input.why.clone(),
                &chair.id,
                model::by_agent(&chair.id),
            )],
            ..Records::default()
        };
        let m = self.advance(&ch, chair, &mut rc, &mut w).await?;
        rc.add_to(&mut w);
        let cmd = WorkflowCmd {
            run: run.id.clone(),
            after: rc.run.status,
            issues: m.issues,
            tell: m.tell,
            until: m.until,
            ..WorkflowCmd::default()
        };
        self.apply(&ch.id, ChainCmd::Workflow(cmd), w).await?;
        let id = run.id;
        Ok(self.store.read(move |r| r.workflow(&id)).await?)
    }

    /// A run of script for ch, engine remit, not stored: refused when the script cannot be read or fails before it makes
    /// its first call.
    pub async fn new_script_run(
        &self,
        ch: &Chain,
        script: &str,
        args: Option<ForeignJson>,
        now: Time,
    ) -> Result<Workflow> {
        if script.trim().is_empty() {
            return Err(Error::refused("script is empty"));
        }
        let first = evaluate(script, args.as_ref(), Vec::new()).await?;
        if first.state == ScriptState::Failed {
            return Err(Error::refused(format!("the script cannot run: {}", first.why())));
        }
        Ok(Workflow {
            id: new_id("w_"),
            home: ch.id.clone(),
            engine: WorkflowEngine::Remit,
            name: first.meta.name,
            script: script.to_owned(),
            args,
            status: WorkflowStatus::Running,
            created_at: now,
            ..Workflow::default()
        })
    }

    /// The steps of run from, an earlier run of the chain, that run's script makes the same: its settled ones up to the
    /// first the script makes differently, as Claude Code's resume reuses its journal's matching prefix.
    async fn reused_steps(&self, chain_id: &str, run: &Workflow, from: &str) -> Result<Vec<WorkflowStep>> {
        let key = from.to_owned();
        let old = self
            .store
            .read(move |r| r.workflow(&key))
            .await
            .map_err(|e| Error::refused(format!("resume_from {from}: {e}")))?;
        if old.home != chain_id || old.engine != WorkflowEngine::Remit {
            return Err(Error::refused(format!("resume_from {from} is not a run of this chain")));
        }
        let mut steps: Vec<WorkflowStep> = old
            .steps
            .iter()
            .filter(|s| s.status != StepStatus::Pending)
            .cloned()
            .collect();
        for _ in 0..=old.steps.len() {
            let out = evaluate(&run.script, run.args.as_ref(), journal_of(&steps)).await?;
            let Some(ScriptError::Diverged { step, .. }) = out.err else {
                break;
            };
            steps.retain(|s| s.seq < step);
        }
        for s in &mut steps {
            run.id.clone_into(&mut s.workflow);
            s.card = String::new();
        }
        Ok(steps)
    }

    /// The runs chain id begins with that have no Chair yet (signals.rs `start_run`).
    pub async fn queued_runs(&self, id: &str) -> Result<Vec<Workflow>> {
        let key = id.to_owned();
        let runs = self.store.read(move |r| r.workflows_of(&key)).await?;
        Ok(runs
            .into_iter()
            .filter(|r| r.card.is_empty() && !r.status.ended())
            .collect())
    }

    /// Gives the runs chain id began with its Chair and advances them: the chain has just started.
    pub async fn start_queued_runs(&self, id: &str) -> Result<()> {
        let runs = self.queued_runs(id).await?;
        if runs.is_empty() {
            return Ok(());
        }
        let _runs = self.lock_runs(id).await?;
        let ch = self.chain(id).await?;
        let chair_id = ch.chair_id.clone();
        let chair = self.store.read(move |r| r.card(&chair_id)).await?;
        for mut run in runs {
            run.card.clone_from(&chair.id);
            self.move_run(&ch, &chair, run, WorkflowCmd::default(), Records::default(), |_| {})
                .await?;
        }
        Ok(())
    }

    /// Advances run, its chain ch and its Chair chair as read under the runs lock, after edit has changed it, and stores
    /// the change with w, as cmd says it came (scheduled, card).
    async fn move_run(
        &self,
        ch: &Chain,
        chair: &Card,
        run: Workflow,
        mut cmd: WorkflowCmd,
        mut w: Records,
        edit: impl FnOnce(&mut RunChange),
    ) -> Result<()> {
        let mut rc = RunChange::new(run, self.now());
        edit(&mut rc);
        let m = self.advance(ch, chair, &mut rc, &mut w).await?;
        rc.add_to(&mut w);
        (cmd.run, cmd.after, cmd.tell, cmd.until) = (rc.run.id.clone(), rc.run.status, m.tell, m.until);
        cmd.issues = cmd.issues || m.issues;
        // a firing's delivery is outside the Tick, whose queued starts take slots too
        let _slot = if cmd.scheduled {
            Some(self.slot_mu.lock().await)
        } else {
            None
        };
        self.apply(&ch.id, ChainCmd::Workflow(cmd), w).await.map(|_| ())
    }

    /// Replays rc's run against its journal and applies the calls it made that have no step yet, adding what they store
    /// to w: until the script waits on calls still at work, returns or fails. A run of a paused chain is paused and
    /// nothing is issued.
    async fn advance(&self, ch: &Chain, chair: &Card, rc: &mut RunChange, w: &mut Records) -> Result<Moved> {
        let mut m = Moved::default();
        if rc.run.status.ended() {
            return Ok(m);
        }
        if *ch.status() == ChainStatus::Paused {
            rc.run.status = WorkflowStatus::Paused;
            return Ok(m);
        }
        for _ in 0..MAX_PASSES {
            let out = evaluate(&rc.run.script, rc.run.args.as_ref(), journal_of(&rc.run.steps)).await?;
            if out.state == ScriptState::Failed {
                rc.end(WorkflowStatus::Failed, None, &out.why());
                break;
            }
            let mut again = false;
            for call in &out.calls {
                if rc.step(call.seq).is_some() {
                    continue;
                }
                let s = WorkflowStep {
                    workflow: rc.run.id.clone(),
                    seq: call.seq,
                    verb: call.verb,
                    label: call.label.clone(),
                    phase: call.phase.clone(),
                    input: Some(call.input.clone()),
                    hash: call.hash.clone(),
                    status: StepStatus::Pending,
                    at: rc.now,
                    ..WorkflowStep::default()
                };
                if let Some(at) = self.issue(ch, chair, rc, s, w).await?
                    && m.until.is_none_or(|until| at < until)
                {
                    m.until = Some(at);
                }
                if let Some(now) = rc.step(call.seq) {
                    again = again || now.status != StepStatus::Pending;
                    m.issues = m.issues || now.status == StepStatus::Pending && !now.card.is_empty();
                }
            }
            if out.state == ScriptState::Done {
                rc.end(WorkflowStatus::Done, out.result, "");
                break;
            }
            if !again {
                break;
            }
        }
        if !rc.run.status.ended() {
            rc.run.status = run_status_of(&rc.run.steps);
            return Ok(m);
        }
        let why = format!("the run {}", rc.run.status);
        self.end_pending(rc, &why, w).await?;
        m.tell = self.end_message(ch, &rc.run, "").await;
        Ok(m)
    }

    /// Applies the call s through the Chair's own path, adding what it stores to w, and puts s into rc: pending while what
    /// it asked for works, done or failed when it is settled at once. A call the Chair's rules refuse fails with the
    /// refusal: the script reads it as a rejected promise. The time it answers is when a wait it made comes due.
    async fn issue(
        &self,
        ch: &Chain,
        chair: &Card,
        rc: &mut RunChange,
        mut s: WorkflowStep,
        w: &mut Records,
    ) -> Result<Option<Time>> {
        match self.issued(ch, chair, rc, &mut s, w).await {
            Ok(at) => Ok(at),
            Err(Error::Refused(why)) => {
                rc.settle(s, None, &why);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// `issue` but for the refusal, which fails the call.
    async fn issued(
        &self,
        ch: &Chain,
        chair: &Card,
        rc: &mut RunChange,
        s: &mut WorkflowStep,
        w: &mut Records,
    ) -> Result<Option<Time>> {
        let input = s.input.as_ref().map_or(serde_json::Value::Null, |j| j.0.clone());
        match &s.verb {
            WorkflowVerb::Log => {
                #[derive(Deserialize, Default)]
                #[serde(default)]
                struct In {
                    message: String,
                }
                let at: In = serde_json::from_value::<Option<In>>(input)
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                if s.label.is_empty() {
                    s.label = first_line(&at.message, 200);
                }
                rc.settle(s.clone(), None, "");
            }
            WorkflowVerb::Phase => {
                #[derive(Deserialize, Default)]
                #[serde(default)]
                struct In {
                    title: String,
                }
                let at: In = serde_json::from_value::<Option<In>>(input)
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                rc.run.phase = at.title;
                rc.settle(s.clone(), None, "");
            }
            WorkflowVerb::Agent | WorkflowVerb::Hire => {
                let hin = self.hire_of_call(chair, rc, s).await?;
                let h = self.domains()?.hiring_of(chair, hin).await?;
                let c = h.card;
                rc.taken.insert(c.employee.clone());
                w.employees.extend(h.employees);
                w.edges.extend(h.edges);
                w.decisions.push(h.decision);
                w.messages.push(Message {
                    to: c.id.clone(),
                    home: ch.id.clone(),
                    title: opening_title(&c),
                    text: handover_brief(
                        &h.chain,
                        &c,
                        &h.reports,
                        &h.decided,
                        "",
                        &self.cfg.principles,
                        &self.cfg.language,
                    ),
                    by: model::by_agent(&chair.id),
                    priority: PRIORITY_OPENING,
                    ..Message::default()
                });
                s.card.clone_from(&c.id);
                if s.label.is_empty() {
                    s.label.clone_from(&c.name);
                }
                w.created.push(c);
                rc.put(s.clone());
            }
            WorkflowVerb::Wait => {
                let sub = self.wait_of_call(ch, chair, &rc.run, s).await?;
                let at = due_of(&sub);
                w.subs.push(sub);
                rc.put(s.clone());
                return Ok(at);
            }
            WorkflowVerb::AskOwner => {
                #[derive(Deserialize, Default)]
                #[serde(default)]
                struct In {
                    question: String,
                }
                let question = serde_json::from_value::<Option<In>>(input)
                    .ok()
                    .flatten()
                    .map(|i| i.question)
                    .filter(|q| !q.trim().is_empty())
                    .ok_or_else(|| Error::refused("askOwner() needs a question"))?;
                let it = InboxItem {
                    id: new_id("i_"),
                    home: ch.id.clone(),
                    card_id: chair.id.clone(),
                    kind: InboxKind::Question,
                    title: first_line(&question, 120),
                    detail: format!(
                        "{question}\n\n(workflow {} asks, and goes on with your answer)",
                        rc.run.name
                    ),
                    status: InboxStatus::Open,
                    created_at: rc.now,
                    workflow: Some(WorkflowRef {
                        run: rc.run.id.clone(),
                        seq: s.seq,
                    }),
                    ..InboxItem::default()
                };
                let fact = fact_write(
                    keys::owner_asked(),
                    &FactValue {
                        chain: ch.id.clone(),
                        item: it.id.clone(),
                        ..FactValue::default()
                    },
                    model::by_agent(&chair.id),
                    &ch.id,
                )
                .map_err(|e| match e {
                    Error::Refused(why) => Error::refused(format!("the owner's question cannot be recorded: {why}")),
                    other => other,
                })?;
                w.decisions.push(decided(
                    DecisionKind::Ask,
                    format!("Ask you: {}", it.title),
                    step_why(&rc.run, s.seq),
                    &chair.id,
                    model::by_agent(&chair.id),
                ));
                w.inbox.push(it);
                w.readings.push(fact);
                rc.put(s.clone());
            }
            WorkflowVerb::Accept | WorkflowVerb::SendBack => {
                self.verdict_of_call(ch, chair, &rc.run, s, w).await?;
                if s.verb == WorkflowVerb::Accept {
                    rc.settle(s.clone(), None, "");
                } else {
                    rc.put(s.clone()); // the card works again: its next report settles the call
                }
            }
            WorkflowVerb::Workflow => {
                return Err(Error::refused(
                    "workflow() runs a saved script: name a script inline; Remit keeps no saved workflows yet",
                ));
            }
        }
        Ok(None)
    }

    /// The hire an agent() or hire() call s of rc's run asks for, in hire_agent's words. agent() puts to work the employee
    /// its agentType names (an employee's id or handle), a new worker of the agent it names, or, naming none, a kept
    /// worker that is idle before a new general-purpose one: the Chair's rule, reuse before you hire.
    async fn hire_of_call(&self, chair: &Card, rc: &RunChange, s: &WorkflowStep) -> Result<HireInput> {
        let run = &rc.run;
        if s.verb == WorkflowVerb::Hire {
            let mut h = hire_of_input(s)?;
            h.why = format!("{} {}", h.why, hire_why(run, s)).trim().to_owned();
            return Ok(h);
        }
        let call = AgentCall::of(s.input.as_ref()).map_err(|e| Error::refused(format!("agent()'s input: {e}")))?;
        let mut h = HireInput {
            name: or(&call.label, &format!("{} · step {}", run.name, s.seq)),
            brief: call.prompt.clone(),
            model: call.model.clone(),
            effort: call.effort.clone(),
            why: hire_why(run, s),
            deliverable: "your report, answering the brief".to_owned(),
            criteria: vec!["the report answers the brief".to_owned()],
            ..HireInput::default()
        };
        if let Some(schema) = call.schema() {
            h.deliverable = format!("your report, ending with one JSON value that matches this JSON schema:\n{schema}");
            h.criteria = vec!["the report's last JSON value matches the schema".to_owned()];
        }
        let staff = self.store.read(|r| r.employees()).await?;
        let who = call.agent_type.trim();
        if who.is_empty() || who == "general-purpose" {
            let idle = self.idle_worker(chair, &staff, &rc.taken).await?;
            if idle.is_empty() {
                h.agent = "general-purpose".to_owned();
            }
            h.employee = idle;
            return Ok(h);
        }
        match staff
            .iter()
            .find(|e| e.id == who || crate::subagents::employee_handle(e) == who)
        {
            Some(e) => h.employee.clone_from(&e.id),
            None => who.clone_into(&mut h.agent),
        }
        Ok(h)
    }

    /// A kept worker among staff that chair may put to work and that holds no card open, nor is taken: empty when there is
    /// none.
    async fn idle_worker(&self, chair: &Card, staff: &[Employee], taken: &HashSet<String>) -> Result<String> {
        let id = chair.employee.clone();
        let hirer = self
            .store
            .read(move |r| r.employee(&id))
            .await
            .map_err(|e| Error::refused(format!("the Chair is no employee on record: {e}")))?;
        let busy = self.busy_of(staff).await?;
        let d = self.domains()?;
        for e in staff {
            let input = HireInput {
                employee: e.id.clone(),
                ..HireInput::default()
            };
            if !busy.contains(&e.id) && !taken.contains(&e.id) && d.standing_refusal(&hirer, e, &input).is_ok() {
                return Ok(e.id.clone());
            }
        }
        Ok(String::new())
    }

    /// The subscription a wait() call s of run makes: subscribe's, firing into the step.
    async fn wait_of_call(&self, ch: &Chain, chair: &Card, run: &Workflow, s: &WorkflowStep) -> Result<Subscription> {
        let mut input: SubscribeInput = serde_json::from_value::<Option<SubscribeInput>>(
            s.input.as_ref().map_or(serde_json::Value::Null, |j| j.0.clone()),
        )
        .map_err(|e| {
            Error::refused(format!(
                "wait()'s input is subscribe's ({{at}} or {{key, condition, until}}): {e}"
            ))
        })?
        .unwrap_or_default();
        input.why = or(input.why.trim(), &step_why(run, s.seq));
        let mut sub = self.subscription_of(ch, chair, &input, self.now()).await?;
        sub.id = step_sub_id(&run.id, s.seq);
        sub.action = SubscriptionAction {
            kind: model::ActionKind::Workflow,
            why: input.why,
            workflow: Some(WorkflowRef {
                run: run.id.clone(),
                seq: s.seq,
            }),
            ..SubscriptionAction::default()
        };
        Ok(sub)
    }

    /// Applies an accept() or sendBack() call s: accept_work's verdict on the card it names, s naming the card.
    async fn verdict_of_call(
        &self,
        ch: &Chain,
        chair: &Card,
        run: &Workflow,
        s: &mut WorkflowStep,
        w: &mut Records,
    ) -> Result<()> {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct In {
            card: String,
            notes: String,
            feedback: String,
        }
        let input: In =
            serde_json::from_value::<Option<In>>(s.input.as_ref().map_or(serde_json::Value::Null, |j| j.0.clone()))
                .map_err(|e| Error::refused(e.to_string()))?
                .unwrap_or_default();
        let c = self.team_card(&ch.id, &input.card).await?;
        let by = model::by_agent(&chair.id);
        let (verdict, kind, text, why) = if s.verb == WorkflowVerb::SendBack {
            let mut m = new_message(
                &c.id,
                "Rework after feedback",
                &format!(
                    "[feedback from the Chair's workflow · rework]\n{}\n\nFix it, then report again against every acceptance criterion.",
                    input.feedback
                ),
                by.clone(),
            );
            (m.home, m.steer) = (ch.id.clone(), true);
            w.messages.push(m);
            (
                ReportVerdict::Rework,
                DecisionKind::Rework,
                format!("Send {} back for rework.", c.name),
                input.feedback,
            )
        } else {
            (
                ReportVerdict::Accept,
                DecisionKind::Accept,
                format!("Accept {}: passed.", c.name),
                or(&input.notes, &step_why(run, s.seq)),
            )
        };
        w.readings.push(crate::fact::taught_verdict(&c, verdict, &by)?);
        w.cards.push(CardEdit::new(&c.id, |c| c.awaiting_review = false));
        w.decisions.push(decided(kind, text, why, &c.id, by));
        s.card = c.id;
        Ok(())
    }

    /// Fails the steps of rc's ended run that still wait, for why: their waits' subscriptions end and their questions are
    /// withdrawn. A card still at work goes on: its report reaches the Chair as any report does.
    async fn end_pending(&self, rc: &mut RunChange, why: &str, w: &mut Records) -> Result<()> {
        let mut asked = false;
        for mut s in rc.pending() {
            match s.verb {
                WorkflowVerb::Wait => w.end_subs.push(step_sub_id(&rc.run.id, s.seq)),
                WorkflowVerb::AskOwner => asked = true,
                _ => {}
            }
            (s.status, s.error, s.ended_at) = (StepStatus::Failed, why.to_owned(), Some(rc.now));
            rc.put(s);
        }
        if !asked {
            return Ok(());
        }
        for it in self.store.read(|r| r.open_inbox()).await? {
            if it.workflow.as_ref().is_some_and(|r| r.run == rc.run.id) {
                let why = why.to_owned();
                w.inbox_edits.push(InboxEdit::new(&it.id, move |it| {
                    (it.status, it.answer) = (InboxStatus::Cancelled, why.clone());
                }));
            }
        }
        Ok(())
    }

    /// What the Chair of run is told when it ended: its result or why it failed or was stopped (by, when set). A Chair that
    /// has had no message yet, as a routine's whose chain began with the run, reads its opening first.
    async fn end_message(&self, ch: &Chain, run: &Workflow, by: &str) -> Option<Message> {
        if run.card.is_empty() {
            return None;
        }
        let mut b = format!("[workflow {} ({}) ended: {}", run.name, run.id, run.status);
        if !by.is_empty() {
            b.push_str(&format!(" by {by}"));
        }
        b.push_str("]\n");
        if run.status == WorkflowStatus::Done {
            b.push_str(&format!(
                "Its result:\n{}",
                run.result.as_ref().map_or_else(|| "null".to_owned(), ForeignJson::text)
            ));
        } else {
            b.push_str(&run.error);
        }
        b.push_str(
            "\nIts steps and the cards they put to work are in list_workflows. Judge the result, then continue.",
        );
        let mut m = Message {
            to: run.card.clone(),
            home: ch.id.clone(),
            title: format!("Workflow {} ended", run.name),
            text: b,
            by: by_remit(),
            ..Message::default()
        };
        let card = run.card.clone();
        if self
            .store
            .read(move |r| r.card_events(&card, 1))
            .await
            .is_ok_and(|e| e.is_empty())
            && let Ok(all) = self.store.read(|r| r.chains()).await
            && let Ok(opening) = self.chair_opening(ch, &all).await
        {
            (m.title, m.priority) = ("Start the chain".to_owned(), PRIORITY_OPENING);
            m.text = format!("{opening}\n\n{}", m.text);
        }
        Some(m)
    }

    /// Records the report h of card c as the result of the run's step that waits on it, with the card's change edit and w,
    /// and advances the run, in one change. False when no step waits on c: the report goes to the Chair as any does.
    pub async fn report_to_run(
        &self,
        c: &Card,
        h: &WorkerReport,
        edit: impl Fn(&mut Card) + Clone + Send + Sync + 'static,
        mut w: Records,
    ) -> Result<bool> {
        let card = c.id.clone();
        let s = match self.store.read(move |r| r.step_of_card(&card)).await {
            Ok(s) if s.status == StepStatus::Pending => s,
            Ok(_) | Err(store::Error::NotFound) => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        let _runs = self.lock_runs(&c.home).await?;
        let (run, ch, chair) = self.run_and_chain(&s.workflow).await?;
        let mut rc = RunChange::new(run.clone(), self.now());
        let Some(mut s) = rc.step(s.seq) else { return Ok(false) };
        if s.status != StepStatus::Pending || run.status.ended() || s.card != c.id {
            return Ok(false);
        }
        let by = model::by_agent(&chair.id);
        if s.verb == WorkflowVerb::Agent {
            match report_value(&h.output, s.input.as_ref()) {
                Err(why) if !s.sent_back => {
                    s.sent_back = true;
                    rc.put(s);
                    let mut m = new_message(
                        &c.id,
                        "Report again",
                        &format!(
                            "[the workflow could not read your report] {why}\nReport again, ending with one JSON value that matches the schema in your deliverable."
                        ),
                        by.clone(),
                    );
                    (m.home, m.steer) = (ch.id.clone(), true);
                    w.cards.push(CardEdit::new(&c.id, edit));
                    w.messages.push(m);
                    w.decisions.push(decided(
                        DecisionKind::Rework,
                        format!("Send {} back: its report does not match the schema.", c.name),
                        why,
                        &c.id,
                        by,
                    ));
                    rc.add_to(&mut w);
                    let cmd = WorkflowCmd {
                        run: run.id.clone(),
                        card: c.id.clone(),
                        after: rc.run.status,
                        issues: true,
                        ..WorkflowCmd::default()
                    };
                    self.apply(&ch.id, ChainCmd::Workflow(cmd), w).await?;
                    return Ok(true);
                }
                Err(why) => {
                    rc.settle(
                        s,
                        None,
                        &format!("the report, sent back once, still does not match the schema: {why}"),
                    );
                    w.cards.push(CardEdit::new(&c.id, edit));
                }
                Ok(value) => {
                    w.readings
                        .push(crate::fact::taught_verdict(c, ReportVerdict::Accept, &by)?);
                    rc.settle(s.clone(), Some(value), "");
                    w.cards.push(CardEdit::new(&c.id, move |x| {
                        edit(x);
                        x.awaiting_review = false;
                    }));
                    w.decisions.push(decided(
                        DecisionKind::Accept,
                        format!("Accept {}: passed.", c.name),
                        step_why(&run, s.seq),
                        &c.id,
                        by,
                    ));
                }
            }
        } else {
            // hire, send_back: the script judges the report itself
            let mut result = std::collections::BTreeMap::new();
            result.insert("card", c.id.as_str());
            result.insert("report", h.output.as_str());
            rc.settle(s, Some(raw_of(&result)?), "");
            w.cards.push(CardEdit::new(&c.id, move |x| {
                edit(x);
                x.awaiting_review = true;
            }));
        }
        let dirty = rc.dirty.clone();
        let cmd = WorkflowCmd {
            card: c.id.clone(),
            ..WorkflowCmd::default()
        };
        self.move_run(&ch, &chair, rc.run, cmd, w, |next| next.dirty = dirty)
            .await?;
        Ok(true)
    }

    /// Reads run id, the chain it runs in and its Chair, under the runs lock.
    async fn run_and_chain(&self, id: &str) -> Result<(Workflow, Chain, Card)> {
        let key = id.to_owned();
        let run = self.store.read(move |r| r.workflow(&key)).await?;
        let ch = self.chain(&run.home).await?;
        let chair_id = or(&run.card, &ch.chair_id);
        let chair = self.store.read(move |r| r.card(&chair_id)).await?;
        Ok((run, ch, chair))
    }

    /// Settles the step r names with result, the firing of its wait, which fired stores done in the same change: a
    /// scheduled wake of a waiting chain (admit). A step that no longer waits stores the firing done alone.
    pub async fn resume_workflow_step(&self, r: &WorkflowRef, result: ForeignJson, fired: Delivered) -> Result<()> {
        let key = r.run.clone();
        let run = self.store.read(move |s| s.workflow(&key)).await?;
        let w = Records {
            fired: Some(fired.clone()),
            ..Records::default()
        };
        let cmd = WorkflowCmd {
            scheduled: true,
            ..WorkflowCmd::default()
        };
        if self.settle_step(&run.home, r, result, cmd, w).await? {
            return Ok(());
        }
        let mut dd = fired;
        "the workflow step no longer waits".clone_into(&mut dd.outcome);
        self.store_done(dd, |_, _| Ok(())).await
    }

    /// Settles the step r of a run of chain id with result, storing w with it, as cmd says it came, and advances the run.
    /// False, and nothing stored, when the step no longer waits.
    async fn settle_step(
        &self,
        id: &str,
        r: &WorkflowRef,
        result: ForeignJson,
        cmd: WorkflowCmd,
        w: Records,
    ) -> Result<bool> {
        let _runs = self.lock_runs(id).await?;
        let (run, ch, chair) = self.run_and_chain(&r.run).await?;
        let rc = RunChange::new(run.clone(), self.now());
        let Some(s) = rc
            .step(r.seq)
            .filter(|s| s.status == StepStatus::Pending && !run.status.ended())
        else {
            return Ok(false);
        };
        self.move_run(&ch, &chair, run, cmd, w, |next| next.settle(s, Some(result), ""))
            .await?;
        Ok(true)
    }

    /// Records the owner's answer to item it, a question a run's askOwner() asked, as that step's result.
    pub async fn answer_step(&self, it: InboxItem, answer: &str) -> Result<()> {
        let Some(r) = it.workflow.clone() else {
            return Err(Error::refused(format!(
                "inbox item {} asks for no workflow step",
                it.id
            )));
        };
        let w = Records {
            inbox: vec![it.clone()],
            ..Records::default()
        };
        if self
            .settle_step(&it.home, &r, raw_of(answer)?, WorkflowCmd::default(), w.clone())
            .await?
        {
            return Ok(());
        }
        self.commit(None, None, w).await.map(|_| ())
    }

    /// The owner stopping a run: its cards at work stopped, its waits ended, its question withdrawn, the run stopped with
    /// who and why, in one change.
    pub async fn stop_workflow(self: &Arc<Self>, req: Request, id: String, why: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "StopWorkflow", move |o| async move {
            let why = or(&why, "stopped by the owner");
            o.stop_workflow_as(&id, &why, &by).await
        })
        .await
    }

    /// Stops run id for why, by: see `stop_workflow`. Its Chair is told unless it stopped the run itself. A run of engine
    /// claude is stopped in its card's process (stop_task), which ends it.
    pub async fn stop_workflow_as(self: &Arc<Self>, id: &str, why: &str, by: &Actor) -> Result<()> {
        let key = id.to_owned();
        let run = self.store.read(move |r| r.workflow(&key)).await?;
        if run.engine == WorkflowEngine::Claude {
            return self.stop_claude_run(run, why, by).await;
        }
        let _runs = self.lock_runs(&run.home).await?;
        let (run, ch, chair) = self.run_and_chain(id).await?;
        if run.status.ended() {
            return Err(Error::refused(format!(
                "workflow {} is {} already",
                run.name, run.status
            )));
        }
        let mut rc = RunChange::new(run.clone(), self.now());
        let mut w = Records::default();
        let mut closing = Vec::new();
        for s in rc.pending().into_iter().filter(|s| !s.card.is_empty()) {
            closing.push(self.closing(&s.card, &model::Cut::WorkflowStopped).await);
            let st = stopped(&s.card, why, self.now());
            w.cards.extend(st.cards);
            w.drops.extend(st.drops);
            w.ended.extend(st.ended);
        }
        rc.end(WorkflowStatus::Stopped, None, why);
        self.end_pending(&mut rc, "the run was stopped", &mut w).await?;
        w.decisions.push(decided(
            DecisionKind::Stop,
            format!("Stop workflow {}.", run.name),
            why.to_owned(),
            &run.card,
            by.clone(),
        ));
        rc.add_to(&mut w);
        let mut cmd = WorkflowCmd {
            run: run.id.clone(),
            after: rc.run.status,
            ..WorkflowCmd::default()
        };
        if *by != model::by_agent(&run.card) {
            cmd.tell = self.end_message(&ch, &rc.run, &stopper_of(by, &chair)).await;
        }
        self.apply(&ch.id, ChainCmd::Workflow(cmd), w).await?;
        drop(closing);
        Ok(())
    }

    /// The command observed under the chain's lock: whether the change leaves the chain resting on its runs. The run waits
    /// only, so does every other run, its Chair is not in a turn and has nothing queued, and nothing else is at work.
    pub async fn observe_workflow(&self, mut c: WorkflowCmd, ch: &Chain) -> WorkflowCmd {
        if *ch.status() != ChainStatus::Running || c.issues || c.tell.is_some() || c.after != WorkflowStatus::Waiting {
            return c;
        }
        if self.run_of(&ch.chair_id).is_some_and(|r| r.busy()) {
            return c; // its turn's end settles it (TurnEndedCmd)
        }
        let chair = ch.chair_id.clone();
        if !self
            .store
            .read(move |r| r.queue(&chair))
            .await
            .is_ok_and(|q| q.is_empty())
        {
            return c;
        }
        if self.others_busy(ch, &c.card).await.0 {
            return c;
        }
        let written = (!c.run.is_empty()).then_some((c.run.as_str(), c.after));
        (c.rest, c.until) = self.runs_resting(&ch.id, written, c.until).await;
        c
    }

    /// Whether every run of chain id that has not ended waits only on a time, a signal or the owner (the run a change
    /// writes, written, counted as the status it leaves it at), and at least one does, with the soonest time one of their
    /// waits comes due (until counted too; none when none has one).
    pub async fn runs_resting(
        &self,
        id: &str,
        written: Option<(&str, WorkflowStatus)>,
        mut until: Option<Time>,
    ) -> (bool, Option<Time>) {
        let key = id.to_owned();
        let Ok(runs) = self.store.read(move |r| r.workflows_of(&key)).await else {
            return (false, until);
        };
        // the run the change writes: stored or not yet, it is as the change leaves it
        let mut waits = written.is_some_and(|(_, after)| after == WorkflowStatus::Waiting);
        for r in &runs {
            if written.is_some_and(|(run, _)| r.id == run) || r.engine != WorkflowEngine::Remit || r.status.ended() {
                continue;
            }
            if r.status != WorkflowStatus::Waiting {
                return (false, until);
            }
            waits = true;
        }
        if !waits {
            return (false, until);
        }
        let owner = wait_owner(id);
        let Ok(subs) = self.store.read(move |r| r.subscriptions_of(&owner)).await else {
            return (false, until);
        };
        for s in subs.iter().filter(|s| s.action.kind == model::ActionKind::Workflow) {
            if let Some(at) = due_of(s)
                && until.is_none_or(|until| at < until)
            {
                until = Some(at);
            }
        }
        (true, until)
    }

    /// The runs of chain id, engine remit, that have not ended.
    async fn open_runs(&self, id: &str) -> Result<Vec<Workflow>> {
        let key = id.to_owned();
        let runs = self.store.read(move |r| r.workflows_of(&key)).await?;
        Ok(runs
            .into_iter()
            .filter(|r| r.engine == WorkflowEngine::Remit && !r.status.ended())
            .collect())
    }

    /// The change that pauses chain id's open runs, for its pause: no call is issued while it is paused.
    pub async fn paused_runs(&self, id: &str) -> Result<Records> {
        let mut w = Records::default();
        for mut run in self.open_runs(id).await? {
            (run.status, run.steps) = (WorkflowStatus::Paused, Vec::new());
            w.runs.push(run);
        }
        Ok(w)
    }

    /// Advances chain id's paused runs, now that it runs again: each issues what came due while it was paused.
    pub async fn resume_runs(&self, id: &str) -> Result<()> {
        let _runs = self.lock_runs(id).await?;
        for run in self.open_runs(id).await? {
            if run.status != WorkflowStatus::Paused || run.card.is_empty() {
                continue;
            }
            let (mut run, ch, chair) = self.run_and_chain(&run.id).await?;
            run.status = run_status_of(&run.steps);
            self.move_run(&ch, &chair, run, WorkflowCmd::default(), Records::default(), |_| {})
                .await?;
        }
        Ok(())
    }

    /// Why the Chair of chain id may not finish yet: a run of it has not ended. Empty when none is open.
    pub async fn workflow_refusal(&self, id: &str) -> Result<String> {
        let runs = self.open_runs(id).await?;
        if runs.is_empty() {
            return Ok(String::new());
        }
        let names: Vec<String> = runs
            .iter()
            .map(|r| format!("{} ({}, {})", r.name, r.id, r.status))
            .collect();
        Ok(format!(
            "These workflows have not ended:\n- {}\nTheir end reaches you as a message. Wait for it, or stop one with stop_workflow, then finish_chain again.",
            names.join("\n- ")
        ))
    }
}
