//! The facts this server records about itself (ADR-0087): `fact:remit.health` (how it stands), which a release of the
//! repository it runs bakes under (that repository's `.remit/release/guard`, ADR-0116), `fact:remit.updated` (what
//! a run of `remit update` did) and `fact:remit.error` (a new issue among its errors, ADR-0119). Any chain reads and watches them. The Tick records them in the background, one round
//! at a time. What only the binary knows (`/healthz`'s rule and its build) comes through `Instance`, a port the server
//! fills; the decisions of when to record are pure.

use std::sync::{Mutex, PoisonError};

use async_trait::async_trait;
use model::{SignalKey, Time};
use serde::{Deserialize, Serialize};

use crate::Shipping;
use chains::fact::{fact_write, keys};
use chains::outbox::by_remit;
use chains::subagents::SEAT_SERVER_NAME;
use chains::tools::MCP_SERVER_NAME;
use chains::{Error, Result};

/// How far back `fact:remit.health` counts failures and failed tool calls.
const HEALTH_WINDOW: time::Duration = time::Duration::minutes(15);
/// The file `remit update` writes its result to, in the releases directory.
const UPDATE_RESULT: &str = "update.json";

/// How this server stands, as `/healthz` would answer now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstanceHealth {
    /// The commit the binary was built from.
    pub revision: String,
    /// The release it runs from; empty when it runs from elsewhere (a working copy).
    pub tag: String,
    pub started_at: Time,
    /// `/healthz`'s rule.
    pub healthy: bool,
    /// Why it is not healthy; empty while it is.
    pub reason: String,
}

/// What this server knows of itself that only the binary can say.
#[async_trait]
pub trait Instance: Send + Sync {
    /// How it stands now, its scheduler having last got through a Tick at `last_tick` (none before the first).
    async fn health(&self, last_tick: Option<Time>) -> InstanceHealth;
}

/// The value of `fact:remit.health`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct RemitHealth {
    pub revision: String,
    pub tag: String,
    pub started_at: Time,
    pub healthy: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
    /// Failures put to the owner (ADR-0081) over the last `HEALTH_WINDOW`.
    pub failures: i64,
    /// Calls of Remit's own tools that failed over the last `HEALTH_WINDOW`, as the agents' telemetry says (ADR-0085).
    pub tool_errors: u64,
}

/// The value of `fact:remit.updated`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct RemitUpdated {
    pub from: String,
    pub to: String,
    pub healthy: bool,
    pub rolled_back: bool,
    pub one_way: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

/// The value of `fact:remit.error`: one issue among the server's errors, as the line that raised it first said it. The
/// summary says it in a sentence; the rest is what an investigation reads behind it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RemitError {
    pub summary: String,
    /// What groups its lines: where it was logged and its message without numbers.
    pub fingerprint: String,
    /// The module that logged it.
    pub target: String,
    pub message: String,
    /// The error the line named; empty when it named none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    /// The chain and card it was logged in, when it was logged in one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub chain: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub card: String,
}

impl RemitError {
    /// What happened, in a sentence the owner and the chain that investigates read first. Pure.
    pub fn sentence(&self) -> String {
        let what = match (self.message.trim(), self.error.trim()) {
            ("", "") => "an error it did not describe".to_owned(),
            (m, "") => m.to_owned(),
            ("", e) => e.to_owned(),
            (m, e) => format!("{m}: {e}"),
        };
        let place = if self.chain.is_empty() {
            String::new()
        } else {
            format!(" while it worked on chain {}", self.chain)
        };
        format!("The server logged an error it had not logged this week{place}: {what}.")
    }
}

/// `<releases>/update.json` as `remit update` writes it at the end of every run: an id per run, and its outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct UpdateResult {
    id: String,
    #[serde(flatten)]
    outcome: RemitUpdated,
}

/// What the recording of these facts remembers in memory.
#[derive(Default)]
pub struct RemitFacts {
    /// One round at a time: a Tick that finds one at work leaves it.
    round: tokio::sync::Mutex<()>,
    seen: Mutex<Seen>,
}

#[derive(Default)]
struct Seen {
    /// `fact:remit.health` was recorded since this server started.
    health: bool,
    /// The run of `remit update` recorded last.
    update: String,
}

/// The prefixes of the names Claude Code gives Remit's own tools: its server's in every session, and a seat's.
fn own_tool_prefixes() -> [String; 2] {
    [MCP_SERVER_NAME, SEAT_SERVER_NAME].map(|server| format!("mcp__{server}__"))
}

/// Whether `fact:remit.health` is recorded now: once at each start, at each round while a watch on it is open (a
/// subscription that watches it, or a release that bakes under it), and otherwise when `healthy` changes from the last
/// reading (none: a change). Pure.
fn health_due(recorded_since_start: bool, watched: bool, last: Option<bool>, healthy: bool) -> bool {
    !recorded_since_start || watched || last != Some(healthy)
}

impl Shipping {
    /// Records what is due of this server's facts, in the background; the Tick calls it once it got through, so the
    /// health read sees this Tick.
    pub fn record_remit_facts(&self, now: Time) {
        let (Some(inst), Some(me)) = (self.instance.clone(), self.me()) else {
            return;
        };
        self.background(move |_| async move {
            let Ok(_one) = me.remit.round.try_lock() else {
                return;
            };
            if let Err(err) = me.record_health(inst.as_ref(), now).await {
                tracing::warn!(%err, "fact:remit.health not recorded");
            }
            if let Err(err) = me.record_updated().await {
                tracing::warn!(%err, "fact:remit.updated not recorded");
            }
        });
    }

    fn remit_seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.remit.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records `fact:remit.health` when it is due (`health_due`).
    async fn record_health(&self, inst: &dyn Instance, now: Time) -> Result<()> {
        let h = inst.health(self.last_tick()).await;
        let since = Time(now.0.checked_sub(HEALTH_WINDOW).unwrap_or(now.0));
        let key = keys::remit_health();
        let k = key.clone();
        let (failures, watched, last) = self
            .store
            .read(move |r| {
                let failures = r.errors_since(since)?;
                let watched = r.subscriptions_on_key(&k)?.iter().any(model::Subscription::watch)
                    || r.open_releases()?
                        .iter()
                        .any(|rel| rel.guards.iter().any(|g| g.signal == k));
                let last = match r.last_reading(&k) {
                    Ok(last) => Some(last),
                    Err(store::Error::NotFound) => None,
                    Err(err) => return Err(err),
                };
                Ok((failures, watched, last))
            })
            .await?;
        let last_healthy = last
            .and_then(|r| r.value)
            .and_then(|v| serde_json::from_value::<RemitHealth>(v.0).ok())
            .map(|v| v.healthy);
        if !health_due(self.remit_seen().health, watched, last_healthy, h.healthy) {
            return Ok(());
        }
        let tool_errors = match &self.cfg.telemetry {
            Some(t) => {
                let since_ns = i64::try_from(since.0.unix_timestamp_nanos()).unwrap_or(i64::MAX);
                let events = t
                    .events_since(telemetry::TOOL_RESULT, since_ns)
                    .await
                    .map_err(|e| Error::refused(e.to_string()))?;
                let prefixes = own_tool_prefixes();
                telemetry::failed_tools(&events, &prefixes.each_ref().map(String::as_str))
            }
            None => 0,
        };
        let v = RemitHealth {
            revision: h.revision,
            tag: h.tag,
            started_at: h.started_at,
            healthy: h.healthy,
            reason: h.reason,
            failures,
            tool_errors,
        };
        self.record_remit(key, &v, None).await?;
        self.remit_seen().health = true;
        Ok(())
    }

    /// Records `fact:remit.updated` once per run of `remit update`, from the result it left in the releases directory:
    /// at the first round after a start, and at each round while the server outlives the run.
    async fn record_updated(&self) -> Result<()> {
        if self.cfg.releases_dir.as_os_str().is_empty() {
            return Ok(());
        }
        let path = self.cfg.releases_dir.join(UPDATE_RESULT);
        let text = match tokio::fs::read(&path).await {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(Error::refused(format!("{}: {err}", path.display()))),
        };
        let run: UpdateResult =
            serde_json::from_slice(&text).map_err(|e| Error::refused(format!("{}: {e}", path.display())))?;
        if run.id.is_empty() || self.remit_seen().update == run.id {
            return Ok(());
        }
        self.record_remit(keys::remit_updated(), &run.outcome, Some(run.id.clone()))
            .await?;
        self.remit_seen().update = run.id;
        Ok(())
    }

    /// Records `fact:remit.error` for issue e, once per issue: the fact's dedupe is its fingerprint, so a line that
    /// repeats an issue the week's readings hold already stores nothing. Its cause names the chain it was logged in.
    pub async fn record_remit_error(&self, mut e: RemitError) -> Result<()> {
        e.summary = e.sentence();
        let mut w = fact_write(keys::remit_error(), &e, by_remit(), &e.chain)?;
        w.dedupes = vec![format!("issue:{}", e.fingerprint)];
        let door = self.door();
        self.store.update(move |tx| door.ingest(tx, w).map(|_| ())).await?;
        self.kick();
        Ok(())
    }

    /// Stores the reading v of key, written by Remit; once per `once` when one is given (a retry or a restart stores it
    /// no second time). The Tick its kick asks for judges it.
    async fn record_remit(&self, key: SignalKey, v: &impl Serialize, once: Option<String>) -> Result<()> {
        let mut w = fact_write(key, v, by_remit(), "")?;
        w.dedupes = once.into_iter().collect();
        let door = self.door();
        self.store.update(move |tx| door.ingest(tx, w).map(|_| ())).await?;
        self.kick();
        Ok(())
    }
}
