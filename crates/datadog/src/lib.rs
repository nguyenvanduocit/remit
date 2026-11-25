//! Everything Remit runs, sent to Datadog (ADR-0119): the server's own logs and spans through its `tracing` subscriber
//! (`layer`), what the agents' sessions export to the `/otlp` door, stamped with their chain and put under their turn
//! (`forward`), and what a page reported failing (`page`). One exporter sends it all to the site's OTLP intake over `http/protobuf` with the one key only the server holds (`run`),
//! from a queue that drops its oldest when Datadog cannot be reached and never holds the work up, every record redacted
//! by the sources' rules (`redact`). The site and the key are the Datadog source's, read at each send
//! (`Account`); with no key nothing is kept or leaves, and nothing else changes.
//!
//! The exporter is where sampling would go, and pointing it at a Collector instead of the intake changes nothing else
//! (the ADR's *Revisit when*).

mod deploy;
mod export;
mod forward;
mod layer;
mod page;
mod queue;
mod redact;
mod site;
mod spans;
mod turns;

#[cfg(test)]
mod sink;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue, any_value};
use opentelemetry_proto::tonic::resource::v1::Resource;

pub use deploy::Deployment;
pub use export::{Account, Login};
pub use forward::{Refused, Stamp};
pub use page::SERVICE_WEB;
pub use site::{NotASite, Signal, Site};
pub use turns::{Turn, Turns};

/// The service each kind of process reports as (Datadog's unified service tagging).
pub const SERVICE_SERVER: &str = "remit";
pub const SERVICE_AGENT: &str = "remit-agent";

/// The scope the server's own records come under.
const SCOPE: &str = "remit";

/// Where Remit sends to Datadog, and what waits to go. Cloning it shares it.
#[derive(Clone, Debug)]
pub struct Datadog {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    /// The site the last read of the account found: where the owner reads what was sent.
    site: Mutex<Option<Site>>,
    /// Where the records go in place of the site's intake: a test's sink.
    intake: Option<String>,
    /// Where the deployments go in place of the site's API: a test's sink. A Collector in place of the intake has no
    /// deployment endpoint, so this is not the intake's.
    api: Option<String>,
    /// `env` and `version` of every record.
    env: String,
    version: String,
    queue: queue::Queue,
    /// The releases published that wait to be told: few, so a plain list; the exporter sends them with the records.
    deployments: Mutex<Vec<Deployment>>,
    /// How many of them were dropped, the oldest, since the start: said with the queue's.
    deployments_dropped: AtomicU64,
    turns: Turns,
    /// The last read of the account found a key: until it does not, records are kept to send.
    wanted: AtomicBool,
    /// The last request the intake took went through: the owner's chain screens link to Datadog only then.
    delivering: AtomicBool,
    /// What the server's own records come under.
    server: Arc<Resource>,
    scope: Arc<InstrumentationScope>,
}

impl Datadog {
    /// Every record tagged env and version (the release tag). Nothing leaves before `run`, and only to the site the
    /// account names.
    pub fn new(env: &str, version: &str) -> Self {
        Self::to(None, None, env, version)
    }

    /// `new`, its records sent to intake and its deployments to api instead of the site's.
    fn to(intake: Option<String>, api: Option<String>, env: &str, version: &str) -> Self {
        let server = Arc::new(resource(SERVICE_SERVER, env, version));
        Self {
            inner: Arc::new(Inner {
                intake,
                api,
                site: Mutex::new(None),
                env: env.to_owned(),
                version: version.to_owned(),
                queue: queue::Queue::default(),
                deployments: Mutex::default(),
                deployments_dropped: AtomicU64::new(0),
                turns: Turns::default(),
                // until the first read, what the server logs as it starts is kept
                wanted: AtomicBool::new(true),
                delivering: AtomicBool::new(false),
                server,
                scope: Arc::new(InstrumentationScope {
                    name: SCOPE.to_owned(),
                    version: version.to_owned(),
                    ..InstrumentationScope::default()
                }),
            }),
        }
    }

    /// Whether records reach Datadog: the last request went through.
    pub fn sending(&self) -> bool {
        self.inner.delivering.load(Ordering::Relaxed)
    }

    /// Where the owner reads what chain did, its traces, logs and errors, while records reach Datadog; none otherwise.
    pub fn chain_links(&self, chain: &str) -> Vec<(&'static str, String)> {
        if !self.sending() {
            return Vec::new();
        }
        let site = self.inner.site.lock().unwrap_or_else(PoisonError::into_inner);
        site.as_ref().map(|s| s.chain_links(chain)).unwrap_or_default()
    }
}

impl Datadog {
    /// Has Datadog told of a release published, on the exporter's next send. It never waits: with no key nothing is kept
    /// and nothing leaves, and a deployment the intake refuses is said once and let go.
    pub fn deployed(&self, d: Deployment) {
        if !self.inner.wanted() {
            return;
        }
        let mut waiting = self.inner.deployments.lock().unwrap_or_else(PoisonError::into_inner);
        // the oldest goes first when Datadog stays unreachable: releases come minutes apart, never by hundreds
        if waiting.len() >= MAX_DEPLOYMENTS {
            waiting.remove(0);
            self.inner.deployments_dropped.fetch_add(1, Ordering::Relaxed);
        }
        waiting.push(d);
        drop(waiting);
        self.inner.queue.put.notify_one();
    }
}

/// How many deployments wait at most.
const MAX_DEPLOYMENTS: usize = 100;

impl Inner {
    /// Whether a record made now is kept to send.
    fn wanted(&self) -> bool {
        self.wanted.load(Ordering::Relaxed)
    }

    /// How many records and deployments were dropped since the start.
    fn dropped(&self) -> u64 {
        self.queue
            .dropped()
            .saturating_add(self.deployments_dropped.load(Ordering::Relaxed))
    }

    fn take_deployments(&self) -> Vec<Deployment> {
        std::mem::take(&mut *self.deployments.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// The resource of a process of service, with Datadog's unified service tags.
fn resource(service: &str, env: &str, version: &str) -> Resource {
    Resource {
        attributes: vec![
            text("service.name", service),
            // Datadog reads env from either name, the second the older semantic convention
            text("deployment.environment.name", env),
            text("deployment.environment", env),
            text("service.version", version),
        ],
        ..Resource::default()
    }
}

/// An attribute whose value is text.
fn text(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(any_value::Value::StringValue(value.to_owned())),
        }),
        ..KeyValue::default()
    }
}

/// An attribute whose value is a whole number.
fn int(key: &str, value: i64) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(any_value::Value::IntValue(value)),
        }),
        ..KeyValue::default()
    }
}
