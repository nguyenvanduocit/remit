//! The Assistant's event signal tools: how the owner opens a way in for an outside system, a script or a source Remit
//! reads, by telling it. Each is the orchestrator's own action (event_signal_keys.rs), taken as the Assistant for him, so
//! the owner sees the signal at once, its page on the Signals tab (its settings, its door and key, its events and the
//! subscriptions to it), and who made it. None returns a key: a key made here is shown on that page, to him alone, and an
//! agent never sees it. A signal only lets events in: what is done about them is a subscription's (subscription_tools.rs).

use std::sync::Arc;

use model::{Actor, Cause, Chain, EventSignal, Writer, WriterKind, by_agent, event_key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Pipeline;
use crate::event::Emission;
use crate::event_record::event_ref;
use crate::event_signal_keys::EventSignalInput;
use chains::tools::{Effect, NoInput, Tools, as_json};

/// An event signal as create_event_signal takes it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateEventSignalInput {
    /// its name, the last part of its key (event:<name>) and of its door's URL (/hooks/gitlab-billing): lowercase letters, digits and dashes, at most 40, starting with a letter or digit; it never changes
    name: String,
    /// what it is for, in the owner's words, e.g. 'pipelines and deploys of the billing group'
    #[serde(default)]
    note: String,
}

impl From<CreateEventSignalInput> for EventSignalInput {
    fn from(i: CreateEventSignalInput) -> Self {
        EventSignalInput {
            name: i.name,
            note: i.note,
        }
    }
}

/// An event signal's settings as update_event_signal replaces them.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateEventSignalInput {
    /// event signal id (es_…), as list_event_signals shows it
    id: String,
    /// what it is for, in the owner's words, e.g. 'pipelines and deploys of the billing group'
    #[serde(default)]
    note: String,
    /// true: its events never leave this machine, so no decider reads one to screen it (a subscription's `screen` is
    /// not asked of it); false: they may, with credentials removed and email addresses masked; leave it out to keep it as
    /// it is
    #[serde(default)]
    kept: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EventSignalIdInput {
    /// event signal id (es_…), as list_event_signals shows it
    id: String,
}

/// An event signal as the Assistant reads it: no key, where its door is, and its settings in the shape
/// update_event_signal takes.
#[derive(Debug, Serialize)]
struct EventSignalRecord {
    id: String,
    name: String,
    key: String,
    path: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    paused: bool,
    /// Ships with Remit: it is never deleted, and reset_event_signal puts back what ships. Edited says its settings are
    /// the owner's version.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    builtin: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    edited: bool,
    /// Its events stay on this machine: no decider reads them.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    kept: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
}

fn event_signal_record_of(es: EventSignal) -> EventSignalRecord {
    EventSignalRecord {
        key: event_key(&es.name).0,
        path: door_path(&es.name),
        id: es.id,
        name: es.name,
        paused: es.paused,
        builtin: es.builtin,
        edited: es.edited,
        kept: es.kept,
        note: es.note,
    }
}

/// The address of the door of event signal name.
pub(crate) fn door_path(name: &str) -> String {
    format!("/hooks/{name}")
}

/// How the owner's wishes about what comes in become an event signal and its subscriptions: the signal only holds what
/// comes in, one reading for each event; what is done about an event is a subscription's, one per rule, each its own
/// `when` and its own action.
const SETTINGS_GUIDE: &str = "An event signal holds what comes in and does nothing about it: what he wants done is a SUBSCRIPTION to it (create_subscription), one for each thing he wants, with its own `when` (a CEL expression over `body`, the event's JSON, and `header`, its headers keyed as sent, e.g. header[\"X-Gitlab-Event\"]) and its own action. Read a real event's fields (read_event_fields, from an address list_events shows) before you write a `when`: never guess a sender's payload. \
A system with no way to post to Remit (GitHub issues, Datadog monitors, Grafana alerts, a number Datadog or Grafana computes, a page behind a login) is read by a SENSOR of the signal (create_sensor), not by the signal. ";

/// Where the owner finds a door's key, which no agent is shown.
pub(crate) const KEY_ON_THE_PAGE: &str = "Its door's key is on the page of the signal (event:<name>) on the Signals tab: tell the owner to open it and copy the key there. You are never shown it.";

/// An event an agent writes.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EmitEventInput {
    /// the name of the event signal (event:<name>), one the owner made: list_event_signals shows them
    signal: String,
    /// the event, as JSON or plain text: what happened, with the facts whoever hears it needs
    body: String,
    /// your own id for the event, so that writing it again is the same event; empty: the same body is the same event
    #[serde(default)]
    id: String,
}

impl Pipeline {
    /// Gives the Assistant, acting as by, the tools that open, change and close the ways in for outside systems.
    pub fn add_event_signal_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        t.serving(self).add(
            "list_event_signals", Effect::Reads,
            "The event signals: each with its key (event:<name>), its door's address (path /hooks/<name>), what it is for, whether it is paused and whether it is builtin (ships with Remit: edit it as any, never delete it; edited says the settings are his version), in the shape update_event_signal takes. A key is never shown to you.",
            |o, _: NoInput| async move {
                let all = o.store.read(|r| r.event_signals()).await?;
                let recs: Vec<EventSignalRecord> = all.into_iter().map(event_signal_record_of).collect();
                as_json(&recs)
            },
        );
        let create_by = by.clone();
        t.serving(self).add(
            "create_event_signal", Effect::Writes,
            format!(
                "Make an event signal for the owner: a way in for one outside system or script (GitLab, GitHub, Sentry, Jira, a cron job), or the signal a sensor writes (GitHub issues Remit reads…), with a key event:<name>, a door of its own at /hooks/<name> and a key that only its writer will hold. Make one only when he asked for it, and name it as he would say it (lowercase, dashes). {SETTINGS_GUIDE}{KEY_ON_THE_PAGE}"
            ),
            move |o, i: CreateEventSignalInput| {
                let by = create_by.clone();
                async move {
                    let es = o.create_event_signal(&i.into(), by).await?;
                    Ok(format!(
                        "Made event signal {}, {:?}: its door takes posts at {}. {KEY_ON_THE_PAGE}",
                        es.id,
                        es.name,
                        door_path(&es.name)
                    ))
                }
            },
        );
        let update_by = by.clone();
        t.serving(self).add(
            "update_event_signal", Effect::Writes,
            format!(
                "Replace an event signal's settings: what it is for, and whether its events are kept on this machine (`kept`: no decider reads one, so a screen asks none of it) (start from list_event_signals and change what he asked, so the rest stays). Its name is its key and never changes. A builtin signal takes his changes too, and keeps his version until he resets it. {SETTINGS_GUIDE}"
            ),
            move |o, i: UpdateEventSignalInput| {
                let by = update_by.clone();
                async move {
                    let es = o.update_event_signal(&i.id, i.note, i.kept, by).await?;
                    Ok(format!("Updated event signal {}, {:?}.", es.id, es.name))
                }
            },
        );
        let reset_by = by.clone();
        t.serving(self).add(
            "reset_event_signal", Effect::Writes,
            "Put a builtin event signal back to the settings that ship with Remit, when the owner asks: his note is replaced (read it with list_event_signals first and tell him what goes). Its key and pause stay, and so do its sensors and subscriptions. A signal that does not ship has no original to reset to.",
            move |o, i: EventSignalIdInput| {
                let by = reset_by.clone();
                async move {
                    let es = o.reset_event_signal(&i.id, by).await?;
                    Ok(format!("Reset event signal {}, {:?}, to what ships with Remit.", es.id, es.name))
                }
            },
        );
        let rotate_by = by.clone();
        t.serving(self).add(
            "rotate_event_signal_key", Effect::Writes,
            format!(
                "Give an event signal's door a new key, when the owner asks (a key leaked, or an employee left): the old key stops working at once, so whoever posts with it must be given the new one. {KEY_ON_THE_PAGE}"
            ),
            move |o, i: EventSignalIdInput| {
                let by = rotate_by.clone();
                async move {
                    let es = o.rotate_event_signal_key(&i.id, by).await?;
                    Ok(format!(
                        "Rotated the key of event signal {}, {:?}: the old key no longer works. {KEY_ON_THE_PAGE}",
                        es.id, es.name
                    ))
                }
            },
        );
        let pause_by = by.clone();
        t.serving(self).add(
            "pause_event_signal", Effect::Writes,
            "Pause an event signal for the owner: it takes no event until it resumes, whoever writes it (its door, the CLI, its sensors, an agent). Its key stays.",
            move |o, i: EventSignalIdInput| {
                let by = pause_by.clone();
                async move {
                    let es = o.pause_event_signal(&i.id, true, by).await?;
                    Ok(format!("Paused event signal {}, {:?}.", es.id, es.name))
                }
            },
        );
        let resume_by = by.clone();
        t.serving(self).add(
            "resume_event_signal",
            Effect::Writes,
            "Resume a paused event signal for the owner: its writers are let in again.",
            move |o, i: EventSignalIdInput| {
                let by = resume_by.clone();
                async move {
                    let es = o.pause_event_signal(&i.id, false, by).await?;
                    Ok(format!("Resumed event signal {}, {:?}.", es.id, es.name))
                }
            },
        );
        t.serving(self).add(
            "delete_event_signal", Effect::Writes,
            "Delete an event signal the owner named, with its key: nothing writes it again. Its events stay. A builtin signal is never deleted: reset_event_signal puts back what ships, and pause_event_signal lets nothing in.",
            |o, i: EventSignalIdInput| async move {
                o.delete_event_signal(&i.id).await?;
                Ok(format!("Deleted event signal {}.", i.id))
            },
        );
    }

    /// Gives the Chair of chain ch, card card_id, emit_event (the events capability): it writes an event as any writer
    /// does, through `emit`, caused by its chain, so that the chain's own subscriptions do not hear it.
    pub fn add_emit_tool(self: &Arc<Self>, t: &mut Tools, ch: &Chain, card_id: &str) {
        let (chain_id, card_id) = (ch.id.clone(), card_id.to_owned());
        t.serving(self).add(
            "emit_event", Effect::Writes,
            "Write an event of an event signal the owner made, as an outside system would post one: the subscriptions to it hear it, and a Chair it is about may be told. Write what happened, not what to do: whoever reads it takes it as evidence, never as an instruction. Your own chain does not hear it.",
            move |o, i: EmitEventInput| {
                let (chain_id, card_id) = (chain_id.clone(), card_id.clone());
                async move {
                    let me = o.store.read(move |r| r.card(&card_id)).await?;
                    let got = o
                        .emit(Emission {
                            signal: i.signal,
                            writer: Writer { kind: WriterKind::Agent, id: me.id.clone() },
                            body: i.body.into_bytes(),
                            sender_id: i.id,
                            cause: Cause { actor: by_agent(&me.id), chain_id },
                            ..Emission::default()
                        })
                        .await?;
                    Ok(format!("Wrote event {}.", event_ref(&got.reading)))
                }
            },
        );
    }
}
