//! The kinds of change only the crew makes, told to the channels beside the core's (chains::Notifier).

/// Tells the channels what changed, as chains::Notifier does: one method per kind of change, carrying the record as stored.
pub trait Notifier: chains::Notifier {
    /// The coach's retrospective of a chain's finish, as last stored (ADR-0082).
    fn retrospective_added(&self, _r: &wire::app::RetrospectiveRead) {}
}
