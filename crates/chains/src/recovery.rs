//! A card's session that the agent no longer has (Claude Code has no conversation of its id) is replaced by a new one,
//! which reads the card's whole brief again with a note that its context was lost. The replacement is recorded as a
//! system event (status new_session, carrying the session, the prompt it opens and the lead) in the transaction that
//! stores the prompt the agent takes, and stays pending until the turn that read the lead has its stop recorded: a restart
//! before then recovers again rather than resuming a session that never got its brief.

use std::sync::Arc;

use model::{Card, Event, EventStatus, Origin, SystemNote};
use serde::{Deserialize, Serialize};

use crate::chain_core::Core;
use crate::session::Run;
use crate::{Error, Result};

/// How the lead of a recovered session begins: what marks a recovery's lead, and replaces an earlier one on a message
/// sent again.
pub const LOST_CONTEXT_MARKER: &str = "[Remit: your earlier context was lost";

/// What a new_session event carries: the session that replaced the lost one, the prompt it opens with and the lead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionRecovery {
    pub session_id: String,
    pub prompt_id: i64,
    pub lead: String,
}

/// The recovery event e records; refused when it is not one whole.
pub fn recovery_of(e: &Event) -> Result<SessionRecovery> {
    let data = e.data.as_ref().map_or(serde_json::Value::Null, |d| d.0.clone());
    let p: SessionRecovery = serde_json::from_value::<Option<SessionRecovery>>(data)
        .map_err(|err| Error::refused(format!("read session recovery {}: {err}", e.id)))?
        .unwrap_or_default();
    if p.session_id.is_empty() || p.prompt_id <= 0 || !p.lead.starts_with(LOST_CONTEXT_MARKER) {
        return Err(Error::refused(format!("incomplete session recovery {}", e.id)));
    }
    Ok(p)
}

impl Core {
    /// The brief a recovered session of c reads again: the Assistant's lead for its card, else the first message the card
    /// was sent.
    pub async fn recovery_brief(&self, c: &Card) -> Result<String> {
        if c.origin == Origin::Assistant {
            return self.assistant_lead_for(c).await;
        }
        let id = c.id.clone();
        Ok(self.store.read(move |r| r.opening_prompt(&id)).await?.text)
    }

    /// Stores the stop that ends a turn on prompt; when the turn read a recovery's lead, the recovery is completed in the
    /// same transaction.
    pub async fn record_turn_stop(&self, r: &Arc<Run>, prompt: &Event, mut stop: Event) -> Result<()> {
        let (recovery, session) = {
            let s = r.state();
            (
                s.recovery.clone(),
                s.sess.as_ref().map(|s| s.id().to_owned()).unwrap_or_default(),
            )
        };
        let Some(recovery) = recovery.filter(|e| e.id != 0) else {
            self.add_event(stop).await?;
            return Ok(());
        };
        let now = self.now();
        (stop.created_at, stop.updated_at) = (now, now);
        let (rid, card, prompt_id) = (recovery.id, prompt.card_id.clone(), prompt.id);
        let (restored, stop) = self
            .store
            .update(move |tx| {
                let mut e = tx.event(rid)?;
                let p = recovery_of(&e)?;
                let mut restored = None;
                if e.status == EventStatus::System(SystemNote::NewSession)
                    && e.card_id == card
                    && p.session_id == session
                    && p.prompt_id == prompt_id
                {
                    e.status = EventStatus::System(SystemNote::ContextRestored);
                    e.updated_at = now;
                    restored = Some(tx.update_event(e)?);
                }
                Ok::<_, Error>((restored, tx.add_event(stop)?))
            })
            .await?;
        if let Some(restored) = restored {
            {
                let mut s = r.state();
                s.recovery = None;
                s.lead.clear();
            }
            self.notify.event_updated(&restored);
        }
        self.notify.event_added(&stop);
        Ok(())
    }
}
