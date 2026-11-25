//! The tools that show and give a source its connections (ADR-0017). list_sources is read only and an agent's: it names
//! the sources, their connections and what each lacks, so an agent chooses the connection a source's tool reads. The owner
//! gives credentials through request_source_credentials, which only the Assistant and a Chair that holds the sources
//! capability have: it takes names and never a value, and the value is typed into the write-only field of the inbox item
//! it raises.

use std::sync::Arc;
use wire::app::{SourceTool, ToolSource};

use std::collections::BTreeMap;

use source::Caller;

use crate::Reach;
use crate::credentials::SourceCredentialsInput;
use chains::tools::Tools;
use chains::tools::{Effect, NoInput};
use chains::{Error, Result};

impl Reach {
    /// Gives t list_sources. For an agent of a chain, who, it also says what the chain may do on each connection; the
    /// Assistant, which is in no chain (who names none), sees the connections alone.
    pub fn add_source_list_tool(self: &Arc<Self>, t: &mut Tools, who: Caller) {
        t.serving(self).add(
            "list_sources", Effect::Reads,
            "The sources Remit reads the outside world through (MongoDB, Bytebase, StarRocks, Intercom, Amplitude, Google Analytics, Grafana, Coroot, CloudWatch, Datadog, GitHub), each with its tools and its connections: a connection is the same source with another set of credentials, so one source can reach a staging and a production system. For each connection: its name, what it reaches, whether it is configured and what it still lacks, whether the owner marked it protected (a chain reads it only through a grant he gave) and what your chain may do on it: read, implied by a capability it holds or granted by the owner, write when granted, or none. A tool of a source takes the connection to read as `connection`. Read only; no value of a secret is ever shown.",
            move |o, _i: NoInput| {
                let who = who.clone();
                async move {
                    let views = o.sources.catalog().await.map_err(|e| Error::refused(e.to_string()))?;
                    let notes = o.connection_notes(&views, &who).await?;
                    Ok(sources_text(&views, &notes))
                }
            },
        );
    }

    /// Gives t request_source_credentials.
    pub fn add_source_request_tool(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "request_source_credentials", Effect::Writes,
            "Ask the owner for the credentials of a source's connection, or make a new connection: a source with another set of credentials reaches another place (a staging and a production MongoDB). It raises one inbox item for each variable the connection lacks (or for the variables you name, which also replaces one that is set), plain ones first; the owner types each value into the item's write-only field, where it is stored encrypted and no agent sees it. You give names only, never a value. A new named connection needs a one-line description of what it reaches and holds.",
            |o, i: SourceCredentialsInput| async move { asked_words(&o.request_source_credentials(i).await?) },
        );
    }
}

/// What request_source_credentials says it asked.
fn asked_words(items: &[model::InboxItem]) -> Result<String> {
    if items.is_empty() {
        return Ok(
            "Nothing to ask: every variable asked for already waits for the owner in his inbox, or the connection lacks none."
                .to_owned(),
        );
    }
    let mut b = format!(
        "Asked the owner in his inbox for {} value(s); the source can be read again once he answers (list_sources shows it configured):",
        items.len()
    );
    for it in items {
        b.push_str(&format!("\n- {}", it.title));
    }
    Ok(b)
}

/// The catalog as an agent reads it, with the note kept for a connection ("<source>/<name>") after its state.
pub(crate) fn sources_text(views: &[ToolSource], notes: &BTreeMap<String, String>) -> String {
    let mut b = String::new();
    for (i, v) in views.iter().enumerate() {
        if i > 0 {
            b.push('\n');
        }
        b.push_str(&format!(
            "{} ({}, capability {})\n  tools: {}\n",
            v.label,
            v.id,
            v.capability,
            tool_labels(&v.tools)
        ));
        for c in &v.connections {
            let mut state = if !c.problem.is_empty() {
                format!("cannot be read: {}", c.problem)
            } else if !c.configured {
                format!("lacks {}", c.missing.join(", "))
            } else {
                "configured".to_owned()
            };
            let mut line = format!("  connection {}", c.name);
            if !c.description.is_empty() {
                line.push_str(&format!(" ({})", c.description));
            }
            if let Some(note) = notes.get(&format!("{}/{}", v.id, c.name)).filter(|n| !n.is_empty()) {
                state.push_str("; ");
                state.push_str(note);
            }
            b.push_str(&format!("{line}: {state}\n"));
        }
    }
    b
}

fn tool_labels(tools: &[SourceTool]) -> String {
    tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ")
}
