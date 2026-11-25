//! The Notifier a channel implements: every kind of change, the core's and each crate's (ADR-0096), and those only the
//! orchestrator's views and the Assistant's feed make.

/// Tells the channels what changed, as chains::Notifier does: one method per kind of change, carrying the record as stored.
pub trait Notifier: pipeline::notify::Notifier + capability::notify::Notifier + crew::notify::Notifier {
    /// The Assistant's feed, laid out whole, each time it moves (assist_feed.rs).
    fn assistant_feed_updated(&self, _f: &wire::app::AssistantFeed) {}
    /// Where a mentor stands, as the views computed it again (mentor_standing.rs).
    fn mentor_standing_updated(&self, _s: &wire::app::MentorStanding) {}
    /// The owner's board, as the views computed it again (views.rs).
    fn board_updated(&self, _b: &wire::app::Board) {}
    /// Where a chain stands, as the views computed it again.
    fn chain_standing_updated(&self, _s: &wire::app::ChainStanding) {}
    /// How a chain's result has been judged, as the views computed it again (verification.rs).
    fn verification_updated(&self, _v: &wire::app::Verification) {}
    /// Where an agent stands, as the views computed it again.
    fn agent_standing_updated(&self, _a: &wire::app::AgentStanding) {}
    /// A repository's queue and releases, as the views computed them again (repo_view.rs).
    fn repo_updated(&self, _v: &wire::app::RepoView) {}
}
