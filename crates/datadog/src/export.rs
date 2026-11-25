//! The exporter: soon after records are put it takes what waits, lays it out in requests under the intake's limits and
//! sends each over `http/protobuf` with the key. The site and the key are read at each send and never held, so a key the
//! owner enters or replaces is used at once. With no key what waits is let go, nothing more is kept and nothing leaves;
//! the account is looked at again every little while. A request that fails for a reason that may pass (the network, 408,
//! 429, 5xx) is retried with backoff until stop; at stop what waits is sent once more, each request tried once, and the
//! exporter ends a few seconds after stop whatever is left.

use std::future::Future;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use crate::deploy;
use crate::queue::requests;
use crate::{Datadog, Signal, Site};

/// Who Remit sends to Datadog as: the site and the API key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    pub site: Site,
    pub api_key: String,
}

/// Where the exporter reads the account it sends as, at each send.
pub trait Account: Send + Sync + 'static {
    /// The site and the key; none while the owner has not entered a key; an error when they cannot be read.
    fn login(&self) -> impl Future<Output = Result<Option<Login>, String>> + Send;
}

/// How long what was put waits for more before it is sent.
const FLUSH_EVERY: Duration = Duration::from_secs(2);
/// How often the account is read when nothing is put, so a key entered while nothing is kept is found.
const LOOK_EVERY: Duration = Duration::from_secs(30);
/// How long the exporter goes on after stop: the send in flight and what still waits.
const AFTER_STOP: Duration = Duration::from_secs(3);
/// How often one request is tried, and the wait before the first retry, doubled at each.
const TRIES: u32 = 5;
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
/// How long one request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// How often the exporter says what it could not send, at most.
const SAY_EVERY: Duration = Duration::from_secs(60);

impl Datadog {
    /// Sends what is put as account until stop, then for `AFTER_STOP` at most.
    pub async fn run(self, account: impl Account, stop: CancellationToken) {
        let http = match reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build() {
            Ok(c) => c,
            Err(err) => {
                tracing::error!(%err, "datadog: no HTTP client: nothing is sent to Datadog");
                return;
            }
        };
        let after_stop = async {
            stop.cancelled().await;
            tokio::time::sleep(AFTER_STOP).await;
        };
        tokio::select! {
            () = self.work(&http, &account, &stop) => {}
            () = after_stop => tracing::warn!("datadog: stopped before everything waiting was sent"),
        }
    }

    async fn work(&self, http: &reqwest::Client, account: &impl Account, stop: &CancellationToken) {
        let mut said = Said::default();
        loop {
            let stopping = tokio::select! {
                () = stop.cancelled() => true,
                () = self.inner.queue.put.notified() => {
                    tokio::select! {
                        () = stop.cancelled() => {}
                        () = tokio::time::sleep(FLUSH_EVERY) => {}
                    }
                    false
                }
                () = tokio::time::sleep(LOOK_EVERY) => false,
            };
            self.flush(http, account, &mut said, stop).await;
            if stopping {
                return;
            }
        }
    }

    async fn flush(&self, http: &reqwest::Client, account: &impl Account, said: &mut Said, stop: &CancellationToken) {
        let items = self.inner.queue.take();
        let (login, readable) = match account.login().await {
            Ok(l) => {
                said.unreadable = None;
                (l, true)
            }
            Err(why) => {
                said.unreadable(why);
                (None, false)
            }
        };
        self.inner.wanted.store(login.is_some(), Ordering::Relaxed);
        *self.inner.site.lock().unwrap_or_else(PoisonError::into_inner) = login.as_ref().map(|l| l.site.clone());
        let Some(login) = login else {
            self.inner.delivering.store(false, Ordering::Relaxed);
            // no key: nothing is kept, deployments included; an account that could not be read keeps them for the next try
            if readable {
                drop(self.inner.take_deployments());
            }
            return;
        };
        let base = self.inner.intake.clone().unwrap_or_else(|| login.site.intake());
        let key = login.api_key.trim();
        let api = self.inner.api.clone().unwrap_or_else(|| login.site.api());
        for d in self.inner.take_deployments() {
            // one Datadog refuses (DORA Metrics is not on for the organisation, say) is said and let go
            if let Err(why) = deploy::send(http, &api, key, &d, stop).await {
                said.failed(&why);
            }
        }
        for (signal, body) in requests(items) {
            let sent = self.send(http, &base, key, signal, body, stop).await;
            self.inner.delivering.store(sent.is_ok(), Ordering::Relaxed);
            if let Err(why) = sent {
                said.failed(&why);
            }
        }
        said.dropped(self.inner.dropped());
    }

    /// Sends one request of records to base, retrying what may pass until stop.
    async fn send(
        &self,
        http: &reqwest::Client,
        base: &str,
        key: &str,
        signal: Signal,
        body: Vec<u8>,
        stop: &CancellationToken,
    ) -> Result<(), String> {
        // the headers docs.datadoghq.com/opentelemetry/setup/otlp_ingest/{logs,metrics,traces} show, read 2026-11-20
        let mut headers = vec![("content-type", "application/x-protobuf")];
        if signal == Signal::Traces {
            // the intake computes APM's trace metrics only when asked
            headers.push(("compute_stats", "true"));
        }
        post(http, &format!("{base}{}", signal.path()), key, &headers, body, stop)
            .await
            .map_err(|why| format!("{signal:?} to {why}"))
    }
}

/// Posts body to url with the key and headers, retrying what may pass (the network, 408, 429, 5xx) with backoff until
/// stop; the error says where and why the last try failed.
pub(crate) async fn post(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
    stop: &CancellationToken,
) -> Result<(), String> {
    let mut wait = FIRST_BACKOFF;
    let mut last = String::new();
    for n in 1..=TRIES {
        let mut req = http.post(url).header("dd-api-key", key).body(body.clone());
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        match req.send().await {
            Ok(r) if r.status().is_success() => return Ok(()),
            Ok(r) => {
                let status = r.status().as_u16();
                last = format!("{url}: {status}");
                if !(status == 408 || status == 429 || status >= 500) {
                    return Err(last);
                }
            }
            Err(err) => last = format!("{url}: {err}"),
        }
        if n < TRIES {
            tokio::select! {
                () = stop.cancelled() => return Err(last),
                () = tokio::time::sleep(wait) => {}
            }
            wait = wait.saturating_mul(2);
        }
    }
    Err(last)
}

/// What the exporter said last, so a Datadog that stays unreachable is said once a minute, not at every send, and an
/// account that cannot be read once until it can.
#[derive(Default)]
struct Said {
    at: Option<Instant>,
    dropped: u64,
    unreadable: Option<String>,
}

impl Said {
    fn due(&mut self) -> bool {
        let due = self.at.is_none_or(|at| at.elapsed() >= SAY_EVERY);
        if due {
            self.at = Some(Instant::now());
        }
        due
    }

    fn unreadable(&mut self, why: String) {
        if self.unreadable.as_ref() != Some(&why) {
            tracing::warn!(
                why,
                "datadog: the Datadog source's site and key could not be read: nothing is sent"
            );
            self.unreadable = Some(why);
        }
    }

    fn failed(&mut self, why: &str) {
        if self.due() {
            tracing::warn!(
                why,
                "datadog: a request to the intake failed and its records were let go"
            );
        }
    }

    fn dropped(&mut self, total: u64) {
        if total > self.dropped && self.due() {
            tracing::warn!(
                dropped = total,
                "datadog: the queue was full and dropped its oldest records"
            );
            self.dropped = total;
        }
    }
}
