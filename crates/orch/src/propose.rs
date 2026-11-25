//! A Chair proposing a chain of its own (the propose capability): the proposal queues as a `work` or a `chore` chain,
//! within what the Chair's own chain may open (ADR-0096 puts it with the orchestrator: it reads kinds, objectives and
//! capabilities). Proposals are deduplicated by the key result they serve (ADR-0115): one that names none serves a new
//! key result refining the proposer's own, and a key result met, retired or with a start already queued takes none.

use model::{Card, Chain, ChainKind, ChainStatus, Serves};
use schemars::JsonSchema;
use serde::Deserialize;

use chains::{Error, Result};

use crate::orch::Orchestrator;
use chains::lifecycle::NewChain;
use chains::schedule::verifying;
use crew::objective_view::closed_for_work;
use model::ChainOrigin;
use model::DecisionKind;
use model::by_agent;

/// What propose_chain takes: a chain found by a chain that looks for work.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposeInput {
    /// short title in the owner's language, e.g. 'Fix swallowed panic in the customerfns SQS consumers'
    pub title: String,
    /// self-contained goal for a new chain: the repository; observations, not conclusions (what each cited file:line
    /// does, what you saw in logs or data), every number and identifier quoted from a file:line you read or a command you
    /// ran; for a bug, the path from trigger to symptom as numbered hops with file:line, 'not read' for any hop you did not
    /// open; the expected result (a landed change with tests); constraints; last, 'Suspected cause (unverified)' and 'Fix idea
    /// (unverified)'
    pub goal: String,
    /// why this is the most valuable thing to do now, in one or two sentences
    pub why: String,
    /// work (the default) or chore: a small, well-understood change whose changes pass QC alone. A playbook that fits is
    /// named in the goal, never here
    #[serde(default)]
    pub kind: ChainKind,
    /// the key result the chain works toward, {objective, keyResult}: yours, one refining yours, or one the employee you
    /// work for answers for; leave out and the chain serves a new key result refining the one your chain serves, made from
    /// its title and goal
    #[serde(default)]
    pub serves: Option<ServesInput>,
    /// true to open the chain as one attempt competing with others at the same key result: only for a key result whose
    /// check is a command, named in serves, and at most as many attempts as its share. Propose each attempt the same way;
    /// their approved versions wait until a judge runs the check on each and picks one to land, and the others do not
    #[serde(default)]
    pub attempt: bool,
}

/// The key result a proposed chain works toward.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ServesInput {
    /// the objective's id
    pub objective: String,
    /// the key result's id within it
    pub key_result: String,
}

/// Refuses a start for key result s while one of chains is queued for it: the refusal names that chain, whose Chair the
/// proposer messages instead.
fn start_queued(chains: &[Chain], s: &Serves) -> Result<()> {
    match chains
        .iter()
        .find(|c| *c.status() == ChainStatus::Queued && c.serves.as_ref() == Some(s))
    {
        Some(c) => Err(Error::refused(format!(
            "key result {} already has a chain queued, {} ({}): message its Chair with what you would add instead",
            s.key_result, c.id, c.title
        ))),
        None => Ok(()),
    }
}

impl Orchestrator {
    /// Queues a new chain found by a chain that looks for work, and says what else refines the key result it serves;
    /// the scheduler starts it.
    pub async fn propose(&self, chair: &Card, input: ProposeInput) -> Result<(Chain, String)> {
        if input.title.trim().is_empty() || input.goal.trim().is_empty() {
            return Err(Error::refused("title and goal are required"));
        }
        let home_id = chair.home.clone();
        let home = self.store.read(move |r| r.chain(&home_id)).await?;
        if verifying(&home) {
            return Err(Error::refused("a verification chain opens no chain"));
        }
        if let Some(why) = capability::kinds::may_not_start(&ChainOrigin::Proposed, input.kind) {
            return Err(Error::refused(why));
        }
        let serves = match input.serves {
            Some(s) => {
                let s = Serves {
                    objective: s.objective,
                    key_result: s.key_result,
                };
                self.crew.may_open_for(&chair.home, &s).await?;
                s
            }
            None => {
                self.crew
                    .proposal_key_result(&home, by_agent(&chair.id), &input.title, &input.goal)
                    .await?
            }
        };
        let (at, now) = (serves.clone(), self.now());
        if let Some(why) = self.store.read(move |r| closed_for_work(r, &at, now)).await? {
            return Err(Error::refused(format!("{why}: no new chain opens for it")));
        }
        let opening = if input.attempt {
            Some(self.shipping.opening_attempt(&serves).await?)
        } else {
            None
        };
        let (at, round) = (serves.clone(), opening.as_ref().map(|o| o.round.clone()));
        let attempt = round.as_ref().map(|r| r.id.clone());
        // in the transaction that queues it: two proposals at once cannot both find the key result free, and an attempt's
        // round is written over the revision read, so one joining a round a judge started on, or that another attempt
        // joined meanwhile, is refused (Error::Conflict) and proposed again
        let admit = move |tx: &store::Tx<'_>| -> Result<()> {
            match round {
                Some(r) => tx.put_round(r).map(|_| ()).map_err(Error::from),
                None => start_queued(&tx.chains()?, &at),
            }
        };
        let goal = match &opening {
            Some(o) => format!("{}{}", input.goal, o.goal),
            None => input.goal,
        };
        let ch = self
            .queue_chain_if(
                NewChain {
                    title: input.title.clone(),
                    goal,
                    kind: input.kind,
                    origin: ChainOrigin::Proposed,
                    proposed_by: chair.home.clone(),
                    by: by_agent(&chair.id),
                    serves: Some(serves.clone()),
                    attempt,
                    ..NewChain::default()
                },
                admit,
            )
            .await?;
        let what = match &opening {
            Some(o) => format!(
                "Queue {} chain, an attempt of round {}: {}",
                ch.kind, o.round.number, input.title
            ),
            None => format!("Queue {} chain: {}", ch.kind, input.title),
        };
        self.decide(
            &chair.home,
            DecisionKind::Propose,
            &what,
            &input.why,
            &chair.id,
            by_agent(&chair.id),
        )
        .await;
        self.kick();
        // the chain is queued: a failed read of what else refines its key result only leaves the reply without it
        let siblings = self.crew.siblings_of(&serves).await.unwrap_or_else(|err| {
            tracing::error!(chain = %ch.id, key_result = %serves.key_result, %err, "read what else refines it");
            String::new()
        });
        Ok((ch, siblings))
    }
}
