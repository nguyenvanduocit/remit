//! A card whose account fails (it is signed out or at its limit, the service is down) goes on another account that can run
//! it, and with none waits for one, instead of waiting for the owner: the failure is the provider's, not the work's. Whether
//! an account is out of reach is outages (health.rs). A turn whose service failed is first tried again on the same session
//! (`prompt_retrying`): a blip is not an outage. A card with a live session on a login moves that session to the other
//! account and is sent its message again (ADR-0074); one with none moves or waits (`wait`) with its message kept, and a
//! Tick opens it again once an account can (`wake_waiting`): running out of capacity is not the owner's to answer, and the
//! Health bar already tells him which accounts are out and why. A card is opened on an account by `account_for`, and one
//! with no account in use waits before a turn is spent finding out (`unreachable`). See ADR 0005, 0006, 0040 and 0074.

use std::sync::Arc;
use std::time::Duration;

use model::{AccountKind, Card, CardStatus, Event, EventStatus, Retry, SystemNote, Time};
use wire::runtime::{Failure, FailureCategory};

use crate::apply::Records;
use crate::chain_core::Core;
use crate::health::{after, cooldown, spread};
use crate::inbox::{Cause, Failed};
use crate::outbox::{Message, sent_again};
use crate::session::Run;
use crate::step::CardEdit;
use crate::{Error, Result};

/// A turn whose provider's service failed is tried again this many times, after a wait of `RETRY_BASE`, doubled at each
/// try and spread (jitter).
const SERVICE_RETRIES: u32 = 2;
const RETRY_BASE: Duration = Duration::from_secs(20);

/// When a card waiting for an account is next looked at, and the failure that spreads the look after it. It is kept in
/// memory: after a restart a waiting card, its message still queued, is opened again with the others
/// (`resume_interrupted`), and waits again while every account is out.
#[derive(Debug, Clone)]
pub struct WaitingCard {
    pub at: Time,
    pub fault: Failure,
}

/// The provider's failure err says, none when err is anything else.
pub fn provider_fault(err: &agent::Error) -> Option<Failure> {
    match err {
        agent::Error::Failure(f) => Some(f.clone()),
        _ => None,
    }
}

/// The effort a new agent starts at when want is asked for: want itself when it is one of `provider::Effort`'s levels,
/// empty (Claude Code's default) when it is empty. Anything else is refused with the levels. Pure.
pub fn resolve_effort(want: &str) -> Result<String> {
    provider::Effort::parse(want).map_err(Error::refused)?;
    Ok(want.to_owned())
}

/// A model and the effort it runs at, as the owner and the Chair read them: `fable · high`, or the model alone at Claude
/// Code's default effort, and "the default model" when neither is named. Pure.
pub fn model_label(model: &str, effort: &str) -> String {
    let model = if model.is_empty() { "the default model" } else { model };
    if effort.is_empty() {
        model.to_owned()
    } else {
        format!("{model} · {effort}")
    }
}

/// d as the owner reads a wait: whole seconds, then minutes.
fn rounded(d: Duration) -> String {
    let secs = d.as_secs_f64().round();
    let whole = Duration::from_secs_f64(secs).as_secs();
    let (m, s) = (whole / 60, whole % 60);
    if m == 0 { format!("{s}s") } else { format!("{m}m{s}s") }
}

impl Core {
    /// The model a new agent starts on when want is asked for: want itself, once the catalog knows it. A model the catalog
    /// does not know is refused with the ones it does; with no catalog a model is taken as named. While the provider is out
    /// of reach the agent still starts on want, and waits for the provider at its first turn (`unreachable`).
    pub fn resolve_model(&self, want: &str) -> Result<String> {
        match &self.cfg.models {
            Some(models) if !models.models().is_empty() && models.lookup(want).is_none() => Err(Error::refused(
                format!("{want:?} is not a model: choose one of {}", models.describe()),
            )),
            _ => Ok(want.to_owned()),
        }
    }

    /// Tells the Chair, whose own model and effort are own and own_effort, which models and efforts it may hire on, and
    /// what happens when the provider fails; empty with no catalog.
    pub fn models_help(&self, own: &str, own_effort: &str) -> String {
        match &self.cfg.models {
            Some(models) if !models.models().is_empty() => format!(
                "Leave the model input empty unless you know better: Remit then chooses it from the brief, the \
                 deliverable and the criteria, the cheapest tier it is sure can do the work, and raises the agent one tier \
                 by itself when its turn fails or its gate sends its work back twice. An agent definition that names a model \
                 keeps it, and where no one chooses, an agent runs on its definition's model and effort, else yours ({}). \
                 Name a model or an effort only when the work needs it, and it is used as given. Models: {}. Effort is how long the model thinks before it acts: {}. The most capable model is for \
                 judgement, design and work no check can prove; more effort for work that is hard to get right, less \
                 where more thinking adds nothing, because a check proves the result or the task is plain. A more \
                 capable model at a lower effort often does better than a weaker one at its highest. When the provider \
                 fails or is out of reach, an agent waits for it and goes on by itself. ",
                model_label(own, own_effort),
                models.describe(),
                provider::Effort::list()
            ),
            _ => String::new(),
        }
    }

    /// A draw in [0, 1) that spreads a wait (jitter).
    fn draw(&self) -> f64 {
        (self.draw)()
    }

    /// Records that the account run r's card runs on answered a turn: whatever failed of it before is over.
    pub fn account_answered(&self, r: &Run) {
        let account = r.card_now().account;
        if !account.is_empty() {
            self.account_up(&account);
        }
    }

    /// Answers a failure f of run r's account: its breaker hears of it, and the card goes on another account that can run
    /// it, or waits for one (`wait`). False when the card is unknown: the failure is then the owner's.
    pub async fn failover(
        self: &Arc<Self>,
        r: &Arc<Run>,
        f: &Failure,
        prompt: Option<&Event>,
        failure: &[i64],
    ) -> bool {
        let id = r.card_id();
        let Ok(card) = self.store.read(move |s| s.card(&id)).await else {
            return false;
        };
        self.account_failed(&card.account, f);
        {
            let mut sh = self.shared();
            sh.failovers = sh.failovers.saturating_add(1); // presses on the capacity (admit.rs)
        }
        let named = Failure {
            category: f.category,
            title: format!("{}: {}", self.account_label(&card.account), f.title),
            resets_at: None,
        };
        if self.move_session(r, &card, &named, prompt, failure).await {
            return true;
        }
        self.wait(r, card, &named, prompt, failure).await;
        true
    }

    /// Moves the live session of run r to another account that can run its card, without closing it (ADR-0074), and sends
    /// it the message its failed turn was on (prompt) again. Only a login moves: a session on a plugin's account, or one
    /// going to one, runs on variables a running process cannot change. False when nothing moved: no session is open, no
    /// other account can run it, or the move failed, and the card then waits (`wait`).
    async fn move_session(
        self: &Arc<Self>,
        r: &Arc<Run>,
        card: &Card,
        f: &Failure,
        prompt: Option<&Event>,
        failure: &[i64],
    ) -> bool {
        let Some(sess) = r.state().sess.clone() else {
            return false;
        };
        if !r.open() {
            return false;
        }
        let is_login = |id: &str| self.account_by_id(id).is_some_and(|a| a.kind() != AccountKind::Plugin);
        // chosen under the lock, moved without it: a move takes as long as the process answers, and every other
        // placement would wait on it. One chosen meanwhile counts this card on its old account, which only weighs it.
        let next = {
            let _place = self.place_mu.lock().await;
            self.account_for(card)
        };
        if next == card.account || !self.outages.usable(&next) || !is_login(&card.account) || !is_login(&next) {
            return false;
        }
        if let Err(err) = sess.move_to(next.clone()).await {
            tracing::warn!(card = card.id, account = next, %err, "move the session: it is closed and resumed there");
            return false;
        }
        let (name, why) = (self.account_label(&next), f.title.clone());
        let activity = format!("Moved to account {name}: {why}");
        let now = self.now();
        let mut recs = Records {
            cards: vec![CardEdit::new(&card.id, {
                let next = next.clone();
                move |c| {
                    c.set_status(CardStatus::Idle, now);
                    c.error.clear();
                    c.activity.clone_from(&activity);
                    c.account.clone_from(&next);
                }
            })],
            ..Records::default()
        };
        if let Some(p) = prompt {
            recs.messages.push(again(card, p));
        }
        let stored = {
            let _place = self.place_mu.lock().await;
            self.commit(None, None, recs).await
        };
        if let Err(err) = stored {
            tracing::error!(card = card.id, %err, "record a session's move to another account");
            return false;
        }
        self.delete_events(failure).await;
        self.note(Event {
            home: card.home.clone(),
            card_id: card.id.clone(),
            status: EventStatus::System(SystemNote::Moved),
            text: format!(
                "Its account cannot run this agent now ({why}): Remit moved its session to account {name}, which goes on \
                 without a restart, and sends it its message again."
            ),
            ..Event::default()
        })
        .await;
        r.mailbox.ring();
        true
    }

    /// The failure that keeps every account out of use, each named with what it failed with; none while one can run
    /// agents. Its category is that of the account that is asked again soonest, so a card looks again when any may answer.
    pub fn out_of_accounts(&self) -> Option<Failure> {
        let mut titles = Vec::new();
        let mut soonest: Option<Failure> = None;
        for a in self.account_list() {
            let f = self.outages.fault(&a.id)?;
            titles.push(format!("{}: {}", a.id, f.title));
            if soonest.as_ref().is_none_or(|s| cooldown(&f) < cooldown(s)) {
                soonest = Some(f);
            }
        }
        soonest.map(|s| Failure {
            category: s.category,
            title: titles.join("; "),
            resets_at: None,
        })
    }

    /// Has the card of run r wait before a turn is spent finding out that every account is out of use: when each one's
    /// breaker is open and a message waits for them, prompt (the one it has just taken, which goes back in its queue) or,
    /// with none taken, one queued. True when the run ended for it.
    pub async fn unreachable(self: &Arc<Self>, r: &Arc<Run>, prompt: Option<&Event>) -> bool {
        let Some(f) = self.out_of_accounts() else { return false };
        let id = r.card_id();
        let Ok(card) = self.store.read(move |s| s.card(&id)).await else {
            return false;
        };
        if prompt.is_none() {
            let id = card.id.clone();
            match self.store.read(move |s| s.queue(&id)).await {
                Ok(q) if !q.is_empty() => {}
                _ => return false,
            }
        }
        self.wait(r, card, &f, prompt, &[]).await;
        true
    }

    /// Parks card, whose run is r, while its account cannot run it (it failed with f, named by account): its session
    /// closes, the message its failed turn was on (prompt; none when none) goes back in its queue, and a Tick opens it
    /// again (`wake_waiting`): at once on another account when one can run agents, and otherwise after f's cooldown, once
    /// an account answers. The failure events that recorded it go: an account out of capacity is not the owner's to
    /// answer. The run is closed first, so a stop, a pause or a shutdown that ended it already is never undone; once it is
    /// closed the card's end is this wait's to record, and a wait that cannot be recorded fails the card
    /// (`wait_not_recorded`), never leaving it in a turn no run holds.
    pub async fn wait(
        self: &Arc<Self>,
        r: &Arc<Run>,
        card: Card,
        f: &Failure,
        prompt: Option<&Event>,
        failure: &[i64],
    ) {
        if !self.close_run_obj(r, &model::Cut::WaitsForAccount) {
            return; // it ended already (stopped, paused, shut down): nothing waits on it
        }
        let why = f.title.clone();
        let place = self.place_mu.lock().await; // the move is chosen and stored as one step
        let next = self.account_for(&card);
        let moved = next != card.account && self.outages.usable(&next);
        let (activity, note) = if moved {
            let name = self.account_label(&next);
            (
                format!("Moving to account {name}: {why}"),
                format!(
                    "Its account cannot run this agent now ({why}): Remit moves it to account {name} and sends it its \
                     message again."
                ),
            )
        } else {
            (
                format!("Waiting for an account: {why}"),
                format!("No account can run this agent now ({why}): Remit sends it its message again once one can."),
            )
        };
        let now = self.now();
        let mut recs = Records {
            cards: vec![CardEdit::new(&card.id, {
                let next = next.clone();
                move |c| {
                    c.set_status(CardStatus::Waiting, now);
                    c.error.clear();
                    c.activity.clone_from(&activity);
                    if moved {
                        c.account.clone_from(&next);
                    }
                }
            })],
            ..Records::default()
        };
        // back in the queue, unread: no session opens on it until the card is woken
        if let Some(p) = prompt {
            recs.messages.push(again(&card, p));
        }
        let stored = self.commit(None, None, recs).await;
        drop(place);
        if let Err(err) = stored {
            self.wait_not_recorded(r, &card, f, prompt, failure, &err).await;
            return;
        }
        if moved {
            self.arm_wait_at(&card.id, f, self.now());
            self.kick();
        } else {
            self.arm_wait(&card.id, f);
        }
        self.delete_events(failure).await;
        self.note(Event {
            home: card.home.clone(),
            card_id: card.id.clone(),
            status: EventStatus::System(SystemNote::Waiting),
            text: note,
            ..Event::default()
        })
        .await;
    }

    /// Fails card, whose run r `wait` closed, as a turn or an opening fails, when its wait for an account (on f) could
    /// not be recorded for err: the failure goes in front of the owner, whose retry sends the message the run was on
    /// (prompt) again, or opens the session again when it was on none.
    async fn wait_not_recorded(
        self: &Arc<Self>,
        r: &Arc<Run>,
        card: &Card,
        f: &Failure,
        prompt: Option<&Event>,
        failure: &[i64],
        err: &Error,
    ) {
        tracing::error!(chain = card.home, card = card.id, %err, "wait for a provider: the card fails instead");
        let failed = Failed::new(
            Cause::Provider {
                category: f.category,
                resets_at: None, // the wait names none: it looks again after a cooldown
            },
            format!("{}; its wait for another account could not be recorded: {err}", f.title),
        );
        let mut failure_ids = failure.to_vec();
        failure_ids.extend(self.record_failed(r, &failed).await);
        let retry = Retry {
            card_id: card.id.clone(),
            prompt_id: prompt.map_or(0, |p| p.id),
            failure_ids,
            reopen: prompt.is_none(),
        };
        self.report_error(card, &failed, Some(retry)).await;
    }

    /// Sets when the waiting card is next looked at: after the cooldown of f, spread, so that cards that waited together
    /// are not all opened together once an account answers.
    fn arm_wait(&self, card: &str, f: &Failure) {
        let at = after(self.now(), spread(cooldown(f), self.draw()));
        self.arm_wait_at(card, f, at);
    }

    fn arm_wait_at(&self, card: &str, f: &Failure, at: Time) {
        self.shared()
            .waits
            .insert(card.to_owned(), WaitingCard { at, fault: f.clone() });
    }

    /// Looks at the cards waiting for an account whose time has come at now: while an account is in use a card is opened
    /// again, and while every one is out it is looked at again a spread cooldown later. A card no longer waiting (stopped
    /// meanwhile) is forgotten.
    pub async fn wake_waiting(&self, now: Time) {
        let mut due: Vec<(String, WaitingCard)> = self
            .shared()
            .waits
            .iter()
            .filter(|(_, w)| w.at <= now)
            .map(|(id, w)| (id.clone(), w.clone()))
            .collect();
        due.sort_by(|a, b| a.0.cmp(&b.0));
        for (id, w) in due {
            let card = id.clone();
            let waiting = self
                .store
                .read(move |s| s.card(&card))
                .await
                .is_ok_and(|c| c.status() == &CardStatus::Waiting);
            if !waiting {
                self.forget_wait(&id);
                continue;
            }
            if self.out_of_accounts().is_some() {
                self.arm_wait(&id, &w.fault);
                continue;
            }
            self.forget_wait(&id);
            self.dispatch(&id).await;
        }
    }

    /// Drops what is kept of card's wait for an account.
    fn forget_wait(&self, card: &str) {
        self.shared().waits.remove(card);
    }

    /// Runs the turn on message e in r's session. A turn the provider's service failed (overloaded, a server error) is
    /// tried again on the same session after a spread wait, up to `SERVICE_RETRIES` times, while the provider stays in use
    /// (its breaker closed) and the run open: each try that failed is noted in the stream. What it returns is the last
    /// try's end.
    pub async fn prompt_retrying(
        self: &Arc<Self>,
        r: &Arc<Run>,
        sess: &dyn agent::Session,
        e: &Event,
    ) -> std::result::Result<model::StopReason, agent::Error> {
        let prompt = self
            .prompt_of(e)
            .await
            .map_err(|err| agent::Error::TurnFailed(format!("read the message: {err}")))?;
        let mut attempt: u32 = 1;
        loop {
            let done = sess.prompt(prompt.clone()).await;
            let Some(f) = done.as_ref().err().and_then(provider_fault) else {
                return done;
            };
            if f.category != FailureCategory::Service || attempt > SERVICE_RETRIES || !self.retryable(r, &f) {
                return done;
            }
            let wait = spread(
                RETRY_BASE.saturating_mul(2_u32.saturating_pow(attempt.saturating_sub(1))),
                self.draw(),
            );
            self.close_stream(r).await;
            self.note(Event {
                home: e.home.clone(),
                card_id: e.card_id.clone(),
                status: EventStatus::System(SystemNote::Retrying),
                text: format!(
                    "The provider's service failed ({}): Remit tries the turn again in {} ({attempt} of {SERVICE_RETRIES}).",
                    f.title,
                    rounded(wait)
                ),
                ..Event::default()
            })
            .await;
            if !self.wait_retry(wait, r).await {
                return done;
            }
            {
                let mut s = r.state();
                s.prompt = e.id;
                s.begin_turn(self.now(), crate::outbox::sent_by(e));
            }
            attempt = attempt.saturating_add(1);
        }
    }

    /// Records that the provider's service failed with f on r's account, and says whether r's turn may be tried again:
    /// the account is still in use and the run open.
    fn retryable(&self, r: &Run, f: &Failure) -> bool {
        let account = r.card_now().account;
        self.account_failed(&account, f);
        self.outages.usable(&account) && r.open()
    }

    /// Waits d before a retry of r's turn; false when the run closed, or the server began to stop, meanwhile.
    async fn wait_retry(&self, d: Duration, r: &Run) -> bool {
        tokio::select! {
            () = tokio::time::sleep(d) => r.open(),
            () = self.stopped.cancelled() => false,
        }
    }
}

/// The message e that card's failed turn was on, sent again: queued as it was, not as a new one.
fn again(card: &Card, e: &Event) -> Message {
    Message {
        to: card.id.clone(),
        home: card.home.clone(),
        ..sent_again(e)
    }
}
