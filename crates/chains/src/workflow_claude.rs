//! Claude Code's own Workflow runs, recorded (ADR-0073, decision 2): a card's process runs the script and owns its
//! journal; Remit keeps one Workflow of engine claude for it, with the script and the handle that resumes it, and a step
//! of verb agent for each agent its progress names. Its agents run inside the run: they have no card and no stream of
//! their own. The run dies with the process: it is stopped then, and its error names the call that resumes it.

use std::sync::Arc;

use model::{
    Actor, Card, ClaudeRun, DecisionKind, ForeignJson, StepStatus, TaskStatus, Time, Workflow, WorkflowEngine,
    WorkflowStatus, WorkflowStep, WorkflowVerb,
};

use crate::apply::Records;
use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::session::Run;
use crate::step::Decided;
use crate::{Error, Result};

/// How long a stop_task request may take to reach the card's process.
const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// a, or b when a is empty.
fn or(a: &str, b: &str) -> String {
    if a.is_empty() { b.to_owned() } else { a.to_owned() }
}

/// JSON of a string.
fn raw_string(s: &str) -> Option<ForeignJson> {
    serde_json::to_string(s).ok().and_then(|j| ForeignJson::parse(&j).ok())
}

/// Whether a and b say the same of a run, its steps aside.
fn same_run(a: &Workflow, b: &Workflow) -> bool {
    a.status == b.status
        && a.name == b.name
        && a.phase == b.phase
        && a.error == b.error
        && a.result == b.result
        && a.script == b.script
        && a.claude == b.claude
}

/// Whether a and b say the same of a step.
fn same_step(a: &WorkflowStep, b: &WorkflowStep) -> bool {
    a.status == b.status
        && a.label == b.label
        && a.phase == b.phase
        && a.error == b.error
        && a.result == b.result
        && a.input == b.input
}

/// Run prev as task t says it stands at now, with the steps that changed. Pure.
pub fn claude_run_of(prev: &Workflow, t: &wire::runtime::Task, now: Time) -> (Workflow, Vec<WorkflowStep>) {
    let mut run = prev.clone();
    run.name = or(&t.name, &or(&t.title, &run.name));
    run.script = or(&t.prompt, &run.script);
    let mut claude = run.claude.take().unwrap_or_default();
    claude.task_id.clone_from(&t.id);
    claude.run_id = or(&t.run_id, &claude.run_id);
    claude.script_path = or(&t.script_path, &claude.script_path);
    claude.transcript_dir = or(&t.transcript_dir, &claude.transcript_dir);
    run.claude = Some(claude);
    match t.status {
        TaskStatus::Running => (run.status, run.ended_at, run.error) = (WorkflowStatus::Running, None, String::new()),
        TaskStatus::Completed => (run.status, run.result) = (WorkflowStatus::Done, raw_string(&t.summary)),
        TaskStatus::Failed => (run.status, run.error) = (WorkflowStatus::Failed, t.summary.clone()),
        TaskStatus::Stopped => (run.status, run.error) = (WorkflowStatus::Stopped, or(&t.summary, "stopped")),
    }
    if run.status.ended() && run.ended_at.is_none() {
        run.ended_at = Some(now);
    }
    let mut arrived = run.steps.iter().map(|s| s.arrived).max().unwrap_or(0);
    let mut changed = Vec::new();
    for (n, a) in (1_i64..).zip(&t.agents) {
        let seq = if a.index == 0 { n } else { a.index }.saturating_sub(1);
        let input = serde_json::json!({"prompt": a.prompt, "agentType": a.agent_type});
        let mut s = WorkflowStep {
            workflow: run.id.clone(),
            seq,
            verb: WorkflowVerb::Agent,
            label: a.label.clone(),
            phase: a.phase.clone(),
            at: now,
            input: Some(ForeignJson(input)),
            status: StepStatus::Pending,
            ..WorkflowStep::default()
        };
        let i = run.steps.iter().position(|x| x.seq == seq);
        if let Some(x) = i.and_then(|i| run.steps.get(i)) {
            (s.at, s.ended_at, s.arrived) = (x.at, x.ended_at, x.arrived);
        }
        match a.state.as_str() {
            "done" => (s.status, s.result) = (StepStatus::Done, raw_string(&a.result)),
            "error" => (s.status, s.error) = (StepStatus::Failed, or(&a.error, "failed")),
            _ => {}
        }
        if s.status != StepStatus::Pending && s.arrived == 0 {
            arrived = arrived.saturating_add(1);
            (s.arrived, s.ended_at) = (arrived, Some(now));
        }
        if !a.phase.is_empty() {
            run.phase.clone_from(&a.phase);
        }
        match i.and_then(|i| run.steps.get_mut(i)) {
            Some(x) if same_step(x, &s) => continue,
            Some(x) => *x = s.clone(),
            None => run.steps.push(s.clone()),
        }
        changed.push(s);
    }
    (run, changed)
}

/// The call that resumes Claude Code's run, which its card makes in a new process.
pub fn resume_call(c: &ClaudeRun) -> String {
    format!(
        "Workflow({{scriptPath: {:?}, resumeFromRunId: {:?}}})",
        c.script_path, c.run_id
    )
}

impl Core {
    /// Records the workflow run task t of r's process as it stands now.
    pub async fn on_workflow_task(&self, r: &Arc<Run>, t: &wire::runtime::Task) {
        let card = r.card_now();
        let home = card.home.clone();
        let runs = match self.store.read(move |s| s.workflows_of(&home)).await {
            Ok(runs) => runs,
            Err(err) => {
                tracing::error!(home = %card.home, card = %card.id, %err, "read the runs of Claude Code's workflow");
                return;
            }
        };
        let found = runs.into_iter().find(|w| {
            w.engine == WorkflowEngine::Claude
                && w.card == card.id
                // a resume of the run continues its record
                && w.claude.as_ref().is_some_and(|c| c.task_id == t.id || !t.run_id.is_empty() && c.run_id == t.run_id)
        });
        let known = found.is_some();
        let prev = found.unwrap_or_else(|| Workflow {
            id: new_id("w_"),
            home: card.home.clone(),
            card: card.id.clone(),
            engine: WorkflowEngine::Claude,
            created_at: self.now(),
            ..Workflow::default()
        });
        let (mut next, steps) = claude_run_of(&prev, t, self.now());
        if known && steps.is_empty() && same_run(&prev, &next) {
            return;
        }
        next.steps = Vec::new();
        let w = Records {
            runs: vec![next],
            steps,
            ..Records::default()
        };
        if let Err(err) = self.commit(None, None, w).await {
            tracing::error!(home = %card.home, card = %card.id, task = %t.id, %err, "record Claude Code's workflow");
        }
    }

    /// Stops the runs of Claude Code's Workflow that card's process ran and that had not ended, as it ended for why, and
    /// names each with the call that resumes it, for the card's recovery message.
    pub async fn cut_claude_runs(&self, card: &Card, why: &model::Cut) -> Vec<String> {
        let home = card.home.clone();
        let runs = match self.store.read(move |s| s.workflows_of(&home)).await {
            Ok(runs) => runs,
            Err(err) => {
                tracing::error!(home = %card.home, card = %card.id, %err, "read the runs a process ended");
                return Vec::new();
            }
        };
        let mut w = Records::default();
        let mut named = Vec::new();
        for mut run in runs {
            let Some(claude) = run.claude.clone() else { continue };
            if run.engine != WorkflowEngine::Claude || run.card != card.id || run.status.ended() {
                continue;
            }
            (run.status, run.ended_at, run.steps) = (WorkflowStatus::Stopped, Some(self.now()), Vec::new());
            run.error = format!(
                "its card's process ended ({why}); it resumes with {}",
                resume_call(&claude)
            );
            named.push(format!(
                "the workflow {}, which {} resumes",
                run.name,
                resume_call(&claude)
            ));
            w.runs.push(run);
        }
        if w.runs.is_empty() {
            return Vec::new();
        }
        if let Err(err) = self.commit(None, None, w).await {
            tracing::error!(home = %card.home, card = %card.id, %err, "stop the runs a process ended");
        }
        named
    }

    /// Stops Claude Code's run in its card's process (stop_task), which records its end; with no process to ask, the run
    /// is stopped here. The stop is the owner's decision, recorded with who and why.
    pub async fn stop_claude_run(&self, mut run: Workflow, why: &str, by: &Actor) -> Result<()> {
        if run.status.ended() {
            return Err(Error::refused(format!(
                "workflow {} is {} already",
                run.name, run.status
            )));
        }
        let decision = Decided {
            kind: DecisionKind::Stop,
            text: format!("Stop workflow {}.", run.name),
            why: why.to_owned(),
            card_id: run.card.clone(),
            by: by.clone(),
        };
        let sess = self
            .run_of(&run.card)
            .filter(|r| r.open())
            .and_then(|r| r.state().sess.clone());
        run.steps = Vec::new();
        match (sess, run.claude.as_ref()) {
            (Some(sess), Some(claude)) => {
                let stop = tokio::time::timeout(STOP_WAIT, sess.stop_task(claude.task_id.clone())).await;
                match stop {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => return Err(Error::refused(format!("stop the workflow in its card's process: {e}"))),
                    Err(_) => {
                        return Err(Error::refused(
                            "stop the workflow in its card's process: it did not answer",
                        ));
                    }
                }
            }
            _ => (run.status, run.error, run.ended_at) = (WorkflowStatus::Stopped, why.to_owned(), Some(self.now())),
        }
        // its stop is written over it: the decision names the run's chain
        let w = Records {
            decisions: vec![decision],
            runs: vec![run],
            ..Records::default()
        };
        self.commit(None, None, w).await.map(|_| ())
    }
}
