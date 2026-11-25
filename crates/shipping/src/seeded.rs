//! Seeded faults (ADR-0102): how well the judges catch a fault is measured on work known to carry one. A trial is a chain
//! of kind `trial` (ADR-0113), whose capability `seeding` plants the fault: an engineer makes the ordinary change its
//! goal names, the Chair plants one fault in it and says what and where with `plant_fault`, and the reviewer, QC and
//! goal judge judge it as any change, knowing nothing of the trial. Remit keeps the fault where no agent's tool reads
//! it, and the outcome report scores each role's first verdict after the planting (crates/outcomes). The code holds the
//! rails: a trial's change never leaves the chain (`land` refuses it), one fault per chain, and the goal judge's
//! first ruling ends the trial.

use std::sync::Arc;

use model::{Card, Chain, SeededFault, by_agent};
use schemars::JsonSchema;
use serde::Deserialize;

use chains::chain_core::{gist, new_id};
use chains::goal::goal_approvals;
use chains::tools::{Effect, SelfCard, Tools};
use chains::{Error, Result};

use crate::Shipping;

/// Refuses a trial's land.
const ERR_TRIAL_LAND: &str = "this chain is a trial of the judges (seeding): its change carries a fault on purpose and \
                              never leaves the chain, so it never lands";

/// Whether chain c is a trial of its judges: its kind says so (ADR-0113).
pub fn is_trial(c: &Chain) -> bool {
    c.kind == model::ChainKind::Trial
}

/// Refuses landing chain c's change when it is a trial: the one rail between a planted fault and the owner's base.
pub fn trial_land_refusal(c: &Chain) -> Result<()> {
    if is_trial(c) {
        return Err(Error::refused(ERR_TRIAL_LAND));
    }
    Ok(())
}

/// What plant_fault takes.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlantInput {
    /// what is wrong in the work, and why a careful judge should find it
    #[serde(default)]
    pub fault: String,
    /// where it is: the path in the chain's worktree and the line, e.g. src/limit.rs:42
    #[serde(default)]
    pub place: String,
}

impl Shipping {
    /// Gives the Chair of a trial plant_fault.
    pub fn add_plant_tool(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let me = me.clone();
        t.serving(self).add(
            "plant_fault",
            Effect::Writes,
            "Record the one fault you planted in this trial's work: what is wrong, why a judge should find it, and its \
             path:line. Remit keeps it where no judge reads it and scores each judge's first verdict after it. Refused for a \
             second fault: a trial carries one.",
            move |o, i: PlantInput| {
                let me = me.clone();
                async move {
                    let chair = me.read(&o).await?;
                    let f = o.plant_fault(&chair, i).await?;
                    Ok(format!("Recorded the fault at {}: {}", f.place, gist(&f.fault, 80)))
                }
            },
        );
    }

    /// Stores the fault the Chair of a trial planted, the chain's only one.
    pub async fn plant_fault(&self, chair: &Card, i: PlantInput) -> Result<SeededFault> {
        let (fault, place) = (i.fault.trim().to_owned(), i.place.trim().to_owned());
        if fault.is_empty() || place.is_empty() {
            return Err(Error::refused(
                "a planted fault needs what is wrong (fault) and where it is (place, a path:line of the worktree)",
            ));
        }
        let f = SeededFault {
            id: new_id("sf_"),
            chain_id: chair.home.clone(),
            by: by_agent(&chair.id),
            fault,
            place,
            planted_at: self.now(),
        };
        let stored = f.clone();
        let fresh = self.store.update(move |tx| tx.put_seeded_fault(&stored)).await?;
        if !fresh {
            return Err(Error::refused(
                "this trial's fault is planted already: a trial carries one, so a verdict that sends the work back is a \
                 verdict on it",
            ));
        }
        Ok(f)
    }

    /// Whether ch is a trial the goal judge has ruled on: its first ruling ends the trial, so the Chair's next finish
    /// ends the chain without another check.
    pub async fn trial_ruled(&self, ch: &Chain) -> Result<bool> {
        if !is_trial(ch) {
            return Ok(false);
        }
        let id = ch.id.clone();
        let approvals = self.store.read(move |r| r.approvals(&id)).await?;
        Ok(!goal_approvals(&approvals).is_empty())
    }
}
