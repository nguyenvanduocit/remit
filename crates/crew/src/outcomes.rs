//! The Outcomes page reads how well the agents do (crates/outcomes) and where the analyst is, both computed here, so a
//! channel shows them without deriving any of it and the analyst reads the same report.

use std::collections::HashMap;

use model::{Chain, ChainStatus, Decision, DecisionKind, OutcomeRange, OutcomeReport, OutcomeWindow, Time};
use outcomes::Span;

use crate::Crew;
use crate::analyst::is_analyst;
use chains::wait::wakes_by;
use chains::{Error, Result};
use wire::app::{CheckBack, OutcomeAnalyst, Outcomes};

/// The decisions the report counts.
const COUNTED_KINDS: [DecisionKind; 4] = [
    DecisionKind::Accept,
    DecisionKind::Rework,
    DecisionKind::GoalFailed,
    DecisionKind::Stall,
];
/// The decisions that begin an analyst's run.
const RUN_KINDS: [DecisionKind; 2] = [DecisionKind::Seed, DecisionKind::Wake];

impl Crew {
    /// The Outcomes page as of now, over the range the owner reads, split by the window he compares.
    pub async fn outcomes(&self, range: OutcomeRange, window: OutcomeWindow) -> Result<Outcomes> {
        let span = Span::new(range, window).map_err(|e| Error::refused(e.to_string()))?;
        let rec = self
            .store
            .record(COUNTED_KINDS.into_iter().chain(RUN_KINDS).collect())
            .await?;
        Ok(outcomes_of(rec, self.now(), &span))
    }

    /// How well the agents do, as of now, over the default span: what the analyst reads.
    pub async fn outcome_report(&self) -> Result<OutcomeReport> {
        let rec = self.store.record(COUNTED_KINDS.to_vec()).await?;
        Ok(outcomes::compute(&input_of(&rec, self.now()), &Span::default()))
    }
}

/// The report's input from the record, at at.
pub(crate) fn input_of(rec: &store::Record, at: Time) -> outcomes::Input<'_> {
    outcomes::Input {
        now: at,
        chains: &rec.chains,
        cards: &rec.cards,
        approvals: &rec.approvals,
        decisions: &rec.decisions,
        landings: &rec.landings,
        tool_calls: &rec.tool_calls,
        seeded: &rec.seeded,
    }
}

/// The Outcomes page from the record and the time, over span.
fn outcomes_of(rec: store::Record, at: Time, span: &Span) -> Outcomes {
    let report = outcomes::compute(&input_of(&rec, at), span);
    let analyst = analyst_of(&rec.chains, &wakes_by(rec.waits.clone()), &rec.decisions);
    Outcomes { report, analyst }
}

/// The analyst's status: the open analyst chain, else the newest one; None when there is none. chains are newest first,
/// wakes what each waits for; decisions hold every chain's run kinds.
fn analyst_of(chains: &[Chain], wakes: &HashMap<String, CheckBack>, decisions: &[Decision]) -> Option<OutcomeAnalyst> {
    let mut pick: Option<&Chain> = None;
    for c in chains.iter().filter(|c| is_analyst(c)) {
        let done = |c: &Chain| *c.status() == ChainStatus::Done;
        if pick.is_none_or(|p| done(p) && !done(c)) {
            pick = Some(c);
        }
    }
    let pick = pick?;
    let last_run_at = decisions
        .iter()
        .filter(|d| d.chain_id == pick.id && RUN_KINDS.contains(&d.kind))
        .map(|d| d.created_at)
        .max();
    let next_run_at = wakes
        .get(&pick.id)
        .filter(|_| *pick.status() == ChainStatus::Waiting)
        .map(|w| w.at);
    Some(OutcomeAnalyst {
        chain_id: pick.id.clone(),
        status: *pick.status(),
        last_run_at,
        next_run_at,
    })
}
