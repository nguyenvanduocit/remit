//! The Chair's workflow tools (ADR-0073, decision 4): run_workflow starts a script Remit runs durably, list_workflows reads
//! each run of the chain with its steps, stop_workflow ends one. Each calls the action the shell gives every caller:
//! `start_workflow`, which records the start with the Chair's why, and `stop_workflow_as`, the owner's stop with the Chair
//! as who stopped it.

use std::sync::Arc;

use std::collections::HashMap;

use model::{Card, ForeignJson, StepStatus, Workflow, WorkflowEngine, WorkflowStatus, WorkflowVerb};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::chain_core::Core;
use crate::chain_core::first_line;
use crate::plan::need_plan;
use crate::tools::{Effect, NoInput, SelfCard, Tools, as_json};
use crate::workflow_run::WorkflowInput;

/// What stop_workflow takes.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkflowStopInput {
    /// The run's id (w_…), as run_workflow or list_workflows names it.
    pub id: String,
    /// One line the owner reads in the chain's history: why the run stops.
    pub why: String,
}

/// A run as list_workflows shows it to the Chair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkflowView {
    pub id: String,
    pub name: String,
    pub engine: WorkflowEngine,
    pub status: WorkflowStatus,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ForeignJson>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resumed_from: String,
    pub steps: Vec<WorkflowStepSummary>,
}

/// One step of a run as list_workflows shows it: what the call asked, where it stands, the card it put to work and the start
/// of its result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkflowStepSummary {
    pub seq: i64,
    pub verb: WorkflowVerb,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub phase: String,
    pub status: StepStatus,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub card: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub card_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
}

/// run as the Chair reads it; names are the chain's cards' names by id.
pub(crate) fn workflow_view_of(run: &Workflow, names: &HashMap<String, String>) -> WorkflowView {
    let steps = run
        .steps
        .iter()
        .map(|s| WorkflowStepSummary {
            seq: s.seq,
            verb: s.verb,
            label: s.label.clone(),
            phase: s.phase.clone(),
            status: s.status,
            card: s.card.clone(),
            card_name: names.get(&s.card).cloned().unwrap_or_default(),
            result: first_line(&s.result.as_ref().map(ForeignJson::text).unwrap_or_default(), 300),
            error: s.error.clone(),
        })
        .collect();
    WorkflowView {
        id: run.id.clone(),
        name: run.name.clone(),
        engine: run.engine,
        status: run.status,
        phase: run.phase.clone(),
        result: run.result.clone(),
        error: run.error.clone(),
        resumed_from: run.resumed_from.clone(),
        steps,
    }
}

/// What run_workflow answers of the run it started.
fn started_text(run: &Workflow) -> String {
    let mut b = format!("Started workflow {} ({}): {}.", run.name, run.id, run.status);
    if run.status.ended() {
        let result = run.result.as_ref().map(ForeignJson::text).unwrap_or_default();
        b.push_str(&format!(
            " It ended at once: {}",
            format!("{result} {}", run.error).trim()
        ));
    } else {
        b.push_str(&format!(
            " Its end reaches you as a message starting with \"[workflow {}\"; list_workflows shows its steps.",
            run.name
        ));
    }
    b
}

const RUN_WORKFLOW: &str = "Run a script that drives the work for you, when its shape is known before it starts: a fan-out to several agents, verify each result, a loop of fix and verify until a check passes, a pipeline, a wait for a time, a signal or the owner between steps. Remit runs it durably (through waits and restarts) and you take no turn while it works; its end reaches you as a message starting with \"[workflow <name>\" with its result. The script is plain JavaScript beginning with export const meta = {name, description}, then a body that awaits agent(), parallel(), pipeline(), hire(), wait(), askOwner(), accept() and sendBack() and returns the result. Each agent() puts an employee to work (agentType names one; none: an idle kept worker, else a new general-purpose one) and returns its report, accepted; give a schema to get a checked JSON value. Date.now, Math.random and timers do not exist: the script is replayed from the start against its recorded results. Read the workflows skill for the API and patterns. Refused until the chain has a plan (update_plan). resume_from reuses an earlier run's results up to the first call the new script makes differently. Go step by step instead, with hire_agent, when each next step needs your judgement of the last report.";

const LIST_WORKFLOWS: &str = "Every workflow run of this chain, yours (engine remit) and those your agents ran in their own turns with Claude Code's Workflow (engine claude): its status, phase, result or error, and each step: the call it made, its label and phase, where it stands, the card it put to work and the start of its result. read_output gives a card's whole report.";

const STOP_WORKFLOW: &str = "Stop a workflow run of this chain that has not ended: its cards at work are stopped, its waits end, its question to the owner is withdrawn, and the run is stopped with your why. finish_chain is refused while a run is open, so stop one whose result you no longer need.";

impl Core {
    /// Gives the Chair run_workflow, list_workflows and stop_workflow.
    pub fn add_workflow_tools(self: &Arc<Self>, t: &mut Tools, card: &Card, me: &SelfCard) {
        let m = me.clone();
        t.serving(self).add(
            "run_workflow",
            Effect::Writes,
            RUN_WORKFLOW,
            move |o, i: WorkflowInput| {
                let m = m.clone();
                async move {
                    let me = m.read(&o).await?;
                    need_plan(&me)?;
                    let run = o.start_workflow(&me, i).await?;
                    Ok(started_text(&run))
                }
            },
        );
        let home = card.home.clone();
        t.serving(self)
            .add("list_workflows", Effect::Reads, LIST_WORKFLOWS, move |o, _: NoInput| {
                let home = home.clone();
                async move {
                    let (h, k) = (home.clone(), home.clone());
                    let runs = o.store.read(move |r| r.workflows_of(&h)).await?;
                    let cards = o.store.read(move |r| r.cards(&k)).await?;
                    let names: HashMap<String, String> = cards.into_iter().map(|c| (c.id, c.name)).collect();
                    let views: Vec<WorkflowView> = runs.iter().map(|r| workflow_view_of(r, &names)).collect();
                    as_json(&views)
                }
            });
        let (home, chair) = (card.home.clone(), card.id.clone());
        t.serving(self).add(
            "stop_workflow",
            Effect::Writes,
            STOP_WORKFLOW,
            move |o, i: WorkflowStopInput| {
                let (home, chair) = (home.clone(), chair.clone());
                async move {
                    if i.why.trim().is_empty() {
                        return Err(Error::refused("why is required: one line the owner reads"));
                    }
                    let id = i.id.clone();
                    let run = match o.store.read(move |r| r.workflow(&id)).await {
                        Ok(run) if run.home == home => run,
                        _ => return Err(Error::refused(format!("{} is no workflow run of this chain", i.id))),
                    };
                    o.stop_workflow_as(&run.id, &i.why, &model::by_agent(&chair)).await?;
                    Ok(format!("Stopped workflow {}.", run.name))
                }
            },
        );
    }
}
