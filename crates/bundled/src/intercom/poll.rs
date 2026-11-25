//! What Remit hears from Intercom: a conversation that changed. The poll reads the conversations updated since its cursor,
//! oldest first, and an item is a conversation at a version, so each change is one event and a replay is the same one. The event
//! names the conversation and says what changed; the words of the customer stay in Intercom, to be read with
//! intercom_conversation.

use plugin_sdk::{Error, PollAnswer, PollItem};
use serde::{Deserialize, Serialize};
use serde_json::json;
use source::shorten;

use super::{CELL, Conversation, Intercom, STATES, filter, query_of, text, unix_rfc3339};

/// The most conversations one poll reads (Intercom's own page is at most 150).
const POLL_MAX: i64 = 150;

/// The owner's query of a poll: space-separated words, each `state=open|closed|snoozed` or `from=customer`.
#[derive(Debug, Default, PartialEq)]
pub(super) struct PollQuery {
    state: String,
    from_customer: bool,
}

impl PollQuery {
    pub(super) fn parse(q: &str) -> Result<PollQuery, String> {
        let mut out = PollQuery::default();
        for word in q.split_whitespace() {
            match word.split_once('=') {
                Some(("state", v)) if STATES.contains(&v) => v.clone_into(&mut out.state),
                Some(("from", "customer")) => out.from_customer = true,
                _ => {
                    return Err(format!(
                        "poll query {:?}: a word is state=open, state=closed, state=snoozed or from=customer",
                        shorten(word, 40)
                    ));
                }
            }
        }
        Ok(out)
    }
}

/// A conversation as an event: what changed and where to read it.
#[derive(Debug, Serialize)]
struct Event {
    id: String,
    state: String,
    subject: String,
    first_words: String,
    started_by: String,
    tags: Vec<String>,
    contact_ids: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    admin_assignee: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    team_assignee: String,
    parts: i64,
    updated_at: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    last_customer_reply: String,
}

fn event_of(c: &Conversation) -> Event {
    Event {
        id: c.id.clone(),
        state: c.state.clone(),
        subject: c.subject(),
        first_words: text(&c.source.message.body, CELL),
        started_by: c.source.delivered_as.clone(),
        tags: c.tags.tags.iter().map(|t| text(&t.name, 40)).collect(),
        contact_ids: c.contacts.contacts.iter().map(|ct| ct.id.clone()).collect(),
        admin_assignee: c.admin_assignee.clone(),
        team_assignee: c.team_assignee.clone(),
        parts: c.statistics.parts,
        updated_at: unix_rfc3339(c.updated_at),
        last_customer_reply: c
            .statistics
            .last_contact_reply
            .filter(|r| *r > 0)
            .map(unix_rfc3339)
            .unwrap_or_default(),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Found {
    #[serde(deserialize_with = "source::null_as_zero")]
    conversations: Vec<Conversation>,
}

impl Intercom {
    /// The plugin's poll. The cursor is the updated time, in Unix seconds, of the last conversation it read: a conversation read
    /// again at the same second is the same event (its id and version are the same), so none is lost at the edge. The first
    /// poll, with no cursor, answers no item and the time now, so a new signal never replays history.
    pub(super) async fn poll(&self, query: &str, cursor: &str, limit: i64) -> Result<PollAnswer, Error> {
        let pq = PollQuery::parse(query)?;
        if cursor.is_empty() {
            return Ok(PollAnswer {
                items: Vec::new(),
                cursor: self.now.as_second().to_string(),
            });
        }
        let since: i64 = cursor
            .parse()
            .map_err(|_| format!("poll cursor {:?} is not one this plugin gave", shorten(cursor, 40)))?;
        let mut filters = vec![filter("updated_at", ">", since.saturating_sub(1))];
        if !pq.state.is_empty() {
            filters.push(filter("state", "=", pq.state.as_str()));
        }
        let search = json!({"query": query_of(filters), "sort_field": "updated_at", "sort_order": "ascending",
            "pagination": {"per_page": limit.clamp(1, POLL_MAX)}});
        let raw = self.api().post_json("/conversations/search", &search).await?;
        let res: Found =
            serde_json::from_slice(&raw).map_err(|e| format!("intercom: the conversations are not JSON: {e}"))?;
        answer(&res.conversations, &pq, since)
    }
}

/// What the conversations read make of a poll: the cursor moves over every one read, the items are those a customer wrote in
/// since the cursor (for `from=customer`) or all of them, and the version of an item is the time of what changed.
fn answer(convs: &[Conversation], pq: &PollQuery, since: i64) -> Result<PollAnswer, Error> {
    let mut out = PollAnswer {
        items: Vec::new(),
        cursor: since.to_string(),
    };
    for c in convs {
        out.cursor = c.updated_at.max(since).to_string();
        let version = if pq.from_customer {
            match c.statistics.last_contact_reply {
                Some(r) if r >= since => r,
                _ => continue,
            }
        } else {
            c.updated_at
        };
        out.items.push(PollItem {
            id: c.id.clone(),
            version: version.to_string(),
            body: serde_json::to_value(event_of(c))?.into(),
        });
    }
    Ok(out)
}
