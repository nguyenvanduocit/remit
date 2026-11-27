//! An outside MCP server as an extension (ADR-0077): Remit is its client, and each of its tools is served as
//! `<extension>_<tool>` through the gate like any tool of a source.
//!
//! The record is an extension's (`Extension::mcp`): the transport, and for stdio the command line, run as a program (form
//! T2: the connection's variables in its environment, its proxy reaching its address variables only, nothing of the
//! server's environment), or for HTTP the variable that holds the URL and the headers that carry the credential, sent by the
//! backend alone (T0). One session per connection, opened at its first use and again after it closed.
//!
//! What the server lists is pinned: the first listing of a source is its pins (`Pins::listed`, stored by the host into the
//! manifest's tools), and a tool is served only while the server still lists it as it was pinned. A tool whose description,
//! schema or effect changed is held (its call refused) and the host raises a change item with the new listing. Effects are
//! the platform's: `readOnlyHint` can only lower a tool to read, every other is irreversible, and the record's `effects`
//! (written in a change item the owner applied) rank a tool by its server name.
//!
//! The pure rules are `pins_of` and `held`; the rest is the shell that speaks MCP through rmcp's client.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, LazyLock, Mutex};

use grant::Effect;
use regex::Regex;
use rmcp::ServiceExt as _;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::Http;
use crate::connection::Set;
use crate::env::Env;
use crate::error::{BoxFuture, Error, Result, err, fail};
use crate::extension::{Extension, ExtensionTool, schema_tool};
use crate::program::{Job, PROGRAM_PATH, Runner};
use crate::redact::scrub;
use crate::script::auth_secrets;
use crate::source::{Provider, Tool, Var};
use crate::text::{OUTPUT_MAX, limit};

/// How Remit reaches an MCP server, and how the platform ranks its tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionMcp {
    #[serde(flatten)]
    pub transport: McpTransport,
    /// The server's tool name -> the effect the platform ranks it at, below what its hint gives (ADR-0077, "Effect").
    #[serde(
        default,
        deserialize_with = "crate::null_json::null_as_zero",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub effects: BTreeMap<String, String>,
}

/// The two transports of slice 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "lowercase")]
pub enum McpTransport {
    /// A command Remit runs, speaking MCP on its stdio.
    Stdio {
        command: String,
        #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
        args: Vec<String>,
    },
    /// A streamable HTTP endpoint the backend reaches.
    Http {
        /// The address variable that holds the URL.
        #[serde(rename = "urlVar")]
        url_var: String,
        /// Header name -> the variable whose value is sent in it, as it is.
        #[serde(
            default,
            deserialize_with = "crate::null_json::null_as_zero",
            skip_serializing_if = "BTreeMap::is_empty"
        )]
        headers: BTreeMap<String, String>,
    },
}

static HEADER: LazyLock<Regex> = LazyLock::new(|| crate::extension::regex(r"^[A-Za-z][A-Za-z0-9-]{0,63}$"));

/// Arguments of a stdio command, and bytes in one.
const MAX_ARGS: usize = 64;
const MAX_ARG_BYTES: usize = 4096;
/// Tools a server may list that are pinned; the rest are not served.
const MAX_TOOLS: usize = 128;
/// A pinned description, in bytes: what an owner reads in a change item.
const MAX_DESCRIPTION: usize = 4096;

impl ExtensionMcp {
    /// Whether the record is one Remit can run: a command it can name, or a URL variable and header variables the
    /// extension declares, and effects on the ladder. Pure.
    pub(crate) fn validate(&self, vars: &[Var]) -> Result<()> {
        let declared = |name: &str| vars.iter().any(|v| v.name == name);
        match &self.transport {
            McpTransport::Stdio { command, args } => {
                if command.is_empty()
                    || command.contains(['\0', ' ', '\t', '\n'])
                    || (command.contains('/') && !command.starts_with('/'))
                {
                    return fail!(
                        "mcp.command {:?} is not a program: a bare name looked up along the program's PATH, or an absolute path",
                        limit(command, 80)
                    );
                }
                if args.len() > MAX_ARGS || args.iter().any(|a| a.len() > MAX_ARG_BYTES || a.contains('\0')) {
                    return fail!(
                        "mcp.args: at most {MAX_ARGS} arguments of at most {MAX_ARG_BYTES} bytes, none with a NUL"
                    );
                }
            }
            McpTransport::Http { url_var, headers } => {
                if !vars.iter().any(|v| v.name == *url_var && v.address) {
                    return fail!(
                        "mcp.urlVar {:?} is not an address variable the extension declares",
                        limit(url_var, 40)
                    );
                }
                for (h, var) in headers {
                    if !HEADER.is_match(h)
                        || ["host", "accept", "content-length", "content-type"].contains(&h.to_lowercase().as_str())
                    {
                        return fail!("mcp.headers {:?} is not a header a credential goes in", limit(h, 40));
                    }
                    if !declared(var) {
                        return fail!(
                            "mcp.headers {h} reads {:?}, which the extension does not declare",
                            limit(var, 40)
                        );
                    }
                }
            }
        }
        for (tool, effect) in &self.effects {
            effect
                .parse::<Effect>()
                .map_err(|e| err!("mcp.effects {}: {e}", limit(tool, 60)))?;
        }
        Ok(())
    }
}

/// One tool as a server lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    /// The author's `readOnlyHint`.
    pub read_only: Option<bool>,
}

/// The pins of a listing: each tool under the name Remit serves it by, its description, its schema and the effect the
/// platform ranks it at. A tool whose name has no Remit form, or one another tool took, is not pinned. Pure.
#[must_use]
pub fn pins_of(listed: &[Listed], effects: &BTreeMap<String, String>) -> Vec<ExtensionTool> {
    let mut out: Vec<ExtensionTool> = Vec::new();
    for l in listed.iter().take(MAX_TOOLS) {
        let Some(name) = remit_name(&l.name) else { continue };
        if out.iter().any(|t| t.name == name) {
            continue;
        }
        let hinted = if l.read_only == Some(true) {
            Effect::Read
        } else {
            Effect::Irreversible
        };
        let ranked = effects.get(&l.name).and_then(|e| e.parse::<Effect>().ok());
        let effect = ranked.filter(|r| *r < hinted).unwrap_or(hinted);
        out.push(ExtensionTool {
            name,
            description: crate::text::shorten(&l.description, MAX_DESCRIPTION),
            input_schema: l.input_schema.to_string(),
            effect,
            mcp_name: l.name.clone(),
        });
    }
    out
}

/// The server names of the pinned tools a fresh listing does not serve as they were pinned: changed in description, schema
/// or effect, or gone. Pure.
#[must_use]
pub fn held(pinned: &[ExtensionTool], fresh: &[ExtensionTool]) -> Vec<String> {
    pinned
        .iter()
        .filter(|p| !fresh.iter().any(|f| same_pin(p, f)))
        .map(|p| p.mcp_name.clone())
        .collect()
}

/// Whether two pins say the same: the schemas compared as JSON, so key order does not count.
fn same_pin(a: &ExtensionTool, b: &ExtensionTool) -> bool {
    let schema = |t: &ExtensionTool| serde_json::from_str::<Value>(&t.input_schema).ok();
    a.mcp_name == b.mcp_name
        && a.name == b.name
        && a.description == b.description
        && a.effect == b.effect
        && schema(a) == schema(b)
}

/// The name Remit serves a server's tool by: lower case, a word of letters, digits and underscores, an upper case letter
/// in a camel-case name starting a word. None when nothing of the name is left.
fn remit_name(server: &str) -> Option<String> {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in server.chars() {
        if c.is_ascii_uppercase() && prev_lower {
            out.push('_');
        }
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_');
    let out = if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("t_{out}")
    } else {
        out.to_owned()
    };
    let out: String = out.chars().take(48).collect();
    let out = out.trim_end_matches('_').to_owned();
    (out.len() >= 2).then_some(out)
}

/// Where the host keeps what a server lists: it stores the first listing of a source as its pins, and raises a change item
/// when a later one differs from them.
pub trait Pins: Send + Sync {
    fn listed(&self, source: &str, tools: Vec<ExtensionTool>);
}

impl Set {
    /// What the MCP server behind a connection lists now, as pins: it opens the connection's session when none is open,
    /// which tells the host's `Pins` what it found. An error for a source that is no MCP server, or a connection that
    /// lacks a variable.
    pub async fn mcp_listing(&self, source: &str, connection: &str) -> Result<Vec<ExtensionTool>> {
        let Some(k) = self.kind(source) else {
            return Err(crate::connection::no_source(source));
        };
        let p = self.provider(&k, connection).await?;
        let missing = p.missing();
        if !missing.is_empty() {
            return fail!("{source}/{connection} lacks {}", missing.join(", "));
        }
        match Arc::clone(&p).listing() {
            Some(listing) => listing.await,
            None => fail!("{source} is not an MCP server"),
        }
    }
}

/// One connection of an MCP server: its session, opened on first use.
pub(crate) struct McpSource {
    pub ext: Extension,
    pub mcp: ExtensionMcp,
    pub env: Env,
    pub http: Http,
    pub runner: Option<Arc<dyn Runner>>,
    pub pins: Option<Arc<dyn Pins>>,
    pub live: tokio::sync::Mutex<Option<Arc<Live>>>,
}

/// An open session and what the server listed when it opened.
pub(crate) struct Live {
    client: RunningService<RoleClient, ()>,
    fresh: Vec<ExtensionTool>,
    /// The running program and its scratch directory, for stdio: dropped, the program ends.
    _hold: Mutex<Option<(Box<dyn Send>, tempfile::TempDir)>>,
}

impl McpSource {
    /// The variables the connection has set, by name.
    fn values(&self) -> BTreeMap<String, String> {
        self.ext
            .vars
            .iter()
            .filter_map(|v| {
                let val = self.env.get(&v.name);
                (!val.is_empty()).then(|| (v.name.clone(), val))
            })
            .collect()
    }

    /// The values of the secret variables, and of a header value its credential after the scheme (`Bearer <token>`): removed
    /// from every answer.
    fn secrets(&self) -> Vec<String> {
        self.ext
            .vars
            .iter()
            .filter(|v| v.secret)
            .map(|v| self.env.get(&v.name))
            .filter(|v| !v.is_empty())
            .flat_map(|v| auth_secrets(&v, &[]))
            .collect()
    }

    /// The open session, or a new one; a session that closed is opened again.
    async fn live(&self) -> Result<Arc<Live>> {
        let mut slot = self.live.lock().await;
        if let Some(l) = slot.as_ref().filter(|l| !l.client.is_closed()) {
            return Ok(Arc::clone(l));
        }
        *slot = None;
        let secrets = self.secrets();
        let l = Arc::new(
            self.open()
                .await
                .map_err(|e| Error::msg(scrub(&format!("{}: {e}", self.ext.id), &secrets)))?,
        );
        if let Some(p) = &self.pins {
            p.listed(&self.ext.id, l.fresh.clone());
        }
        *slot = Some(Arc::clone(&l));
        Ok(l)
    }

    /// Opens a session over the record's transport and lists the server's tools.
    async fn open(&self) -> Result<Live> {
        let (client, hold) = match &self.mcp.transport {
            McpTransport::Stdio { command, args } => {
                let Some(runner) = self.runner.clone() else {
                    return fail!("this server runs no program");
                };
                let dir = tempfile::Builder::new()
                    .prefix("remit-mcp-")
                    .tempdir()
                    .map_err(|e| err!("scratch directory: {e}"))?;
                let job = self.job(command, args, &dir);
                let s = runner.spawn(&job).await.map_err(|e| err!("start: {e}"))?;
                let client = ().serve((s.stdout, s.stdin)).await.map_err(|e| err!("initialize: {e}"))?;
                (client, Some((s.hold, dir)))
            }
            McpTransport::Http { url_var, headers } => {
                let mut sent = HashMap::new();
                for (h, var) in headers {
                    let name = http::HeaderName::from_bytes(h.as_bytes()).map_err(|e| err!("header {h}: {e}"))?;
                    let mut value = http::HeaderValue::from_str(&self.env.get(var))
                        .map_err(|_| err!("header {h}: not a header value"))?;
                    value.set_sensitive(true);
                    sent.insert(name, value);
                }
                let cfg = StreamableHttpClientTransportConfig::with_uri(self.env.get(url_var)).custom_headers(sent);
                let transport = StreamableHttpClientTransport::with_client(self.http.client()?, cfg);
                let client = ().serve(transport).await.map_err(|e| err!("initialize: {e}"))?;
                (client, None)
            }
        };
        let tools = client.list_all_tools().await.map_err(|e| err!("tools/list: {e}"))?;
        let listed: Vec<Listed> = tools
            .into_iter()
            .map(|t| Listed {
                name: t.name.into_owned(),
                description: t.description.map(std::borrow::Cow::into_owned).unwrap_or_default(),
                input_schema: Value::Object((*t.input_schema).clone()),
                read_only: t.annotations.and_then(|a| a.read_only_hint),
            })
            .collect();
        Ok(Live {
            client,
            fresh: pins_of(&listed, &self.mcp.effects),
            _hold: Mutex::new(hold),
        })
    }

    /// The run of a stdio server: its command, the connection's variables and nothing of the server's environment,
    /// egress to the connection's addresses.
    fn job(&self, command: &str, args: &[String], dir: &tempfile::TempDir) -> Job {
        let d = dir.path().to_string_lossy().into_owned();
        let mut env = vec![
            format!("PATH={PROGRAM_PATH}"),
            format!("HOME={d}"),
            format!("TMPDIR={d}"),
        ];
        env.extend(self.values().into_iter().map(|(k, v)| format!("{k}={v}")));
        let mut argv = vec![command.to_owned()];
        argv.extend(args.iter().cloned());
        Job {
            argv,
            env,
            dir: dir.path().to_path_buf(),
            egress: self
                .ext
                .vars
                .iter()
                .filter(|v| v.address)
                .filter_map(|v| egress_of(&self.env.get(&v.name)))
                .collect(),
            inject: None,
            // No deadline: the server lives until its session drops it.
            timeout: std::time::Duration::MAX,
            max_output: OUTPUT_MAX,
        }
    }

    /// One call of the server's tool `server_name`, pinned as `pin`: refused when the server no longer lists it as pinned.
    async fn call(&self, pin: &ExtensionTool, arguments: String) -> Result<String> {
        let secrets = self.secrets();
        let live = self.live().await?;
        if held(std::slice::from_ref(pin), &live.fresh).contains(&pin.mcp_name) {
            return fail!(
                "{}_{} is held: the server no longer lists it as the owner's change pinned it (its description, schema or effect changed, or it is gone). A change item shows the difference; it is served again once that is applied.",
                self.ext.id,
                pin.name
            );
        }
        let args = match serde_json::from_str::<Value>(&arguments) {
            Ok(Value::Object(m)) => m,
            _ => return fail!("{}_{}: the arguments are not an object", self.ext.id, pin.name),
        };
        let res = live
            .client
            .call_tool(CallToolRequestParams::new(pin.mcp_name.clone()).with_arguments(args))
            .await;
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                // A session that broke is opened again by the next call; this one is not repeated, as it may have acted.
                let mut slot = self.live.lock().await;
                if slot.as_ref().is_some_and(|l| Arc::ptr_eq(l, &live)) {
                    *slot = None;
                }
                return Err(Error::msg(scrub(
                    &format!("{}_{}: {e}", self.ext.id, pin.name),
                    &secrets,
                )));
            }
        };
        let (text, failed) = answer_text(&res);
        let text = scrub(&text, &secrets);
        if failed {
            return Err(Error::msg(format!(
                "{}_{}: the server answered an error: {text}",
                self.ext.id, pin.name
            )));
        }
        Ok(text)
    }
}

/// What a tool answered, as text: its text blocks, the structured content when there is none, a line for anything else.
fn answer_text(r: &CallToolResult) -> (String, bool) {
    let mut parts: Vec<String> = r
        .content
        .iter()
        .map(|c| match c {
            ContentBlock::Text(t) => t.text.clone(),
            ContentBlock::Image(_) => "[an image, not shown]".to_owned(),
            ContentBlock::Audio(_) => "[audio, not shown]".to_owned(),
            ContentBlock::Resource(_) | ContentBlock::ResourceLink(_) => "[a resource, not read]".to_owned(),
            _ => "[content of a kind Remit does not read]".to_owned(),
        })
        .collect();
    if parts.is_empty()
        && let Some(s) = &r.structured_content
    {
        parts.push(s.to_string());
    }
    (parts.join("\n"), r.is_error == Some(true))
}

/// The host:port an address variable's value names: a URL's host and port, or a host:port as written.
fn egress_of(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(u) = url::Url::parse(value)
        && let (Some(host), Some(port)) = (u.host_str(), u.port_or_known_default())
    {
        return Some(format!("{host}:{port}"));
    }
    let (host, port) = value.rsplit_once(':')?;
    (!host.is_empty() && port.parse::<u16>().is_ok()).then(|| value.to_owned())
}

/// The line under every tool of an MCP server: what enforces that it only reads (ADR-0018, invariant 4).
pub(crate) const MCP_ENFORCED: &str =
    "A tool of an outside MCP server: that it only reads is its author's claim, enforced: credential.";

impl Provider for McpSource {
    fn name(&self) -> String {
        self.ext.label.clone()
    }

    fn missing(&self) -> Vec<String> {
        self.ext
            .vars
            .iter()
            .filter(|v| !v.optional && self.env.get(&v.name).is_empty())
            .map(|v| v.name.clone())
            .collect()
    }

    fn vars(&self) -> Vec<Var> {
        self.ext.vars.clone()
    }

    /// The pinned tools, each called through the session of this connection.
    fn tools(self: Arc<Self>) -> Vec<Tool> {
        let mut out = Vec::with_capacity(self.ext.tools.len());
        for pin in &self.ext.tools {
            let (me, p) = (Arc::clone(&self), pin.clone());
            out.push(schema_tool(
                &format!("{}_{}", self.ext.id, pin.name),
                &format!("{} {MCP_ENFORCED}", pin.description),
                &pin.input_schema,
                pin.effect,
                Arc::new(move |arguments: String| {
                    let (me, p) = (Arc::clone(&me), p.clone());
                    Box::pin(async move { me.call(&p, arguments).await })
                }),
            ));
        }
        out
    }

    fn listing(self: Arc<Self>) -> Option<BoxFuture<'static, Result<Vec<ExtensionTool>>>> {
        Some(Box::pin(async move { Ok(self.live().await?.fresh.clone()) }))
    }
}
