//! What a plugin adds to the signal pipeline (docs/adr/0049) are the sensor kinds it describes, named `plugin:<id>.<name>`
//! in a sensor record's kind, and the processors it describes, named the same in a subscription's action. The enabled
//! plugins' are found beside the in-process ones (`plugin_sensing`, `plugin_processing`), and nothing else asks whose a
//! kind is. A disabled plugin has none: its sensors are held silent with that reason, and a firing that names its
//! processor is not processed.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use grant::Effect;
use model::{
    ActionKind, Cause, ForeignJson, Sensor, SignalKey, SubscriptionAction, Writer, WriterKind, by_remit, plugin_name,
};
use plugin_sdk::{Firing, Plugin, ProcessAnswer, SenseOpen, Sensed, Vars};
use source::{Caller, Extension, ExtensionSensorKind, PROCESS_CALL, PluginRef, SENSE_CALL};
use wire::plugin::ProcessedReading;

use crate::Pipeline;
use crate::event::Emission;
use crate::processing::Delivery;
use crate::sensor::{reads_as_sensor, sensor_writer};
use chains::ingest::PROCESSOR_AGENT;
use chains::ingest::SENSOR_AGENT;
use chains::ingest::Write;
use chains::tools::{NoInput, Tools, as_json};
use chains::{Error, Result};
use reach::grants::connection_of_settings;

/// How long one plugin processor's firing may take: its deadline. It runs on its subscription's delivery worker
/// (deliver.rs), so while it works that subscription's later firings wait behind it, and no other subscription's. Thirty
/// seconds covers starting the plugin cold (the host gives a start 15 s) and one call to an outside service.
pub(crate) const PROCESSOR_TIMEOUT: Duration = Duration::from_secs(30);

/// The most characters a key suffix has: it may name a repository (`group/app.v2`).
const KEY_SUFFIX_MAX: usize = 96;

/// One sensor kind of an enabled plugin, as the poller runs it.
#[derive(Debug, Clone)]
pub(crate) struct PluginSensing {
    plugin: PluginRef,
    pub kind: ExtensionSensorKind,
    /// The plugin has credential variables: it is lent its connection's, through the gate.
    lends: bool,
}

/// One processor of an enabled plugin, as a delivery runs it.
#[derive(Debug, Clone)]
pub(crate) struct PluginProcessing {
    plugin: PluginRef,
    name: String,
    /// What it does, in the plugin's words, and the JSON Schema of the params it takes (empty: any).
    label: String,
    params_schema: String,
    effect: Effect,
    lends: bool,
}

/// What the enabled plugins add, by the name a record carries.
#[derive(Debug, Default)]
pub struct PluginPipeline {
    sensing: HashMap<String, PluginSensing>,
    processing: HashMap<String, PluginProcessing>,
}

/// Where the program of ext, kept under root, is on this server. The manifest passed `validate` when it was applied, so
/// its program stays under root.
fn plugin_ref(root: &Path, ext: &Extension) -> PluginRef {
    if ext.bundled {
        return PluginRef {
            id: ext.id.clone(),
            bundled: true,
            ..PluginRef::default()
        };
    }
    PluginRef {
        id: ext.id.clone(),
        binary: root.join(&ext.program),
        sha256: ext.sha256.clone(),
        bundled: false,
    }
}

/// The pipeline the enabled extensions exts, kept under root, add.
fn pipeline_of(root: &Path, exts: &[Extension]) -> PluginPipeline {
    let mut p = PluginPipeline::default();
    for ext in exts {
        let (plugin, lends) = (plugin_ref(root, ext), !ext.vars.is_empty());
        for k in &ext.sensor_kinds {
            let s = PluginSensing {
                plugin: plugin.clone(),
                kind: k.clone(),
                lends,
            };
            p.sensing.insert(plugin_name(&ext.id, &k.name), s);
        }
        for pr in &ext.processors {
            let effect = ext.lend_effect(&pr.name);
            let s = PluginProcessing {
                plugin: plugin.clone(),
                name: pr.name.clone(),
                label: pr.label.clone(),
                params_schema: pr.params_schema.clone(),
                effect,
                lends,
            };
            p.processing.insert(plugin_name(&ext.id, &pr.name), s);
        }
    }
    p
}

/// Whether suffix is a word a plugin's reading may add to its sensor's signal: letters, digits, dots, slashes, dashes and
/// underscores, starting with a letter or a digit, at most `KEY_SUFFIX_MAX` long.
fn is_key_suffix(suffix: &str) -> bool {
    let mut chars = suffix.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && suffix.len() <= KEY_SUFFIX_MAX
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
}

/// The key a plugin's reading is written under: its sensor's signal, then the word it names, so a sensor writes under its
/// own signal and no other.
fn plugin_key(signal: &SignalKey, suffix: &str) -> Result<SignalKey> {
    if suffix.is_empty() {
        return Ok(signal.clone());
    }
    if !is_key_suffix(suffix) {
        return Err(Error::refused(format!(
            "the key suffix {:?} is not a word of letters, digits, dots, slashes, dashes and underscores",
            source::limit(suffix, 40)
        )));
    }
    Ok(SignalKey(format!("{}.{suffix}", signal.0)))
}

/// The JSON object text holds, as a plugin is given it: null for none, so the plugin reads its defaults.
fn json_of(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or(serde_json::Value::Null)
}

/// The reading a firing covers as the plugin reads it; an expiry covers none, and the plugin is given its key.
fn processed_reading(d: &Delivery) -> ProcessedReading {
    let Some(r) = d.readings.first() else {
        return ProcessedReading {
            key: d.firing.key.0.clone(),
            rev: 0,
            seq: 0,
            at_ms: 0,
            value: model::ForeignJson::default(),
            cause_kind: String::new(),
            cause_who: String::new(),
            chain_id: String::new(),
        };
    };
    ProcessedReading {
        key: r.key.0.clone(),
        rev: r.rev,
        seq: r.seq,
        at_ms: i64::try_from(r.at.0.unix_timestamp_nanos() / 1_000_000).unwrap_or_default(),
        value: r.value.as_ref().map_or(serde_json::Value::Null, |v| v.0.clone()).into(),
        cause_kind: r.cause.actor.kind.as_str().to_owned(),
        cause_who: r.cause.actor.who.clone(),
        chain_id: r.cause.chain_id.clone(),
    }
}

impl Pipeline {
    /// Makes the sensor kinds and processors of exts, the enabled extensions kept under root, the plugins' in the
    /// pipeline, in place of the ones it had.
    pub fn use_pipeline(&self, root: &Path, exts: &[Extension]) {
        let next = Arc::new(pipeline_of(root, exts));
        *self.signals.pipeline.lock().unwrap_or_else(PoisonError::into_inner) = next;
    }

    fn pipeline(&self) -> Arc<PluginPipeline> {
        Arc::clone(&self.signals.pipeline.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// The sensor kind an enabled plugin adds under name; none when no enabled plugin does.
    pub(crate) fn plugin_sensing(&self, name: &str) -> Option<PluginSensing> {
        self.pipeline().sensing.get(name).cloned()
    }

    /// The processor an enabled plugin adds under name; none when no enabled plugin does.
    pub(crate) fn plugin_processing(&self, name: &str) -> Option<PluginProcessing> {
        self.pipeline().processing.get(name).cloned()
    }

    /// The kinds the enabled plugins add, by name, with what each is and the schema of the settings it takes.
    pub(crate) fn sensor_kinds(&self) -> Vec<SensorKindRecord> {
        let mut out: Vec<SensorKindRecord> = self
            .pipeline()
            .sensing
            .iter()
            .map(|(name, s)| SensorKindRecord {
                kind: name.clone(),
                label: s.kind.label.clone(),
                settings_schema: serde_json::from_str(&s.kind.settings_schema).ok(),
            })
            .collect();
        out.sort_by(|a, b| a.kind.cmp(&b.kind));
        out
    }

    /// The processors the enabled plugins add, by name, with what each does and the schema of the params it takes.
    fn processors(&self) -> Vec<ProcessorRecord> {
        let mut out: Vec<ProcessorRecord> = self
            .pipeline()
            .processing
            .iter()
            .map(|(name, p)| ProcessorRecord {
                action: name.clone(),
                label: p.label.clone(),
                params_schema: serde_json::from_str(&p.params_schema).ok(),
            })
            .collect();
        out.sort_by(|a, b| a.action.cmp(&b.action));
        out
    }

    /// Refuses action when it is a plugin's processor that no enabled plugin has, or its params do not fit the schema the
    /// processor declares: the Assistant hears at once why, not on the first firing.
    pub(crate) fn check_processor(&self, action: &SubscriptionAction) -> Result<()> {
        let ActionKind::Plugin(p) = &action.kind else {
            return Ok(());
        };
        let Some(pr) = self.plugin_processing(p.as_str()) else {
            return Err(Error::refused(format!(
                "no enabled plugin has the processor {:?}: list_processors shows the ones there are",
                p.as_str()
            )));
        };
        let params = if action.params.is_empty() {
            None
        } else {
            Some(ForeignJson(serde_json::from_str(&action.params)?))
        };
        settings_fit(&pr.params_schema, params.as_ref()).map_err(|e| Error::refused(format!("params: {e}")))
    }

    /// Gives list_processors: the Assistant reads which processors the enabled plugins add and what params each takes,
    /// then a subscription names one as its action.
    pub(crate) fn add_processor_tool(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_processors", chains::tools::Effect::Reads,
            "The processors the enabled plugins add: each with its name (plugin:<id>.<name>, the `action` of create_subscription), what it does, and the schema of the params it takes. A subscription runs one on each reading it holds for, an event or a fact (fact:turn.ended gives it a card's reply), and the processor may write events back.",
            |o, _: NoInput| async move { as_json(&o.processors()) },
        );
    }

    /// Gives list_sensor_kinds: the Assistant reads which kinds are installed and what settings each takes, then
    /// create_sensor names the kind and gives the settings.
    pub(crate) fn add_sensor_kind_tool(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_sensor_kinds", chains::tools::Effect::Reads,
            "The kinds of sensor the enabled plugins add, besides poll: each with its name (plugin:<id>.<name>, the `kind` of create_sensor), what it is, whether its readings are kept, and the schema of the settings it takes. A sensor of a kind reads nothing else than what its plugin does.",
            |o, _: NoInput| async move { as_json(&o.sensor_kinds()) },
        );
    }

    /// The program of a plugin and the variables of the connection it works through: lent through the gate when the
    /// plugin has credential variables, the program as it is when it has none.
    async fn lend(
        &self,
        lends: bool,
        plugin: &PluginRef,
        caller: &Caller,
        connection: &str,
        call: &str,
        effect: Effect,
    ) -> Result<(PluginRef, Vars)> {
        if !lends {
            return Ok((plugin.clone(), Vars::new()));
        }
        self.sources
            .lend(caller, &plugin.id, connection, call, effect)
            .await
            .map_err(|e| Error::refused(e.to_string()))
    }

    /// The running plugin of p: a bundled one, or the program the owner applied, started on its first call.
    fn plugin_of(&self, p: &PluginRef) -> Result<Arc<dyn Plugin>> {
        if p.bundled {
            return self
                .bundled
                .lookup(&p.id)
                .ok_or_else(|| Error::refused(format!("no plugin {:?} ships with Remit", p.id)));
        }
        let spec = plugin::Spec {
            id: p.id.clone(),
            binary: p.binary.clone(),
            args: Vec::new(),
            sha256: p.sha256.clone(),
        };
        Ok(self.plugins.plugin(&spec))
    }

    /// Has the plugin of p sense for sensor s: its stream, opened from the sensor's cursor and read until the plugin or
    /// the sensor ends it. A stream that ended is opened again on a later period. A failure counts toward the sensor's
    /// silence.
    pub(crate) async fn sense_plugin(&self, p: &PluginSensing, mut s: Sensor) -> Result<()> {
        let sensed = self.open_sense(p, &s).await;
        let mut senses = match sensed {
            Ok(senses) => senses,
            Err(err) => {
                self.signals.sensors.result(&s.id, Some(&err.to_string()));
                return Err(err);
            }
        };
        while let Some(item) = senses.next().await {
            let item = match item {
                Ok(item) => item,
                Err(f) => {
                    self.signals.sensors.result(&s.id, Some(&f.to_string()));
                    return Err(Error::refused(f.to_string()));
                }
            };
            if !self.sensed_still(&s).await? {
                return Ok(()); // the sensor was changed, paused or deleted meanwhile: its stream ends here
            }
            match item {
                Sensed::Reading(r) => {
                    if let Err(err) = self.write_plugin_reading(&p.plugin.id, &s, r).await {
                        self.signals.sensors.result(&s.id, Some(&err.to_string()));
                        return Err(err);
                    }
                }
                Sensed::Cursor(c) if !c.is_empty() => {
                    self.keep_cursor(&s, c.clone()).await?;
                    s.cursor = c;
                }
                Sensed::Cursor(_) => {}
            }
            self.signals.sensors.result(&s.id, None);
        }
        Ok(())
    }

    /// Opens the plugin's stream for sensor s, lent its connection, from its cursor and the time of its signal's newest
    /// reading.
    async fn open_sense(&self, p: &PluginSensing, s: &Sensor) -> Result<plugin_sdk::Senses> {
        let key = s.signal.clone();
        let last_at_ms = match self.store.read(move |r| r.last_reading(&key)).await {
            Ok(last) => i64::try_from(last.at.0.unix_timestamp_nanos() / 1_000_000).unwrap_or_default(),
            Err(store::Error::NotFound) => 0,
            Err(err) => return Err(err.into()),
        };
        let settings = s.settings.as_ref().map(ForeignJson::text).unwrap_or_default();
        let caller = Caller {
            agent: SENSOR_AGENT.to_owned(),
            ..Caller::default()
        };
        let (plugin, vars) = self
            .lend(
                p.lends,
                &p.plugin,
                &caller,
                &connection_of_settings(&settings),
                SENSE_CALL,
                Effect::Read,
            )
            .await?;
        let open = SenseOpen {
            kind: p.kind.name.clone(),
            name: s.name.clone(),
            settings: json_of(&settings).into(),
            vars,
            cursor: s.cursor.clone(),
            last_at_ms,
        };
        self.plugin_of(&plugin)?
            .sense(open)
            .await
            .map_err(|f| Error::refused(f.to_string()))
    }

    /// Whether sensor s is still there, awake and the one the stream was opened for.
    async fn sensed_still(&self, s: &Sensor) -> Result<bool> {
        let id = s.id.clone();
        match self.store.read(move |r| r.sensor(&id)).await {
            Ok(cur) => Ok(!cur.paused && reads_as_sensor(&cur, s)),
            Err(store::Error::NotFound) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    /// Ingests one reading of plugin id's, as sensor s's: a retry of one stored (the same dedupe) is the reading stored.
    async fn write_plugin_reading(&self, id: &str, s: &Sensor, r: plugin_sdk::Reading) -> Result<()> {
        let w = Write {
            key: plugin_key(&s.signal, &r.key_suffix)?,
            value: Some(r.value.clone()),
            cause: Cause {
                actor: by_remit(&format!("plugin:{id}")),
                chain_id: String::new(),
            },
            writer: sensor_writer(s),
            dedupes: if r.dedupe.is_empty() {
                Vec::new()
            } else {
                vec![r.dedupe]
            },
            ..Write::default()
        };
        let door = self.door();
        self.store.update(move |tx| door.ingest(tx, w)).await?;
        self.kick(); // the Tick judges it
        Ok(())
    }

    /// Runs processor p of a plugin for the firing d names: the plugin first, then the firing done in one write. A firing
    /// the plugin did and the server did not record done runs the plugin again with the same request id, which the SDK
    /// tells a processor to key what it does by.
    pub(crate) async fn fire_plugin(&self, p: &PluginProcessing, d: &Delivery) -> Result<()> {
        let caller = Caller {
            agent: PROCESSOR_AGENT.to_owned(),
            ..Caller::default()
        };
        let params = &d.sub.action.params;
        let (plugin, vars) = self
            .lend(
                p.lends,
                &p.plugin,
                &caller,
                &connection_of_settings(params),
                PROCESS_CALL,
                p.effect,
            )
            .await?;
        let firing = Firing {
            processor: p.name.clone(),
            request_id: d.request_id.clone(),
            reading: Some(processed_reading(d)),
            params: json_of(params).into(),
            vars,
        };
        let answer = self
            .plugin_of(&plugin)?
            .process(firing)
            .await
            .map_err(|f| Error::refused(f.to_string()))?;
        let refused = self.emit_answered(&plugin.id, &answer).await;
        let text = if refused.is_empty() {
            answer.text
        } else {
            format!("{} ({refused})", answer.text)
        };
        self.done_saying(d, &source::limit(&text, 500)).await
    }

    /// Writes the events a processor of plugin id answered as its own: each names, as its key suffix, an event signal, and
    /// is written there by the plugin, which ingest lets write only the signals it ships. It answers what it refused and
    /// why, for the firing's outcome; empty when every emit was written.
    async fn emit_answered(&self, id: &str, answer: &ProcessAnswer) -> String {
        let mut refused = Vec::new();
        for r in &answer.emits {
            let e = Emission {
                signal: r.key_suffix.clone(),
                writer: Writer {
                    kind: WriterKind::Plugin,
                    id: id.to_owned(),
                },
                body: serde_json::to_vec(&r.value).unwrap_or_default(),
                sender_id: r.dedupe.clone(),
                cause: Cause {
                    actor: by_remit(&format!("plugin:{id}")),
                    chain_id: String::new(),
                },
                ..Emission::default()
            };
            if let Err(err) = self.emit(e).await {
                refused.push(format!("emit to {:?} refused: {err}", source::limit(&r.key_suffix, 40)));
            }
        }
        refused.join("; ")
    }
}

/// Nothing when settings (a JSON object) are valid against schema, a JSON Schema document; a kind that declares none
/// takes any.
pub(crate) fn settings_fit(schema: &str, settings: Option<&ForeignJson>) -> Result<()> {
    if schema.is_empty() {
        return Ok(());
    }
    let doc: serde_json::Value = serde_json::from_str(schema)
        .map_err(|e| Error::refused(format!("the kind's settings schema is not a schema: {e}")))?;
    let validator = jsonschema::validator_for(&doc)
        .map_err(|e| Error::refused(format!("the kind's settings schema cannot be used: {e}")))?;
    let instance: serde_json::Value =
        serde_json::from_value(settings.map_or(serde_json::Value::Null, |j| j.0.clone()))?;
    validator
        .validate(&instance)
        .map_err(|e| Error::refused(format!("the settings do not fit the kind: {e}")))
}

/// A sensor kind as the Assistant reads it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SensorKindRecord {
    pub kind: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_schema: Option<serde_json::Value>,
}

/// A plugin's processor as the Assistant reads it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessorRecord {
    action: String,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params_schema: Option<serde_json::Value>,
}
