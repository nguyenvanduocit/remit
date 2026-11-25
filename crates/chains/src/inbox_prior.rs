//! What the deciders say of an inbox item before the Assistant's turn on it (ADR-0118, point 5). `inbox.option` (a choice
//! over the item's options) and `inbox.reversible` (can answering be undone) are asked once, when the item is put to the
//! Assistant; the Assistant reads the answers as a prior and still decides (`Acts::Informs`). They are scored by what
//! the owner and the second opinion later say of the Assistant's answer:
//!
//! - **The owner overturns** an answer that took the option a decider picked: that pick proved `Wrong`. An overturn
//!   says nothing of an option the Assistant did not take (the owner may have chosen none of them), and nothing of
//!   `inbox.reversible`.
//! - **A second opinion** is a fresh judge that decides the item without seeing the Assistant's answer. Its option is
//!   the reference for `inbox.option` (`Right` when the decider picked it, else `Wrong`), and its `undoable` the one for
//!   `inbox.reversible`. It only settles what is not proved yet: the owner's word outranks it.
//!
//! The second opinion also goes where the Assistant and a decider part (`Prior::dissents`), besides its random share.

use judgment::{ChoiceOption, Definition, INBOX_OPTION, INBOX_REVERSIBLE, Kind, Question, decider_name};
use model::{
    Decision, DecisionKind, InboxItem, Judgment, JudgmentOutcome, JudgmentPick, JudgmentSubject, OutcomeSource, Proved,
    SecondOpinion, Stance, SubjectKind, Time,
};
use serde_json::json;
use store::{Reader, Tx};

use crate::Result;
use crate::chain_core::Core;
use crate::judge::Judged;
use crate::owner_acts::holds_a_secret;

/// The probability at or above which a decider's answer that the Assistant did not follow sends its answer to a second
/// opinion: sure enough that the part is worth a second judge's time.
pub const DISAGREES_AT: f64 = 0.9;

/// What the deciders said of one item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prior {
    /// The option it would take; none when the item lists fewer than two options or no decider answered.
    pub option: Option<Judged>,
    /// Whether answering can be undone, picked yes or no; none when no decider answered.
    pub reversible: Option<Judged>,
}

/// The option question of item it, each option named by its id so a pick is the option the Assistant answers with; none
/// for an item with fewer than two options.
fn option_question(it: &InboxItem) -> Option<Question> {
    let options = it
        .options
        .iter()
        .map(|op| ChoiceOption {
            name: op.id.clone(),
            criterion: op.name.clone(),
        })
        .collect();
    question(&INBOX_OPTION, Kind::choice(options))
}

fn reversible_question() -> Option<Question> {
    question(
        &INBOX_REVERSIBLE,
        Kind::noul(
            "Answering it can be undone later at little cost: a permission can be withdrawn, a change reverted, a choice \
             made again, and nothing is sent, spent or deleted for good.",
            "Answering it cannot be undone cheaply: it spends money, deletes or sends something, or lets something out \
             that cannot be taken back.",
        ),
    )
}

/// definition asked as kind, or none (logged) when the item cannot be asked it, as an item with fewer than two options.
fn question(definition: &'static Definition, kind: std::result::Result<Kind, judgment::Invalid>) -> Option<Question> {
    kind.and_then(|kind| Question::new(definition, kind))
        .inspect_err(
            |err| tracing::debug!(%err, question = definition.key(), "inbox: a question not asked of this item"),
        )
        .ok()
}

impl Core {
    /// What the deciders say of item it before the Assistant decides it: asked by `asker`, in a chain whose goal is goal.
    /// Nothing for a credential or a sign-in, and where no decider answers in time: the Assistant then decides as it
    /// always has.
    pub async fn prior_for_item(&self, it: &InboxItem, asker: &str, goal: &str) -> Prior {
        if holds_a_secret(it) {
            return Prior::default();
        }
        let (option_q, reversible_q) = (option_question(it), reversible_question());
        let questions: Vec<Question> = option_q.iter().chain(&reversible_q).cloned().collect();
        if questions.is_empty() {
            return Prior::default();
        }
        let state = json!({
            "item": {"kind": it.kind.as_str(), "title": it.title, "detail": it.detail},
            "asker": asker,
            "chainGoal": goal,
        });
        let subject = JudgmentSubject {
            kind: SubjectKind::InboxItem,
            id: it.id.clone(),
            home: it.home.clone(),
        };
        let mut answers = self.judge(subject, &state, questions).await.into_iter();
        let option = option_q.and_then(|_| answers.next().flatten());
        let reversible = reversible_q.and_then(|_| answers.next().flatten());
        Prior { option, reversible }
    }
}

/// What the deciders said of item, as it was recorded: the newest asking of each question.
pub fn item_prior(r: &Reader<'_>, item: &str) -> Result<Prior> {
    let keys = [INBOX_OPTION.key(), INBOX_REVERSIBLE.key()];
    let judged = r.item_judgments_of(item, &keys)?;
    let newest = |key: String| judged.iter().find(|j: &&Judgment| j.question == key).map(Judged::from);
    Ok(Prior {
        option: newest(INBOX_OPTION.key()),
        reversible: newest(INBOX_REVERSIBLE.key()),
    })
}

impl Prior {
    /// Whether the Assistant's answer parts from a decider's, sure of itself (`DISAGREES_AT`): the answer took option
    /// `option` (empty when it answered in words) and the decider picked another that is not the same answer in other
    /// words (`stance_of` reads an option's stance, as the decision records it: `allow_once` and `allow_always` both
    /// approve); or it approved (`stance`) what a decider holds cannot be undone. Pure.
    pub fn dissents(&self, option: &str, stance: Stance, stance_of: impl Fn(&str) -> Stance) -> bool {
        let other_option = self.option.as_ref().is_some_and(|j| {
            let JudgmentPick::Option { name } = &j.pick else {
                return false;
            };
            let picked = stance_of(name);
            let same = if stance == Stance::Neither || picked == Stance::Neither {
                name == option
            } else {
                stance == picked
            };
            !option.is_empty() && j.p >= DISAGREES_AT && !same
        });
        let approved_irreversible = stance == Stance::Approve
            && self
                .reversible
                .as_ref()
                .is_some_and(|j| j.p >= DISAGREES_AT && j.pick == JudgmentPick::No);
        other_option || approved_irreversible
    }

    /// Whether a second judge is asked whether answering it can be undone: a decider answered that question.
    pub const fn asks_undoable(&self) -> bool {
        self.reversible.is_some()
    }

    /// What the Assistant reads of it beside the item: a prior, never a decision. Empty when no decider answered. Pure.
    pub fn words(&self, it: &InboxItem) -> String {
        let mut said = Vec::new();
        if let Some(j) = &self.option
            && let JudgmentPick::Option { name } = &j.pick
        {
            let words = it
                .options
                .iter()
                .find(|op| op.id == *name)
                .map_or("", |op| op.name.as_str());
            said.push(format!(
                "{} would take {name} ({words}), {} sure",
                decider_name(&j.decider),
                percent(j.p)
            ));
        }
        if let Some(j) = &self.reversible {
            let can = if j.pick == JudgmentPick::Yes { "can" } else { "cannot" };
            said.push(format!(
                "{} judges that answering {can} be undone later at little cost, {} sure",
                decider_name(&j.decider),
                percent(j.p)
            ));
        }
        if said.is_empty() {
            return String::new();
        }
        format!(
            "\n\nFast deciders read this item before you: {}. That is a prior, not a decision: they have not read his \
             principles or his last decisions. Decide as you judge he would, and where you part from them, your reason \
             says why.",
            said.join("; ")
        )
    }
}

fn percent(p: f64) -> String {
    format!("{:.0}%", p * 100.0)
}

/// Records in tx what the owner's overturn d of an answer that took option `d.option` proves, in the overturn's own
/// transaction (it is the answer's option that `d` carries): the pick of that very option is `Wrong`; and a second judge
/// that chose that option has lost its reference, so the outcome it gave to any other pick is withdrawn. The owner's
/// word is final, in either order with the second opinion (`opinion_given`).
pub fn overturned_answer(tx: &Tx<'_>, d: &Decision, at: Time) -> Result<()> {
    if d.option.is_empty() {
        return Ok(());
    }
    let Some(j) = item_prior(tx, &d.item)?.option else {
        return Ok(());
    };
    if matches!(&j.pick, JudgmentPick::Option { name } if *name == d.option) {
        return settle(tx, &j.judgment, Proved::Wrong, OutcomeSource::Owner, at, true);
    }
    if tx
        .second_opinions_of_item(&d.item)?
        .iter()
        .any(|s| s.option == d.option)
    {
        match tx.clear_judgment_outcome(&j.judgment) {
            Ok(()) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

/// Records in tx what the second opinion s says of the deciders' picks on its item, as it gives its answer: each pick it
/// has a reference for proved right when it is the judge's, wrong when it is not. What the owner proved already stands,
/// and so does his overturn of the answer that took the judge's option: that reference is spent, so the option's pick
/// is not scored. A second opinion on an answer that parted from a decider says so in its source (`s.targeted`), which
/// no standing is computed from.
pub fn opinion_given(tx: &Tx<'_>, s: &SecondOpinion, at: Time) -> Result<()> {
    let prior = item_prior(tx, &s.item)?;
    let overturned = tx
        .decisions_of_kind(&s.chain, &DecisionKind::Overturn)?
        .iter()
        .any(|o| o.item == s.item && o.option == s.option);
    let option = prior.option.as_ref().and_then(|j| match &j.pick {
        JudgmentPick::Option { name } if !s.option.is_empty() && !overturned => Some((j, *name == s.option)),
        _ => None,
    });
    let reversible = prior
        .reversible
        .as_ref()
        .zip(s.undoable)
        .map(|(j, undoable)| (j, (j.pick == JudgmentPick::Yes) == undoable));
    let source = if s.targeted {
        OutcomeSource::SecondOpinionTargeted
    } else {
        OutcomeSource::SecondOpinion
    };
    for (j, right) in option.into_iter().chain(reversible) {
        let proved = if right { Proved::Right } else { Proved::Wrong };
        settle(tx, &j.judgment, proved, source, at, false)?;
    }
    Ok(())
}

/// Records proved on judgment, replacing what is recorded when replace, else only where nothing is.
fn settle(tx: &Tx<'_>, judgment: &str, proved: Proved, source: OutcomeSource, at: Time, replace: bool) -> Result<()> {
    let outcome = JudgmentOutcome { proved, source, at };
    let set = if replace {
        tx.set_judgment_outcome(judgment, &outcome)
    } else {
        tx.settle_judgment_outcome(judgment, &outcome)
    };
    match set {
        Ok(()) | Err(store::Error::NotFound) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{InboxOption, JudgmentAnswer, JudgmentUse};

    fn judged(pick: JudgmentPick, p: f64) -> Option<Judged> {
        Some(Judged {
            judgment: "ju_1".to_owned(),
            decider: String::new(),
            answer: JudgmentAnswer::Noul { p },
            pick,
            p,
            used: JudgmentUse::Informed,
            threshold: None,
        })
    }

    fn picks(option: &str, p: f64) -> Prior {
        Prior {
            option: judged(
                JudgmentPick::Option {
                    name: option.to_owned(),
                },
                p,
            ),
            reversible: None,
        }
    }

    /// The stance of a permission's option ids, as `stance_of` reads them.
    fn by_name(id: &str) -> Stance {
        match id {
            i if i.starts_with("allow") => Stance::Approve,
            i if i.starts_with("reject") => Stance::Decline,
            _ => Stance::Neither,
        }
    }

    #[test]
    fn an_answer_dissents_from_a_decider_that_picked_another_option_and_was_sure() {
        assert!(picks("allow_once", 0.93).dissents("reject_once", Stance::Decline, by_name));
        assert!(
            !picks("allow_once", 0.93).dissents("allow_once", Stance::Approve, by_name),
            "the same option"
        );
        assert!(
            !picks("allow_always", 0.93).dissents("allow_once", Stance::Approve, by_name),
            "the same answer in other words"
        );
        assert!(
            picks("b", 0.93).dissents("a", Stance::Neither, by_name),
            "no stance to compare: the ids differ"
        );
        assert!(
            !picks("allow_once", 0.8).dissents("reject_once", Stance::Decline, by_name),
            "not sure enough"
        );
        assert!(
            !picks("allow_once", 0.99).dissents("", Stance::Neither, by_name),
            "an answer in words took no option"
        );
    }

    #[test]
    fn an_approval_of_what_a_decider_holds_cannot_be_undone_dissents_a_refusal_does_not() {
        let prior = Prior {
            option: None,
            reversible: judged(JudgmentPick::No, 0.95),
        };
        assert!(prior.dissents("", Stance::Approve, by_name));
        assert!(!prior.dissents("", Stance::Decline, by_name));
    }

    #[test]
    fn an_item_with_one_option_is_asked_no_option_question() {
        let it = InboxItem {
            options: vec![InboxOption {
                id: "ok".to_owned(),
                name: "Go on".to_owned(),
                ..InboxOption::default()
            }],
            ..InboxItem::default()
        };
        assert!(option_question(&it).is_none());
    }
}
