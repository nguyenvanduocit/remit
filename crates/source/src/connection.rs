//! A source is reached through connections: the same code with another set of credentials reaches another place (a staging
//! and a production MongoDB, two Grafanas). The owner names a connection; `default` is the one the process environment and
//! the env files serve. A tool takes the connection it reads as an argument, so tool names stay one per question.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use grant::Effect;
use serde_json::Value;
use wire::app::{ConnectionView, SourceTool, SourceVarState, SourceVariable, ToolSource};

use crate::env::Env;
use crate::error::{BoxError, BoxFuture, Error, Result, err, fail};
use crate::extension::{PluginRef, Plugins};
use crate::gate::{Caller, Form, Gate, Policy, Recorder, Resolution};
use crate::mcp::Pins;
use crate::poll::{POLL_CALL, Pollable};
use crate::program::{RUN_TOOL, Runner, run_what};
use crate::redact::redact_text;
use crate::script::{Language, SCRIPT_ENFORCED, SCRIPT_TOOL, ScriptInput, script_languages, script_what};
use crate::source::{EVIDENCE, Exec, Kind, Provider, Shared, with_connection};

/// The connection the process environment and env files serve.
pub const DEFAULT_CONNECTION: &str = "default";

/// One connection the owner made: its name, and the line that says what it reaches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConnectionInfo {
    pub name: String,
    pub description: String,
}

/// What the sources need of the place the owner's credentials are kept. The orchestrator implements it over its sealed
/// store; this crate depends on nothing of the workspace but `grant` (ADR-0015).
pub trait Store: Send + Sync {
    /// The connections the owner made for a source.
    fn connections<'a>(&'a self, source: &'a str) -> BoxFuture<'a, std::result::Result<Vec<ConnectionInfo>, BoxError>>;
    /// The values stored for one connection, by variable name, secrets already opened. A connection with none has an empty
    /// map. An error stops the call that asked.
    fn values<'a>(
        &'a self,
        source: &'a str,
        connection: &'a str,
    ) -> BoxFuture<'a, std::result::Result<BTreeMap<String, String>, BoxError>>;
}

/// A script run for Remit itself (`Set::run_script`): what it printed.
pub(crate) type ScriptValue = Arc<dyn Fn(ScriptInput) -> BoxFuture<'static, Result<String>> + Send + Sync>;

/// What a call's input was, without its content, for the recorder.
pub(crate) type Detail = Arc<dyn Fn(&Value) -> String + Send + Sync>;

/// What answers one call: how to carry out the act (`exec` for a tool, a program or a script, which answer text; `poll` for a
/// poll, which answers items; `script` for a script Remit runs itself; `lend` for a plugin's own run), the connection it
/// belongs to (its name, and as "<source>/<name>" the label that heads the answer), what the act does to the system and the
/// form it is made in.
#[derive(Clone)]
pub(crate) struct Resolved {
    pub exec: Option<Exec>,
    pub poll: Option<Arc<dyn Pollable>>,
    pub script: Option<ScriptValue>,
    pub lend: Option<(PluginRef, BTreeMap<String, String>)>,
    pub label: String,
    pub connection: String,
    pub effect: Effect,
    pub form: Form,
    /// When set, what the input was without its content, for the recorder (a script's language, size and hash).
    pub detail: Option<Detail>,
}

impl Resolved {
    pub(crate) fn new(label: String, connection: &str, effect: Effect, form: Form) -> Self {
        Self {
            exec: None,
            poll: None,
            script: None,
            lend: None,
            label,
            connection: connection.to_owned(),
            effect,
            form,
            detail: None,
        }
    }
}

/// What a Set's gate asks and tells, and where its programs and scripts run.
#[derive(Clone, Default)]
pub struct Options {
    policy: Option<Arc<dyn Policy>>,
    recorder: Option<Arc<dyn Recorder>>,
    runner: Option<Arc<dyn Runner>>,
    plugins: Option<Arc<dyn Plugins>>,
    pins: Option<Arc<dyn Pins>>,
    languages: Vec<Language>,
}

impl Options {
    /// Every call passes `p`. Without one the gate lets a read by a typed tool through and refuses everything else.
    #[must_use]
    pub fn with_policy(mut self, p: Arc<dyn Policy>) -> Self {
        self.policy = Some(p);
        self
    }

    /// The gate tells `r` of every call.
    #[must_use]
    pub fn with_recorder(mut self, r: Arc<dyn Recorder>) -> Self {
        self.recorder = Some(r);
        self
    }

    /// A place to run programs. Without one the run tool is not offered.
    #[must_use]
    pub fn with_runner(mut self, r: Arc<dyn Runner>) -> Self {
        self.runner = Some(r);
        self
    }

    /// The place the programs of extensions run. Without one an extension's tools answer that it runs no program.
    #[must_use]
    pub fn with_plugins(mut self, p: Arc<dyn Plugins>) -> Self {
        self.plugins = Some(p);
        self
    }

    /// Where what an MCP server lists is pinned (mcp.rs). Without one a server's listing is kept nowhere.
    #[must_use]
    pub fn with_pins(mut self, p: Arc<dyn Pins>) -> Self {
        self.pins = Some(p);
        self
    }

    /// Offers the languages whose interpreter the host's jail shows: `has` says whether a program of that name is there
    /// (the orchestrator knows what the jail mounts). Without any the script tool is not offered.
    #[must_use]
    pub fn with_script_interpreters(mut self, has: impl Fn(&str) -> bool) -> Self {
        self.languages = script_languages().into_iter().filter(|l| has(l.binary)).collect();
        self
    }
}

/// The providers built from the connections' values, until `invalidate`.
#[derive(Default)]
struct Cache {
    /// "<source>/<connection>": built once from the connection's values.
    providers: HashMap<String, Arc<dyn Provider>>,
    /// Bumped by `invalidate`, so a provider built from older values is not kept.
    generation: HashMap<String, u64>,
}

pub(crate) struct SetInner {
    store: Arc<dyn Store>,
    process: Env,
    pub(crate) shared: Shared,
    /// The sources that ship with Remit.
    kinds: Vec<Kind>,
    /// The extensions the owner applied, which change while the server runs (`use_extensions`).
    exts: RwLock<Arc<Vec<Kind>>>,
    pub(crate) policy: Option<Arc<dyn Policy>>,
    pub(crate) recorder: Option<Arc<dyn Recorder>>,
    pub(crate) runner: Option<Arc<dyn Runner>>,
    pub(crate) plugins: Option<Arc<dyn Plugins>>,
    pub(crate) pins: Option<Arc<dyn Pins>>,
    /// The languages a script may be written in on this host; none: no script runs.
    pub(crate) languages: Vec<Language>,
    cache: Mutex<Cache>,
}

/// The sources of this machine and their connections, read with the owner's stored credentials. Cloning it shares it.
#[derive(Clone)]
pub struct Set {
    pub(crate) inner: Arc<SetInner>,
}

/// "<source>/<connection>"
pub(crate) fn key(source: &str, connection: &str) -> String {
    format!("{source}/{connection}")
}

impl Set {
    /// The set over the store of the owner's credentials, the process environment (read by `default` only), and the clock
    /// and the HTTP client the sources share. The options say what the gate asks and tells on every call.
    #[must_use]
    pub fn open(store: Arc<dyn Store>, process: Env, shared: Shared, opts: Options) -> Self {
        Self::of(crate::source::kinds(), store, process, shared, opts)
    }

    pub(crate) fn of(kinds: Vec<Kind>, store: Arc<dyn Store>, process: Env, shared: Shared, opts: Options) -> Self {
        Self {
            inner: Arc::new(SetInner {
                store,
                process,
                shared,
                kinds,
                exts: RwLock::new(Arc::new(Vec::new())),
                policy: opts.policy,
                recorder: opts.recorder,
                runner: opts.runner,
                plugins: opts.plugins,
                pins: opts.pins,
                languages: opts.languages,
                cache: Mutex::new(Cache::default()),
            }),
        }
    }

    /// The gate for the calls to a source's tools.
    pub(crate) fn gate(&self, k: &Kind) -> Gate {
        Gate {
            source: k.id.clone(),
            how: Resolution::Source(k.clone()),
            set: self.clone(),
        }
    }

    /// A gate whose resolution fails with `why`, so a call to nothing is still recorded once.
    pub(crate) fn refusing_gate(&self, source: &str, why: String) -> Gate {
        Gate {
            source: source.to_owned(),
            how: Resolution::Refused(why),
            set: self.clone(),
        }
    }

    /// Every source this machine has: the ones that ship, then the extensions applied so far.
    pub(crate) fn all(&self) -> Vec<Kind> {
        let exts = Arc::clone(&self.inner.exts.read().unwrap_or_else(PoisonError::into_inner));
        self.inner.kinds.iter().chain(exts.iter()).cloned().collect()
    }

    pub(crate) fn kind(&self, source: &str) -> Option<Kind> {
        self.all().into_iter().find(|k| k.id == source)
    }

    pub(crate) fn shipped(&self) -> &[Kind] {
        &self.inner.kinds
    }

    pub(crate) fn set_extensions(&self, ks: Vec<Kind>) {
        *self.inner.exts.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(ks);
        // What was built from an earlier manifest is not kept: the next call builds from this one.
        let mut cache = self.inner.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let stale: Vec<String> = cache
            .providers
            .keys()
            .filter(|k| {
                !self
                    .inner
                    .kinds
                    .iter()
                    .any(|base| k.starts_with(&format!("{}/", base.id)))
            })
            .cloned()
            .collect();
        for k in stale {
            cache.providers.remove(&k);
            bump(&mut cache.generation, &k);
        }
    }

    /// Drops what was built for a connection, so the next call reads its values again. Its owner calls it after a value or
    /// the connection itself changed. A provider that is dropped frees what it held (a client's pool) as it goes.
    pub fn invalidate(&self, source: &str, connection: &str) {
        let k = key(source, connection);
        let dropped = {
            let mut cache = self.inner.cache.lock().unwrap_or_else(PoisonError::into_inner);
            bump(&mut cache.generation, &k);
            cache.providers.remove(&k)
        };
        drop(dropped);
    }

    /// How a provider reads a connection's credentials: its stored value, and for `default` then the process environment.
    fn env(&self, connection: &str, values: BTreeMap<String, String>) -> Env {
        let process = self.inner.process.clone();
        let is_default = connection == DEFAULT_CONNECTION;
        Env::new(move |name| match values.get(name) {
            Some(v) if !v.is_empty() => v.clone(),
            _ if is_default => process.get(name),
            _ => String::new(),
        })
    }

    /// Variables of a connection, as its provider would read them now, in the order named: for Remit's own use of a
    /// credential the owner entered (the Datadog exporter's site and key, ADR-0119), never for an agent.
    pub async fn read(&self, source: &str, connection: &str, names: &[&str]) -> Result<Vec<String>> {
        let env = self.env(connection, self.values(source, connection).await?);
        Ok(names.iter().map(|n| env.get(n)).collect())
    }

    async fn values(&self, source: &str, connection: &str) -> Result<BTreeMap<String, String>> {
        self.inner
            .store
            .values(source, connection)
            .await
            .map_err(|e| Error::msg(e.to_string()))
    }

    /// The connection's provider: the one built before, or one built from its stored values now.
    pub(crate) async fn provider(&self, k: &Kind, connection: &str) -> Result<Arc<dyn Provider>> {
        let ck = key(&k.id, connection);
        let generation = {
            let cache = self.inner.cache.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(p) = cache.providers.get(&ck) {
                return Ok(Arc::clone(p));
            }
            cache.generation.get(&ck).copied().unwrap_or(0)
        };
        let values = self.values(&k.id, connection).await?;
        let p = (k.new)(self.env(connection, values), self.inner.shared.clone());
        let mut cache = self.inner.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(kept) = cache.providers.get(&ck) {
            return Ok(Arc::clone(kept));
        }
        if cache.generation.get(&ck).copied().unwrap_or(0) == generation {
            cache.providers.insert(ck, Arc::clone(&p));
        }
        Ok(p)
    }

    /// The connections of a source: the ones the owner made, and `default`, which always exists and comes first.
    pub(crate) async fn connections(&self, k: &Kind) -> Result<Vec<ConnectionInfo>> {
        let stored = self
            .inner
            .store
            .connections(&k.id)
            .await
            .map_err(|e| Error::msg(e.to_string()))?;
        let mut default = ConnectionInfo {
            name: DEFAULT_CONNECTION.to_owned(),
            description: String::new(),
        };
        let mut others = Vec::new();
        for c in stored {
            if c.name == DEFAULT_CONNECTION {
                default.description = c.description;
                continue;
            }
            others.push(c);
        }
        others.sort_by(|a, b| a.name.cmp(&b.name));
        let mut out = vec![default];
        out.extend(others);
        Ok(out)
    }

    /// The connection a call reads: the one it names, else the only configured one. With none configured it is `default`,
    /// so the answer says what to set; with several the agent is told to choose.
    pub(crate) async fn pick(&self, k: &Kind, name: &str) -> Result<String> {
        let all = self.connections(k).await?;
        if !name.is_empty() {
            if !all.iter().any(|c| c.name == name) {
                return fail!(
                    "{} has no connection {name:?}: the ones that exist are {}",
                    k.id,
                    describe(&all)
                );
            }
            return Ok(name.to_owned());
        }
        let mut configured = Vec::new();
        for c in all {
            if self.provider(k, &c.name).await?.missing().is_empty() {
                configured.push(c);
            }
        }
        match configured.as_slice() {
            [] => Ok(DEFAULT_CONNECTION.to_owned()),
            [one] => Ok(one.name.clone()),
            _ => fail!(
                "{} has several connections: name the one to read as `connection`: {}",
                k.id,
                describe(&configured)
            ),
        }
    }

    /// What answers a call of `started`'s tools. A call is resolved against the source as it is when the call is made: an
    /// extension the owner applied again since the session began runs its new program, and one he removed answers that it
    /// is gone.
    pub(crate) async fn resolve_call(&self, started: &Kind, tool: &str, connection: &str) -> Result<Resolved> {
        let Some(k) = self.kind(&started.id) else {
            return fail!("{} is no longer a source on this server", started.id);
        };
        let name = self.pick(&k, connection).await?;
        let p = self.provider(&k, &name).await?;
        let m = p.missing();
        if !m.is_empty() {
            return fail!(
                "{} is not configured on this server (connection {name}): set {}; the owner enters them when the Assistant asks him",
                p.name(),
                m.join(", ")
            );
        }
        match tool {
            RUN_TOOL => return self.program_resolved(&p, &k, &name),
            SCRIPT_TOOL => return self.script_resolved(&p, &k, &name),
            POLL_CALL => return self.poll_resolved(&p, &k, &name),
            _ => {}
        }
        let label = key(&k.id, &name);
        let provider_name = p.name();
        match p.tools().into_iter().find(|t| t.name == tool) {
            Some(t) => {
                let mut res = Resolved::new(label, &name, t.effect, Form::Tool);
                res.exec = Some(t.exec);
                Ok(res)
            }
            None => fail!("{provider_name} has no tool {tool}"),
        }
    }

    /// The tools of the sources under `caps`, for a server an agent talks to. A source with no configured connection is
    /// served too: its tools answer with the variables to set. A tool is served once; the connection that answers a call is
    /// found when it is made. The run tool is one tool for every source among them that runs programs, and the script tool
    /// one for every source that runs scripts.
    #[must_use]
    pub fn serve(&self, caps: &[&str]) -> Vec<ServedTool> {
        let mut out = Vec::new();
        let mut held = Vec::new();
        for k in self.all() {
            if !caps.contains(&k.capability.as_str()) {
                continue;
            }
            for t in self.template(&k).tools() {
                out.push(self.served(&k, &t));
            }
            held.push(k);
        }
        out.extend(self.run_tool(&held));
        out.extend(self.script_tool(&held));
        out
    }

    /// The tool `t` of source `k`, as an agent calls it: through the gate.
    fn served(&self, k: &Kind, t: &crate::source::Tool) -> ServedTool {
        let gate = self.gate(k);
        let name = t.name.clone();
        ServedTool {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: with_connection((t.schema)()),
            read_only: t.effect == Effect::Read,
            call: Arc::new(move |caller: Caller, args: Value| {
                let gate = gate.clone();
                let name = name.clone();
                Box::pin(async move {
                    let connection = asked_connection(&args);
                    gate.run(&caller, &name, &connection, args).await
                })
            }),
        }
    }

    /// The tools of the sources under `caps` that have at least one configured connection: what a chain can really read
    /// with, as against what it was offered.
    pub async fn ready_tools(&self, caps: &[&str]) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for k in self.all() {
            if !caps.contains(&k.capability.as_str()) || !self.configured_somewhere(&k).await? {
                continue;
            }
            out.extend(self.template(&k).tools().into_iter().map(|t| t.name));
        }
        Ok(out)
    }

    async fn configured_somewhere(&self, k: &Kind) -> Result<bool> {
        for c in self.connections(k).await? {
            if self.provider(k, &c.name).await?.missing().is_empty() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Every connection of every source with the variables it still needs: what the server logs at start.
    pub async fn status(&self) -> Result<Vec<Status>> {
        let mut out = Vec::new();
        for k in self.all() {
            for c in self.connections(&k).await? {
                let p = self.provider(&k, &c.name).await?;
                out.push(Status {
                    source: k.id.clone(),
                    connection: c.name,
                    capability: k.capability.clone(),
                    missing: p.missing(),
                });
            }
        }
        Ok(out)
    }

    /// Every source with its tools and connections.
    pub async fn catalog(&self) -> Result<Vec<ToolSource>> {
        let mut out = Vec::new();
        for k in self.all() {
            let tmpl = self.template(&k);
            let mut v = ToolSource {
                id: k.id.clone(),
                label: tmpl.name(),
                capability: k.capability.clone(),
                tools: Vec::new(),
                connections: Vec::new(),
            };
            for t in Arc::clone(&tmpl).tools() {
                let what = t
                    .description
                    .strip_suffix(&format!(" {EVIDENCE}"))
                    .unwrap_or(&t.description)
                    .to_owned();
                v.tools.push(SourceTool { name: t.name, what });
            }
            if let Some(spec) = tmpl.program() {
                v.tools.push(SourceTool {
                    name: RUN_TOOL.to_owned(),
                    what: format!("{} (enforced: credential)", run_what(&tmpl.name(), &spec)),
                });
            }
            if let Some(spec) = tmpl.inject().filter(|_| self.can_script()) {
                v.tools.push(SourceTool {
                    name: SCRIPT_TOOL.to_owned(),
                    what: format!("{}. {SCRIPT_ENFORCED}", script_what(&tmpl.name(), &spec)),
                });
            }
            for c in self.connections(&k).await? {
                let (values, problem) = match self.values(&k.id, &c.name).await {
                    Ok(v) => (v, String::new()),
                    Err(e) => (BTreeMap::new(), redact_text(&e.to_string())),
                };
                let p = (k.new)(self.env(&c.name, values.clone()), self.inner.shared.clone());
                let mut cv = ConnectionView {
                    name: c.name.clone(),
                    description: c.description,
                    configured: problem.is_empty() && p.missing().is_empty(),
                    missing: p.missing(),
                    problem,
                    vars: Vec::new(),
                };
                for x in p.vars() {
                    let mut vv = unset_variable(&x);
                    let stored = values.get(&x.name).filter(|v| !v.is_empty());
                    let from_env = self.inner.process.get(&x.name);
                    if let Some(v) = stored {
                        vv.state = SourceVarState::Set;
                        if !x.secret {
                            vv.value.clone_from(v);
                        }
                    } else if c.name == DEFAULT_CONNECTION && !from_env.is_empty() {
                        vv.state = SourceVarState::Environment;
                        if !x.secret {
                            vv.value = from_env;
                        }
                    }
                    cv.vars.push(vv);
                }
                v.connections.push(cv);
            }
            out.push(v);
        }
        Ok(out)
    }

    /// A connection of the source that has no value yet: the variables a new connection needs. It reads nothing, so it is
    /// what the owner is asked for when the Assistant makes one.
    #[must_use]
    pub fn blank(&self, source: &str) -> Option<(ToolSource, ConnectionView)> {
        let k = self.kind(source)?;
        let p = (k.new)(Env::none(), self.inner.shared.clone());
        let entry = ToolSource {
            id: k.id.clone(),
            label: p.name(),
            capability: k.capability,
            tools: Vec::new(),
            connections: Vec::new(),
        };
        let cv = ConnectionView {
            name: String::new(),
            description: String::new(),
            configured: false,
            missing: p.missing(),
            problem: String::new(),
            vars: p.vars().iter().map(unset_variable).collect(),
        };
        Some((entry, cv))
    }

    /// The ids of the sources, in the order the registry lists them.
    #[must_use]
    pub fn source_ids(&self) -> Vec<String> {
        self.all().into_iter().map(|k| k.id).collect()
    }
}

fn bump(generation: &mut HashMap<String, u64>, k: &str) {
    let g = generation.entry(k.to_owned()).or_insert(0);
    *g = g.saturating_add(1);
}

/// "name: description" lines, for an agent that must choose.
fn describe(cs: &[ConnectionInfo]) -> String {
    cs.iter()
        .map(|c| {
            if c.description.is_empty() {
                c.name.clone()
            } else {
                format!("{}: {}", c.name, c.description)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The `connection` argument a call carried: the kernel adds it to every tool's schema, so a tool's own input type does not
/// hold it.
pub(crate) fn asked_connection(args: &Value) -> String {
    args.get("connection")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// A tool as a server serves it: the name, the description and the input schema an agent reads, and the call that passes
/// the gate.
#[derive(Clone)]
pub struct ServedTool {
    pub name: String,
    pub description: String,
    /// A JSON Schema object; its `properties` hold `connection`.
    pub input_schema: Value,
    /// Every call only reads (the tool's effect is read), so the agent's runtime may run it beside other readers.
    pub read_only: bool,
    call: Arc<dyn Fn(Caller, Value) -> BoxFuture<'static, Result<String>> + Send + Sync>,
}

impl ServedTool {
    pub(crate) fn new(
        name: String,
        description: String,
        input_schema: Value,
        read_only: bool,
        call: Arc<dyn Fn(Caller, Value) -> BoxFuture<'static, Result<String>> + Send + Sync>,
    ) -> Self {
        Self {
            name,
            description,
            input_schema,
            read_only,
            call,
        }
    }

    /// One call by `caller` with the agent's `arguments` (a JSON object): the answer, or an error that is safe to show.
    pub async fn call(&self, caller: Caller, arguments: Value) -> Result<String> {
        (self.call)(caller, arguments).await
    }
}

/// Whether a connection of a source can be read, and what it lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub source: String,
    pub connection: String,
    pub capability: String,
    pub missing: Vec<String>,
}

/// Variable x of a source's connection, as one that is not set.
fn unset_variable(x: &crate::source::Var) -> SourceVariable {
    SourceVariable {
        name: x.name.clone(),
        what: x.what.clone(),
        secret: x.secret,
        optional: x.optional,
        address: x.address,
        state: SourceVarState::Unset,
        value: String::new(),
    }
}

/// `Err` naming `id` as no longer a source, for the callers that look one up.
pub(crate) fn no_source(id: &str) -> Error {
    err!("there is no source {id:?}")
}
