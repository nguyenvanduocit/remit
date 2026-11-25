use std::collections::HashMap;

use model::TaskStatus;
use serde::Deserialize;
use wire::runtime::{TASK_SUBAGENT, TASK_WORKFLOW, Task, WorkflowAgent};

use crate::agent::SdkMessage;

/// One entry of a workflow run's progress, in the SDK's fields: a phase or an agent.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct WorkflowEntry {
    /// workflow_phase or workflow_agent.
    r#type: String,
    index: i64,
    label: String,
    phase_title: String,
    agent_id: String,
    agent_type: String,
    state: String,
    prompt_preview: String,
    result_preview: String,
    error: String,
}

/// The answer of a Workflow tool call that launched a run (its `tool_use_result`).
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct WorkflowLaunch {
    pub status: String,
    pub task_id: String,
    pub task_type: String,
    pub run_id: String,
    pub script_path: String,
    pub transcript_dir: String,
}

/// A task and the subagent whose task it is: empty for the session's own agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedTask {
    pub owner: String,
    pub task: Task,
}

/// Follows the tasks of one agent process from the raw SDK messages Claude Code sends of them. Which background tasks run
/// is the level (background_tasks_changed, the whole set after each change, which Claude Code says to trust over pairing
/// the edges); task_started and task_notification only say more of those: the tool call that started a task, and how it
/// ended. The order of the level and the edges of one change is not promised, so a task the level drops before its
/// notification ends stopped, and the notification then says how it ended. A subagent or a workflow run is not the
/// level's: it is live from its task_started to its task_notification. The log is per process: a new process starts with
/// an empty one.
#[derive(Debug, Default)]
pub(crate) struct TaskLog {
    /// In the order they started.
    live: Vec<Task>,
    /// The tool call that started each task, from task_started.
    tool_call: HashMap<String, String>,
    /// The subagent whose task each is, from task_started; absent for the session's own.
    owner: HashMap<String, String>,
    /// As each ended: a level that still lists one does not start it again.
    ended: HashMap<String, Task>,
}

/// How a task ended, as its task_notification's status says: completed, failed or stopped (Claude Code's three). A word
/// it does not document reads as stopped, the end that says least.
fn ended_as(status: &str) -> TaskStatus {
    match status.parse() {
        Ok(s @ (TaskStatus::Completed | TaskStatus::Failed)) => s,
        _ => TaskStatus::Stopped,
    }
}

/// Says whether a task of `kind` is live from its start to its notification, outside the level.
fn spawned(kind: &str) -> bool {
    kind == TASK_SUBAGENT || kind == TASK_WORKFLOW
}

impl TaskLog {
    /// Takes one SDK message and returns the tasks it changed, as they are now.
    pub(crate) fn read(&mut self, m: &SdkMessage) -> Vec<OwnedTask> {
        match m.subtype.as_str() {
            "task_started" => self.started(m),
            "task_progress" => {
                let Some(progress) = &m.workflow_progress else {
                    return Vec::new();
                };
                let Some(i) = self.index(&m.task_id) else {
                    return Vec::new();
                };
                let agents = workflow_agents(progress);
                let Some(t) = self.live.get_mut(i) else {
                    return Vec::new();
                };
                if t.agents == agents {
                    return Vec::new();
                }
                t.agents = agents;
                let t = t.clone();
                self.owned(t)
            }
            "task_notification" => {
                let mut t = match self.index(&m.task_id) {
                    Some(i) => self.live.remove(i),
                    // Foreground work, which never ran in the background, is not in either.
                    None => match self.ended.get(&m.task_id) {
                        Some(t) => t.clone(),
                        None => return Vec::new(),
                    },
                };
                t.status = ended_as(&m.status);
                t.summary.clone_from(&m.summary);
                t.output_file.clone_from(&m.output_file);
                if !m.tool_use_id.is_empty() {
                    t.tool_call_id.clone_from(&m.tool_use_id);
                }
                self.ended.insert(t.id.clone(), t.clone());
                self.owned(t)
            }
            "background_tasks_changed" => self.level(m),
            _ => Vec::new(),
        }
    }

    /// A task_started: a subagent or a workflow run starts; any other task only learns the tool call that started it.
    fn started(&mut self, m: &SdkMessage) -> Vec<OwnedTask> {
        if m.ambient || (m.tool_use_id.is_empty() && !spawned(&m.task_type)) {
            return Vec::new();
        }
        self.tool_call.insert(m.task_id.clone(), m.tool_use_id.clone());
        if !m.parent_task_id.is_empty() {
            self.owner.insert(m.task_id.clone(), m.parent_task_id.clone());
        }
        match self.index(&m.task_id) {
            None if spawned(&m.task_type) => {
                if self.ended.contains_key(&m.task_id) {
                    return Vec::new();
                }
                let t = Task {
                    id: m.task_id.clone(),
                    kind: m.task_type.clone(),
                    title: m.description.clone(),
                    tool_call_id: m.tool_use_id.clone(),
                    status: TaskStatus::Running,
                    agent_type: m.subagent_type.clone(),
                    prompt: m.prompt.clone(),
                    depth: m.spawn_depth,
                    background: m.is_backgrounded,
                    name: m.workflow_name.clone(),
                    ..Task::default()
                };
                self.live.push(t.clone());
                self.owned(t)
            }
            Some(i) => {
                let Some(t) = self.live.get_mut(i) else {
                    return Vec::new();
                };
                if t.tool_call_id == m.tool_use_id {
                    return Vec::new();
                }
                t.tool_call_id.clone_from(&m.tool_use_id);
                let t = t.clone();
                self.owned(t)
            }
            None => Vec::new(),
        }
    }

    /// A background_tasks_changed: the background tasks that run now. A subagent or a workflow run stays live whatever
    /// the level says.
    fn level(&mut self, m: &SdkMessage) -> Vec<OwnedTask> {
        let (mut live, mut changed) = (Vec::new(), Vec::new());
        for e in &m.tasks {
            if e.ambient || self.ended.contains_key(&e.task_id) {
                continue;
            }
            if let Some(t) = self.live.iter().find(|t| t.id == e.task_id) {
                live.push(t.clone());
                continue;
            }
            if spawned(&e.task_type) {
                continue; // live from its task_started, which has not come yet
            }
            let t = Task {
                id: e.task_id.clone(),
                kind: e.task_type.clone(),
                title: e.description.clone(),
                tool_call_id: self.tool_call.get(&e.task_id).cloned().unwrap_or_default(),
                status: TaskStatus::Running,
                ..Task::default()
            };
            live.push(t.clone());
            changed.push(t);
        }
        for mut t in std::mem::take(&mut self.live) {
            if live.iter().any(|n| n.id == t.id) {
                continue;
            }
            if spawned(&t.kind) {
                live.push(t);
                continue;
            }
            t.status = TaskStatus::Stopped;
            self.ended.insert(t.id.clone(), t.clone());
            changed.push(t);
        }
        self.live = live;
        changed.into_iter().flat_map(|t| self.owned(t)).collect()
    }

    /// Takes the answer of the Workflow call that launched a run, and returns the run with the handle that resumes it.
    pub(crate) fn launched(&mut self, w: &WorkflowLaunch) -> Vec<OwnedTask> {
        if w.status != "async_launched" || w.task_type != TASK_WORKFLOW {
            return Vec::new();
        }
        let Some(t) = self.live.iter_mut().find(|t| t.id == w.task_id) else {
            return Vec::new();
        };
        for (field, said) in [
            (&mut t.run_id, &w.run_id),
            (&mut t.script_path, &w.script_path),
            (&mut t.transcript_dir, &w.transcript_dir),
        ] {
            if !said.is_empty() {
                field.clone_from(said);
            }
        }
        let t = t.clone();
        self.owned(t)
    }

    fn owned(&self, task: Task) -> Vec<OwnedTask> {
        let owner = self.owner.get(&task.id).cloned().unwrap_or_default();
        vec![OwnedTask { owner, task }]
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.live.iter().position(|t| t.id == id)
    }
}

/// The agents of a workflow run's progress, in the order it lists them.
fn workflow_agents(entries: &[WorkflowEntry]) -> Vec<WorkflowAgent> {
    entries
        .iter()
        .filter(|e| e.r#type == "workflow_agent")
        .map(|e| WorkflowAgent {
            index: e.index,
            label: e.label.clone(),
            phase: e.phase_title.clone(),
            agent_id: e.agent_id.clone(),
            agent_type: e.agent_type.clone(),
            state: e.state.clone(),
            prompt: e.prompt_preview.clone(),
            result: e.result_preview.clone(),
            error: e.error.clone(),
        })
        .collect()
}
