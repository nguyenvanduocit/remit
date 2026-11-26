//! What the chain core and the pipeline hand up (ADR-0096), done by the crate that does each: `Domains` for the core's
//! calls into the domains, `Actions` for what a firing does. Each method names the one it calls, so what is asked from
//! below is read in one place.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use model::{Card, Chain, ChainKind, ChainOrigin, Duel, Employee};

use chains::apply::Records;
use chains::domains::Recall;
use chains::hire_input::{HireInput, Hiring};
use chains::session::Run;
use chains::step::{CardEdit, EndGoalCheckCmd, JudgeEnd};
use chains::{Domains, Result};
use pipeline::actions::Actions;
use pipeline::automation::DigestPeriod;
use pipeline::processing::Delivery;

use crate::orch::Orchestrator;

#[async_trait]
impl Domains for Orchestrator {
    async fn recall(&self, c: &Card, text: &str, given: &HashSet<String>) -> Option<Recall> {
        match self.crew.recall_for_turn(c, text, given).await {
            Ok(found) if !found.is_empty() => Some(Recall {
                note: crew::memory::recall_note(crew::memory::TURN_LEAD, &found),
                ids: found.into_iter().map(|m| m.id).collect(),
            }),
            Ok(_) => None,
            Err(err) => {
                tracing::warn!(card = %c.id, %err, "memory: the turn is given nothing it remembers");
                None
            }
        }
    }

    async fn recall_at(&self, c: &Card, m: &wire::runtime::Moment, given: &HashSet<String>) -> Option<Recall> {
        match self.crew.recall_at(c, m, given).await {
            Ok(found) if !found.is_empty() => Some(Recall {
                note: crew::memory::recall_note(crew::tool_recall::lead_of(m), &found),
                ids: found.into_iter().map(|m| m.id).collect(),
            }),
            Ok(_) => None,
            Err(err) => {
                tracing::warn!(card = %c.id, %err, "memory: the agent is given nothing it remembers as it works");
                None
            }
        }
    }

    async fn given(&self, c: &Card, ids: &[String]) {
        self.crew.count_given(c, ids).await;
    }

    async fn spec(&self, c: &Card, resume: &str, token: &str) -> Result<(agent::Spec, library::Session)> {
        Orchestrator::spec(self, c, resume, token).await
    }

    async fn record_launch(
        &self,
        c: &Card,
        spec: &agent::Spec,
        acc: &library::Session,
        session_id: &str,
        resumed: bool,
    ) -> Result<()> {
        Orchestrator::record_launch(self, c, spec, acc, session_id, resumed).await
    }

    fn kind_capabilities(&self, kind: ChainKind) -> Vec<String> {
        capability::kinds::capabilities_of(kind)
    }

    fn kind_brief(&self, kind: ChainKind) -> String {
        capability::kinds::brief_of(kind)
    }

    fn kind_refusal(&self, origin: &ChainOrigin, kind: ChainKind) -> Option<String> {
        capability::kinds::may_not_start(origin, kind)
    }

    async fn hiring_of(&self, chair: &Card, i: HireInput) -> Result<Hiring> {
        Orchestrator::hiring_of(self, chair, i).await
    }

    fn standing_refusal(&self, hirer: &Employee, emp: &Employee, input: &HireInput) -> Result<()> {
        crew::employee_rules::standing_refusal(hirer, emp, input)
    }

    async fn duel_judge_ended(&self, judge: &Card, why: &str) {
        self.crew.duel_judge_ended(judge, why).await;
    }

    async fn round_judge_ended(&self, judge: &Card, why: &str) {
        self.shipping.round_judge_ended(judge, why).await;
    }

    async fn second_judge_ended(&self, judge: &Card, why: &str) {
        Orchestrator::second_judge_ended(self, judge, why).await;
    }

    async fn duel_reported(&self, d: &Duel) -> Result<()> {
        self.crew.duel_reported(d).await
    }

    async fn end_goal_check(&self, home: &str, judge: &str, why: &str, end: JudgeEnd, w: Records) -> Result<Chain> {
        self.shipping.end_goal_check(home, judge, why, end, w).await
    }

    async fn judge_turn_ended(&self, r: &Arc<Run>, judge: &Card, why: &str, end: JudgeEnd, edit: CardEdit) {
        self.shipping.judge_turn_ended(r, judge, why, end, edit).await;
    }

    async fn observe_goal_check(&self, c: EndGoalCheckCmd, ch: &Chain) -> EndGoalCheckCmd {
        self.shipping.observe_goal_check(c, ch).await
    }

    async fn tell_judge_what_verification_found(&self, vch: &Chain, summary: &str) {
        self.shipping.tell_judge_what_verification_found(vch, summary).await;
    }

    fn use_pipeline(&self, root: &Path, exts: &[source::Extension]) {
        self.pipeline.use_pipeline(root, exts);
    }

    async fn carry_on_later(&self, chain_id: &str, repo: &str) {
        self.shipping.carry_on_later(chain_id, repo).await;
    }
}

#[async_trait]
impl Actions for Orchestrator {
    async fn fire_hire(&self, d: &Delivery) -> Result<()> {
        self.shipping.fire_hire(d).await
    }

    async fn fire_end(&self, d: &Delivery) -> Result<()> {
        self.shipping.fire_end(d).await
    }

    async fn fire_deadline(&self, d: &Delivery) -> Result<()> {
        self.crew.fire_deadline(d).await
    }

    async fn fire_assist(&self, d: &Delivery) -> Result<()> {
        Orchestrator::fire_assist(self, d).await
    }

    async fn fire_record_landing_checks(&self, d: &Delivery) -> Result<()> {
        self.shipping.fire_record_landing_checks(d).await
    }

    async fn fire_teach(&self, d: &Delivery) -> Result<()> {
        self.crew.fire_teach(d).await
    }

    async fn embed_memories(&self) {
        self.crew.embed_waiting().await;
    }

    async fn review_memories(&self) {
        self.crew.review_memories().await;
    }

    async fn fire_digest(&self, d: &Delivery, period: DigestPeriod) -> Result<()> {
        Orchestrator::fire_digest(self, d, period).await
    }

    async fn sync_landings(&self) {
        self.shipping.sync_landings().await;
    }

    async fn follow_chain_landings(&self, id: &str) {
        self.shipping.follow_chain_landings(id).await;
    }
}
