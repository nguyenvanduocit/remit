//! OpenRouter's embeddings and rerank APIs (ADR-0117): the vectors memories are searched by, and the relevance a
//! cross-encoder gives each candidate for a query; and its Decisions API (ADR-0118): a decision model's calibrated answers
//! to typed questions about a state. The key is read from its file at each call, never held: a key the owner
//! writes or replaces is used at once, and a server with no key answers `Error::NoKey`, which memory reads as "search by
//! words and links". A caller that cannot wait (a turn's recall) bounds a call by dropping it at its own deadline: the
//! request in flight is cancelled with it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Where the APIs are: the stable ones under `v1`, the Decisions API under `alpha`.
const BASE: &str = "https://openrouter.ai/api";

/// How long one try of a call may take.
const CALL_FOR: Duration = Duration::from_secs(30);

/// How many texts one embeddings call carries.
const BATCH: usize = 64;

/// How many times a call is tried when OpenRouter is busy or failing (429, 5xx, a lost connection), and the wait before the
/// second try, doubled for each after.
const TRIES: u32 = 3;
const FIRST_WAIT: Duration = Duration::from_millis(500);

/// How much of an error's body is kept.
const BODY_KEPT: usize = 300;

/// Why a call gave nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The key file is missing or empty.
    #[error("no OpenRouter key in {0}")]
    NoKey(String),
    /// OpenRouter answered with an error.
    #[error("OpenRouter answered {status}: {body}")]
    Status { status: u16, body: String },
    /// The call did not complete: no connection, or it was lost or timed out. Tried again.
    #[error("OpenRouter unreachable: {0}")]
    Unreachable(String),
    /// The answer was not what the API documents. Not tried again: the same request gets the same answer.
    #[error("OpenRouter: {0}")]
    Unreadable(String),
}

/// A client of OpenRouter, keyed by the file at `key_file`.
#[derive(Clone)]
pub struct OpenRouter {
    client: reqwest::Client,
    key_file: PathBuf,
    base: String,
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedItem>,
}

#[derive(Deserialize)]
struct EmbedItem {
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Serialize)]
struct DecideRequest<'a> {
    model: &'a str,
    state: &'a serde_json::Value,
    questions: &'a BTreeMap<String, DecisionQuestion>,
}

/// A typed question as the Decisions API reads it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionQuestion {
    /// Yes or no, with what makes each.
    Noul {
        instructions: String,
        criteria: NoulCriteria,
    },
    /// One of the options, each by its name with what it means.
    Choice {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
    /// A level of the rubric, the lowest first.
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}

/// What makes a yes-or-no question's answer yes (`true`) and no (`false`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoulCriteria {
    #[serde(rename = "true")]
    pub yes: String,
    #[serde(rename = "false")]
    pub no: String,
}

/// The Decisions API's answer: the model that answered (a dated build), an answer to each question by its key, and the
/// call's usage.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Decisions {
    pub model: String,
    pub answers: BTreeMap<String, DecisionAnswer>,
    #[serde(default)]
    pub usage: DecisionUsage,
}

/// One answer, typed as its question; or none to it (`Refusal`, `Other`), which the guide says to read as "not
/// answered", never as a verdict.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionAnswer {
    /// The probability of yes.
    Noul { noul: f64 },
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    /// The expected level, and the probability of each level by its index (`"0"` the lowest).
    Score {
        score: f64,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    /// The provider declined this question (a content policy, say); the others in the call are answered.
    Refusal,
    /// A type the API added after this was written.
    #[serde(other)]
    Other,
}

/// What a decisions call cost, in US dollars.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct DecisionUsage {
    #[serde(default)]
    pub cost: f64,
}

#[derive(Serialize)]
struct RerankRequest<'a> {
    model: &'a str,
    query: &'a str,
    documents: &'a [String],
    top_n: usize,
}

#[derive(Deserialize)]
struct RerankResponse {
    results: Vec<RerankItem>,
}

#[derive(Deserialize)]
struct RerankItem {
    index: usize,
    relevance_score: f32,
}

impl OpenRouter {
    /// A client reading its key from key_file.
    pub fn new(key_file: PathBuf) -> Result<Self, Error> {
        Self::at(key_file, BASE.to_owned())
    }

    /// A client of the API at base: OpenRouter's, or a test's.
    pub fn at(key_file: PathBuf, base: String) -> Result<Self, Error> {
        let client = reqwest::Client::builder()
            .timeout(CALL_FOR)
            .build()
            .map_err(|e| Error::Unreachable(e.to_string()))?;
        Ok(Self { client, key_file, base })
    }

    /// The vectors model gives texts, in their order.
    pub async fn embed(&self, model: &str, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        let mut out = Vec::with_capacity(texts.len());
        for batch in texts.chunks(BATCH) {
            let answer: EmbedResponse = self
                .post("v1/embeddings", &EmbedRequest { model, input: batch })
                .await?;
            out.extend(in_order(
                answer.data.into_iter().map(|d| (d.index, d.embedding)).collect(),
                batch.len(),
            )?);
        }
        Ok(out)
    }

    /// How relevant model judges each of documents to query, in the documents' order: a score from 0 to 1.
    pub async fn rerank(&self, model: &str, query: &str, documents: &[String]) -> Result<Vec<f32>, Error> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let answer: RerankResponse = self
            .post(
                "v1/rerank",
                &RerankRequest {
                    model,
                    query,
                    documents,
                    top_n: documents.len(),
                },
            )
            .await?;
        in_order(
            answer
                .results
                .into_iter()
                .map(|r| (r.index, r.relevance_score))
                .collect(),
            documents.len(),
        )
    }

    /// model's answers to questions (each answered under its key) about state, a text or a JSON object. Whether each
    /// question has an answer, and of its type, is the caller's to read.
    pub async fn decide(
        &self,
        model: &str,
        state: &serde_json::Value,
        questions: &BTreeMap<String, DecisionQuestion>,
    ) -> Result<Decisions, Error> {
        self.post(
            "alpha/decisions",
            &DecideRequest {
                model,
                state,
                questions,
            },
        )
        .await
    }

    async fn key(&self) -> Result<String, Error> {
        let missing = || Error::NoKey(self.key_file.display().to_string());
        let key = tokio::fs::read_to_string(&self.key_file).await.map_err(|_| missing())?;
        let key = key.trim();
        if key.is_empty() {
            return Err(missing());
        }
        Ok(key.to_owned())
    }

    /// Posts body to path and reads the answer, trying again while OpenRouter is busy or failing.
    async fn post<B: Serialize + Sync, A: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<A, Error> {
        let key = self.key().await?;
        let url = format!("{}/{path}", self.base);
        let mut wait = FIRST_WAIT;
        let mut tries: u32 = 0;
        loop {
            tries = tries.saturating_add(1);
            let err = match self.post_once(&url, &key, body).await {
                Ok(a) => return Ok(a),
                Err(err) => err,
            };
            let again = match &err {
                Error::Status { status, .. } => *status == 429 || *status >= 500,
                Error::Unreachable(_) => true,
                Error::NoKey(_) | Error::Unreadable(_) => false,
            };
            if !again || tries >= TRIES {
                return Err(err);
            }
            tracing::debug!(%err, path, "openrouter: tried again");
            tokio::time::sleep(wait).await;
            wait = wait.saturating_mul(2);
        }
    }

    async fn post_once<B: Serialize + Sync, A: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        key: &str,
        body: &B,
    ) -> Result<A, Error> {
        let res = self
            .client
            .post(url)
            .bearer_auth(key)
            .header("X-Title", "Remit")
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Unreachable(e.without_url().to_string()))?;
        let status = res.status();
        let text = res
            .text()
            .await
            .map_err(|e| Error::Unreachable(e.without_url().to_string()))?;
        if !status.is_success() {
            return Err(Error::Status {
                status: status.as_u16(),
                body: text.chars().take(BODY_KEPT).collect(),
            });
        }
        serde_json::from_str(&text).map_err(|e| {
            Error::Unreadable(format!(
                "an answer the API does not document ({e}): {}",
                text.chars().take(BODY_KEPT).collect::<String>()
            ))
        })
    }
}

/// items placed at their index, each of the n there once.
fn in_order<T>(items: Vec<(usize, T)>, n: usize) -> Result<Vec<T>, Error> {
    let mut slots: Vec<Option<T>> = std::iter::repeat_with(|| None).take(n).collect();
    for (i, item) in items {
        let slot = slots
            .get_mut(i)
            .ok_or_else(|| Error::Unreadable(format!("an answer for item {i} of {n}")))?;
        *slot = Some(item);
    }
    slots
        .into_iter()
        .enumerate()
        .map(|(i, s)| s.ok_or_else(|| Error::Unreadable(format!("no answer for item {i} of {n}"))))
        .collect()
}
