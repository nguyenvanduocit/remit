//! The owner's words in a message to an agent (ADR-0107, *Typed values*): the prompt event stores what he wrote, marked
//! in its data as his (`ownerWords`), and the line that tells the agent who speaks ("[message from the owner]") is
//! written when the agent reads it. A message Remit words for him (an answer to a question, an exception sent back) is
//! not marked: its text heads itself.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{Actor, ActorKind, Event, ForeignJson};

/// The key of a prompt's data that marks its text as the owner's own words.
const OWNER_WORDS_KEY: &str = "ownerWords";

/// Who speaks for the owner to an agent: the owner himself, or the Assistant he asked to.
pub fn owner_voice(by: &Actor) -> &'static str {
    if by.kind == ActorKind::Assistant {
        "the owner, through his Assistant"
    } else {
        "the owner"
    }
}

/// A prompt's data with its text marked as the owner's own words, beside what it holds.
pub fn with_owner_words(data: Option<&ForeignJson>) -> ForeignJson {
    ForeignJson::with_field(data, OWNER_WORDS_KEY, serde_json::Value::Bool(true))
}

/// Whether prompt e's text is the owner's own words, by him or by the Assistant on his words.
pub fn holds_owner_words(e: &Event) -> bool {
    e.by.as_ref().is_some_and(Actor::for_owner)
        && e.data
            .as_ref()
            .and_then(|d| d.field(OWNER_WORDS_KEY))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
}

/// Prompt e's text as its agent reads it: the owner's words headed by the line that says who speaks for him, so the agent
/// takes them as his instructions; any other text as it is.
pub fn text_as_read(e: &Event) -> Cow<'_, str> {
    match e.by.as_ref() {
        Some(by) if holds_owner_words(e) => Cow::Owned(format!("{}{}", head(by), e.text)),
        _ => Cow::Borrowed(&e.text),
    }
}

/// The line that heads the owner's words by by.
fn head(by: &Actor) -> String {
    format!("[message from {}]\n", owner_voice(by))
}

/// The owner's words in text by by, stored with the line that heads them as a build before migration 130 stored them;
/// none when that line does not head it. Only the store's event reader calls it, on an event written in that older shape.
pub fn owner_words_under_head<'t>(by: &Actor, text: &'t str) -> Option<&'t str> {
    text.strip_prefix(head(by).as_str())
}

/// Where the owner is in a channel as he writes to the Assistant, as the channel names it: a page, and on a chain's page
/// the chain, its tab and the agent open there; on the Playbooks page the playbook open, and likewise on the other pages
/// that show one of a list. His prompt event stores it, and the Assistant reads it ahead of his words, so "this one"
/// means what he is looking at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Where {
    pub page: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playbook_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective_id: Option<String>,
    /// The project open on the Projects page, or the one the board or the Objectives tab is narrowed to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employee_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentor_id: Option<String>,
}
