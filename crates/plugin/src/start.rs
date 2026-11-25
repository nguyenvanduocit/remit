//! Starting one plugin process: the hash pin, checked before exec; the host environment withheld; the plugin's `hello`
//! admitted, granting `forge.read` only to a plugin Remit granted the forge; and ending it: the session closed, a grace,
//! then a kill.

use std::collections::HashMap;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use link::{Config, Conn, ErrorCode, Failure, Hello, Node, Refusal, RefusalCode};
use plugin_sdk::pipe::{Note, Peer, Serve};
use plugin_sdk::{Forge, Sensed};
use sha2::{Digest, Sha256};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use wire::plugin::{PLUGIN_CONTRACT, forge};

use crate::remote::Spec;

const START_TIMEOUT: Duration = Duration::from_secs(15);
const GRACE: Duration = Duration::from_secs(2);

pub(crate) type Sink = mpsc::UnboundedSender<Result<Sensed, Failure>>;

/// The open sensors of a process by sensor id; none once its session ended.
pub(crate) type Sensors = Arc<Mutex<Option<HashMap<String, Sink>>>>;

pub(crate) fn sensors(s: &Sensors) -> MutexGuard<'_, Option<HashMap<String, Sink>>> {
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How a plugin process ended when it was stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// It exited by itself within the grace once its session closed.
    Exited(ExitStatus),
    /// It was killed.
    Killed,
}

/// One running plugin process.
pub(crate) struct Process {
    child: tokio::sync::Mutex<Child>,
    pub(crate) peer: Peer,
    pub(crate) sensors: Sensors,
}

impl Process {
    /// Whether the process ended or its session did; false while another caller is ending it.
    pub(crate) fn exited(&self) -> bool {
        if sensors(&self.sensors).is_none() {
            return true;
        }
        match self.child.try_lock() {
            Ok(mut child) => !matches!(child.try_wait(), Ok(None)),
            Err(_) => false,
        }
    }

    /// Closes the session, which closes the plugin's stdin, gives it the grace to exit, and kills it then.
    pub(crate) async fn shutdown(&self) -> Stopped {
        self.peer.close();
        let mut child = self.child.lock().await;
        match tokio::time::timeout(GRACE, child.wait()).await {
            Ok(Ok(status)) => Stopped::Exited(status),
            _ => {
                child.kill().await.ok();
                Stopped::Killed
            }
        }
    }

    /// Kills the process now, for one that died or cannot be reached.
    pub(crate) async fn kill(&self) {
        self.child.lock().await.kill().await.ok();
    }
}

/// Starts the program `spec` names, refusing one whose hash is not the one applied.
pub(crate) async fn start(spec: &Spec, forge: Option<Arc<dyn Forge>>) -> Result<Process, String> {
    check_hash(&spec.binary, &spec.sha256).await?;
    #[expect(
        clippy::disallowed_methods,
        reason = "a plugin starts with an empty environment (env_clear) and its life is held by Process, which kills it on drop"
    )]
    let mut child = Command::new(&spec.binary)
        .args(&spec.args)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Remit reads nothing a plugin logs.
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("start {}: {e}", spec.binary.display()))?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err("the plugin has no stdin or stdout pipe".into());
    };
    let granted = forge.is_some();
    let (peer, idle) = Peer::new();
    let node = Node {
        id: "remit".to_owned(),
        public_key: String::new(),
    };
    let mut notes = idle.run(Conn::accept(Config::default(), node), stdout, stdin, serve_forge(forge));
    let hello = match tokio::time::timeout(START_TIMEOUT, notes.recv()).await {
        Ok(Some(Note::Hello(hello))) => hello,
        Ok(_) => return Err("the plugin exited before it said hello".into()),
        Err(_) => return Err("timeout while waiting for the plugin's hello".into()),
    };
    match admission(&hello, granted) {
        Ok(capabilities) => peer.admit(capabilities),
        Err(refusal) => {
            let reason = refusal.reason.clone();
            peer.refuse(refusal);
            return Err(reason);
        }
    }
    let sensors: Sensors = Arc::new(Mutex::new(Some(HashMap::new())));
    tokio::spawn(route(notes, Arc::clone(&sensors), spec.id.clone()));
    Ok(Process {
        child: tokio::sync::Mutex::new(child),
        peer,
        sensors,
    })
}

/// What a plugin's hello is granted: `forge.read` when it asks and Remit granted it the forge, nothing else. A plugin of
/// another contract, or one that answers no plugin calls, is refused.
fn admission(hello: &Hello, forge: bool) -> Result<Vec<String>, Refusal> {
    let contract = PLUGIN_CONTRACT.to_string();
    if hello.bundle.as_deref() != Some(contract.as_str()) {
        return Err(Refusal {
            code: RefusalCode::NewBundle,
            reason: format!(
                "the plugin speaks contract {:?}, not {contract}: build it again",
                hello.bundle.as_deref().unwrap_or("")
            ),
        });
    }
    if !hello.offers.iter().any(|o| o == wire::plugin::CAPABILITY) {
        return Err(Refusal {
            code: RefusalCode::BadCredential,
            reason: "the program offers no plugin calls".to_owned(),
        });
    }
    Ok(hello
        .capabilities
        .iter()
        .filter(|c| forge && *c == forge::CAPABILITY)
        .cloned()
        .collect())
}

/// Answers the plugin's `forge.read` calls with the forge Remit granted; the link refuses them for a plugin it did not grant.
fn serve_forge(forge: Option<Arc<dyn Forge>>) -> Serve {
    Arc::new(move |contract, method, body| {
        let forge = forge.clone();
        Box::pin(async move {
            match forge {
                Some(forge) if contract == forge::CAPABILITY => forge::serve(&Lent(forge), &method, &body).await,
                _ => Err(Failure::new(
                    ErrorCode::Forbidden,
                    format!("{contract} was not granted"),
                )),
            }
        })
    })
}

/// The granted forge as the contract's handler.
struct Lent(Arc<dyn Forge>);

impl forge::Handler for Lent {
    async fn checks(&self, host: String, project: String, sha: String) -> Result<model::ForeignJson, Failure> {
        self.0.checks(host, project, sha).await.map(model::ForeignJson)
    }
}

/// Delivers each sensor's pushes to its `Senses`; when the session ends, every open sensor ends `DISCONNECTED`.
async fn route(mut notes: mpsc::UnboundedReceiver<Note>, sensors: Sensors, id: String) {
    while let Some(note) = notes.recv().await {
        match note {
            Note::Push { contract, name, body } if contract == wire::plugin::CAPABILITY => {
                let mut open = crate::start::sensors(&sensors);
                let Some(open) = open.as_mut() else { return };
                // A push names its sensor inside the body: one that cannot be read cannot be told to its sensor, and a
                // sensor that silently missed a reading would stand on a stale value. Every open sensor ends failed.
                let push = match link::decode_push::<wire::plugin::Push>(&name, &body) {
                    Ok(push) => push,
                    Err(failure) => {
                        let failure = Failure::new(
                            ErrorCode::Failed,
                            format!(
                                "plugin {id} sent a push {name:?} Remit cannot read: {}",
                                failure.message
                            ),
                        );
                        for (_, sink) in std::mem::take(open) {
                            sink.send(Err(failure.clone())).ok();
                        }
                        continue;
                    }
                };
                match push {
                    wire::plugin::Push::reading { sensor, reading } => {
                        if let Some(sink) = open.get(&sensor) {
                            sink.send(Ok(Sensed::Reading(reading))).ok();
                        }
                    }
                    wire::plugin::Push::cursor { sensor, cursor } => {
                        if let Some(sink) = open.get(&sensor) {
                            sink.send(Ok(Sensed::Cursor(cursor))).ok();
                        }
                    }
                    wire::plugin::Push::ended { sensor, error } => {
                        if let Some(sink) = open.remove(&sensor)
                            && !error.is_empty()
                        {
                            sink.send(Err(Failure::new(ErrorCode::Failed, error))).ok();
                        }
                    }
                }
            }
            Note::Closed(closing) => {
                let open = crate::start::sensors(&sensors).take().unwrap_or_default();
                let failure = Failure::new(
                    ErrorCode::Disconnected,
                    format!("plugin {id} could not be reached: its session ended ({closing:?})"),
                );
                for sink in open.into_values() {
                    sink.send(Err(failure.clone())).ok();
                }
                return;
            }
            Note::Hello(_) | Note::Welcomed(_) | Note::Refused(_) | Note::Push { .. } => {}
        }
    }
}

async fn check_hash(binary: &Path, want: &str) -> Result<(), String> {
    let want = hex::decode(want)
        .ok()
        .filter(|sum| sum.len() == 32)
        .ok_or("sha256 is not the 64 hex digits of a hash")?;
    let body = tokio::fs::read(binary)
        .await
        .map_err(|e| format!("read {}: {e}", binary.display()))?;
    if Sha256::digest(&body).as_slice() != want.as_slice() {
        return Err("checksums did not match: the program is not the one applied".into());
    }
    Ok(())
}
