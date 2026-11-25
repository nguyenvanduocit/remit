//! How deep a change's review goes (ADR-0118, point 4). When a version of a chain's change is recorded, two questions are
//! asked of a decider about its summary, the paths it touches and the chain's goal: how risky it is (`gate.risk`, a score
//! of three levels, each an effort), and whether it alters what a rail does though no path it touches is one the
//! repository declares (`gate.touches_rail`). The first sets the effort its reviewer and QC run at, the second adds the
//! security gate. Both only tighten: a judge runs at the higher of its usual effort and the one judged (`tightened`), and a
//! gate is added, never removed or lowered; a decider that is unsure, late or absent leaves the depth as the chain's kind
//! and the rails file make it. What the decider said is kept on the landing's version (`LandingVersion::judged`), and what
//! the gates and the landing then show scores it, in the commit that stores each (`ended`, called from `commit.rs`).

use judgment::{GATE_RISK, GATE_TOUCHES_RAIL, Kind, Question, decider_name};
use model::{
    Chain, JudgmentOutcome, JudgmentPick, JudgmentSubject, JudgmentUse, Landing, LandingState, OutcomeSource, Proved,
    RevertCause, SubjectKind, Time, Verdict, VerdictRole,
};
use provider::Effort;
use serde_json::json;
use store::Tx;

use crate::Result;
use crate::chain_core::Core;
use crate::hire_choice::sure;
use crate::judge::Judged;

/// The efforts `gate.risk` chooses between, the least risky first, each with its level in words: the levels of its score.
const DEPTHS: [(Effort, &str); 3] = [
    (
        Effort::High,
        "A careful read and a run settle it: the change is contained, easy to take back, and little that others depend \
         on rests on it.",
    ),
    (
        Effort::XHigh,
        "It needs a harder look: it changes behaviour that other code, stored data or customers depend on, or has many \
         parts that act on each other.",
    ),
    (
        Effort::Max,
        "It needs everything: a slip would be costly and hard to undo (lost data, money, a broken contract, an outage), \
         or it changes how the system protects itself.",
    ),
];

/// What Claude Code runs at when no effort is named (ADR-0070): the level an empty effort ranks as.
const DEFAULT_EFFORT: Effort = Effort::XHigh;

/// What a version of a change is, as the deciders read it. The repository's rails file does not leave the machine: only
/// whether a path matched a rail does.
pub struct Change<'a> {
    /// Its commit's title and description.
    pub title: &'a str,
    pub description: &'a str,
    /// The paths it touches.
    pub paths: &'a [String],
    /// The effort the Chair is at, which the judges run at unless their agent names one: a judgment that would not raise
    /// it changes nothing.
    pub usual_effort: &'a str,
}

/// What the deciders judged of a version and what the code did with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reviewed {
    /// Every judgment answered, by id: what the gates and the landing then score.
    pub judged: Vec<String>,
    /// The effort the reviewer and QC are raised to: a decider acted on how risky the change is, and it is above the usual.
    pub effort: Option<Effort>,
    /// Whether a decider acted on the change altering a rail: the security gate is added.
    pub security: bool,
    /// What was done, in the owner's words, one entry for each act.
    pub said: Vec<Said>,
}

/// One act of a decider on a change, as the owner reads it in the chain's decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub what: String,
    pub why: String,
}

/// What happened to a version that shows how right a judgment of its change was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// The release that carried it baked healthy: it holds.
    Held,
    /// It was taken off the base.
    TakenBack(RevertCause),
    /// A gate signed its verdict on it.
    Judged { role: VerdictRole, verdict: Verdict },
}

fn risk_question() -> Option<Question> {
    asked(
        &GATE_RISK,
        Kind::score(DEPTHS.iter().map(|(_, words)| (*words).to_owned()).collect()),
    )
}

fn rail_question() -> Option<Question> {
    asked(
        &GATE_TOUCHES_RAIL,
        Kind::noul(
            "The change alters what authentication, pause and stop, merging, the jail, secrets or credentials do, \
             though no path it touches is one of the repository's declared rails.",
            "The change leaves what those rails do as it was, or only reads or names them.",
        ),
    )
}

fn asked(
    definition: &'static judgment::Definition,
    kind: std::result::Result<Kind, judgment::Invalid>,
) -> Option<Question> {
    kind.and_then(|kind| Question::new(definition, kind))
        .inspect_err(
            |err| tracing::warn!(%err, question = definition.key(), "review depth: a question that cannot be asked"),
        )
        .ok()
}

/// The effort a pick of `gate.risk` stands for. Pure.
fn depth_of(pick: &JudgmentPick) -> Option<Effort> {
    match pick {
        JudgmentPick::Level { index, .. } => DEPTHS.get(usize::try_from(*index).ok()?).map(|(e, _)| *e),
        _ => None,
    }
}

/// The level `usual` ranks as: an empty one is Claude Code's default.
fn rank(usual: &str) -> Effort {
    Effort::parse(usual).ok().flatten().unwrap_or(DEFAULT_EFFORT)
}

/// The effort a judge runs at when its usual is `usual` (empty: Claude Code's default) and a decider raised the change's
/// to `raised`: `raised` when it is strictly above the usual, else the usual as written. Pure.
pub fn tightened(usual: &str, raised: Option<Effort>) -> String {
    match raised {
        Some(r) if r > rank(usual) => r.as_str().to_owned(),
        _ => usual.to_owned(),
    }
}

/// The effort the judges of a chain's change on `repo` (every repository's when none) are raised to: the highest any
/// version waiting for its gates carries. Pure.
pub fn raised_by(landings: &[Landing], repo: Option<&str>) -> Option<Effort> {
    landings
        .iter()
        .filter(|l| l.state == LandingState::Judging && l.reverts.is_none() && repo.is_none_or(|r| l.repo == r))
        .filter_map(|l| Effort::parse(l.version.effort.as_deref()?).ok().flatten())
        .max()
}

/// What an ending proves of one judgment's pick, none when it proves nothing. Pure.
///
/// `gate.risk`: a version a gate sent back, or that was taken back, was risky: the lowest level was too shallow and the
/// others were right. One that went live was contained: the lowest level was right, the highest cost more than it gave, and
/// the middle proves nothing. A revert a conflict made, and an approval, prove nothing of how risky it was.
/// `gate.touches_rail` is judged by the evidence about rails only, the security gate's verdict on that version: it asked
/// for changes, so there was something a rail guards to find; it approved, so the review the yes added found nothing.
/// Whatever else ends the landing proves nothing of it (`Unknown`).
fn proved_by(question: &str, pick: &JudgmentPick, ending: Ending) -> Option<Proved> {
    if question == GATE_RISK.key() {
        let JudgmentPick::Level { index, .. } = pick else {
            return None;
        };
        let index = usize::try_from(*index).ok()?;
        let risky = if index == 0 { Proved::Wrong } else { Proved::Right };
        return match ending {
            Ending::TakenBack(RevertCause::Conflict) => Some(Proved::Unknown),
            Ending::TakenBack(_)
            | Ending::Judged {
                verdict: Verdict::RequestChanges,
                ..
            } => Some(risky),
            Ending::Judged {
                verdict: Verdict::Approve,
                ..
            } => None,
            Ending::Held if index == 0 => Some(Proved::Right),
            Ending::Held if index == DEPTHS.len().saturating_sub(1) => Some(Proved::Wrong),
            Ending::Held => Some(Proved::Unknown),
        };
    }
    if question == GATE_TOUCHES_RAIL.key() {
        return match (pick, ending) {
            (
                JudgmentPick::Yes,
                Ending::Judged {
                    role: VerdictRole::Security,
                    verdict,
                },
            ) => Some(if verdict == Verdict::RequestChanges {
                Proved::Right
            } else {
                Proved::Wrong
            }),
            (_, Ending::Judged { .. }) => None,
            (_, Ending::Held | Ending::TakenBack(_)) => Some(Proved::Unknown),
        };
    }
    None
}

impl Core {
    /// What the deciders make of the version `landing` of chain's change, which `has_security` says is already to be
    /// judged by security (the path rule's verdict, so whether it alters a rail is not asked). Nothing when no decider
    /// answers in time.
    pub async fn judge_change(
        &self,
        chain: &Chain,
        landing: &str,
        change: &Change<'_>,
        has_security: bool,
    ) -> Reviewed {
        let risk = risk_question();
        let rail = if has_security { None } else { rail_question() };
        let questions: Vec<Question> = risk.iter().chain(rail.iter()).cloned().collect();
        if questions.is_empty() {
            return Reviewed::default();
        }
        let state = json!({
            "goal": chain.goal,
            "summary": {"title": change.title, "description": change.description},
            "paths": change.paths,
            "aDeclaredRailMatchesAPath": has_security,
        });
        let subject = JudgmentSubject {
            kind: SubjectKind::Change,
            id: landing.to_owned(),
            home: chain.id.clone(),
        };
        let mut answers = self.judge(subject, &state, questions).await.into_iter();
        let risk = risk.and_then(|_| answers.next().flatten());
        let rail = rail.and_then(|_| answers.next().flatten());
        reviewed(risk.as_ref(), rail.as_ref(), change.usual_effort)
    }
}

/// What the answers to the two questions come to, for judges whose usual effort is `usual`. Pure.
fn reviewed(risk: Option<&Judged>, rail: Option<&Judged>, usual: &str) -> Reviewed {
    let mut out = Reviewed {
        judged: [risk, rail].into_iter().flatten().map(|j| j.judgment.clone()).collect(),
        ..Reviewed::default()
    };
    if let Some(j) = risk.filter(|j| j.used == JudgmentUse::Acted)
        && let Some(effort) = depth_of(&j.pick).filter(|e| *e > rank(usual))
    {
        out.effort = Some(effort);
        out.said.push(Said {
            what: format!("Review and QC run at {} effort for this change", effort.as_str()),
            why: format!("{} is {} that it is that risky", decider_name(&j.decider), sure(j.p)),
        });
    }
    if let Some(j) = rail.filter(|j| j.used == JudgmentUse::Acted && j.pick == JudgmentPick::Yes) {
        out.security = true;
        out.said.push(Said {
            what: "A security review is added for this change".to_owned(),
            why: format!(
                "{} is {} that it alters what a rail does, though no path it touches is a declared rail",
                decider_name(&j.decider),
                sure(j.p)
            ),
        });
    }
    out
}

/// Records in tx what `ending` of landing l proves of the judgments made of its change, at `at`. What proves nothing
/// (`Unknown`) and what a landing that holds proves settle only a judgment with no outcome yet, so they never replace what
/// a gate or a revert proved; the rest replaces an outcome, and is not written again when it is the same, so a replayed
/// write leaves when it was first known. It is part of the commit that stores the ending, so every path that ends a
/// landing scores it and none scores it without the ending. A revert's own landing, which no decider judged, is left out.
pub(crate) fn ended(tx: &Tx<'_>, l: &Landing, ending: Ending, at: Time) -> Result<()> {
    if l.reverts.is_some() || l.version.judged.is_empty() {
        return Ok(());
    }
    for j in tx.judgments_named(&l.version.judged)? {
        let Some(proved) = proved_by(&j.question, &j.pick, ending) else {
            continue;
        };
        let outcome = JudgmentOutcome {
            proved,
            source: OutcomeSource::Gate,
            at,
        };
        if proved == Proved::Unknown || ending == Ending::Held {
            tx.settle_judgment_outcome(&j.id, &outcome)?;
        } else {
            match tx.change_judgment_outcome(&j.id, &outcome) {
                Ok(()) | Err(store::Error::NotFound) => {}
                Err(err) => return Err(err.into()),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::JudgmentAnswer;

    fn level(index: u32) -> JudgmentPick {
        JudgmentPick::Level {
            index,
            name: String::new(),
        }
    }

    fn judged(pick: JudgmentPick, used: JudgmentUse) -> Judged {
        Judged {
            judgment: "ju_1".to_owned(),
            decider: judgment::DEFAULT_DECIDER.to_owned(),
            answer: JudgmentAnswer::Noul { p: 0.95 },
            pick,
            p: 0.95,
            used,
            threshold: Some(0.9),
        }
    }

    fn security(verdict: Verdict) -> Ending {
        Ending::Judged {
            role: VerdictRole::Security,
            verdict,
        }
    }

    #[test]
    fn a_judge_is_raised_only_strictly_above_its_usual_and_an_empty_usual_is_claude_codes_default() {
        assert_eq!(tightened("max", Some(Effort::High)), "max");
        assert_eq!(tightened("high", Some(Effort::XHigh)), "xhigh");
        assert_eq!(
            tightened("", Some(Effort::High)),
            "",
            "never lower than the default, xhigh"
        );
        assert_eq!(tightened("", Some(Effort::XHigh)), "");
        assert_eq!(tightened("", Some(Effort::Max)), "max");
        assert_eq!(tightened("medium", None), "medium");
    }

    #[test]
    fn a_change_sent_back_or_taken_back_was_risky_so_only_the_lowest_depth_was_wrong() {
        let risk = GATE_RISK.key();
        let sent = Ending::Judged {
            role: VerdictRole::Qc,
            verdict: Verdict::RequestChanges,
        };
        for ending in [sent, Ending::TakenBack(RevertCause::Guardrail)] {
            assert_eq!(proved_by(&risk, &level(0), ending), Some(Proved::Wrong));
            assert_eq!(proved_by(&risk, &level(1), ending), Some(Proved::Right));
            assert_eq!(proved_by(&risk, &level(2), ending), Some(Proved::Right));
        }
        let conflict = Ending::TakenBack(RevertCause::Conflict);
        assert_eq!(proved_by(&risk, &level(0), conflict), Some(Proved::Unknown));
    }

    #[test]
    fn a_change_that_went_live_proves_the_lowest_depth_enough_and_the_highest_too_much() {
        let risk = GATE_RISK.key();
        assert_eq!(proved_by(&risk, &level(0), Ending::Held), Some(Proved::Right));
        assert_eq!(proved_by(&risk, &level(1), Ending::Held), Some(Proved::Unknown));
        assert_eq!(proved_by(&risk, &level(2), Ending::Held), Some(Proved::Wrong));
        let approved = Ending::Judged {
            role: VerdictRole::Reviewer,
            verdict: Verdict::Approve,
        };
        assert_eq!(
            proved_by(&risk, &level(2), approved),
            None,
            "an approval waits for the release"
        );
    }

    #[test]
    fn a_rail_judged_altered_is_proved_by_the_security_gate_alone() {
        let rail = GATE_TOUCHES_RAIL.key();
        assert_eq!(
            proved_by(&rail, &JudgmentPick::Yes, security(Verdict::RequestChanges)),
            Some(Proved::Right)
        );
        assert_eq!(
            proved_by(&rail, &JudgmentPick::Yes, security(Verdict::Approve)),
            Some(Proved::Wrong)
        );
        let qc = Ending::Judged {
            role: VerdictRole::Qc,
            verdict: Verdict::RequestChanges,
        };
        assert_eq!(proved_by(&rail, &JudgmentPick::Yes, qc), None);
        assert_eq!(proved_by(&rail, &JudgmentPick::No, security(Verdict::Approve)), None);
        for ending in [Ending::Held, Ending::TakenBack(RevertCause::Overturn)] {
            assert_eq!(proved_by(&rail, &JudgmentPick::Yes, ending), Some(Proved::Unknown));
            assert_eq!(proved_by(&rail, &JudgmentPick::No, ending), Some(Proved::Unknown));
        }
    }

    #[test]
    fn only_an_answer_the_code_acted_on_that_changes_something_changes_the_review_and_every_answer_is_kept() {
        let risk = judged(level(2), JudgmentUse::Acted);
        let rail = judged(JudgmentPick::Yes, JudgmentUse::Escalated);
        let r = reviewed(Some(&risk), Some(&rail), "high");
        assert_eq!(r.effort, Some(Effort::Max));
        assert!(!r.security, "a yes too unsure to act adds no gate");
        assert_eq!(r.judged.len(), 2);
        assert_eq!(r.said.len(), 1);
        let unsure = reviewed(Some(&judged(level(2), JudgmentUse::Escalated)), None, "high");
        assert_eq!(unsure.effort, None, "the depth stays as it was");
        assert_eq!(unsure.judged.len(), 1);
        let nothing_to_raise = reviewed(Some(&judged(level(0), JudgmentUse::Acted)), None, "");
        assert_eq!((nothing_to_raise.effort, nothing_to_raise.said.len()), (None, 0));
    }

    #[test]
    fn a_no_never_adds_a_gate_even_when_the_code_would_act_on_it() {
        let rail = judged(JudgmentPick::No, JudgmentUse::Acted);
        assert!(!reviewed(None, Some(&rail), "").security);
    }

    #[test]
    fn the_judges_of_a_repository_run_at_the_highest_depth_any_version_waiting_for_them_asks() {
        let version = |repo: &str, effort: &str, state: LandingState| Landing {
            repo: repo.to_owned(),
            state,
            version: model::LandingVersion {
                effort: Some(effort.to_owned()),
                ..model::LandingVersion::default()
            },
            ..Landing::default()
        };
        let a = version("web", "xhigh", LandingState::Judging);
        let b = version("api", "max", LandingState::Judging);
        let landed = version("api", "max", LandingState::Landed);
        let all = [a, b, landed];
        assert_eq!(raised_by(&all, None), Some(Effort::Max));
        assert_eq!(raised_by(&all, Some("web")), Some(Effort::XHigh));
        assert_eq!(raised_by(&all, Some("docs")), None);
        assert_eq!(raised_by(&[Landing::default()], None), None);
    }
}
