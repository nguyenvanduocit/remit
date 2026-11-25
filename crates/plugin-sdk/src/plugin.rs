//! The one contract every runner of a plugin keeps (ADR-0072 decision 7): a plugin that ships with Remit implements it and is
//! called in-process; an applied extension is a child process that `serve`s one, and Remit calls it through an adapter that
//! implements the same trait over `link`. The gate decides on every call before it reaches a `Plugin`.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use link::{ErrorCode, Failure, Request};
use serde_json::Value;
use tokio::sync::mpsc;
use wire::plugin::{Firing, Manifest, PollAnswer, PollAsk, ProcessAnswer, Reading, SenseOpen, Vars};

/// A plugin, however it runs. A failure's code says what happened: `NOT_FOUND` for a tool, kind, processor or file it does not
/// have, `INVALID_ARGUMENT` for input that is not what it declared, `UNAVAILABLE` for a plugin that could not be started,
/// `DISCONNECTED` for one whose process went (a call is not repeated: a tool that ran half way may have acted), and the
/// plugin's own code and words otherwise.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Remit grants the plugin `forge.read` (ADR-0057): what it asks of the forge is answered by `forge`. Granted before the
    /// plugin is first called; a plugin never granted it is answered `FORBIDDEN`.
    fn grant_forge(&mut self, forge: Arc<dyn Forge>);

    /// What it says it is: the owner is shown it before he applies it.
    async fn describe(&self) -> Result<Manifest, Failure>;

    /// Runs one tool; `request` is the agent's call, the same on a retry of it.
    async fn call(&self, request: Request, tool: String, arguments: Value, vars: Vars) -> Result<String, Failure>;

    /// The events after a cursor.
    async fn poll(&self, ask: PollAsk) -> Result<PollAnswer, Failure>;

    /// Opens one sensor, which senses until the `Senses` is dropped or it stops by itself.
    async fn sense(&self, open: SenseOpen) -> Result<Senses, Failure>;

    /// One firing through one of its processors.
    async fn process(&self, firing: Firing) -> Result<ProcessAnswer, Failure>;

    /// One file of its frontend.
    async fn asset(&self, name: String) -> Result<Asset, Failure>;

    /// The environment a session runs with on the account the connection of `vars` is (ADR-0071), unchecked: its caller
    /// parses it.
    async fn account(&self, vars: Vars) -> Result<Vars, Failure>;
}

/// The forge's one question, as Remit answers a plugin granted `forge.read`.
#[async_trait]
pub trait Forge: Send + Sync {
    /// The checks the pipeline of `project` on `host` ran on commit `sha`: a `model::Checks` as JSON, or null while it has
    /// run none.
    async fn checks(&self, host: String, project: String, sha: String) -> Result<Value, Failure>;
}

/// One thing a sensor said.
#[derive(Debug, Clone, PartialEq)]
pub enum Sensed {
    Reading(Reading),
    /// The cursor to keep and hand back when the sensor is opened again.
    Cursor(String),
}

/// What one open sensor says, in order: `Sensed` items, then at most one failure, then the end. Dropping it ends the sensor.
pub struct Senses {
    rx: mpsc::UnboundedReceiver<Result<Sensed, Failure>>,
    end: Option<Box<dyn FnOnce() + Send>>,
}

impl Senses {
    /// The sensor whose items come on `rx`; `end` stops it, and runs when this is dropped before the sensor stopped by itself.
    pub fn new(rx: mpsc::UnboundedReceiver<Result<Sensed, Failure>>, end: impl FnOnce() + Send + 'static) -> Senses {
        Senses {
            rx,
            end: Some(Box::new(end)),
        }
    }

    /// The next thing the sensor said; none once it stopped.
    pub async fn next(&mut self) -> Option<Result<Sensed, Failure>> {
        let next = self.rx.recv().await;
        if next.is_none() {
            self.end = None;
        }
        next
    }
}

impl Drop for Senses {
    fn drop(&mut self) {
        if let Some(end) = self.end.take() {
            end();
        }
    }
}

/// One file of a plugin's frontend; `content_type` is empty when unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub content: Vec<u8>,
    pub content_type: String,
}

impl Asset {
    /// The file as the wire carries it.
    pub fn to_wire(&self) -> wire::plugin::Asset {
        wire::plugin::Asset {
            content: STANDARD.encode(&self.content),
            content_type: self.content_type.clone(),
        }
    }

    pub fn from_wire(asset: wire::plugin::Asset) -> Result<Asset, Failure> {
        let content = STANDARD
            .decode(asset.content)
            .map_err(|e| Failure::new(ErrorCode::InternalError, format!("the asset is not base64: {e}")))?;
        Ok(Asset {
            content,
            content_type: asset.content_type,
        })
    }
}
