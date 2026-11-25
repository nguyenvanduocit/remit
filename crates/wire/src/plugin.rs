//! The contract between Remit and a plugin it runs as a child process (ADR-0072 decision 7), from `sdk/pluginpb/plugin.proto`.
//! The plugin dials over its stdio: its `hello` offers `plugin` (the calls below, which Remit makes) and asks for `forge.read`
//! (the forge's one question, `forge`: the checks on a landed commit), which Remit's `welcome` grants only to a plugin the owner granted it; a plugin that asks
//! without the grant is answered `FORBIDDEN`. Its `bundle` is `PLUGIN_CONTRACT`. A sensor's readings come as pushes on the
//! stream of its sensor id, in order, and its last push is `ended`. JSON documents (schemas, settings, params, bodies, values)
//! travel as JSON, not as text.

use std::collections::BTreeMap;

use model::ForeignJson;
use serde::{Deserialize, Serialize};

use crate::Ack;

/// The contract's number, the plugin's `hello.bundle`. Additive changes keep it; any other change bumps it, and Remit refuses a
/// plugin built against another (`new_bundle`): it is built again.
pub const PLUGIN_CONTRACT: i64 = 3;

/// The credential variables of the connection a call came through, by name.
pub type Vars = BTreeMap<String, String>;

/// What a tool or processor does to the system it reaches, and so the grant an agent needs to call it: the grant ladder's
/// one `Effect`, which travels as [`EffectDef`] says.
pub use grant::Effect;

/// How an [`Effect`] is written in the contract and in a stored manifest: the variant, as serde's remote derive for a type
/// of another crate writes it (`#[serde(with = "EffectDef")]`). The compiler holds its variants to the ladder's.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Effect", rename_all = "snake_case")]
pub enum EffectDef {
    Read,
    Reversible,
    Compensable,
    Irreversible,
}

/// Whether `name` is a tool's: a lower case word of 2 to 48 letters, digits and underscores, starting with a letter.
#[must_use]
pub fn is_tool_name(name: &str) -> bool {
    word(
        name,
        |c| c.is_ascii_lowercase(),
        |c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_',
        2..=48,
    )
}

/// Whether `name` is a credential variable's: upper case words of 2 to 64 letters, digits and underscores that do not move
/// where a program is loaded from or what it runs (a credential never sets `PATH`, `LD_PRELOAD` and the like).
#[must_use]
pub fn is_var_name(name: &str) -> bool {
    let reserved = matches!(
        name,
        "PATH"
            | "HOME"
            | "TMPDIR"
            | "SHELL"
            | "IFS"
            | "ENV"
            | "BASH_ENV"
            | "PYTHONPATH"
            | "PYTHONHOME"
            | "NODE_OPTIONS"
            | "NODE_PATH"
            | "RUBYOPT"
            | "PERL5OPT"
            | "GOFLAGS"
    ) || name.starts_with("LD_")
        || name.starts_with("DYLD_");
    !reserved
        && word(
            name,
            |c| c.is_ascii_uppercase(),
            |c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_',
            2..=64,
        )
}

/// Whether `s` is `len` bytes long, its first byte one `first` takes and every other one `rest` takes.
fn word(s: &str, first: fn(u8) -> bool, rest: fn(u8) -> bool, len: std::ops::RangeInclusive<usize>) -> bool {
    let b = s.as_bytes();
    len.contains(&b.len()) && b.first().is_some_and(|c| first(*c)) && b.iter().skip(1).all(|c| rest(*c))
}

/// One credential variable the plugin reads: `secret` is stored sealed and never shown, `address` says where the system is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Var {
    pub name: String,
    pub what: String,
    pub secret: bool,
    pub optional: bool,
    pub address: bool,
}

/// One question or act an agent can ask of the plugin; `inputSchema` is a JSON Schema document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: ForeignJson,
    #[serde(with = "EffectDef")]
    pub effect: Effect,
}

/// An event signal the plugin ships, `event:<name>`, filled by its poll with `pollQuery` every `pollEveryMinutes` (zero: its
/// events come to its door); `paused` ships it waiting for the owner.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signal {
    pub name: String,
    pub note: String,
    pub poll_query: String,
    pub poll_every_minutes: i32,
    pub paused: bool,
}

/// An agent template the plugin ships; `group` is the kind of work it is for, empty for general.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub group: String,
    pub instructions: String,
}

/// A kind of sensor the plugin adds (ADR-0049); `settingsSchema` is a JSON Schema document of a sensor's settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorKind {
    pub name: String,
    pub label: String,
    pub settings_schema: ForeignJson,
}

/// Something a firing can do that the plugin adds, named by a subscription as `plugin:<id>.<name>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Processor {
    pub name: String,
    pub label: String,
    pub params_schema: ForeignJson,
    #[serde(with = "EffectDef")]
    pub effect: Effect,
}

/// A subscription the plugin ships: exactly one of `processor` (with `params`, a JSON object) and `action` (`tell` or `run`,
/// with `job` and the expressions of `with`) is set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub name: String,
    pub note: String,
    pub r#match: String,
    pub expr: String,
    pub processor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<ForeignJson>,
    pub action: String,
    pub job: String,
    pub with: BTreeMap<String, String>,
    pub paused: bool,
}

/// How the injecting proxy reaches the plugin's API and adds the connection's credential (ADR-0018, form T1).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auth {
    pub base: String,
    pub scheme: String,
    pub header: String,
    pub param: String,
    pub hash: String,
    pub vars: Vec<String>,
    pub read_posts: Vec<String>,
    pub base_var: String,
    pub headers: BTreeMap<String, String>,
}

/// One place in the app the plugin fills with a component its remote exposes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub slot: String,
    pub id: String,
    pub title: String,
    pub expose: String,
}

/// The frontend a plugin brings: the entry of its Module Federation remote, one of its assets, and the slots it fills.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ui {
    pub entry: String,
    pub slots: Vec<Slot>,
}

/// Everything the owner reads before applying a plugin; `poll` makes it a source of events, `accounts` makes each of its
/// connections a Claude account (ADR-0071).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub label: String,
    pub capability: String,
    pub vars: Vec<Var>,
    pub tools: Vec<Tool>,
    pub poll: bool,
    pub accounts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<Ui>,
    pub signals: Vec<Signal>,
    pub agents: Vec<Agent>,
    pub sensor_kinds: Vec<SensorKind>,
    pub processors: Vec<Processor>,
    pub subscriptions: Vec<Subscription>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
}

/// What a poll is asked: the owner's query, the cursor of the last poll (empty on the first) and the most items to answer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollAsk {
    pub query: String,
    pub cursor: String,
    pub limit: i64,
    pub vars: Vars,
}

/// One thing that happened; `body` is a JSON object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollItem {
    pub id: String,
    pub version: String,
    pub body: ForeignJson,
}

/// The items oldest first and the cursor to ask from next; the first poll answers none and the cursor that marks now.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollAnswer {
    pub items: Vec<PollItem>,
    pub cursor: String,
}

/// One sensor to open: its kind and name, its settings (a JSON object), the credential variables it reads through, and what
/// Remit kept of its last run: the cursor the plugin last gave and the instant of its last reading (ms since the epoch, 0 on
/// the first).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SenseOpen {
    pub kind: String,
    pub name: String,
    pub settings: ForeignJson,
    pub vars: Vars,
    pub cursor: String,
    pub last_at_ms: i64,
}

/// One reading a sensor writes, or a processor asks Remit to write. Remit decides the key: `keySuffix`, when not empty, is a
/// word added to the sensor's signal after a dot; `dedupe`, when not empty, makes a retry of the same reading one reading.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub key_suffix: String,
    pub value: ForeignJson,
    pub dedupe: String,
}

/// A reading as a processor reads it: `rev` is its revision of its key, `seq` its place in the log; `causeKind` and
/// `causeWho` say who made it, `chainId` the chain whose agent did, when one did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedReading {
    pub key: String,
    pub rev: i64,
    pub seq: i64,
    pub at_ms: i64,
    pub value: ForeignJson,
    pub cause_kind: String,
    pub cause_who: String,
    pub chain_id: String,
}

/// One firing handed to a processor: `requestId` names it and is the same on every delivery of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Firing {
    pub processor: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading: Option<ProcessedReading>,
    pub params: ForeignJson,
    pub vars: Vars,
}

/// What a processor did, in words, and the readings it asks Remit to write as a result.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAnswer {
    pub text: String,
    pub emits: Vec<Reading>,
}

/// One file of the plugin's frontend: `content` is its bytes in base64; `contentType` is empty when unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub content: String,
    pub content_type: String,
}

link::contract! {
    capability: "plugin";
    reads {
        describe() -> Manifest, why = "reads what the plugin says it is";
        poll(ask: PollAsk) -> PollAnswer, why = "reads the events after a cursor; only the host keeps the cursor it answers";
        asset(name: String) -> Asset, why = "reads one file of the plugin's frontend";
        /// The environment a Claude Code session runs with on the account the connection of `vars` is (ADR-0071).
        account(vars: Vars) -> Vars, why = "answers the environment of an account; asked again, it answers again";
    }
    changes {
        /// Runs one tool; `arguments` is a JSON object valid against its input schema. Remit never sends one again after its
        /// session ended: a tool that ran half way may have acted.
        call(tool: String, arguments: ForeignJson, vars: Vars) -> String, timeout_ms = 600000;
    }
    idempotent {
        process(firing: Firing) -> ProcessAnswer, why = "a firing delivered again acts once by its requestId";
        /// Starts sensing and returns at once: what it senses comes as pushes on the stream of `sensor`, the last `ended`.
        sense_open(sensor: String, open: SenseOpen) -> Ack, why = "a sensor already sensing is not opened again";
        /// Ends a sensor's run; no `ended` follows.
        sense_close(sensor: String) -> Ack, why = "a sensor closed twice is closed";
    }
    pushes {
        reading(sensor: String, reading: Reading);
        /// The cursor Remit keeps for the sensor and hands back when it opens it again.
        cursor(sensor: String, cursor: String);
        /// The run stopped by itself: `error` is why it failed, empty when it did not.
        ended(sensor: String, error: String);
    }
}

/// The one question a plugin granted `forge.read` asks Remit (ADR-0057, ADR-0114): the checks a forge's pipeline ran on a
/// commit Remit landed. The forge's login stays Remit's; nothing here merges, opens or pushes.
pub mod forge {
    use model::ForeignJson;

    link::contract! {
        capability: "forge.read";
        reads {
            /// The checks the pipeline of `project` on `host` ran on commit `sha`: a `model::Checks` as JSON, or JSON null
            /// while the pipeline has run none.
            checks(host: String, project: String, sha: String) -> ForeignJson, why = "reads the checks a forge's pipeline ran on a commit";
        }
        changes {}
        idempotent {}
        pushes {}
    }
}
