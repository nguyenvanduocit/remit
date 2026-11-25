//! A plugin written as data: its tools, its poll, its sensor kinds and processors, what it ships and the frontend it brings,
//! each handler a typed function. `Definition` implements `Plugin`, so the same value runs in-process or as a child.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use link::{ErrorCode, Failure, Request};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;
use wire::plugin::{
    Agent, Auth, Effect, Firing, Manifest, PollAnswer, PollAsk, ProcessAnswer, SenseOpen, Signal, Subscription, Ui,
    Var, Vars, is_tool_name, is_var_name,
};

use crate::pipe::BoxFuture;
use crate::plugin::{Asset, Forge, Plugin, Senses};
use crate::sense::{Emitter, Processor, SensorKind, decode_object, schema_of};

/// What a handler fails with. A `link::Failure` is answered as it is; any other error is answered `FAILED` with its words,
/// which the agent reads.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// The answer a handler's error is.
pub(crate) fn failure_of(err: Error) -> Failure {
    match err.downcast::<Failure>() {
        Ok(failure) => *failure,
        Err(err) => Failure::new(ErrorCode::Failed, err.to_string()),
    }
}

fn not_found(what: String) -> Failure {
    Failure::new(ErrorCode::NotFound, what)
}

const NO_FORGE: &str = "Remit granted this plugin no forge.read";

/// What a tool, a poll, a sense, a processor or an account is called with: the credential variables of the connection the
/// call came through, and the forge when Remit granted the plugin `forge.read` ([`Call::checks`]).
#[derive(Clone, Default)]
pub struct Call {
    pub vars: Vars,
    pub(crate) forge: Option<Arc<dyn Forge>>,
}

impl Call {
    /// The value of the credential variable `name`, empty when it is not set.
    pub fn var(&self, name: &str) -> &str {
        self.vars.get(name).map_or("", String::as_str)
    }

    /// The checks the pipeline of `project` on `host` ran on commit `sha`: a `model::Checks` as JSON (`state`, `failed`), or
    /// null while it has run none. Remit holds the forge's login and has no way to merge, open or push for a plugin.
    pub async fn checks(&self, host: &str, project: &str, sha: &str) -> Result<Value, Error> {
        let forge = self.forge.as_ref().ok_or(NO_FORGE)?;
        forge
            .checks(host.to_owned(), project.to_owned(), sha.to_owned())
            .await
            .map_err(|f| match f.code {
                ErrorCode::Forbidden => Failure::new(ErrorCode::Forbidden, NO_FORGE),
                _ => Failure::new(f.code, format!("the forge on {host}: {}", f.message)),
            })
            .map_err(Error::from)
    }
}

type ToolRun = Arc<dyn Fn(Call, Value) -> BoxFuture<Result<String, Error>> + Send + Sync>;

/// One question or act an agent can ask of the plugin. Make it with [`Tool::new`].
#[derive(Clone)]
pub struct Tool {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) effect: Effect,
    pub(crate) schema: Value,
    pub(crate) run: ToolRun,
}

impl Tool {
    /// The tool `name`, answering with what `run` returns. `In` is the tool's input: its JSON Schema, from its `schemars`
    /// derive, is what the agent reads.
    pub fn new<In, F, Fut>(name: &str, description: &str, effect: Effect, run: F) -> Tool
    where
        In: DeserializeOwned + JsonSchema + Send + 'static,
        F: Fn(Call, In) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, Error>> + Send + 'static,
    {
        let run = Arc::new(run);
        let tool = name.to_owned();
        Tool {
            name: name.to_owned(),
            description: description.to_owned(),
            effect,
            schema: schema_of::<In>(),
            run: Arc::new(move |call, arguments| {
                let (run, tool) = (Arc::clone(&run), tool.clone());
                Box::pin(async move {
                    let input: In = decode_object(arguments).map_err(|e| {
                        Failure::new(
                            ErrorCode::InvalidArgument,
                            format!("{tool}: the arguments are not the tool's input: {e}"),
                        )
                    })?;
                    run(call, input).await
                })
            }),
        }
    }
}

/// What a poll is asked: the owner's query, the cursor of the last poll (empty on the first) and the most items to answer.
#[derive(Clone, Debug, Default)]
pub struct PollQuery {
    pub query: String,
    pub cursor: String,
    pub limit: i64,
}

type PollRun = Arc<dyn Fn(Call, PollQuery) -> BoxFuture<Result<PollAnswer, Error>> + Send + Sync>;

/// The poll that makes a plugin a source of events. On the first poll (empty cursor) it answers no items and the cursor that
/// marks now, so a new signal never replays history.
#[derive(Clone)]
pub struct Poll(PollRun);

impl Poll {
    pub fn new<F, Fut>(run: F) -> Poll
    where
        F: Fn(Call, PollQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<PollAnswer, Error>> + Send + 'static,
    {
        Poll(Arc::new(move |call, q| Box::pin(run(call, q))))
    }
}

type AccountRun = Arc<dyn Fn(Call) -> BoxFuture<Result<Vars, Error>> + Send + Sync>;

/// How a plugin answers the environment a Claude Code session runs with on the account one of its connections is (ADR-0071):
/// variables of Claude Code's that say where its requests go and what authenticates them. It is asked at every session's
/// start, so a key the owner rotated or a short-lived token the plugin mints reaches the next session; Remit stores nothing
/// of the answer and refuses one that is empty or holds a name outside its list.
#[derive(Clone)]
pub struct Account(AccountRun);

impl Account {
    pub fn new<F, Fut>(run: F) -> Account
    where
        F: Fn(Call) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vars, Error>> + Send + 'static,
    {
        Account(Arc::new(move |call| Box::pin(run(call))))
    }
}

/// Everything a plugin is, as data.
#[derive(Clone, Default)]
pub struct Definition {
    /// The name the owner reads.
    pub label: String,
    /// Which capability of the catalog a chain holds to get this plugin's tools.
    pub capability: String,
    pub vars: Vec<Var>,
    pub tools: Vec<Tool>,
    /// When set, makes the plugin a source of events.
    pub poll: Option<Poll>,
    /// What the plugin ships besides tools. A signal with a poll is filled by `poll`, so it needs one.
    pub signals: Vec<Signal>,
    pub agents: Vec<Agent>,
    /// What the plugin adds to the signal pipeline (ADR-0049), and the subscriptions it ships.
    pub sensor_kinds: Vec<SensorKind>,
    pub processors: Vec<Processor>,
    pub subscriptions: Vec<Subscription>,
    pub ui: Option<Ui>,
    /// When set, makes the plugin an API behind the injecting proxy too.
    pub auth: Option<Auth>,
    /// When set, makes each connection of the plugin a Claude account agents run on (ADR-0071).
    pub account: Option<Account>,
    /// The files of the frontend, by name (`include_bytes!` them into the program).
    pub assets: BTreeMap<String, Vec<u8>>,
    /// The forge Remit granted, set through [`Plugin::grant_forge`]; an author leaves it `None`.
    pub forge: Option<Arc<dyn Forge>>,
}

/// Why a plugin is not one Remit accepts.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

/// The name of a signal, an agent, a sensor kind, a processor or a subscription: the shape of an event signal's name.
fn shipped_name(s: &str) -> bool {
    let lower = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    (1..=40).contains(&s.len()) && s.bytes().next().is_some_and(lower) && s.bytes().all(|c| lower(c) || c == b'-')
}

macro_rules! invalid {
    ($($t:tt)*) => { return Err(Invalid(format!($($t)*))) };
}

impl Definition {
    /// Whether the plugin is one Remit accepts: names that are words, nothing declared twice, what Remit needs to make what it
    /// ships.
    pub fn validate(&self) -> Result<(), Invalid> {
        if self.label.is_empty() || self.capability.is_empty() {
            invalid!("a plugin has a label and a capability");
        }
        let mut seen = HashSet::new();
        for t in &self.tools {
            if !is_tool_name(&t.name) {
                invalid!(
                    "tool {:?} is not a lower case word of 2 to 48 letters, digits and underscores",
                    t.name
                );
            }
            if !seen.insert(t.name.clone()) {
                invalid!("tool {:?} is declared twice", t.name);
            }
        }
        for v in &self.vars {
            if !is_var_name(&v.name) {
                invalid!(
                    "variable {:?} is not a credential name: upper case words that do not move the loader or the path",
                    v.name
                );
            }
        }
        self.validate_shipments()?;
        self.validate_pipeline()?;
        if let Some(ui) = &self.ui
            && !self.assets.contains_key(&ui.entry)
        {
            invalid!("ui entry {} is not in assets", ui.entry);
        }
        Ok(())
    }

    fn validate_shipments(&self) -> Result<(), Invalid> {
        let polls = |s: &Signal| !s.poll_query.is_empty() || s.poll_every_minutes != 0;
        if self.poll.is_none() && self.signals.iter().any(polls) {
            invalid!("a plugin that ships a signal with a poll has a poll to fill it");
        }
        let mut seen = HashSet::new();
        for s in &self.signals {
            if !shipped_name(&s.name) {
                invalid!(
                    "signal {:?} is not lower case letters, digits and dashes, at most 40",
                    s.name
                );
            }
            if !seen.insert(format!("signal:{}", s.name)) {
                invalid!("signal {:?} is declared twice", s.name);
            }
            if s.note.is_empty() {
                invalid!("signal {} has a note: say what it is for", s.name);
            }
            if polls(s) && !(1..=10080).contains(&s.poll_every_minutes) {
                invalid!(
                    "signal {} polls every {} minutes: from 1 to 10080 (a week)",
                    s.name,
                    s.poll_every_minutes
                );
            }
        }
        for a in &self.agents {
            if !shipped_name(&a.id) {
                invalid!(
                    "agent {:?} is not lower case letters, digits and dashes, at most 40",
                    a.id
                );
            }
            if !seen.insert(format!("agent:{}", a.id)) {
                invalid!("agent {:?} is declared twice", a.id);
            }
            if a.name.is_empty() || a.description.is_empty() || a.instructions.is_empty() {
                invalid!("agent {} has a name, a description and instructions", a.id);
            }
        }
        Ok(())
    }

    fn validate_pipeline(&self) -> Result<(), Invalid> {
        let mut seen = HashSet::new();
        for k in &self.sensor_kinds {
            if !shipped_name(&k.name) {
                invalid!(
                    "sensor kind {:?} is not lower case letters, digits and dashes, at most 40",
                    k.name
                );
            }
            if !seen.insert(format!("kind:{}", k.name)) {
                invalid!("sensor kind {:?} is declared twice", k.name);
            }
        }
        for p in &self.processors {
            if !shipped_name(&p.name) {
                invalid!(
                    "processor {:?} is not lower case letters, digits and dashes, at most 40",
                    p.name
                );
            }
            if !seen.insert(format!("processor:{}", p.name)) {
                invalid!("processor {:?} is declared twice", p.name);
            }
        }
        for s in &self.subscriptions {
            let (processor, action) = (!s.processor.is_empty(), !s.action.is_empty());
            if !shipped_name(&s.name) {
                invalid!(
                    "subscription {:?} is not lower case letters, digits and dashes, at most 40",
                    s.name
                );
            }
            if seen.contains(&format!("subscription:{}", s.name)) {
                invalid!("subscription {:?} is declared twice", s.name);
            }
            if s.note.is_empty() {
                invalid!("subscription {} has a note: say what it is for", s.name);
            }
            if s.r#match.is_empty() {
                invalid!(
                    "subscription {} has a match: the pattern of the keys it listens to",
                    s.name
                );
            }
            match (processor, action) {
                (true, true) => invalid!(
                    "subscription {} names a processor and an action: one or the other, not both",
                    s.name
                ),
                (false, false) => invalid!("subscription {} does nothing: name a processor or an action", s.name),
                _ => {}
            }
            if processor && (!s.job.is_empty() || !s.with.is_empty()) {
                invalid!(
                    "subscription {} gives a job or with to a processor: they are for an action (its parameters are params)",
                    s.name
                );
            }
            if processor && !seen.contains(&format!("processor:{}", s.processor)) {
                invalid!(
                    "subscription {} names the processor {:?}, which this plugin does not have",
                    s.name,
                    s.processor
                );
            }
            if action && s.action != "tell" && s.action != "run" {
                invalid!(
                    "subscription {} names the action {:?}: a plugin ships tell or run",
                    s.name,
                    s.action
                );
            }
            if s.action == "run" && s.job.is_empty() {
                invalid!("subscription {} runs no job: name the job", s.name);
            }
            if s.action == "tell" && !s.job.is_empty() {
                invalid!("subscription {} tells and names a job: a job is for run", s.name);
            }
            if s.params.as_ref().is_some_and(|p| !p.0.is_object()) {
                invalid!("subscription {}: the parameters are a JSON object", s.name);
            }
            seen.insert(format!("subscription:{}", s.name));
        }
        Ok(())
    }

    /// What the plugin says it is, read without calling it.
    pub fn manifest(&self) -> Manifest {
        Manifest {
            label: self.label.clone(),
            capability: self.capability.clone(),
            vars: self.vars.clone(),
            tools: self
                .tools
                .iter()
                .map(|t| wire::plugin::Tool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.schema.clone().into(),
                    effect: t.effect,
                })
                .collect(),
            poll: self.poll.is_some(),
            accounts: self.account.is_some(),
            ui: self.ui.clone(),
            signals: self.signals.clone(),
            agents: self.agents.clone(),
            sensor_kinds: self
                .sensor_kinds
                .iter()
                .map(|k| wire::plugin::SensorKind {
                    name: k.name.clone(),
                    label: k.label.clone(),
                    settings_schema: k.schema.clone().into(),
                })
                .collect(),
            processors: self
                .processors
                .iter()
                .map(|p| wire::plugin::Processor {
                    name: p.name.clone(),
                    label: p.label.clone(),
                    params_schema: p.schema.clone().into(),
                    effect: p.effect,
                })
                .collect(),
            subscriptions: self.subscriptions.clone(),
            auth: self.auth.clone(),
        }
    }

    fn call_of(&self, vars: Vars) -> Call {
        Call {
            vars,
            forge: self.forge.clone(),
        }
    }
}

#[async_trait]
impl Plugin for Definition {
    fn grant_forge(&mut self, forge: Arc<dyn Forge>) {
        self.forge = Some(forge);
    }

    async fn describe(&self) -> Result<Manifest, Failure> {
        self.validate()
            .map_err(|e| Failure::new(ErrorCode::InvalidArgument, e.0))?;
        Ok(self.manifest())
    }

    async fn call(&self, _: Request, tool: String, arguments: Value, vars: Vars) -> Result<String, Failure> {
        let found = self
            .tools
            .iter()
            .find(|t| t.name == tool)
            .ok_or_else(|| not_found(format!("no tool {tool:?}")))?;
        (found.run)(self.call_of(vars), arguments).await.map_err(failure_of)
    }

    async fn poll(&self, ask: PollAsk) -> Result<PollAnswer, Failure> {
        let poll = self
            .poll
            .as_ref()
            .ok_or_else(|| not_found("this plugin is not a source of events".to_owned()))?;
        let query = PollQuery {
            query: ask.query,
            cursor: ask.cursor,
            limit: ask.limit,
        };
        (poll.0)(self.call_of(ask.vars), query).await.map_err(failure_of)
    }

    async fn sense(&self, open: SenseOpen) -> Result<Senses, Failure> {
        let kind = self
            .sensor_kinds
            .iter()
            .find(|k| k.name == open.kind)
            .ok_or_else(|| not_found(format!("no sensor kind {:?}", open.kind)))?;
        let (tx, rx) = mpsc::unbounded_channel();
        let call = self.call_of(open.vars.clone());
        let run = (kind.run)(call, open, Emitter { tx: tx.clone() })?;
        let task = tokio::spawn(async move {
            if let Err(err) = run.await {
                tx.send(Err(failure_of(err))).ok();
            }
        });
        Ok(Senses::new(rx, move || task.abort()))
    }

    async fn process(&self, firing: Firing) -> Result<ProcessAnswer, Failure> {
        let processor = self
            .processors
            .iter()
            .find(|p| p.name == firing.processor)
            .ok_or_else(|| not_found(format!("no processor {:?}", firing.processor)))?;
        let call = self.call_of(firing.vars.clone());
        (processor.run)(call, firing).await.map_err(failure_of)
    }

    async fn asset(&self, name: String) -> Result<Asset, Failure> {
        let missing = || not_found(format!("no asset {name:?}"));
        let parts: Vec<&str> = name.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
        if name.starts_with('/') || parts.contains(&"..") {
            return Err(missing());
        }
        let clean = parts.join("/");
        let content = self.assets.get(&clean).ok_or_else(missing)?.clone();
        Ok(Asset {
            content,
            content_type: content_type(&clean).to_owned(),
        })
    }

    async fn account(&self, vars: Vars) -> Result<Vars, Failure> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| not_found("this plugin provides no accounts".to_owned()))?;
        (account.0)(self.call_of(vars)).await.map_err(failure_of)
    }
}

/// The content type of a frontend's asset by its extension; empty when unknown.
fn content_type(name: &str) -> &'static str {
    match name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("wasm") => "application/wasm",
        Some("pdf") => "application/pdf",
        Some("xml") => "text/xml; charset=utf-8",
        _ => "",
    }
}
