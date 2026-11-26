//! What the chain core, and a domain crate below another, hand up to the application (ADR-0096): the calls a turn's end,
//! a commit, a session, a hand-over and an applied extension make into the domains above, each a named method called
//! where it was called before the split, in the same order. `orch` implements it once, so what is asked of each domain
//! from below is read in one place.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use model::{Card, Chain, ChainKind, ChainOrigin, Duel, Employee};

use crate::Result;
use crate::apply::Records;
use crate::hire_input::{HireInput, Hiring};
use crate::session::Run;
use crate::step::{CardEdit, EndGoalCheckCmd, JudgeEnd};

/// What a turn is given of its card's memory (ADR-0117): the note the agent reads ahead of the message, and the memories it
/// names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recall {
    pub note: String,
    pub ids: Vec<String>,
}

#[async_trait]
pub trait Domains: Send + Sync {
    /// What card c's turn on a message reading text is given of its memory, none of given (what the session was given
    /// already); none when the card remembers nothing or nothing bears on the message.
    async fn recall(&self, c: &Card, text: &str, given: &HashSet<String>) -> Option<Recall>;

    /// What card c's agent is given of its memory at moment m of its work (after a call of a tool, as a subagent
    /// starts), none of given; none when the card remembers nothing or nothing bears on the moment.
    async fn recall_at(&self, c: &Card, m: &wire::runtime::Moment, given: &HashSet<String>) -> Option<Recall>;

    /// Counts the memories ids as given to card c's agent, once their note has reached it.
    async fn given(&self, c: &Card, ids: &[String]);

    /// What card c's session is launched with, resuming the session resume with token as its key: its spec and access.
    async fn spec(&self, c: &Card, resume: &str, token: &str) -> Result<(agent::Spec, library::Session)>;

    /// Records what card c's session session_id was launched with (cardcontext.rs).
    async fn record_launch(
        &self,
        c: &Card,
        spec: &agent::Spec,
        acc: &library::Session,
        session_id: &str,
        resumed: bool,
    ) -> Result<()>;

    /// The capabilities a chain of kind starts with (ADR-0113: the kinds' table in capability).
    fn kind_capabilities(&self, kind: ChainKind) -> Vec<String>;

    /// The section of the Chair's brief a chain of kind reads after its goal (the kinds' table in capability).
    fn kind_brief(&self, kind: ChainKind) -> String;

    /// Why a chain of origin may not be started as kind, or none when it may (`capability::kinds::may_not_start`): asked
    /// where a starter other than Remit names a kind.
    fn kind_refusal(&self, origin: &ChainOrigin, kind: ChainKind) -> Option<String>;

    /// The hire chair asks for with i, made ready to store: a hand-over's, a workflow's, a duel's side. The model and
    /// effort are i's, else the agent's, else the Chair's: no decider is asked, so nothing is judged.
    async fn hiring_of(&self, chair: &Card, i: HireInput) -> Result<Hiring>;

    /// Why hirer may not put kept employee emp to work for input, or none when it may.
    fn standing_refusal(&self, hirer: &Employee, emp: &Employee, input: &HireInput) -> Result<()>;

    /// A duel's judge ended its turn or its card without a pick, for why: the duel ends without one.
    async fn duel_judge_ended(&self, judge: &Card, why: &str);

    /// A round's judge ended its turn or its card without a pick, for why: the round ends without one, and every
    /// attempt's version waiting on it goes back to its Chair.
    async fn round_judge_ended(&self, judge: &Card, why: &str);

    /// A second judge ended its turn or its card without comparing, for why: its second opinion ends without one, and
    /// the Assistant's answer stands as it was.
    async fn second_judge_ended(&self, judge: &Card, why: &str);

    /// One side of duel d reported: once both have, a judge is hired.
    async fn duel_reported(&self, d: &Duel) -> Result<()>;

    /// Ends the goal check of chain home whose judge is judge, for why, stored with w; end: what of the judge's work ended.
    async fn end_goal_check(&self, home: &str, judge: &str, why: &str, end: JudgeEnd, w: Records) -> Result<Chain>;

    /// The goal judge's turn on run r ended (end: on the check or on a question), for why, its card edited by edit.
    async fn judge_turn_ended(&self, r: &Arc<Run>, judge: &Card, why: &str, end: JudgeEnd, edit: CardEdit);

    /// The end of a goal check c on chain ch with what its verdict says observed.
    async fn observe_goal_check(&self, c: EndGoalCheckCmd, ch: &Chain) -> EndGoalCheckCmd;

    /// Tells the judge that opened verification chain vch what it found: summary.
    async fn tell_judge_what_verification_found(&self, vch: &Chain, summary: &str);

    /// Makes exts, the extensions the server holds and the owner did not disable, under root the plugin pipeline's.
    fn use_pipeline(&self, root: &std::path::Path, exts: &[source::Extension]);

    /// Carries on the submit of chain chain_id's repository repo that a restart cut off, as work of its own.
    async fn carry_on_later(&self, chain_id: &str, repo: &str);
}
