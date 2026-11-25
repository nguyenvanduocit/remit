//! A hired judge may open a verification chain when its check needs work only a chain can do: reproduce a defect, run a
//! scenario, read what a team of agents reads (ADR-0032). The chain answers to the owner, its Chair works at the judge's
//! own ceiling so it cannot change what it checks, it has no judge (`request_finish` finishes it at its Chair's word) and
//! it opens no chain, so judging does not recurse. A check opens one such chain: the chain names the judge card that
//! opened it.

use model::{ActorKind, Card, CardRole, Ceiling, Chain, ChainOrigin, ChainStatus, EmployeeStatus, JudgeKind, Title};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Shipping;
use chains::chain_core::first_line;
use chains::lifecycle::NewChain;
use chains::schedule::verifying;
use chains::tools::{Effect, Tools};
use chains::{Error, Result};

/// What open_verification is asked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpenVerificationInput {
    /// what to verify, self-contained: the claim to check, where the result is (paths of the worktrees you judge), the
    /// steps to run, and what each outcome would show
    pub request: String,
}

/// Gives open_verification to a goal judge that is a hired employee; the tool itself refuses a card that is not at its
/// check as a judge employee.
pub fn add_verification_tool(svc: &std::sync::Arc<Shipping>, t: &mut Tools, card: &Card) {
    if card.employee.is_empty() {
        return;
    }
    let card_id = card.id.clone();
    t.serving(svc).add(
        "open_verification", Effect::Writes,
        "Open a verification chain for work only a chain can do: reproduce a defect, run a scenario, read what a team reads. A Chair and the agents it hires, who read and run but change nothing, answer to the owner; the chain has no judge of its own and opens no chain. One per check: a second call names the chain already open. Its summary is what the Chair reports when it finishes.",
        move |o, i: OpenVerificationInput| {
            let card_id = card_id.clone();
            async move {
                let me = o.store.read(move |r| r.card(&card_id)).await?;
                let ch = o.open_verification(&me, &i.request).await?;
                Ok(format!("Verification chain {} is open: {}", ch.id, ch.title))
            }
        },
    );
}

impl Shipping {
    /// open_verification: judge, a hired judge whose check of its chain is open, asks for a verification chain.
    pub(crate) async fn open_verification(&self, judge: &Card, request: &str) -> Result<Chain> {
        let request = request.trim();
        if request.is_empty() {
            return Err(Error::refused("request is required"));
        }
        if CardRole::of(judge) != CardRole::Judge(JudgeKind::Goal) || judge.employee.is_empty() {
            return Err(Error::refused(
                "only a hired judge at its check opens a verification chain",
            ));
        }
        let emp_id = judge.employee.clone();
        let emp = self
            .store
            .read(move |r| r.employee(&emp_id))
            .await
            .map_err(|e| Error::refused(format!("the employee of card {}: {e}", judge.id)))?;
        if emp.title != Title::Judge || emp.status != EmployeeStatus::Active {
            return Err(Error::refused("only a judge at work opens a verification chain"));
        }
        let home = judge.home.clone();
        let parent = self.store.read(move |r| r.chain(&home)).await?;
        if parent.finishing.as_ref().is_none_or(|f| f.judge != judge.id) {
            return Err(Error::refused(
                "your check is over: a verification chain is opened while you judge",
            ));
        }
        let chains = self.store.read(|r| r.chains()).await?;
        if let Some(opened) = opened_verification(&chains, &judge.id) {
            return Ok(opened.clone());
        }
        let ch = self
            .queue_chain(NewChain {
                title: format!("Verify: {}", first_line(request, 70)),
                goal: verification_goal(&parent, request),
                origin: ChainOrigin::Verification,
                by: model::by_agent(&judge.id),
                capabilities: reading_capabilities(&parent),
                ..NewChain::default()
            })
            .await?;
        self.kick();
        Ok(ch)
    }

    /// Whether the check of judge stays open although its turn ended without a verdict.
    pub(crate) async fn holds_for_verification(&self, judge: &Card) -> bool {
        let home = judge.home.clone();
        let Ok(ch) = self.store.read(move |r| r.chain(&home)).await else {
            return false;
        };
        if ch
            .finishing
            .as_ref()
            .is_none_or(|f| f.judge != judge.id || f.approval_id.is_some())
        {
            return false;
        }
        match self.store.read(|r| r.chains()).await {
            Ok(chains) => awaits_verification(&chains, judge),
            Err(err) => {
                tracing::error!(chain = %judge.home, card = %judge.id, %err, "verification chains of a judge");
                false
            }
        }
    }

    /// Sends the summary of finished verification chain vch to the judge that opened it, as one message that starts the
    /// judge's next turn, while that judge's check is still open: a check that ended has no one to tell.
    pub async fn tell_judge_what_verification_found(&self, vch: &Chain, summary: &str) {
        let Some(by) = vch.by.as_ref().filter(|_| verifying(vch)) else {
            return;
        };
        let who = by.who.clone();
        let judge = match self.store.read(move |r| r.card(&who)).await {
            Ok(k) => k,
            Err(err) => {
                tracing::error!(chain = %vch.id, judge = %by.who, %err, "the judge of a verification chain");
                return;
            }
        };
        let home = judge.home.clone();
        let Ok(parent) = self.store.read(move |r| r.chain(&home)).await else {
            return;
        };
        if parent
            .finishing
            .as_ref()
            .is_none_or(|f| f.judge != judge.id || f.approval_id.is_some())
        {
            return;
        }
        let text = format!("[verification result] {}: {summary}", vch.id);
        if let Err(err) = self
            .deliver_now(&judge.id, "Verification result", &text, model::by_remit(""))
            .await
        {
            tracing::error!(chain = %vch.id, judge = %judge.id, %err, "tell the judge what the verification found");
        }
    }

    /// The ceiling of the judge that opened verification chain ch.
    pub async fn opener_ceiling(&self, ch: &Chain) -> Result<Ceiling> {
        let Some(by) = ch.by.as_ref().filter(|b| b.kind == ActorKind::Agent) else {
            return Err(Error::refused(format!(
                "verification chain {} names no judge that opened it",
                ch.id
            )));
        };
        let who = by.who.clone();
        let judge = self
            .store
            .read(move |r| r.card(&who))
            .await
            .map_err(|e| Error::refused(format!("the judge that opened chain {}: {e}", ch.id)))?;
        let emp_id = judge.employee.clone();
        let emp = self
            .store
            .read(move |r| r.employee(&emp_id))
            .await
            .map_err(|e| Error::refused(format!("the employee of judge {}: {e}", judge.id)))?;
        Ok(emp.ceiling)
    }
}

/// The verification chain the judge card judge_id opened, if it did.
fn opened_verification<'a>(chains: &'a [Chain], judge_id: &str) -> Option<&'a Chain> {
    let by = model::by_agent(judge_id);
    chains.iter().find(|c| verifying(c) && c.by.as_ref() == Some(&by))
}

/// Whether judge's check is open on a verification chain that has not finished: its turn may end without a verdict,
/// because the chain's summary, which finish sends it, starts its next one. The chain's own stall rules bound the wait.
pub(crate) fn awaits_verification(chains: &[Chain], judge: &Card) -> bool {
    opened_verification(chains, &judge.id).is_some_and(|c| *c.status() != ChainStatus::Done)
}

/// The goal of a verification chain: the judge's request, and where the result it checks is.
fn verification_goal(parent: &Chain, request: &str) -> String {
    let mut b = format!(
        "A judge asks you to verify a result of chain {:?}. You and the agents you hire read and run; you change nothing, and you open no chain. Finish with what you found, each claim with its evidence.\n\nREQUEST\n{request}\n\nWHERE THE RESULT IS\n- the chain's directory: {}\n",
        parent.title, parent.workdir
    );
    for rw in &parent.repos {
        b.push_str(&format!("- {}: worktree {}, branch {}\n", rw.repo, rw.path, rw.branch));
    }
    b
}

/// The capabilities of chain ch that change nothing: what a verification chain reads with.
fn reading_capabilities(ch: &Chain) -> Vec<String> {
    let writing = capability::capabilities::writes(&ch.capabilities);
    ch.capabilities
        .iter()
        .filter(|id| !writing.contains(id))
        .cloned()
        .collect()
}
