//! Memories judged by the decision layer (ADR-0118, point 3). Cosine only finds the memories near a new one (at 0.65 or
//! more); whether a near memory teaches the same lesson, or advises what cannot be followed beside it, is asked of the
//! deciders, both questions in one call per pair (`memory.same_claim@1`, `memory.contradicts@1`). Where the decider is
//! sure enough to act, a pair it calls the same lesson must be named by the librarian before `remember` writes beside it,
//! as the cosine rule demanded; where it is not, the librarian is not made to. With no decider the old rule is the
//! System Two: cosine 0.84 or more must be named (`consolidate::DUPLICATE_COSINE`). A pair it calls contradicting goes to
//! the daily review (consolidate.rs). When the librarian names a judged pair, its naming is what the answer is scored on
//! (`Truth`, `OutcomeSource::Librarian`). Pure: `questions`, `state_of`, `Close::says_the_same`, `Truth`.

use std::sync::Arc;

use chains::judge::{DECIDE_FOR, Judged};
use judgment::{Invalid, Kind, Question};
use model::{
    Judgment, JudgmentPick, JudgmentSubject, JudgmentUse, Memory, MemoryLinkKind, OutcomeSource, Proved, Teaching,
};
use serde_json::{Value, json};
use tokio::task::JoinSet;

use crate::Crew;
use crate::consolidate::DUPLICATE_COSINE;
use chains::Result;

/// The cosine from which a memory is asked about at all: measured on the owner's 70 memories (2026-11-19), the
/// duplicates the 0.84 rule missed lay between 0.70 and 0.84, so the net is cast wider and the decider does the sorting.
pub(crate) const CANDIDATE_COSINE: f64 = 0.65;

/// How many of the nearest memories one remember asks the deciders about: each is a call, so a memory near many is judged
/// by its closest. Those nearer than `DUPLICATE_COSINE` beyond them stay under the cosine rule.
const JUDGED_MAX: usize = 6;

/// What makes the answer to each question yes or no, as the decider reads it: the words that raised Jev's AUC on the
/// owner's memory pairs from 0.912 to 0.994 (ADR-0118).
const SAME_YES: &str = "Both give the same advice, in other words or with different detail: one could be merged into the other without losing advice.";
const SAME_NO: &str = "They are about related topics but give different advice: merging would lose a lesson.";
const CONTRADICTS_YES: &str = "Following one breaks the other: they give opposite advice about the same thing.";
const CONTRADICTS_NO: &str = "Both can be followed together: they agree, or they are about different things.";

/// The two questions asked of every pair, the same lesson first.
pub(crate) fn questions() -> std::result::Result<[Question; 2], Invalid> {
    Ok([
        Question::new(&judgment::MEMORY_SAME_CLAIM, Kind::noul(SAME_YES, SAME_NO)?)?,
        Question::new(
            &judgment::MEMORY_CONTRADICTS,
            Kind::noul(CONTRADICTS_YES, CONTRADICTS_NO)?,
        )?,
    ])
}

/// The keys of the two questions, for reading what was judged of them.
pub(crate) fn question_keys() -> [String; 2] {
    [judgment::MEMORY_SAME_CLAIM.key(), judgment::MEMORY_CONTRADICTS.key()]
}

/// A memory as a decider reads it: its title and its body, which is what its lesson is. Pure.
fn said(m: &Memory) -> String {
    format!("{}. {}", m.title.trim(), m.body.trim())
}

/// The state both questions are asked about. Pure.
pub(crate) fn state_of(a: &Memory, b: &Memory) -> Value {
    json!({"memory_A": said(a), "memory_B": said(b)})
}

/// A memory the holder may recall that is near a new one, with what the deciders said of the pair.
#[derive(Debug, Clone)]
pub(crate) struct Close {
    pub memory: Memory,
    pub cosine: f64,
    /// None when no decider answered.
    pub same: Option<Judged>,
    pub contradicts: Option<Judged>,
}

impl Close {
    /// Whether the librarian must name it: the decider acted on it and says the same lesson. Whatever it did not act on
    /// (no answer, or one too unsure to act, which its System Two decides) is the cosine rule's. Pure.
    pub(crate) fn says_the_same(&self) -> bool {
        match &self.same {
            Some(j) if j.used == JudgmentUse::Acted => j.pick == JudgmentPick::Yes,
            _ => self.cosine >= DUPLICATE_COSINE,
        }
    }

    /// What its judgments proved, by the id of each, under what the librarian's act on the pair says. Pure.
    fn proofs(&self, truth: Truth) -> Vec<(String, Proved)> {
        [(&self.same, truth.same), (&self.contradicts, truth.contradicts)]
            .into_iter()
            .filter_map(|(j, said)| {
                let j = j.as_ref()?;
                Some((j.judgment.clone(), proved(&j.pick, said?)?))
            })
            .collect()
    }
}

/// What the librarian's act on a pair says of it: whether the two teach the same lesson, whether they contradict; none
/// where the act does not say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Truth {
    same: Option<bool>,
    contradicts: Option<bool>,
}

impl Truth {
    /// One says what the other did again (`mark_retaught`): they teach the same lesson. A supersedes says nothing: a
    /// correction gives different advice.
    pub(crate) const JOINED: Self = Self {
        same: Some(true),
        contradicts: None,
    };
    /// Both stand, kept as other lessons (`distinct_from`, a related or because link): not the same lesson. Whether
    /// they oppose is not said: a contradiction is kept as another lesson too.
    pub(crate) const KEPT: Self = Self {
        same: Some(false),
        contradicts: None,
    };
    /// Both stand, linked as contradicting: another lesson, and opposed.
    const OPPOSED: Self = Self {
        same: Some(false),
        contradicts: Some(true),
    };
    /// A review read both and linked them as bearing on each other: they can be followed together.
    const COMPATIBLE: Self = Self {
        same: None,
        contradicts: Some(false),
    };
    /// What the act does not say of the pair: a supersedes (a correction or a join), a refines.
    pub(crate) const UNSAID: Self = Self {
        same: None,
        contradicts: None,
    };

    /// What linking a pair as kind says of it when a memory is written or a teaching links it. Pure.
    pub(crate) const fn of_link(kind: MemoryLinkKind) -> Self {
        match kind {
            MemoryLinkKind::Contradicts => Self::OPPOSED,
            MemoryLinkKind::Because | MemoryLinkKind::Related => Self::KEPT,
            MemoryLinkKind::Supersedes | MemoryLinkKind::Refines => Self::UNSAID,
        }
    }

    /// What linking a pair as kind says of it under a review, which read both memories to settle whether they
    /// contradict: any link but contradicts says they can be followed together. Pure.
    pub(crate) const fn of_reviewed_link(kind: MemoryLinkKind) -> Self {
        match kind {
            MemoryLinkKind::Contradicts => Self::OPPOSED,
            MemoryLinkKind::Because | MemoryLinkKind::Related | MemoryLinkKind::Refines => Self::COMPATIBLE,
            MemoryLinkKind::Supersedes => Self::UNSAID,
        }
    }
}

/// Whether a pick proved right when the pair is said to be yes (`said`) or no. Only a yes or no pick is one. Pure.
fn proved(pick: &JudgmentPick, said: bool) -> Option<Proved> {
    let picked = match pick {
        JudgmentPick::Yes => true,
        JudgmentPick::No => false,
        JudgmentPick::Option { .. } | JudgmentPick::Level { .. } => return None,
    };
    Some(if picked == said { Proved::Right } else { Proved::Wrong })
}

/// What the stored judgments of the pair's questions proved under truth, by the id of each. Pure.
fn stored_proofs(judgments: &[Judgment], truth: Truth) -> Vec<(String, Proved)> {
    let [same, contradicts] = question_keys();
    judgments
        .iter()
        .filter(|j| j.outcome.is_none())
        .filter_map(|j| {
            let said = if j.question == same {
                truth.same
            } else if j.question == contradicts {
                truth.contradicts
            } else {
                None
            }?;
            Some((j.id.clone(), proved(&j.pick, said)?))
        })
        .collect()
}

impl Crew {
    /// The memories near m (of the vectors near, with their cosines) that its teaching's holder may recall, each with
    /// what the deciders say of the pair: asked now, or read back when this pair was asked before (a remember run again
    /// after a refusal asks nothing twice). The nearest `JUDGED_MAX` are asked; the rest that the cosine rule would make
    /// the librarian name come without an answer. Nearest first.
    pub(crate) async fn judge_close(
        self: &Arc<Self>,
        t: &Teaching,
        m: &Memory,
        near: Vec<(String, f64)>,
    ) -> Result<Vec<Close>> {
        let shown: Vec<(usize, String, f64)> = near
            .into_iter()
            .enumerate()
            .filter(|(k, (_, cosine))| *k < JUDGED_MAX || *cosine >= DUPLICATE_COSINE)
            .map(|(k, (id, cosine))| (k, id, cosine))
            .collect();
        let ids: Vec<String> = shown.iter().map(|(_, id, _)| id.clone()).collect();
        let (mid, keys) = (m.id.clone(), question_keys().to_vec());
        let (memories, stored) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.memories_named(&ids)?, r.pair_judgments_of(&mid, &keys)?)))
            .await?;
        let [same_key, contradicts_key] = question_keys();
        let asked = match questions() {
            Ok(q) => Some(q),
            Err(err) => {
                tracing::error!(%err, "memory: the questions asked of a pair cannot be built; the cosine rule decides");
                None
            }
        };
        let mut close: Vec<Close> = Vec::new();
        let mut asking: JoinSet<(usize, Vec<Option<Judged>>)> = JoinSet::new();
        for (k, id, cosine) in shown {
            let Some(old) = memories.iter().find(|o| o.id == id) else {
                continue;
            };
            let subject = JudgmentSubject::memory_pair(&m.id, &old.id, &t.chain);
            let before = |key: &str| {
                stored
                    .iter()
                    .find(|j| j.question == key && j.subject.id == subject.id)
                    .map(Judged::from)
            };
            let c = Close {
                memory: old.clone(),
                cosine,
                same: before(&same_key),
                contradicts: before(&contradicts_key),
            };
            let at = close.len();
            if c.same.is_none()
                && k < JUDGED_MAX
                && let Some(asked) = asked.clone()
            {
                let (me, state) = (Arc::clone(self), state_of(m, old));
                asking.spawn(async move { (at, me.judge(subject, &state, asked.to_vec()).await) });
            }
            close.push(c);
        }
        // All the pairs together get one `DECIDE_FOR`, not one each, nor one a decider's round: a pair left unanswered
        // when it runs out is the cosine rule's, and its tasks end with the set.
        let collect = async {
            while let Some(done) = asking.join_next().await {
                match done {
                    Ok((at, mut answers)) => {
                        let contradicts = answers.pop().flatten();
                        let same = answers.pop().flatten();
                        if let Some(c) = close.get_mut(at) {
                            (c.same, c.contradicts) = (c.same.take().or(same), c.contradicts.take().or(contradicts));
                        }
                    }
                    Err(err) => tracing::warn!(%err, "memory: a pair's judgment ended"),
                }
            }
        };
        if tokio::time::timeout(DECIDE_FOR, collect).await.is_err() {
            tracing::warn!("memory: the pairs were not all judged in time; the cosine rule decides the rest");
        }
        Ok(close)
    }

    /// Records what the proofs say, by the librarian's word: a pick that proved right or wrong. One that cannot be
    /// recorded is logged: the memory it was about is written either way.
    pub(crate) async fn record_proofs(&self, proofs: Vec<(String, Proved)>) {
        if proofs.is_empty() {
            return;
        }
        if let Err(err) = self.record_outcomes(proofs, OutcomeSource::Librarian).await {
            tracing::warn!(%err, "memory: what the librarian said of a pair is not recorded");
        }
    }

    /// Records what the librarian's naming of each of close in the memory just written proved of the answers about it:
    /// naming says what it did of a memory by its id, none when it did not name it.
    pub(crate) async fn prove_named(&self, close: &[Close], naming: impl Fn(&str) -> Option<Truth>) {
        let proofs = close
            .iter()
            .filter_map(|c| Some(c.proofs(naming(&c.memory.id)?)))
            .flatten()
            .collect();
        self.record_proofs(proofs).await;
    }

    /// Records what linking memory `from` to `to` says of the pair (`truth`, by who made the link), when the deciders
    /// were asked about it.
    pub(crate) async fn prove_link(&self, from: &str, to: &str, truth: Truth) {
        let (me, keys) = (from.to_owned(), question_keys().to_vec());
        let Ok(asked) = self.store.read(move |r| r.pair_judgments_of(&me, &keys)).await else {
            return;
        };
        let pair: Vec<Judgment> = asked
            .into_iter()
            .filter(|j| {
                j.subject
                    .pair()
                    .is_some_and(|(a, b)| (a, b) == (from, to) || (a, b) == (to, from))
            })
            .collect();
        self.record_proofs(stored_proofs(&pair, truth)).await;
    }

    /// Records that teaching `t` taught memory `old` again, which proves the pairs asked about it in the teaching's chain
    /// since it was recorded the same lesson, bar those whose newer memory the teaching wrote (it did not say it again).
    /// A teaching of no chain has none to tell its pairs by.
    pub(crate) async fn prove_retaught(&self, t: &Teaching, old: &str) {
        if t.chain.is_empty() {
            return;
        }
        let (me, keys) = (old.to_owned(), question_keys().to_vec());
        let Ok(asked) = self.store.read(move |r| r.pair_judgments_of(&me, &keys)).await else {
            return;
        };
        let pairs: Vec<Judgment> = asked
            .into_iter()
            .filter(|j| j.subject.home == t.chain && j.created_at >= t.created_at)
            .filter(|j| {
                j.subject
                    .pair()
                    .is_some_and(|(newer, older)| older == old && !t.written.iter().any(|w| w == newer))
            })
            .collect();
        self.record_proofs(stored_proofs(&pairs, Truth::JOINED)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answered(pick: JudgmentPick, used: JudgmentUse) -> Option<Judged> {
        Some(Judged {
            judgment: "ju_1".to_owned(),
            decider: String::new(),
            answer: model::JudgmentAnswer::Noul { p: 0.95 },
            pick,
            p: 0.95,
            threshold: Some(0.9),
            used,
        })
    }

    fn close(cosine: f64, same: Option<Judged>) -> Close {
        Close {
            memory: Memory::default(),
            cosine,
            same,
            contradicts: None,
        }
    }

    // The librarian must name a pair when the decider acts on "the same lesson"; a decider that acts on "different" frees
    // it; what the decider did not act on (unsure, or no answer) is the cosine rule's.
    #[test]
    fn a_pair_must_be_named_when_the_decider_acts_on_yes_and_otherwise_the_cosine_rule_says_so() {
        let yes = answered(JudgmentPick::Yes, JudgmentUse::Acted);
        assert!(close(0.70, yes).says_the_same());
        assert!(!close(0.95, answered(JudgmentPick::No, JudgmentUse::Acted)).says_the_same());
        assert!(close(0.92, answered(JudgmentPick::No, JudgmentUse::Escalated)).says_the_same());
        assert!(!close(0.80, answered(JudgmentPick::Yes, JudgmentUse::Escalated)).says_the_same());
        assert!(close(0.85, None).says_the_same());
        assert!(!close(0.80, None).says_the_same());
    }
}
