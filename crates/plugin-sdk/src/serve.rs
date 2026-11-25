//! The child side of an applied extension: a `Plugin` answering `wire::plugin` over `link` on the process's stdio. Stdout is
//! the link, so a plugin writes its logs to stderr.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use link::{Closing, Config, Conn, ErrorCode, Failure, Hello, Node, Request};
use serde_json::Value;
use tokio::task::JoinHandle;
use wire::Ack;
use wire::plugin::{Firing, Manifest, PLUGIN_CONTRACT, PollAnswer, PollAsk, ProcessAnswer, SenseOpen, Vars, forge};

use crate::pipe::{Note, Peer, Serve};
use crate::plugin::{Forge, Plugin, Sensed};

/// Runs `plugin` as a child of Remit and returns when Remit ends the session. It is the whole of a plugin's `main`.
pub fn serve(plugin: impl Plugin + 'static) -> ExitCode {
    if std::io::stdin().is_terminal() {
        eprintln!("This program is a Remit plugin: Remit starts it and speaks to it on its stdin and stdout.");
        return ExitCode::FAILURE;
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("plugin: start the runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let served = runtime.block_on(run(plugin));
    // A read of stdin may still wait in a blocking thread; the process ends without it.
    runtime.shutdown_background();
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("plugin: {why}");
            ExitCode::FAILURE
        }
    }
}

async fn run(mut plugin: impl Plugin + 'static) -> Result<(), String> {
    let (peer, idle) = Peer::new();
    // The plugin always asks; Remit answers FORBIDDEN unless its welcome granted forge.read.
    plugin.grant_forge(Arc::new(LinkForge(peer.clone())));
    let served = Arc::new(Served {
        plugin: Box::new(plugin),
        peer: peer.clone(),
        sensors: Arc::new(Mutex::new(HashMap::new())),
    });
    let serve: Serve = Arc::new(move |contract, method, body| {
        let served = Arc::clone(&served);
        Box::pin(async move {
            if contract == wire::plugin::CAPABILITY {
                wire::plugin::serve(&*served, &method, &body).await
            } else {
                Err(Failure::new(
                    ErrorCode::Forbidden,
                    format!("a plugin answers no {contract}"),
                ))
            }
        })
    });
    let hello = Hello {
        node: Node {
            id: "plugin".to_owned(),
            public_key: String::new(),
        },
        credential: String::new(),
        capabilities: vec![forge::CAPABILITY.to_owned()],
        offers: vec![wire::plugin::CAPABILITY.to_owned()],
        bundle: Some(PLUGIN_CONTRACT.to_string()),
    };
    let conn = Conn::dial(Config::default(), hello);
    let mut notes = idle.run(conn, tokio::io::stdin(), tokio::io::stdout(), serve);
    while let Some(note) = notes.recv().await {
        match note {
            Note::Refused(refusal) => return Err(format!("Remit refused the plugin: {}", refusal.reason)),
            Note::Closed(Closing::Done) => return Ok(()),
            Note::Closed(closing) => return Err(format!("the session ended: {closing:?}")),
            Note::Hello(_) | Note::Welcomed(_) | Note::Push { .. } => {}
        }
    }
    Ok(())
}

/// The forge as a child reaches it: Remit's `forge.read` over the link.
struct LinkForge(Peer);

#[async_trait]
impl Forge for LinkForge {
    async fn checks(&self, host: String, project: String, sha: String) -> Result<Value, Failure> {
        self.0
            .call(forge::client::checks(host, project, sha))
            .await
            .map(|s| s.0)
    }
}

/// The runs of the open sensors by sensor id.
type Runs = Arc<Mutex<HashMap<String, JoinHandle<()>>>>;

/// The plugin as it answers `wire::plugin`, and the runs of its open sensors.
struct Served {
    plugin: Box<dyn Plugin>,
    peer: Peer,
    sensors: Runs,
}

impl Served {
    fn sensors(&self) -> std::sync::MutexGuard<'_, HashMap<String, JoinHandle<()>>> {
        runs(&self.sensors)
    }
}

fn runs(sensors: &Runs) -> std::sync::MutexGuard<'_, HashMap<String, JoinHandle<()>>> {
    sensors.lock().unwrap_or_else(PoisonError::into_inner)
}

impl wire::plugin::Handler for Served {
    async fn describe(&self) -> Result<Manifest, Failure> {
        self.plugin.describe().await
    }

    async fn poll(&self, ask: PollAsk) -> Result<PollAnswer, Failure> {
        self.plugin.poll(ask).await
    }

    async fn asset(&self, name: String) -> Result<wire::plugin::Asset, Failure> {
        Ok(self.plugin.asset(name).await?.to_wire())
    }

    async fn account(&self, vars: Vars) -> Result<Vars, Failure> {
        self.plugin.account(vars).await
    }

    async fn call(
        &self,
        request: Request,
        tool: String,
        arguments: model::ForeignJson,
        vars: Vars,
    ) -> Result<String, Failure> {
        self.plugin.call(request, tool, arguments.0, vars).await
    }

    async fn process(&self, firing: Firing) -> Result<ProcessAnswer, Failure> {
        self.plugin.process(firing).await
    }

    async fn sense_open(&self, sensor: String, open: SenseOpen) -> Result<Ack, Failure> {
        if self.sensors().contains_key(&sensor) {
            return Ok(Ack {});
        }
        let mut senses = self.plugin.sense(open).await?;
        let (peer, all) = (self.peer.clone(), Arc::clone(&self.sensors));
        let id = sensor.clone();
        // Held while the run starts, so a run that ends at once removes itself after it was put in.
        let mut sensors = self.sensors();
        let run = tokio::spawn(async move {
            let home = Some(id.clone());
            let error = loop {
                let push = match senses.next().await {
                    Some(Ok(Sensed::Reading(reading))) => wire::plugin::Push::reading {
                        sensor: id.clone(),
                        reading,
                    },
                    Some(Ok(Sensed::Cursor(cursor))) => wire::plugin::Push::cursor {
                        sensor: id.clone(),
                        cursor,
                    },
                    Some(Err(failure)) => break Some(failure.message),
                    None => break Some(String::new()),
                };
                if peer.push(home.clone(), push).await.is_err() {
                    break None;
                }
            };
            if let Some(error) = error {
                let ended = wire::plugin::Push::ended {
                    sensor: id.clone(),
                    error,
                };
                peer.push(home, ended).await.ok();
            }
            // The run is over: its entry goes, unless a sensor opened again under the id since holds it.
            let me = tokio::task::id();
            let mut open = runs(&all);
            if open.get(&id).is_some_and(|run| run.id() == me) {
                open.remove(&id);
            }
        });
        sensors.insert(sensor, run);
        Ok(Ack {})
    }

    async fn sense_close(&self, sensor: String) -> Result<Ack, Failure> {
        if let Some(run) = self.sensors().remove(&sensor) {
            run.abort();
        }
        Ok(Ack {})
    }
}
