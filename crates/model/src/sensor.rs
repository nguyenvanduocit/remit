//! A sensor reads the world and writes readings of one signal (docs/adr/0047): the only thing that goes out and reads. A
//! signal carries what it writes and a subscription listens to the signal, so none of the three owns another: a sensor
//! can be replaced under a standing subscription. Two kinds exist, told apart by what they read:
//!
//! - a poll reads a source's connection with a query, from a cursor, and every new or changed item it answers is an
//!   event of an event signal. It is standing: `chain` is empty.
//! - a script runs behind a connection's proxy as a chain's Chair and prints the state as one JSON value; a state that
//!   differs from the newest is the next reading of the chain's own sensor key. It belongs to that chain.

use serde::{Deserialize, Serialize};

use crate::{Actor, EVENT_KEY_PREFIX, ForeignJson, Invalid, SignalKey, SignalPattern, Time};

/// What a sensor is. The set is open, so a kind is text: Remit senses poll and script itself, and a plugin brings more
/// (docs/adr/0049). A record whose kind nobody senses is held silent, with the reason, never dropped.
pub const KIND_POLL: &str = "poll";
pub const KIND_SCRIPT: &str = "script";

/// One reader, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Sensor {
    pub id: String,
    /// What it is: poll, script, or a plugin's.
    pub kind: String,
    /// What the owner and the chain call it: unique among the standing sensors, and among the sensors of one chain.
    pub name: String,
    pub note: String,
    /// The chain a script sensor belongs to; empty for a poll.
    pub chain: String,
    /// The key whose readings it writes: `event:<name>` for a poll, `chain:<chain>.sensor.<name>` for a script.
    pub signal: SignalKey,
    pub source: String,
    pub connection: String,
    /// What a poll reads, in the source's words. Empty for the other kinds.
    pub query: String,
    /// What a script sensor runs. None for the other kinds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<SensorScript>,
    /// How often a poll or a script reads, at least 1.
    pub every_minutes: i64,
    /// Where a poll's next read starts, as the source gave it; empty before the first read, which emits nothing older.
    pub cursor: String,
    /// What a sensor of a plugin's kind is given: a JSON object valid against the kind's settings schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<ForeignJson>,
    pub paused: bool,
    /// Who made it or last changed it.
    pub by: Actor,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts its stored changes.
    pub rev: i64,
}

/// The code a script sensor runs: its language, as the script tool takes it, and the whole script.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SensorScript {
    pub language: String,
    pub code: String,
}

/// Begins the key of every record a chain makes: chain:<id>.landing.<repo>, chain:<id>.live, chain:<id>.sensor.<name>.
pub const CHAIN_KEY_PREFIX: &str = "chain:";

/// Names the chain records that are a script sensor's readings: chain:<id>.sensor.<name>.
const SENSOR_KEY_INFIX: &str = ".sensor.";

/// The key of the readings of the script sensor name of chain chain_id.
pub fn chain_sensor_key(chain_id: &str, name: &str) -> SignalKey {
    SignalKey(format!("{CHAIN_KEY_PREFIX}{chain_id}{SENSOR_KEY_INFIX}{name}"))
}

/// The name of the sensor of a chain that reads how the forge's pipeline stands on what the chain landed (ADR-0114,
/// ADR-0057's pattern): a sensor of the plugin forge's kind landing, which the server keeps one of for each chain with a
/// landing whose pipeline has not settled. Its readings are the chain's own, under the key of the chain's sensor named
/// for the landing (parse_landing_sensor_key).
pub const LANDING_SENSOR: &str = "landing";

/// The id of the landing sensor of chain chain_id.
pub fn landing_sensor_id(chain_id: &str) -> String {
    format!("ls_{chain_id}")
}

/// The keys of the readings of the landing sense of every chain.
pub fn landing_sensor_pattern() -> SignalPattern {
    SignalPattern(format!("{CHAIN_KEY_PREFIX}*{SENSOR_KEY_INFIX}{LANDING_SENSOR}.*"))
}

/// The chain and the landing a key of the landing sense names; none for any other key. A script sensor's name has no
/// dot, so no script writes one of these.
pub fn parse_landing_sensor_key(key: &SignalKey) -> Option<(&str, &str)> {
    let rest = key.as_str().strip_prefix(CHAIN_KEY_PREFIX)?;
    let (chain_id, landing) = rest.split_once(&format!("{SENSOR_KEY_INFIX}{LANDING_SENSOR}."))?;
    (!chain_id.is_empty() && !landing.is_empty()).then_some((chain_id, landing))
}

impl Sensor {
    /// Whether the sensor is one the owner configures and keeps: it belongs to no chain.
    pub fn standing(&self) -> bool {
        self.chain.is_empty()
    }

    /// Whether the sensor is sound, as the one place that decides which sets of fields may exist: a sensor is built
    /// through it at the boundary and trusted inside. Each kind Remit senses itself says what makes it sound; a kind a
    /// plugin brings has no rule here: the host that installs it checks its records.
    pub fn check(&self) -> Result<(), Invalid> {
        if self.name.trim().is_empty() || self.kind.trim().is_empty() {
            return Err(Invalid::new("a sensor has a name and a kind"));
        }
        match self.kind.as_str() {
            KIND_POLL => self.check_poll(),
            KIND_SCRIPT => self.check_script(),
            _ => Ok(()),
        }
    }

    /// Whether the sensor goes through a source's connection, as a poll and a script do.
    fn reads(&self) -> Result<(), Invalid> {
        if self.source.trim().is_empty() || self.connection.trim().is_empty() {
            return Err(Invalid::new(
                "a sensor that reads names the source and the connection it reads through",
            ));
        }
        if self.every_minutes < 1 {
            return Err(Invalid::new(
                "a sensor reads at most once a minute: its period is a minute or more",
            ));
        }
        Ok(())
    }

    fn check_poll(&self) -> Result<(), Invalid> {
        self.reads()?;
        if self.query.trim().is_empty() || self.script.is_some() {
            return Err(Invalid::new("a poll reads a query, and no script"));
        }
        if !self.chain.is_empty() {
            return Err(Invalid::new(
                "a poll is standing and has no chain: only a script belongs to a chain",
            ));
        }
        let signal = self.signal.as_str();
        if !signal.starts_with(EVENT_KEY_PREFIX) || signal.len() == EVENT_KEY_PREFIX.len() {
            return Err(Invalid::new(format!(
                "a poll writes an event signal, {EVENT_KEY_PREFIX}<name>: {signal} is not one"
            )));
        }
        Ok(())
    }

    fn check_script(&self) -> Result<(), Invalid> {
        self.reads()?;
        let Some(script) = self.script.as_ref().filter(|_| self.query.trim().is_empty()) else {
            return Err(Invalid::new("a script sensor runs a script, and has no query"));
        };
        if script.language.trim().is_empty() || script.code.trim().is_empty() {
            return Err(Invalid::new(
                "a script sensor names the language of its script and gives the script",
            ));
        }
        if self.chain.is_empty() {
            return Err(Invalid::new(
                "a script sensor belongs to the chain that waits on what it reads: it names that chain",
            ));
        }
        if self.signal != chain_sensor_key(&self.chain, &self.name) {
            return Err(Invalid::new(format!(
                "a script writes its own key, {}: {} is not it, and a script never writes a record Remit writes",
                chain_sensor_key(&self.chain, "<name>"),
                self.signal
            )));
        }
        Ok(())
    }
}
