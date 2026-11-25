//! What a plugin adds to the signal pipeline (ADR-0049): a kind of sensor that writes readings of a signal, and a processor that a
//! subscription's action names. Remit keeps the log, the router and the rails; these are the two places a plugin plugs in.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use link::{ErrorCode, Failure};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;
use wire::plugin::{Effect, Firing, ProcessAnswer, Reading, SenseOpen};

use crate::definition::{Call, Error};
use crate::pipe::BoxFuture;
use crate::plugin::Sensed;

/// What a sensor kind is run with: the sensor's name and settings, and what Remit kept of its last run: the cursor it last gave
/// [`Emitter::cursor`] (empty on the first run) and when it last wrote a reading (`None` on the first run).
#[derive(Clone, Debug)]
pub struct Sensing<Settings> {
    pub name: String,
    pub settings: Settings,
    pub cursor: String,
    pub last_at: Option<SystemTime>,
}

/// Where a sense handler writes what it senses. A write that fails means the sensor was ended: return its error.
pub struct Emitter {
    pub(crate) tx: mpsc::UnboundedSender<Result<Sensed, Failure>>,
}

impl Emitter {
    /// Writes `r` to the log: a retry with the same dedupe is the reading stored already.
    pub async fn reading(&self, r: Reading) -> Result<(), Error> {
        self.send(Sensed::Reading(r))
    }

    /// Makes `c` the cursor Remit keeps for the sensor and hands back when it opens the sensor again.
    pub async fn cursor(&self, c: &str) -> Result<(), Error> {
        self.send(Sensed::Cursor(c.to_owned()))
    }

    /// Resolves when Remit has ended the sensor: a sensor that senses until it is stopped awaits this (its future is dropped
    /// then, so what follows the await does not run).
    pub async fn closed(&self) {
        self.tx.closed().await;
    }

    fn send(&self, sensed: Sensed) -> Result<(), Error> {
        self.tx
            .send(Ok(sensed))
            .map_err(|_| Error::from("the sensor has ended"))
    }
}

type SenseRun = Arc<dyn Fn(Call, SenseOpen, Emitter) -> Result<BoxFuture<Result<(), Error>>, Failure> + Send + Sync>;

/// A kind of sensor the plugin adds. Make it with [`SensorKind::new`].
#[derive(Clone)]
pub struct SensorKind {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) schema: Value,
    pub(crate) run: SenseRun,
}

impl SensorKind {
    /// The kind `name`. `run` senses for one sensor: it writes readings through its emitter until Remit ends the sensor (the
    /// future is dropped then); a run that returns before that has stopped sensing, and Remit opens the sensor again from its
    /// cursor (an error it returns is shown to the owner on the sensor). `Settings` is the sensor's settings, its schema from
    /// `schemars`.
    pub fn new<Settings, F, Fut>(name: &str, label: &str, run: F) -> SensorKind
    where
        Settings: DeserializeOwned + JsonSchema + Send + 'static,
        F: Fn(Call, Sensing<Settings>, Emitter) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Error>> + Send + 'static,
    {
        let run = Arc::new(run);
        let kind = name.to_owned();
        SensorKind {
            name: name.to_owned(),
            label: label.to_owned(),
            schema: schema_of::<Settings>(),
            run: Arc::new(move |call, open, out| {
                let settings: Settings = decode_object(open.settings.0).map_err(|e| {
                    Failure::new(
                        ErrorCode::InvalidArgument,
                        format!("{kind}: the settings are not the sensor's settings: {e}"),
                    )
                })?;
                let sensing = Sensing {
                    name: open.name,
                    settings,
                    cursor: open.cursor,
                    last_at: instant(open.last_at_ms),
                };
                Ok(Box::pin(run(call, sensing, out)))
            }),
        }
    }
}

/// The instant `ms` milliseconds after the epoch; none for 0, which says there was none.
fn instant(ms: i64) -> Option<SystemTime> {
    u64::try_from(ms)
        .ok()
        .filter(|ms| *ms > 0)
        .and_then(|ms| UNIX_EPOCH.checked_add(Duration::from_millis(ms)))
}

/// A reading as a processor reads it: `rev` is its revision among the readings of its key, `seq` its place in the log; `cause_kind`
/// and `cause_who` say who made it, `chain_id` the chain whose agent did, when one did.
#[derive(Clone, Debug)]
pub struct ProcessedReading {
    pub key: String,
    pub rev: i64,
    pub seq: i64,
    pub at: SystemTime,
    pub value: Value,
    pub cause_kind: String,
    pub cause_who: String,
    pub chain_id: String,
}

/// One firing handed to a processor: the reading that fired it and the parameters its subscription gave. `request_id` names the
/// firing and is the same on every delivery of it: key what the processor does outside Remit by it, so a firing delivered again
/// does not do it twice.
#[derive(Clone, Debug)]
pub struct ProcessInput<Params> {
    pub processor: String,
    pub request_id: String,
    pub reading: Option<ProcessedReading>,
    pub params: Params,
}

type ProcessRun = Arc<dyn Fn(Call, Firing) -> BoxFuture<Result<ProcessAnswer, Error>> + Send + Sync>;

/// Something a firing can do that the plugin adds. Make it with [`Processor::new`].
#[derive(Clone)]
pub struct Processor {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) schema: Value,
    pub(crate) effect: Effect,
    pub(crate) run: ProcessRun,
}

impl Processor {
    /// The processor `name`, which a subscription's action names as `plugin:<id>.<name>`. `Params` is the parameters the
    /// subscription gives it, its schema from `schemars`. `run` is called once for each firing, and again for a firing whose
    /// delivery failed or was cut by a restart: it acts at most once for the same request id.
    pub fn new<Params, F, Fut>(name: &str, label: &str, run: F) -> Processor
    where
        Params: DeserializeOwned + JsonSchema + Send + 'static,
        F: Fn(Call, ProcessInput<Params>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ProcessAnswer, Error>> + Send + 'static,
    {
        let run = Arc::new(run);
        let processor = name.to_owned();
        Processor {
            name: name.to_owned(),
            label: label.to_owned(),
            schema: schema_of::<Params>(),
            effect: Effect::Read,
            run: Arc::new(move |call, firing| {
                let (run, processor) = (Arc::clone(&run), processor.clone());
                Box::pin(async move {
                    let params: Params = decode_object(firing.params.0).map_err(|e| {
                        Failure::new(
                            ErrorCode::InvalidArgument,
                            format!("{processor}: the parameters are not the processor's: {e}"),
                        )
                    })?;
                    let reading = firing.reading.map(|r| ProcessedReading {
                        key: r.key,
                        rev: r.rev,
                        seq: r.seq,
                        at: instant(r.at_ms).unwrap_or(UNIX_EPOCH),
                        value: r.value.0,
                        cause_kind: r.cause_kind,
                        cause_who: r.cause_who,
                        chain_id: r.chain_id,
                    });
                    let input = ProcessInput {
                        processor: firing.processor,
                        request_id: firing.request_id,
                        reading,
                        params,
                    };
                    run(call, input).await
                })
            }),
        }
    }

    /// What the processor does to the connection Remit lends it (read by default). The owner's grants and the gate decide on it as
    /// they do for a tool of that effect.
    #[must_use]
    pub fn effect(mut self, effect: Effect) -> Processor {
        self.effect = effect;
        self
    }
}

/// The JSON Schema document of `T` in the shape the agents and the owner read: no meta-schema or title, subschemas inline, an
/// object closed to properties it does not name.
pub(crate) fn schema_of<T: JsonSchema>() -> Value {
    let generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|s| {
            s.meta_schema = None;
            s.inline_subschemas = true;
        })
        .into_generator();
    let mut schema = generator.into_root_schema_for::<T>().to_value();
    if let Some(obj) = schema.as_object_mut() {
        obj.remove("title");
        if obj.get("type").and_then(Value::as_str) == Some("object") {
            obj.entry("additionalProperties").or_insert(Value::Bool(false));
        }
    }
    schema
}

/// Reads a JSON object into `T`; null is an object with no fields. A value that is not an object is an error.
pub(crate) fn decode_object<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    let value = match value {
        Value::Null => Value::Object(serde_json::Map::new()),
        Value::Object(_) => value,
        _ => return Err("not a JSON object".to_owned()),
    };
    serde_json::from_value(value).map_err(|e| e.to_string())
}
