//! Permission prompts (ADR-0059). Every session's prompt goes up one ladder: a deny rule refuses it first and a prompt
//! that names a secret is refused next, neither asked of anyone; then the chain's Chair decides, or passes it up to the
//! owner's inbox, where the Assistant decides it for him (ADR-0087); when nobody has decided within the owner's time
//! limit (`permission_wait`, from the ask), the Chair above an AI employee's session becomes its driver and decides
//! alone, and a session with no Chair above it (whose prompts go to the owner's inbox, and so the Assistant, first; the
//! Assistant's own wait for the owner) is refused by the rule: the asker never decides its own. A mentor never decides
//! one. The first answer wins (`answer_permission` takes the request under the run's lock) and its event names who
//! decided. Whom a card's prompt waits on is stored on the card (`Card::permission_wait`), the one place a reader finds
//! it.

use std::sync::Arc;
use std::time::Duration;

use model::{
    Actor, ActorKind, Card, CardStatus, ChainStatus, DecisionKind, Event, EventStatus, InboxItem, InboxKind,
    InboxOption, InboxStatus, PermissionAsk, PermissionWait, Setting, Settings, SystemNote, Time,
};
use serde::Serialize;
use serde_json::{Map, Value};
use tokio::sync::oneshot;

use crate::apply::{InboxEdit, Records};
use crate::chain_core::Core;
use crate::chain_core::{first_line, new_id};
use crate::deny::denied_by;
use crate::health::after;
use crate::homes::is_standalone_home;
use crate::outbox::{PRIORITY_BLOCKING, by_remit, new_message};
use crate::owner_acts::raised_by_assistant;
use crate::permission_ask::{Call, ask_of, decided as ask_decided, decider, detail_of, withdrawn};
use crate::policy::{deny_for, refuses_secret};
use crate::session::Run;
use crate::step::CardEdit;
use crate::{Error, Result};

/// The answer of a permission request in the owner's inbox that allows the call this once.
pub const OPTION_ALLOW: &str = "allow";
/// The answer that refuses it.
pub const OPTION_REFUSE: &str = "reject";

/// The longest time limit the owner may set: one day.
const MAX_PERMISSION_WAIT_MINUTES: i64 = 24 * 60;

/// Refuses a permission request nobody answered: its turn was cancelled, or its run is gone.
pub fn unanswered() -> wire::runtime::PermissionAnswer {
    wire::runtime::PermissionAnswer::Deny {
        message: "Nobody answered this permission request: its turn ended.".to_owned(),
    }
}

/// The refusal of an answer to a request that waits no more: the first answer won, or the turn ended.
fn decided() -> Error {
    Error::refused("this permission prompt was decided already, or the session no longer waits on it")
}

/// Whom a card's prompt waits on first: the Chair above it, or with none (the Chair's own, a judge's, the Assistant's)
/// the owner's inbox. At the time limit the Chair becomes its driver; a prompt with no Chair above it is refused then
/// (`driver_decides`).
pub fn ladder_of(chair: Option<&Card>) -> PermissionWait {
    if chair.is_some() {
        PermissionWait::Chair
    } else {
        PermissionWait::Owner
    }
}

/// The step of the ladder wait is, as the permission_asked event names it, which a channel reads (`chains::activity`).
fn step_word(wait: &PermissionWait) -> &'static str {
    match wait {
        PermissionWait::Chair => "chair",
        PermissionWait::Owner => "owner",
        PermissionWait::Driver { .. } => "driver",
    }
}

/// One request a session waits on, as it was asked.
#[derive(Debug, Clone)]
pub struct Asked {
    /// "p_…".
    pub id: String,
    pub title: String,
    pub detail: String,
    /// Claude Code's name for the tool asked for.
    pub tool: String,
    pub input: Map<String, Value>,
    /// Whom it waits on now.
    pub step: PermissionWait,
    /// The owner's permission wait as it was asked.
    pub wait: Duration,
    /// Asked + wait: from the ask, whatever step it is at.
    pub deadline: Time,
    /// An owner's inbox item of the request's id is open.
    pub inbox: bool,
    /// The call in words, as its inbox item carries it.
    pub ask: PermissionAsk,
}

/// A request a session waits on, in `RunState::permissions`: dropping it unanswered wakes its waiter, refused.
pub struct AskedPermission {
    pub asked: Asked,
    answer: oneshot::Sender<Answer>,
}

/// A decision on a request: allowed or refused, why, by whom.
#[derive(Debug, Clone)]
struct Answer {
    allow: bool,
    why: String,
    by: Actor,
}

/// The data of the permission_asked event.
#[derive(Serialize)]
struct AskedData<'a> {
    request: &'a str,
    tool: &'a str,
    title: &'a str,
    #[serde(skip_serializing_if = "Map::is_empty")]
    input: &'a Map<String, Value>,
    step: &'static str,
    deadline: Time,
}

/// The data of the permission_answered event.
#[derive(Serialize)]
struct AnsweredData<'a> {
    request: &'a str,
    allow: bool,
    by: &'a Actor,
    why: &'a str,
}

/// The one rule of who may answer a permission request of a card whose chain's Chair is chair_id (empty when no Chair is
/// above it): the Chair and the owner (or the Assistant on his words), at any step; nobody else.
pub fn may_answer(by: &Actor, chair_id: &str) -> Result<()> {
    if by.for_owner() || (!chair_id.is_empty() && *by == model::by_agent(chair_id)) {
        return Ok(());
    }
    Err(Error::refused("only the Chair or the owner decides this prompt"))
}

/// Whether request p may go up to the owner at now: only while it is at the Chair, before its time limit.
pub fn may_escalate(p: &Asked, now: Time) -> Result<()> {
    if now >= p.deadline || matches!(p.step, PermissionWait::Driver { .. }) {
        return Err(Error::refused(
            "its time limit passed: it is yours to decide now, with answer_permission; it can no longer go to the owner",
        ));
    }
    if p.step != PermissionWait::Chair {
        return Err(Error::refused("this prompt is with the owner already"));
    }
    Ok(())
}

/// What a card waiting on a prompt titled title shows, waiting on wait. It is words only: whom the card waits on is
/// read from `Card::permission_wait`.
fn permission_activity(wait: &PermissionWait, title: &str) -> String {
    match wait {
        PermissionWait::Chair | PermissionWait::Driver { .. } => format!("Waiting for the Chair's answer: {title}"),
        PermissionWait::Owner => format!("Waiting for permission: {title}"),
    }
}

/// The options of a permission request in the owner's inbox.
fn permission_options() -> Vec<InboxOption> {
    vec![
        InboxOption {
            id: OPTION_ALLOW.to_owned(),
            name: "Allow".to_owned(),
            kind: "allow_once".to_owned(),
        },
        InboxOption {
            id: OPTION_REFUSE.to_owned(),
            name: "Refuse".to_owned(),
            kind: "reject_once".to_owned(),
        },
    ]
}

/// What the session reads of answer a: allowed, or refused with who refused it and why.
fn decision_of(a: &Answer) -> wire::runtime::PermissionAnswer {
    if a.allow {
        return wire::runtime::PermissionAnswer::Allow;
    }
    let mut message = format!("Refused by {}.", answerer_name(&a.by));
    if !a.why.is_empty() {
        message.push(' ');
        message.push_str(&a.why);
    }
    wire::runtime::PermissionAnswer::Deny { message }
}

/// Who decided a prompt, as the session reads it.
fn answerer_name(by: &Actor) -> &'static str {
    match by.kind {
        ActorKind::Owner => "the owner",
        ActorKind::Assistant => "the owner, through his Assistant",
        ActorKind::Remit => "a rule",
        _ => "the lead",
    }
}

/// The message that puts a prompt to the Chair, minutes being what is left of its time limit.
fn permission_ask(worker: &Card, p: &Asked, minutes: u64) -> String {
    let mut b = format!(
        "[permission request from {} · card {} · request {}]\n{} wants to: {}\n",
        worker.name, worker.id, p.id, worker.name, p.title
    );
    let mut detail = p.detail.clone();
    if detail.chars().count() > 1500 {
        detail = detail.chars().take(1500).collect::<String>() + "…";
    }
    if !detail.is_empty() {
        b.push_str(&detail);
        b.push('\n');
    }
    b.push_str(&format!(
        "\nIt is blocked until it is decided. Decide with answer_permission (request_id {id}, allow, a one-line why): it is \
         yours, the work is. Pass it up with escalate_permission (request_id {id}, why) only when you cannot judge it, \
         or it is not your chain's to decide; the Assistant then decides it for the owner. If nobody decides in \
         {minutes} minutes, the decision becomes yours alone.",
        id = p.id
    ));
    b
}

fn minutes_of(d: Duration) -> u64 {
    d.as_secs() / 60
}

fn raw(v: &impl Serialize) -> Option<model::ForeignJson> {
    serde_json::to_string(v)
        .ok()
        .and_then(|s| model::ForeignJson::parse(&s).ok())
}

/// Takes a request off its run when it ends unanswered (its future dropped: the agent no longer waits), and closes the
/// owner's item of it.
struct Pending {
    o: Arc<Core>,
    r: Arc<Run>,
    home: String,
    id: String,
    done: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        let was = self.r.state().permissions.remove(&self.id);
        if !self.done && was.is_some() {
            let (o, home, id) = (Arc::clone(&self.o), self.home.clone(), self.id.clone());
            self.o
                .background(move |_| async move { o.cancel_asked(&home, &id, "the turn ended").await });
        }
    }
}

/// The request of permissions that is due first; none of none.
fn due_first<'a>(permissions: impl Iterator<Item = &'a Asked>) -> Option<&'a Asked> {
    permissions.min_by(|a, b| a.deadline.cmp(&b.deadline).then_with(|| a.id.cmp(&b.id)))
}

impl Core {
    /// The edit that shows card card_id as its run's prompts stand now: waiting on the one due first, or, with none left,
    /// running again. It moves a card at work only, so a card stopped meanwhile stays stopped.
    fn waiting_edit(&self, card_id: &str) -> CardEdit {
        let shown = self.run_of(card_id).and_then(|r| {
            let s = r.state();
            due_first(s.permissions.values().map(|p| &p.asked)).map(|p| (p.step.clone(), p.title.clone()))
        });
        let now = self.now();
        CardEdit::new(card_id, move |c| match &shown {
            Some((wait, title)) if c.status().works() => {
                c.activity = permission_activity(wait, title);
                c.ask_permission(wait.clone(), now);
            }
            None if c.status() == &CardStatus::Permission => c.set_status(CardStatus::Running, now),
            _ => {}
        })
    }

    /// How long a prompt waits for the lead or the owner before its driver decides: the owner's setting.
    async fn permission_wait(&self) -> Duration {
        let s = match self.store.read(|s| s.settings()).await {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(%err, "the owner's settings could not be read: the permission wait is the default");
                Settings::default()
            }
        };
        u64::try_from(s.permission_wait().0).map_or(Duration::from_secs(600), Duration::from_nanos)
    }

    /// Sets the owner's time limit for permission prompts to minutes (1 to a day), by the owner or the Assistant on his
    /// words, and returns the settings it was set over. A prompt already asked keeps the deadline it was asked with.
    pub async fn set_permission_wait(&self, by: &Actor, minutes: i64, why: &str) -> Result<Settings> {
        if !by.for_owner() {
            return Err(Error::refused("only the owner sets the permission wait"));
        }
        if !(1..=MAX_PERMISSION_WAIT_MINUTES).contains(&minutes) {
            return Err(Error::refused(format!(
                "the permission wait is 1 to {MAX_PERMISSION_WAIT_MINUTES} minutes, not {minutes}"
            )));
        }
        let why = why.trim();
        if why.is_empty() {
            return Err(Error::refused("why is required: what the owner said"));
        }
        let setting = Setting {
            value: minutes,
            by: by.clone(),
            why: why.to_owned(),
            at: self.now(),
        };
        let was = self
            .store
            .update(move |tx| {
                let was = tx.settings()?;
                let mut s = was.clone();
                s.permission_wait_minutes = Some(setting);
                tx.put_settings(s)?;
                Ok::<_, store::Error>(was)
            })
            .await?;
        tracing::info!(minutes, was = was.permission_wait().0, by = ?by, why, "the permission wait was set");
        Ok(was)
    }

    /// Takes run r's prompt up the ladder and waits for its decision, or for the turn to be cancelled; the agent dropping
    /// the wait ends it too.
    pub async fn on_permission(
        self: &Arc<Self>,
        r: &Arc<Run>,
        req: wire::runtime::PermissionRequest,
    ) -> wire::runtime::PermissionAnswer {
        let card = r.card_now();
        let title = if req.tool.title.is_empty() {
            "Permission".to_owned()
        } else {
            req.tool.title.clone()
        };
        // the tools a chain's capabilities withhold are denied to the session itself, so none is ever asked for here
        if let Some(rule) = denied_by(&deny_for(&card, None), &req.tool) {
            return self
                .refuse_by_rule(&card, &title, &format!("the rule {rule} forbids it"))
                .await;
        }
        if refuses_secret(&title, &req.tool.input, &self.own_secrets()) {
            return self
                .refuse_by_rule(&card, &title, "secrets are off limits to agents")
                .await;
        }
        let chair = self.chair_for(&r.process().card_now()).await; // a child card's request goes up its parent's ladder
        let step = ladder_of(chair.as_ref());
        let wait = self.permission_wait().await;
        let ask = ask_of(&Call {
            tool: &req.tool.name,
            input: &req.tool.input,
            title: &title,
            reason: &req.reason,
            classifier: &req.classifier,
            cwd: &card.cwd,
        });
        let asked = Asked {
            id: new_id("p_"),
            detail: detail_of(&ask, &req.tool.input),
            tool: req.tool.name.clone(),
            input: req.tool.input.clone(),
            title: ask.action.clone(),
            step,
            wait,
            deadline: after(self.now(), wait),
            inbox: false,
            ask,
        };
        let (tx, mut rx) = oneshot::channel();
        {
            let mut s = r.state();
            if r.mailbox.is_closed() {
                return unanswered();
            }
            s.permissions.insert(
                asked.id.clone(),
                AskedPermission {
                    asked: asked.clone(),
                    answer: tx,
                },
            );
        }
        let mut pending = Pending {
            o: Arc::clone(self),
            r: Arc::clone(r),
            home: card.home.clone(),
            id: asked.id.clone(),
            done: false,
        };
        if let Err(err) = self.record_asked(&card, chair.as_ref(), &asked).await {
            tracing::error!(home = %card.home, card = %card.id, %err, "permission asked");
            return unanswered();
        }
        let limit = tokio::time::sleep(wait);
        tokio::pin!(limit);
        let mut limit_passed = false;
        loop {
            tokio::select! {
                a = &mut rx => {
                    pending.done = true;
                    return match a {
                        Ok(a) => decision_of(&a),
                        Err(_) => {
                            self.cancel_asked(&card.home, &asked.id, "the turn ended").await;
                            unanswered()
                        }
                    };
                }
                () = &mut limit, if !limit_passed => {
                    limit_passed = true;
                    if let Some(a) = self.driver_decides(r, &card, chair.as_ref(), &asked).await {
                        pending.done = true;
                        return decision_of(&a);
                    }
                }
            }
        }
    }

    /// Stores prompt p of card as asked, in one change: the card waits on its prompt due first, the permission_asked
    /// event, and at the Chair's step the Chair's message (steered into its turn), or at the owner's an inbox item of the
    /// request's id, asked of him so the Assistant decides it, unless the Assistant is the one asking.
    async fn record_asked(&self, card: &Card, chair: Option<&Card>, p: &Asked) -> Result<()> {
        let now = self.now();
        let data = raw(&AskedData {
            request: &p.id,
            tool: &p.tool,
            title: &p.title,
            input: &p.input,
            step: step_word(&p.step),
            deadline: p.deadline,
        });
        let mut w = Records {
            cards: vec![self.waiting_edit(&card.id)],
            events: vec![Event {
                home: card.home.clone(),
                card_id: card.id.clone(),
                status: EventStatus::System(SystemNote::PermissionAsked),
                title: p.title.clone(),
                data,
                created_at: now,
                updated_at: now,
                ..Event::default()
            }],
            ..Records::default()
        };
        let chair = match chair {
            Some(chair) if p.step == PermissionWait::Chair => chair,
            _ => {
                let mut it = self.permission_item(card, p, "");
                if raised_by_assistant(&card.home) {
                    it.owner_only = true; // the asker never decides its own: it waits for the owner
                    w.inbox = vec![it];
                } else {
                    w.asked = vec![it];
                }
                self.mark_inbox(card, &p.id);
                self.commit(None, None, w).await?;
                return Ok(());
            }
        };
        let mut m = new_message(
            &chair.id,
            &format!("{} asks permission: {}", card.name, first_line(&p.title, 100)),
            &permission_ask(card, p, minutes_of(p.wait)),
            by_remit(),
        );
        (m.priority, m.steer) = (PRIORITY_BLOCKING, true);
        self.send(chair, m, w).await?;
        Ok(())
    }

    /// Notes on the run that an owner's inbox item of request id is open.
    fn mark_inbox(&self, card: &Card, id: &str) {
        if let Some(r) = self.run_of(&card.id)
            && let Some(p) = r.state().permissions.get_mut(id)
        {
            p.asked.inbox = true;
        }
    }

    /// The owner's inbox item for prompt p of card, its id the request's; why is the Chair's when it passed it up.
    fn permission_item(&self, card: &Card, p: &Asked, why: &str) -> InboxItem {
        let detail = if why.is_empty() {
            p.detail.clone()
        } else {
            format!("The Chair passed this to you: {why}\n\n{}", p.detail)
        };
        InboxItem {
            id: p.id.clone(),
            home: card.home.clone(),
            card_id: card.id.clone(),
            kind: InboxKind::Permission,
            title: p.title.clone(),
            detail,
            options: permission_options(),
            status: InboxStatus::Open,
            permission: Some(p.ask.clone()),
            created_at: self.now(),
            ..InboxItem::default()
        }
    }

    /// Hands prompt p of card to its driver as its time limit passes: the Chair above it is told the decision is its own,
    /// and the owner's inbox holds the prompt from then on, opened here when the Chair kept it, so he can always break a
    /// tie the driver leaves. A session with no Chair above it has no driver: its prompt was the owner's from the ask, so
    /// the Assistant had it all along (or he did, when the Assistant asks), and the rule refuses it, since the asker never
    /// decides its own (ADR-0059, ADR-0087); the session reads why and may ask again.
    async fn driver_decides(
        self: &Arc<Self>,
        r: &Arc<Run>,
        card: &Card,
        chair: Option<&Card>,
        p: &Asked,
    ) -> Option<Answer> {
        let Some(chair) = chair else {
            let a = Answer {
                allow: false,
                by: model::by_remit("permission wait"),
                why: format!(
                    "nobody decided it in {} minutes, and a session never decides its own request: do without it, or \
                     ask again when it is still needed",
                    minutes_of(p.wait)
                ),
            };
            // decided at this very moment by someone else: its answer reaches the waiter
            let taken = r.state().permissions.remove(&p.id)?;
            self.permission_decided(card, &taken.asked, &a).await;
            return Some(a);
        };
        let open_item = {
            let mut s = r.state();
            match s.permissions.get_mut(&p.id) {
                Some(asked) => {
                    asked.asked.step = PermissionWait::Driver { who: chair.id.clone() };
                    let open = !asked.asked.inbox;
                    asked.asked.inbox = true;
                    open
                }
                None => false,
            }
        };
        let mut w = Records {
            cards: vec![self.waiting_edit(&card.id)],
            ..Records::default()
        };
        if open_item {
            w.inbox = vec![self.permission_item(card, p, "")];
        }
        let mut m = new_message(
            &chair.id,
            &format!("Decide {}'s request", card.name),
            &format!(
                "[permission request {} is now yours to decide: answer it with answer_permission; it can no longer be \
                 passed up, though the owner may still answer it from his inbox]\n{} wants to: {}",
                p.id, card.name, p.title
            ),
            by_remit(),
        );
        (m.priority, m.steer) = (PRIORITY_BLOCKING, true);
        if let Err(err) = self.send(chair, m, w).await {
            tracing::error!(home = %card.home, card = %card.id, %err, "permission handed to the Chair");
        }
        None
    }

    /// By deciding permission request request_id: allowed or refused, for why. Who may answer, and when, is
    /// `may_answer`'s; the first answer wins and every later one is refused. The winner's decision reaches the session,
    /// the card's stream says who decided (permission_answered), the chain records it, and the owner's inbox item closes.
    pub async fn answer_permission_request(&self, by: &Actor, request_id: &str, allow: bool, why: &str) -> Result<()> {
        let why = why.trim();
        let r = self.run_asking(request_id).ok_or_else(decided)?;
        let card = r.card_now();
        let chair_id = self.chair_for(&card).await.map(|c| c.id).unwrap_or_default();
        let taken = {
            let mut s = r.state();
            if !s.permissions.contains_key(request_id) {
                return Err(decided());
            }
            may_answer(by, &chair_id)?;
            if !chair_id.is_empty() && *by == model::by_agent(&chair_id) && why.is_empty() {
                return Err(Error::refused("why is required: the owner reads it beside your answer"));
            }
            s.permissions.remove(request_id).ok_or_else(decided)?
        };
        let a = Answer {
            allow,
            why: why.to_owned(),
            by: by.clone(),
        };
        // the waiter gone meanwhile (its turn ended): the decision is recorded all the same
        taken.answer.send(a.clone()).ok();
        self.permission_decided(&card, &taken.asked, &a).await;
        Ok(())
    }

    /// The owner's answer to permission item it, from his inbox: allowed when option_id is `OPTION_ALLOW`. An item nobody
    /// waits on any more is closed.
    pub async fn answer_permission(&self, it: &InboxItem, option_id: &str, text: &str, by: &Actor) -> Result<()> {
        let allow = option_id == OPTION_ALLOW;
        match self.answer_permission_request(by, &it.id, allow, text).await {
            Err(err) if self.run_asking(&it.id).is_none() => {
                self.cancel_asked(&it.home, &it.id, "nobody waits on it any more").await;
                Err(err)
            }
            other => other,
        }
    }

    /// Chair passing request request_id of one of its team up to the owner, for why: an inbox item of the request's id
    /// opens, asked of him, so the Assistant decides it for him; the time limit stays the one it was asked with.
    pub async fn escalate_permission(&self, chair: &Card, request_id: &str, why: &str) -> Result<()> {
        if chair.origin != model::Origin::Chair {
            return Err(Error::refused("only the Chair passes a permission request up"));
        }
        let why = why.trim();
        if why.is_empty() {
            return Err(Error::refused(
                "why is required: the Assistant decides on it, and the owner reads it",
            ));
        }
        let r = self
            .run_asking(request_id)
            .filter(|r| r.home == chair.home)
            .ok_or_else(decided)?;
        let now = self.now();
        let asked = {
            let mut s = r.state();
            let p = s.permissions.get_mut(request_id).ok_or_else(decided)?;
            may_escalate(&p.asked, now)?;
            p.asked.step = PermissionWait::Owner;
            p.asked.inbox = true;
            p.asked.clone()
        };
        let card = r.card_now();
        let it = self.permission_item(&card, &asked, why);
        let w = Records {
            asked: vec![it],
            cards: vec![self.waiting_edit(&card.id)],
            ..Records::default()
        };
        self.commit(None, None, w).await?;
        let text = format!(
            "Chair passed {}'s request to you: {}.",
            card.name,
            first_line(&asked.title, 100)
        );
        self.decide(
            &chair.home,
            DecisionKind::Permission,
            &text,
            why,
            &card.id,
            model::by_agent(&chair.id),
        )
        .await;
        Ok(())
    }

    /// Records answer a to prompt p of card, taken off its run already, in one change: the card runs again (or waits on
    /// its next prompt, when it has one), its stream says who decided
    /// (permission_answered), and the owner's inbox item closes, answered when he (or his Assistant) decided, cancelled
    /// naming the decider otherwise. The chain records the decision.
    async fn permission_decided(&self, card: &Card, p: &Asked, a: &Answer) {
        let now = self.now();
        let data = raw(&AnsweredData {
            request: &p.id,
            allow: a.allow,
            by: &a.by,
            why: &a.why,
        });
        let mut w = Records {
            cards: vec![self.waiting_edit(&card.id)],
            events: vec![Event {
                home: card.home.clone(),
                card_id: card.id.clone(),
                status: EventStatus::System(SystemNote::PermissionAnswered),
                by: Some(a.by.clone()),
                title: p.title.clone(),
                text: a.why.clone(),
                data,
                created_at: now,
                updated_at: now,
                ..Event::default()
            }],
            ..Records::default()
        };
        if p.inbox {
            let (by, allow) = (a.by.clone(), a.allow);
            w.inbox_edits.push(InboxEdit::new(&p.id, move |it| {
                if by.for_owner() {
                    it.status = InboxStatus::Answered;
                    it.answer = if allow { "Allow" } else { "Refuse" }.to_owned();
                } else {
                    it.status = InboxStatus::Cancelled;
                    it.answer = format!("decided by {}", decider(&by, "the owner"));
                }
                (it.answered_at, it.answered_by) = (Some(now), Some(by.clone()));
                it.permission = it.permission.take().map(|ask| ask_decided(ask, allow, &by));
            }));
        }
        if let Err(err) = self.commit(None, None, w).await {
            tracing::error!(home = %card.home, card = %card.id, %err, "permission decided");
            return;
        }
        if !is_standalone_home(&card.home) {
            let word = if a.allow { "allowed" } else { "refused" };
            let text = format!(
                "{} {word} {}'s request: {}.",
                decider(&a.by, "the owner"),
                card.name,
                first_line(&p.title, 100)
            );
            self.decide(
                &card.home,
                DecisionKind::Permission,
                &text,
                &a.why,
                &card.id,
                a.by.clone(),
            )
            .await;
        }
    }

    /// Closes the owner's inbox item of request id in home, if one is open, saying why: nobody waits on it any more.
    async fn cancel_asked(&self, home: &str, id: &str, why: &str) {
        let (key, why, now) = (id.to_owned(), why.to_owned(), self.now());
        let closed = self
            .store
            .update(move |tx| {
                let mut it = match tx.inbox_item(&key) {
                    Ok(it) if it.status == InboxStatus::Open => it,
                    Ok(_) | Err(store::Error::NotFound) => return Ok(None),
                    Err(err) => return Err(err),
                };
                it.status = InboxStatus::Cancelled;
                it.answered_at = Some(now);
                it.permission = it.permission.take().map(|ask| withdrawn(ask, &why));
                it.answer = why;
                tx.put_inbox(it).map(Some)
            })
            .await;
        match closed {
            Ok(Some(it)) => self.notify.inbox_updated(&it),
            Ok(None) => {}
            Err::<_, store::Error>(err) => tracing::error!(home, item = id, %err, "permission cancelled"),
        }
    }

    /// The run whose session waits on request id, none when none does.
    fn run_asking(&self, id: &str) -> Option<Arc<Run>> {
        self.all_runs()
            .into_iter()
            .find(|r| r.state().permissions.contains_key(id))
    }

    /// The Chair above card: the card is one of its team, and the chain is not finished. None for the Chair itself, a
    /// judge and the Assistant, which have no one above them.
    pub async fn chair_for(&self, card: &Card) -> Option<Card> {
        if !card.on_team() {
            return None;
        }
        let ch = self
            .chain(&card.home)
            .await
            .ok()
            .filter(|ch| *ch.status() != ChainStatus::Done)?;
        let id = ch.chair_id.clone();
        self.store.read(move |s| s.card(&id)).await.ok()
    }

    /// Refuses a prompt without asking anyone, for why (a deny rule forbids it, or it names a secret), and records in the
    /// card's stream that a rule decided it. Claude Code refuses what a deny rule forbids itself (disallowedTools); this
    /// holds when one reaches here all the same, so nobody, the Chair included, can allow it.
    async fn refuse_by_rule(&self, card: &Card, title: &str, why: &str) -> wire::runtime::PermissionAnswer {
        let by = model::by_remit("rules");
        let data = raw(&AnsweredData {
            request: "",
            allow: false,
            by: &by,
            why,
        });
        self.note(Event {
            home: card.home.clone(),
            card_id: card.id.clone(),
            status: EventStatus::System(SystemNote::PermissionAnswered),
            by: Some(by),
            title: title.to_owned(),
            text: why.to_owned(),
            data,
            ..Event::default()
        })
        .await;
        wire::runtime::PermissionAnswer::Deny {
            message: format!("Refused: {why}."),
        }
    }
}

/// The Chair's answer to a permission request of one of its team (answer_permission).
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerPermissionInput {
    /// the id in the permission request message
    pub request_id: String,
    /// true lets the agent do it this once; false refuses it, and the agent finds another way
    #[serde(default)]
    pub allow: bool,
    /// one line, in words the owner reads beside your answer
    pub why: String,
}

/// The Chair passing a permission request up to the owner's inbox, where the Assistant decides it (escalate_permission).
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EscalatePermissionInput {
    /// the id in the permission request message
    pub request_id: String,
    /// why it is not yours to decide, in one line the Assistant decides on and the owner reads
    pub why: String,
}
