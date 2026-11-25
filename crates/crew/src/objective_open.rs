//! What an applied objective opens: the chain of each key result an employee answers for, and a word to the Chairs of
//! the chains that serve a key result the owner retired.

use model::{Accountable, Chain, ChainOrigin, ChainStatus, DecisionKind, Employee, KeyResult, Objective, Serves};

use crate::Crew;
use crate::employee_change::answerer;
use chains::apply::Records;
use chains::cap_names::CAP_PROPOSE;
use chains::chain_core::first_line;
use chains::lifecycle::NewChain;
use chains::outbox::{by_remit, new_message};
use chains::step::Decided;
use chains::{Error, Result};

impl Crew {
    /// Opens the chain of each key result of an applied objective whose owner is an employee, once the objective is
    /// committed: one chain per key result, whose one job is to open that key result's chains with propose_chain, each
    /// serving it. The wake is Remit's own turn for the employee, not a chain another proposed, so the rule that a proposed
    /// chain cannot propose does not reach it. A key result the owner answers for, or whose employee has retired
    /// (`answerer`), starts nothing: the owner opens its chains through his Assistant. A retired key result starts nothing
    /// either: each chain still serving it has its Chair told once, and none is stopped.
    pub(crate) async fn wake_key_result_owners(&self, obj: &Objective) {
        let staff = match self.store.read(|r| r.employees()).await {
            Ok(staff) => staff,
            Err(err) => {
                tracing::error!(objective = %obj.id, %err, "wake key result owners");
                return;
            }
        };
        for kr in &obj.key_results {
            if kr.retired {
                self.tell_chairs_of_retirement(obj, kr).await;
                continue;
            }
            let owner = answerer(&kr.owner, &staff);
            if owner.is_owner() {
                continue;
            }
            let queued = self
                .queue_chain(NewChain {
                    title: format!("Open the chains of {}", first_line(&kr.text, 60)),
                    goal: wake_goal(obj, kr),
                    origin: ChainOrigin::Owner,
                    by: by_remit(),
                    owner: owner.clone(),
                    capabilities: vec![CAP_PROPOSE.to_owned()],
                    ..NewChain::default()
                })
                .await;
            if let Err(err) = queued {
                tracing::error!(objective = %obj.id, key_result = %kr.id, %owner, %err, "wake key result owner");
            }
        }
        self.kick();
    }

    /// Tells the Chair of each unfinished chain that serves retired key result kr to finish what is under way and open
    /// nothing new for it. The message and the decision that records it are stored together, and a Chair that has the
    /// decision is not told again, so applying the objective twice tells once. A chain not yet started has no Chair to
    /// tell.
    async fn tell_chairs_of_retirement(&self, obj: &Objective, kr: &KeyResult) {
        let chains = match self.store.read(|r| r.chains()).await {
            Ok(chains) => chains,
            Err(err) => {
                tracing::error!(objective = %obj.id, key_result = %kr.id, %err, "tell the chairs of a retired key result");
                return;
            }
        };
        let note = format!("Key result {} retired: told the Chair", kr.id);
        let serves = Serves {
            objective: obj.id.clone(),
            key_result: kr.id.clone(),
        };
        for ch in chains {
            if *ch.status() == ChainStatus::Done || ch.chair_id.is_empty() || ch.serves.as_ref() != Some(&serves) {
                continue;
            }
            if let Err(err) = self.tell_chair_of_retirement(&ch, obj, kr, &note).await {
                tracing::error!(chain = %ch.id, key_result = %kr.id, %err, "tell the chair of a retired key result");
            }
        }
    }

    /// Sends the retirement of kr to the Chair of ch unless a decision of ch says it was sent.
    async fn tell_chair_of_retirement(&self, ch: &Chain, obj: &Objective, kr: &KeyResult, note: &str) -> Result<()> {
        let (chain_id, chair_id) = (ch.id.clone(), ch.chair_id.clone());
        let (decisions, chair) = self
            .store
            .read(move |r| Ok::<_, Error>((r.decisions(&chain_id)?, r.card(&chair_id)?)))
            .await?;
        if decisions
            .iter()
            .any(|d| d.kind == DecisionKind::Event && d.text == note)
        {
            return Ok(());
        }
        let text = format!(
            "[key result retired] The owner retired key result {} ({}) of objective {}. Finish what is under way for it \
             and open nothing new for it: no new chain, no new work. The chain is not stopped.",
            kr.id,
            first_line(&kr.text, 120),
            obj.id
        );
        let records = Records {
            decisions: vec![Decided {
                kind: DecisionKind::Event,
                text: note.to_owned(),
                why: kr.id.clone(),
                card_id: chair.id.clone(),
                by: by_remit(),
            }],
            ..Records::default()
        };
        self.send(
            &chair,
            new_message(&chair.id, "Key result retired", &text, by_remit()),
            records,
        )
        .await?;
        Ok(())
    }

    /// Refuses chain home opening work for key result s (a chain serving it, or a key result refining it) unless the
    /// employee that answers for s is the one home works for, or s is the key result home serves or refines it and home
    /// works for an employee or on the owner's direct request (ADR-0115): a key result's chains are opened by its owner,
    /// never by another's, and a key result the owner answers for in an objective he committed to is refined by his
    /// Assistant, never by a chain serving it.
    pub async fn may_open_for(&self, home: &str, s: &Serves) -> Result<()> {
        let (home, s) = (home.to_owned(), s.clone());
        self.store
            .read(move |r| {
                let proposing = r.chain(&home)?;
                let Some(serving) = proposing.serves else {
                    return Err(Error::refused("this chain serves no key result, so it opens no chain for one"));
                };
                let staff = r.employees()?;
                let mine = key_result_owner(r.key_result(&serving.objective, &serving.key_result), &serving, &staff)?;
                let under_mine = serving.objective == s.objective
                    && refines(&r.key_results(&s.objective)?, &s.key_result, &serving.key_result);
                if under_mine && (!mine.is_owner() || r.objective(&serving.objective)?.task) {
                    return Ok(());
                }
                let served = key_result_owner(r.key_result(&s.objective, &s.key_result), &s, &staff)?;
                if mine.is_owner() || mine != served {
                    return Err(Error::refused(format!(
                        "only the owner of key result {} opens chains for it, and this chain is not worked for that owner",
                        s.key_result
                    )));
                }
                Ok(())
            })
            .await
    }
}

/// Whether key result id of tree is ancestor or refines it, through any number of parents.
fn refines(tree: &[KeyResult], id: &str, ancestor: &str) -> bool {
    model::lineage(tree, id).contains(&ancestor)
}

/// Who answers for key result s now, kr as the store read it.
pub fn key_result_owner(kr: store::Result<KeyResult>, s: &Serves, staff: &[Employee]) -> Result<Accountable> {
    let kr =
        kr.map_err(|err| Error::refused(format!("objective {} key result {}: {err}", s.objective, s.key_result)))?;
    Ok(answerer(&kr.owner, staff))
}

/// The one instruction of a key result owner's wake.
fn wake_goal(obj: &Objective, kr: &KeyResult) -> String {
    format!(
        "KEY RESULT {}: {}\nOBJECTIVE {}: {}\nHow it is checked: {}.\n\nYou answer for this key result. Open the chains \
         that work toward it. A key result runs one chain at a time, so when it has parts that can be worked at once, \
         refine it into a key result per part with refine_key_result (parent {:?}) and open one chain per part; else \
         open one chain for it. Call propose_chain with serves {{objective: {:?}, keyResult: <the key result it works \
         on>}} and a goal that says what that chain does toward it. This chain's own result \
         is that those chains are queued, and each is judged by its own check: declare that with state_check (kind submit, \
         its reason that this chain only opens chains). Do no other work, and finish_chain once the chains are queued.",
        kr.id,
        kr.text,
        obj.id,
        obj.outcome,
        chains::goal::check_words(kr.check.as_ref()),
        kr.id,
        obj.id
    )
}
