//! One row per turn of an agent's work: what the turn touched (files, commands, sources, tools), each told once with how
//! many times it was touched, and its failures and refusals kept apart, since a fold must never hide them. Computed
//! once, as the turn ends, from the turn's events and stored with its end (`Activity::TurnEnd`), so every channel shows
//! the same row and none folds the stream itself. Pure: `turn_of`. At the edge: `turn_data`.

use std::time::Duration;

use model::Event;
use wire::app::{Activity, FailureKind, ToolState, Touched, TurnFailure, TurnSummary};

use crate::activity::activity_of;
use crate::chain_core::Core;

/// Events of one turn `turn_data` reads at most; a turn with more is summed up from these, and says so (`partial`).
const TURN_EVENTS_READ: i64 = 5000;

/// The row of a turn whose events, after the message it took, are events, oldest first. The messages it sent other
/// agents are steps of their own, not things it touched.
pub fn turn_of(events: &[Event]) -> TurnSummary {
    let mut t = TurnSummary::default();
    for e in events {
        match activity_of(e) {
            Some(Activity::Tool {
                status: ToolState::Failed,
                name,
                summary,
                ..
            }) => t.failures.push(TurnFailure {
                event_id: e.id,
                kind: FailureKind::Failed,
                name,
                summary,
            }),
            Some(Activity::Tool {
                group, name, summary, ..
            }) => match t
                .touched
                .iter_mut()
                .find(|x| x.group == group && x.name == name && x.summary == summary)
            {
                Some(x) => x.count = x.count.saturating_add(1),
                None => t.touched.push(Touched {
                    group,
                    name,
                    summary,
                    count: 1,
                }),
            },
            Some(Activity::Decided {
                allowed: false,
                request,
                ..
            }) => t.failures.push(TurnFailure {
                event_id: e.id,
                kind: FailureKind::Refused,
                name: request,
                summary: String::new(),
            }),
            _ => {}
        }
    }
    t
}

impl Core {
    /// The data a turn's end carries: how long the turn on message p took, and its row (`turn_of`) from the events that
    /// came after p. A read that fails is logged and leaves the row out: the turn's end is stored all the same.
    pub async fn turn_data(&self, p: &Event, took: Duration) -> Option<model::ForeignJson> {
        let (card, after) = (p.card_id.clone(), p.id);
        let read = self
            .store
            .read(move |r| r.card_events_after(&card, after, TURN_EVENTS_READ.saturating_add(1)))
            .await;
        let ms = u64::try_from(took.as_millis()).unwrap_or(u64::MAX);
        let turn = match read {
            Ok(mut events) => {
                let partial = i64::try_from(events.len()).unwrap_or(i64::MAX) > TURN_EVENTS_READ;
                events.truncate(usize::try_from(TURN_EVENTS_READ).unwrap_or(usize::MAX));
                let mut row = turn_of(&events);
                row.partial = partial;
                serde_json::to_value(row)
                    .inspect_err(|err| tracing::warn!(card = %p.card_id, %err, "turn row not written"))
                    .ok()
            }
            Err(err) => {
                tracing::warn!(card = %p.card_id, %err, "turn's events not read: its row is left out");
                None
            }
        };
        let data = match turn {
            Some(turn) => serde_json::json!({ "durationMs": ms, "turn": turn }),
            None => serde_json::json!({ "durationMs": ms }),
        };
        Some(model::ForeignJson(data))
    }
}
