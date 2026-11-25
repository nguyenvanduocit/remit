//! The model of a hire the Chair hires when neither it nor the agent names one (ADR-0118, point 1). It is the cheapest
//! tier a decider is sure can do the work: `hire.tier_can` is asked once for each tier below the strongest, about the
//! agent, the brief, the deliverable, the criteria and the chain's kind. A stronger tier is never asked to act, so none
//! is rewarded for being strong. Where none is sure, a tenth of hires (`EXPLORE_PERCENT`) try the cheapest tier that is
//! more likely yes than not (`EXPLORE_FLOOR`), which is how a decider's answers below its act threshold get outcomes; the
//! climb (`climb.rs`) is what bounds that risk. `hire.effort` is asked in the same call and only informs. A hire that
//! is not chosen runs on the agent's own model and effort, else the Chair's. The card keeps that its model was chosen
//! (`Card::chosen`): it is accepted (`accepted_hire`, in the commit of the acceptance) or climbs a tier, and either
//! records what the choice proved.

use judgment::{ChoiceOption, HIRE_EFFORT, HIRE_TIER_CAN, Kind, Question, decider_name};
use model::{
    Agent, Chain, Chosen, Decision, DecisionKind, JudgmentOutcome, JudgmentPick, JudgmentSubject, JudgmentUse,
    OutcomeSource, Proved, SubjectKind, Time,
};
use provider::{Catalog, Effort, Tier};
use serde_json::json;
use store::Tx;

use crate::Result;
use crate::chain_core::Core;
use crate::hire_input::HireInput;
use crate::judge::{Judged, sampled};

/// The least probability of yes at which a hire that no decider is sure of tries a tier: more likely yes than not, by a
/// margin, so the climb is rarely needed. Below it nothing is tried.
pub const EXPLORE_FLOOR: f64 = 0.6;

/// The share of unsure hires, in percent, that try the cheapest tier at or above `EXPLORE_FLOOR`. Only answers at or above
/// an act threshold (0.9 until a decider has outcomes) ever get an outcome, so without it a decider never learns whether
/// its middling answers are right; a tenth bounds the cost of finding out.
pub const EXPLORE_PERCENT: u64 = 10;

/// What the deciders chose for a hire.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Picked {
    /// The model chosen: none where it was named or no decider acted on it.
    pub model: Option<String>,
    /// The card's record of it: none when nothing was chosen.
    pub chosen: Option<Chosen>,
    /// Who chose and how sure, and the effort a decider suggested, for the hire's decision as the owner reads it; empty
    /// when no decider answered.
    pub said: String,
}

/// A tier below the strongest, asked whether it can do the work.
struct Can {
    tier: Tier,
    question: Question,
}

/// One question for each tier of catalog below its strongest, the cheapest first: none with fewer than two tiers.
fn tier_questions(catalog: &Catalog) -> Vec<Can> {
    let mut tiers: Vec<Tier> = catalog.models().iter().map(|m| m.tier).collect();
    tiers.sort();
    tiers.dedup();
    tiers.pop();
    tiers
        .into_iter()
        .filter_map(|tier| {
            let kind = Kind::noul(
                format!(
                    "A {tier} model can do this work as well as a more capable one would: {}.",
                    tier.fits()
                ),
                format!(
                    "The work needs a more capable model than a {tier} one: it is harder, or a mistake costs more."
                ),
            );
            question(&HIRE_TIER_CAN, kind).map(|question| Can { tier, question })
        })
        .collect()
}

fn effort_question() -> Option<Question> {
    let options = Effort::ALL
        .into_iter()
        .map(|e| ChoiceOption {
            name: e.as_str().to_owned(),
            criterion: e.fits().to_owned(),
        })
        .collect();
    question(&HIRE_EFFORT, Kind::choice(options))
}

fn question(
    definition: &'static judgment::Definition,
    kind: std::result::Result<Kind, judgment::Invalid>,
) -> Option<Question> {
    kind.and_then(|kind| Question::new(definition, kind))
        .inspect_err(|err| tracing::warn!(%err, question = definition.key(), "hire: a question that cannot be asked"))
        .ok()
}

/// Which answer decides the model, and whether it is an exploration: the cheapest tier whose yes acts; else, when this
/// hire explores, the cheapest whose yes is at or above `EXPLORE_FLOOR`. answers are the tiers' in order, cheapest first.
/// Pure.
fn cheapest(answers: &[Option<Judged>], explores: bool) -> Option<(usize, bool)> {
    fn yes(a: &Option<Judged>) -> Option<&Judged> {
        a.as_ref().filter(|j| j.pick == JudgmentPick::Yes)
    }
    if let Some(i) = answers
        .iter()
        .position(|a| yes(a).is_some_and(|j| j.used == JudgmentUse::Acted))
    {
        return Some((i, false));
    }
    if !explores {
        return None;
    }
    answers
        .iter()
        .position(|a| yes(a).is_some_and(|j| j.p >= EXPLORE_FLOOR))
        .map(|i| (i, true))
}

/// The option a judgment picked.
fn picked_name(j: &Judged) -> &str {
    match &j.pick {
        JudgmentPick::Option { name } => name,
        _ => "",
    }
}

/// How sure a decider was, as the owner reads it: "82% sure".
pub(crate) fn sure(p: f64) -> String {
    format!("{:.0}% sure", p * 100.0)
}

impl Core {
    /// The model the deciders choose for the hire i that card_id is, of agent, in chain: the cheapest tier a decider is
    /// sure of, when neither i nor agent names a model (a tenth of the unsure hires try a tier, `EXPLORE_PERCENT`). The
    /// effort is asked in the same call but only informs: it is recorded and shown, never used (`HIRE_EFFORT`). Nothing
    /// when the catalog or the deciders give no answer: the hire runs as it always has.
    pub async fn pick_for_hire(&self, card_id: &str, chain: &Chain, agent: &Agent, i: &HireInput) -> Picked {
        let tiers = if i.named_model(agent).is_empty() {
            self.cfg.models.as_ref().map(tier_questions).unwrap_or_default()
        } else {
            Vec::new()
        };
        let effort_q = i.named_effort(agent).is_empty().then(effort_question).flatten();
        let questions: Vec<Question> = tiers
            .iter()
            .map(|c| c.question.clone())
            .chain(effort_q.clone())
            .collect();
        if questions.is_empty() {
            return Picked::default();
        }
        let state = json!({
            "agent": {"name": agent.name, "description": agent.description},
            "brief": i.brief,
            "deliverable": i.deliverable,
            "criteria": i.criteria,
            "chainKind": chain.kind.as_str(),
        });
        let subject = JudgmentSubject {
            kind: SubjectKind::Card,
            id: card_id.to_owned(),
            home: chain.id.clone(),
        };
        let mut answers = self.judge(subject, &state, questions).await;
        let effort = effort_q.is_some().then(|| answers.pop().flatten()).flatten();
        let mut tier = None;
        if let Some((at, explored)) = cheapest(&answers, sampled(card_id, EXPLORE_PERCENT))
            && let (Some(Can { tier: t, .. }), Some(Some(j))) = (tiers.get(at), answers.get(at))
            && (!explored || self.acted_on(&j.judgment).await)
        {
            tier = Some((*t, j.clone(), explored));
        }
        let model = tier
            .as_ref()
            .and_then(|(t, ..)| self.cfg.models.as_ref()?.of(*t))
            .map(|m| m.id.clone());
        let mut said = Vec::new();
        if let (Some((_, j, explored)), Some(m)) = (&tier, &model) {
            let by = format!("{}, {}", decider_name(&j.decider), sure(j.p));
            said.push(if *explored {
                format!("trying {m}; it moves up if it struggles ({by})")
            } else {
                format!("model chosen by {by}")
            });
        }
        if let Some(j) = effort.as_ref().filter(|j| !picked_name(j).is_empty()) {
            said.push(format!(
                "effort left as set; {} suggests {}, {}",
                decider_name(&j.decider),
                picked_name(j),
                sure(j.p)
            ));
        }
        let chosen = model.as_ref().and(tier.as_ref()).map(|(_, j, _)| Chosen {
            judgment: Some(j.judgment.clone()),
        });
        Picked {
            model,
            chosen,
            said: said.join("; "),
        }
    }

    /// Records that the code acted on the judgment, though it was below its act threshold: an exploration. False when it
    /// could not be recorded, in which case nothing is tried: an answer acted on must be on record.
    async fn acted_on(&self, judgment: &str) -> bool {
        let id = judgment.to_owned();
        match self
            .store
            .update(move |tx| tx.set_judgment_used(&id, JudgmentUse::Acted))
            .await
        {
            Ok(()) => true,
            Err::<_, store::Error>(err) => {
                tracing::warn!(%judgment, %err, "hire: an exploration could not be recorded; the hire is not tried");
                false
            }
        }
    }

    /// Marks the choice of a card whose hire was never stored as unknown: nothing proves it.
    pub async fn hire_unused(&self, chosen: &Chosen) {
        if let Some(j) = chosen.judgment.as_deref()
            && let Err(err) = self.record_outcome(j, Proved::Unknown, OutcomeSource::Card).await
        {
            tracing::warn!(judgment = %j, %err, "hire: an unused choice could not be recorded");
        }
    }
}

/// Records in tx that the choice of the card d accepts proved right, when d accepts a card whose model code chose. It is
/// part of the acceptance's own commit, so every path that accepts (the Chair's, a workflow's) records it and none
/// records it without the acceptance.
pub(crate) fn accepted_hire(tx: &Tx<'_>, d: &Decision, at: Time) -> Result<()> {
    if d.kind != DecisionKind::Accept || d.card_id.is_empty() {
        return Ok(());
    }
    let card = match tx.card(&d.card_id) {
        Ok(card) => card,
        Err(store::Error::NotFound) => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let proved = JudgmentOutcome {
        proved: Proved::Right,
        source: OutcomeSource::Card,
        at,
    };
    if let Some(j) = card.chosen.as_ref().and_then(|c| c.judgment.as_deref()) {
        match tx.set_judgment_outcome(j, &proved) {
            Ok(()) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::JudgmentAnswer;

    fn answer(p: f64, used: JudgmentUse) -> Option<Judged> {
        Some(Judged {
            judgment: "j".to_owned(),
            decider: String::new(),
            answer: JudgmentAnswer::Noul { p },
            pick: JudgmentPick::Yes,
            p,
            used,
            threshold: Some(0.9),
        })
    }

    #[test]
    fn the_cheapest_tier_that_acts_decides_whatever_a_stronger_one_says() {
        let answers = [
            answer(0.5, JudgmentUse::Escalated),
            answer(0.95, JudgmentUse::Acted),
            answer(0.99, JudgmentUse::Acted),
        ];
        assert_eq!(cheapest(&answers, true), Some((1, false)));
    }

    #[test]
    fn nothing_acting_only_a_hire_that_explores_tries_the_cheapest_tier_over_the_floor() {
        let answers = [
            answer(0.4, JudgmentUse::Escalated),
            answer(0.7, JudgmentUse::Escalated),
            answer(0.8, JudgmentUse::Escalated),
        ];
        assert_eq!(cheapest(&answers, false), None);
        assert_eq!(cheapest(&answers, true), Some((1, true)));
    }
}
