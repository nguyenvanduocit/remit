//! Key results that refine key results (ADR-0115): whoever answers for a key result writes what refines it with
//! refine_key_result (that employee's chain, or the Assistant for a key result the owner answers for), applied at once
//! and recorded as a `Change` the owner reads and overturns: it changes how a result is reached, not what was promised,
//! and the parent's check stays the commitment. A chain refines under the key result it serves only when it works for an
//! employee or on the owner's direct request (`Crew::may_open_for`): a key result the owner answers for in an objective
//! he committed to is refined by the Assistant. The Assistant lifts the progress rail off a stalled key result with
//! lift_key_result_stall, saying why.

use std::sync::Arc;

use model::{Accountable, Actor, ChangeKind, Check, CheckKind, KeyResult, Lift, Objective, Serves};
use schemars::JsonSchema;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::Crew;
use crate::employee_change::{answerer, key_result_names_who_answers};
use crate::objective_view::closed_for_work;
use chains::admit::{outcomes_of, stall_on, stalled};
use chains::chain_core::{first_line, optional};
use chains::changes::{Authorship, record_change};
use chains::goal::finding_lines;
use chains::tools::{Effect, SelfCard, Tools};
use chains::{Error, Result};

/// What refine_key_result takes.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RefineInput {
    /// the objective's id
    pub objective: String,
    /// the id of the key result this one refines: one you answer for
    pub parent: String,
    /// the id of a key result that refines parent, to change it; empty for a new one
    #[serde(default)]
    pub id: String,
    /// what is true when it is met, in the customer's words; required for a new one, empty keeps it on a change
    #[serde(default)]
    pub text: String,
    /// {kind, ...}: a command (repo, name) or a reading (signal, condition, window, expiry) that can fail, or submit
    /// (how) with the reason no executable check can exist; leave out for a new one met when every key result refining
    /// it is, or to keep the check on a change
    #[serde(default)]
    pub check: Option<Check>,
    /// who answers for it: an active chair employee's id or owner; empty is who answers for it now, or for parent
    #[serde(default)]
    pub owner: String,
    /// who judges it: an active judge employee's id or owner, outside its owner's team; empty is who judges it now, or
    /// parent
    #[serde(default)]
    pub judge: String,
    /// true to retire it: its chains finish, no new chain opens for it, and what refines it retires with it
    #[serde(default)]
    pub retire: bool,
    /// why this refinement, in one or two sentences: the owner reads it
    pub why: String,
}

/// A key result, named.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LiftInput {
    /// the objective's id
    pub objective: String,
    /// the stalled key result's id
    pub key_result: String,
    /// why it may take new chains again: what is different now; the owner reads it
    pub why: String,
}

/// Writes key result next of objective in tx, as w at at, with the `Change` from what was stored under its id.
pub fn write_key_result(
    tx: &store::Tx<'_>,
    w: &Authorship,
    at: model::Time,
    objective: &str,
    next: KeyResult,
) -> Result<()> {
    let cur = optional(tx.key_result(objective, &next.id))?;
    let name = format!("{objective}/{}", next.id);
    record_change(tx, w, at, ChangeKind::KeyResult, &name, cur.as_ref(), Some(&next))?;
    tx.put_key_result(objective, next)?;
    Ok(())
}

/// The id of a key result written for words under seed (the proposing chain, or the parent refined): the same for the
/// same seed and words, so a write made again finds the one it made.
fn key_result_id(seed: &str, words: &str) -> String {
    let digest = Sha256::digest(format!("{seed}/{}", words.trim().to_lowercase()).as_bytes());
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("kr_{hex}")
}

/// field as written, or keep when it is empty.
fn or_kept(field: &str, keep: Accountable) -> Accountable {
    if field.is_empty() {
        keep
    } else {
        Accountable(field.to_owned())
    }
}

/// The key results that refine parent of objective, each with the chains that serve it, as the writer reads them before
/// it writes, so a similar intent under another key result is seen.
fn siblings_words(r: &store::Reader<'_>, objective: &str, parent: &str) -> Result<String> {
    let tree = r.key_results(objective)?;
    let chains = r.chains()?;
    let mut lines = Vec::new();
    for kr in tree.iter().filter(|kr| kr.parent.as_deref() == Some(parent)) {
        let serving: Vec<String> = chains
            .iter()
            .filter(|c| {
                c.serves
                    .as_ref()
                    .is_some_and(|s| s.objective == objective && s.key_result == kr.id)
            })
            .map(|c| format!("{} {} ({})", c.id, first_line(&c.title, 60), c.status()))
            .collect();
        let retired = if kr.retired { ", retired" } else { "" };
        let served = if serving.is_empty() {
            "no chain".to_owned()
        } else {
            serving.join("; ")
        };
        lines.push(format!("- {} {}{retired}: {served}", kr.id, first_line(&kr.text, 80)));
    }
    if lines.is_empty() {
        return Ok(format!("Nothing else refines {parent}."));
    }
    Ok(format!("What refines {parent} now:\n{}", lines.join("\n")))
}

impl Crew {
    /// Gives the Chair of a chain that opens work refine_key_result, acting as me.
    pub fn add_key_result_chair_tools(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let me = me.clone();
        t.serving(self).add(
            "refine_key_result",
            Effect::Writes,
            REFINE_WORDS,
            move |o, i: RefineInput| {
                let me = me.clone();
                async move {
                    let card = me.read(&o).await?;
                    o.may_open_for(
                        &card.home,
                        &Serves {
                            objective: i.objective.clone(),
                            key_result: i.parent.clone(),
                        },
                    )
                    .await?;
                    o.refine_key_result(model::by_agent(&card.id), i).await
                }
            },
        );
    }

    /// Gives the Assistant, acting as by, refine_key_result for the key results the owner answers for, and
    /// lift_key_result_stall.
    pub fn add_key_result_assistant_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        let who = by.clone();
        t.serving(self).add(
            "refine_key_result",
            Effect::Writes,
            REFINE_WORDS,
            move |o, i: RefineInput| {
                let by = who.clone();
                async move {
                    o.owner_answers_for(&i.objective, &i.parent).await?;
                    o.refine_key_result(by, i).await
                }
            },
        );
        let who = by.clone();
        t.serving(self).add(
            "lift_key_result_stall", Effect::Writes,
            "Lift the progress rail off a stalled key result (read_objective's tree says which are and why: the last 3 chains serving it finished without progress). It takes new chains again, and stalls again after 3 more without progress. Say why in `why`, what is different now: the owner reads it and may overturn it. Prefer refining the key result (refine_key_result) or changing how it is worked when the same work would only fail again.",
            move |o, i: LiftInput| {
                let by = who.clone();
                async move { o.lift_key_result_stall(by, i).await }
            },
        );
    }

    /// Refuses key result id of objective unless the owner answers for it: the Assistant refines only what is his.
    async fn owner_answers_for(&self, objective: &str, id: &str) -> Result<()> {
        let (o, k) = (objective.to_owned(), id.to_owned());
        let (kr, staff) = self
            .store
            .read(move |r| Ok::<_, Error>((r.key_result(&o, &k)?, r.employees()?)))
            .await
            .map_err(|e| Error::refused(format!("key result {objective}/{id}: {e}")))?;
        if !answerer(&kr.owner, &staff).is_owner() {
            return Err(Error::refused(format!(
                "key result {id} is answered by {}: its own chains refine it",
                kr.owner
            )));
        }
        Ok(())
    }

    /// Writes the key result input says, refining input.parent, as by, and says what else refines that parent. Read,
    /// checked and written in one transaction, and written again to the same end: a new one's id comes from its parent
    /// and words. The caller has checked that by may write for the parent.
    pub(crate) async fn refine_key_result(&self, by: Actor, input: RefineInput) -> Result<String> {
        let now = self.now();
        if input.why.trim().is_empty() {
            return Err(Error::refused("why is required: the owner reads it"));
        }
        let check = input.check.clone().map(crate::objective::trimmed_check);
        if let Some(c) = &check {
            if c.kind == CheckKind::Unverifiable {
                return Err(Error::refused(
                    "a key result's check can fail: a command, a reading, or submit with the reason no executable check \
                     can exist; leave it out for one met by what refines it",
                ));
            }
            c.valid().map_err(|e| Error::refused(e.to_string()))?;
        }
        let objective = input.objective.clone();
        let w = Authorship { by, approved_by: None };
        let (id, parent, siblings) = self
            .store
            .update(move |tx| {
                let o = input.objective.as_str();
                let refused = |e: store::Error| Error::refused(format!("objective {o}: {e}"));
                let obj = tx.objective(o).map_err(refused)?;
                if obj.status != model::ObjectiveStatus::Active {
                    return Err(Error::refused(format!(
                        "objective {o} is not active: only an active objective's key results are refined"
                    )));
                }
                let parent = tx.key_result(o, &input.parent).map_err(refused)?;
                let text = input.text.trim();
                let id = if input.id.is_empty() {
                    key_result_id(&parent.id, text)
                } else {
                    input.id.clone()
                };
                let cur = optional(tx.key_result(o, &id))?;
                let base = match &cur {
                    Some(c) if c.parent.as_deref() != Some(parent.id.as_str()) => {
                        return Err(Error::refused(format!("key result {id} does not refine {}", parent.id)));
                    }
                    Some(c) if c.retired && !input.retire => {
                        return Err(Error::refused(
                            "a retired key result stays retired: a new one takes a new id",
                        ));
                    }
                    Some(c) => c.clone(),
                    None if !input.id.is_empty() => {
                        return Err(Error::refused(format!("objective {o} has no key result {id}")));
                    }
                    None if text.is_empty() => return Err(Error::refused("text is required for a new key result")),
                    None => {
                        let s = Serves {
                            objective: o.to_owned(),
                            key_result: parent.id.clone(),
                        };
                        if let Some(why) = closed_for_work(tx, &s, now)? {
                            return Err(Error::refused(format!("{why}: nothing new refines it")));
                        }
                        KeyResult {
                            owner: parent.owner.clone(),
                            judge: parent.judge.clone(),
                            ..KeyResult::default()
                        }
                    }
                };
                let next = KeyResult {
                    id: id.clone(),
                    text: if text.is_empty() { base.text } else { text.to_owned() },
                    owner: or_kept(&input.owner, base.owner),
                    judge: or_kept(&input.judge, base.judge),
                    check: check.or(base.check),
                    retired: input.retire,
                    parent: Some(parent.id.clone()),
                    lifted: base.lifted,
                };
                key_result_names_who_answers(&next, &tx.employees()?)?;
                dry_run(tx, &obj, &next, now)?;
                write_key_result(tx, &w, now, o, next)?;
                let siblings = siblings_words(tx, o, &parent.id)?;
                Ok::<_, Error>((id, parent.id, siblings))
            })
            .await?;
        self.tell_objective(&objective).await;
        self.kick();
        Ok(format!("Recorded key result {id}, refining {parent}.\n\n{siblings}"))
    }

    /// The key result a proposal from chain home titled title serves when it names none (ADR-0115): a new key result
    /// refining the one home serves, made from the proposal's title and goal, answered and judged as that one is. It is
    /// written once: a proposal made again with that title finds it. Its check is stated by the Chair of the chain that
    /// serves it.
    pub async fn proposal_key_result(&self, home: &model::Chain, by: Actor, title: &str, goal: &str) -> Result<Serves> {
        let Some(own) = home.serves.clone() else {
            return Err(Error::refused(
                "this chain serves no key result, so it opens no chain for one",
            ));
        };
        let now = self.now();
        let s = own.clone();
        if let Some(why) = self.store.read(move |r| closed_for_work(r, &s, now)).await? {
            return Err(Error::refused(format!("{why}: no new chain opens for it")));
        }
        self.may_open_for(&home.id, &own).await?;
        let id = key_result_id(&home.id, title);
        let (o, p) = (own.objective.clone(), own.key_result.clone());
        let text = format!("{}: {}", title.trim(), first_line(goal, 200));
        let w = Authorship { by, approved_by: None };
        let child = id.clone();
        self.store
            .update(move |tx| {
                if optional(tx.key_result(&o, &child))?.is_some() {
                    return Ok::<_, Error>(());
                }
                let parent = tx.key_result(&o, &p)?;
                let next = KeyResult {
                    id: child,
                    text,
                    owner: parent.owner,
                    judge: parent.judge,
                    parent: Some(parent.id),
                    ..KeyResult::default()
                };
                write_key_result(tx, &w, now, &o, next)
            })
            .await?;
        self.tell_objective(&own.objective).await;
        Ok(Serves {
            objective: own.objective,
            key_result: id,
        })
    }

    /// What else refines the key result s refines, for the proposer to read: empty for a key result the owner committed
    /// to.
    pub async fn siblings_of(&self, s: &Serves) -> Result<String> {
        let s = s.clone();
        self.store
            .read(move |r| {
                let kr = r.key_result(&s.objective, &s.key_result)?;
                match kr.parent {
                    Some(p) => siblings_words(r, &s.objective, &p),
                    None => Ok(String::new()),
                }
            })
            .await
    }

    /// Lifts the progress rail off the stalled key result input names, as by.
    async fn lift_key_result_stall(&self, by: Actor, input: LiftInput) -> Result<String> {
        if input.why.trim().is_empty() {
            return Err(Error::refused("why is required: the owner reads it"));
        }
        let now = self.now();
        let w = Authorship {
            by: by.clone(),
            approved_by: None,
        };
        let (o, id, why) = (
            input.objective.clone(),
            input.key_result.clone(),
            input.why.trim().to_owned(),
        );
        self.store
            .update(move |tx| {
                let kr = tx
                    .key_result(&o, &id)
                    .map_err(|e| Error::refused(format!("key result {o}/{id}: {e}")))?;
                let tree = tx.key_results(&o)?;
                let outcomes = outcomes_of(tx, tx.chains()?.iter())?;
                if stalled(&o, &kr, &tree, &outcomes).is_none() {
                    let held = stall_on(&o, &tree, &id, &outcomes)
                        .map(|why| format!(": what it refines holds it, {why}"))
                        .unwrap_or_default();
                    return Err(Error::refused(format!("key result {id} is not stalled{held}")));
                }
                let next = KeyResult {
                    lifted: Some(Lift { at: now, by, why }),
                    ..kr
                };
                write_key_result(tx, &w, now, &o, next)
            })
            .await?;
        self.tell_objective(&input.objective).await;
        self.kick();
        Ok(format!(
            "Lifted: key result {} takes new chains again, and stalls again after 3 more without progress.",
            input.key_result
        ))
    }
}

/// Refuses kr of obj, read in r at now, when its check cannot be run as stated, as an objective's measure is
/// (ADR-0045): a submit is no measure and passes.
fn dry_run(r: &store::Reader<'_>, obj: &Objective, kr: &KeyResult, now: model::Time) -> Result<()> {
    if kr.check.as_ref().is_none_or(|c| c.kind == CheckKind::Submit) {
        return Ok(());
    }
    let draft = Objective {
        key_results: vec![kr.clone()],
        ..obj.clone()
    };
    let found = crate::objective::checked_objective(r, &draft, now)?;
    let lines = finding_lines(&found.into_iter().filter(|f| f.key_result == kr.id).collect::<Vec<_>>());
    if !lines.is_empty() {
        return Err(Error::refused(format!(
            "the key result is not stored, its check cannot be run as stated:\n- {}",
            lines.join("\n- ")
        )));
    }
    Ok(())
}

const REFINE_WORDS: &str = "Refine a key result you answer for into a key result of its own, or change or retire one that refines it. It is applied at once and recorded as a change the owner reads and may overturn: it changes how the result is reached, not what was promised, and the parent's check stays the commitment. A key result with a check is met when its check is; one with none is met when every key result refining it is. What refines a key result retires with it, and once it is met. Each key result runs one chain at a time, so refining is how work on it goes wide: give each part a key result and open its chain with propose_chain serving it. Read the reply: it lists what else refines the parent and the chains serving each, so a similar intent is not written twice.";
