//! An applied extension as a `Plugin`: every call reaches the plugin's process over `link`, starting it first when none runs.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use link::{ErrorCode, Failure, Outgoing, Request};
use plugin_sdk::{Asset, Forge, Plugin, Senses};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{Mutex, mpsc};
use wire::plugin::{Call, Firing, Manifest, PollAnswer, PollAsk, ProcessAnswer, SenseOpen, Vars, client};

pub use crate::start::Stopped;
use crate::start::{Process, sensors, start};

/// The program of one plugin and the hash it must have: a file with another hash does not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub id: String,
    pub binary: PathBuf,
    /// What the program is started with; none for a plugin that is a program of its own.
    pub args: Vec<String>,
    pub sha256: String,
}

/// One applied extension. Its process starts on the first call and again on the first call after it died; a call that found
/// it dead fails `DISCONNECTED` and is not repeated, as a tool that ran half way may have acted. Dropping it kills the process.
pub struct RemotePlugin {
    spec: Spec,
    forge: Option<Arc<dyn Forge>>,
    running: Mutex<Option<Arc<Process>>>,
    next_sensor: AtomicU64,
}

impl RemotePlugin {
    pub fn new(spec: Spec) -> RemotePlugin {
        RemotePlugin {
            spec,
            forge: None,
            running: Mutex::new(None),
            next_sensor: AtomicU64::new(0),
        }
    }

    /// Ends the process, if it runs: a plugin removed or replaced is not left running.
    pub async fn stop(&self) -> Option<Stopped> {
        let process = self.running.lock().await.take()?;
        Some(process.shutdown().await)
    }

    /// The running process, started when none runs or the one that ran has ended.
    async fn process(&self) -> Result<Arc<Process>, Failure> {
        let mut running = self.running.lock().await;
        if let Some(p) = running.as_ref().filter(|p| !p.exited()) {
            return Ok(Arc::clone(p));
        }
        if let Some(old) = running.take() {
            old.kill().await;
        }
        let p = start(&self.spec, self.forge.clone())
            .await
            .map(Arc::new)
            .map_err(|reason| Failure::new(ErrorCode::Unavailable, format!("plugin {}: {reason}", self.spec.id)))?;
        *running = Some(Arc::clone(&p));
        Ok(p)
    }

    async fn ask<A: DeserializeOwned + Send + 'static>(&self, out: Outgoing<Call, A>) -> Result<A, Failure> {
        let p = self.process().await?;
        match p.peer.call(out).await {
            Ok(answer) => Ok(answer),
            Err(failure) => Err(self.fail(&p, failure).await),
        }
    }

    /// How a call to `p` that answered `failure` failed. A session that ended is a process that went: it is dropped, unless
    /// it was replaced already, so the next call starts a fresh one. Any other failure is the plugin's own answer.
    async fn fail(&self, p: &Arc<Process>, failure: Failure) -> Failure {
        if failure.code != ErrorCode::Disconnected && !p.exited() {
            return failure;
        }
        let mut running = self.running.lock().await;
        if running.as_ref().is_some_and(|q| Arc::ptr_eq(q, p)) {
            running.take();
            p.kill().await;
        }
        Failure::new(
            ErrorCode::Disconnected,
            format!("plugin {} could not be reached: {}", self.spec.id, failure.message),
        )
    }
}

#[async_trait]
impl Plugin for RemotePlugin {
    /// The grant reaches the plugin in the `welcome` of its next process.
    fn grant_forge(&mut self, forge: Arc<dyn Forge>) {
        self.forge = Some(forge);
    }

    async fn describe(&self) -> Result<Manifest, Failure> {
        self.ask(client::describe()).await
    }

    async fn call(&self, request: Request, tool: String, arguments: Value, vars: Vars) -> Result<String, Failure> {
        self.ask(client::call(request, tool, arguments.into(), vars)).await
    }

    async fn poll(&self, ask: PollAsk) -> Result<PollAnswer, Failure> {
        self.ask(client::poll(ask)).await
    }

    async fn sense(&self, open: SenseOpen) -> Result<Senses, Failure> {
        let p = self.process().await?;
        let sensor = self.next_sensor.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        let registered = sensors(&p.sensors)
            .as_mut()
            .map(|open| open.insert(sensor.clone(), tx))
            .is_some();
        if !registered {
            let failure = Failure::new(ErrorCode::Disconnected, "the session ended");
            return Err(self.fail(&p, failure).await);
        }
        if let Err(failure) = p.peer.call(client::sense_open(sensor.clone(), open)).await {
            if let Some(open) = sensors(&p.sensors).as_mut() {
                open.remove(&sensor);
            }
            return Err(self.fail(&p, failure).await);
        }
        let (peer, open) = (p.peer.clone(), Arc::clone(&p.sensors));
        Ok(Senses::new(rx, move || {
            if let Some(open) = sensors(&open).as_mut() {
                open.remove(&sensor);
            }
            peer.call_detached(client::sense_close(sensor));
        }))
    }

    async fn process(&self, firing: Firing) -> Result<ProcessAnswer, Failure> {
        self.ask(client::process(firing)).await
    }

    async fn asset(&self, name: String) -> Result<Asset, Failure> {
        Asset::from_wire(self.ask(client::asset(name)).await?)
    }

    async fn account(&self, vars: Vars) -> Result<Vars, Failure> {
        self.ask(client::account(vars)).await
    }
}
