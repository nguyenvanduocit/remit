//! How the page starts its client, as JSON, once. What it asks of the core and what it is told after are
//! `replica::Ask` and `replica::Told`, as plain objects (`shell.rs`).

use serde::Deserialize;

/// How the page starts its client: who it is (`nodeId`, `publicKey`), the build of its contracts (none for a
/// development build) and the random `salt` of its request ids. Where the server is, each dial asks (`Tabs::browser`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Start {
    pub node_id: String,
    pub public_key: String,
    #[serde(default)]
    pub bundle: Option<String>,
    pub salt: String,
}

impl Start {
    pub(crate) fn options(&self) -> replica::Options {
        replica::Options {
            node: link::Node {
                id: self.node_id.clone(),
                public_key: self.public_key.clone(),
            },
            bundle: self.bundle.clone(),
            salt: self.salt.clone(),
            config: link::Config::default(),
            clock: crate::shell::precise_now,
        }
    }
}
