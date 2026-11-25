//! What a session's agent streams becomes events and card changes: its words and thoughts are joined into one event per
//! run of chunks, each chunk pushed at once as a `EventDelta` and the row written in batches; its tool calls are one event
//! each, kept as they change; what the card shows of the moment (its activity, its context fill) is held on the run and
//! pushed at most every `SHOW_EVERY`.

use std::sync::Arc;
use std::time::Duration;

use model::{Card, Event, EventStatus, ForeignJson};
use serde_json::{Map, Value};
use wire::app::{CardShown, EventDelta};

use crate::chain_core::Core;
use crate::health::elapsed;
use crate::session::{Run, RunState};

/// Database writes of a growing message.
const SAVE_EVERY: Duration = Duration::from_millis(500);
/// Pushes of what a card shows of the moment (`show`).
const SHOW_EVERY: Duration = Duration::from_secs(1);

impl RunState {
    /// What the card shows now, noting that it was pushed at now.
    fn shown(&mut self, now: model::Time) -> CardShown {
        self.pushed_at = now;
        let c = &self.card;
        CardShown {
            id: c.id.clone(),
            home: c.home.clone(),
            rev: c.rev,
            activity: c.activity.clone(),
            ctx_used: c.ctx_used,
            ctx_size: c.ctx_size,
        }
    }
}

impl Core {
    /// Turns what r's agent streams into events and card changes.
    pub async fn on_update(self: &Arc<Self>, r: &Arc<Run>, u: agent::Update) {
        match u {
            agent::Update::MessageChunk(text) => self.append_text(r, EventStatus::Message, &text).await,
            agent::Update::ThoughtChunk(text) => self.append_text(r, EventStatus::Thought, &text).await,
            agent::Update::ToolCall(t) => self.on_tool_call(r, t).await,
            agent::Update::Plan(plan) => self.display(&r.card_id(), move |c| c.plan.clone_from(&plan)).await,
            agent::Update::Usage(u) => {
                let (used, size) = (
                    i64::try_from(u.used).unwrap_or(i64::MAX),
                    i64::try_from(u.size).unwrap_or(i64::MAX),
                );
                self.show(r, |c| (c.ctx_used, c.ctx_size) = (used, size));
            }
            agent::Update::Note(n) => self.add_note(r, n).await,
        }
    }

    /// Records in r's stream a note of what Claude Code did to its session beside the agent's work: a system event whose
    /// status names the note's kind, after the message it came during.
    async fn add_note(self: &Arc<Self>, r: &Arc<Run>, n: wire::runtime::Note) {
        self.close_stream(r).await;
        let (home, card_id) = {
            let mut s = r.state();
            if n.kind == wire::runtime::NoteKind::Compacted {
                // the agent holds a summary of what it was given now: its next turns may be given it again
                s.recalled.clear();
            }
            (s.card.home.clone(), s.card.id.clone())
        };
        self.note(Event {
            home,
            card_id,
            status: EventStatus::System(n.kind.into()),
            title: n.title,
            text: n.text,
            ..Event::default()
        })
        .await;
    }

    /// Records a tool call of r's agent as it now is: a new event when it starts, the same event when it changes.
    async fn on_tool_call(self: &Arc<Self>, r: &Arc<Run>, t: wire::runtime::ToolCall) {
        let (home, card_id) = {
            let s = r.state();
            (s.card.home.clone(), s.card.id.clone())
        };
        let repeats = self.repeats_of(r, &t);
        let (key, call) = (card_id.clone(), t.id.clone());
        let known = self.store.read(move |s| s.tool_event(&key, &call)).await;
        let data = tool_data(&t.input, &t.output);
        let title = t.title.clone();
        match known {
            Ok(mut e) => {
                e.status = EventStatus::Tool(t.status);
                e.title = t.title;
                e.tool_kind = t.kind.as_str().to_owned();
                e.data = data;
                self.save_event(e).await;
            }
            Err(_) => {
                self.close_stream(r).await;
                self.note(Event {
                    home,
                    card_id,
                    status: EventStatus::Tool(t.status),
                    tool_call_id: t.id,
                    tool_kind: t.kind.as_str().to_owned(),
                    tool_name: t.name,
                    title: t.title,
                    data,
                    ..Event::default()
                })
                .await;
            }
        }
        self.show(r, move |c| c.activity.clone_from(&title));
        self.repeat_rail(r, repeats).await;
    }

    /// Stores e as it now is and tells the channels; a failure is logged.
    pub async fn save_event(&self, mut e: Event) {
        e.updated_at = self.now();
        match self.store.update(move |tx| tx.update_event(e)).await {
            Ok(saved) => self.notify.event_updated(&saved),
            Err::<_, store::Error>(err) => tracing::error!(%err, "update event"),
        }
    }

    /// Joins consecutive chunks of one kind into one event, so a message is one row, not hundreds. Each chunk goes out at
    /// once as a delta; the row is written at most every `SAVE_EVERY` and when the stream closes.
    async fn append_text(self: &Arc<Self>, r: &Arc<Run>, status: EventStatus, text: &str) {
        if text.is_empty() {
            return;
        }
        let same = r.state().stream.as_ref().is_some_and(|e| e.status == status);
        if !same {
            self.close_stream(r).await;
            let (home, card_id) = {
                let mut s = r.state();
                // the turn's output is its last message, the report the briefs ask for: what an agent said on the way
                // there stays in its events, and a message followed only by tool calls is still the last one
                if status == EventStatus::Message {
                    text.clone_into(&mut s.turn);
                }
                (s.card.home.clone(), s.card.id.clone())
            };
            let added = self
                .add_event(Event {
                    home,
                    card_id,
                    status,
                    text: text.to_owned(),
                    ..Event::default()
                })
                .await;
            // with no stored event to grow, the next chunk opens one again
            let e = match added {
                Ok(e) => e,
                Err(err) => return tracing::error!(card = %r.card_id(), %err, "stream event"),
            };
            {
                let mut s = r.state();
                s.stream = Some(e);
                s.stream_len = utf16_len(text);
                s.saved_at = self.now();
            }
            self.show(r, |c| c.activity = tail_line(text, 90));
            return;
        }
        let now = self.now();
        let (e, offset, save) = {
            let mut s = r.state();
            if status == EventStatus::Message {
                s.turn.push_str(text);
            }
            let offset = s.stream_len;
            s.stream_len = s.stream_len.saturating_add(utf16_len(text));
            let save = elapsed(s.saved_at, now) >= SAVE_EVERY;
            if save {
                s.saved_at = now;
            }
            let Some(cur) = s.stream.as_mut() else { return };
            cur.text.push_str(text);
            (cur.clone(), offset, save)
        };
        self.notify.event_delta(&EventDelta {
            id: e.id,
            home: e.home.clone(),
            card_id: e.card_id.clone(),
            kind: status.kind(),
            text: text.to_owned(),
            offset,
        });
        let activity = tail_line(&e.text, 90);
        if save {
            self.write_stream(r, e).await;
        }
        self.show(r, move |c| c.activity.clone_from(&activity));
    }

    /// Writes the current message or thought whole and ends it, and returns it; none when none was open. The event pushed
    /// then is at a revision above any copy a channel holds of it, grown from deltas or read while it streamed.
    pub async fn close_stream(self: &Arc<Self>, r: &Arc<Run>) -> Option<Event> {
        let cur = r.state().stream.take()?;
        if let Some(e) = self.write_stream(r, cur.clone()).await {
            self.notify.event_updated(&e);
        }
        if cur.status == EventStatus::Message {
            let activity = tail_line(&cur.text, 90);
            self.show(r, move |c| c.activity.clone_from(&activity));
        }
        Some(cur)
    }

    /// Stores a streaming event as it stands, and keeps the revision it is stored at on the stream (while it is still
    /// r's): a channel reading the event mid-stream gets that revision.
    async fn write_stream(&self, r: &Run, mut e: Event) -> Option<Event> {
        e.updated_at = self.now();
        let id = e.id;
        match self.store.update(move |tx| tx.update_event(e)).await {
            Ok(saved) => {
                let mut s = r.state();
                if let Some(cur) = s
                    .stream
                    .as_mut()
                    .filter(|cur| cur.id == saved.id && cur.rev < saved.rev)
                {
                    cur.rev = saved.rev;
                }
                Some(saved)
            }
            Err::<_, store::Error>(err) => {
                tracing::error!(id, %err, "update event");
                None
            }
        }
    }

    /// Changes what r's card shows of the moment (its activity, its context fill), which changes many times a turn. It is
    /// not a card write: the change is held on the run, pushed to the channels as a `CardShown` at most every
    /// `SHOW_EVERY` (the last one always goes, when its time comes), read with the card while the run lasts
    /// (`live_cards`), and stored with the card's next write, which starts from the run's copy (commit.rs). Only those
    /// fields: every other card change is a write.
    pub fn show(self: &Arc<Self>, r: &Arc<Run>, edit: impl FnOnce(&mut Card)) {
        let now = self.now();
        let wait = {
            let mut s = r.state();
            edit(&mut s.card);
            if s.pushing {
                return; // the push on its way carries this
            }
            let wait = SHOW_EVERY.saturating_sub(elapsed(s.pushed_at, now));
            if wait.is_zero() {
                let shown = s.shown(now);
                drop(s);
                self.notify.card_shown(&shown);
                return;
            }
            s.pushing = true;
            wait
        };
        let (o, run) = (Arc::clone(self), Arc::clone(r));
        let late = self.background(move |_| async move {
            tokio::time::sleep(wait).await;
            let shown = {
                let mut s = run.state();
                s.pushing = false;
                s.shown(o.now())
            };
            o.notify.card_shown(&shown);
        });
        if !late {
            r.state().pushing = false; // shutting down: the card's next write carries it
        }
    }

    /// Cards as they are now: those a session runs show its moment (`show`), which is not stored yet.
    pub fn live_cards(&self, mut cards: Vec<Card>) -> Vec<Card> {
        for c in &mut cards {
            if let Some(r) = self.run_of(&c.id) {
                let s = r.state();
                c.activity.clone_from(&s.card.activity);
                (c.ctx_used, c.ctx_size) = (s.card.ctx_used, s.card.ctx_size);
            }
        }
        cards
    }

    /// Stores a change to what a card shows that is a fact of it (its plan, the messages waiting for it): a write like
    /// any other, but only a display, which the next change brings up to date, so a failure is logged and the work goes
    /// on.
    pub async fn display(&self, card_id: &str, edit: impl Fn(&mut Card) + Send + Sync + 'static) {
        if let Err(err) = self.update_card(card_id, edit).await {
            tracing::warn!(card = %card_id, %err, "card display not stored");
        }
    }
}

/// How long s is as a JavaScript string.
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The last non-empty line of s, keeping its last limit characters: what an agent is writing right now.
pub fn tail_line(s: &str, limit: usize) -> String {
    let last = last_line(s);
    let n = last.chars().count();
    if n <= limit {
        return last.to_owned();
    }
    let mut out = String::from("…");
    out.extend(last.chars().skip(n.saturating_sub(limit.saturating_sub(1))));
    out
}

/// The last non-empty line of s, trimmed. It reads s from its end, as far as that line: it runs on every chunk of a
/// growing message.
fn last_line(s: &str) -> &str {
    s.rsplit('\n')
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}

/// What the UI shows of a tool call: its input, and its output, trimmed to 4000 bytes.
pub fn tool_data(input: &Map<String, Value>, output: &str) -> Option<ForeignJson> {
    let mut out = Map::new();
    if !input.is_empty() {
        out.insert("input".to_owned(), Value::Object(input.clone()));
    }
    if !output.is_empty() {
        let mut text = output.to_owned();
        if text.len() > 4000 {
            let mut end = 4000;
            while !text.is_char_boundary(end) {
                end = end.saturating_sub(1);
            }
            text.truncate(end);
            text.push_str("\n…");
        }
        out.insert("output".to_owned(), Value::String(text));
    }
    if out.is_empty() {
        return None;
    }
    Some(ForeignJson(Value::Object(out)))
}
