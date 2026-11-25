//! An employee's runs across chains, for the owner: each card it held, with the first line of what it was asked, how it
//! ended and where the ask came from, newest first, and how many failed and how many wait. Its "Now" line is its seat in
//! the at-work view (atwork.rs), which names the employee. Pure: `runs_of`. At the edge: `employee_runs`.

use std::collections::HashMap;

use model::{Card, CardRole, CardStatus, Chain, Decision, Origin};
use wire::app::{EmployeeRuns, Run};

use crate::orch::Orchestrator;
use capability::my_work_tools::card_outcome;
use chains::Result;

/// Runs an `employee_runs` lists, the newest; the counts cover all of them.
const RUNS_SHOWN: usize = 50;

/// The runs of employee from its cards (oldest first, as the store gives them), with their chains and the chains'
/// decisions by chain id.
pub(crate) fn runs_of(
    employee: &str,
    cards: &[Card],
    chains: &HashMap<String, Chain>,
    decisions: &HashMap<String, Vec<Decision>>,
) -> EmployeeRuns {
    let mut out = EmployeeRuns {
        employee: employee.to_owned(),
        more: cards.len() > RUNS_SHOWN,
        ..EmployeeRuns::default()
    };
    for k in cards.iter().rev() {
        let ch = chains.get(&k.home);
        let outcome = card_outcome(k, decisions.get(&k.home).map_or(&[], Vec::as_slice));
        let waiting = matches!(k.status(), CardStatus::Permission | CardStatus::Waiting) || k.awaiting_review;
        if outcome == "failed" {
            out.failures = out.failures.saturating_add(1);
        }
        if waiting {
            out.waiting = out.waiting.saturating_add(1);
        }
        if out.runs.len() >= RUNS_SHOWN {
            continue;
        }
        let asked = if k.origin == Origin::Chair {
            ch.map_or("", |c| c.goal.as_str())
        } else {
            k.brief.as_str()
        };
        out.runs.push(Run {
            chain_id: k.home.clone(),
            chain_title: ch.map(|c| c.title.clone()).unwrap_or_default(),
            card_id: k.id.clone(),
            request: first_line(asked),
            outcome: outcome.to_owned(),
            source: source_of(k, ch),
            waiting,
            live: k.status().works(),
            started: k.created_at,
            ended: k.ended_at(),
        });
    }
    out
}

/// The first line of text that says something.
fn first_line(text: &str) -> String {
    text.lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// Where the ask card k worked on came from.
fn source_of(k: &Card, ch: Option<&Chain>) -> String {
    match CardRole::of(k) {
        CardRole::Chair => ch.map_or_else(String::new, |c| c.origin.as_str().to_owned()),
        CardRole::Spawned => "agent".to_owned(),
        CardRole::Judge(_) => "remit".to_owned(),
        CardRole::Worker | CardRole::Assistant => "chair".to_owned(),
    }
}

impl Orchestrator {
    /// Employee id's runs across chains (`runs_of`).
    pub async fn employee_runs(&self, id: String) -> Result<EmployeeRuns> {
        let emp = id.clone();
        let (cards, chains, decisions) = self
            .store
            .read(move |r| {
                let cards = r.employee_cards(&emp)?;
                let mut chains = HashMap::new();
                let mut decisions = HashMap::new();
                for k in &cards {
                    if chains.contains_key(&k.home) {
                        continue;
                    }
                    match r.chain(&k.home) {
                        Ok(c) => {
                            decisions.insert(k.home.clone(), r.decisions(&k.home)?);
                            chains.insert(k.home.clone(), c);
                        }
                        Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                Ok::<_, store::Error>((cards, chains, decisions))
            })
            .await?;
        Ok(runs_of(&id, &cards, &chains, &decisions))
    }
}
