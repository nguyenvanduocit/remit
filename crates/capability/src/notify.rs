//! The kinds of change only the capability crate makes, told to the channels beside the core's (chains::Notifier).

/// Tells the channels what changed, as chains::Notifier does: one method per kind of change, carrying the record as stored.
pub trait Notifier: chains::Notifier {
    /// A skill of the library as a channel lists it, as last stored (library.rs).
    fn skill_updated(&self, _s: &wire::app::ListedSkill) {}
}
