//! Asking the deciders (ADR-0118). A point that judges asks its typed questions about a state with `Core::judge`: each
//! question goes to the deciders that may act for it in their order (`judgment::ranked`: the best Brier score with 30
//! outcomes first, its default last), the next asked only for a question those before it left unanswered, and, once
//! one answered, to up to two shadows, which only answer to be measured. The state leaves the machine as its questions
//! allow (`judgment::MayLeave`). Every answer is recorded as a `model::Judgment` under one asking per question, the
//! state kept only as its digest; the point records what its answer proved with `Core::record_outcome`. A question no
//! decider answered in time gets none: its point runs its System Two as before, and nothing is recorded for it.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use judgment::{ActsOn, Decider, DeciderError, MayLeave, Question, Scored, Standing, Threshold};
use model::{
    Judgment, JudgmentAnswer, JudgmentOutcome, JudgmentPick, JudgmentSubject, JudgmentUse, OutcomeSource, Proved,
    Settings, Time,
};
use serde_json::Value;
use sha2::Digest;
use store::JudgmentScore;
use tokio::task::JoinSet;

use crate::Result;
use crate::chain_core::{Core, new_id};
use crate::steering;

/// How long a point waits for one decider before it asks the next or runs its System Two. It bounds each round of asking
/// (`judge` asks one round per ranked decider, so up to as many rounds as there are ranked deciders): a point that must not
/// be held up longer than this for the whole of it puts one `DECIDE_FOR` around its call to `judge`, as the watchdog does. A
/// shadow is given as long, so it is measured on the terms the acting decider is held to.
pub const DECIDE_FOR: Duration = Duration::from_secs(2);

/// Whether the subject `id` is in the sample of `percent` in a hundred: a fixed function of the id, which is random, so
/// the share is a random sample and asking twice gives the same answer. Pure.
pub fn sampled(id: &str, percent: u64) -> bool {
    let sum = sha2::Sha256::digest(id.as_bytes());
    let mut head = [0u8; 8];
    head.copy_from_slice(sum.get(..8).unwrap_or(&[0; 8]));
    u64::from_be_bytes(head) % 100 < percent
}

/// The acting decider's answer to one question, as the point that asked reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Judged {
    /// The judgment it is recorded as, which its outcome is recorded on (`Core::record_outcome`).
    pub judgment: String,
    /// The pinned id of the decider that answered (`judgment::decider_name` says it in words).
    pub decider: String,
    pub answer: JudgmentAnswer,
    pub pick: JudgmentPick,
    /// Its probability of the pick.
    pub p: f64,
    /// What the point makes of it: act on it alone, escalate to its System Two, or show it to the agent that decides.
    pub used: JudgmentUse,
    /// The probability its question's bar was at when it was asked (the act threshold of its decider at the precision the
    /// question is held to), where the question has one; none when no probability reaches the precision. The one place the
    /// bar is worked out: a point reads it here and the page reads it from the judgment.
    pub threshold: Option<f64>,
}

impl From<&Judgment> for Judged {
    /// The answer as it was recorded: what a point that asked the same thing before reads again instead of asking twice.
    fn from(j: &Judgment) -> Self {
        Self {
            judgment: j.id.clone(),
            decider: j.decider.clone(),
            answer: j.answer.clone(),
            pick: j.pick.clone(),
            p: j.p,
            used: j.used,
            threshold: j.threshold,
        }
    }
}

/// One question as it is asked now.
struct Plan {
    question: Question,
    key: String,
    asking: String,
    /// The share of its acting answers that must prove right: the owner's, else its declaration's.
    target: f64,
    /// The deciders that may act for it, in the order they are asked: the one the owner pinned once it is measured, then by
    /// standing.
    ranked: Vec<String>,
    /// The decider the owner pinned while it has fewer than `MIN_OUTCOMES` results: it is asked beside the acting one and
    /// only informs, until its own results say whether to trust it.
    on_trial: Option<String>,
}

/// What one decider is asked at once: questions, each with its index among the plans.
struct Call {
    decider: String,
    questions: Vec<(usize, Question)>,
}

/// One decider's answer to the question of the plan at `index`, read as answering it.
struct Answered {
    index: usize,
    /// The pinned id it was asked as.
    decider: String,
    /// The model that answered: the provider's name for the build behind the pinned id.
    model: String,
    answer: JudgmentAnswer,
    /// What it picked for its question.
    pick: JudgmentPick,
    /// Its share of the call's cost.
    cost_usd: f64,
    latency_ms: i64,
}

/// What the code made of an answer: the asking's pick, this answer's probability of it, its use, and the threshold
/// that use was held to.
struct Made {
    pick: JudgmentPick,
    p: f64,
    used: JudgmentUse,
    threshold: Option<f64>,
}

/// What every judgment of one call to `judge` shares.
struct Asked {
    subject: JudgmentSubject,
    state_digest: String,
}

/// The judgments of question that proved right or wrong, by decider.
pub(crate) fn scored_by_decider(scores: &[JudgmentScore], question: &str) -> BTreeMap<String, Vec<Scored>> {
    let mut by: BTreeMap<String, Vec<Scored>> = BTreeMap::new();
    for s in scores.iter().filter(|s| s.question == question) {
        by.entry(s.decider.clone())
            .or_default()
            .push(Scored { p: s.p, right: s.right });
    }
    by
}

/// Whether the decider the owner pinned has results enough to be trusted to act: a model nobody has measured on the
/// question is asked beside the acting one until it has `MIN_OUTCOMES`, so a choice made on a name never decides alone.
fn measured(pinned: &str, scored: &BTreeMap<String, Vec<Scored>>) -> bool {
    scored.get(pinned).is_some_and(|s| s.len() >= judgment::MIN_OUTCOMES)
}

/// The decider pinned that is still on trial: pinned and not yet measured (`measured`).
pub(crate) fn on_trial<'a>(pinned: Option<&'a str>, scored: &BTreeMap<String, Vec<Scored>>) -> Option<&'a str> {
    pinned.filter(|pin| !measured(pin, scored))
}

/// The deciders that may act for a question whose default is default, in their order: the one the owner pinned once it is
/// measured (`steering`), then those its scores rank.
pub(crate) fn ranked_by(default: &str, pinned: Option<&str>, scored: &BTreeMap<String, Vec<Scored>>) -> Vec<String> {
    let standings: Vec<Standing> = scored.iter().map(|(d, s)| Standing::of(d, s)).collect();
    let mut order: Vec<String> = judgment::ranked(default, &standings)
        .into_iter()
        .map(str::to_owned)
        .collect();
    if let Some(pin) = pinned.filter(|pin| measured(pin, scored)) {
        order.retain(|d| d != pin);
        order.insert(0, pin.to_owned());
    }
    order
}

/// The results of decider on the question key that its act threshold is measured over: all its picks, or those of the one
/// side the question acts on (`ActsOn`), where a pick of the other side that proved right says nothing of how often the
/// acting one does.
pub(crate) fn threshold_results(scores: &[JudgmentScore], key: &str, acts_on: ActsOn, decider: &str) -> Vec<Scored> {
    scores
        .iter()
        .filter(|s| s.question == key && s.decider == decider && acts_on.side().is_none_or(|side| s.picked == side))
        .map(|s| Scored { p: s.p, right: s.right })
        .collect()
}

/// The act threshold of decider on the question key held to target: where its own results put it (`threshold_results`),
/// 0.9 until it has `MIN_OUTCOMES`. The one place the bar is worked out.
pub(crate) fn threshold_of(
    scores: &[JudgmentScore],
    key: &str,
    acts_on: ActsOn,
    decider: &str,
    target: f64,
) -> Threshold {
    judgment::act_threshold(&threshold_results(scores, key, acts_on, decider), target)
}

/// What leaves the machine for questions: the state with its credential fields dropped and its texts and field names
/// redacted, as a reading's body is, and each question with every text of it redacted the same way. Every question leaves
/// so today; one that may not leave is a `MayLeave` this stops at.
/// None when a question cannot be asked redacted (two of its options come out named alike).
fn leaving(state: &Value, questions: Vec<Question>) -> Option<(Value, Vec<Question>)> {
    for q in &questions {
        let MayLeave::Redacted = q.definition().may_leave();
    }
    let questions = questions
        .iter()
        .map(|q| q.cleaned(source::clean_text))
        .collect::<Result<Vec<_>, _>>()
        .inspect_err(|err| tracing::warn!(%err, "judge: a question that cannot be asked redacted; System Two decides"))
        .ok()?;
    Some((source::clean_texts(source::redact_value(state)), questions))
}

impl Core {
    /// The acting deciders' answers to questions about state, which is subject, in the questions' order, each recorded
    /// with the shadows asked beside it. None for a question no decider answered in time (or when there is no decider,
    /// or the judgments could not be stored): its point runs its System Two.
    pub async fn judge(
        &self,
        subject: JudgmentSubject,
        state: &Value,
        questions: Vec<Question>,
    ) -> Vec<Option<Judged>> {
        let n = questions.len();
        let unanswered = || std::iter::repeat_with(|| None).take(n).collect();
        let Some(decider) = self.cfg.decider.clone() else {
            return unanswered();
        };
        let keys: Vec<String> = questions.iter().map(Question::key).collect();
        let read = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.judgment_scores(&keys)?, r.settings()?)))
            .await;
        let (scores, settings) = match read {
            Ok(read) => read,
            Err(err) => {
                tracing::warn!(%err, "judge: the deciders' standings could not be read; System Two decides");
                return unanswered();
            }
        };
        let Some((state, questions)) = leaving(state, questions) else {
            return unanswered();
        };
        let state = Arc::new(state);
        let plans: Vec<Plan> = questions.into_iter().map(|q| plan(q, &scores, &settings)).collect();
        let asked = Asked {
            subject,
            state_digest: judgment::state_digest(&state),
        };
        let answers = ask_acting(&decider, &state, &plans).await;
        let now = self.now();
        let mut acting = Vec::new();
        for (p, a) in plans.iter().zip(answers) {
            let Some(a) = a else { continue };
            let definition = p.question.definition();
            let threshold = threshold_of(&scores, &p.key, definition.acts_on(), &a.decider, p.target);
            let chance = judgment::chance(&a.answer, &a.pick);
            let off_side = definition.acts_on().side().is_some_and(|side| !on_side(&a.pick, side));
            let made = Made {
                // a question that acts on one side only is never acted on by a pick of the other
                used: if off_side {
                    JudgmentUse::Escalated
                } else {
                    judgment::use_of(definition, &a.pick, chance, threshold)
                },
                threshold: if definition.has_bar() { threshold.at() } else { None },
                pick: a.pick.clone(),
                p: chance,
            };
            acting.push((a.index, judgment_of(&asked, p, a, made, now)));
        }
        let mut judged: Vec<Option<Judged>> = unanswered();
        for (i, j) in &acting {
            if let Some(slot) = judged.get_mut(*i) {
                *slot = Some(Judged::from(j));
            }
        }
        let picked: Vec<(usize, String, JudgmentPick)> = acting
            .iter()
            .map(|(i, j)| (*i, j.decider.clone(), j.pick.clone()))
            .collect();
        // An answer that would be shown is stored as held: it is shown only in the change that sends it (`Records::shown`),
        // so one whose caller stopped waiting, or whose change never committed, is not recorded as shown. What a question
        // that acts on a no only stores is escalated whatever it would do: the commit that acts on the answer records the
        // act (`Tx::set_judgment_used`), so none reads as acted on that was not.
        let stored: Vec<Judgment> = acting
            .into_iter()
            .map(|(i, mut j)| {
                let definition = plans.get(i).map(|p| p.question.definition());
                let speaks_only_when_sure = definition.is_some_and(|d| !matches!(d.speaks(), judgment::Speaks::Always));
                if speaks_only_when_sure && j.used == JudgmentUse::Informed {
                    j.used = JudgmentUse::Held;
                }
                if definition.is_some_and(|d| d.acts_on() == ActsOn::No) {
                    j.used = JudgmentUse::Escalated;
                }
                j
            })
            .collect();
        if stored.is_empty() {
            return judged;
        }
        if let Err::<_, store::Error>(err) = self.store.update(move |tx| tx.put_judgments(stored)).await {
            // An answer acted on must be on record, so its outcome can be: unrecorded, it is not used.
            tracing::warn!(%err, "judge: the judgments could not be stored; System Two decides");
            return unanswered();
        }
        self.ask_shadows(decider, state, asked, plans, picked);
        judged
    }

    /// Asks each answered question's shadows in the background and records their answers, each scored on its asking's
    /// pick. picked holds, for each answered question, its plan's index, the decider that answered and its pick.
    fn ask_shadows(
        &self,
        decider: Arc<dyn Decider>,
        state: Arc<Value>,
        asked: Asked,
        plans: Vec<Plan>,
        picked: Vec<(usize, String, JudgmentPick)>,
    ) {
        let Some(core) = self.me() else { return };
        self.work.spawn(async move {
            let mut by: BTreeMap<String, Vec<(usize, Question)>> = BTreeMap::new();
            for (i, acting, _) in &picked {
                let Some(p) = plans.get(*i).filter(|p| p.question.definition().scored()) else {
                    continue;
                };
                // A decider ranked before the one that answered gave no answer just now: it is not asked again.
                let skipped: Vec<&String> = p.ranked.iter().take_while(|d| *d != acting).collect();
                // The pinned decider on trial is asked first: its results are what the pin waits for.
                let mut beside: Vec<String> = p.on_trial.iter().filter(|d| *d != acting).cloned().collect();
                for s in judgment::shadows(p.question.definition().default_decider(), acting) {
                    if beside.len() < 2 && !beside.contains(&s) {
                        beside.push(s);
                    }
                }
                for s in beside.into_iter().filter(|s| !skipped.contains(&s)) {
                    by.entry(s).or_default().push((*i, p.question.clone()));
                }
            }
            let calls = by
                .into_iter()
                .map(|(decider, questions)| Call { decider, questions })
                .collect();
            let mut shadows = Vec::new();
            for a in ask(&decider, &state, calls).await {
                let (Some(p), Some((_, _, pick))) = (plans.get(a.index), picked.iter().find(|(i, ..)| *i == a.index))
                else {
                    continue;
                };
                let made = Made {
                    p: judgment::chance(&a.answer, pick),
                    pick: pick.clone(),
                    used: JudgmentUse::Shadow,
                    threshold: None,
                };
                shadows.push(judgment_of(&asked, p, a, made, core.now()));
            }
            if shadows.is_empty() {
                return;
            }
            if let Err::<_, store::Error>(err) = core.store.update(move |tx| tx.put_judgments(shadows)).await {
                tracing::warn!(%err, "judge: the shadows' judgments could not be stored");
            }
        });
    }

    /// Records what the pick of judgment proved, said by source, on it and on every judgment of its asking. Recorded
    /// again, it replaces the one before.
    pub async fn record_outcome(&self, judgment: &str, proved: Proved, source: OutcomeSource) -> Result<()> {
        self.record_outcomes(vec![(judgment.to_owned(), proved)], source).await
    }

    /// Records what each pick proved (judgment id, proved), all said by source, in one transaction: all of them or none.
    /// A judgment no longer stored is left out, as it has no outcome to be scored on.
    pub async fn record_outcomes(&self, proofs: Vec<(String, Proved)>, source: OutcomeSource) -> Result<()> {
        let at = self.now();
        self.store
            .update(move |tx| {
                for (id, proved) in &proofs {
                    match tx.set_judgment_outcome(
                        id,
                        &JudgmentOutcome {
                            proved: *proved,
                            source,
                            at,
                        },
                    ) {
                        Ok(()) | Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                Ok(())
            })
            .await?;
        Ok(())
    }
}

/// question as it is asked now: the deciders that may act for it in their order, and the precision it is held to, by its
/// scores and the owner's steering.
fn plan(question: Question, scores: &[JudgmentScore], settings: &Settings) -> Plan {
    let key = question.key();
    let scored = scored_by_decider(scores, &key);
    let pinned = steering::pinned(settings, question.definition()).map(|p| p.value.as_str());
    Plan {
        asking: new_id("ja_"),
        target: steering::target_precision(settings, question.definition()),
        ranked: ranked_by(question.definition().default_decider(), pinned, &scored),
        on_trial: on_trial(pinned, &scored).map(str::to_owned),
        key,
        question,
    }
}

/// Each plan's acting answer, in the plans' order: its first ranked decider's, else the next's for the questions those
/// before left unanswered, round by round; none for a question no ranked decider answered.
async fn ask_acting(decider: &Arc<dyn Decider>, state: &Arc<Value>, plans: &[Plan]) -> Vec<Option<Answered>> {
    let mut answers: Vec<Option<Answered>> = std::iter::repeat_with(|| None).take(plans.len()).collect();
    for round in 0.. {
        let mut by: BTreeMap<&str, Vec<(usize, Question)>> = BTreeMap::new();
        for (i, p) in plans.iter().enumerate() {
            if answers.get(i).is_some_and(Option::is_none)
                && let Some(d) = p.ranked.get(round)
            {
                by.entry(d).or_default().push((i, p.question.clone()));
            }
        }
        if by.is_empty() {
            break;
        }
        let calls = by
            .into_iter()
            .map(|(decider, questions)| Call {
                decider: decider.to_owned(),
                questions,
            })
            .collect();
        for a in ask(decider, state, calls).await {
            if let Some(slot) = answers.get_mut(a.index) {
                *slot = Some(a);
            }
        }
    }
    answers
}

/// The judgment a is recorded as, for the question of plan, about what was asked.
fn judgment_of(asked: &Asked, plan: &Plan, a: Answered, made: Made, now: Time) -> Judgment {
    Judgment {
        id: new_id("ju_"),
        asking: plan.asking.clone(),
        question: plan.key.clone(),
        subject: asked.subject.clone(),
        state_digest: asked.state_digest.clone(),
        decider: a.decider,
        model: a.model,
        answer: a.answer,
        pick: made.pick,
        p: made.p,
        cost_usd: a.cost_usd,
        latency_ms: a.latency_ms,
        used: made.used,
        threshold: made.threshold,
        outcome: None,
        created_at: now,
    }
}

/// Every call made at once, each bounded by `DECIDE_FOR`: the answers that came in time and answer their question. A
/// call that failed or was late gives none, logged as its failure deserves.
async fn ask(decider: &Arc<dyn Decider>, state: &Arc<Value>, calls: Vec<Call>) -> Vec<Answered> {
    let mut set = JoinSet::new();
    for call in calls {
        let (decider, state) = (Arc::clone(decider), Arc::clone(state));
        set.spawn(async move { answers_of(decider.as_ref(), &state, call).await });
    }
    let mut out = Vec::new();
    while let Some(done) = set.join_next().await {
        match done {
            Ok(answers) => out.extend(answers),
            Err(err) => tracing::warn!(%err, "judge: a decider's call ended"),
        }
    }
    out
}

/// What call's decider answers in `DECIDE_FOR`: of its questions, those it gave an answer that answers them. A question
/// it refused, left out or answered in another shape has none; the others keep theirs.
async fn answers_of(decider: &dyn Decider, state: &Value, call: Call) -> Vec<Answered> {
    let questions: Vec<Question> = call.questions.iter().map(|(_, q)| q.clone()).collect();
    let started = Instant::now();
    let got = tokio::time::timeout(DECIDE_FOR, decider.decide(&call.decider, state, &questions)).await;
    let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
    let decided = match got {
        Ok(Ok(d)) => d,
        Ok(Err(err)) => return failed(&call.decider, &err),
        Err(_) => {
            let why = format!("no answer in {} ms", DECIDE_FOR.as_millis());
            return failed(&call.decider, &DeciderError::Failed(why));
        }
    };
    let share = decided.cost_usd / f64::from(u32::try_from(questions.len()).unwrap_or(u32::MAX).max(1));
    let answers = decided.answers.into_iter().chain(std::iter::repeat(None));
    call.questions
        .into_iter()
        .zip(answers)
        .filter_map(|((index, q), answer)| {
            let Some(answer) = answer else {
                tracing::info!(
                    decider = call.decider,
                    question = q.key(),
                    "judge: the decider gave no answer to a question"
                );
                return None;
            };
            let Some(pick) = judgment::pick(&q, &answer) else {
                tracing::warn!(
                    decider = call.decider,
                    question = q.key(),
                    ?answer,
                    "judge: an answer that does not answer its question"
                );
                return None;
            };
            Some(Answered {
                index,
                decider: call.decider.clone(),
                model: decided.model.clone(),
                answer,
                pick,
                cost_usd: share,
                latency_ms,
            })
        })
        .collect()
}

/// No answers from decider, logged as err deserves: no key is how a server without one runs.
fn failed(decider: &str, err: &DeciderError) -> Vec<Answered> {
    match err {
        DeciderError::NoKey => tracing::debug!(decider, "judge: no OpenRouter key; System Two decides"),
        DeciderError::Failed(why) => tracing::warn!(decider, why, "judge: the decider gave no answer"),
    }
    Vec::new()
}

/// Whether pick is the side (`yes` or `no`) a question acts on. Pure.
fn on_side(pick: &JudgmentPick, side: &str) -> bool {
    matches!((pick, side), (JudgmentPick::Yes, "yes") | (JudgmentPick::No, "no"))
}
