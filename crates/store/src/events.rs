//! Events
//!
//! A prompt event is also the message itself, waiting in the card's queue (the outbox): written "queued" in the
//! transaction of the change that sends it, taken "delivered" by the card's session when its turn on it starts,
//! "steered" when it is put into the turn the card is already in, or "dropped" when the session it waited for has ended
//! for good; the owner may take his back while it is still queued, and then it is deleted. So the activity and the queue are one record, and a message written is a message delivered, across any
//! crash.

use model::{
    Actor, ActorKind, Cut, Event, EventKind, EventStatus, ForeignJson, JobStatus, PromptStatus, SystemNote, TaskStatus,
    Time, ToolStatus, ToolTally, UnknownValue, holds_owner_words, owner_words_under_head, with_owner_words,
};
use rusqlite::{Params, Row, params, params_from_iter};

use crate::pack::stored;
use crate::store::{Error, Reader, Result, Tx, from_ms, ms};

macro_rules! event_cols {
    () => {
        concat!(
            "id, home, card_id, kind, tool_call_id, tool_kind, ",
            tool_name_read!(),
            ", status, title, text, by_kind, by_who, by_via, priority, data, created_at, updated_at, rev, ",
            older_shape_read!()
        )
    };
}

/// Whether an event was written before each shape migration 130 records began (`event_shapes`): the owner's words
/// headed by the line that says who speaks, and a stopped task's cut in its words alone. `scan_event` converts such an
/// event to today's shape; the boundary is the event's id, never a guess from what it holds.
macro_rules! older_shape_read {
    () => {
        "id < (SELECT from_id FROM event_shapes WHERE shape = 'owner_words'), \
         id < (SELECT from_id FROM event_shapes WHERE shape = 'task_cut')"
    };
}

/// An event's tool name as every read gives it. A tool call recorded before calls carried their name (migration 41) is
/// named from its title where the title names a tool: an MCP tool (`mcp__<server>__<tool>`, or `mcp.<server>.<tool>`
/// from before Claude Code ran every agent, read in today's form) or the Skill tool ("Load skill: <name>"); any other
/// title names no tool. The older shape is converted here, when read, as a migration never rewrites events.
macro_rules! tool_name_read {
    () => {
        "(CASE WHEN tool_name <> '' OR kind <> 'tool' THEN tool_name
  WHEN title GLOB 'mcp__*' THEN title
  WHEN title GLOB 'mcp.*' THEN 'mcp__' || replace(substr(title, 5), '.', '__')
  WHEN title GLOB 'Load skill: *' THEN 'Skill'
  ELSE '' END)"
    };
}

/// An event row whose kind, status or actor this build does not name: a newer build wrote it (ADR-0107).
struct UnreadableEvent {
    id: i64,
    why: UnknownValue,
}

/// Reads the event row holds, unpacking its packed values (`pack::stored`). by_kind '' is no actor; data that is NULL
/// or '' is none.
fn scan_event(r: &Row<'_>) -> rusqlite::Result<Result<Event, UnreadableEvent>> {
    let id: i64 = r.get(0)?;
    let unreadable = |why| UnreadableEvent { id, why };
    let by_kind: String = r.get(10)?;
    let by = if by_kind.is_empty() {
        None
    } else {
        Some(Actor {
            kind: match by_kind.parse() {
                Ok(kind) => kind,
                Err(why) => return Ok(Err(unreadable(why))),
            },
            who: r.get(11)?,
            via: r.get(12)?,
        })
    };
    let data = match stored(r, 14)? {
        Some(s) if !s.is_empty() => Some(
            ForeignJson::parse(&s)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(14, rusqlite::types::Type::Text, Box::new(e)))?,
        ),
        _ => None,
    };
    let kind: EventKind = match r.get::<_, String>(3)?.parse() {
        Ok(kind) => kind,
        Err(why) => return Ok(Err(unreadable(why))),
    };
    let status = match EventStatus::read(kind, &r.get::<_, String>(7)?) {
        Ok(status) => status,
        Err(why) => return Ok(Err(unreadable(why))),
    };
    let e = Event {
        id,
        home: r.get(1)?,
        card_id: r.get(2)?,
        status,
        tool_call_id: r.get(4)?,
        tool_kind: r.get(5)?,
        tool_name: r.get(6)?,
        title: r.get(8)?,
        text: stored(r, 9)?.unwrap_or_default(),
        by,
        priority: r.get(13)?,
        data,
        created_at: from_ms(r.get(15)?),
        updated_at: from_ms(r.get(16)?),
        rev: r.get(17)?,
    };
    Ok(Ok(in_todays_shape(e, r.get(18)?, r.get(19)?)))
}

/// Event e as today's build writes it, when it was written before a shape began (`older_shape_read`): an owner's message
/// below `owner_words` loses the line that headed his words and is marked as his, and a stopped task below `task_cut`
/// keeps the cut its words say in its data. One already in today's shape (rewritten since) is left as it is.
fn in_todays_shape(mut e: Event, before_owner_words: bool, before_task_cut: bool) -> Event {
    match e.status {
        EventStatus::Prompt(_) if before_owner_words && !holds_owner_words(&e) => {
            let words =
                e.by.as_ref()
                    .filter(|by| by.for_owner())
                    .and_then(|by| owner_words_under_head(by, &e.text))
                    .map(str::to_owned);
            if let Some(words) = words {
                e.text = words;
                e.data = Some(with_owner_words(e.data.as_ref()));
            }
        }
        EventStatus::Task(TaskStatus::Stopped) if before_task_cut && Cut::of(e.data.as_ref()).is_none() => {
            if let Some(cut) = Cut::read_from_words(&e.text) {
                e.data = Some(cut.into_data(e.data.as_ref()));
            }
        }
        _ => {}
    }
    e
}

/// Data is stored as its JSON text, none as NULL.
fn data_text(data: Option<&ForeignJson>) -> Option<String> {
    data.map(ForeignJson::text)
}

impl Reader<'_> {
    /// The event row holds; `Error::Unreadable` for one this build cannot read.
    fn read_event(&self, row: &Row<'_>) -> Result<Event> {
        scan_event(row)?.map_err(|e| self.left_out("Event", &e.id.to_string(), &e.why))
    }

    /// The events query reads, those this build cannot read left out.
    pub(crate) fn query_events(&self, query: &str, params: impl Params) -> Result<Vec<Event>> {
        let mut stmt = self.c.prepare_cached(query)?;
        let mut rows = stmt.query(params)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            match self.read_event(row) {
                Ok(e) => out.push(e),
                Err(Error::Unreadable { .. }) => {}
                Err(other) => return Err(other),
            }
        }
        Ok(out)
    }

    /// The one event query reads; `Error::NotFound` when it reads none, `Error::Unreadable` when this build cannot.
    fn one_event(&self, query: &str, params: impl Params) -> Result<Event> {
        let mut stmt = self.c.prepare_cached(query)?;
        let mut rows = stmt.query(params)?;
        match rows.next()? {
            Some(row) => self.read_event(row),
            None => Err(Error::NotFound),
        }
    }

    /// Runs a query that reads events newest first, as an index on id serves it with no sort, and returns them oldest
    /// first.
    fn newest_events(&self, query: &str, params: impl Params) -> Result<Vec<Event>> {
        let mut events = self.query_events(query, params)?;
        events.reverse();
        Ok(events)
    }

    /// The event id. `Error::NotFound` when there is none.
    pub fn event(&self, id: i64) -> Result<Event> {
        self.one_event(concat!("SELECT ", event_cols!(), " FROM events WHERE id=?"), [id])
    }

    pub fn tool_event(&self, card_id: &str, tool_call_id: &str) -> Result<Event> {
        self.one_event(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND tool_call_id=? ORDER BY id DESC LIMIT 1"
            ),
            [card_id, tool_call_id],
        )
    }

    /// The newest thing a card's agent did or was given to read: its newest event that is not a message still waiting
    /// in its queue. `Error::NotFound` when it has none.
    pub fn last_done(&self, card_id: &str) -> Result<Event> {
        self.one_event(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND NOT (kind=? AND status=?) ORDER BY id DESC LIMIT 1"
            ),
            [card_id, EventKind::Prompt.as_str(), PromptStatus::Queued.as_str()],
        )
    }

    /// The newest message a card's session took to open a turn (an event id), whether its agent answered anything after
    /// it (a message, a tool call, a thought), and when it was taken (its delivery, the turn's start). `Error::NotFound`
    /// when it has taken none.
    pub fn last_taken(&self, card_id: &str) -> Result<(i64, bool, Time)> {
        crate::store::found(
            self.c
                .prepare_cached(
                    "SELECT p.id, EXISTS(SELECT 1 FROM events a WHERE a.card_id=p.card_id AND a.id>p.id AND a.kind IN (?,?,?)), p.updated_at
		FROM events p WHERE p.card_id=? AND p.kind=? AND p.status=? ORDER BY p.id DESC LIMIT 1",
                )?
                .query_row(
                    [
                        EventKind::Message.as_str(),
                        EventKind::Tool.as_str(),
                        EventKind::Thought.as_str(),
                        card_id,
                        EventKind::Prompt.as_str(),
                        PromptStatus::Delivered.as_str(),
                    ],
                    |r| Ok((r.get(0)?, r.get(1)?, from_ms(r.get(2)?))),
                ),
        )
    }

    /// A card's newest events, at most limit, oldest first.
    pub fn card_events(&self, card_id: &str, limit: i64) -> Result<Vec<Event>> {
        self.card_events_before(card_id, i64::MAX, limit)
    }

    /// A card's events with an id after after, the oldest first, at most limit: a page of its stream read forward, as a
    /// person reads their own session.
    pub fn card_events_after(&self, card_id: &str, after: i64, limit: i64) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND id>? ORDER BY id LIMIT ?"
            ),
            params![card_id, after, limit],
        )
    }

    /// The newest of a card's events older than the event before (its id), at most limit, oldest first: the page before
    /// the one a channel holds, read on events_card(card_id, id) as the newest page is.
    pub fn card_events_before(&self, card_id: &str, before: i64, limit: i64) -> Result<Vec<Event>> {
        self.newest_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND id<? ORDER BY id DESC LIMIT ?"
            ),
            params![card_id, before, limit],
        )
    }

    /// The newest of the messages sent to a card, older than the event before (its id), that accept accepts, and None
    /// when none does. It reads them newest first and stops at the first accepted: what accept looks for is in a
    /// message's data, which may be packed, so SQL cannot pick it.
    pub fn newest_prompt_before(
        &self,
        card_id: &str,
        before: i64,
        mut accept: impl FnMut(&Event) -> bool,
    ) -> Result<Option<Event>> {
        let mut stmt = self.c.prepare_cached(concat!(
            "SELECT ",
            event_cols!(),
            " FROM events WHERE card_id=? AND id<? AND kind=? ORDER BY id DESC"
        ))?;
        let mut rows = stmt.query(params![card_id, before, EventKind::Prompt.as_str()])?;
        while let Some(row) = rows.next()? {
            let e = match self.read_event(row) {
                Ok(e) => e,
                Err(Error::Unreadable { .. }) => continue,
                Err(other) => return Err(other),
            };
            if accept(&e) {
                return Ok(Some(e));
            }
        }
        Ok(None)
    }

    /// Every message queued for a card and not yet taken, card by card in the order each takes them.
    pub fn queued_prompts(&self) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE kind=? AND status=? ORDER BY card_id, priority DESC, id"
            ),
            [EventKind::Prompt.as_str(), PromptStatus::Queued.as_str()],
        )
    }

    /// The messages queued for a card, in the order its session takes them.
    pub fn queue(&self, card_id: &str) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE kind=? AND status=? AND card_id=? ORDER BY priority DESC, id"
            ),
            [EventKind::Prompt.as_str(), PromptStatus::Queued.as_str(), card_id],
        )
    }

    /// The last message a card's session took before the event id: the one its turn was answering.
    pub fn prompt_before(&self, card_id: &str, id: i64) -> Result<Event> {
        self.one_event(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND id<? AND kind=? AND status=? ORDER BY id DESC LIMIT 1"
            ),
            params![
                card_id,
                id,
                EventKind::Prompt.as_str(),
                PromptStatus::Delivered.as_str()
            ],
        )
    }

    /// The replacement whose brief the session has yet to finish reading. Older new_session events without a session
    /// association are not recoveries. Two pending records for one session are inconsistent. The session a record names
    /// is in its data, which may be packed, so SQL cannot pick it: the card's records are read and unpacked first.
    pub fn pending_session_recovery(&self, card_id: &str, session_id: &str) -> Result<Event> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Names {
            #[serde(default)]
            session_id: String,
        }
        let names = |e: &Event| {
            e.data
                .as_ref()
                .and_then(|d| serde_json::from_value::<Names>(d.0.clone()).ok())
                .is_some_and(|n| n.session_id == session_id)
        };
        let mut events: Vec<Event> = self
            .query_events(
                concat!(
                    "SELECT ",
                    event_cols!(),
                    " FROM events WHERE card_id=? AND kind=? AND status=? ORDER BY id"
                ),
                [card_id, EventKind::System.as_str(), SystemNote::NewSession.as_str()],
            )?
            .into_iter()
            .filter(names)
            .collect();
        match (events.pop(), events.is_empty()) {
            (None, _) => Err(Error::NotFound),
            (Some(e), true) => Ok(e),
            (Some(_), false) => Err(Error::Refused(format!(
                "multiple pending recoveries for card {card_id} session {session_id}"
            ))),
        }
    }

    /// The messages sent to a card since its last turn ended (its newest stop event), oldest first: the one its open turn
    /// is on and those waiting for it. A dropped message is left out, as nobody will read it.
    pub fn messages_since_turn_end(&self, card_id: &str) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND kind=? AND status<>?
		AND id>COALESCE((SELECT MAX(id) FROM events WHERE card_id=? AND kind=?), 0) ORDER BY id"
            ),
            [
                card_id,
                EventKind::Prompt.as_str(),
                PromptStatus::Dropped.as_str(),
                card_id,
                EventKind::Stop.as_str(),
            ],
        )
    }

    /// The first message a card was sent: the brief it started with.
    pub fn opening_prompt(&self, card_id: &str) -> Result<Event> {
        self.one_event(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND kind=? ORDER BY id LIMIT 1"
            ),
            [card_id, EventKind::Prompt.as_str()],
        )
    }

    /// A chain's jobs that have not ended, oldest first.
    pub fn running_jobs(&self, chain_id: &str) -> Result<Vec<Event>> {
        self.query_events(
            RUNNING_QUERY,
            [chain_id, EventKind::Job.as_str(), JobStatus::Running.as_str()],
        )
    }

    /// The background tasks of home's cards (a chain's, or the Assistant's) that have not ended, oldest first.
    pub fn running_tasks(&self, home: &str) -> Result<Vec<Event>> {
        self.query_events(
            RUNNING_QUERY,
            [home, EventKind::Task.as_str(), TaskStatus::Running.as_str()],
        )
    }

    /// The background tasks of home's cards that were stopped at or after since, oldest first.
    pub fn stopped_tasks(&self, home: &str, since: Time) -> Result<Vec<Event>> {
        self.query_events(
            STOPPED_QUERY,
            params![home, EventKind::Task.as_str(), TaskStatus::Stopped.as_str(), ms(since)],
        )
    }

    /// A card's tool calls after event id after that have not ended (neither completed nor failed), oldest first.
    pub fn open_tool_calls(&self, card_id: &str, after: i64) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE card_id=? AND id>? AND kind=? AND status NOT IN (?, ?) ORDER BY id"
            ),
            params![
                card_id,
                after,
                EventKind::Tool.as_str(),
                ToolStatus::Completed.as_str(),
                ToolStatus::Failed.as_str()
            ],
        )
    }

    /// The tool calls of every card in home whose tool name begins with prefix (an MCP tool's is
    /// mcp__<server>__<tool>), oldest first.
    pub fn tool_calls_named(&self, home: &str, prefix: &str) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE home=? AND kind=? AND ",
                tool_name_read!(),
                " GLOB ? ORDER BY id"
            ),
            params![home, EventKind::Tool.as_str(), format!("{}*", glob_literal(prefix))],
        )
    }

    /// Counts the tool calls of the cards of home per card and tool, by name alone: what a judge reads of what each agent
    /// of a chain ran (goal.rs). A call whose title names no tool (`tool_name_read!`) is not counted.
    pub fn tool_tallies_of(&self, home: &str) -> Result<Vec<ToolTally>> {
        let mut stmt = self.c.prepare_cached(concat!(
            "SELECT name, card_id, COUNT(*), SUM(status=?) FROM (SELECT ",
            tool_name_read!(),
            " AS name, card_id, status FROM events WHERE home=? AND kind=?) WHERE name <> '' GROUP BY 1, 2"
        ))?;
        let rows = stmt.query_map(
            params![ToolStatus::Failed.as_str(), home, EventKind::Tool.as_str()],
            |r| {
                Ok(ToolTally {
                    name: r.get(0)?,
                    title: String::new(),
                    card_id: r.get(1)?,
                    calls: r.get(2)?,
                    failed: r.get(3)?,
                })
            },
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Counts every card's tool calls per tool that has a name (`tool_name_read!`). A Skill's title, which holds the skill
    /// loaded, is kept; for any other tool the title (a command, a path) would only split its row.
    pub fn tool_tallies(&self) -> Result<Vec<ToolTally>> {
        let mut stmt = self.c.prepare_cached(concat!(
            "SELECT name, CASE WHEN name = 'Skill' THEN title ELSE '' END, card_id, COUNT(*), SUM(status=?)
		FROM (SELECT ",
            tool_name_read!(),
            " AS name, title, card_id, status FROM events WHERE kind=?) WHERE name <> '' GROUP BY 1, 2, 3"
        ))?;
        let rows = stmt.query_map([ToolStatus::Failed.as_str(), EventKind::Tool.as_str()], |r| {
            Ok(ToolTally {
                name: r.get(0)?,
                title: r.get(1)?,
                card_id: r.get(2)?,
                calls: r.get(3)?,
                failed: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The newest events of every card in a chain, at most limit, oldest first: what the owner's views of the chain
    /// replay.
    pub fn chain_events(&self, chain_id: &str, limit: i64) -> Result<Vec<Event>> {
        self.newest_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE home=? ORDER BY id DESC LIMIT ?"
            ),
            params![chain_id, limit],
        )
    }

    /// What the owner said in a chain, to any of its agents: his messages and the Assistant's on his words, oldest
    /// first.
    pub fn owner_prompts(&self, chain_id: &str) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "SELECT ",
                event_cols!(),
                " FROM events WHERE home=? AND kind=? AND by_kind IN (?, ?) ORDER BY id"
            ),
            [
                chain_id,
                EventKind::Prompt.as_str(),
                ActorKind::Owner.as_str(),
                ActorKind::Assistant.as_str(),
            ],
        )
    }
}

/// Reads a home's events of one kind that are running, its kind's running status bound. The WHERE implies that of the
/// partial index events_running (status 'running'), which SQLite sees from the bound value, so it reads them alone
/// however many events the home has.
const RUNNING_QUERY: &str = concat!(
    "SELECT ",
    event_cols!(),
    " FROM events WHERE home=? AND kind=? AND status=? ORDER BY id"
);

/// Reads a home's events of one kind that were stopped, from a time on, on the partial index events_stopped.
const STOPPED_QUERY: &str = concat!(
    "SELECT ",
    event_cols!(),
    " FROM events WHERE home=? AND kind=? AND status=? AND updated_at>=? ORDER BY id"
);

impl Tx<'_> {
    /// Inserts e and returns it with its id, at revision 1.
    pub fn add_event(&self, mut e: Event) -> Result<Event> {
        e.rev = 1;
        // by_kind '' is no actor (`scan_event`)
        let (by_kind, by_who, by_via) =
            e.by.as_ref()
                .map_or(("", "", ""), |b| (b.kind.as_str(), b.who.as_str(), b.via.as_str()));
        self.exec(
            "INSERT INTO events(home, card_id, kind, tool_call_id, tool_kind, tool_name, status, title, text, by_kind, by_who, by_via, priority, data, created_at, updated_at)
		VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                e.home,
                e.card_id,
                e.kind().as_str(),
                e.tool_call_id,
                e.tool_kind,
                e.tool_name,
                e.status.as_str(),
                e.title,
                e.text,
                by_kind,
                by_who,
                by_via,
                e.priority,
                data_text(e.data.as_ref()),
                ms(e.created_at),
                ms(e.updated_at)
            ],
        )?;
        e.id = self.c.last_insert_rowid();
        Ok(e)
    }

    /// Rewrites the mutable fields of an existing event and returns it at its new revision. Its kind stays: a status of
    /// another kind changes nothing and fails with `Error::NotFound`.
    pub fn update_event(&self, mut e: Event) -> Result<Event> {
        e.rev = self.value(
            "UPDATE events SET status=?, title=?, text=?, tool_kind=?, data=?, updated_at=?, rev=rev+1 WHERE id=? AND kind=? RETURNING rev",
            params![
                e.status.as_str(),
                e.title,
                e.text,
                e.tool_kind,
                data_text(e.data.as_ref()),
                ms(e.updated_at),
                e.id,
                e.kind().as_str()
            ],
        )?;
        Ok(e)
    }

    /// The event query changes and returns, None when it changes none.
    fn changed_event(&self, query: &str, params: impl Params) -> Result<Option<Event>> {
        Ok(self.query_events(query, params)?.into_iter().next())
    }

    /// Takes the next message queued for a card: the highest priority, then the oldest. It is "delivered" from then on;
    /// None when none waits.
    pub fn take_prompt(&self, card_id: &str, at: Time) -> Result<Option<Event>> {
        self.changed_event(
            concat!(
                "UPDATE events SET status=?, updated_at=?, rev=rev+1 WHERE id = (SELECT id FROM events WHERE kind=? AND status=?",
                " AND card_id=? ORDER BY priority DESC, id LIMIT 1) RETURNING ",
                event_cols!()
            ),
            params![
                PromptStatus::Delivered.as_str(),
                ms(at),
                EventKind::Prompt.as_str(),
                PromptStatus::Queued.as_str(),
                card_id
            ],
        )
    }

    /// Takes message id out of its card's queue into the turn the card is in: it is "steered" from then on, read by a
    /// turn that was already running, and no turn starts on it. None when it is not queued any more (the card's session
    /// took it for a turn first).
    pub fn steer_prompt(&self, id: i64, at: Time) -> Result<Option<Event>> {
        self.changed_event(
            concat!(
                "UPDATE events SET status=?, updated_at=?, rev=rev+1 WHERE id=? AND kind=? AND status=? RETURNING ",
                event_cols!()
            ),
            params![
                PromptStatus::Steered.as_str(),
                ms(at),
                id,
                EventKind::Prompt.as_str(),
                PromptStatus::Queued.as_str()
            ],
        )
    }

    /// Puts message id, taken for a turn that failed, back in its card's queue with data as the message now carries it.
    /// It is the oldest of its priority, so the card reads it again before what was sent after it. None when it is not a
    /// message taken for a turn: still waiting, steered, dropped, or not a message at all.
    pub fn requeue_prompt(&self, id: i64, data: Option<&ForeignJson>, at: Time) -> Result<Option<Event>> {
        self.changed_event(
            concat!(
                "UPDATE events SET status=?, data=?, updated_at=?, rev=rev+1 WHERE id=? AND kind=? AND status=? RETURNING ",
                event_cols!()
            ),
            params![
                PromptStatus::Queued.as_str(),
                data_text(data),
                ms(at),
                id,
                EventKind::Prompt.as_str(),
                PromptStatus::Delivered.as_str()
            ],
        )
    }

    /// Takes message id back before any session reads it (ADR-0080): it is deleted, and returned as it was for its words
    /// to go back to the draft and the channels to be told. None when it is not queued any more: taken, steered or
    /// dropped, it has been read or never will be, and stays as the record of that.
    pub fn withdraw_prompt(&self, id: i64) -> Result<Option<Event>> {
        self.changed_event(
            concat!(
                "DELETE FROM events WHERE id=? AND kind=? AND status=? RETURNING ",
                event_cols!()
            ),
            params![id, EventKind::Prompt.as_str(), PromptStatus::Queued.as_str()],
        )
    }

    /// Deletes the events ids name and returns them as they were, oldest first, for the channels to be told. An id that
    /// names no event is not an error: what it named is gone already.
    pub fn delete_events(&self, ids: &[i64]) -> Result<Vec<Event>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let marks = vec!["?"; ids.len()].join(",");
        let mut gone = self.query_events(
            &format!(
                concat!("DELETE FROM events WHERE id IN ({}) RETURNING ", event_cols!()),
                marks
            ),
            params_from_iter(ids),
        )?;
        gone.sort_by_key(|e| e.id);
        Ok(gone)
    }

    /// Drops the messages queued for a card from priority min_priority up, and returns them: the session they waited for
    /// has ended for good, or the one starting replaces what they were.
    pub fn drop_queued(&self, card_id: &str, min_priority: i64, at: Time) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "UPDATE events SET status=?, updated_at=?, rev=rev+1 WHERE kind=? AND status=? AND card_id=? AND priority>=? RETURNING ",
                event_cols!()
            ),
            params![
                PromptStatus::Dropped.as_str(),
                ms(at),
                EventKind::Prompt.as_str(),
                PromptStatus::Queued.as_str(),
                card_id,
                min_priority
            ],
        )
    }

    /// Drops the messages queued for every card of a chain, and returns them: nothing waits on it any more.
    pub fn drop_chain_queue(&self, chain_id: &str, at: Time) -> Result<Vec<Event>> {
        self.query_events(
            concat!(
                "UPDATE events SET status=?, updated_at=?, rev=rev+1 WHERE kind=? AND status=? AND home=? RETURNING ",
                event_cols!()
            ),
            params![
                PromptStatus::Dropped.as_str(),
                ms(at),
                EventKind::Prompt.as_str(),
                PromptStatus::Queued.as_str(),
                chain_id
            ],
        )
    }
}

/// Escapes what GLOB reads as a pattern in s, so it matches s itself.
pub(crate) fn glob_literal(s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '*' | '?' | '[' | ']' => {
                b.push('[');
                b.push(c);
                b.push(']');
            }
            _ => b.push(c),
        }
    }
    b
}
