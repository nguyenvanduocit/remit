//! A finished chain whose last goal verdict waited on the owner's merge or deploy is looked at again once he acts: after
//! the merge (fact:chain.merged, ADR-0039) and after the live mark (fact:chain.marked-live, ADR-0067). The new verdict is
//! the chain's.

use model::{Approval, ChainStatus, FindingBlocker, FindingStatus};

use crate::Shipping;
use crate::hire::{SignalHire, after_hire};
use chains::fact::{FactValue, keys};
use chains::goal::goal_approvals;
use chains::goal::goal_deadline;
use chains::schedule::verifying;
use chains::step::{ChainCmd, HireRecheckCmd};
use chains::{Error, Result};

/// Whether the goal judge's last verdict left a key result unverified that a merge or what follows it settles: its blocker
/// is write (a merge) or deploy (it shows once deployed).
pub(crate) fn awaits_merge(approvals: &[Approval]) -> bool {
    goal_approvals(approvals).last().is_some_and(|last| {
        last.findings.iter().any(|f| {
            f.status != FindingStatus::Met && matches!(f.blocker, Some(FindingBlocker::Write | FindingBlocker::Deploy))
        })
    })
}

/// What the goal judge is told when it looks again after the owner marked the change live.
const LIVE_RECHECK_BRIEF: &str = "
THIS IS A SECOND LOOK, AFTER THE CHANGE WAS MARKED LIVE
The chain finished earlier, its change was merged, and the owner (or his deploy system) has since said it is live. An earlier check left a key result unverified because it showed only once deployed (the findings above, with blocker write or deploy). Check it now against the live system with your tools (a request to the live site, a query, a log). Nothing is the owner's to settle any more: a key result you confirm is met, with the command and what it showed as evidence; one you cannot confirm makes your verdict request_changes, saying what you tried. You change nothing: the team's work is closed.
";

/// What the goal judge is told when it looks again after the merge.
const RECHECK_BRIEF: &str = "
THIS IS A SECOND LOOK, AFTER THE MERGE
The chain finished earlier and its change was merged since. An earlier check left a key result unverified because it showed only once merged or deployed (the findings above, with blocker write or deploy). Check it now against what is live, with your tools: the merged code, and the real system when you can reach it (a request to the live site, a query, a log). A deploy takes minutes after a merge: when the effect is not there yet, try again for a few minutes before you call it unverified, and say what you tried. A key result you can now confirm is met, with the command and what it showed as evidence; one you still cannot confirm stays unverified with its blocker. You change nothing: the team's work is closed.
";

impl Shipping {
    /// Hires a goal judge for a finished chain whose change was merged (ADR-0039), or marked live (ADR-0067), after the
    /// verdict that closed it, when that verdict left something unverified that only the merge or the deploy could settle:
    /// the judge looks again at what shows now, and its verdict is the chain's new one. A chain that waits on nothing of
    /// the kind, or is not finished, is moved on from.
    pub(crate) async fn hire_goal_recheck(&self, h: &SignalHire, v: &FactValue) -> Result<()> {
        let id = v.chain.clone();
        let ch = match self.store.read(move |r| r.chain(&id)).await {
            Err(store::Error::NotFound) => return Err(Error::NothingToDo),
            other => other?,
        };
        if !self.cfg.goal_check || verifying(&ch) || *ch.status() != ChainStatus::Done || ch.finishing.is_some() {
            return Err(Error::NothingToDo);
        }
        let id = ch.id.clone();
        let approvals = self.store.read(move |r| r.approvals(&id)).await?;
        if !awaits_merge(&approvals) {
            return Err(Error::NothingToDo);
        }
        let (after, more) = if h.d.firing.key == keys::chain_marked_live() {
            ("the live mark", LIVE_RECHECK_BRIEF)
        } else {
            ("the merge", RECHECK_BRIEF)
        };
        let (judge, brief, _) = self.judge_of(&ch, &ch.summary, more).await?;
        let cmd = HireRecheckCmd {
            judge: judge.id.clone(),
            name: judge.name.clone(),
            after: after.to_owned(),
            brief,
            deadline: goal_deadline(&ch.id, h.now),
        };
        let w = h.records(&format!("hired the goal judge to check again after {after}"), judge);
        let r = self.apply(&ch.id, ChainCmd::HireRecheck(cmd), w).await;
        after_hire(r.as_ref().is_ok_and(|a| a.changed), r.map(|_| ()))
    }
}
