//! An employee's strengths are what it says it is good at, in its own words (ADR-0062): the Chair reads them beside the
//! owner's scope and the employee's judged cards when it chooses whom to put to work, and the record checks them. A
//! statement replaces the one before.

use std::sync::Arc;

use model::{Card, Employee};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Crew;
use chains::tools::{Effect, Tools};
use chains::{Error, Result};

/// The most characters a statement of strengths holds.
const STRENGTHS_MAX: usize = 300;

/// The input of state_strengths.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrengthsInput {
    /// what you are good at, in a sentence or two, from the work you have done: for example tracing checkout failures
    /// through Datadog logs, in at most 300 characters. It replaces what you said before
    #[schemars(length(min = 1, max = STRENGTHS_MAX))]
    strengths: String,
}

impl Crew {
    /// Gives an agent employee state_strengths.
    pub async fn add_strengths_tool(self: &Arc<Self>, t: &mut Tools, card: &Card) {
        if card.employee.is_empty() {
            return;
        }
        let id = card.employee.clone();
        if self.store.read(move |r| r.employee(&id)).await.is_err() {
            return;
        }
        let id = card.employee.clone();
        t.serving(self).add(
            "state_strengths", Effect::Writes,
            "Say what you are good at, in your own words, from the work you have done: the Chair reads it beside your \
             judged cards when it chooses whom to put to work. Say it when your work shows it, and again when it changes: \
             each statement replaces the last. Nothing waits on it.",
            move |o: Arc<Crew>, input: StrengthsInput| {
                let id = id.clone();
                async move {
                    o.state_strengths(&id, &input.strengths).await?;
                    Ok("Noted: the Chair reads it when it chooses whom to put to work.".to_owned())
                }
            },
        );
    }

    /// Stores what agent employee id says it is good at, trimmed, replacing what it said before.
    pub(crate) async fn state_strengths(&self, id: &str, raw: &str) -> Result<Employee> {
        let strengths = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let n = strengths.chars().count();
        if n == 0 {
            return Err(Error::refused("say what you are good at, in a sentence or two"));
        }
        if n > STRENGTHS_MAX {
            return Err(Error::refused(format!(
                "strengths are at most {STRENGTHS_MAX} characters: say it shorter"
            )));
        }
        let id = id.to_owned();
        let stated = self
            .store
            .update(move |tx| -> Result<Employee> {
                let mut emp = tx.employee(&id)?;
                emp.strengths = strengths;
                tx.put_employee(emp)?;
                Ok(tx.employee(&id)?)
            })
            .await?;
        self.notify.employee_updated(&stated);
        Ok(stated)
    }
}
