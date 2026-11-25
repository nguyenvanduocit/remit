//! Taking a landed change back (ADR-0114): a revert is a landing of the queue's own, ready at once and landed ahead of
//! every change. Code queues one when the forge's pipeline fails on a push where the base before it passed, when a
//! stage's guardrail breaches while it bakes, and when a revert conflicts with what landed after its target; the owner,
//! or the Assistant for him, overturns a landing with `revert_landing`, recorded with who and why.

use model::{Actor, Landing, RevertCause};
use serde::Deserialize;

use crate::Shipping;
use crate::landing::{revert_of, revertible, reverting};
use crate::queue::QUEUE;
use crate::queue::queue_cause;
use chains::apply::Records;
use chains::chain_core::new_id;
use chains::live::LandingWrite;
use chains::{Error, Result};

/// revert_landing: the landing to take back and why.
#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevertInput {
    /// the landing's id (ln_…), as the chain's landings show it
    pub landing: String,
    /// why it is taken back, in the owner's words: the Chair whose change it was reads it
    pub why: String,
}

impl Shipping {
    /// Takes landing input.landing back on the word of by, the owner or the Assistant for him: a revert is queued ahead
    /// of every change, and the Chair whose change it was is told why.
    pub async fn revert_landing(&self, by: Actor, input: RevertInput) -> Result<Landing> {
        if input.why.trim().is_empty() {
            return Err(Error::refused(
                "why is required: the Chair whose change it was reads it",
            ));
        }
        let id = input.landing.trim().to_owned();
        let target = self.store.read(move |r| r.landing(&id)).await?;
        self.queue_revert(&target, RevertCause::Overturn, by, input.why.trim())
            .await
    }

    /// Queues the revert of target for cause, by by, saying why: the revert is ready at once, says what it takes back,
    /// who asked and why, and target's Chair is told. Refused for a landing not on the base, one already being taken
    /// back, or a revert. The revert is caused by Remit in no chain, so the chain's own watches hear it, unless by is the
    /// agent of that chain.
    pub(crate) async fn queue_revert(
        &self,
        target: &Landing,
        cause: RevertCause,
        by: Actor,
        why: &str,
    ) -> Result<Landing> {
        let q = target.queue();
        let queue = self.store.read(move |r| r.queue_landings(&q)).await?;
        if !revertible(target, &queue) {
            return Err(Error::refused(format!(
                "landing {} is {}{}: only a change on its base that nothing takes back yet is reverted",
                target.id,
                target.state.kind(),
                if reverting(target, &queue).is_some() {
                    ", already being taken back"
                } else {
                    ""
                }
            )));
        }
        let now = self.now();
        let revert = revert_of(target, &new_id("ln_"), cause, by.clone(), why, now);
        let chair = self.chair_of(&target.chain_id).await;
        let who = by.who.clone();
        let own = by.kind == model::ActorKind::Agent
            && self
                .store
                .read(move |r| r.card(&who))
                .await
                .is_ok_and(|card| card.home == target.chain_id);
        let mut caused = queue_cause();
        if own {
            caused = model::Cause {
                actor: by.clone(),
                chain_id: target.chain_id.clone(),
            };
        }
        let rec = Records {
            landings: vec![LandingWrite {
                landing: revert.clone(),
                cause: caused,
            }],
            decisions: vec![crate::land::decided(
                &chair,
                format!(
                    "Revert {} version {} ({cause}): {}.",
                    target.repo, target.version.number, target.version.title
                ),
                why.to_owned(),
            )],
            ..Records::default()
        };
        let tell = format!(
            "{} version {} ({}) is being reverted ({cause}): {why}\nThe revert lands ahead of every change; you hear when \
             it has. Fix the cause in the worktree and land a new version.",
            target.repo, target.version.number, target.version.title
        );
        self.write_on_chain(&target.chain_id, &target.repo, tell, rec).await?;
        tracing::info!(landing = %target.id, revert = %revert.id, %cause, by = QUEUE, "revert queued");
        self.kick_queue(revert.queue());
        Ok(revert)
    }
}
