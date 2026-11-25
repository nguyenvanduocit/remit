//! The kinds of change only the signal pipeline makes, told to the channels beside the core's (chains::Notifier).

/// Tells the channels what changed, as chains::Notifier does: one method per kind of change, carrying the record as stored.
pub trait Notifier: chains::Notifier {
    /// An event and what became of it, each time a firing on it is done or it comes in (event_record.rs).
    fn event_read(&self, _e: &wire::app::EventRecord) {}
    /// The Signals board, each time the Tick lays it out otherwise (signalboard.rs).
    fn signal_board_updated(&self, _b: &wire::app::SignalBoard) {}
}
