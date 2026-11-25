//! The sensor tools (ADR-0047). The Assistant's make a standing sensor for the owner, by his words: a reader that fills an
//! event signal from a source Remit reads itself. A Chair's create_sensor makes a script that reads the state it waits on,
//! for its chain; its create_standing_sensor asks for a standing sensor, for what must be read after its chain ends
//! (ADR-0065). A sensor anyone but the owner asks for waits for his Apply (change_item.rs `hold_sensor`): the tool then
//! says so in the words the agent acts on (`held_words`), not as a failure.

use std::sync::Arc;

use std::collections::BTreeMap;

use model::{Actor, Sensor, event_key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Pipeline;
use crate::event_signal_keys::EventSignalInput;
use crate::event_signal_tools::{CreateEventSignalInput, KEY_ON_THE_PAGE, door_path};
use crate::sensor_settings::{ChainSensorInput, SensorInput, SensorSettings};
use chains::Result;
use chains::change_item::held_words;
use chains::tools::{Effect, NoInput, SelfCard, Tools, as_json};

/// The settings update_sensor replaces, of the sensor it names. Its settings sit beside its id, so the input is not closed
/// to other fields (serde cannot close a flattened struct).
#[derive(Deserialize, JsonSchema)]
struct UpdateSensorInput {
    /// sensor id (sn_…), as list_sensors shows it
    id: String,
    #[serde(flatten)]
    settings: SensorSettings,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SensorIdInput {
    /// sensor id (sn_…), as list_sensors shows it
    id: String,
}

/// A standing sensor as the Assistant reads it: where it writes, and its settings in the shape update_sensor takes.
#[derive(Debug, Serialize)]
struct SensorRecord {
    id: String,
    name: String,
    signal: String,
    kind: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    paused: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    source: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    connection: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    query: String,
    every: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<BTreeMap<String, Value>>,
}

fn sensor_record_of(s: Sensor) -> SensorRecord {
    SensorRecord {
        every: format!("{}m", s.every_minutes),
        settings: settings_map(&s),
        id: s.id,
        name: s.name,
        signal: s.signal.0,
        kind: s.kind,
        paused: s.paused,
        note: s.note,
        source: s.source,
        connection: s.connection,
        query: s.query,
    }
}

/// The settings of a plugin's sensor as the Assistant reads and writes them: a JSON object; none when they are not one.
fn settings_map(s: &Sensor) -> Option<BTreeMap<String, Value>> {
    serde_json::from_value(s.settings.as_ref()?.0.clone()).ok()
}

/// How a sensor relates to the signal and the subscriptions: it only reads, and what is done about what it read is a
/// subscription's.
const SENSOR_GUIDE: &str = "A sensor reads a source on a period, as Remit itself and only to read, and writes each new or changed item as an event of its event signal; the first read sets where it starts and emits nothing older. It does nothing about what it reads: what he wants done is a subscription to the signal (create_subscription). A sensor that reads a source, a connection or a query the signal did not have read before waits for an Apply in his inbox, which the Assistant decides for him: say which item waits.";

/// What a tool that makes or changes sensor sn answers: done says what was written; a write that waits for the owner
/// says so in the words the agent acts on.
fn written_or_held(sn: Result<Sensor>, done: impl FnOnce(Sensor) -> String) -> Result<String> {
    match sn {
        Ok(sn) => Ok(done(sn)),
        Err(err) => held_words(&err).ok_or(err),
    }
}

impl Pipeline {
    /// Gives the Assistant, acting as by, the tools that keep the owner's standing sensors.
    pub fn add_sensor_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        self.add_sensor_kind_tool(t);
        t.serving(self).add(
            "list_sensors", Effect::Reads,
            "The standing sensors: each with the event signal it writes, what it reads, how often, and whether it is paused, in the shape update_sensor takes.",
            |o, _: NoInput| async move {
                let all = o.store.read(|r| r.sensors()).await?;
                let recs: Vec<SensorRecord> = all.into_iter().filter(Sensor::standing).map(sensor_record_of).collect();
                as_json(&recs)
            },
        );
        let create_by = by.clone();
        t.serving(self).add(
            "create_sensor", Effect::Writes,
            format!(
                "Make a sensor for the owner: it reads a source on a period and writes what it reads as events of an event signal he made (create_event_signal first when none fits). Make one only when he asked for it, and name it as he would say it. {SENSOR_GUIDE}"
            ),
            move |o, i: SensorInput| {
                let by = create_by.clone();
                async move {
                    written_or_held(o.create_sensor(&i, &by).await, |sn| {
                        format!("Made sensor {}, {:?}: it reads for {}.", sn.id, sn.name, sn.signal)
                    })
                }
            },
        );
        let update_by = by.clone();
        t.serving(self).add(
            "update_sensor", Effect::Writes,
            format!(
                "Replace what a standing sensor reads and how often (start from list_sensors and change what he asked, so the rest stays). Its name and its signal never change. {SENSOR_GUIDE}"
            ),
            move |o, i: UpdateSensorInput| {
                let by = update_by.clone();
                async move {
                    written_or_held(o.update_sensor(&i.id, &i.settings, &by).await, |sn| {
                        format!("Updated sensor {}, {:?}.", sn.id, sn.name)
                    })
                }
            },
        );
        let pause_by = by.clone();
        t.serving(self).add(
            "pause_sensor", Effect::Writes,
            "Stop a standing sensor reading until it is resumed, for the owner. Its signal still takes its other writers.",
            move |o, i: SensorIdInput| {
                let by = pause_by.clone();
                async move {
                    let sn = o.pause_sensor(&i.id, true, by).await?;
                    Ok(format!("Paused sensor {}, {:?}.", sn.id, sn.name))
                }
            },
        );
        let resume_by = by.clone();
        t.serving(self).add(
            "resume_sensor",
            Effect::Writes,
            "Let a paused standing sensor read again, for the owner, from where it stopped.",
            move |o, i: SensorIdInput| {
                let by = resume_by.clone();
                async move {
                    let sn = o.pause_sensor(&i.id, false, by).await?;
                    Ok(format!("Resumed sensor {}, {:?}.", sn.id, sn.name))
                }
            },
        );
        t.serving(self).add(
            "delete_sensor",
            Effect::Writes,
            "Delete a standing sensor the owner named: nothing reads for its signal again. Its events stay.",
            |o, i: SensorIdInput| async move {
                o.delete_sensor(&i.id).await?;
                Ok(format!("Deleted sensor {}.", i.id))
            },
        );
    }

    /// Gives the Chair of a chain, card me, create_sensor: a script that reads the state it waits on, for its chain.
    pub(crate) fn add_chain_sensor_tool(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let me = me.clone();
        t.serving(self).add(
            "create_sensor", Effect::Writes,
            "Make a sensor for this chain when no signal gives the state you need to see (a deploy's status, a queue's depth, a page's content): a script that prints that state as one JSON value. It needs the signals capability. Remit runs it every few minutes, as you, behind a source's connection (your grants decide whether it may read there), while a watch on its key is open, and each state that differs from the one before is a reading of the key chain:<chain id>.sensor.<name>. It runs once now: a script that fails is refused and nothing is stored. Then watch the key with subscribe, with a condition over `value` (the state) and an until. It stops with the watch: what must be read after this chain ends (a key result's number) is a standing sensor, create_standing_sensor.",
            move |o, i: ChainSensorInput| {
                let me = me.clone();
                async move {
                    let card = me.read(&o).await?;
                    let sn = o.create_chain_sensor(&card, &i).await?;
                    Ok(format!(
                        "Made sensor {:?}: its key is {}, and its first reading is stored. Watch it with subscribe.",
                        sn.name, sn.signal
                    ))
                }
            },
        );
    }

    /// Gives the Chair of a chain with the signals capability, acting as by, what keeps reading after its chain ends: an
    /// event signal, made at once as the Assistant makes one (it lets events in and moves no rail; its key is the owner's
    /// alone), and a standing sensor of it, which waits for the owner's Apply as every sensor anyone but he makes does
    /// (`hold_sensor`). A chain makes what a key result reads itself, so it never asks the owner to make it by hand
    /// (ADR-0065).
    pub fn add_standing_signal_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        let signal_by = by.clone();
        t.serving(self).add(
            "create_event_signal", Effect::Writes,
            format!(
                "Make an event signal, key event:<name>, when what you need must keep being read after this chain ends (the number a key result, a guardrail or a measure reads): the signal a standing sensor writes (create_standing_sensor next), or a way in for an outside system or script, with a door of its own at /hooks/<name>. Name it for what it carries (lowercase, dashes). It is made at once, by you, and the owner sees it on the Signals tab; an error that the name exists means it is there already. Never ask the owner to make one. {KEY_ON_THE_PAGE} Only a system that posts to the door needs that key: then ask the owner to give it to that system, naming the page."
            ),
            move |o, i: CreateEventSignalInput| {
                let by = signal_by.clone();
                async move {
                    let input: EventSignalInput = i.into();
                    let es = o.create_event_signal(&input, by).await?;
                    Ok(format!(
                        "Made event signal {}, {:?}: its key is {} and its door takes posts at {}.",
                        es.id,
                        es.name,
                        event_key(&es.name),
                        door_path(&es.name)
                    ))
                }
            },
        );
        let sensor_by = by.clone();
        t.serving(self).add(
            "create_standing_sensor", Effect::Writes,
            "Ask for a standing sensor: Remit reads a source on a period and writes what it reads as events of an event signal (create_event_signal first), and keeps reading after this chain ends, so a key result or a measure can read it. Use it when what you need must outlive the chain; for a state only this chain watches, use create_sensor. A source that can be polled is read with a query in its words (GitHub issues, Datadog); a reading no source gives (a page, a counter, a local program) is a script: source script, connection default, the code in query under a first line naming its runtime (#!sh, #!node, #!python3), printing one JSON line {\"id\": \"...\", \"body\": {...}} per event. Remit reads as itself, only to read, and the first read sets where it starts and emits nothing older. It is never written at once: it waits in the owner's inbox, where the Assistant reads the whole query and decides it for him, Apply or Not now, and a message starting [change result tells you which. Never ask the owner to make it himself; until it is applied, it does not exist.",
            move |o, i: SensorInput| {
                let by = sensor_by.clone();
                async move {
                    written_or_held(o.create_sensor(&i, &by).await, |sn| {
                        format!("Made sensor {}, {:?}: it reads for {}.", sn.id, sn.name, sn.signal)
                    })
                }
            },
        );
    }
}
