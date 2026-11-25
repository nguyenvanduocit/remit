//! The HTTP client every REST source uses.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde::Serialize;

use crate::error::{Error, Result, StatusError, err, fail};
use crate::redact::redact_text;
use crate::text::limit;

/// How long one tool call may take, the request to the system included.
pub(crate) const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// The largest answer read from a system: a bigger one is refused, never held.
const MAX_BODY: usize = 4 << 20;
/// How much of a failed answer an error carries.
const ERROR_SNIPPET: usize = 300;
const MAX_REDIRECTS: usize = 5;

/// The parameters of a query, in the order given; they are encoded sorted by name.
pub type Query = Vec<(String, String)>;

/// A REST system reached through its credentials. The address is the credential's, so an agent cannot point a call at
/// another host; a path is checked; the verb is GET, or POST for an endpoint the source names as a read (a search that takes
/// its query in a body), or any other for `send`, which only a tool of an effect above read calls; a redirect to another host
/// is not followed; an answer over 4 MiB is refused.
#[derive(Clone)]
pub struct Api {
    /// The system, as an error names it.
    pub name: String,
    /// scheme://host[:port][/prefix]
    pub base: String,
    /// The credential, set on every request.
    pub headers: Vec<(String, String)>,
    pub http: Http,
}

/// The HTTP client the sources and plugins it is handed to share: a pool of connections, the call's deadline, TLS with the
/// system's roots and the rule that a redirect stays on its host. It is built on first use, so what is never called never
/// reads the system's roots. Cloning it shares it.
#[derive(Clone, Default)]
pub struct Http(Arc<OnceLock<std::result::Result<reqwest::Client, String>>>);

impl Http {
    /// A client not built yet, shared by every clone.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The client, or why it could not be built.
    pub fn client(&self) -> Result<reqwest::Client> {
        self.0.get_or_init(http_client).clone().map_err(|e| err!("{e}"))
    }
}

/// A new client with the deadline, the redirect rule and the system's roots, for a caller that holds its own.
pub fn http_client() -> std::result::Result<reqwest::Client, String> {
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        // A system certificate rustls cannot read is one fewer authority, not a reason to have no client.
        if roots.add(cert).is_err() {
            continue;
        }
    }
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .timeout(CALL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(same_host))
        .build()
        .map_err(|e| cause_chain(&e.without_url()))
}

fn same_host(attempt: reqwest::redirect::Attempt<'_>) -> reqwest::redirect::Action {
    let first = attempt
        .previous()
        .first()
        .map(|u| (u.host_str().map(str::to_owned), u.port_or_known_default()));
    let here = (
        attempt.url().host_str().map(str::to_owned),
        attempt.url().port_or_known_default(),
    );
    if attempt.previous().len() >= MAX_REDIRECTS || first.as_ref() != Some(&here) {
        return attempt.stop();
    }
    attempt.follow()
}

/// The text of an error and every cause under it.
pub(crate) fn cause_chain(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        let more = c.to_string();
        if !text.contains(&more) {
            text = format!("{text}: {more}");
        }
        cause = c.source();
    }
    text
}

impl Api {
    /// The answer to GET `path` with `query`.
    pub async fn get(&self, path: &str, query: &[(String, String)]) -> Result<Vec<u8>> {
        self.exchange(reqwest::Method::GET, path, query, None).await
    }

    /// The answer to a POST of `body` to `path`. Only for an endpoint that reads.
    pub async fn post_json(&self, path: &str, body: &impl Serialize) -> Result<Vec<u8>> {
        let b = serde_json::to_vec(body).map_err(|e| err!("{}: {e}", self.name))?;
        self.exchange(reqwest::Method::POST, path, &[], Some(b)).await
    }

    /// The answer to a request of `method` with `body` as JSON (none when `body` is None). It is for a tool whose effect is
    /// above read: the gate has ranked it and asked the chain's grant before it runs, so `get` and `post_json` keep the
    /// read-only rule.
    pub async fn send(&self, method: reqwest::Method, path: &str, body: Option<&serde_json::Value>) -> Result<Vec<u8>> {
        let b = match body {
            Some(v) => Some(serde_json::to_vec(v).map_err(|e| err!("{}: {e}", self.name))?),
            None => None,
        };
        self.exchange(method, path, &[], b).await
    }

    async fn exchange(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(String, String)],
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        let target = self.target(path, query)?;
        let client = self.http.client().map_err(|e| e.context(&self.name))?;
        let mut req = client.request(method, target).header("Accept", "application/json");
        if let Some(b) = body {
            req = req.header("Content-Type", "application/json").body(b);
        }
        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let mut res = req
            .send()
            .await
            .map_err(|e| err!("{}: {}", self.name, redact_text(&cause_chain(&e.without_url()))))?;
        let mut data = Vec::new();
        while let Some(chunk) = res
            .chunk()
            .await
            .map_err(|e| err!("{}: reading the answer: {}", self.name, cause_chain(&e.without_url())))?
        {
            data.extend_from_slice(&chunk);
            if data.len() > MAX_BODY {
                return fail!(
                    "{}: the answer is larger than {} MiB: narrow the request",
                    self.name,
                    MAX_BODY >> 20
                );
            }
        }
        let status = res.status();
        if !status.is_success() {
            let text = String::from_utf8_lossy(&data);
            return Err(Error::Status(StatusError {
                system: self.name.clone(),
                code: status.as_u16(),
                status: format!("{} {}", status.as_u16(), status.canonical_reason().unwrap_or_default())
                    .trim()
                    .to_owned(),
                body: redact_text(limit(&text, ERROR_SNIPPET).trim()),
            }));
        }
        Ok(data)
    }

    /// The address of `path` under the base, or why the path is refused. A path is rooted, names no parent directory and no
    /// other host, and carries no query (the query is given apart, so it is encoded once).
    pub(crate) fn target(&self, path: &str, query: &[(String, String)]) -> Result<String> {
        let ok_base = url::Url::parse(&self.base)
            .ok()
            .is_some_and(|b| matches!(b.scheme(), "http" | "https") && b.host_str().is_some_and(|h| !h.is_empty()));
        if !ok_base {
            return fail!(
                "{}: the address {:?} is not an http or https address",
                self.name,
                self.base
            );
        }
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.contains(['?', '#', '\\', '\0', '\r', '\n'])
            || path.contains("..")
            || path.contains("://")
        {
            return fail!("{}: the path {path:?} is not one this tool reads", self.name);
        }
        let mut target = format!("{}{path}", self.base.trim_end_matches('/'));
        if !query.is_empty() {
            target.push('?');
            target.push_str(&encode_query(query));
        }
        Ok(target)
    }
}

/// The query as a string: sorted by name (values of one name in their order), each escaped by `urlescape::query_value`.
#[must_use]
pub fn encode_query(query: &[(String, String)]) -> String {
    let mut sorted: Vec<&(String, String)> = query.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    sorted
        .iter()
        .map(|(k, v)| format!("{}={}", urlescape::query_value(k), urlescape::query_value(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// The pairs of a query string (`+` is a space).
#[must_use]
pub fn parse_query(q: &str) -> Query {
    url::form_urlencoded::parse(q.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// `(name, value)` pairs from string slices.
#[must_use]
pub fn query(pairs: &[(&str, &str)]) -> Query {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}
