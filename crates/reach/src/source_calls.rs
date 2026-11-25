//! The account of the calls agents make to the outside world (ADR-0018, invariant 3): the gate tells it once per call,
//! whatever the outcome, and it writes one row of source_calls. The row holds what the input was without its secrets (a
//! program's argv; a script's language, size and hash, never the script) and no value of a connection. It is in the
//! database, which no agent session can read: a log file of the same OS user could be read, and edited out, by the agent it
//! accounts for.

use model::SourceCall;
use source::{BoxFuture, Call};

use crate::sources::Bound;

/// Keeps the account of calls for the gate (`source::Recorder`).
pub(crate) struct CallAccount(pub(crate) Bound);

impl source::Recorder for CallAccount {
    /// Keeps c. A call that cannot be kept is logged: the gate cannot refuse a call for it, but the owner reads the log.
    fn record(&self, c: Call) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let o = match self.0.get() {
                Ok(o) => o,
                Err(err) => {
                    tracing::error!(source = %c.source, tool = %c.tool, %err, "a source call was not accounted for");
                    return;
                }
            };
            let row = SourceCall {
                chain: c.caller.chain.clone(),
                agent: c.caller.agent.clone(),
                source: c.source.clone(),
                connection: c.connection.clone(),
                tool: c.tool.clone(),
                // none: the call failed before its tool was found
                effect: c.effect.map(|e| e.as_str().to_owned()).unwrap_or_default(),
                form: c.form.as_str().to_owned(),
                detail: c.detail.clone(),
                outcome: c.outcome.as_str().to_owned(),
                refusal: c.refusal.map(|r| r.as_str().to_owned()).unwrap_or_default(),
                at: o.now(),
            };
            if let Err(err) = o.store.update(move |tx| tx.add_source_call(&row)).await {
                tracing::error!(chain = %c.caller.chain, source = %c.source, tool = %c.tool, outcome = %c.outcome.as_str(), %err, "a source call was not accounted for");
            }
        })
    }
}
