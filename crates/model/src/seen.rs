use serde::{Deserialize, Serialize};

use crate::Time;

/// The subject of the Assistant feed's mark: what the Assistant decided for the owner.
pub const SEEN_ASSISTANT_FEED: &str = "assistant.feed";

/// What the owner last viewed of a subject (ADR-0079): a home (a chain's id, the Assistant's) or
/// [`SEEN_ASSISTANT_FEED`]. A mark only moves forward.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Seen {
    pub subject: String,
    /// The newest event of the home he viewed; 0 for the feed, whose mark is its time alone.
    pub event_id: i64,
    /// When he viewed it, by the server's clock.
    pub at: Time,
}
