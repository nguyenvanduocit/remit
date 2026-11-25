//! An event signal (`model::EventSignal`) is the settings of `event:<name>`: what it is for, whether writers are let in,
//! and a key that only its holder can post to its door with. The owner never fills in a form for one: he tells the
//! Assistant, whose tools are the actions here. The key is made by Remit, sealed under the key file beside the database
//! and kept apart from the record, so no record, push, decision, transcript or tool result carries it. Only two things
//! read it back: the door, which checks a post against it (`authorize_event_signal`), and the owner, who asks the
//! signal's page to show it (`event_signal_key`). An agent that made the signal never sees it.

use std::sync::Arc;

use hmac::{Hmac, Mac};
use model::{Actor, EventSignal, by_remit, event_key};
use secrets::Sealed;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::Pipeline;
use chains::chain_core::{first_line, new_id};
use chains::request::Service;
use chains::request::{Done, Request};
use chains::{Error, Result};

/// The longest note an event signal may carry.
const EVENT_SIGNAL_NOTE_MAX: usize = 200;
/// What begins every key of an event signal's door.
const EVENT_SIGNAL_KEY_PREFIX: &str = "evk_";

/// An event signal as the Assistant's tools take it: its name, the last part of its key (event:<name>) and of its door's
/// URL, which never changes, and what it is for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EventSignalInput {
    pub name: String,
    pub note: String,
}

/// es with note, cleaned up, as its settings: the owner's own words kept as he said them.
fn with_note(mut es: EventSignal, note: &str) -> EventSignal {
    es.note = first_line(note.trim(), EVENT_SIGNAL_NOTE_MAX);
    es
}

impl EventSignalInput {
    /// The event signal in checked: name and settings cleaned up, with no id, author or time of its own yet.
    pub(crate) fn event_signal(&self) -> Result<EventSignal> {
        let name = self.name.trim().to_lowercase();
        if !is_event_signal_name(&name) {
            return Err(Error::refused(format!(
                "the name {:?} cannot name an event signal: use lowercase letters, digits and dashes, at most 40, starting with a letter or digit, e.g. gitlab-billing",
                self.name
            )));
        }
        Ok(with_note(
            EventSignal {
                name,
                ..EventSignal::default()
            },
            &self.note,
        ))
    }
}

/// Names the event signal a sealed key belongs to, so a key moved to another signal does not open there. The prefix is
/// the one the keys stored so far were sealed under, when the record was a webhook: a sealed key opens only with the
/// additional data it was sealed with.
fn event_signal_aad(id: &str) -> String {
    format!("webhook/{id}")
}

fn no_event_signal(id: &str) -> Error {
    Error::refused(format!("no event signal {id:?}: list_event_signals shows the ids"))
}

/// What a post shows of an event signal's key: the key itself, or messages signed with it. A sender that never sends the
/// key signs in its own way (GitHub signs the body, Stripe and Slack a timestamp and the body): the door builds the message
/// each signed, and whether the stamp is fresh is its to say.
#[derive(Debug, Clone, Default)]
pub struct KeyProof {
    pub key: String,
    pub signed: Vec<Signed>,
}

/// A message and the HMAC-SHA256 a sender gave of it.
#[derive(Debug, Clone, Default)]
pub struct Signed {
    pub message: Vec<u8>,
    pub mac: Vec<u8>,
}

impl KeyProof {
    /// Whether the post holds key: it carries it, or one of its signatures is of its message made with it. Every
    /// comparison takes the same time whatever it compares.
    pub fn opens(&self, key: &str) -> bool {
        let mut signed = false;
        for s in &self.signed {
            let Ok(mut m) = Hmac::<Sha256>::new_from_slice(key.as_bytes()) else {
                continue;
            };
            m.update(&s.message);
            if !s.mac.is_empty() && m.verify_slice(&s.mac).is_ok() {
                signed = true;
            }
        }
        bool::from(key.as_bytes().ct_eq(self.key.as_bytes())) || signed
    }
}

impl Pipeline {
    /// Makes an event signal from the owner's words, with a key of its own, by who asked, and tells the channels.
    pub(crate) async fn create_event_signal(&self, input: &EventSignalInput, by: Actor) -> Result<EventSignal> {
        self.open_event_signal(input.event_signal()?, by).await
    }

    /// Stores es, checked, with a key of its own made for it, and tells the channels.
    pub(crate) async fn open_event_signal(&self, mut es: EventSignal, by: Actor) -> Result<EventSignal> {
        let now = self.now();
        (es.id, es.by, es.created_at, es.updated_at) = (new_id("es_"), by, now, now);
        let sealed = self
            .seal_new_key(EVENT_SIGNAL_KEY_PREFIX, &event_signal_aad(&es.id))
            .await?;
        let name = es.name.clone();
        let es = match self.store.update(move |tx| tx.create_event_signal(es, &sealed)).await {
            Err(store::Error::EventSignalName) => {
                return Err(Error::refused(format!(
                    "an event signal named {name:?} exists already: list_event_signals shows it"
                )));
            }
            done => done?,
        };
        self.notify.event_signal_updated(&es);
        Ok(es)
    }

    /// Stores what change makes of event signal id, read and written in one transaction so no change is lost, as by, and
    /// tells the channels. A change that returns a sealed key stores it with the record.
    pub(crate) async fn change_event_signal<F>(&self, id: &str, by: Actor, change: F) -> Result<EventSignal>
    where
        F: FnOnce(EventSignal) -> Result<(EventSignal, Option<Sealed>)> + Send + 'static,
    {
        let (key, now) = (id.to_owned(), self.now());
        let saved = self
            .store
            .update(move |tx| {
                let cur = match tx.event_signal(&key) {
                    Err(store::Error::NotFound) => return Err(no_event_signal(&key)),
                    cur => cur?,
                };
                let (mut next, sealed) = change(cur)?;
                (next.by, next.updated_at) = (by, now);
                Ok(match sealed {
                    Some(sealed) => tx.rotate_event_signal(next, &sealed)?,
                    None => tx.put_event_signal(next)?,
                })
            })
            .await?;
        self.notify.event_signal_updated(&saved);
        Ok(saved)
    }

    /// Replaces an event signal's note and, when kept is given, whether its events are kept on this machine. Its name is its key and never
    /// changes; a builtin one given back what ships follows Remit again.
    pub(crate) async fn update_event_signal(
        &self,
        id: &str,
        note: String,
        kept: Option<bool>,
        by: Actor,
    ) -> Result<EventSignal> {
        let shipped = self.shipped_signals();
        self.change_event_signal(id, by, move |es| {
            let mut next = with_note(es, &note);
            next.kept = kept.unwrap_or(next.kept);
            if next.builtin
                && let Some((_, want)) = shipped.0.iter().find(|(_, s)| s.name == next.name)
            {
                next.edited = next.note != first_line(want.note.trim(), EVENT_SIGNAL_NOTE_MAX);
            }
            Ok((next, None))
        })
        .await
    }

    /// Puts back the note a builtin event signal ships with, and none of the owner's; its key and pause stay.
    pub(crate) async fn reset_event_signal(&self, id: &str, by: Actor) -> Result<EventSignal> {
        let shipped = self.shipped_signals();
        self.change_event_signal(id, by, move |mut es| {
            let Some((_, want)) = shipped.0.iter().find(|(_, s)| s.name == es.name).filter(|_| es.builtin) else {
                return Err(Error::refused(format!(
                    "the event signal {} does not ship with Remit: there is no original to reset it to",
                    es.name
                )));
            };
            (es.note, es.edited) = (first_line(want.note.trim(), EVENT_SIGNAL_NOTE_MAX), false);
            Ok((es, None))
        })
        .await
    }

    /// Lets no writer in until it is resumed, whichever way it comes; resuming lets them in again. Its key stays.
    pub(crate) async fn pause_event_signal(&self, id: &str, paused: bool, by: Actor) -> Result<EventSignal> {
        self.change_event_signal(id, by, move |mut es| {
            es.paused = paused;
            Ok((es, None))
        })
        .await
    }

    /// Gives an event signal's door a new key: the old one stops opening it at once.
    pub(crate) async fn rotate_event_signal_key(&self, id: &str, by: Actor) -> Result<EventSignal> {
        let sealed = self
            .seal_new_key(EVENT_SIGNAL_KEY_PREFIX, &event_signal_aad(id))
            .await?;
        self.change_event_signal(id, by, move |es| Ok((es, Some(sealed)))).await
    }

    /// Removes an event signal, its key and its sensors: nothing writes it again. Its events stay. One that ships with
    /// Remit is never deleted.
    pub(crate) async fn delete_event_signal(&self, id: &str) -> Result<()> {
        let key = id.to_owned();
        let es = match self.store.read(move |r| r.event_signal(&key)).await {
            Err(store::Error::NotFound) => return Err(no_event_signal(id)),
            es => es?,
        };
        if es.builtin {
            return Err(Error::refused(format!(
                "the event signal {} is part of Remit and is never deleted: edit its settings, or reset them to what ships; pause it to let nothing in",
                es.name
            )));
        }
        let signal = event_key(&es.name);
        let on = signal.clone();
        let sensors = self.store.read(|r| r.sensors()).await?;
        let (key, gone) = (id.to_owned(), signal.clone());
        // nothing is left to write a signal that is gone
        self.store
            .update(move |tx| -> store::Result<()> {
                tx.delete_sensors_on(&gone)?;
                tx.delete_event_signal(&key)
            })
            .await?;
        for sn in sensors.iter().filter(|sn| sn.signal == on) {
            self.notify.sensor_deleted(&sn.id);
        }
        self.notify.event_signal_deleted(id);
        Ok(())
    }

    /// The owner stopping an event signal from letting writers in until he resumes it.
    pub async fn pause_event_signal_now(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "PauseEventSignal", move |o| async move {
            o.pause_event_signal(&id, true, by).await.map(|_| ())
        })
        .await
    }

    /// The owner letting a paused event signal take events again.
    pub async fn resume_event_signal_now(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "ResumeEventSignal", move |o| async move {
            o.pause_event_signal(&id, false, by).await.map(|_| ())
        })
        .await
    }

    /// Whether proof opens the door of the event signal called name: it exists, it is not paused and the proof is of its
    /// own key. Every way of failing gives the same answer, so a sender learns nothing about which names exist. An error is
    /// the server's: the key could not be opened, which is for the owner to fix by rotating it.
    pub async fn authorize_event_signal(&self, name: &str, proof: &KeyProof) -> Result<Option<EventSignal>> {
        let name = name.to_lowercase();
        let (es, sealed) = match self.store.read(move |r| r.event_signal_named(&name)).await {
            Err(store::Error::NotFound) => return Ok(None),
            found => found?,
        };
        if es.paused {
            return Ok(None);
        }
        let key = self
            .opened_key(&event_signal_aad(&es.id), &sealed)
            .map_err(|e| Error::refused(format!("the key of event signal {} {e}", es.name)))?;
        Ok(proof.opens(&key).then_some(es))
    }

    /// Event signal id's key, for the owner who asked to see it: the one place a channel is given it. It is read, never
    /// pushed, and the server's log says who asked.
    pub async fn event_signal_key(&self, id: &str, by: &Actor) -> Result<String> {
        chains::owner_acts::for_owner(chains::owner_acts::ACT_SEE_KEY, by)?;
        let key = id.to_owned();
        let sealed = match self.store.read(move |r| r.sealed_event_signal_key(&key)).await {
            Err(store::Error::NotFound) => return Err(no_event_signal(id)),
            sealed => sealed?,
        };
        let key = self.opened_key(&event_signal_aad(id), &sealed).map_err(|e| {
            Error::refused(format!(
                "the key {e}; or rotate it, and paste the new one where the old one was"
            ))
        })?;
        tracing::info!(signal = id, by = %by.kind, who = %by.who, "event signal key shown");
        Ok(key)
    }

    /// Makes each shipped event signal that is not there, with a key of its own, and brings the ones that are there up to
    /// what ships, unless the owner changed them. A signal the owner made under a shipped name becomes the shipped one
    /// without losing a word of his. A start that finds all as it should be writes nothing. Called after `load_shipped`.
    pub async fn seed_event_signals(&self) -> Result<()> {
        for (plugin, s) in self.shipped_signals().0.iter() {
            let input = EventSignalInput {
                name: s.name.clone(),
                note: s.note.clone(),
            };
            let mut want = input
                .event_signal()
                .map_err(|e| Error::refused(format!("the shipped event signal {}: {e}", s.name)))?;
            want.builtin = true;
            let name = want.name.clone();
            let cur = match self.store.read(move |r| r.event_signal_named(&name)).await {
                Err(store::Error::NotFound) => {
                    // a sensor the owner has no credential for yet reads nothing until he resumes the signal
                    want.paused = s.paused;
                    self.open_event_signal(want, by_remit("seed")).await?;
                    self.seed_sensor(plugin, s).await?;
                    continue;
                }
                found => found?.0,
            };
            if !cur.builtin {
                let note = want.note.clone();
                self.change_event_signal(&cur.id, by_remit("seed"), move |mut cur| {
                    cur.edited = cur.note != note;
                    cur.builtin = true;
                    Ok((cur, None))
                })
                .await?;
            } else if !cur.edited && cur.note != want.note {
                self.change_event_signal(&cur.id, by_remit("seed"), move |mut cur| {
                    (cur.note, cur.edited) = (want.note, false);
                    Ok((cur, None))
                })
                .await?;
            }
        }
        Ok(())
    }
}

/// Whether name may name an event signal: it is part of its key and of its door's URL, so it is short and plain —
/// lowercase letters, digits and dashes, at most 40, starting with a letter or digit.
pub fn is_event_signal_name(name: &str) -> bool {
    let plain = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    name.len() <= 40 && name.chars().next().is_some_and(plain) && name.chars().all(|c| plain(c) || c == '-')
}
