//! Every agent works to a plan the owner can watch: the Chair's plan is the chain's workflow, each agent's plan the steps
//! of its brief. Agents do not keep one on their own, so it is part of the protocol: the Chair writes the chain's plan
//! before it acts, gives each hire its expected steps, and every agent keeps its plan true with update_plan as steps
//! start, end or change. A worker's turn does not end before its plan is true (turn_end.rs's `owed_before_end`).

use model::{Card, Origin, PlanEntry};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{Error, Result};

/// One step of the plan an agent writes with update_plan.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanStep {
    /// The step, a few words.
    #[serde(default)]
    pub content: String,
    /// pending, in_progress or completed.
    #[serde(default)]
    pub status: String,
}

/// The whole plan, in order: update_plan's input.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanInput {
    /// The steps, in order.
    #[serde(default)]
    pub steps: Vec<PlanStep>,
}

impl PlanInput {
    /// The plan the input writes, each step checked.
    pub fn entries(&self) -> Result<Vec<PlanEntry>> {
        if self.steps.is_empty() {
            return Err(Error::refused("a plan has at least one step"));
        }
        self.steps
            .iter()
            .map(|st| {
                let status = match st.status.trim() {
                    "" => "pending",
                    s @ ("pending" | "in_progress" | "completed") => s,
                    _ => {
                        return Err(Error::refused(format!(
                            "step {:?}: status is pending, in_progress or completed",
                            st.content
                        )));
                    }
                };
                if st.content.trim().is_empty() {
                    return Err(Error::refused("every step needs content"));
                }
                Ok(entry(st.content.trim(), status))
            })
            .collect()
    }
}

fn entry(content: &str, status: &str) -> PlanEntry {
    PlanEntry {
        content: content.to_owned(),
        status: status.to_owned(),
        priority: "medium".to_owned(),
    }
}

/// The plan a hire starts with: the Chair's expected steps, all pending.
pub fn plan_of(steps: &[String]) -> Vec<PlanEntry> {
    steps
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| entry(s, "pending"))
        .collect()
}

/// Stops the Chair from acting before it has written the chain's plan.
pub fn need_plan(c: &Card) -> Result<()> {
    if c.origin == Origin::Chair && c.plan.is_empty() {
        return Err(Error::refused(
            "write the chain's plan with update_plan first: the steps from here to the result. The owner follows the chain through it",
        ));
    }
    Ok(())
}

/// Whether a plan still has steps not completed.
pub fn plan_open(plan: &[PlanEntry]) -> bool {
    plan.iter().any(|p| p.status != "completed")
}

/// The plan with its last step completed when it is the only one left open: finishing the chain is that step.
pub fn finish_plan(plan: &[PlanEntry]) -> Vec<PlanEntry> {
    let Some((last, before)) = plan.split_last() else {
        return Vec::new();
    };
    if last.status == "completed" || before.iter().any(|p| p.status != "completed") {
        return plan.to_vec();
    }
    let mut out = plan.to_vec();
    if let Some(last) = out.last_mut() {
        "completed".clone_into(&mut last.status);
    }
    out
}

/// An agent's plan as the Chair reads it under the agent's report: what is done and what is left.
pub fn plan_text(plan: &[PlanEntry]) -> String {
    if plan.is_empty() {
        return String::new();
    }
    let done = plan.iter().filter(|p| p.status == "completed").count();
    let mut b = String::new();
    for p in plan {
        b.push_str(&format!("\n- [{}] {}", p.status, p.content));
    }
    format!("\n\n[its plan: {done}/{} done]{b}", plan.len())
}

/// The plan's steps not completed, leaving out the last one when it is the finish itself.
pub fn open_steps(plan: &[PlanEntry], finishing: bool) -> Vec<String> {
    let last = plan.len().saturating_sub(1);
    plan.iter()
        .enumerate()
        .filter(|(i, p)| p.status != "completed" && !(finishing && *i == last))
        .map(|(_, p)| p.content.clone())
        .collect()
}
