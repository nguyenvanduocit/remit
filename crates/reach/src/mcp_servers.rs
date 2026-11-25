//! Bringing in an outside MCP server (ADR-0077): the owner asks the Assistant ("bring in my MCP servers"), which reads the
//! servers configured for this machine's Claude Code with list_importable_mcp_servers (names and variable names,
//! never a value) and raises one change item per server with propose_mcp_server. The record is an extension's
//! (`source::Extension::mcp`), applied as one (extensions.rs).
//!
//! On the owner's Apply the server copies each value the machine's configuration holds into the connection's variables,
//! sealing the secret ones, and asks for the rest in credential items: no value passes through an agent. Remit then
//! connects and pins what the server lists (`McpPins`): the first listing is stored as the source's tools, recorded as a
//! change by Remit; a later listing that differs is held and raised as a change item the Assistant may apply.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use model::{ChangeKind, InboxItem, InboxKind, InboxOption, InboxStatus, PendingChange, StoredExtension};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use source::{DEFAULT_CONNECTION, Extension, ExtensionMcp, ExtensionTool, McpTransport, Var, sensitive_key};
use store::SourceValue;

use crate::Reach;
use crate::credentials::{SourceCredentialsInput, secret_aad};
use crate::extensions::{ExtensionProposal, extension_rail, stored_extension};
use crate::sources::Bound;
use chains::chain_core::gist;
use chains::chain_core::new_id;
use chains::change_item::{CHANGE_APPLY, CHANGE_NOT_NOW, change_detail, code_span};
use chains::changes::{Authorship, doc_of, record_change};
use chains::tools::{Effect, Tools};
use chains::{Error, Result};

/// Where a proposal's values are found again at Apply: the server's name in the configuration. It holds no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct McpImport {
    pub server: String,
}

/// A server as a configuration holds it, values included: it never leaves this file but as names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Configured {
    pub name: String,
    pub reach: ServerReach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ServerReach {
    Stdio {
        command: String,
        args: Vec<String>,
        /// Name -> value as written; a value that names another variable (`${X}`, `$X`) is not one.
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        /// Header -> value as written.
        headers: BTreeMap<String, String>,
    },
}

/// What list_importable_mcp_servers answers of a server: names, never a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Importable {
    pub name: String,
    pub transport: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// The environment variables a stdio server is given.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<String>,
    /// The headers an HTTP server is sent.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,
}

/// The files the servers are configured in, from the Claude Code configuration directory Remit was started with: its
/// `.claude.json` and the one beside it in the home directory.
pub(crate) fn config_files(claude_config_dir: &Path) -> [PathBuf; 2] {
    let home = claude_config_dir.parent().unwrap_or(claude_config_dir);
    [claude_config_dir.join(".claude.json"), home.join(".claude.json")]
}

/// The servers of a Claude Code `.claude.json`: the user's, then each project's, the first of a name kept. Pure.
pub(crate) fn claude_servers(text: &str) -> Vec<Configured> {
    let Ok(doc) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut maps: Vec<&serde_json::Map<String, Value>> = Vec::new();
    maps.extend(doc.get("mcpServers").and_then(Value::as_object));
    if let Some(projects) = doc.get("projects").and_then(Value::as_object) {
        maps.extend(
            projects
                .values()
                .filter_map(|p| p.get("mcpServers"))
                .filter_map(Value::as_object),
        );
    }
    let mut out: Vec<Configured> = Vec::new();
    for (name, s) in maps.into_iter().flatten() {
        if out.iter().any(|c| c.name == *name) {
            continue;
        }
        let text = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let strings = |k: &str| -> BTreeMap<String, String> {
            s.get(k)
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default()
        };
        let reach = if !text("url").is_empty() {
            ServerReach::Http {
                url: text("url"),
                headers: strings("headers"),
            }
        } else if !text("command").is_empty() {
            ServerReach::Stdio {
                command: text("command"),
                args: s
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                    .unwrap_or_default(),
                env: strings("env"),
            }
        } else {
            continue;
        };
        out.push(Configured {
            name: name.clone(),
            reach,
        });
    }
    out
}

/// What the tool answers of a configured server: the URL without its login or query, and each argument whose flag names a
/// credential without its value. Pure.
pub(crate) fn importable(c: &Configured) -> Importable {
    let mut out = Importable {
        name: c.name.clone(),
        transport: "stdio",
        command: String::new(),
        args: Vec::new(),
        url: String::new(),
        variables: Vec::new(),
        headers: Vec::new(),
    };
    match &c.reach {
        ServerReach::Stdio { command, args, env } => {
            out.command.clone_from(command);
            out.args = shown_args(args);
            out.variables = env.keys().cloned().collect();
        }
        ServerReach::Http { url, headers } => {
            out.transport = "http";
            out.url = bare_url(url);
            out.headers = headers.keys().cloned().collect();
        }
    }
    out
}

/// A URL as it is shown: its scheme, host, port and path.
fn bare_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            u.set_query(None);
            u.set_fragment(None);
            u.set_username("").ok();
            u.set_password(None).ok();
            u.to_string()
        }
        Err(_) => "[not a URL]".to_owned(),
    }
}

/// The index of each argument that is the value of a flag naming a credential: `--token x`, `--token=x`.
fn credential_args(args: &[String]) -> Vec<usize> {
    let flag = |a: &str| a.starts_with('-') && sensitive_key(a.trim_start_matches('-'));
    let mut out = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if let Some((name, _)) = a.split_once('=')
            && flag(name)
        {
            out.push(i);
        } else if i
            .checked_sub(1)
            .and_then(|j| args.get(j))
            .is_some_and(|p| !p.contains('=') && flag(p))
        {
            out.push(i);
        }
    }
    out
}

fn shown_args(args: &[String]) -> Vec<String> {
    let hidden = credential_args(args);
    args.iter()
        .enumerate()
        .map(|(i, a)| {
            if !hidden.contains(&i) {
                return a.clone();
            }
            match a.split_once('=') {
                Some((name, _)) => format!("{name}=[removed]"),
                None => "[removed]".to_owned(),
            }
        })
        .collect()
}

/// A value a configuration holds as it is, not as the name of another variable.
fn literal(v: &str) -> Option<&str> {
    (!v.is_empty() && !v.contains('$')).then_some(v)
}

/// The variable an HTTP header's value is kept in: `<ID>_<HEADER>`.
fn header_var(id: &str, header: &str) -> String {
    let h: String = header
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("{}_{h}", id.to_uppercase())
}

/// The manifest of a configured server under id: its variables (secret but the ones named plain) and its record. Pure.
pub(crate) fn manifest_of(c: &Configured, input: &ProposeMcpServerInput) -> Result<Extension> {
    let what = |name: &str| format!("{name}, from the Claude Code configuration of {}", c.name);
    let var = |name: String, address: bool| Var {
        what: what(&name),
        secret: !address && !input.plain.contains(&name),
        optional: false,
        address,
        name,
    };
    let (vars, transport) = match &c.reach {
        ServerReach::Stdio { command, args, env } => {
            if !credential_args(args).is_empty() {
                return Err(Error::refused(format!(
                    "{} passes a credential on its command line ({}): a value in the arguments would be stored and shown; move it into an environment variable of the server's configuration first",
                    c.name,
                    shown_args(args).join(" ")
                )));
            }
            (
                env.keys().map(|n| var(n.clone(), false)).collect(),
                McpTransport::Stdio {
                    command: command.clone(),
                    args: args.clone(),
                },
            )
        }
        ServerReach::Http { url, headers } => {
            let url_var = format!("{}_URL", input.id.to_uppercase());
            let mut vars = vec![Var {
                what: format!("the address of the MCP server ({})", what(&url_var)),
                ..var(url_var.clone(), true)
            }];
            if url::Url::parse(url).is_ok_and(|u| !u.username().is_empty() || u.query().is_some()) {
                return Err(Error::refused(format!(
                    "{}'s URL holds a login or a query, which would be stored and shown as an address: move its credential into a header of the server's configuration first",
                    c.name
                )));
            }
            let mut sent = BTreeMap::new();
            for h in headers.keys() {
                let name = header_var(&input.id, h);
                if !vars.iter().any(|v| v.name == name) {
                    vars.push(Var {
                        what: format!("the whole value of the {h} header, as sent ({})", what(&name)),
                        ..var(name.clone(), false)
                    });
                }
                sent.insert(h.clone(), name);
            }
            (vars, McpTransport::Http { url_var, headers: sent })
        }
    };
    let ext = Extension {
        id: input.id.clone(),
        label: if input.label.trim().is_empty() {
            c.name.clone()
        } else {
            input.label.trim().to_owned()
        },
        capability: input.capability.clone(),
        vars,
        mcp: Some(ExtensionMcp {
            transport,
            effects: input.effects.clone(),
        }),
        ..Extension::default()
    };
    ext.validate().map_err(|e| Error::refused(e.to_string()))?;
    Ok(ext)
}

/// The values a configured server holds for the variables of ext, by variable: the ones written as they are. Pure.
pub(crate) fn found_values(c: &Configured, ext: &Extension) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    match (&c.reach, ext.mcp.as_ref().map(|m| &m.transport)) {
        (ServerReach::Stdio { env, .. }, Some(McpTransport::Stdio { .. })) => {
            for v in &ext.vars {
                if let Some(val) = env.get(&v.name).map(String::as_str).and_then(literal) {
                    out.insert(v.name.clone(), val.to_owned());
                }
            }
        }
        (ServerReach::Http { url, headers: given }, Some(McpTransport::Http { url_var, headers })) => {
            if let Some(u) = literal(url) {
                out.insert(url_var.clone(), u.to_owned());
            }
            for (h, var) in headers {
                if let Some(val) = given.get(h).map(String::as_str).and_then(literal) {
                    out.insert(var.clone(), val.to_owned());
                }
            }
        }
        _ => {}
    }
    out
}

/// What a Chair or the Assistant writes to propose a configured server.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProposeMcpServerInput {
    /// the server's name, as list_importable_mcp_servers answers it
    pub server: String,
    /// the source's id: a lower case word of 2 to 24 letters and digits, which its connections and tools are named by
    pub id: String,
    /// the name the owner reads; empty: the server's name
    #[serde(default)]
    pub label: String,
    /// which capability gives a chain its tools: data, observe or customers
    pub capability: String,
    /// the variables that are not secret (a user, a region): every other is sealed and never shown; the URL of an HTTP
    /// server is an address, shown
    #[serde(default)]
    pub plain: Vec<String>,
    /// a tool of the server, by the name the server calls it, ranked below what its hint gives (read, reversible,
    /// compensable): without one, a tool whose readOnlyHint is true is read and every other irreversible
    #[serde(default)]
    pub effects: BTreeMap<String, String>,
}

/// list_importable_mcp_servers takes nothing.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoInput {}

/// The pins port: what a server lists, from the source crate, handed to the orchestrator off the caller's path.
pub(crate) struct McpPins(pub(crate) Bound);

impl source::Pins for McpPins {
    fn listed(&self, source: &str, tools: Vec<ExtensionTool>) {
        let Ok(o) = self.0.get() else { return };
        let source = source.to_owned();
        tokio::spawn(async move { o.mcp_listed(&source, tools).await });
    }
}

impl Reach {
    /// Every server configured for this machine's Claude Code, as values the configuration holds.
    fn configured_servers(&self) -> Vec<Configured> {
        let mut out: Vec<Configured> = Vec::new();
        for f in config_files(&self.cfg.claude_config_dir) {
            for c in std::fs::read_to_string(&f)
                .map(|t| claude_servers(&t))
                .unwrap_or_default()
            {
                if !out.iter().any(|o| o.name == c.name) {
                    out.push(c);
                }
            }
        }
        out
    }

    /// The servers an owner can bring in, as names.
    pub(crate) fn importable_mcp_servers(&self) -> Vec<Importable> {
        self.configured_servers().iter().map(importable).collect()
    }

    /// The configured server a proposal or an Apply names.
    fn configured(&self, server: &str) -> Result<Configured> {
        let all = self.configured_servers();
        all.iter().find(|c| c.name == server).cloned().ok_or_else(|| {
            Error::refused(format!(
                "no MCP server {:?} is configured for this machine's Claude Code: the ones that are {}",
                gist(server, 60),
                all.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
            ))
        })
    }

    /// Raises the change item that asks the owner to bring in a configured server, as me.
    pub(crate) async fn propose_mcp_server(&self, me: &model::Card, input: ProposeMcpServerInput) -> Result<String> {
        if self.bundled.lookup(&input.id).is_some() {
            return Err(Error::refused(format!(
                "id {:?} is a plugin that ships with Remit",
                input.id
            )));
        }
        let c = self.configured(&input.server)?;
        let mut next = manifest_of(&c, &input)?;
        let all = self.store.read(|r| r.extensions()).await?;
        let cur = stored_extension(&all, &next.id)?;
        if let Some(cur) = cur.as_ref().filter(|e| e.mcp.is_some()) {
            next.tools.clone_from(&cur.tools); // the pins stay until a listing says otherwise
        }
        let fields = extension_rail(cur.as_ref(), &next, 0);
        if fields.is_empty() {
            return Err(Error::refused(format!(
                "{} is applied as it is: nothing to change",
                next.id
            )));
        }
        let detail = format!(
            "{}\n{}",
            mcp_head(&me.name, &next, &c),
            change_detail(&me.name, &ChangeKind::Extension, &next.id, &fields)
        );
        let proposal = ExtensionProposal {
            manifest: next.clone(),
            import: Some(McpImport { server: c.name.clone() }),
            ..ExtensionProposal::default()
        };
        let item = change_item(
            self,
            me.home.clone(),
            me.id.clone(),
            format!("{} asks to bring in MCP server {}", me.name, next.id),
            detail,
            PendingChange {
                kind: ChangeKind::Extension,
                name: next.id.clone(),
                record: serde_json::to_string(&proposal)?,
                fields,
            },
        );
        self.raise_change(item).await?;
        Ok(format!(
            "MCP server {} waits in the owner's inbox. On Apply the values its Claude Code configuration holds are copied into its connection by the server and the rest asked of the owner; Remit then connects and pins the tools it lists, each served as {}_<tool> with the grant its effect needs. You are told the answer; until then nothing of it exists.",
            next.id, next.id
        ))
    }

    /// Copies the values the configuration holds into the default connection of the server applied as id, sealing the secret
    /// ones, and asks the owner for what is still missing.
    pub(crate) async fn import_mcp_values(&self, id: &str, imp: &McpImport) {
        if let Err(err) = self.try_import_mcp_values(id, imp).await {
            tracing::warn!(id, %err, "the values of an MCP server were not imported");
        }
    }

    async fn try_import_mcp_values(&self, id: &str, imp: &McpImport) -> Result<()> {
        let all = self.store.read(|r| r.extensions()).await?;
        let Some(ext) = stored_extension(&all, id)? else {
            return Ok(());
        };
        // Not found any more is no failure: the credential items ask for every value.
        let found = self
            .configured(&imp.server)
            .map(|c| found_values(&c, &ext))
            .unwrap_or_default();
        let key = if ext.vars.iter().any(|v| v.secret && found.contains_key(&v.name)) {
            Some(self.sealing_key().await?)
        } else {
            None
        };
        let mut stored: Vec<(String, SourceValue)> = Vec::new();
        for v in &ext.vars {
            let Some(val) = found.get(&v.name) else { continue };
            let value = match (&key, v.secret) {
                (Some(k), true) => {
                    let aad = secret_aad(id, DEFAULT_CONNECTION, &v.name);
                    SourceValue::Secret(
                        secrets::seal(k, &aad, val.as_bytes())
                            .map_err(|e| Error::refused(format!("seal {aad}: {e}")))?,
                    )
                }
                _ => SourceValue::Plain(val.clone()),
            };
            stored.push((v.name.clone(), value));
        }
        let src = id.to_owned();
        self.store
            .update(move |tx| -> Result<()> {
                for (name, value) in &stored {
                    tx.put_source_value(&src, DEFAULT_CONNECTION, name, value)?;
                }
                Ok(())
            })
            .await?;
        self.source_changed(id, DEFAULT_CONNECTION).await;
        self.request_source_credentials(SourceCredentialsInput {
            source: id.to_owned(),
            ..SourceCredentialsInput::default()
        })
        .await?;
        Ok(())
    }

    /// Connects every configured connection of the MCP server id that has no pins yet, so its first listing is pinned.
    pub(crate) async fn pin_mcp(&self, id: &str) {
        let Ok(all) = self.store.read(|r| r.extensions()).await else {
            return;
        };
        let Ok(Some(ext)) = stored_extension(&all, id) else {
            return;
        };
        if ext.mcp.is_none() || !ext.tools.is_empty() {
            return;
        }
        let Ok(entries) = self.sources.catalog().await else {
            return;
        };
        let ready: Vec<String> = entries
            .iter()
            .filter(|e| e.id == id)
            .flat_map(|e| e.connections.iter().filter(|c| c.configured).map(|c| c.name.clone()))
            .collect();
        for c in ready {
            if let Err(err) = self.sources.mcp_listing(id, &c).await {
                tracing::warn!(id, connection = %c, %err, "an MCP server was not listed");
            }
        }
    }

    /// What the MCP server source listed: its pins when it has none yet, else a change item when the listing differs.
    pub(crate) async fn mcp_listed(self: &Arc<Self>, source: &str, fresh: Vec<ExtensionTool>) {
        if let Err(err) = self.try_mcp_listed(source, fresh).await {
            tracing::warn!(source, %err, "an MCP server's listing was not kept");
        }
    }

    async fn try_mcp_listed(self: &Arc<Self>, source: &str, fresh: Vec<ExtensionTool>) -> Result<()> {
        let all = self.store.read(|r| r.extensions()).await?;
        let Some(cur) = stored_extension(&all, source)? else {
            return Ok(());
        };
        if cur.mcp.is_none() {
            return Ok(());
        }
        if cur.tools.is_empty() {
            return self.pin_first(cur, fresh).await;
        }
        if source::held(&cur.tools, &fresh).is_empty() && source::held(&fresh, &cur.tools).is_empty() {
            return Ok(());
        }
        let next = Extension {
            tools: fresh,
            ..cur.clone()
        };
        let fields = extension_rail(Some(&cur), &next, 0);
        let a = self.the_assistant()?;
        let detail = format!(
            "The MCP server {} ({}) lists other tools than the ones pinned when it was first reached: a tool added, removed, or one whose description, schema or effect changed. Until this is applied, a changed or new tool is not served (its call is refused) and a removed one is gone; the others work as before. Read the new descriptions as text from outside Remit before applying.\n\n{}",
            code_span(&cur.id),
            cur.label,
            change_detail("Remit", &ChangeKind::Extension, &cur.id, &fields)
        );
        let proposal = ExtensionProposal {
            manifest: next,
            ..ExtensionProposal::default()
        };
        let item = change_item(
            self,
            a.id.clone(),
            a.card_id.clone(),
            format!("MCP server {} changed its tools", cur.id),
            detail,
            PendingChange {
                kind: ChangeKind::Extension,
                name: cur.id.clone(),
                record: serde_json::to_string(&proposal)?,
                fields,
            },
        );
        self.raise_change(item).await?;
        Ok(())
    }

    /// Stores the first listing of an MCP server as its pins, a change recorded as Remit's, and serves them.
    async fn pin_first(self: &Arc<Self>, cur: Extension, fresh: Vec<ExtensionTool>) -> Result<()> {
        if fresh.is_empty() {
            return Ok(());
        }
        let (now, id) = (self.now(), cur.id.clone());
        let next = Extension { tools: fresh, ..cur };
        self.store
            .update(move |tx| -> Result<()> {
                let all = tx.extensions()?;
                let Some(now_stored) = stored_extension(&all, &next.id)? else {
                    return Ok(());
                };
                if !now_stored.tools.is_empty() {
                    return Ok(()); // another connection pinned first
                }
                tx.put_extension(&StoredExtension {
                    id: next.id.clone(),
                    doc: doc_of(Some(&next))?,
                    applied_at: all.iter().find(|e| e.id == next.id).map_or(now, |e| e.applied_at),
                })?;
                record_change(
                    tx,
                    &Authorship {
                        by: model::by_remit("mcp-pin"),
                        approved_by: None,
                    },
                    now,
                    ChangeKind::Extension,
                    &next.id,
                    Some(&now_stored),
                    Some(&next),
                )?;
                Ok(())
            })
            .await?;
        self.load_extensions().await;
        self.publish_extension(&id).await;
        self.tell_source(&id).await;
        Ok(())
    }
}

/// What the owner reads first on a proposal: what Remit runs or reaches, and what it gives the server.
fn mcp_head(by: &str, ext: &Extension, c: &Configured) -> String {
    let vars = |secret: bool| -> String {
        let names: Vec<&str> = ext
            .vars
            .iter()
            .filter(|v| v.secret == secret)
            .map(|v| v.name.as_str())
            .collect();
        if names.is_empty() {
            "none".to_owned()
        } else {
            names.join(", ")
        }
    };
    let reach = match &c.reach {
        ServerReach::Stdio { command, args, .. } => format!(
            "Remit runs {} in a jail, with the variables below in its environment and nothing else of this server's, reaching only the addresses among them",
            code_span(format!("{command} {}", args.join(" ")).trim())
        ),
        ServerReach::Http { url, .. } => format!(
            "Remit reaches {} over HTTP, sending the headers below from this server alone",
            code_span(&bare_url(url))
        ),
    };
    format!(
        "{by} asks to bring in the MCP server {} ({}) from this machine's Claude Code configuration. {reach}. Secret variables (sealed, never shown): {}; plain: {}. Values the configuration holds are copied by the server on Apply; you are asked for the rest. Its tools are pinned when Remit first reaches it: a tool whose readOnlyHint is true needs a read grant and every other an irreversible one, unless ranked lower here; a tool the server later changes is held until a change item is applied.\n",
        code_span(&ext.id),
        ext.label,
        vars(true),
        vars(false)
    )
}

/// A change item with Apply and Not now, raised on card_id's behalf in home.
fn change_item(
    o: &Reach,
    home: String,
    card_id: String,
    title: String,
    detail: String,
    change: PendingChange,
) -> InboxItem {
    InboxItem {
        id: new_id("i_"),
        home,
        card_id,
        kind: InboxKind::Change,
        title,
        detail,
        options: vec![
            InboxOption {
                id: CHANGE_APPLY.to_owned(),
                name: "Apply".to_owned(),
                ..InboxOption::default()
            },
            InboxOption {
                id: CHANGE_NOT_NOW.to_owned(),
                name: "Not now".to_owned(),
                ..InboxOption::default()
            },
        ],
        status: InboxStatus::Open,
        created_at: o.now(),
        change: Some(change),
        ..InboxItem::default()
    }
}

impl Reach {
    /// Gives list_importable_mcp_servers and propose_mcp_server, for card card_id: the Assistant, or a Chair whose chain
    /// holds extensions.
    pub fn add_mcp_server_tools(self: &Arc<Self>, t: &mut Tools, card_id: &str) {
        t.serving(self).add(
            "list_importable_mcp_servers", Effect::Reads,
            "The MCP servers configured for this machine's Claude Code, which the owner can bring into Remit with propose_mcp_server: each with its name, its transport (stdio or http), its command and arguments or its URL, and the names of its environment variables or headers. No value is ever answered.",
            |o, _: NoInput| async move { chains::tools::as_json(&o.importable_mcp_servers()) },
        );
        let card_id = card_id.to_owned();
        t.serving(self).add(
            "propose_mcp_server", Effect::Writes,
            "Ask the owner to bring in one MCP server list_importable_mcp_servers names, as a source: one change item per server shows what it runs or reaches and its variables. On Apply the server copies the values the configuration holds into the source's connection (you never see them) and asks the owner for the rest; Remit then connects, pins the tools it lists and serves each as <id>_<tool> through the grants every source call passes. Ask for grants on it afterwards as for any source.",
            move |o, input: ProposeMcpServerInput| {
                let card_id = card_id.clone();
                async move {
                    let me = o.store.read(move |r| r.card(&card_id)).await?;
                    o.propose_mcp_server(&me, input).await
                }
            },
        );
    }
}
