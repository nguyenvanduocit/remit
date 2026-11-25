//! The owner changes what Remit does on its own by telling the Assistant (ADR-0035): he pauses the judge's hire to have no
//! goal check, or a job to have it stop. Pausing is the only change the shipped subscriptions take here; their settings
//! are the code's.

use std::sync::Arc;

use model::{Actor, Control, OwnerKind, Subscription};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Capabilities;
use chains::tools::{Effect, Tools};
use chains::{Error, Result};
use pipeline::automation::put_shipped;
use pipeline::processing::remit_only;
use pipeline::routine::resumed;

/// What Remit does on its own, named.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AutomationIdInput {
    /// the id of what Remit does on its own (sub_hire_…, sub_run_…), as read_signals shows it
    id: String,
}

fn no_automation(id: &str) -> Error {
    Error::refused(format!(
        "no automation {id:?}: read_signals lists them, each of kind automation"
    ))
}

/// Says whether s is one of the subscriptions that ship with Remit.
pub(crate) fn is_automation(s: &Subscription) -> bool {
    s.builtin && s.owner.kind == OwnerKind::Remit && remit_only(&s.action.kind)
}

impl Capabilities {
    /// Gives the Assistant, acting as by, the tools that pause and resume what Remit does on its own.
    pub fn add_automation_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        let pause_by = by.clone();
        t.serving(self).add(
            "pause_automation", Effect::Writes,
            "Pause something Remit does on its own, for the owner: the hire of the goal judge, or a job (the merge queues' run, worktree collection, snapshot, event packing). Pausing the goal judge's hire means a chain's finish is not checked: the chain finishes without a check when the check's time is up.",
            move |o, i: AutomationIdInput| {
                let by = pause_by.clone();
                async move {
                    let a = o.set_automation_paused(&i.id, true, by).await?;
                    Ok(format!("Paused {}.", a.id))
                }
            },
        );
        let resume_by = by.clone();
        t.serving(self).add(
            "resume_automation", Effect::Writes,
            "Resume what Remit does on its own that the owner paused. A job waits for its next time; a hire answers what happened while it was paused.",
            move |o, i: AutomationIdInput| {
                let by = resume_by.clone();
                async move {
                    let a = o.set_automation_paused(&i.id, false, by).await?;
                    Ok(format!("Resumed {}.", a.id))
                }
            },
        );
    }

    /// Pauses or resumes the shipped subscription id as by, read and written in one transaction. A job that resumes counts
    /// its next time from now, so the times it missed are not run.
    pub(crate) async fn set_automation_paused(&self, id: &str, paused: bool, by: Actor) -> Result<Subscription> {
        let (key, now) = (id.to_owned(), self.now());
        let saved = self
            .store
            .update(move |tx| {
                let cur = match tx.subscription(&key) {
                    Ok(cur) if is_automation(&cur) => cur,
                    Ok(_) | Err(store::Error::NotFound) => return Err(no_automation(&key)),
                    Err(err) => return Err(err.into()),
                };
                let mut next = if paused {
                    Subscription {
                        control: Control::Paused,
                        ..cur
                    }
                } else {
                    resumed(cur, now)?
                };
                (next.by, next.updated_at) = (by, now);
                put_shipped(tx, next)
            })
            .await?;
        self.kick();
        Ok(saved)
    }
}
