//! The owner's seen marks (ADR-0079, seen.rs in chains): the browser reports a real view and the mark moves forward.

use std::sync::Arc;

use model::{SEEN_ASSISTANT_FEED, Seen};
use wire::app::SeenSubject;

use chains::request::{Done, Request, Service};
use chains::{Error, Result};

use crate::orch::Orchestrator;

impl Orchestrator {
    /// The owner viewed subject: its mark moves forward to now, and to the event he saw, never back. A home's views and
    /// the board are computed again (what is unseen moved); the feed is pushed again.
    pub async fn seen_mark(self: &Arc<Self>, req: Request, subject: SeenSubject) -> Result<Done> {
        self.act_done(req, "SeenMark", move |o| async move {
            let now = o.now();
            let (mark, home) = match subject {
                SeenSubject::Home { home, event_id } => {
                    if home.trim().is_empty() || event_id < 0 {
                        return Err(Error::refused("a seen mark names a home and one of its events"));
                    }
                    let s = Seen {
                        subject: home.clone(),
                        event_id,
                        at: now,
                    };
                    (s, Some(home))
                }
                SeenSubject::AssistantFeed => {
                    let s = Seen {
                        subject: SEEN_ASSISTANT_FEED.to_owned(),
                        event_id: 0,
                        at: now,
                    };
                    (s, None)
                }
            };
            let moved = o.store.update(move |tx| tx.put_seen(&mark)).await?;
            if !moved {
                return Ok(());
            }
            match home {
                Some(home) => o.views.marks.touch(&home, true),
                None => o.push_feed().await,
            }
            Ok(())
        })
        .await
    }
}
