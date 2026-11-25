//! How a sensor (`model::Sensor`, ADR-0047) is made and changed. A standing sensor, a poll that fills an event signal, is
//! made by the owner telling the Assistant: Remit reading a source on its own is a standing use of that source's
//! credential. A script sensor is made by the Chair of the chain that waits on what it reads (`create_chain_sensor`), and the
//! chain's grants decide what the script may read when it runs.

use serde::Serialize;
use std::collections::BTreeMap;

use model::{
    Actor, Card, ChainStatus, EVENT_KEY_PREFIX, ForeignJson, KIND_POLL, KIND_SCRIPT, Origin, PLUGIN_ACTION_PREFIX,
    Sensor, SensorScript, SignalKey, by_agent, chain_sensor_key,
};

use crate::Pipeline;
use crate::event_signal_keys::is_event_signal_name;
use crate::plugin_pipeline::settings_fit;
use crate::sensor::reads_as_sensor;
use chains::cap_names::CAP_SIGNALS;
use chains::chain_core::{first_line, new_id};
use chains::changes::doc_of;
use chains::{Error, Result};
use model::ActorKind;
use model::ChangeKind;
use model::ChangedField;
use reach::grants::{connection_in, connection_names};

/// The most script sensors one chain runs: each runs a script in the jail on every period, a standing use of the host.
const CHAIN_SENSORS_MAX: usize = 20;
/// The longest note a sensor may carry.
const SENSOR_NOTE_MAX: usize = 200;
/// The period a sensor reads on when none is named, in minutes.
const SENSOR_EVERY_DEFAULT_MINUTES: i64 = 5;

/// What a standing sensor reads and how often: what update_sensor replaces.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub(crate) struct SensorSettings {
    /// What it is for, in the owner's words.
    pub note: String,
    /// For a poll: the id of a source that can be polled, as list_sources names it.
    pub source: String,
    /// For a poll: the name of the connection of that source; empty is the default connection.
    pub connection: String,
    /// What to read, as the source's poll takes it.
    pub query: String,
    /// How often to read it, as 5m or 2h; at least 1m; empty is 5m.
    pub every: String,
    /// For a sensor of a plugin's kind: its settings, a JSON object valid against the kind's schema.
    pub settings: Option<BTreeMap<String, serde_json::Value>>,
}

/// A standing sensor as the Assistant's create_sensor and a Chair's create_standing_sensor take it.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub(crate) struct SensorInput {
    /// Its name: lowercase letters, digits and dashes, at most 40, starting with a letter or digit; it never changes.
    pub name: String,
    /// The event signal it writes the events of, event:<name>.
    pub signal: String,
    /// poll (the default) or a kind a plugin adds, plugin:<id>.<name>.
    pub kind: String,
    #[serde(flatten)]
    pub settings: SensorSettings,
}

/// A script sensor as a Chair's create_sensor takes it: what it is named, where it runs and what it runs are required.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct ChainSensorInput {
    /// Its name, the last part of its key (chain:<chain id>.sensor.<name>); naming an earlier sensor of this chain replaces
    /// it.
    pub name: String,
    /// The id of a source that runs scripts, one the script tool's source lists: the script runs behind that source's
    /// connection and nowhere else, so no source (and no shell) runs it without one.
    pub source: String,
    /// The name of the connection of that source; empty is the default connection.
    #[serde(default)]
    pub connection: String,
    /// The language of the script, one the script tool's language lists.
    pub language: String,
    /// The whole script; it prints one JSON value, the state to watch as it is now.
    pub code: String,
    /// How often to run it, as 5m or 2h; at least 1m; empty is 5m.
    #[serde(default)]
    pub every: String,
}

/// The period every names, in minutes, the default for none: a whole number of minutes and hours (`90m`, `2h`, `1h30m`),
/// at least a minute.
fn sensor_every_of(every: &str) -> Result<i64> {
    let s = every.trim();
    if s.is_empty() {
        return Ok(SENSOR_EVERY_DEFAULT_MINUTES);
    }
    let no_period = || Error::refused(format!("every {every:?} is no period: write it as 5m or 2h"));
    let (mut minutes, mut rest, mut seconds) = (0i64, s, 0i64);
    while !rest.is_empty() {
        let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        let (n, after) = rest.split_at(digits);
        let n: i64 = n.parse().map_err(|_| no_period())?;
        let unit = after.chars().next().ok_or_else(no_period)?;
        match unit {
            'h' => {
                minutes = minutes
                    .checked_add(n.checked_mul(60).ok_or_else(no_period)?)
                    .ok_or_else(no_period)?
            }
            'm' => minutes = minutes.checked_add(n).ok_or_else(no_period)?,
            's' => seconds = seconds.checked_add(n).ok_or_else(no_period)?,
            _ => return Err(no_period()),
        }
        rest = after.get(1..).unwrap_or_default();
    }
    let minutes = minutes.checked_add(seconds.div_euclid(60)).ok_or_else(no_period)?;
    if minutes < 1 {
        return Err(Error::refused(format!(
            "every {s} is too often: a sensor reads at most once a minute"
        )));
    }
    Ok(minutes)
}

fn invalid(e: model::Invalid) -> Error {
    Error::refused(e.to_string())
}

impl SensorSettings {
    /// s with these settings, checked: cleaned up, with the owner's own words kept as he said them. A sensor that reads
    /// what it read before keeps where it was; one that reads anything else starts from now.
    pub(crate) fn settings_on(&self, s: &Sensor) -> Result<Sensor> {
        let every = sensor_every_of(&self.every)?;
        let mut next = s.clone();
        next.note = first_line(self.note.trim(), SENSOR_NOTE_MAX);
        next.every_minutes = every;
        if s.kind.starts_with(PLUGIN_ACTION_PREFIX) {
            if !self.source.is_empty() || !self.connection.is_empty() || !self.query.is_empty() {
                return Err(Error::refused(
                    "a sensor of a plugin's kind takes settings, not a source, a connection or a query: put the connection it reads through in settings",
                ));
            }
            let settings = serde_json::to_string(&self.settings.clone().unwrap_or_default())?;
            next.settings = Some(ForeignJson::parse(&settings)?);
        } else {
            if self.settings.as_ref().is_some_and(|m| !m.is_empty()) {
                return Err(Error::refused(
                    "a poll takes a source, a connection and a query, no settings: settings are for a sensor of a plugin's kind",
                ));
            }
            next.source = self.source.trim().to_lowercase();
            next.connection = self.connection.trim().to_owned();
            next.query = self.query.trim().to_owned();
            if next.connection.is_empty() {
                source::DEFAULT_CONNECTION.clone_into(&mut next.connection);
            }
        }
        if !reads_as_sensor(s, &next) {
            next.cursor.clear();
        }
        next.check().map_err(invalid)?;
        Ok(next)
    }
}

impl SensorInput {
    /// The standing sensor this asks for, checked: no id, author or time of its own yet.
    pub(crate) fn sensor(&self) -> Result<Sensor> {
        let kind = self.kind.trim();
        let s = Sensor {
            kind: if kind.is_empty() {
                KIND_POLL.to_owned()
            } else {
                kind.to_owned()
            },
            name: self.name.trim().to_lowercase(),
            signal: SignalKey(self.signal.trim().to_owned()),
            ..Sensor::default()
        };
        if !is_event_signal_name(&s.name) {
            return Err(Error::refused(format!(
                "the name {:?} cannot name a sensor: use lowercase letters, digits and dashes, at most 40, starting with a letter or digit, e.g. replies",
                self.name
            )));
        }
        self.settings.settings_on(&s)
    }
}

impl ChainSensorInput {
    /// The script sensor of chain chain_id this asks for.
    pub(crate) fn chain_sensor(&self, chain_id: &str) -> Result<Sensor> {
        let name = self.name.trim().to_lowercase();
        if !is_event_signal_name(&name) {
            return Err(Error::refused(format!(
                "the name {:?} cannot name a sensor: use lowercase letters, digits and dashes, at most 40, starting with a letter or digit, e.g. deploy",
                self.name
            )));
        }
        let mut connection = self.connection.trim().to_owned();
        if connection.is_empty() {
            source::DEFAULT_CONNECTION.clone_into(&mut connection);
        }
        let s = Sensor {
            kind: KIND_SCRIPT.to_owned(),
            signal: chain_sensor_key(chain_id, &name),
            name,
            chain: chain_id.to_owned(),
            source: self.source.trim().to_lowercase(),
            connection,
            script: Some(SensorScript {
                language: self.language.trim().to_owned(),
                code: self.code.clone(),
            }),
            every_minutes: sensor_every_of(&self.every)?,
            ..Sensor::default()
        };
        s.check().map_err(invalid)?;
        Ok(s)
    }
}

fn no_sensor(id: &str) -> Error {
    Error::refused(format!("no sensor {id:?}: list_sensors shows the ids"))
}

impl Pipeline {
    /// Stores s, a sensor that is new or changed, and tells the channels.
    pub async fn put_sensor(&self, s: Sensor) -> Result<Sensor> {
        let name = s.name.clone();
        let saved = match self.store.update(move |tx| tx.put_sensor(s)).await {
            Err(store::Error::SensorName) => {
                return Err(Error::refused(format!(
                    "a sensor named {name:?} exists already: list_sensors shows it"
                )));
            }
            saved => saved?,
        };
        self.notify.sensor_updated(&saved);
        Ok(saved)
    }

    /// Refuses a poll of a source or a connection this server does not have.
    async fn check_sensor_source(&self, s: &Sensor) -> Result<()> {
        if self.sources.blank(&s.source).is_none() {
            return Err(Error::refused(format!(
                "no source {:?}: the ones that exist are {}",
                s.source,
                self.sources.source_ids().join(", ")
            )));
        }
        let source = s.source.clone();
        let conns = self.store.read(move |r| r.source_connections(&source)).await?;
        if !connection_in(&conns, &s.connection) {
            return Err(Error::refused(format!(
                "{} has no connection {:?}: the ones that exist are {}",
                s.source,
                s.connection,
                connection_names(&conns)
            )));
        }
        Ok(())
    }

    /// Refuses a sensor of what this server cannot read: a poll of a source or a connection it does not have, a sensor of
    /// a plugin's kind that no enabled plugin adds or whose settings its kind does not take.
    async fn check_sensor_reads(&self, s: &Sensor) -> Result<()> {
        if !s.kind.starts_with(PLUGIN_ACTION_PREFIX) {
            return self.check_sensor_source(s).await;
        }
        let Some(kind) = self.plugin_sensing(&s.kind).map(|p| p.kind) else {
            return Err(Error::refused(format!(
                "no sensor kind {:?} is installed: list_sensor_kinds shows the ones there are",
                s.kind
            )));
        };
        settings_fit(&kind.settings_schema, s.settings.as_ref())
    }

    /// Refuses a poll whose event signal nobody made.
    async fn check_sensor_signal(&self, s: &Sensor) -> Result<()> {
        let name = s
            .signal
            .0
            .strip_prefix(EVENT_KEY_PREFIX)
            .unwrap_or(&s.signal.0)
            .to_owned();
        let key = name.clone();
        match self.store.read(move |r| r.event_signal_named(&key)).await {
            Ok(_) => Ok(()),
            Err(store::Error::NotFound) => Err(Error::refused(format!(
                "no event signal {name:?}: make it first (create_event_signal), then its sensor; list_event_signals shows the ones there are"
            ))),
            Err(err) => Err(err.into()),
        }
    }

    /// Makes a standing sensor from the owner's words, by who asked, and tells the channels. A sensor made by anyone but
    /// the owner waits for him (`hold_sensor`): nothing is written until he answers.
    pub(crate) async fn create_sensor(&self, input: &SensorInput, by: &Actor) -> Result<Sensor> {
        let mut s = input.sensor()?;
        // both checks run, so one answer names everything to fix
        match (self.check_sensor_reads(&s).await, self.check_sensor_signal(&s).await) {
            (Ok(()), Ok(())) => {}
            (Err(a), Err(b)) => return Err(Error::refused(format!("{a}\n{b}"))),
            (Err(e), Ok(())) | (Ok(()), Err(e)) => return Err(e),
        }
        let name = s.name.clone();
        match self.store.read(move |r| r.sensor_named("", &name)).await {
            Ok(_) => {
                return Err(Error::refused(format!(
                    "a sensor named {:?} exists already: list_sensors shows it",
                    s.name
                )));
            }
            Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
        let now = self.now();
        (s.id, s.by, s.created_at, s.updated_at) = (new_id("sn_"), by.clone(), now, now);
        self.hold_sensor(by, None, &s).await?;
        self.put_sensor(s).await
    }

    /// Replaces what a standing sensor reads and how often. Its name and its signal never change. A sensor that reads a
    /// source, a connection or a query it did not read before waits for the owner when anyone but he asks.
    pub(crate) async fn update_sensor(&self, id: &str, settings: &SensorSettings, by: &Actor) -> Result<Sensor> {
        let cur = self.standing_sensor(id).await?;
        let mut next = settings.settings_on(&cur)?;
        self.check_sensor_reads(&next).await?;
        (next.by, next.updated_at) = (by.clone(), self.now());
        self.hold_sensor(by, Some(&cur), &next).await?;
        self.put_sensor(next).await
    }

    /// The standing sensor id; a chain's script is no one's to change but its Chair's.
    async fn standing_sensor(&self, id: &str) -> Result<Sensor> {
        let key = id.to_owned();
        match self.store.read(move |r| r.sensor(&key)).await {
            Ok(s) if s.chain.is_empty() => Ok(s),
            Ok(_) | Err(store::Error::NotFound) => Err(no_sensor(id)),
            Err(err) => Err(err.into()),
        }
    }

    /// Stops a standing sensor reading until it is resumed; resuming starts it again from where it was.
    pub(crate) async fn pause_sensor(&self, id: &str, paused: bool, by: model::Actor) -> Result<Sensor> {
        let mut s = self.standing_sensor(id).await?;
        (s.paused, s.by, s.updated_at) = (paused, by, self.now());
        self.put_sensor(s).await
    }

    /// Removes a standing sensor: nothing reads for its signal again. Its events stay.
    pub(crate) async fn delete_sensor(&self, id: &str) -> Result<()> {
        let s = self.standing_sensor(id).await?;
        let key = s.id.clone();
        self.store.update(move |tx| tx.delete_sensor(&key)).await?;
        self.notify.sensor_deleted(&s.id);
        Ok(())
    }

    /// Makes the script sensor input asks for, for the chain of chair, and runs it once: a script that fails or prints no
    /// JSON is refused while the Chair can fix it, and nothing is stored. The state it printed is the key's first reading,
    /// and its period runs from now. A sensor of the same name replaces the earlier one.
    pub(crate) async fn create_chain_sensor(&self, chair: &Card, input: &ChainSensorInput) -> Result<Sensor> {
        if chair.origin != Origin::Chair {
            return Err(Error::refused("only the Chair can make a sensor"));
        }
        let home = chair.home.clone();
        let ch = self.store.read(move |r| r.chain(&home)).await?;
        if !ch.capabilities.iter().any(|c| c == CAP_SIGNALS) {
            return Err(Error::refused(format!(
                "a sensor needs the {CAP_SIGNALS} capability, which this chain does not hold: ask for it with request_capability"
            )));
        }
        if *ch.status() != ChainStatus::Running {
            return Err(Error::refused(format!("the chain is {}", ch.status())));
        }
        let mut s = input.chain_sensor(&ch.id)?;
        let now = self.now();
        (s.by, s.created_at, s.updated_at) = (by_agent(&chair.id), now, now);
        let (chain_id, name) = (ch.id.clone(), s.name.clone());
        match self.store.read(move |r| r.sensor_named(&chain_id, &name)).await {
            Ok(earlier) => (s.id, s.created_at) = (earlier.id, earlier.created_at),
            Err(store::Error::NotFound) => {
                s.id = new_id("sn_");
                let all = self.store.read(|r| r.sensors()).await?;
                let n = all.iter().filter(|x| x.chain == ch.id && x.kind == KIND_SCRIPT).count();
                if n >= CHAIN_SENSORS_MAX {
                    return Err(Error::refused(format!("this chain already has {n} sensors: no more")));
                }
            }
            Err(err) => return Err(err.into()),
        }
        let state = self
            .read_script(&ch.id, &ch.chair_id, &s)
            .await
            .map_err(|e| Error::refused(format!("the sensor's script did not give a state: {e}")))?;
        let s = self.put_sensor(s).await?;
        self.signals
            .sensors
            .claim(&s.id, now, time::Duration::minutes(s.every_minutes));
        if let Err(err) = self.keep_sensed(&s, state).await {
            // the next run stores it again
            tracing::warn!(chain = %ch.id, sensor = %s.name, %err, "sensor's first reading");
        }
        Ok(s)
    }

    /// Makes the sensor a bundled plugin ships with event signal name, if it ships one. It is made once, with the signal,
    /// and is the owner's after that: a sensor he deleted is not made again.
    pub(crate) async fn seed_sensor(&self, plugin: &str, s: &plugin_sdk::Signal) -> Result<()> {
        if s.poll_every_minutes <= 0 {
            return Ok(()); // a signal with no poll is filled at its door: no sensor reads for it
        }
        let input = SensorInput {
            name: s.name.clone(),
            signal: model::event_key(&s.name).0,
            settings: SensorSettings {
                source: plugin.to_owned(),
                query: s.poll_query.clone(),
                every: format!("{}m", s.poll_every_minutes),
                ..SensorSettings::default()
            },
            ..SensorInput::default()
        };
        let mut sn = input
            .sensor()
            .map_err(|e| Error::refused(format!("the shipped sensor {}: {e}", s.name)))?;
        let now = self.now();
        (sn.id, sn.by, sn.created_at, sn.updated_at) = (new_id("sn_"), model::by_remit("seed"), now, now);
        self.put_sensor(sn).await.map(|_| ())
    }
}

/// The fields of a sensor's write that move a rail: what it reads, when it is a source, a connection, a query or a script
/// the sensor (cur, none for a new one) did not read before. A sensor read more or less often, or noted, moves none.
pub fn sensor_rail(cur: Option<&Sensor>, next: &Sensor) -> Result<Vec<ChangedField>> {
    if cur.is_some_and(|c| reads_as_sensor(c, next)) {
        return Ok(Vec::new());
    }
    #[derive(Serialize)]
    struct Reads<'a> {
        signal: &'a model::SignalKey,
        kind: &'a str,
        source: &'a str,
        connection: &'a str,
        query: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        settings: Option<&'a model::ForeignJson>,
    }
    let reads = |s: Option<&Sensor>| {
        doc_of(
            s.map(|s| Reads {
                signal: &s.signal,
                kind: &s.kind,
                source: &s.source,
                connection: &s.connection,
                query: &s.query,
                settings: s.settings.as_ref(),
            })
            .as_ref(),
        )
    };
    Ok(vec![ChangedField {
        name: "reads".to_owned(),
        before: reads(cur)?,
        after: reads(Some(next))?,
    }])
}

impl Pipeline {
    /// Asks the owner before anyone but he and Remit itself writes standing sensor next, which reads what cur (none for a
    /// new sensor) did not: Remit reading a source on its own, with no chain, is a standing use of the source's
    /// credential, which no agent starts, the Assistant included, since what it reads may steer it. It says so with
    /// `Error::WaitingForOwner`, and is Ok when the write may go ahead.
    pub(crate) async fn hold_sensor(&self, by: &Actor, cur: Option<&Sensor>, next: &Sensor) -> Result<()> {
        if matches!(by.kind, ActorKind::Owner | ActorKind::Remit) {
            return Ok(());
        }
        let fields = sensor_rail(cur, next)?;
        self.hold_change(by, ChangeKind::Sensor, &next.name, doc_of(Some(next))?, fields)
            .await
    }
}
