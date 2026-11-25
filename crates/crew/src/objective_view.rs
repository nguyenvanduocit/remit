//! The objectives as the owner is shown them, and where a key result stands as the rules that open work for it read it.

use std::collections::{BTreeMap, HashMap};

use model::{Chain, CheckKind, KRStatus, KeyResult, ObjectiveView, Serves, SignalKey, Time};

use crate::Crew;
use crate::employee_change::{answering_key_result, answering_objective};
use crate::objective::{ServedChain, key_result_status, reading_status, retired_in, tree_status};
use chains::admit::{outcomes_of, stall_on};
use chains::chain_core::optional;
use chains::goal::goal_approvals;
use chains::{Error, Result};

/// The chains that serve each key result, by objective id and key result id.
type Serving = HashMap<(String, String), Vec<ServedChain>>;

/// The chains that serve a key result, each with its goal judges' verdicts.
fn serving_of(r: &store::Reader<'_>, chains: &[Chain]) -> Result<Serving> {
    let mut serving = Serving::new();
    for ch in chains {
        let Some(serves) = ch.serves.clone() else {
            continue;
        };
        let approvals = r.approvals(&ch.id)?;
        let served = ServedChain {
            approvals: goal_approvals(&approvals).into_iter().cloned().collect(),
        };
        serving
            .entry((serves.objective, serves.key_result))
            .or_default()
            .push(served);
    }
    Ok(serving)
}

/// Where each key result of objective's tree stands, by id: one with a check by `key_result_status` over the chains that
/// serve it and the verdicts of their goal judges, or for a reading what its signal says (`reading_status`); one with no
/// check by its children (`tree_status`). Derived on every read and never stored.
fn tree_status_of(
    r: &store::Reader<'_>,
    objective: &str,
    tree: &[KeyResult],
    serving: &Serving,
    now: Time,
) -> Result<BTreeMap<String, KRStatus>> {
    let mut own = BTreeMap::new();
    for kr in tree {
        let reading = kr.check.as_ref().filter(|c| c.kind == CheckKind::Reading);
        let s = if let Some(c) = reading {
            let head = optional(r.last_reading(&SignalKey(c.signal.clone())))?;
            reading_status(c, head.as_ref(), now)
        } else {
            let at = (objective.to_owned(), kr.id.clone());
            key_result_status(kr, serving.get(&at).map(Vec::as_slice).unwrap_or_default())
        };
        own.insert(kr.id.clone(), s);
    }
    Ok(tree_status(tree, |kr| {
        own.get(&kr.id).copied().unwrap_or(KRStatus::Unserved)
    }))
}

/// Why no new chain opens for key result s, read in r at now: its objective is not active, or it is met, or retired (by
/// the owner, with its parent, or once its parent is met); none when work may open for it (ADR-0115).
pub fn closed_for_work(r: &store::Reader<'_>, s: &Serves, now: Time) -> Result<Option<String>> {
    let obj = r.objective(&s.objective)?;
    if obj.status != model::ObjectiveStatus::Active {
        return Ok(Some(format!("objective {} is {}", s.objective, obj.status)));
    }
    let tree = r.key_results(&s.objective)?;
    if !tree.iter().any(|kr| kr.id == s.key_result) {
        return Err(Error::refused(format!(
            "objective {} has no key result {}",
            s.objective, s.key_result
        )));
    }
    let chains = r.chains()?;
    let serving = serving_of(r, &chains)?;
    let status = tree_status_of(r, &s.objective, &tree, &serving, now)?;
    if status.get(&s.key_result) == Some(&KRStatus::Met) {
        return Ok(Some(format!("key result {} is met", s.key_result)));
    }
    if retired_in(&tree, &status).contains(&s.key_result) {
        return Ok(Some(format!("key result {} is retired", s.key_result)));
    }
    Ok(None)
}

impl Crew {
    /// The stored objectives, newest first, each with its tree of key results, where each stands (`tree_status_of`) and
    /// why each one held by a stall takes no new chain (`stall_on`, ADR-0115). A key result shows retired when refinement retires it
    /// (`retired_in`). Who answers is read as `answering_objective` says: the
    /// owner wherever an employee has retired.
    pub async fn objective_views(&self) -> Result<Vec<ObjectiveView>> {
        let now = self.now();
        self.store
            .read(move |r| {
                let objectives = r.objectives()?;
                let mut trees = r.every_key_result()?;
                let staff = r.employees()?;
                let chains = r.chains()?;
                let outcomes = outcomes_of(r, chains.iter())?;
                let serving = serving_of(r, &chains)?;
                let mut views = Vec::with_capacity(objectives.len());
                for obj in objectives {
                    let mut tree = trees.remove(&obj.id).unwrap_or_default();
                    let status = tree_status_of(r, &obj.id, &tree, &serving, now)?;
                    let stalls = tree
                        .iter()
                        .filter_map(|kr| stall_on(&obj.id, &tree, &kr.id, &outcomes).map(|why| (kr.id.clone(), why)))
                        .collect();
                    let retired = retired_in(&tree, &status);
                    let mut objective = answering_objective(obj, &staff);
                    objective.key_results = Vec::new(); // the view carries the whole tree beside it
                    for kr in &mut tree {
                        answering_key_result(kr, &staff);
                        // shown retired as refinement retires it too: with its parent, or once its parent is met
                        kr.retired = retired.contains(&kr.id);
                        // a check that is not valid confirms nothing (ADR-0068), and the contract carries only a check
                        // of its kind with the fields that kind needs: it shows none
                        kr.check = kr.check.take().filter(|c| c.valid().is_ok());
                    }
                    views.push(ObjectiveView {
                        objective,
                        key_results: tree,
                        status,
                        stalled: stalls,
                    });
                }
                Ok(views)
            })
            .await
    }
}
