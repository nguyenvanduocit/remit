//! An extension is a source somebody else wrote: a plugin (ADR-0034), a program that serves the plugin protocol, which the
//! owner applied as a manifest. Its tools are the ones it described when the owner was shown it; an agent calls them through
//! the gate like any tool, and the plugin receives the credential variables of the connection it was called through. An
//! extension may instead be an outside MCP server (`mcp`, mcp.rs): Remit is its client, and its tools are the ones pinned
//! from what it listed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::SystemTime;

use base64::Engine as _;
use grant::Effect;
use hmac::{Hmac, Mac};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Sha256, Sha512};
use wire::plugin::{EffectDef, is_tool_name, is_var_name};

use crate::api::{encode_query, parse_query};
use crate::connection::Set;
use crate::env::Env;
use crate::error::{BoxError, BoxFuture, Error, Result, err, fail};
use crate::mcp::{ExtensionMcp, McpSource};
use crate::poll::{PollResult, Pollable, without_secrets};
use crate::redact::scrub;
use crate::script::{InjectSpec, Sign, auth_secrets};
use crate::source::{CAPABILITIES, EVIDENCE, FROM_OUTSIDE, Kind, Provider, Shared, Tool, Var, kinds};
use crate::text::limit;

/// A manifest: what the owner applied, as data. It names the program by a path inside the extensions directory and by the
/// hash of the file, so a file that is another by the time it runs does not run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    /// What a connection and a tool name are prefixed by: a lower case word.
    #[serde(default)]
    pub id: String,
    /// The name the owner reads.
    #[serde(default)]
    pub label: String,
    /// Which capability of the catalog a chain holds to get this source.
    #[serde(default)]
    pub capability: String,
    /// The file, relative to the extensions directory: <id>/<file>.
    #[serde(default)]
    pub program: String,
    /// The hash of the file, in hex: the one the owner applied.
    #[serde(default)]
    pub sha256: String,
    /// The variables of the credential.
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub vars: Vec<Var>,
    /// What the program described: the tools an agent calls.
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub tools: Vec<ExtensionTool>,
    /// The program is a source of events that a sensor of Remit polls.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub poll: bool,
    /// The program provides Claude accounts (ADR-0071): each configured connection is an account agents run on, and the
    /// program answers the environment of a session that opens on it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub accounts: bool,
    /// The frontend the program brings: the entry of its remote and the slots it fills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<ExtensionUi>,
    /// What the program adds to the signal pipeline (ADR-0049): a sensor record names one of the kinds, a subscription's
    /// action one of the processors as `plugin:<id>.<name>`. Subscriptions are the ones it ships, which only a plugin that
    /// ships with Remit may.
    #[serde(
        rename = "sensorKinds",
        default,
        deserialize_with = "crate::null_json::null_as_zero",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub sensor_kinds: Vec<ExtensionSensorKind>,
    #[serde(
        default,
        deserialize_with = "crate::null_json::null_as_zero",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub processors: Vec<ExtensionProcessor>,
    #[serde(
        default,
        deserialize_with = "crate::null_json::null_as_zero",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub subscriptions: Vec<ExtensionSubscription>,
    /// The program is Remit's own binary, which serves the plugin when started as `remit plugin <id>`: it has no file of its
    /// own to pin. It is never stored: only the server sets it, for the plugins that ship with it.
    #[serde(skip)]
    pub bundled: bool,
    /// Instead of a program, the extension is an API reached through the injecting proxy (ADR-0018, form T1): the platform
    /// adds the credential under one scheme of a closed set, and an agent reaches the API with scripts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<ExtensionAuth>,
    /// Instead of a program, an outside MCP server Remit is the client of (ADR-0077): `tools` are then its pins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<ExtensionMcp>,
}

/// One tool of a plugin as it described it: the input schema is a JSON Schema document, the effect one of the grant
/// ladder's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionTool {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "inputSchema", default)]
    pub input_schema: String,
    #[serde(with = "EffectDef")]
    pub effect: Effect,
    /// For a tool of an MCP server: the name the server calls it by.
    #[serde(rename = "mcpName", default, skip_serializing_if = "String::is_empty")]
    pub mcp_name: String,
}

/// A kind of sensor a plugin adds, as it described it: the settings schema is a JSON Schema document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSensorKind {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "settingsSchema", default)]
    pub settings_schema: String,
}

/// A processor a plugin adds, as it described it: the params schema is a JSON Schema document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionProcessor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "paramsSchema", default)]
    pub params_schema: String,
    /// What it does to the connection it is lent; a manifest that names none lends a read.
    #[serde(with = "EffectDef", default = "lends_a_read")]
    pub effect: Effect,
}

const fn lends_a_read() -> Effect {
    Effect::Read
}

/// A subscription a bundled plugin ships: the pattern of keys it listens to, its condition, and what it does: a processor
/// of the plugin with its parameters (a JSON object), or a core action (tell, run) with the job and the expressions (`with`)
/// that fill its parameters, shipped as data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSubscription {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub note: String,
    #[serde(rename = "match", default)]
    pub pattern: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expr: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub processor: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub params: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub action: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub job: String,
    #[serde(
        default,
        deserialize_with = "crate::null_json::null_as_zero",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub with: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
}

/// The frontend of a plugin: a Module Federation remote whose entry is one of its assets, and the slots it fills.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionUi {
    #[serde(default)]
    pub entry: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub slots: Vec<ExtensionSlot>,
}

/// One place in the app a plugin fills with a component its remote exposes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSlot {
    #[serde(default)]
    pub slot: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub expose: String,
}

/// How the proxy reaches an API and adds its credential, as data. The schemes are a closed set, each a pure function from the
/// connection's variables to headers: bearer (one variable: Authorization: Bearer <v>), basic (two: the user and the
/// password), hmac-query (one: the secret; the platform sorts the query's parameters, takes an HMAC of them as name=value
/// joined with &, and adds it as the parameter the manifest names, as payment gateways of the VNPay kind ask), header (one:
/// <Header>: <v>) and sigv4 (four: the access key id, the secret key, the region and the service, in that order: the proxy
/// signs each request as AWS Signature Version 4, so the key never reaches a script).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionAuth {
    /// https://host[:port][/prefix]: the one host a script reaches.
    #[serde(default)]
    pub base: String,
    #[serde(default)]
    pub scheme: String,
    /// For the header scheme: the header's name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub header: String,
    /// For the hmac-query scheme: the query parameter the hash goes in.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub param: String,
    /// For the hmac-query scheme: sha256 or sha512.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
    /// The variables the scheme reads, each one the extension declares.
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub vars: Vec<String>,
    /// Paths where POST only reads, as for every proxied source.
    #[serde(
        rename = "readPosts",
        default,
        deserialize_with = "crate::null_json::null_as_zero",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub read_posts: Vec<String>,
    /// A declared address variable that, when set, replaces `base`: the API of an Enterprise server.
    #[serde(rename = "baseVar", default, skip_serializing_if = "String::is_empty")]
    pub base_var: String,
    /// Sent with every request, besides the credential: the version of an API.
    #[serde(
        default,
        deserialize_with = "crate::null_json::null_as_zero",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub headers: BTreeMap<String, String>,
}

pub(crate) fn regex(pattern: &str) -> Regex {
    #[expect(clippy::expect_used, reason = "the patterns are constants a test compiles")]
    Regex::new(pattern).expect("a manifest shape compiles")
}

static EXTENSION_ID: LazyLock<Regex> = LazyLock::new(|| regex(r"^[a-z][a-z0-9]{1,23}$"));
static PIN_HEX: LazyLock<Regex> = LazyLock::new(|| regex(r"^[0-9a-f]{64}$"));
static AUTH_HEADER: LazyLock<Regex> = LazyLock::new(|| regex(r"^[A-Za-z][A-Za-z0-9-]{0,63}$"));
static QUERY_PARAM: LazyLock<Regex> = LazyLock::new(|| regex(r"^[A-Za-z][A-Za-z0-9_]{0,63}$"));

impl Extension {
    /// Whether this is a manifest an owner may apply. It is pure: the manifest is the one hostile input of an extension.
    pub fn validate(&self) -> Result<()> {
        if !EXTENSION_ID.is_match(&self.id) {
            return fail!(
                "id {:?} is not a lower case word of 2 to 24 letters and digits",
                limit(&self.id, 40)
            );
        }
        if kinds().iter().any(|k| k.id == self.id) {
            return fail!("id {:?} is a source that ships with Remit", self.id);
        }
        if !CAPABILITIES.iter().any(|(id, _)| *id == self.capability) {
            return fail!("capability {:?} is not one of the catalog", limit(&self.capability, 40));
        }
        if self.label.trim().is_empty() {
            return fail!("label is empty");
        }
        if self.vars.len() > 16 {
            return fail!("more than 16 credential variables");
        }
        for v in &self.vars {
            if !is_var_name(&v.name) {
                return fail!(
                    "variable {:?} is not a credential name: upper case words that do not move the loader or the path",
                    limit(&v.name, 40)
                );
            }
        }
        if let Some(mcp) = &self.mcp {
            return self.validate_mcp(mcp);
        }
        let Some(auth) = &self.auth else {
            return self.validate_program();
        };
        auth.validate(&self.vars)?;
        if !self.has_program() {
            return Ok(());
        }
        // An API reached through the proxy and a program with tools, a poll or a pipeline: both.
        self.validate_program()
    }

    /// Whether the extension is a program: it names one, or it is Remit's own, or it has what only a program brings.
    fn has_program(&self) -> bool {
        self.bundled
            || !self.program.is_empty()
            || !self.sha256.is_empty()
            || !self.tools.is_empty()
            || self.ui.is_some()
            || self.poll
            || self.accounts
            || self.adds_to_pipeline()
    }

    /// An MCP server: its record, and the pins it has so far, which may be none before its first listing.
    fn validate_mcp(&self, mcp: &ExtensionMcp) -> Result<()> {
        if self.auth.is_some()
            || !self.program.is_empty()
            || !self.sha256.is_empty()
            || self.poll
            || self.accounts
            || self.ui.is_some()
            || self.adds_to_pipeline()
        {
            return fail!(
                "an MCP server is reached as one: no program, auth, poll, account, frontend or pipeline beside it"
            );
        }
        mcp.validate(&self.vars)?;
        self.validate_tools()?;
        if self.tools.iter().any(|t| t.mcp_name.is_empty()) {
            return fail!("a pin of an MCP server names the tool the server calls it by");
        }
        Ok(())
    }

    /// Whether the extension adds a sensor kind, a processor or a subscription to the signal pipeline (ADR-0049).
    fn adds_to_pipeline(&self) -> bool {
        !self.sensor_kinds.is_empty() || !self.processors.is_empty() || !self.subscriptions.is_empty()
    }

    fn validate_program(&self) -> Result<()> {
        if self.tools.is_empty() && !self.poll && !self.accounts && self.ui.is_none() && !self.adds_to_pipeline() {
            return fail!(
                "the program has no tool, no poll, no account, no sensor kind, no processor, no subscription and no ui: an extension that does nothing is not one"
            );
        }
        if !self.bundled {
            if !PIN_HEX.is_match(&self.sha256) {
                return fail!(
                    "sha256 is not the 64 hex digits of the hash of the program: an extension is pinned to the file the owner applied"
                );
            }
            let under = format!("{}/", self.id);
            if !self
                .program_path()
                .is_ok_and(|rel| rel.starts_with(&under) && rel != under)
            {
                return fail!(
                    "program {:?} is not a file under {}/ in the extensions directory",
                    limit(&self.program, 80),
                    self.id
                );
            }
        }
        self.validate_tools()?;
        if self
            .ui
            .as_ref()
            .is_some_and(|ui| ui.entry.is_empty() || ui.slots.is_empty())
        {
            return fail!("ui has no entry or no slot");
        }
        Ok(())
    }

    /// Whether every tool is a word, declared once, with a JSON input schema.
    fn validate_tools(&self) -> Result<()> {
        let mut seen: Vec<&str> = Vec::new();
        for t in &self.tools {
            if !is_tool_name(&t.name) || seen.contains(&t.name.as_str()) {
                return fail!(
                    "tool {:?} is not a lower case word of 2 to 48 letters, digits and underscores, or is declared twice",
                    limit(&t.name, 60)
                );
            }
            seen.push(&t.name);
            if serde_json::from_str::<Value>(&t.input_schema).is_err() {
                return fail!("tool {}: input schema is not JSON", t.name);
            }
        }
        Ok(())
    }

    /// What the processor `name` does to the connection it is lent: its declared effect, read for a processor it does not
    /// declare.
    #[must_use]
    pub fn lend_effect(&self, processor: &str) -> Effect {
        self.processors
            .iter()
            .find(|p| p.name == processor)
            .map_or(Effect::Read, |p| p.effect)
    }

    /// The program relative to the extensions directory, cleaned, or an error when it leaves it.
    fn program_path(&self) -> Result<String> {
        if self.program.is_empty() || self.program.starts_with('/') {
            return fail!("not relative");
        }
        let rel = clean(&self.program);
        if rel == "." || rel == ".." || rel.starts_with("../") {
            return fail!("leaves the directory");
        }
        Ok(rel)
    }
}

/// A relative slash path cleaned lexically: no empty or `.` element, and each `..` takes
/// the element before it when there is one.
fn clean(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if out.last().is_some_and(|l| *l != "..") => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    if out.is_empty() { ".".to_owned() } else { out.join("/") }
}

impl ExtensionAuth {
    /// Whether this is a way to reach an API that the proxy can enforce: one https host that is a name, a scheme of the
    /// closed set whose variables are declared, and paths that start at the root.
    fn validate(&self, declared: &[Var]) -> Result<()> {
        let base = url::Url::parse(&self.base)
            .ok()
            .filter(|b| b.scheme() == "https" && b.host_str().is_some());
        let Some(base) = base else {
            return fail!("auth.base {:?} is not an https address", limit(&self.base, 80));
        };
        if !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.cannot_be_a_base()
        {
            return fail!("auth.base holds a login, a query or a fragment: it is a scheme, a host and a path");
        }
        let host = base.host_str().unwrap_or_default();
        if !matches!(base.host(), Some(url::Host::Domain(_))) || host == "localhost" || !host.contains('.') {
            return fail!(
                "auth.base host {:?} is not a public name: an address of a machine is not an API to reach",
                limit(host, 60)
            );
        }
        let want = match self.scheme.as_str() {
            "bearer" | "header" | "hmac-query" => 1,
            "basic" => 2,
            "sigv4" => 4,
            _ => {
                return fail!(
                    "auth.scheme {:?} is not one of bearer, basic, header, sigv4, hmac-query",
                    limit(&self.scheme, 40)
                );
            }
        };
        if self.vars.len() != want {
            return fail!(
                "auth.scheme {} reads {want} variable(s), not {}",
                self.scheme,
                self.vars.len()
            );
        }
        const HOP: &[&str] = &[
            "host",
            "content-length",
            "transfer-encoding",
            "connection",
            "upgrade",
            "te",
            "trailer",
        ];
        if self.scheme == "header"
            && (!AUTH_HEADER.is_match(&self.header) || HOP.contains(&self.header.to_lowercase().as_str()))
        {
            return fail!(
                "auth.header {:?} is not a header a credential goes in",
                limit(&self.header, 40)
            );
        }
        if self.scheme != "header" && !self.header.is_empty() {
            return fail!("auth.header is for the header scheme");
        }
        if self.scheme == "hmac-query"
            && (!QUERY_PARAM.is_match(&self.param) || (self.hash != "sha256" && self.hash != "sha512"))
        {
            return fail!(
                "auth.param is the name of a query parameter and auth.hash is sha256 or sha512, for the hmac-query scheme"
            );
        }
        if self.scheme != "hmac-query" && (!self.param.is_empty() || !self.hash.is_empty()) {
            return fail!("auth.param and auth.hash are for the hmac-query scheme");
        }
        for v in &self.vars {
            if !declared.iter().any(|d| d.name == *v) {
                return fail!(
                    "auth reads {:?}, which the extension does not declare in vars",
                    limit(v, 40)
                );
            }
        }
        if !self.base_var.is_empty() && !declared.iter().any(|d| d.name == self.base_var && d.address) {
            return fail!(
                "auth.baseVar {:?} is not an address variable the extension declares",
                limit(&self.base_var, 40)
            );
        }
        for (name, value) in &self.headers {
            if !AUTH_HEADER.is_match(name)
                || value.len() > 200
                || value.contains(['\r', '\n'])
                || name.eq_ignore_ascii_case("authorization")
            {
                return fail!(
                    "auth.headers {:?} is not a header sent with every request",
                    limit(name, 40)
                );
            }
        }
        for p in &self.read_posts {
            if !p.starts_with('/') || p.len() > 200 || p.contains([' ', '\t', '\r', '\n', '?', '#']) {
                return fail!("auth.readPosts {:?} is not a path", limit(p, 60));
            }
        }
        Ok(())
    }
}

/// The program of an extension: where it is and the hash it must have.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginRef {
    pub id: String,
    pub binary: PathBuf,
    pub sha256: String,
    /// The program is Remit itself, which the Plugins implementation knows how to start.
    pub bundled: bool,
}

/// Runs the programs of extensions: the narrow part of a plugin this crate calls. The host implements it over the plugin
/// crate; this crate declares what it needs and depends on neither. `vars` are the credential variables of the connection
/// the call came through.
pub trait Plugins: Send + Sync {
    fn call<'a>(
        &'a self,
        p: &'a PluginRef,
        tool: &'a str,
        arguments: &'a str,
        vars: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, std::result::Result<String, BoxError>>;

    fn poll<'a>(
        &'a self,
        p: &'a PluginRef,
        query: &'a str,
        cursor: &'a str,
        limit: usize,
        vars: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, std::result::Result<PollResult, BoxError>>;
}

impl Set {
    /// Makes the extensions under `root` the Set's extensions, in place of the ones it had: sessions started after it see
    /// them, and the connections of an extension no longer there answer that it is gone. A manifest that does not validate
    /// adds nothing: whoever applies one validates it first, and what is stored is only what passed.
    pub fn use_extensions(&self, root: &Path, exts: &[Extension]) {
        let mut ks: Vec<Kind> = Vec::new();
        for e in exts {
            if e.validate().is_err() || self.shipped().iter().any(|k| k.id == e.id) || ks.iter().any(|k| k.id == e.id) {
                continue;
            }
            if let Some(mcp) = e.mcp.clone() {
                let (ext, runner, pins) = (e.clone(), self.inner.runner.clone(), self.inner.pins.clone());
                ks.push(Kind {
                    id: e.id.clone(),
                    capability: e.capability.clone(),
                    new: Arc::new(move |env: Env, shared: Shared| -> Arc<dyn Provider> {
                        Arc::new(McpSource {
                            ext: ext.clone(),
                            mcp: mcp.clone(),
                            env,
                            http: shared.http,
                            runner: runner.clone(),
                            pins: pins.clone(),
                            live: tokio::sync::Mutex::new(None),
                        })
                    }),
                });
                continue;
            }
            let (ext, root, plugins) = (e.clone(), root.to_path_buf(), self.inner.plugins.clone());
            ks.push(Kind {
                id: e.id.clone(),
                capability: e.capability.clone(),
                new: Arc::new(move |env: Env, _: Shared| -> Arc<dyn Provider> {
                    Arc::new(ExtensionSource {
                        ext: ext.clone(),
                        root: root.clone(),
                        env,
                        plugins: plugins.clone(),
                    })
                }),
            });
        }
        self.set_extensions(ks);
    }
}

/// One connection of an extension. What it is reached as follows from the manifest: a program has its tools and lends its
/// connection, a polling program is polled, and an API behind the proxy (`auth`) is reached by scripts.
struct ExtensionSource {
    ext: Extension,
    root: PathBuf,
    env: Env,
    /// The Set's: where the program is called.
    plugins: Option<Arc<dyn Plugins>>,
}

impl ExtensionSource {
    /// Whether the extension is a program, and not only an API behind the proxy.
    fn is_program(&self) -> bool {
        self.ext.auth.is_none() || self.ext.has_program()
    }

    /// The program the owner applied, where it is on this server.
    fn reference(&self) -> PluginRef {
        if self.ext.bundled {
            return PluginRef {
                id: self.ext.id.clone(),
                bundled: true,
                ..PluginRef::default()
            };
        }
        // The manifest passed `validate`, so the path stays under the directory.
        let rel = self.ext.program_path().unwrap_or_default();
        PluginRef {
            id: self.ext.id.clone(),
            binary: self.root.join(rel),
            sha256: self.ext.sha256.clone(),
            bundled: false,
        }
    }

    /// The credential variables the connection has set, which the program receives with every call.
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

    /// The values of the credential variables that are secret: removed from what the program answers.
    fn secrets(&self) -> Vec<String> {
        self.ext
            .vars
            .iter()
            .filter(|v| v.secret)
            .map(|v| self.env.get(&v.name))
            .filter(|v| !v.is_empty())
            .collect()
    }
}

impl Provider for ExtensionSource {
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

    /// The program's tools, named <id>_<tool> so that no two sources share one. Each answers with what the program says,
    /// cleaned of the connection's secrets. An API behind the proxy has none: it is reached by the script tool.
    fn tools(self: Arc<Self>) -> Vec<Tool> {
        if !self.is_program() {
            return Vec::new();
        }
        let mut tools = Vec::with_capacity(self.ext.tools.len());
        for t in &self.ext.tools {
            let effect = t.effect;
            let me = Arc::clone(&self);
            let tool = t.name.clone();
            tools.push(schema_tool(
                &format!("{}_{}", self.ext.id, t.name),
                &t.description,
                &t.input_schema,
                effect,
                Arc::new(move |arguments: String| {
                    let (me, tool) = (Arc::clone(&me), tool.clone());
                    Box::pin(async move {
                        let Some(plugins) = me.plugins.clone() else {
                            return fail!("{} runs no program on this server", me.ext.id);
                        };
                        let secrets = me.secrets();
                        match plugins.call(&me.reference(), &tool, &arguments, &me.values()).await {
                            Ok(text) => Ok(scrub(&text, &secrets)),
                            Err(e) => Err(Error::msg(scrub(&e.to_string(), &secrets))),
                        }
                    })
                }),
            ));
        }
        tools
    }

    fn poller(self: Arc<Self>) -> Option<Arc<dyn Pollable>> {
        if self.ext.poll { Some(self) } else { None }
    }

    fn lend(&self) -> Option<(PluginRef, BTreeMap<String, String>)> {
        self.is_program().then(|| (self.reference(), self.values()))
    }

    /// The proxy's way to the API: the manifest's address, and the credential added under the manifest's scheme from the
    /// connection's variables. Every spelling of the credential an API may echo is removed from what a script is answered.
    fn inject(&self) -> Option<InjectSpec> {
        let a = self.ext.auth.as_ref()?;
        let v = |i: usize| a.vars.get(i).map(|name| self.env.get(name)).unwrap_or_default();
        let custom = self.env.get(&a.base_var).trim().trim_end_matches('/').to_owned();
        let base = if !a.base_var.is_empty() && !custom.is_empty() {
            custom
        } else {
            a.base.clone()
        };
        let mut spec = InjectSpec {
            base,
            read_posts: a.read_posts.clone(),
            ..InjectSpec::default()
        };
        match a.scheme.as_str() {
            "bearer" => {
                let value = format!("Bearer {}", v(0));
                spec.headers.insert("Authorization".to_owned(), value.clone());
                spec.secrets = auth_secrets(&value, &[v(0)]);
            }
            "basic" => {
                let login = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", v(0), v(1)));
                let value = format!("Basic {login}");
                spec.headers.insert("Authorization".to_owned(), value.clone());
                spec.secrets = auth_secrets(&value, &[login, v(1)]);
            }
            "header" => {
                spec.headers.insert(a.header.clone(), v(0));
                spec.secrets = vec![v(0)];
            }
            "hmac-query" => {
                spec.sign = Some(hmac_query(v(0), a.param.clone(), a.hash.clone()));
                spec.secrets = vec![v(0)];
            }
            "sigv4" => {
                spec.sign = Some(sig_v4(v(0), v(1), v(2), v(3)));
                spec.secrets = vec![v(1)];
            }
            _ => {}
        }
        for (name, value) in &a.headers {
            spec.headers.insert(name.clone(), value.clone());
        }
        Some(spec)
    }
}

/// The bytes of a query: room for a script the owner reads in one Apply (ADR-0044, the `script` source), not for a file.
const MAX_POLL_QUERY: usize = 256 << 10;

impl Pollable for ExtensionSource {
    /// Asks the program for the events after `cursor`, with the owner's query and the connection's variables. It answers the
    /// items oldest first, each a JSON object. With no cursor it answers where "now" is and the items are dropped: a new
    /// signal never replays history.
    fn poll<'a>(&'a self, query: &'a str, cursor: &'a str, limit: usize) -> BoxFuture<'a, Result<PollResult>> {
        Box::pin(async move {
            if query.len() > MAX_POLL_QUERY || query.contains('\0') || cursor.contains('\0') {
                return fail!("poll: the query or the cursor is not text of a size a program takes");
            }
            let Some(plugins) = self.plugins.clone() else {
                return fail!("{} runs no program on this server", self.ext.id);
            };
            let secrets = self.secrets();
            let res = plugins
                .poll(&self.reference(), query, cursor, limit, &self.values())
                .await
                .map_err(|e| Error::msg(format!("poll: {}", scrub(&e.to_string(), &secrets))))?;
            // What an upstream echoed into an item (a key pasted into an issue) has no shape `redact_text` knows.
            without_secrets(checked_poll(res, cursor.is_empty(), limit), &secrets)
        })
    }
}

/// `res` if it is an answer a poll may give: no item on the first poll, none beyond the limit, each with an id and a body
/// that is a JSON object.
fn checked_poll(res: PollResult, first: bool, limit: usize) -> Result<PollResult> {
    if first {
        return Ok(PollResult {
            items: Vec::new(),
            cursor: res.cursor,
        });
    }
    if res.items.len() > limit {
        return fail!(
            "poll: the program answered {} items to a poll of at most {limit}",
            res.items.len()
        );
    }
    for (i, it) in res.items.iter().enumerate() {
        if it.id.is_empty() || !it.body.trim_start().starts_with('{') {
            return fail!(
                "poll: item {} has no id or its body is not a JSON object",
                i.saturating_add(1)
            );
        }
    }
    Ok(res)
}

/// Carries one call of a plugin's tool: the arguments as JSON text, without `connection`.
type RunText = Arc<dyn Fn(String) -> BoxFuture<'static, Result<String>> + Send + Sync>;

/// A tool whose input schema is a document, not a Rust type: a plugin's. The kernel adds the `connection` argument.
pub(crate) fn schema_tool(name: &str, description: &str, schema: &str, effect: Effect, run: RunText) -> Tool {
    let description = if effect == Effect::Read {
        format!("{description} {EVIDENCE}")
    } else {
        format!("{description} {FROM_OUTSIDE}")
    };
    let doc: Value = serde_json::from_str(schema).unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
    let tool = name.to_owned();
    Tool {
        name: name.to_owned(),
        description,
        effect,
        schema: Arc::new(move || doc.clone()),
        exec: Arc::new(move |args: Value| {
            let (run, tool) = (Arc::clone(&run), tool.clone());
            Box::pin(async move {
                let Value::Object(mut args) = args else {
                    return fail!("{tool}: input of the wrong type");
                };
                args.remove("connection");
                let body = serde_json::to_string(&args).map_err(|e| err!("{tool}: {e}"))?;
                run(body).await
            })
        }),
    }
}

/// Signs a request as AWS Signature Version 4 with a key, for a region and a service. The signature covers the method, the
/// path and query, the signed headers and the hash of the body, so the proxy signs what the upstream receives and nothing
/// else.
pub(crate) fn sig_v4(key_id: String, secret: String, region: String, service: String) -> Sign {
    Arc::new(move |parts: &mut http::request::Parts, body: &[u8], now: SystemTime| {
        sign_v4(parts, body, now, &key_id, &secret, &region, &service)
    })
}

/// One request signed as AWS Signature Version 4: the headers the signature makes are set on `parts`.
pub(crate) fn sign_v4(
    parts: &mut http::request::Parts,
    body: &[u8],
    now: SystemTime,
    key_id: &str,
    secret: &str,
    region: &str,
    service: &str,
) -> std::result::Result<(), BoxError> {
    use aws_sigv4::http_request::{SignableBody, SignableRequest, SigningSettings, sign};
    let identity = aws_credential_types::Credentials::new(key_id, secret, None, None, "remit").into();
    let params = aws_sigv4::sign::v4::SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name(service)
        .time(now)
        .settings(SigningSettings::default())
        .build()?
        .into();
    let uri = parts.uri.to_string();
    let headers: Vec<(&str, &str)> = parts
        .headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str(), v)))
        .collect();
    let signable = SignableRequest::new(
        parts.method.as_str(),
        uri.as_str(),
        headers.into_iter(),
        SignableBody::Bytes(body),
    )?;
    let (instructions, _) = sign(signable, &params)?.into_parts();
    let (signed, _) = instructions.into_parts();
    for h in signed {
        parts.headers.insert(
            http::HeaderName::from_bytes(h.name().as_bytes())?,
            http::HeaderValue::from_str(h.value())?,
        );
    }
    Ok(())
}

/// Signs the query of a request as some payment gateways ask: its parameters sorted by name, written name=value (values in
/// URL form) joined with &, the HMAC of that under the secret in hex, added as one more parameter. A parameter of that name
/// that the script sent is dropped first: a script cannot send a hash of its own.
fn hmac_query(secret: String, param: String, hash: String) -> Sign {
    Arc::new(move |parts: &mut http::request::Parts, _: &[u8], _: SystemTime| {
        let mut q: Vec<(String, String)> = parse_query(parts.uri.query().unwrap_or_default())
            .into_iter()
            .filter(|(k, _)| *k != param)
            .collect();
        let signed = encode_query(&q);
        let sum = if hash == "sha512" {
            let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes())?;
            mac.update(signed.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        } else {
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
            mac.update(signed.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        q.push((param.clone(), sum));
        let mut uri = http::uri::Parts::from(parts.uri.clone());
        let path = uri
            .path_and_query
            .as_ref()
            .map(|p| p.path().to_owned())
            .unwrap_or_else(|| "/".to_owned());
        uri.path_and_query = Some(format!("{path}?{}", encode_query(&q)).parse()?);
        parts.uri = http::Uri::from_parts(uri)?;
        Ok(())
    })
}
