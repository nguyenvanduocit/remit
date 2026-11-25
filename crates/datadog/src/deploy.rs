//! A release's deployment event (ADR-0119, *Products lead back*): what Datadog is told when Remit publishes a version, so
//! an error in a product that runs it leads to the landings it carries and, by the `remit.chain_id` tags, to their
//! chains. It goes through Datadog's DORA deployment endpoint, `POST https://api.<site>/api/v2/dora/deployment` with
//! the `DD-API-KEY` header; `service`, `started_at` and `finished_at` (epoch nanoseconds) are required, `git` carries
//! `repository_url` and `commit_sha`, `custom_tags` are `key:value` strings and an optional `id` identifies the deployment
//! (docs.datadoghq.com/dora_metrics/setup/deployments, read 2026-11-20; that Datadog takes a second send of one `id` as one
//! deployment is not in the page: an assumption). The page gives no limit on the tags, so Remit caps them at
//! `MAX_TAGS`, the most tags the events API takes (docs.datadoghq.com/api/latest/events/post-an-event.md, same day), and
//! cuts the tags after them. Pure but for `send`.

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::export::post;

/// One release published, as Datadog is told of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployment {
    /// The release's id: the deployment's.
    pub id: String,
    /// What was deployed: the repository's project name, as its services are named.
    pub service: String,
    pub version: String,
    /// Where the commit is, and which it is: what Datadog's suspect commits and lead time read.
    pub repository_url: String,
    pub commit: String,
    /// When its steps began and ended, in nanoseconds since the epoch.
    pub started_at: i128,
    pub finished_at: i128,
    /// `key:value` tags, most important first: the release, the chains it carries, their landings. Past `MAX_TAGS` the
    /// last are cut.
    pub tags: Vec<String>,
}

/// The most tags a deployment is sent with.
const MAX_TAGS: usize = 100;

impl Deployment {
    /// The request body.
    pub(crate) fn body(&self) -> Vec<u8> {
        let nanos = |n: i128| i64::try_from(n).unwrap_or(i64::MAX);
        let mut attributes = serde_json::Map::new();
        for (k, v) in [
            ("id", json!(self.id)),
            ("service", json!(self.service)),
            ("started_at", json!(nanos(self.started_at))),
            ("finished_at", json!(nanos(self.finished_at))),
            (
                "custom_tags",
                json!(self.tags.iter().take(MAX_TAGS).collect::<Vec<_>>()),
            ),
        ] {
            attributes.insert(k.to_owned(), v);
        }
        if !self.version.is_empty() {
            attributes.insert("version".to_owned(), json!(self.version));
        }
        if !self.repository_url.is_empty() && !self.commit.is_empty() {
            attributes.insert(
                "git".to_owned(),
                json!({"repository_url": self.repository_url, "commit_sha": self.commit}),
            );
        }
        json!({"data": {"attributes": attributes}}).to_string().into_bytes()
    }
}

/// Posts d to the API at base, retrying what may pass until stop.
pub(crate) async fn send(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    d: &Deployment,
    stop: &CancellationToken,
) -> Result<(), String> {
    let headers = [("content-type", "application/json"), ("accept", "application/json")];
    post(
        http,
        &format!("{base}/api/v2/dora/deployment"),
        key,
        &headers,
        d.body(),
        stop,
    )
    .await
    .map_err(|why| format!("deployment {} to {why}", d.id))
}
