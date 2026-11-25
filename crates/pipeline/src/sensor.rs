//! A sensor (`model::Sensor`, ADR-0047) is how Remit reads the world: the only thing that goes out, on a period, and writes
//! readings of a signal. Each Tick has, in the background and never under `tick_mu`, every sensor that is wanted run once,
//! one run at a time for a sensor. A sensor that fails its reads in a row is silent (polls.rs), and the signal's line on
//! the Signals tab says so. Two kinds are Remit's own, by what they read:
//!
//! - a poll reads a source's connection with its query from its cursor on, through the sources' gate as the caller
//!   `SENSOR_AGENT`. Each item the source answers, oldest first and at most `SENSOR_ITEMS_MAX` a run, is an event of its
//!   event signal, written through `emit` with the item's id@version as its sender id, so a run replayed after a crash is
//!   the same events. The cursor the source answers is stored once the items are written; the first run, with no cursor,
//!   only sets it, so a new sensor emits nothing older than itself.
//! - a script runs behind a connection's proxy as the Chair of its chain, so the chain's grants decide whether it may read
//!   there and the script holds no credential. It prints one JSON value, the state as it is now; a value that differs from
//!   the key's newest is its next reading. It runs while an open watch listens to its key and its chain is not paused.
//!
//! A plugin's kind is sensed by its plugin (plugin_pipeline.rs). A sensor of a kind nobody senses (its plugin is not
//! installed, or the owner disabled it) is held silent with the reason, never dropped.

use std::collections::{HashMap, HashSet};

use model::{
    ChainStatus, Control, EVENT_KEY_PREFIX, ForeignJson, KIND_POLL, KIND_SCRIPT, Sensor, SignalKey, Time, Writer,
    WriterKind, by_remit,
};
use source::{Caller, ScriptInput};

use crate::Pipeline;
use crate::event::Emission;
use chains::ingest::SENSOR_AGENT;
use chains::ingest::Write;
use chains::{Error, Result};
use wire::app::SilentSensor;

/// The most items one run of a poll writes as events.
const SENSOR_ITEMS_MAX: usize = 50;
/// The shortest period a sensor reads on.
const SENSOR_FLOOR: time::Duration = time::Duration::MINUTE;

/// How often s reads: its period, never more often than once a minute.
fn sensor_every(s: &Sensor) -> time::Duration {
    time::Duration::minutes(s.every_minutes).max(SENSOR_FLOOR)
}

/// The writer of what sensor s reads.
pub(crate) fn sensor_writer(s: &Sensor) -> Writer {
    Writer {
        kind: WriterKind::Sensor,
        id: s.id.clone(),
    }
}

/// Why a sensor of a kind nobody senses is silent.
fn no_kind(kind: &str) -> String {
    format!("no sensor of kind {kind:?} is installed: its plugin is not, or the owner disabled it")
}

/// Whether a and b read the same: one source's connection, with one query or one script. Where it starts is not a part of
/// what it reads.
pub fn reads_as_sensor(a: &Sensor, b: &Sensor) -> bool {
    a.kind == b.kind
        && a.source == b.source
        && a.connection == b.connection
        && a.query == b.query
        && a.signal == b.signal
        && a.settings == b.settings
        && a.script == b.script
}

/// Whether a and b are the same JSON value, whatever their spacing and the order of their members.
fn same_json(a: &str, b: &str) -> bool {
    match (
        serde_json::from_str::<serde_json::Value>(a),
        serde_json::from_str::<serde_json::Value>(b),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

impl Pipeline {
    /// Has the sensors that are to sense run, each by its kind. Called from the Tick with `tick_mu` held; it only reads the
    /// store and leaves every sensing to the background.
    pub async fn poll_sensors(&self, now: Time) {
        let all = match self.store.read(|r| r.sensors()).await {
            Ok(all) => all,
            Err(err) => return tracing::error!(%err, "sensors: the ones to run"),
        };
        let mut ids = Vec::new();
        for s in all.into_iter().filter(|s| !s.paused) {
            let wanted = match s.kind.as_str() {
                KIND_POLL => self.poll_wanted(&s).await,
                KIND_SCRIPT => self.script_wanted(&s, now).await,
                // a plugin's sensor with a period is read on it; one with none is read when something asks, as the
                // landing sensor is by the job that follows the landings whose pipeline has not settled
                other if self.plugin_sensing(other).is_some() => Ok(s.every_minutes > 0),
                other => {
                    ids.push(s.id.clone());
                    self.signals.sensors.result(&s.id, Some(&no_kind(other)));
                    continue;
                }
            };
            match wanted {
                Ok(true) => {}
                Ok(false) => continue,
                Err(err) => {
                    tracing::error!(sensor = %s.name, %err, "sensors: is it wanted");
                    continue;
                }
            }
            ids.push(s.id.clone());
            if !self.signals.sensors.claim(&s.id, now, sensor_every(&s)) {
                continue;
            }
            let Some(me) = self.me() else { return };
            let started = self.background(move |_| async move {
                if let Err(err) = me.run_sensor(&s.id).await {
                    tracing::warn!(sensor = %s.name, %err, "sensor not read");
                }
            });
            if !started {
                return;
            }
        }
        self.signals.sensors.keep_only(&ids);
    }

    /// Whether poll s is to read: while its event signal is there and runs.
    async fn poll_wanted(&self, s: &Sensor) -> Result<bool> {
        let name = s
            .signal
            .0
            .strip_prefix(EVENT_KEY_PREFIX)
            .unwrap_or(&s.signal.0)
            .to_owned();
        match self.store.read(move |r| r.event_signal_named(&name)).await {
            Ok((es, _)) => Ok(!es.paused),
            Err(store::Error::NotFound) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    /// Whether script s is to run: while an open watch listens to its key and its chain is not paused.
    async fn script_wanted(&self, s: &Sensor, now: Time) -> Result<bool> {
        if !self.watched_keys(now).await?.contains(&s.signal) {
            return Ok(false);
        }
        let id = s.chain.clone();
        Ok(self
            .store
            .read(move |r| r.chain(&id))
            .await
            .is_ok_and(|ch| *ch.status() != ChainStatus::Paused))
    }

    /// The signals an open watch listens to at now.
    async fn watched_keys(&self, now: Time) -> Result<HashSet<SignalKey>> {
        let subs = self.store.read(|r| r.chain_subscriptions()).await?;
        Ok(subs
            .into_iter()
            .filter(|s| s.watch() && s.control == Control::Active && s.until.is_none_or(|until| now < until))
            .map(|s| SignalKey(s.r#match.0))
            .collect())
    }

    /// Has sensor id sense once, as it is now: its settings may have changed since the Tick. A failed read counts toward
    /// the sensor's silence.
    pub async fn run_sensor(&self, id: &str) -> Result<()> {
        let poll = self.signals.sensors.of(id);
        let _one = poll.run.lock().await;
        let key = id.to_owned();
        let s = match self.store.read(move |r| r.sensor(&key)).await {
            Err(store::Error::NotFound) => return Ok(()),
            s => s?,
        };
        if s.paused {
            return Ok(());
        }
        match s.kind.as_str() {
            KIND_POLL => self.poll_sensor(&s).await,
            KIND_SCRIPT => self.script_sensor(&s).await,
            other => match self.plugin_sensing(other) {
                Some(p) => self.sense_plugin(&p, s).await,
                None => Err(Error::refused(no_kind(other))),
            },
        }
    }

    /// Reads the source of poll s once and writes what it answers as events of its signal. A write that fails leaves the
    /// cursor where it was, so the next run answers the same items and writes those that were not written.
    async fn poll_sensor(&self, s: &Sensor) -> Result<()> {
        let name = s
            .signal
            .0
            .strip_prefix(EVENT_KEY_PREFIX)
            .unwrap_or(&s.signal.0)
            .to_owned();
        let named = name.clone();
        let (es, _) = self.store.read(move |r| r.event_signal_named(&named)).await?;
        if es.paused {
            return Ok(());
        }
        let caller = Caller {
            agent: SENSOR_AGENT.to_owned(),
            ..Caller::default()
        };
        let got = self
            .sources
            .poll(&caller, &s.source, &s.connection, &s.query, &s.cursor, SENSOR_ITEMS_MAX)
            .await;
        self.signals
            .sensors
            .result(&s.id, got.as_ref().err().map(ToString::to_string).as_deref());
        let got = got.map_err(|e| Error::refused(e.to_string()))?;
        if !s.cursor.is_empty() {
            // the first run only says where the next one starts
            for it in got.items.iter().take(SENSOR_ITEMS_MAX) {
                let e = Emission {
                    signal: name.clone(),
                    writer: sensor_writer(s),
                    body: it.body.clone().into_bytes(),
                    sender_id: format!("{}@{}", it.id, it.version),
                    cause: model::Cause {
                        actor: by_remit(SENSOR_AGENT),
                        chain_id: String::new(),
                    },
                    ..Emission::default()
                };
                self.emit(e)
                    .await
                    .map_err(|err| Error::refused(format!("item {}@{}: {err}", it.id, it.version)))?;
            }
        }
        self.keep_cursor(s, got.cursor).await
    }

    /// Stores cursor on sensor s, whose read was what s says, unless it reads something else now: the cursor is that
    /// read's. It is the poller's bookkeeping, not a change of the sensor's settings, so it tells no channel.
    pub(crate) async fn keep_cursor(&self, s: &Sensor, cursor: String) -> Result<()> {
        if cursor == s.cursor {
            return Ok(());
        }
        let read = s.clone();
        self.store
            .update(move |tx| -> store::Result<()> {
                let mut cur = match tx.sensor(&read.id) {
                    Err(store::Error::NotFound) => return Ok(()),
                    cur => cur?,
                };
                if !reads_as_sensor(&cur, &read) {
                    return Ok(());
                }
                cur.cursor = cursor;
                tx.put_sensor(cur).map(|_| ())
            })
            .await?;
        Ok(())
    }

    /// Runs the script of s once as the Chair of its chain and stores the state it printed as the next reading of its key.
    async fn script_sensor(&self, s: &Sensor) -> Result<()> {
        let id = s.chain.clone();
        let ch = self.store.read(move |r| r.chain(&id)).await?;
        let value = self.read_script(&ch.id, &ch.chair_id, s).await;
        self.signals
            .sensors
            .result(&s.id, value.as_ref().err().map(ToString::to_string).as_deref());
        self.keep_sensed(s, value?).await
    }

    /// Runs the script of s once as the Chair of chain chain_id and answers the one JSON value it printed.
    pub(crate) async fn read_script(&self, chain_id: &str, chair_id: &str, s: &Sensor) -> Result<String> {
        let Some(script) = &s.script else {
            return Err(Error::refused(format!("sensor {} holds no script", s.id)));
        };
        let caller = Caller {
            chain: chain_id.to_owned(),
            agent: chair_id.to_owned(),
        };
        let input = ScriptInput {
            source: s.source.clone(),
            language: script.language.clone(),
            code: script.code.clone(),
        };
        let out = self
            .sources
            .run_script(&caller, &s.source, &s.connection, input)
            .await
            .map_err(|e| Error::refused(e.to_string()))?;
        let raw = out.trim();
        if serde_json::from_str::<serde_json::Value>(raw).is_err() {
            return Err(Error::refused(format!(
                "the script must print one JSON value (the state to watch), and what it printed is not JSON: {}",
                source::limit(raw, 120)
            )));
        }
        Ok(raw.to_owned())
    }

    /// Ingests value as the next reading of sensor s's signal unless it is the newest one's.
    pub(crate) async fn keep_sensed(&self, s: &Sensor, value: String) -> Result<()> {
        let key = s.signal.clone();
        let last = self.store.read(move |r| r.last_reading(&key)).await;
        match last {
            Err(store::Error::NotFound) => {}
            Err(err) => return Err(Error::refused(format!("the newest reading of {}: {err}", s.signal))),
            Ok(last) if last.value.as_ref().is_some_and(|v| same_json(&v.text(), &value)) => return Ok(()),
            Ok(_) => {}
        }
        let w = Write {
            key: s.signal.clone(),
            cause: model::Cause {
                actor: by_remit(SENSOR_AGENT),
                chain_id: String::new(),
            },
            value: Some(ForeignJson::parse(&value)?),
            writer: sensor_writer(s),
            ..Write::default()
        };
        let door = self.door();
        self.store.update(move |tx| door.ingest(tx, w)).await?;
        self.kick(); // the Tick judges it
        Ok(())
    }

    /// What the Signals tab says of each signal whose sensor has failed its reads in a row, by its key: one sentence for
    /// the signal's line, and the sensors that are silent, so a signal with two sensors names the one.
    pub(crate) async fn sensor_silence(&self) -> (HashMap<SignalKey, String>, HashMap<SignalKey, Vec<SilentSensor>>) {
        let (mut line, mut sensors) = (HashMap::new(), HashMap::<SignalKey, Vec<SilentSensor>>::new());
        let silent = self.signals.sensors.silent();
        if silent.is_empty() {
            return (line, sensors);
        }
        let Ok(all) = self.store.read(|r| r.sensors()).await else {
            return (line, sensors);
        };
        for s in all {
            if let Some(f) = silent.get(&s.id) {
                let why = format!(
                    "its sensor has failed {} reads in a row (last error: {})",
                    f.reads, f.err
                );
                line.insert(s.signal.clone(), why.clone());
                sensors
                    .entry(s.signal)
                    .or_default()
                    .push(SilentSensor { sensor_id: s.id, why });
            }
        }
        (line, sensors)
    }
}
