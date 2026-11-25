//! Hiring follows the rules of good delegation. The Chair assigns work openly (brief, plan, deliverable, acceptance
//! criteria); the agent reports against each criterion, honestly; the Chair accepts every report against those criteria
//! or sends it back with specific feedback (a message, not an edge). Nothing is accepted silently: a chain cannot finish
//! while a report waits for acceptance.

use model::{Card, DecisionKind, Origin, ReportVerdict, by_agent};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Capabilities;
use chains::apply::Records;
use chains::outbox::new_message;
use chains::step::{CardEdit, Decided};
use chains::{Error, Result};

/// The Chair's judgement of an agent's report (accept_work).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptInput {
    /// the agent whose report you judge
    #[serde(default)]
    pub card_id: String,
    /// accept, or rework
    pub verdict: ReportVerdict,
    /// how the report meets or misses each acceptance criterion, in a few lines
    #[serde(default)]
    pub notes: String,
    /// for rework: each point the agent must fix, specific and actionable (what, where, how you will check it)
    #[serde(default)]
    pub feedback: Vec<String>,
}

/// xs trimmed, the empty ones left out. Pure.
pub fn non_empty(xs: &[String]) -> Vec<String> {
    xs.iter()
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The agents whose reports wait for the Chair, as "name (id)". Pure.
pub fn awaiting_acceptance(cards: &[Card]) -> Vec<String> {
    cards
        .iter()
        .filter(|c| c.awaiting_review && c.on_team())
        .map(|c| format!("{} ({})", c.name, c.id))
        .collect()
}

impl Capabilities {
    /// Records the Chair's acceptance of an agent's report, or sends the work back with its feedback.
    pub async fn accept_work(&self, chair: &Card, i: AcceptInput) -> Result<()> {
        if chair.origin != Origin::Chair {
            return Err(Error::refused("only the Chair accepts work"));
        }
        let c = self.team_card(&chair.home, &i.card_id).await?;
        if i.notes.trim().is_empty() {
            return Err(Error::refused(
                "notes are required: how the report meets or misses its acceptance criteria",
            ));
        }
        let by = by_agent(&chair.id);
        // The verdict teaches the card's employee (teach.rs): its fact is stored with the decision.
        let verdict = |kind: DecisionKind, text: String, why: String| -> Result<Records> {
            let fact = chains::fact::taught_verdict(&c, i.verdict, &by)?;
            Ok(Records {
                cards: vec![CardEdit::new(&c.id, |c| c.awaiting_review = false)],
                decisions: vec![Decided {
                    kind,
                    text,
                    why,
                    card_id: c.id.clone(),
                    by: by.clone(),
                }],
                readings: vec![fact],
                ..Records::default()
            })
        };
        match i.verdict {
            ReportVerdict::Accept => {
                let w = verdict(
                    DecisionKind::Accept,
                    format!("Accept {}: passed.", c.name),
                    i.notes.clone(),
                )?;
                self.commit(None, None, w).await?;
            }
            ReportVerdict::Rework => {
                let fb = non_empty(&i.feedback);
                if fb.is_empty() {
                    return Err(Error::refused(
                        "rework needs feedback: each point the agent must fix, specific and actionable",
                    ));
                }
                let points = fb.join("\n- ");
                let w = verdict(
                    DecisionKind::Rework,
                    format!("Send {} back for rework ({} points).", c.name, fb.len()),
                    format!("{}\n- {points}", i.notes),
                )?;
                let text = format!(
                    "[feedback from the Chair · rework]\n{}\n\nFix each point:\n- {points}\nThen report again against every acceptance criterion.",
                    i.notes
                );
                let mut m = new_message(&c.id, "Rework after feedback", &text, by.clone());
                m.steer = true;
                self.send(&c, m, w).await?;
            }
        }
        self.kick(); // the Tick judges the teaching
        Ok(())
    }
}
