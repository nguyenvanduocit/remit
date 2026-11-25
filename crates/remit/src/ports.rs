//! The orchestrator's ports filled with what lives elsewhere: the accounts' config dirs and the transcripts are the AI
//! runtime's, asked over the link; the models memory is searched and judged with, and the deciders small judgments are
//! asked of, are OpenRouter's. Each adapter only translates.

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

use async_trait::async_trait;

/// The link the core speaks to its runtime on.
pub type RuntimeLink = link_quic::Link;

/// The accounts' config dirs, which live in the AI runtime (`orch::AccountDirs`).
pub struct LinkAccounts(pub runtimelink::Accounts<RuntimeLink>);

fn agent_err(e: runtimelink::Error) -> agent::Error {
    agent::Error::Prepare(e.to_string())
}

#[async_trait]
impl orch::AccountDirs for LinkAccounts {
    async fn prepare(&self, id: &str) -> Result<(), agent::Error> {
        self.0.prepare(id).await.map_err(agent_err)
    }

    async fn sign_in(&self, id: &str) -> Result<Arc<dyn orch::Login>, agent::Error> {
        let login = self.0.sign_in(id).await.map_err(agent_err)?;
        Ok(Arc::new(LinkLogin(Arc::new(login))))
    }

    async fn email(&self, id: &str) -> Result<String, agent::Error> {
        self.0.email(id).await.map_err(agent_err)
    }

    async fn discard(&self, id: &str) -> Result<(), agent::Error> {
        self.0.discard(id).await.map_err(agent_err)
    }
}

/// A sign-in in progress in the runtime, as the orchestrator holds one (`orch::Login`).
struct LinkLogin(Arc<runtimelink::Login<RuntimeLink>>);

#[async_trait]
impl orch::Login for LinkLogin {
    fn url(&self) -> String {
        self.0.url().to_owned()
    }

    async fn code(&self, code: &str) -> Result<(), agent::Error> {
        self.0.code(code.to_owned()).await.map_err(agent_err)
    }

    fn done(&self) -> agent::BoxFuture<'static, ()> {
        let login = Arc::clone(&self.0);
        Box::pin(async move { login.done().await })
    }

    fn err(&self) -> Option<String> {
        self.0.err().map(str::to_owned)
    }

    fn cancel(&self) {
        let login = Arc::clone(&self.0);
        // The orchestrator asks without waiting; the runtime ends the sign-in, and `done` follows.
        tokio::spawn(async move { login.cancel().await });
    }
}

/// The transcripts Claude Code writes where the runtime runs it (`orch::Transcripts`), read whole over the link.
pub struct LinkTranscripts(pub runtimelink::Transcripts<RuntimeLink>);

#[async_trait]
impl orch::Transcripts for LinkTranscripts {
    async fn open(&self, cwd: &str, session_id: &str) -> (String, io::Result<Box<dyn io::Read + Send>>) {
        let mut t = match self.0.open(cwd, session_id).await {
            Ok(t) => t,
            Err(runtimelink::Error::NoTranscript(path)) => {
                return (
                    path,
                    Err(io::Error::new(io::ErrorKind::NotFound, "no transcript written yet")),
                );
            }
            Err(e) => return (String::new(), Err(io::Error::other(e.to_string()))),
        };
        let path = t.path().to_owned();
        let mut all = Vec::new();
        loop {
            match t.next_part().await {
                Ok(Some(part)) => all.extend(part),
                Ok(None) => break,
                Err(e) => return (path, Err(io::Error::other(e.to_string()))),
            }
        }
        let read: Box<dyn io::Read + Send> = Box::new(io::Cursor::new(all));
        (path, Ok(read))
    }
}

/// OpenRouter's embeddings and rerank APIs (`orch::MemoryModels`).
pub struct OpenRouterModels(pub openrouter::OpenRouter);

/// What OpenRouter's failure e means to memory (`orch::ModelError`): no key, the account refused (401, 402, 403), the
/// input refused (another 4xx but a timeout or a rate limit, which pass), or anything else, asked again later.
fn model_error(e: openrouter::Error) -> orch::ModelError {
    match e {
        openrouter::Error::NoKey(_) => orch::ModelError::NoKey,
        openrouter::Error::Status { status: 401..=403, .. } => orch::ModelError::Account(e.to_string()),
        openrouter::Error::Status { status, .. } if (400..500).contains(&status) && status != 408 && status != 429 => {
            orch::ModelError::Rejected(e.to_string())
        }
        e => orch::ModelError::Failed(e.to_string()),
    }
}

/// OpenRouter's Decisions API (`judgment::Decider`, ADR-0118): each question asked under its index.
pub struct OpenRouterDecider(pub openrouter::OpenRouter);

#[async_trait]
impl judgment::Decider for OpenRouterDecider {
    async fn decide(
        &self,
        decider: &str,
        state: &serde_json::Value,
        questions: &[judgment::Question],
    ) -> Result<judgment::Decided, judgment::DeciderError> {
        let asked: BTreeMap<String, openrouter::DecisionQuestion> = questions
            .iter()
            .enumerate()
            .map(|(i, q)| (format!("q{i}"), decision_question(q)))
            .collect();
        let mut got = self.0.decide(decider, state, &asked).await.map_err(|e| match e {
            openrouter::Error::NoKey(_) => judgment::DeciderError::NoKey,
            e => judgment::DeciderError::Failed(e.to_string()),
        })?;
        let answers = (0..questions.len())
            .map(|i| got.answers.remove(&format!("q{i}")).and_then(judgment_answer))
            .collect();
        Ok(judgment::Decided {
            model: got.model,
            answers,
            cost_usd: got.usage.cost,
        })
    }
}

/// q as the Decisions API reads it.
fn decision_question(q: &judgment::Question) -> openrouter::DecisionQuestion {
    let instructions = q.instructions().to_owned();
    match q.kind().shape() {
        judgment::Shape::Noul { yes, no } => openrouter::DecisionQuestion::Noul {
            instructions,
            criteria: openrouter::NoulCriteria {
                yes: yes.clone(),
                no: no.clone(),
            },
        },
        judgment::Shape::Choice { options } => openrouter::DecisionQuestion::Choice {
            instructions,
            criteria: options.iter().map(|o| (o.name.clone(), o.criterion.clone())).collect(),
        },
        judgment::Shape::Score { levels } => openrouter::DecisionQuestion::Score {
            instructions,
            criteria: levels.clone(),
        },
    }
}

/// a as a judgment's answer, a score's probabilities keyed by level index put in the levels' order; none for a refusal,
/// a type the API added since, or a score missing a level's probability: that question is not answered.
fn judgment_answer(a: openrouter::DecisionAnswer) -> Option<model::JudgmentAnswer> {
    match a {
        openrouter::DecisionAnswer::Noul { noul } => Some(model::JudgmentAnswer::Noul { p: noul }),
        openrouter::DecisionAnswer::Choice {
            choice,
            probabilities,
            confidence,
        } => Some(model::JudgmentAnswer::Choice {
            choice,
            probabilities,
            confidence,
        }),
        openrouter::DecisionAnswer::Score {
            score,
            mut probabilities,
            confidence,
        } => {
            let by_level = (0..probabilities.len())
                .map(|i| probabilities.remove(&i.to_string()))
                .collect::<Option<_>>()?;
            Some(model::JudgmentAnswer::Score {
                score,
                probabilities: by_level,
                confidence,
            })
        }
        openrouter::DecisionAnswer::Refusal | openrouter::DecisionAnswer::Other => None,
    }
}

#[async_trait]
impl orch::MemoryModels for OpenRouterModels {
    async fn embed(&self, model: &str, texts: &[String]) -> Result<Vec<Vec<f32>>, orch::ModelError> {
        self.0.embed(model, texts).await.map_err(model_error)
    }

    async fn rerank(&self, model: &str, query: &str, documents: &[String]) -> Result<Vec<f32>, orch::ModelError> {
        self.0.rerank(model, query, documents).await.map_err(model_error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use judgment::{ChoiceOption, Decider, Kind, Question};
    use model::JudgmentAnswer;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::OpenRouterDecider;

    /// The response example of OpenRouter's Decisions guide (openrouter.ai/docs/guides/features/decisions.md,
    /// "Response example", read 2026-11-19), its keys renamed to the ones Remit asks under, with a fourth question
    /// refused as its "Handling provider refusals" shows.
    const GUIDE: &str = r#"{
      "id": "gen-dec-1791209504-cXRBLHSCBmsjQX93jkWe",
      "model": "typesafe/jev-1.14",
      "provider": "TypeSafe",
      "answers": {
        "q0": { "type": "noul", "noul": 0.0412, "confidence": 0.9588 },
        "q1": {
          "type": "choice",
          "choice": "billing",
          "probabilities": { "billing": 0.9821, "technical_support": 0.0142, "sales": 0.0037 },
          "confidence": 0.9821
        },
        "q2": { "type": "score", "score": 1.84, "probabilities": { "0": 0.01, "1": 0.14, "2": 0.85 }, "confidence": 0.85 },
        "q3": { "type": "refusal" }
      },
      "usage": { "input_tokens": 128, "output_tokens": 0, "cost": 0.0000384, "input_tokens_details": { "cached_tokens": 0 } }
    }"#;

    /// A server that answers one request with body and status 200.
    async fn serve(body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();
            let mut buf = vec![0_u8; 64 * 1024];
            let _ = conn.read(&mut buf).await.unwrap();
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            conn.write_all(reply.as_bytes()).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn o(name: &str) -> ChoiceOption {
        ChoiceOption {
            name: name.to_owned(),
            criterion: format!("{name} work"),
        }
    }

    #[tokio::test]
    async fn the_guides_answers_read_as_judgments_and_a_refused_question_as_none() {
        // As main does: the client is built on rustls with no provider of its own.
        rustls::crypto::ring::default_provider().install_default().ok();
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("openrouter.key");
        std::fs::write(&key, "sk-test").unwrap();
        let client = openrouter::OpenRouter::at(key, serve(GUIDE).await).unwrap();
        let levels = vec!["low".to_owned(), "medium".to_owned(), "high".to_owned()];
        let questions = vec![
            Question::new(
                &judgment::GATE_TOUCHES_RAIL,
                Kind::noul("it does", "it does not").unwrap(),
            )
            .unwrap(),
            Question::new(
                &judgment::INBOX_OPTION,
                Kind::choice(vec![o("billing"), o("technical_support"), o("sales")]).unwrap(),
            )
            .unwrap(),
            Question::new(&judgment::GATE_RISK, Kind::score(levels).unwrap()).unwrap(),
            Question::new(
                &judgment::MEMORY_SAME_CLAIM,
                Kind::noul("the same", "different").unwrap(),
            )
            .unwrap(),
            Question::new(&judgment::CARD_PROGRESS, Kind::noul("moved", "stuck").unwrap()).unwrap(),
        ];
        let got = OpenRouterDecider(client)
            .decide(
                judgment::DEFAULT_DECIDER,
                &serde_json::json!({"memory": "a"}),
                &questions,
            )
            .await
            .unwrap();
        assert_eq!(got.model, "typesafe/jev-1.14");
        assert!((got.cost_usd - 0.000_038_4).abs() < 1e-12);
        let probabilities: BTreeMap<String, f64> =
            [("billing", 0.9821), ("technical_support", 0.0142), ("sales", 0.0037)]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect();
        assert_eq!(
            got.answers,
            vec![
                Some(JudgmentAnswer::Noul { p: 0.0412 }),
                Some(JudgmentAnswer::Choice {
                    choice: "billing".to_owned(),
                    probabilities,
                    confidence: 0.9821
                }),
                Some(JudgmentAnswer::Score {
                    score: 1.84,
                    probabilities: vec![0.01, 0.14, 0.85],
                    confidence: 0.85
                }),
                None,
                None,
            ],
            "the refused question and the one left out are not answered; the others are"
        );
    }
}
