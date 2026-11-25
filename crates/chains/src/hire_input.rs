//! A hire as the Chair asks for one (hire_agent, chair_hire.rs in crew), and as a hand-over and a workflow's hire() call
//! ask for one: the core builds these, the crew does the hiring.

use crate::prompts::Given;
use crate::step::Decided;
use model::Agent;
use model::Card;
use model::Chain;
use model::Edge;
use model::Employee;
use schemars::JsonSchema;
use serde::Deserialize;

/// A hire as the Chair asks for it (hire_agent), and as a hand-over and a workflow's hire() call ask for one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HireInput {
    /// the id of the agent to hire, one of those this tool's description lists, or a short word for a plain agent (every
    /// tool, nothing from the library), e.g. research, architect, critic, engineer, security, pm, release; leave it out with
    /// employee
    pub agent: String,
    /// the id of a kept employee to put to work instead of hiring a new one, from this tool's description or list_agents'
    /// standing; leave agent out
    pub employee: String,
    /// with employee only: worker (the default), reviewer, qc or security (judges a change that touches a rail the
    /// repository declares in .remit/rails); a reviewer, QC or security reviewer lists the cards whose work it judges in
    /// inputs
    pub role: String,
    /// display name, e.g. 'Engineer · web-app'
    pub name: String,
    /// the agent's whole task: goal, context with paths, what to deliver, how you will judge it
    pub brief: String,
    /// one or two short sentences for the owner, in the owner's language, about the work this agent adds. Reuse a kept
    /// worker first (this tool's description lists them): a new hire while you keep an idle worker of any agent (one with
    /// no card open) is refused unless why names that worker by its id and says why its strengths do not fit. Do not quote
    /// the owner.
    pub why: String,
    /// other agents' reports this agent builds on; each becomes a handoff edge and its full report is added to the brief
    pub inputs: Vec<HireInputRef>,
    /// a model id from this tool's description, only when you know better; empty lets Remit choose from the brief. A
    /// reviewer, QC or security card runs on the judges' model, the strongest, whatever you name
    pub model: String,
    /// how long the model thinks before it acts: low, medium, high, xhigh or max, only when you know better; empty keeps
    /// the agent's, else yours
    pub effort: String,
    /// absolute working directory; empty uses the chain's directory. An agent that changes, reviews or verifies code gets
    /// the path open_repo returned for its repository
    pub cwd: String,
    /// optional: the steps you expect this agent to take, in order, each a few words, when you already see the path;
    /// without it the agent writes its own. Either way it keeps its plan true as it works
    pub plan: Vec<String>,
    /// exactly what the agent hands over when done, e.g. 'a commit on the worktree fixing X with a failing-then-passing
    /// test' or 'a table of Y with sources'
    pub deliverable: String,
    /// acceptance criteria: each a concrete, checkable condition you will judge the work against, e.g. 'go test
    /// ./overview/... passes', 'every handler listed returns the error'
    pub criteria: Vec<String>,
    /// the Claude Code tools this agent may use, instead of its agent's: leave out to keep them, [] for none; names as
    /// list_agent_definitions shows them, e.g. Read, Grep, Bash. A plain agent left without it has every tool
    pub tools: Option<Vec<String>>,
    /// the library skills this agent holds, instead of its agent's (list_library shows them): leave out to keep them, []
    /// for none
    pub skills: Option<Vec<String>>,
}

impl HireInput {
    /// The model its Chair or its agent definition names, the Chair's first; empty when neither does. Named, a model is
    /// theirs: no decider chooses it and the climb never moves it (ADR-0118). The one place that precedence is written.
    pub fn named_model<'a>(&'a self, def: &'a Agent) -> &'a str {
        if self.model.is_empty() { &def.model } else { &self.model }
    }

    /// The effort its Chair or its agent definition names, the Chair's first; empty when neither does.
    pub fn named_effort<'a>(&'a self, def: &'a Agent) -> &'a str {
        if self.effort.is_empty() {
            &def.effort
        } else {
            &self.effort
        }
    }
}

/// A report handed to a new agent: whose, and what it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HireInputRef {
    /// the card whose report it is
    pub card_id: String,
    /// what is handed over, in a few words, e.g. 'list of untested functions'
    pub label: String,
}

/// A hire as it is stored: the new card, its employee when fresh, the reports handed to it with their handoff edges, the
/// Chair's decision, and the chain and the decisions people made for the chain (`chain_decided`) its brief is written
/// for.
#[derive(Debug, Clone, Default)]
pub struct Hiring {
    pub card: Card,
    pub employees: Vec<Employee>,
    pub reports: Vec<Given>,
    pub edges: Vec<Edge>,
    pub decision: Decided,
    pub chain: Chain,
    pub decided: String,
}
