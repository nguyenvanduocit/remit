//! A block an agent shows in its stream (ADR-0091): a spec of parts the app draws with its own components (a choice, a
//! chart), checked once as the agent writes it (`check_block`) and stored as an event of its card. The owner's pick in a
//! choice (`pick_in_block`) changes the block and reaches the agent as one queued message by him (`pick_words`), which
//! replaces one from the same block the card has not read yet. Nothing waits on the pick: the agent goes on with its own.

use std::collections::HashSet;
use std::sync::Arc;

use model::{Actor, Card, Event, EventKind, EventStatus, ForeignJson, SystemNote};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use wire::app::{Block, BlockOption, BlockPart, BlockSpec, Delivery};

use crate::apply::Records;
use crate::chain_core::Core;
use crate::outbox::owner_message;
use crate::request::Request;
use crate::request::Service;
use crate::tools::{Effect, SelfCard, Tools};
use crate::wait::rfc3339;
use crate::{Error, Result};

/// The most one block may weigh as JSON, so a block fits the stream.
const MOST_BYTES: usize = 16 * 1024;

/// The title of the message a pick sends: the turn's title wherever it shows.
const PICK_TITLE: &str = "Your pick in a block";

/// show_block's input.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShowBlockInput {
    /// The block: a title and its parts. Every id (of a part, of an option) is unique within the block.
    pub spec: BlockSpec,
}

impl Core {
    /// Gives every agent show_block, on t: it writes only in the agent's own stream, so no capability gates it.
    pub fn add_block_tool(self: &Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let t = &mut t.serving(self);
        let me = me.clone();
        t.add(
            "show_block", Effect::Writes,
            format!(
                "Show the owner a block in your stream: options laid side by side (a `choice`, each option with a line of `detail` that compares it) or numbers drawn as a `chart` (bar or line, one value per label of `x` in each series). A block never waits: set `picked` to the option you go ahead with, and go on. When the owner picks another, you get his pick as a message from him, which you act on in your next turn. What the work cannot go on without is no block: ask it with ask_owner or ask_chair. A table needs no block: write it in Markdown. Labels are plain text. At most {MOST_BYTES} bytes as JSON. Answers the block's event id."
            ),
            move |o, i: ShowBlockInput| {
                let me = me.clone();
                async move {
                    let card = me.read(&o).await?;
                    let e = o.show_block(&card, i.spec).await?;
                    Ok(format!(
                        "Shown as block {}. Go on with what you picked: a change of the owner's reaches you as his message.",
                        e.id
                    ))
                }
            },
        );
    }

    /// Stores spec, once it passes `check_block`, as a block event of card, and pushes it to where the owner watches it.
    pub async fn show_block(&self, card: &Card, spec: BlockSpec) -> Result<Event> {
        check_block(&spec).map_err(Error::refused)?;
        let now = self.now();
        let block = Block {
            spec,
            picked_by: None,
            picked_at: None,
        };
        let e = Event {
            home: card.home.clone(),
            card_id: card.id.clone(),
            status: EventStatus::System(SystemNote::Block),
            title: block.spec.title.clone(),
            data: Some(block_data(&block)?),
            by: Some(model::by_agent(&card.id)),
            created_at: now,
            updated_at: now,
            ..Event::default()
        };
        let e = self
            .in_home(&card.home, || async {
                Ok(self.store.update(move |tx| tx.add_event(e)).await?)
            })
            .await?;
        self.notify.event_added(&e);
        Ok(e)
    }

    /// The owner picks `picked` in the choice `part` of the block event event_id: the block holds his pick, and its card
    /// is told in one queued message by him. Answers the block event as stored.
    pub async fn pick_in_block(
        self: &Arc<Self>,
        req: Request,
        event_id: i64,
        part: String,
        picked: Vec<String>,
    ) -> Result<Event> {
        let by = req.by.clone();
        self.act(req, "PickInBlock", move |o| async move {
            o.pick(event_id, &part, picked, by).await
        })
        .await
    }

    async fn pick(self: &Arc<Self>, event_id: i64, part: &str, picked: Vec<String>, by: Actor) -> Result<Event> {
        // held from the read to the store: the pick reads the block and the unread message the store then changes, and a
        // send cannot run inside the home's lock it takes itself
        let _picking = self.pick_mu.lock().await;
        let e = self.store.read(move |r| r.event(event_id)).await?;
        let block = block_of(&e).ok_or_else(|| Error::refused(format!("event {event_id} is no block")))?;
        let spec = picked_in(&block.spec, part, picked).map_err(Error::refused)?;
        let words = pick_words(&spec);
        let now = self.now();
        let block = Block {
            spec,
            picked_by: Some(by.clone()),
            picked_at: Some(rfc3339(now)),
        };
        let mut shown = e.clone();
        shown.data = Some(block_data(&block)?);
        shown.updated_at = now;
        let card_id = e.card_id.clone();
        let to = self.store.read(move |r| r.card(&card_id)).await?;
        let mut m = owner_message(&to.id, PICK_TITLE, &words, by);
        if let Some(rewritten) = self.replace_unread(&to, event_id, &m.text, shown.clone()).await? {
            return Ok(rewritten);
        }
        m.data = Some(ForeignJson(serde_json::json!({ "block": event_id })));
        let w = Records {
            events: vec![shown],
            ..Records::default()
        };
        // the card's own rule for taking a message refuses before anything is stored: the block keeps its pick
        self.owner_says(&to, m, Delivery::Queue, w).await?;
        Ok(self.store.read(move |r| r.event(event_id)).await?)
    }

    /// Rewrites, in one change, the block event as shown and the message from block event_id that card `to` has not read
    /// yet, to text. None when no such message waits: the card read it, or none was sent.
    async fn replace_unread(&self, to: &Card, event_id: i64, text: &str, shown: Event) -> Result<Option<Event>> {
        let (card_id, text) = (to.id.clone(), text.to_owned());
        let rewritten = self
            .in_home(&to.home, || async {
                Ok(self
                    .store
                    .update(move |tx| {
                        let Some(mut m) = tx.queue(&card_id)?.into_iter().find(|q| from_block(q, event_id)) else {
                            return Ok::<_, store::Error>(None);
                        };
                        m.text = text;
                        m.updated_at = shown.updated_at;
                        Ok(Some((tx.update_event(shown)?, tx.update_event(m)?)))
                    })
                    .await?)
            })
            .await?;
        Ok(rewritten.map(|(block, m)| {
            self.notify.event_updated(&m);
            self.notify.event_updated(&block);
            block
        }))
    }
}

/// Whether queued message m is the owner's pick in block event_id.
fn from_block(m: &Event, event_id: i64) -> bool {
    m.kind() == EventKind::Prompt
        && m.data
            .as_ref()
            .and_then(|d| serde_json::from_value::<Value>(d.0.clone()).ok())
            .and_then(|d| d.get("block").and_then(Value::as_i64))
            == Some(event_id)
}

/// The block event e holds; none for any other event.
pub fn block_of(e: &Event) -> Option<Block> {
    if e.status != EventStatus::System(SystemNote::Block) {
        return None;
    }
    let mut data: Value = serde_json::from_value(e.data.as_ref()?.0.clone()).ok()?;
    serde_json::from_value(data.get_mut("block")?.take()).ok()
}

/// The data a block event stores: `{"block": Block}`.
fn block_data(block: &Block) -> Result<ForeignJson> {
    Ok(ForeignJson(serde_json::json!({ "block": block })))
}

/// Whether spec may be shown; the refusal names the place that is wrong (`parts[1].series[0].values: 9 values, x has
/// 10`), so the agent fixes it in the same turn.
pub fn check_block(spec: &BlockSpec) -> std::result::Result<(), String> {
    let bytes = serde_json::to_vec(spec).map_err(|e| e.to_string())?.len();
    if bytes > MOST_BYTES {
        return Err(format!("the block is {bytes} bytes as JSON: at most {MOST_BYTES}"));
    }
    named("title", &spec.title)?;
    if spec.parts.is_empty() {
        return Err("parts: none: a block holds at least one".to_owned());
    }
    let mut ids = HashSet::new();
    for (n, part) in spec.parts.iter().enumerate() {
        let at = format!("parts[{n}]");
        match part {
            BlockPart::Choice {
                id,
                label,
                options,
                many,
                picked,
            } => {
                unique(&mut ids, &format!("{at}.id"), id)?;
                named(&format!("{at}.label"), label)?;
                if options.len() < 2 {
                    return Err(format!(
                        "{at}.options: {} options, a choice offers at least 2",
                        options.len()
                    ));
                }
                for (k, o) in options.iter().enumerate() {
                    unique(&mut ids, &format!("{at}.options[{k}].id"), &o.id)?;
                    named(&format!("{at}.options[{k}].label"), &o.label)?;
                }
                picks_fit(options, *many, picked).map_err(|why| format!("{at}.picked: {why}"))?;
            }
            BlockPart::Chart {
                id, label, x, series, ..
            } => {
                unique(&mut ids, &format!("{at}.id"), id)?;
                named(&format!("{at}.label"), label)?;
                if x.is_empty() {
                    return Err(format!("{at}.x: none: a chart has at least one label"));
                }
                for (k, l) in x.iter().enumerate() {
                    named(&format!("{at}.x[{k}]"), l)?;
                }
                if series.is_empty() {
                    return Err(format!("{at}.series: none: a chart has at least one"));
                }
                for (k, s) in series.iter().enumerate() {
                    named(&format!("{at}.series[{k}].name"), &s.name)?;
                    if s.values.len() != x.len() {
                        return Err(format!(
                            "{at}.series[{k}].values: {} values, x has {}",
                            s.values.len(),
                            x.len()
                        ));
                    }
                    if let Some(v) = s.values.iter().position(|v| !v.is_finite()) {
                        return Err(format!("{at}.series[{k}].values[{v}]: not a finite number"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn named(at: &str, label: &str) -> std::result::Result<(), String> {
    if label.trim().is_empty() {
        return Err(format!("{at}: empty"));
    }
    Ok(())
}

fn unique(ids: &mut HashSet<String>, at: &str, id: &str) -> std::result::Result<(), String> {
    named(at, id)?;
    if !ids.insert(id.to_owned()) {
        return Err(format!("{at}: {id:?} is used twice: ids are unique within a block"));
    }
    Ok(())
}

/// Whether picked names options of a choice, each once, and one of them unless it takes many.
fn picks_fit(options: &[BlockOption], many: bool, picked: &[String]) -> std::result::Result<(), String> {
    if !many && picked.len() != 1 {
        return Err(format!("{} picked: a choice that is not `many` has one", picked.len()));
    }
    let mut seen = HashSet::new();
    for p in picked {
        if !options.iter().any(|o| &o.id == p) {
            return Err(format!("{p:?} is no option of it"));
        }
        if !seen.insert(p) {
            return Err(format!("{p:?} is picked twice"));
        }
    }
    Ok(())
}

/// spec with picked as the picks of its choice part.
fn picked_in(spec: &BlockSpec, part: &str, picked: Vec<String>) -> std::result::Result<BlockSpec, String> {
    let mut spec = spec.clone();
    let Some((options, many, picks)) = spec.parts.iter_mut().find_map(|p| match p {
        BlockPart::Choice {
            id,
            options,
            many,
            picked: now,
            ..
        } if id == part => Some((&*options, *many, now)),
        _ => None,
    }) else {
        return Err(format!("{part:?} is no choice of this block"));
    };
    picks_fit(options, many, &picked)?;
    *picks = picked;
    Ok(spec)
}

/// What the agent reads of the owner's picks in spec: the block's title and, for every choice, its label and the labels
/// of what is picked. Every choice, so a message rewritten by a later pick in another choice still carries the first.
fn pick_words(spec: &BlockSpec) -> String {
    let picks: Vec<String> = spec
        .parts
        .iter()
        .filter_map(|p| match p {
            BlockPart::Choice {
                label, options, picked, ..
            } => {
                let labels: Vec<&str> = options
                    .iter()
                    .filter(|o| picked.contains(&o.id))
                    .map(|o| o.label.as_str())
                    .collect();
                let pick = if labels.is_empty() {
                    "none of them".to_owned()
                } else {
                    labels.join(", ")
                };
                Some(format!("for \"{label}\" I pick: {pick}"))
            }
            BlockPart::Chart { .. } => None,
        })
        .collect();
    let go_on = if picks.len() == 1 {
        "Go on with this pick."
    } else {
        "Go on with these picks."
    };
    format!("In your block \"{}\", {}. {go_on}", spec.title, picks.join("; "))
}
