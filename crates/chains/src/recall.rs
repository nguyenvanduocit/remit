//! What a session is given of its card's memory as it works (ADR-0117): a note on each turn's message, stored with it
//! before the turn sends it, and what Claude Code adds to the agent's context at a moment of its work (after a call of a
//! tool, as a subagent starts). The run keeps what its session was given (`RunState::recalled`), so nothing is given
//! twice: a resumed session starts with what its card was given before, and a compaction, which leaves the agent a
//! summary, forgets it. Recall never holds a turn up for long (`RECALL_WITHIN`), and a stop, a pause or a shutdown is
//! seen while it waits. Which memories bear on what is the domains' to say (`Domains::recall`, `Domains::recall_at`).

use std::sync::Arc;
use std::time::Duration;

use model::{Event, ForeignJson};
use tokio_util::sync::CancellationToken;
use wire::runtime::Moment;

use crate::chain_core::Core;
use crate::outbox::{note_of, read_of, with_note};
use crate::session::Run;

/// How long a turn's message or a moment of the agent's work waits for what its memory gives it: past it, the agent goes
/// on with nothing given. Recall asks a model twice (embed the query, judge the candidates), each well under a second
/// when OpenRouter is well.
pub const RECALL_WITHIN: Duration = Duration::from_millis(2500);

/// The key of a message's data that names the memories its note gave.
const RECALLED: &str = "recalled";

/// The ids of the memories a message's note gave, when it gave any: a message sent again carries them. Pure.
fn recalled_on(data: Option<&ForeignJson>) -> Option<Vec<String>> {
    let ids = data?.0.get(RECALLED)?.as_array()?;
    Some(ids.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
}

/// A message's data with note, what its card remembers of it, as the last part of its note (after a lead, which the
/// reader meets first), and ids, the memories it names. Pure.
fn with_recall(data: Option<&ForeignJson>, note: &str, ids: &[String]) -> ForeignJson {
    let before = note_of(data).unwrap_or_default();
    let joined = if before.is_empty() {
        note.to_owned()
    } else {
        format!("{before}\n\n{note}")
    };
    let mut out = with_note(data, &joined);
    if let serde_json::Value::Object(fields) = &mut out.0 {
        fields.insert(RECALLED.to_owned(), serde_json::json!(ids));
    }
    out
}

/// Resolves once r's mailbox is closed: the run was stopped or ended.
async fn closed(r: &Run) {
    while r.mailbox.wait().await {}
}

impl Core {
    /// Message e with what its card remembers of it: a note after any other on the message, which the agent reads first
    /// and the owner sees on the message in the stream, stored before the turn sends it, and its memories count as given once it is. e as it is when nothing bears on
    /// it, the card remembers nothing, the recall took longer than `RECALL_WITHIN`, or the note cannot be stored (logged):
    /// the turn goes on. A message sent again keeps the note it was given, and its memories count as given. None when the
    /// run closed or the server began to stop while it waited: no turn starts.
    pub(crate) async fn with_recall(&self, work: &CancellationToken, r: &Arc<Run>, e: Event) -> Option<Event> {
        if let Some(ids) = recalled_on(e.data.as_ref()) {
            r.state().recalled.extend(ids);
            return Some(e);
        }
        let (card, given) = {
            let s = r.state();
            (s.card.clone(), s.recalled.clone())
        };
        let Ok(domains) = self.domains() else { return Some(e) };
        let text = read_of(&e);
        let recall = tokio::select! {
            found = tokio::time::timeout(RECALL_WITHIN, domains.recall(&card, &text, &given)) => found,
            () = work.cancelled() => return None,
            () = closed(r) => return None,
        };
        let recall = match recall {
            Ok(Some(recall)) => recall,
            Ok(None) => return Some(e),
            Err(_) => {
                tracing::info!(card = %card.id, "memory: the turn goes on before its recall came");
                return Some(e);
            }
        };
        let mut next = e.clone();
        next.data = Some(with_recall(e.data.as_ref(), &recall.note, &recall.ids));
        match self.store.update(move |tx| tx.update_event(next)).await {
            Ok(stored) => {
                r.state().recalled.extend(recall.ids.iter().cloned());
                self.notify.event_updated(&stored);
                let ids = recall.ids;
                self.background(move |_| async move { domains.given(&card, &ids).await });
                Some(stored)
            }
            Err(err) => {
                tracing::warn!(card = %card.id, %err, "memory: the turn goes on without what it recalled");
                Some(e)
            }
        }
    }

    /// What r's agent reads beside moment m of its work, of what its card remembers, none of what its session was given:
    /// none when nothing bears on it or the recall took longer than `RECALL_WITHIN`. What it gives counts as given once
    /// the note is handed over.
    pub(crate) async fn told_at(&self, r: &Arc<Run>, m: Moment) -> Option<String> {
        let (card, given) = {
            let s = r.state();
            (s.card.clone(), s.recalled.clone())
        };
        let domains = self.domains().ok()?;
        let recall = tokio::time::timeout(RECALL_WITHIN, domains.recall_at(&card, &m, &given))
            .await
            .ok()??;
        r.state().recalled.extend(recall.ids.iter().cloned());
        let ids = recall.ids;
        self.background(move |_| async move { domains.given(&card, &ids).await });
        Some(recall.note)
    }

    /// Gives resumed run r what its card's sessions were given in its chain before, so its session is not given it again.
    /// Logged and left empty when it cannot be read: a memory may then be given twice.
    pub(crate) async fn seed_recalled(&self, r: &Run) {
        let card = r.card_id();
        match self.recalled_of(&r.home).await {
            Ok((recalled, _)) => {
                if let Some(c) = recalled.into_iter().find(|c| c.card == card) {
                    r.state().recalled.extend(c.memories);
                }
            }
            Err(err) => tracing::warn!(%card, %err, "memory: a resumed session starts with nothing given"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A message sent again (a retry, another account, a pause) carries the note it was given, after the lead it reads
    // first: its recall is not given a second time.
    #[test]
    fn a_message_sent_again_carries_what_it_was_given() {
        let data = ForeignJson(serde_json::json!({ "note": "[lead]" }));
        let ids = vec!["m1".to_owned(), "m2".to_owned()];
        let recalled = with_recall(Some(&data), "What you remember", &ids);
        assert_eq!(recalled.0["note"], "[lead]\n\nWhat you remember");
        assert_eq!(recalled_on(Some(&recalled)), Some(ids));
        assert_eq!(recalled_on(Some(&data)), None);
    }
}
