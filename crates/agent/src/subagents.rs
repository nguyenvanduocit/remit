//! Which handler hears each subagent of a session, and who spawned it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::Handler;

/// Finds the handler of each subagent of one session: the session's own agent's is the root, and a subagent's is the one
/// its spawner's handler gave (`Handler::on_subagent`), asked once. Who spawned a subagent is the owner of its task: the
/// agent whose handler heard the task start (`owns`). Both ends of a session use it: the Claude Code adapter, and the
/// core's end of the AI runtime, which hears the same tasks as entries.
pub struct Subagents {
    root: Arc<dyn Handler>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The agent that spawned each subagent; absent: the session's own agent.
    owner: HashMap<String, String>,
    /// Each subagent's handler, once asked; `None` when its spawner's handler gave none.
    subs: HashMap<String, Option<Arc<dyn Handler>>>,
}

impl Subagents {
    /// The subagents of the session whose own agent `root` handles.
    pub fn new(root: Arc<dyn Handler>) -> Self {
        Self {
            root,
            state: Mutex::new(State::default()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records that the agent `owner` (empty: the session's own) spawned the subagent `agent_id`.
    pub fn owns(&self, owner: &str, agent_id: &str) {
        if owner.is_empty() {
            return;
        }
        self.state().owner.insert(agent_id.to_owned(), owner.to_owned());
    }

    /// The handler of `agent_id` (empty: the session's own agent's); `None` when its spawner's handler gave none, and
    /// what it does is dropped.
    pub fn handler(&self, agent_id: &str) -> Option<Arc<dyn Handler>> {
        let mut state = self.state();
        self.handler_locked(&mut state, agent_id)
    }

    /// The handler a permission request of `agent_id` is asked of: its own, or else the nearest spawner's that has one,
    /// the session's own agent's at last.
    pub fn asker(&self, agent_id: &str) -> Arc<dyn Handler> {
        let mut state = self.state();
        let mut id = agent_id.to_owned();
        // Each step goes up one spawner; more steps than spawners known is a loop in what Claude Code said.
        for _ in 0..=state.owner.len() {
            if let Some(h) = self.handler_locked(&mut state, &id) {
                return h;
            }
            match state.owner.get(&id) {
                Some(owner) => id.clone_from(owner),
                None => break,
            }
        }
        Arc::clone(&self.root)
    }

    /// Asks a subagent's spawner for its handler the first time; the state is held, so each is asked once.
    fn handler_locked(&self, state: &mut State, agent_id: &str) -> Option<Arc<dyn Handler>> {
        if agent_id.is_empty() {
            return Some(Arc::clone(&self.root));
        }
        if let Some(h) = state.subs.get(agent_id) {
            return h.clone();
        }
        // Marked asked first, so a loop in what Claude Code said of the spawners ends here.
        state.subs.insert(agent_id.to_owned(), None);
        let owner = state.owner.get(agent_id).cloned().unwrap_or_default();
        let h = self
            .handler_locked(state, &owner)
            .and_then(|spawner| spawner.on_subagent(agent_id));
        state.subs.insert(agent_id.to_owned(), h.clone());
        h
    }
}
