//! How well the agents are doing: what the Chairs accept at once, what the goal judge, reviewers and QC find, what
//! becomes of what they land, and how often something breaks. It is one pure function ([`compute`]) over what Remit has
//! recorded, so the dashboard and the analyst agent read the same numbers, and a number means one thing.
//!
//! The report (`model::OutcomeReport`) is sent, never stored. Remit records no cost and caps none
//! (ADR-0010), so no figure here is a spend.

mod kpi;
mod outcomes;
mod span;
mod usage;

pub use kpi::*;
pub use outcomes::*;
pub use span::*;

/// `n` as a float, for a share or an average.
#[expect(
    clippy::as_conversions,
    reason = "no From<i64> for f64; a count of events or of nanoseconds in an hour is far below 2^53, which a float holds exactly"
)]
fn float(n: i64) -> f64 {
    n as f64
}
