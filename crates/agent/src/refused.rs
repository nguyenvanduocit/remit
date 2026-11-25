//! A call Claude Code's auto-mode classifier refused comes back as a permission request (ADR-0083). The refusal is final
//! in auto mode and reaches no one, so the session leaves auto mode, tells the model it may retry, and the retried call,
//! with no classifier to answer it, is asked over `can_use_tool` and goes up the permission ladder like any other. The
//! session goes back to its mode at the first permission request after that, or when the turn ends.

use serde_json::{Map, Value};

/// The mode a session waits in for a refused call to come back: a call no rule allows is asked over `can_use_tool`.
pub(crate) const ASKING_MODE: &str = "default";

/// A call the classifier refused, waiting to come back, and the mode the session left.
struct Refused {
    tool: String,
    input: Value,
    reason: String,
    mode: String,
}

/// What became of the open refusal when a permission request came.
pub(crate) struct Closed {
    /// The mode the session goes back to.
    pub mode: String,
    /// The classifier's reason, when the request is the refused call come back.
    pub classifier: Option<String>,
}

/// The refusals of a session's turn: at most one open, and the calls refused in the turn, each sent back once.
#[derive(Default)]
pub(crate) struct Refusals {
    open: Option<Refused>,
    seen: Vec<(String, Value)>,
}

impl Refusals {
    /// Opens a refusal of call (tool, input), which the session left mode for, unless one is open or the call was refused
    /// in the turn already: a call refused again after it came back stays refused, so the two never loop.
    pub(crate) fn open(&mut self, tool: &str, input: &Value, reason: &str, mode: &str) -> bool {
        let call = (tool.to_owned(), input.clone());
        if self.open.is_some() || self.seen.contains(&call) {
            return false;
        }
        self.seen.push(call);
        self.open = Some(Refused {
            tool: tool.to_owned(),
            input: input.clone(),
            reason: reason.to_owned(),
            mode: mode.to_owned(),
        });
        true
    }

    /// Drops the open refusal: the session could not leave its mode.
    pub(crate) fn abandon(&mut self) {
        self.open = None;
    }

    /// Closes the open refusal at a permission request for (tool, input), if one is open.
    pub(crate) fn close(&mut self, tool: &str, input: &Map<String, Value>) -> Option<Closed> {
        let r = self.open.take()?;
        let back = r.tool == tool && matches!(&r.input, Value::Object(m) if m == input);
        Some(Closed {
            mode: r.mode,
            classifier: back.then_some(r.reason),
        })
    }

    /// Ends the turn: forgets its calls, and gives the mode to go back to when a refusal is still open.
    pub(crate) fn end_turn(&mut self) -> Option<String> {
        self.seen.clear();
        self.open.take().map(|r| r.mode)
    }
}
