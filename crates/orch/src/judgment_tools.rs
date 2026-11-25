//! `ask_decider`, the tool of the `judgment` capability (ADR-0118, point 7): an agent asks a decision model yes/no, choice
//! or score questions it writes about a state it sends, and gets a calibrated probability for each in about a tenth of a
//! second, for a fraction of a cent. It is the same `Core::judge` Remit's own points ask through, so the state leaves
//! redacted as theirs does (`MayLeave`), and each answer is recorded on the asking card as a judgment of the question
//! `agent.ask`. No point of Remit scores an agent's question, so it has no outcome, no standing and no shadows: the answer
//! informs the agent, which decides.

use std::sync::Arc;

use chains::tools::{Effect, Tools, as_json};
use chains::{Error, Result};
use judgment::{AGENT_ASK, ChoiceOption, Kind, Question, decider_name, pick_words};
use model::{Card, JudgmentSubject, SubjectKind};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::orch::Orchestrator;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AskDeciderInput {
    /// what the questions are about, as you gathered it: text, an object or a list (log lines to screen, candidates to rank,
    /// claims with their citations). A decision model reads it and nothing else; it is sent redacted as a source's reading
    /// is, so quote only what the questions need
    state: Value,
    /// the questions, each answered on its own from the state
    questions: Vec<AskInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AskInput {
    /// the question in plain words, about the state ("Does this log line show the deploy failing?")
    ask: String,
    /// yes_no (answered as the probability of yes: give yes and no), choice (one of options) or score (a level of levels)
    form: AskForm,
    /// for yes_no: what makes the answer yes
    #[serde(default)]
    yes: String,
    /// for yes_no: what makes the answer no
    #[serde(default)]
    no: String,
    /// for choice: at least two options, each with its name and what it means
    #[serde(default)]
    options: Vec<AskOption>,
    /// for score: at least two levels, the lowest first, each said in words
    #[serde(default)]
    levels: Vec<String>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum AskForm {
    YesNo,
    Choice,
    Score,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AskOption {
    /// what the answer names when it picks this option
    name: String,
    /// what makes this option the answer
    means: String,
}

impl AskInput {
    /// The question as the layer asks it: checked as every question is, so one that could not be answered is refused here,
    /// in words the agent can fix.
    fn question(&self) -> std::result::Result<Question, judgment::Invalid> {
        let kind = match self.form {
            AskForm::YesNo => Kind::noul(&self.yes, &self.no),
            AskForm::Choice => Kind::choice(
                self.options
                    .iter()
                    .map(|o| ChoiceOption {
                        name: o.name.clone(),
                        criterion: o.means.clone(),
                    })
                    .collect(),
            ),
            AskForm::Score => Kind::score(self.levels.clone()),
        }?;
        Question::worded(&AGENT_ASK, &self.ask, kind)
    }
}

impl Orchestrator {
    /// Gives card the tool that asks a decision model about what it gathered.
    pub(crate) fn add_decider_tool(self: &Arc<Self>, t: &mut Tools, card: &Card) {
        let (id, home) = (card.id.clone(), card.home.clone());
        t.serving(self).add(
            "ask_decider", Effect::Writes,
            "Ask a decision model questions about a state you send: it answers each in about a tenth of a second, for a fraction of a cent, with a calibrated probability and no text. Use it to screen a thousand log lines for the ones that matter, rank candidates, or check whether a claim is backed by its citation, before you spend a turn on them. Write each question in plain words with what makes each answer (yes and no, the options or the levels): the criteria are what make it accurate. It reads only the state you send, so its probability is an estimate to weigh against what you know, never a verdict; a question it gives no answer to says so, and you decide it yourself. What you send leaves this machine redacted as a source's reading does, and each answer is recorded on your card.",
            move |o, i: AskDeciderInput| {
                let (id, home) = (id.clone(), home.clone());
                async move {
                    if i.state.is_null() || i.state.as_str().is_some_and(|s| s.trim().is_empty()) {
                        return Err(Error::refused("send the state the questions are about"));
                    }
                    if i.questions.is_empty() {
                        return Err(Error::refused("ask at least one question"));
                    }
                    let questions = i
                        .questions
                        .iter()
                        .enumerate()
                        .map(|(n, a)| {
                            a.question()
                                .map_err(|why| Error::refused(format!("question {}: {why}", n.saturating_add(1))))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let subject = JudgmentSubject {
                        kind: SubjectKind::Card,
                        id,
                        home,
                    };
                    let answers = o.judge(subject, &i.state, questions).await;
                    let answered: Vec<Value> = i
                        .questions
                        .iter()
                        .zip(answers)
                        .map(|(q, a)| match a {
                            Some(j) => json!({
                                "ask": q.ask,
                                "answer": pick_words(&j.pick),
                                "sure": j.p,
                                "decider": decider_name(&j.decider),
                                "detail": j.answer,
                            }),
                            None => json!({
                                "ask": q.ask,
                                "answer": null,
                                "why": "no decision model answered it in time (none is configured, it refused the question, or it was late): decide it yourself",
                            }),
                        })
                        .collect();
                    as_json(&answered)
                }
            },
        );
    }
}
