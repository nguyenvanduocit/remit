//! What a running server says when it is asked from outside, as its supervisor and its owner ask: `/healthz`, and a
//! hello with the owner key on its link, which only a server that serves the owner's channel welcomes. `remit update`
//! waits on these and `remit doctor` reports them; neither changes anything on the server.

use std::path::Path;
use std::time::Duration;

use link::{Hello, Node};
use remit_client::dial::{self, Down, LinkJson};
use serde_json::Value;

/// How long one probe may take.
const PROBE_WAIT: Duration = Duration::from_secs(5);

/// The server's address as HTTP is reached at it: `-addr`.
pub fn http(addr: &str) -> String {
    format!("http://{addr}")
}

/// What `/healthz` answered: its status and its body.
#[derive(Debug, Clone, PartialEq)]
pub struct Healthz {
    pub status: u16,
    pub body: Value,
}

impl Healthz {
    /// The commit the server says it runs, empty when it says none.
    pub fn revision(&self) -> &str {
        self.text("revision")
    }

    /// Why it is not healthy, empty when it says nothing.
    pub fn reason(&self) -> &str {
        self.text("reason")
    }

    /// The commit was built from a tree with uncommitted changes.
    pub fn modified(&self) -> bool {
        self.body.get("modified").and_then(Value::as_bool) == Some(true)
    }

    pub fn ok(&self) -> bool {
        self.status == 200 && self.body.get("ok").and_then(Value::as_bool) == Some(true)
    }

    fn text(&self, field: &str) -> &str {
        self.body.get(field).and_then(Value::as_str).unwrap_or_default()
    }
}

/// The server's `/healthz`, or why it did not answer.
pub async fn healthz(addr: &str) -> Result<Healthz, String> {
    let at = format!("{}/healthz", http(addr));
    let client = reqwest::Client::builder()
        .timeout(PROBE_WAIT)
        .build()
        .map_err(|e| e.to_string())?;
    let res = client.get(&at).send().await.map_err(|e| format!("{at}: {e}"))?;
    let status = res.status().as_u16();
    let body = res.bytes().await.map_err(|e| format!("{at}: {e}"))?;
    let body = serde_json::from_slice(&body).map_err(|e| format!("{at} answered no JSON: {e}"))?;
    Ok(Healthz { status, body })
}

/// The server's `/link.json`.
pub async fn link_json(addr: &str) -> Result<LinkJson, String> {
    tokio::time::timeout(PROBE_WAIT, dial::link_json(&http(addr)))
        .await
        .map_err(|_| format!("{}/link.json did not answer in {}s", http(addr), PROBE_WAIT.as_secs()))?
}

/// The owner key a server with its data at data comes in with, None when it cannot be read.
pub fn owner_key(data: &Path) -> Option<String> {
    std::fs::read_to_string(data.join("owner.key"))
        .ok()
        .map(|k| k.trim().to_owned())
        .filter(|k| !k.is_empty())
}

/// Says hello on the link `link` names, as the owner's channel with `key` (empty: none): welcomed, or why not.
pub async fn hello(link: &LinkJson, key: &str) -> Result<(), Down> {
    let (url, trust) = dial::trust_of(link).map_err(Down::Unreachable)?;
    let hello = Hello {
        node: Node {
            id: "remit-probe".to_owned(),
            public_key: String::new(),
        },
        credential: key.to_owned(),
        capabilities: vec![wire::app::CAPABILITY.to_owned()],
        offers: Vec::new(),
        bundle: None,
    };
    match tokio::time::timeout(PROBE_WAIT, dial::open_at(url, trust, hello)).await {
        Ok(Ok(_peer)) => Ok(()),
        Ok(Err(down)) => Err(down),
        Err(_) => Err(Down::Unreachable(format!(
            "no answer on the link in {}s: is UDP to its port let through?",
            PROBE_WAIT.as_secs()
        ))),
    }
}
