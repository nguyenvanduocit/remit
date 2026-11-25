//! What a card's session was given and holds, as its session reports it: its opening message, its init and its context
//! window. What the owner reads of it, with the tools it was given, is the application's (cardcontext.rs in orch).

use std::sync::Arc;

use model::{ContextCount, InitReport, SessionContext, SessionInit};

use crate::chain_core::Core;
use crate::session::Run;

impl Core {
    /// Records text, sent as the first message of card's session `session_id`, as the session's opening; a session
    /// whose start was not recorded, or whose opening was, is left as it is.
    pub async fn record_opening(&self, card: &str, session_id: &str, text: String) {
        let (id, session) = (card.to_owned(), session_id.to_owned());
        let done = self
            .store
            .update(move |tx| match tx.session_launch(&id) {
                Ok(mut l) if l.session_id == session && l.given.opening.is_none() => {
                    l.given.opening = Some(text);
                    tx.put_session_launch(&l)
                }
                Ok(_) | Err(store::Error::NotFound) => Ok(()),
                Err(err) => Err(err),
            })
            .await;
        if let Err(err) = done {
            tracing::error!(%card, %err, "record the session's opening");
        }
    }
    /// Keeps what Claude Code says run r's session loaded at the start of this turn, over what it said before.
    pub async fn on_init(&self, r: &Arc<Run>, init: SessionInit) {
        let c = r.card_now();
        let report = InitReport { init, at: self.now() };
        let (card, home) = (c.id.clone(), c.home.clone());
        if let Err(err) = self
            .store
            .update(move |tx| tx.put_session_init(&c.id, &c.home, &report))
            .await
        {
            tracing::error!(%home, %card, %err, "session init");
        }
    }
    /// Keeps what fills run r's session's context, counted as it went to rest, over the count before.
    pub async fn on_context(&self, r: &Arc<Run>, context: SessionContext) {
        let c = r.card_now();
        let count = ContextCount {
            context,
            at: self.now(),
        };
        let (card, home) = (c.id.clone(), c.home.clone());
        if let Err(err) = self
            .store
            .update(move |tx| tx.put_session_context(&c.id, &c.home, &count))
            .await
        {
            tracing::error!(%home, %card, %err, "session context");
        }
    }
}
