//! What a Chair reads of the kept employees it may put to work (ADR-0052, ADR-0061): every employee is kept until the
//! owner retires it, so "standing" is any kept one. hire_agent's description lists them before the agents to hire anew
//! (`standing_help`), list_agents' standing gives the same records (`standing_summaries`), each from
//! `standing_summary_of`.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use model::{Card, Chain, Decision, DecisionKind, Employee, Time};
use serde::Serialize;

use crate::Crew;
use crate::employee_rules::standing_refusal;
use crate::lineage_standing::{by_template, standing_words};
use chains::Result;
use chains::health::elapsed;
use chains::hire_input::HireInput;
use chains::lifecycle::holds_open;
use wire::app::LineageStanding;

/// How many of a kept employee's memories a Chair reads.
const MEMORY_GIST_SHOWN: usize = 3;

/// How many judged cards of an employee a Chair reads.
const LAST_CARDS_SHOWN: usize = 5;

/// Where a kept employee stands, for a Chair choosing whom to put to work: what it is, the cards it holds open in every
/// chain, its last cards judged, how its lineage stands in the chain's kind of work, and the gist of what it remembers
/// that bears on the chain.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct StandingSummary {
    pub id: String,
    /// Its name when it has one, its code and what it was hired from (`Employee::shown`).
    pub shown: String,
    pub template: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scope: String,
    /// In its own words (state_strengths).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub strengths: String,
    #[serde(rename = "open_cards")]
    pub open: Vec<OpenCard>,
    /// The newest judged first, at most `LAST_CARDS_SHOWN`.
    #[serde(rename = "last_cards")]
    pub last: Vec<EndedCard>,
    /// How its template stands in the chain's kind of work, from the outside's results and the duels fought there; none
    /// before its lineage has either.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<LineageStanding>,
    /// The gist of its memories, at most `MEMORY_GIST_SHOWN`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<String>,
}

/// A card an employee holds open: in a turn or about to be.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OpenCard {
    pub chain: String,
    pub card: String,
    pub status: String,
    /// Since the hire.
    pub age: String,
}

/// A card the Chair judged: accepted or sent back, and how long it took from the hire to that verdict.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EndedCard {
    pub chain: String,
    pub card: String,
    /// What the card was: its name, or its deliverable when it has none.
    pub work: String,
    pub outcome: String,
    pub took: String,
}

impl StandingSummary {
    /// s as one line of hire_agent's description.
    fn line(&self) -> String {
        let mut b = format!("- {}: {}", self.id, self.shown); // shown names the template
        if !self.scope.is_empty() {
            b.push_str(&format!(", {}", self.scope));
        }
        if !self.strengths.is_empty() {
            b.push_str(&format!(", strong in: {}", self.strengths));
        }
        b.push_str(&format!("; {} open card(s)", self.open.len()));
        if !self.last.is_empty() {
            let last: Vec<String> = self
                .last
                .iter()
                .map(|c| format!("{}: {} in {}", c.outcome, c.work, c.took))
                .collect();
            b.push_str(&format!("; last judged: {}", last.join(", ")));
        }
        if let Some(l) = &self.lineage {
            b.push_str(&format!("; {}", standing_words(l)));
        }
        if !self.memory.is_empty() {
            b.push_str(&format!("; remembers: {}", self.memory.join(" | ")));
        }
        b
    }
}

impl Crew {
    /// How a Chair reads, first in hire_agent's description, the kept employees it may put to work by their id (the
    /// employee input): what each is, the cards it holds open, how its last ones went and the gist of its memory. Empty
    /// when there are none, or when they cannot be read (logged).
    pub async fn standing_help(&self, chair: &Card) -> String {
        let home = chair.home.clone();
        let staff = match self.store.read(move |r| r.chain(&home)).await {
            Ok(chain) => self.summaries_for(&chain, chair).await,
            Err(err) => Err(err.into()),
        };
        let staff = match staff {
            Ok(s) => s,
            Err(err) => {
                tracing::error!(home = %chair.home, card = %chair.id, %err, "kept employees for hire_agent");
                return String::new();
            }
        };
        if staff.is_empty() {
            return String::new();
        }
        let mut b = String::from(
            "Reuse first. The employees you keep and can put to work by their id as employee, in the role you give; \
             choose by what each is strong in and how its judged cards went, whatever agent it was hired from; a new hire \
             while one of them holds no card open is refused unless why names that one by its id and says why its \
             strengths do not fit. Their open cards below are as they stood when this session opened: list_agents' \
             standing shows who holds none now, so read it before a new hire:",
        );
        for e in &staff {
            b.push('\n');
            b.push_str(&e.line());
        }
        b.push('\n');
        b
    }

    /// Every standing employee chair may put to work (`standing_refusal` passes it), the oldest first. A Chair with no
    /// employee on record puts none to work.
    pub async fn standing_staff(&self, chair: &Card) -> Result<Vec<Employee>> {
        let id = chair.employee.clone();
        self.store
            .read(move |r| -> Result<Vec<Employee>> {
                let hirer = match r.employee(&id) {
                    Ok(h) => h,
                    Err(store::Error::NotFound) => return Ok(Vec::new()),
                    Err(err) => return Err(err.into()),
                };
                let mut staff = r.employees()?;
                staff.retain(|e| {
                    let input = HireInput {
                        employee: e.id.clone(),
                        ..HireInput::default()
                    };
                    standing_refusal(&hirer, e, &input).is_ok()
                });
                Ok(staff)
            })
            .await
    }

    /// Where each kept employee the Chair of chain chain_id may put to work stands.
    pub async fn standing_summaries(&self, chain_id: &str) -> Result<Vec<StandingSummary>> {
        let id = chain_id.to_owned();
        let (chain, chair) = self
            .store
            .read(move |r| -> Result<(Chain, Card)> {
                let chain = r.chain(&id)?;
                let chair = r.card(&chain.chair_id)?;
                Ok((chain, chair))
            })
            .await?;
        self.summaries_for(&chain, &chair).await
    }

    /// Where each kept employee chair, the Chair of chain, may put to work stands.
    async fn summaries_for(&self, chain: &Chain, chair: &Card) -> Result<Vec<StandingSummary>> {
        let staff = self.standing_staff(chair).await?;
        let lineages = by_template(self.standings_in(chain).await?);
        let ids: Vec<String> = staff.iter().map(|e| e.id.clone()).collect();
        // each employee's cards, with the decisions of every chain it worked in, read once for every employee at work there
        let worked = self
            .store
            .read(move |r| -> Result<Vec<(Vec<Card>, Vec<Decision>)>> {
                let mut decisions: HashMap<String, Vec<Decision>> = HashMap::new();
                let mut out = Vec::with_capacity(ids.len());
                for id in &ids {
                    let cards = r.employee_cards(id)?;
                    let mut judged = Vec::new();
                    let mut chains = HashSet::new();
                    for c in &cards {
                        if !decisions.contains_key(&c.home) {
                            decisions.insert(c.home.clone(), r.decisions(&c.home)?);
                        }
                        if chains.insert(c.home.clone()) {
                            judged.extend(decisions.get(&c.home).into_iter().flatten().cloned());
                        }
                    }
                    out.push((cards, judged));
                }
                Ok(out)
            })
            .await?;
        let now = self.now();
        let mut out = Vec::with_capacity(staff.len());
        for (e, (cards, judged)) in staff.iter().zip(worked) {
            let mut summary = standing_summary_of(e, &cards, &judged, now);
            summary.lineage = lineages.get(&e.template).cloned();
            summary.memory = self.memory_gist(e, MEMORY_GIST_SHOWN).await;
            out.push(summary);
        }
        Ok(out)
    }
}

/// Where employee e stands, from the cards it was put to work in and the decisions of their chains.
fn standing_summary_of(e: &Employee, cards: &[Card], decisions: &[Decision], now: Time) -> StandingSummary {
    let mut s = StandingSummary {
        id: e.id.clone(),
        shown: e.shown(),
        template: e.template.clone(),
        scope: e.scope.clone(),
        strengths: e.strengths.clone(),
        ..StandingSummary::default()
    };
    let mut verdicts: HashMap<&str, &Decision> = HashMap::new(); // each card's newest verdict
    for d in decisions
        .iter()
        .filter(|d| matches!(d.kind, DecisionKind::Accept | DecisionKind::Rework))
    {
        if verdicts
            .get(d.card_id.as_str())
            .is_none_or(|v| d.created_at >= v.created_at)
        {
            verdicts.insert(&d.card_id, d);
        }
    }
    let mut judged: Vec<(EndedCard, Time)> = Vec::new();
    for c in cards {
        if holds_open(c) {
            s.open.push(OpenCard {
                chain: c.home.clone(),
                card: c.id.clone(),
                status: c.status().to_string(),
                age: span(elapsed(c.created_at, now)),
            });
        }
        if let Some(d) = verdicts.get(c.id.as_str()) {
            let outcome = if d.kind == DecisionKind::Rework {
                "sent back"
            } else {
                "accepted"
            };
            let work = if c.name.is_empty() {
                c.deliverable.clone()
            } else {
                c.name.clone()
            };
            judged.push((
                EndedCard {
                    chain: c.home.clone(),
                    card: c.id.clone(),
                    work,
                    outcome: outcome.to_owned(),
                    took: span(elapsed(c.created_at, d.created_at)),
                },
                d.created_at,
            ));
        }
    }
    judged.sort_by_key(|j| std::cmp::Reverse(j.1)); // stable: the newest first
    s.last = judged.into_iter().take(LAST_CARDS_SHOWN).map(|(c, _)| c).collect();
    s
}

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// d as a Chair reads an age: in whole minutes, hours or days, the next unit beside them.
pub(crate) fn span(d: Duration) -> String {
    let s = d.as_secs();
    if s < MINUTE {
        "<1m".to_owned()
    } else if s < HOUR {
        format!("{}m", s / MINUTE)
    } else if s < DAY {
        units_of(s, HOUR, "h", MINUTE, "m")
    } else {
        units_of(s, DAY, "d", HOUR, "h")
    }
}

/// s seconds in whole big units, and the whole small units left when there are any.
fn units_of(s: u64, big: u64, big_name: &str, small: u64, small_name: &str) -> String {
    let mut out = format!("{}{big_name}", s.checked_div(big).unwrap_or_default());
    let rest = s
        .checked_rem(big)
        .unwrap_or_default()
        .checked_div(small)
        .unwrap_or_default();
    if rest > 0 {
        out.push_str(&format!("{rest}{small_name}"));
    }
    out
}
