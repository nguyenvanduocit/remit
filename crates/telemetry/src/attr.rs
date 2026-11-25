//! The names every log, span and metric about a chain carries (ADR-0119), so one search on `@remit.chain_id` finds all
//! a chain did, in every process. A span sets them where it opens (`tracing::info_span!("remit.turn", { CHAIN_ID } =
//! ...)`); the server's layer hands them down to every record inside it, and the `/otlp` door stamps them on what an
//! agent's session exports, from the run's token, never from the payload.

/// The chain: on every record about one.
pub const CHAIN_ID: &str = "remit.chain_id";
pub const CARD_ID: &str = "remit.card_id";
pub const SESSION_ID: &str = "remit.session_id";
/// The message a card's turn answers (the event that opened it).
pub const TURN: &str = "remit.turn";
pub const KEY_RESULT: &str = "remit.key_result";
pub const LANDING_ID: &str = "remit.landing_id";
pub const RELEASE_ID: &str = "remit.release_id";
pub const CHAIN_KIND: &str = "remit.chain_kind";

/// Every correlation name: a record inside a span carries the ones its spans set.
pub const ALL: [&str; 8] = [
    CHAIN_ID, CARD_ID, SESSION_ID, TURN, KEY_RESULT, LANDING_ID, RELEASE_ID, CHAIN_KIND,
];

/// What every name above begins with: an attribute under it is Remit's to set, and one a payload brings is dropped.
pub const PREFIX: &str = "remit.";

/// The span a card's turn runs in, the root of the turn's trace: the agent's own spans are put under it.
pub const TURN_SPAN: &str = "remit.turn";
/// The span a chain's step runs in.
pub const STEP_SPAN: &str = "remit.step";
/// The step's command, on `STEP_SPAN`.
pub const STEP: &str = "remit.step";
/// The span a chain's checks run in, outside its steps (a submit's, a finish's): it names the chain its children run for.
pub const CHECKS_SPAN: &str = "remit.checks";
/// The span a call to an outside system runs in (`source`'s gate), naming the chain that made it: a program or script
/// it runs is a child of it.
pub const SOURCE_SPAN: &str = "remit.source";
/// The span a child process Remit starts runs in (a check, a release step, a program), under the span of the work it runs
/// for: the child is told it (`childenv::trace`), and it records how the child ended.
pub const CHILD_SPAN: &str = "remit.child";
/// The span a release's work runs in (its procedure, its going live, its taking back): the root of that work's trace.
pub const RELEASE_SPAN: &str = "remit.release";
/// A child of a release's span for each landing it carries, with that landing's chain: one search on a chain finds the
/// release trace its change went out in.
pub const RELEASE_LANDING_SPAN: &str = "remit.release.landing";
