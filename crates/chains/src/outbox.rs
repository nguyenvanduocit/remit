//! The outbox: everything that reaches an agent is a prompt event queued for its card, written in the transaction of the
//! change that sends it (step's messages, apply.rs). The card's session takes its messages from the store one turn at a
//! time, the highest priority first; the mailbox only wakes the session. A message is never held in memory alone, so a
//! crash, a restart or a session closing under it loses none: what is still queued is handed over again.

use std::sync::Arc;

use model::{Actor, Card, CardRole, Event, EventStatus, ForeignJson, PromptStatus, SystemNote, Time};
use store::Tx;

use crate::admit::{Needs, fits};
use crate::apply::Records;
use crate::assistant::ASSISTANT_MARKER;
use crate::chain_core::Core;
use crate::session::Run;
use crate::step::{ChainCmd, SendCmd};
use crate::{Error, Result};
use model::ActorKind;

/// A prompt a change queues for a card: stored as the card's queued prompt event, in the same transaction.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Message {
    /// Card id.
    pub to: String,
    /// The card's home: set by the writer that stores it (a chain's, for step's messages).
    pub home: String,
    /// What the turn it starts is about, in the owner's words: the turn's title wherever it shows.
    pub title: String,
    pub text: String,
    /// Who sends it.
    pub by: Actor,
    pub priority: i64,
    pub data: Option<ForeignJson>,
    /// The sender needs it read now: in the turn its card is in, when it is in one (`steer`).
    pub steer: bool,
    /// The event of a message the card took for a turn that failed, sent again: it is queued as it was, not as a new one.
    pub again: i64,
    /// Text is the owner's own words (`owner_message`): its event's data marks them as his.
    pub owner_words: bool,
}

// Priorities of queued messages: a session reads its opening first (it knows nothing yet), then the owner's and what blocks
// an agent, then the rest, each in the order sent.
pub const PRIORITY_OTHER: i64 = 0;
/// The owner's messages, and the Assistant's, which it sends on his words.
pub const PRIORITY_OWNER: i64 = 1;
/// An agent is blocked until it is read, as when a worker waits for its Chair's answer: with the owner's.
pub const PRIORITY_BLOCKING: i64 = 1;
pub const PRIORITY_OPENING: i64 = model::PRIORITY_OPENING;

/// The note a message's data holds, when it holds one.
pub(crate) fn note_of(data: Option<&ForeignJson>) -> Option<String> {
    let fields = serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(data?.0.clone()).ok()?;
    fields
        .get("note")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// A message's data with lead, which starts with marker, as the note its reader meets first. A lead with the same
/// marker that an earlier change left there is replaced, and another note stays after it.
pub fn with_lead(data: Option<&ForeignJson>, lead: &str, marker: &str) -> Option<ForeignJson> {
    if lead.is_empty() {
        return data.cloned();
    }
    let note = note_of(data).unwrap_or_default();
    let lead = if !note.starts_with(marker) && !note.is_empty() {
        format!("{lead}\n\n{note}")
    } else {
        lead.to_owned()
    };
    Some(with_note(data, &lead))
}

/// What the agent reads of message e, handed over for a turn or steered into one: its note, then its text, the owner's
/// words headed by who speaks for him (`model::text_as_read`).
pub fn read_of(e: &Event) -> String {
    note_then(&note_of(e.data.as_ref()).unwrap_or_default(), &model::text_as_read(e))
}

/// A message's note, when it has one, then its text: how every reader meets the two.
pub fn note_then(note: &str, text: &str) -> String {
    if note.is_empty() {
        text.to_owned()
    } else {
        format!("{note}\n\n{text}")
    }
}

/// The message by sends to a card, with the priority its sender has: the owner's come first, and so do the
/// Assistant's, which it sends on his words.
pub fn new_message(to: &str, title: &str, text: &str, by: Actor) -> Message {
    let priority = if by.for_owner() { PRIORITY_OWNER } else { PRIORITY_OTHER };
    Message {
        to: to.to_owned(),
        title: title.to_owned(),
        text: text.to_owned(),
        by,
        priority,
        ..Message::default()
    }
}

/// The owner's words to card to, by him or by the Assistant on his behalf: stored as he wrote them and marked as his, so
/// its agent reads them headed as his instructions (`read_of`); title is the turn's, in the owner's words, said as the
/// Assistant's when it sends them.
pub fn owner_message(to: &str, title: &str, text: &str, by: Actor) -> Message {
    let title = if by.kind == model::ActorKind::Assistant {
        format!("{title}, through the Assistant")
    } else {
        title.to_owned()
    };
    Message {
        owner_words: true,
        ..new_message(to, &title, text, by)
    }
}

/// Message e, a prompt event, sent again as it was: its own event goes back in the queue.
pub(crate) fn sent_again(e: &Event) -> Message {
    Message {
        title: e.title.clone(),
        text: e.text.clone(),
        by: sent_by(e),
        priority: e.priority,
        data: e.data.clone(),
        again: e.id,
        owner_words: model::holds_owner_words(e),
        ..Message::default()
    }
}

/// How long the agent has to answer a steering request, which it answers at once.
const STEER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Who sends what Remit itself has to say: the Chair's opening, a wake, a resume, a submit's result.
pub fn by_remit() -> Actor {
    model::by_remit("")
}

/// Who sends the message that resumes a turn a server restart cut off: the same turn goes on, so it is not news that
/// replaces a check-back.
pub fn restart_by() -> Actor {
    model::by_remit("restart")
}

/// Who sent prompt e.
pub fn sent_by(e: &Event) -> Actor {
    e.by.clone().unwrap_or_default()
}

/// A message's data with note added beside what it holds. A message's note is what Remit tells its reader ahead of it,
/// such as the check-back the message cancelled: stored with the message, apart from its text, which stays its sender's
/// words.
pub fn with_note(data: Option<&ForeignJson>, note: &str) -> ForeignJson {
    ForeignJson::with_field(data, "note", serde_json::Value::String(note.to_owned()))
}

/// Stores message m in its card's queue, in the transaction tx of the change that sends it: a new event, or, for a
/// message sent again, the event it was, back in the queue.
pub fn queue_message(tx: &Tx<'_>, m: &Message, now: Time) -> Result<Event> {
    if m.home.is_empty() {
        return Err(Error::refused(format!("a message to {} names no home", m.to)));
    }
    let data = if m.owner_words {
        Some(model::with_owner_words(m.data.as_ref()))
    } else {
        m.data.clone()
    };
    if m.again == 0 {
        return Ok(tx.add_event(Event {
            home: m.home.clone(),
            card_id: m.to.clone(),
            status: EventStatus::Prompt(PromptStatus::Queued),
            by: Some(m.by.clone()),
            title: m.title.clone(),
            text: m.text.clone(),
            priority: m.priority,
            data,
            created_at: now,
            updated_at: now,
            ..Event::default()
        })?);
    }
    tx.requeue_prompt(m.again, data.as_ref(), now)?
        .ok_or_else(|| Error::refused("the message was sent again already"))
}

impl Core {
    /// Queues m for card `to`, with w in the same transaction, and hands it over. It is refused when the card can never
    /// read it (a child card reads none: its parent does), and by the chain (step) when the chain cannot take it.
    pub async fn send(&self, to: &Card, m: Message, w: Records) -> Result<Event> {
        // boxed: reports, replies and turn ends all send, often several times in one function, and the hand-over's
        // whole state would otherwise sit inline in each of those futures and their debug-build stack frames
        Box::pin(self.send_to(to, m, w)).await
    }

    /// `send`, unboxed.
    async fn send_to(&self, to: &Card, mut m: Message, w: Records) -> Result<Event> {
        match CardRole::of(to) {
            CardRole::Spawned => return Err(self.inside_parent(to).await),
            CardRole::Assistant => return self.send_to_standalone(to, m, w).await,
            CardRole::Judge(_) => return self.send_to_judge(to, m, w).await,
            CardRole::Chair | CardRole::Worker => {}
        }
        to.id.clone_into(&mut m.to);
        self.reachable(to, &m).await?;
        let sender = self.sender(&m.by, to).await;
        let cmd = ChainCmd::Send(SendCmd { msg: m, sender });
        let a = self.apply(&to.home, cmd, w).await?;
        a.sent
            .last()
            .cloned()
            .ok_or_else(|| Error::refused("the message was not queued"))
    }

    /// Queues the owner's message for a judge. A judge stands apart from the team and from the chain's course, so what is
    /// said to it leaves the chain as it is: a finished one stays finished, and nothing wakes. A message that has to open the
    /// judge's session waits for a provider, as every start does. It is stored under the chain's lock, as every change of
    /// the chain is, so that browsers see changes in the order they were made.
    async fn send_to_judge(&self, to: &Card, mut m: Message, mut w: Records) -> Result<Event> {
        to.id.clone_into(&mut m.to);
        to.home.clone_into(&mut m.home);
        self.reachable(to, &m).await?;
        let cs = self.state(&to.home).await?;
        if !self.session_open(&to.id) {
            let room = self.room(&to.home).await?;
            fits(&Needs::default(), &room)?;
        }
        w.messages.push(m);
        let a = {
            let held = cs.held.lock().await;
            if held.gone {
                return Err(Error::ChainDeleted);
            }
            self.commit(None, None, w).await?
        };
        self.hand_over(&a).await;
        a.sent
            .last()
            .cloned()
            .ok_or_else(|| Error::refused("the message was not queued"))
    }

    /// Whether a session will ever read a message to the card: it has one now, or one it can resume, or the message (or one
    /// already queued for it) opens a new one.
    pub async fn reachable(&self, to: &Card, m: &Message) -> Result<()> {
        if to.status() == &model::CardStatus::Stopped && self.handed_over(to).await? {
            // its work moved to another card in the same worktree: a session here would work beside it
            return Err(Error::refused(format!(
                "card {} handed its work over to another card: message that card (list_agents shows it)",
                to.id
            )));
        }
        if self.session_open(&to.id) || !to.session_id.is_empty() || m.priority == PRIORITY_OPENING {
            return Ok(());
        }
        let id = to.id.clone();
        let queue = self.store.read(move |r| r.queue(&id)).await?;
        if queue.iter().any(|e| e.priority == PRIORITY_OPENING) {
            return Ok(());
        }
        if matches!(CardRole::of(to), CardRole::Judge(_)) {
            return Err(Error::refused(format!(
                "{} never opened a session, so there is nothing to ask",
                to.name
            )));
        }
        Err(Error::refused(format!(
            "{} has no session to resume; restart it instead",
            to.name
        )))
    }

    /// Sends a message to a card: queued, and handed to its session, which is opened when the card has none.
    pub async fn deliver(&self, card_id: &str, title: &str, text: &str, by: Actor) -> Result<Event> {
        self.deliver_message(card_id, new_message(card_id, title, text, by))
            .await
    }

    /// `deliver` for a message its sender needs read now: a card that is in a turn reads it there, not after the turn,
    /// which may last many minutes (`steer`).
    pub async fn deliver_now(&self, card_id: &str, title: &str, text: &str, by: Actor) -> Result<Event> {
        let mut m = new_message(card_id, title, text, by);
        m.steer = true;
        self.deliver_message(card_id, m).await
    }

    async fn deliver_message(&self, card_id: &str, m: Message) -> Result<Event> {
        let id = card_id.to_owned();
        let c = self.store.read(move |r| r.card(&id)).await?;
        self.send(&c, m, Records::default()).await
    }

    /// Sends a card again the message its failed turn was on, as it was: the message's own event goes back in the queue,
    /// so the stream holds it once, where it was sent.
    pub async fn resend(&self, card_id: &str, prompt_id: i64) -> Result<()> {
        let id = card_id.to_owned();
        let (to, e) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.card(&id)?, r.event(prompt_id)?)))
            .await?;
        self.send(&to, sent_again(&e), Records::default()).await?;
        Ok(())
    }

    /// Hands a card the messages queued for it: its open session's loop takes them, or a session is opened. The queue is
    /// read under the card's lock (`sessions`), as the session opens: a card whose queue a stop emptied as it was
    /// dispatched gets no session.
    pub async fn dispatch(&self, card_id: &str) {
        if self.run_of(card_id).is_some_and(|r| r.mailbox.ring()) {
            return;
        }
        let _card = self.sessions.lock(card_id).await;
        let id = card_id.to_owned();
        let opened = match self.store.read(move |r| r.queue(&id)).await {
            Ok(q) if q.is_empty() => Ok(()),
            Ok(_) => self.open_locked(card_id).await,
            Err(err) => Err(err.into()),
        };
        if let Err(err) = opened {
            tracing::warn!(card = %card_id, %err, "open session");
        }
    }

    /// Gives a card with no open session a new one, under the card's lock (`open_locked`).
    pub async fn open(&self, card_id: &str) -> Result<()> {
        let _card = self.sessions.lock(card_id).await;
        self.open_locked(card_id).await
    }

    /// Gives a card with no open session a new one: its own session resumed, so its conversation is intact, or, for a
    /// card that never had one, a session started fresh, which reads its opening message first. The loop takes the card's
    /// queued messages. When another message opened one a moment ago, that one is rung instead and the card is left as
    /// it is. The card's lock is held from its read to its store, so a stop of the card is stored before the read or
    /// closes the session made here.
    async fn open_locked(&self, card_id: &str) -> Result<()> {
        let Some(o) = self.me() else {
            return Err(Error::Stopping);
        };
        let id = card_id.to_owned();
        let c = self.store.read(move |r| r.card(&id)).await?;
        let r = Arc::new(Run::new(c.clone()));
        if CardRole::of(&c) == CardRole::Assistant && r.resume.is_empty() {
            let lead = self.assistant_lead_for(&c).await?;
            let mut s = r.state();
            s.lead = lead;
            ASSISTANT_MARKER.clone_into(&mut s.lead_marker);
        }
        {
            let mut sh = self.shared();
            if sh.runs.get(&c.id).is_some_and(|live| live.mailbox.ring()) {
                return Ok(());
            }
            if self.is_stopping() {
                return Err(Error::Stopping); // what is queued stays for the next start
            }
            sh.runs.insert(c.id.clone(), Arc::clone(&r));
            sh.tokens.insert(r.token.clone(), c.id.clone());
        }
        let now = self.now();
        let activity = if r.resume.is_empty() { "" } else { "resuming session" };
        self.update_card(&c.id, move |c| {
            c.set_status(model::CardStatus::Starting, now);
            c.error.clear();
            activity.clone_into(&mut c.activity);
        })
        .await?;
        o.go_loop(r);
        Ok(())
    }

    /// Puts message e, just queued, into the turn its card is in, and says so. False: the message waits for a turn of its
    /// own, as it would have without steering, because no turn is running (or it ended meanwhile), the agent cannot take
    /// it, or older messages are queued before it, which a message must not pass. It is "steered" from then on, and no
    /// turn starts on it. A crash between the agent taking it and the record leaves it queued: it is read again, never
    /// lost.
    pub async fn steer(&self, e: &Event) -> bool {
        let Some(r) = self.run_of(&e.card_id).filter(|r| r.busy()) else {
            return false;
        };
        let steered = {
            let _take = r.take_mu.lock().await; // the session's loop does not take it for a turn while it is steered
            let id = e.card_id.clone();
            match self.store.read(move |s| s.queue(&id)).await {
                Ok(q) if q.first().is_some_and(|first| first.id == e.id) => {}
                _ => return false,
            }
            let Some(sess) = r.state().sess.clone() else {
                return false;
            };
            let prompt = match self.prompt_of(e).await {
                Ok(p) => p,
                Err(err) => {
                    tracing::warn!(home = %e.home, card = %e.card_id, %err, "steer: read the message");
                    return false;
                }
            };
            match tokio::time::timeout(STEER_TIMEOUT, sess.steer(prompt)).await {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => return false,
                Ok(Err(err)) => {
                    tracing::warn!(home = %e.home, card = %e.card_id, %err, "steer");
                    return false;
                }
                Err(_) => {
                    tracing::warn!(home = %e.home, card = %e.card_id, "steer: the agent did not answer in time");
                    return false;
                }
            }
            let (id, now) = (e.id, self.now());
            match self.store.update(move |tx| tx.steer_prompt(id, now)).await {
                Ok(marked) => marked.unwrap_or_else(|| e.clone()),
                Err(err) => {
                    tracing::error!(home = %e.home, card = %e.card_id, event = e.id, %err, "record a steered message");
                    e.clone()
                }
            }
        };
        self.notify.event_updated(&steered);
        true
    }

    /// Hands over again what is still queued for the chain's cards once their sessions closed (a message handed to a
    /// session as it closed). A failed card keeps its messages for the owner's retry.
    pub async fn redispatch(&self, chain_id: &str) {
        let id = chain_id.to_owned();
        let waiting = self
            .store
            .read(move |r| {
                let mut out = Vec::new();
                for c in r.cards(&id)? {
                    if c.status() != &model::CardStatus::Failed && !r.queue(&c.id)?.is_empty() {
                        out.push(c.id);
                    }
                }
                Ok::<_, store::Error>(out)
            })
            .await;
        match waiting {
            Ok(cards) => {
                for c in cards {
                    self.dispatch(&c).await;
                }
            }
            Err(err) => tracing::error!(chain = %chain_id, %err, "hand over queued messages"),
        }
    }

    /// Takes the next message queued for r's card and starts the turn on it, in one transaction: the card is running
    /// from the moment its message is delivered, so at every moment a message is either queued or being worked on. None
    /// when nothing waits, the session has closed or the chain is paused (`open_to_take`).
    pub async fn take(&self, r: &Arc<Run>) -> Option<Event> {
        let _take = r.take_mu.lock().await;
        let (card, lead, marker, recovery, sess) = {
            let s = r.state();
            if r.mailbox.is_closed() {
                return None; // closing takes the run's lock: once it is closed no take starts
            }
            if s.own.is_some() || !s.own_turns.is_empty() {
                return None; // the agent is in a turn of its own: the message waits for its end
            }
            (
                s.card.clone(),
                s.lead.clone(),
                s.lead_marker.clone(),
                s.recovery.clone(),
                s.sess.clone(),
            )
        };
        let now = self.now();
        let (session, context_lost) = sess
            .as_ref()
            .map_or((String::new(), false), |s| (s.id().to_owned(), s.context_lost()));
        let resume = r.resume.clone();
        let mut next = card.clone();
        next.set_status(model::CardStatus::Running, now);
        next.turns = next.turns.saturating_add(1);
        next.error.clear();
        next.updated_at = now;
        let taken = self
            .store
            .update(move |tx| {
                if !open_to_take(tx, &next)? {
                    return Ok(Taken::Paused);
                }
                let Some(mut e) = tx.take_prompt(&next.id, now)? else {
                    return Ok(Taken::Nothing);
                };
                let mut new_recovery = None;
                if !lead.is_empty() {
                    e.data = with_lead(e.data.as_ref(), &lead, &marker);
                    e = tx.update_event(e)?;
                    new_recovery = record_lead(
                        tx,
                        &mut next,
                        &e,
                        &lead,
                        recovery.as_ref(),
                        &session,
                        context_lost,
                        &resume,
                        now,
                    )?;
                }
                let card = tx.put_card(next)?;
                Ok::<_, Error>(Taken::Message(Box::new((e, card, new_recovery))))
            })
            .await;
        let (e, stored, new_recovery) = match taken {
            Ok(Taken::Message(t)) => *t,
            Ok(Taken::Paused) => {
                r.mailbox.ring(); // the message waits for the resume: the loop comes round to wait_open
                return None;
            }
            Ok(Taken::Nothing) => return None,
            Err(err) => {
                tracing::error!(home = %card.home, card = %card.id, %err, "take message");
                return None;
            }
        };
        {
            let mut s = r.state();
            // the agent going to work from here on is this message's turn, not one of its own
            s.prompt = e.id;
            // what cut the last turn short is past; what cuts this one from here on, before its message is sent too, holds
            s.cut = None;
            s.begin_turn(self.now(), sent_by(&e));
            let activity = (s.card.activity.clone(), s.card.ctx_used, s.card.ctx_size);
            s.card = stored.clone();
            (s.card.activity, s.card.ctx_used, s.card.ctx_size) = activity;
            if new_recovery.is_none() {
                s.lead.clear();
            }
        }
        self.notify.card_updated(&stored);
        self.notify.event_updated(&e);
        if let Some(rec) = new_recovery {
            let was = r.state().recovery.replace(rec.clone());
            if was.is_none() {
                self.notify.event_added(&rec);
            } else {
                self.notify.event_updated(&rec);
            }
        }
        Some(e)
    }
}

/// What take's transaction did.
enum Taken {
    /// The message taken, its card as stored, and the recovery brief it carries.
    Message(Box<(Event, Card, Option<Event>)>),
    Nothing,
    Paused,
}

/// Whether card c may take a message in take's transaction tx: not while c's chain is paused. `wait_open` keeps the
/// session's loop from taking then; this decides on the chain as stored, so a take that passed `wait_open` before a pause
/// and stores after it waits for the resume too. A chain gone is not paused: its deletion closes the session.
fn open_to_take(tx: &Tx<'_>, c: &Card) -> Result<bool> {
    if crate::homes::standalone_card(c) {
        return Ok(true);
    }
    match tx.chain(&c.home) {
        Ok(ch) => Ok(*ch.status() != model::ChainStatus::Paused),
        Err(store::Error::NotFound) => Ok(true),
        Err(err) => Err(err.into()),
    }
}

/// Keeps, in take's transaction, what a lost context or a pending recovery needs of the session's lead on e, the message
/// it opens with: the recovery event (none when there is none), and, for a session replaced for a lost context, the new
/// session's id on the card.
#[expect(
    clippy::too_many_arguments,
    reason = "the parts of the run take reads under its lock, passed into the transaction"
)]
fn record_lead(
    tx: &Tx<'_>,
    card: &mut Card,
    e: &Event,
    lead: &str,
    recovery: Option<&Event>,
    session: &str,
    context_lost: bool,
    resume: &str,
    now: Time,
) -> Result<Option<Event>> {
    let data = |session: &str| {
        let p = crate::recovery::SessionRecovery {
            session_id: session.to_owned(),
            prompt_id: e.id,
            lead: lead.to_owned(),
        };
        serde_json::to_string(&p).ok().and_then(|d| ForeignJson::parse(&d).ok())
    };
    if let Some(rec) = recovery.filter(|r| r.id != 0) {
        let mut rec = rec.clone();
        rec.data = data(session);
        rec.updated_at = now;
        return Ok(Some(tx.update_event(rec)?));
    }
    if !context_lost {
        return Ok(None);
    }
    session.clone_into(&mut card.session_id);
    Ok(Some(tx.add_event(Event {
        home: card.home.clone(),
        card_id: card.id.clone(),
        status: EventStatus::System(SystemNote::NewSession),
        data: data(session),
        created_at: now,
        updated_at: now,
        text: format!(
            "Replaced session {resume} with {session} because its saved context was not found. The next prompt repeats \
             its brief and asks it to check git status and git log."
        ),
        ..Event::default()
    })?))
}

/// Who acted, in the owner's words.
pub fn actor_words(a: &Actor) -> String {
    match a.kind {
        ActorKind::Owner => return format!("owner {}", a.who),
        ActorKind::Assistant => return "the Assistant, for the owner".to_owned(),
        _ => {}
    }
    if a.who.is_empty() {
        return a.kind.to_string();
    }
    format!("{} {}", a.kind, a.who)
}
