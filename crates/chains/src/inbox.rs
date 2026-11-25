//! The owner's inbox: the failures and questions of the agents put to him, and his answers to them. An item that waits on
//! a decision is asked of him (`put_asked`), which puts it to the Assistant first (ADR-0087): it decides for him, he reads
//! after and overturns. The asker never decides its own item, so an item about the Assistant itself is only stored.

use model::{Actor, ActorKind, Card, CardRole, InboxItem, InboxKind, InboxOption, InboxStatus, JudgeKind, Retry, Time};
use store::Tx;
use wire::runtime::FailureCategory;

use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::fact::put_asked_fact;
use crate::ingest::Door;
use crate::owner_acts::{may_decide, raised_by_assistant};
use crate::{Error, Result};

/// Stores in tx item it, which waits for a decision, and asks it of the owner: the fact owner.asked puts it to the
/// Assistant, which decides it for him (ADR-0087). The item as stored, for the caller to tell the channels once tx
/// commits and to kick the Tick that tells the Assistant.
pub fn put_asked(tx: &Tx<'_>, door: &Door, it: InboxItem) -> Result<InboxItem> {
    let it = tx.put_inbox(it)?;
    put_asked_fact(tx, door, &it)?;
    Ok(it)
}

/// What refuses a retry that does not say what to do again: one stored before retries named their message.
const NOTHING_TO_RETRY: &str = "this failure has nothing to retry; send the card a new message instead";

/// Why an agent stopped, typed where it failed (ADR-0081): the owner reads a sentence of its cause (`failure_words`),
/// whoever debugs reads its text in the details. An outside error's shape stops here, never reaching a title.
#[derive(Debug, Clone, PartialEq)]
pub struct Failed {
    pub cause: Cause,
    /// What the failure said, verbatim.
    pub text: String,
}

/// What stopped an agent: a closed set, each with its sentence. A cause that recurs as `Unknown` earns a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The provider: a limit (and when it resets, when it said), an account it refuses, or its service failing.
    Provider {
        category: FailureCategory,
        resets_at: Option<Time>,
    },
    /// Its session could not open.
    NotOpened,
    /// Its agent's process ended.
    ProcessEnded,
    /// The server restarted during the same turn again and again, so Remit parked it.
    Restarts,
    /// It sent nothing for too long, so Remit's liveness rail cut its turn.
    Silent,
    /// Claude Code ended its turn with an error of its own.
    Harness,
    Unknown,
}

impl Failed {
    pub fn new(cause: Cause, text: impl Into<String>) -> Self {
        Self {
            cause,
            text: text.into(),
        }
    }

    /// A turn's failure, typed from the agent's error.
    pub fn of_turn(err: &agent::Error) -> Self {
        let cause = match err {
            agent::Error::Failure(f) => Cause::Provider {
                category: f.category,
                resets_at: f.resets_at,
            },
            agent::Error::TurnFailed(_) => Cause::Harness,
            agent::Error::Ended(_) => Cause::ProcessEnded,
            _ => Cause::Unknown,
        };
        Self::new(cause, err.to_string())
    }
}

/// What the owner reads of a failure: the item's title, and what he can do about it, its first detail line (empty when
/// the cause leaves him nothing to do but what the options offer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureWords {
    pub title: String,
    pub next: String,
}

/// Who an agent is, as a sentence about it names it.
pub fn who_of(c: &Card) -> String {
    match CardRole::of(c) {
        CardRole::Chair => "The Chair".to_owned(),
        CardRole::Assistant => "The Assistant".to_owned(),
        CardRole::Worker | CardRole::Spawned | CardRole::Judge(_) => c.name.clone(),
    }
}

/// The words of failure f of who (`who_of`) at now: one sentence naming who and the cause, never the failure's own text.
/// Pure.
pub fn failure_words(who: &str, f: &Failed, now: Time) -> FailureWords {
    let (title, next) = match f.cause {
        Cause::Provider {
            category: FailureCategory::Limit,
            resets_at,
        } => (
            format!("{who} stopped: its Claude account hit its usage limit"),
            resets_at.map_or_else(
                || "The provider did not say when the limit resets.".to_owned(),
                |at| format!("It resets at {}.", crate::wait::clock(at, now)),
            ),
        ),
        Cause::Provider {
            category: FailureCategory::Access,
            ..
        } => (
            format!("{who} stopped: its Claude account cannot be used"),
            "The account needs signing in again, or the agent another account.".to_owned(),
        ),
        Cause::Provider {
            category: FailureCategory::Service,
            ..
        } => (
            format!("{who} stopped: Claude's service is failing"),
            "Remit tried the turn again and the service still failed; it is usually back within minutes.".to_owned(),
        ),
        Cause::NotOpened => (format!("{who} could not start its session"), String::new()),
        Cause::ProcessEnded => (format!("{who} stopped: its Claude Code process ended"), String::new()),
        Cause::Restarts => (
            format!("{who} is parked: the server restarted during the same turn again and again"),
            "Remit does not resume it on its own.".to_owned(),
        ),
        Cause::Silent => (
            format!("{who} stopped: it went silent, so Remit cut its turn"),
            String::new(),
        ),
        Cause::Harness => (
            format!("{who} stopped: Claude Code ended its turn with an error"),
            String::new(),
        ),
        Cause::Unknown => (
            format!("{who} stopped on an error Remit does not recognise"),
            String::new(),
        ),
    };
    FailureWords { title, next }
}

/// What the owner's retry does, in his words; empty when there is nothing to retry.
fn retry_words(retry: Option<&Retry>) -> &'static str {
    match retry {
        Some(rt) if rt.reopen => "Retry opens its session again.",
        Some(rt) if rt.prompt_id != 0 => "Retry sends the same message again.",
        _ => "",
    }
}

/// A failure item's detail: what the owner can do, then the failure's own text under "Details".
fn failure_detail(next: &str, retry: Option<&Retry>, text: &str) -> String {
    let lines: Vec<&str> = [next, retry_words(retry)]
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect();
    let mut detail = lines.join("\n");
    if !text.trim().is_empty() {
        if !detail.is_empty() {
            detail.push_str("\n\n");
        }
        detail.push_str("Details\n");
        detail.push_str(text.trim());
    }
    detail
}

impl Core {
    /// The sentence card c's failure f reads as now: its item's title and the card's error.
    pub fn failure_title(&self, c: &Card, f: &Failed) -> String {
        failure_words(&who_of(c), f, self.now()).title
    }

    /// Puts a failure in front of the owner, asked of him (`put_asked`): nothing an agent fails at may stay silent. retry
    /// is what a retry does, none when nothing can be retried.
    pub async fn report_error(&self, c: &Card, f: &Failed, retry: Option<Retry>) {
        if matches!(CardRole::of(c), CardRole::Judge(JudgeKind::Goal)) {
            // its failure ended its check, which the chain's summary and decisions say: there is nothing to retry
            return;
        }
        let words = failure_words(&who_of(c), f, self.now());
        let detail = failure_detail(&words.next, retry.as_ref(), &f.text);
        let mut options = Vec::new();
        if retry.is_some() {
            options.push(option(OPTION_RETRY, "Retry"));
        }
        options.push(option("dismiss", "Dismiss"));
        let it = InboxItem {
            id: new_id("i_"),
            home: c.home.clone(),
            card_id: c.id.clone(),
            kind: InboxKind::Error,
            title: words.title,
            detail,
            options,
            status: InboxStatus::Open,
            created_at: self.now(),
            retry,
            // the Assistant's own failure is not put to it: it would fail again on it
            owner_only: raised_by_assistant(&c.home),
            ..InboxItem::default()
        };
        let asked = !it.owner_only;
        let door = self.door();
        let stored = self
            .store
            .update(move |tx| {
                if asked {
                    put_asked(tx, &door, it)
                } else {
                    Ok(tx.put_inbox(it)?)
                }
            })
            .await;
        match stored {
            Ok(it) => {
                self.notify.inbox_updated(&it);
                if asked {
                    self.kick(); // the Assistant is told on the Tick
                }
            }
            Err(err) => tracing::error!(chain = %c.home, card = %c.id, %err, "report error"),
        }
    }

    /// Puts in front of the owner the files a deleted chain left that could not be removed: the chain is gone, so its
    /// inbox item is all that says so. He removes them himself, and dismisses it.
    pub async fn report_left(&self, chain_id: &str, left: &str) {
        tracing::warn!(chain = %chain_id, err = %left, "files of a deleted chain are left");
        let it = InboxItem {
            id: new_id("i_"),
            home: chain_id.to_owned(),
            kind: InboxKind::Error,
            title: format!("Files of deleted chain {chain_id} are left"),
            detail: format!("{left}\n\nRemove them by hand, then dismiss this."),
            options: vec![option("dismiss", "Dismiss")],
            status: InboxStatus::Open,
            created_at: self.now(),
            // no agent reaches the server's files: only he removes them
            owner_only: true,
            ..InboxItem::default()
        };
        match self.store.update(move |tx| tx.put_inbox(it)).await {
            Ok(it) => self.notify.inbox_updated(&it),
            Err(err) => tracing::error!(chain = %chain_id, %err, "files left not reported"),
        }
    }

    /// Does a failure's work again, and then deletes the stream's records of the failure: they are the owner's to read
    /// until the work is on its way, and a retry that was refused leaves them.
    async fn retry(&self, rt: &Retry) -> Result<()> {
        if rt.reopen {
            // the card's own session, resumed, or a fresh one for a card that never had one
            self.open(&rt.card_id).await?;
        } else if rt.prompt_id != 0 {
            self.resend(&rt.card_id, rt.prompt_id).await?;
        } else {
            return Err(Error::refused(NOTHING_TO_RETRY));
        }
        self.delete_events(&rt.failure_ids).await;
        Ok(())
    }

    /// Closes the open errors of a card whose turn has just succeeded: the failure is behind it, and the owner has
    /// nothing left to answer. Only the inbox's view: a failure to store it is logged.
    pub async fn supersede_errors(&self, card_id: &str) {
        let card = card_id.to_owned();
        let closed = self
            .store
            .update(move |tx| {
                let mut closed = Vec::new();
                for mut it in tx.open_inbox()? {
                    if it.kind != InboxKind::Error || it.card_id != card {
                        continue;
                    }
                    it.status = InboxStatus::Cancelled;
                    "the agent's next turn succeeded".clone_into(&mut it.answer);
                    closed.push(tx.put_inbox(it)?);
                }
                Ok::<_, store::Error>(closed)
            })
            .await;
        match closed {
            Ok(closed) => closed.iter().for_each(|it| self.notify.inbox_updated(it)),
            Err(err) => tracing::warn!(card = %card_id, %err, "supersede errors"),
        }
    }

    /// The titles of what the chain's Chair and agents asked the owner and he has not answered: a question, or a change
    /// that moves a rail, a decided one waiting out its overturn window included. A Chair that would finish without his
    /// answer is refused.
    pub async fn awaiting_owner(&self, chain_id: &str) -> Result<Vec<String>> {
        let (mut open, decided) = self
            .store
            .read(|r| Ok::<_, store::Error>((r.open_inbox()?, r.decided_inbox()?)))
            .await?;
        open.extend(decided);
        Ok(open
            .into_iter()
            .filter(|it| it.home == chain_id && matches!(it.kind, InboxKind::Question | InboxKind::Change))
            .map(|it| it.title)
            .collect())
    }

    /// Claims open item id for by's answer in one transaction: it is stored answered, so a second decider at the same
    /// time is refused rather than acting twice. The item as stored.
    pub async fn claim_answer(&self, id: &str, answer: String, by: &Actor) -> Result<InboxItem> {
        let (id, by, now) = (id.to_owned(), by.clone(), self.now());
        self.store
            .update(move |tx| -> Result<InboxItem> {
                let mut it = tx.inbox_item(&id)?;
                if it.status != InboxStatus::Open {
                    return Err(Error::refused(format!("item {id} is {}", it.status)));
                }
                may_decide(&it, &by)?;
                it.status = InboxStatus::Answered;
                it.answer = answer;
                (it.answered_at, it.answered_by) = (Some(now), Some(by));
                Ok(tx.put_inbox(it)?)
            })
            .await
    }

    /// Puts claimed item id back open: what its answer asked failed, so nothing was decided and it waits again.
    pub async fn reopen(&self, id: &str) {
        let wanted = id.to_owned();
        let reopened = self
            .store
            .update(move |tx| -> Result<InboxItem> {
                let mut it = tx.inbox_item(&wanted)?;
                it.status = InboxStatus::Open;
                it.answer.clear();
                (it.answered_at, it.answered_by) = (None, None);
                Ok(tx.put_inbox(it)?)
            })
            .await;
        if let Err(err) = reopened {
            tracing::error!(item = %id, %err, "reopen an inbox item whose answer failed");
        }
    }

    /// Does what by's answer to claimed item it asks: a failure retried, an exception sent back to its author (who reads
    /// the decider's words and changes it), a question's answer told to its Chair. Nothing for a dismissal or an
    /// accepted exception.
    pub async fn act_on_answer(&self, it: &InboxItem, option_id: &str, text: &str, by: &Actor) -> Result<()> {
        match it.kind {
            InboxKind::Error if option_id == OPTION_RETRY => {
                let rt = it.retry.as_ref().ok_or_else(|| Error::refused(NOTHING_TO_RETRY))?;
                self.retry(rt).await
            }
            InboxKind::Exception if option_id == EXCEPTION_REVISE => {
                let said = format!(
                    "[{} sent back the exception {:?}]\n{}\nChange what it names, then go on.",
                    model::owner_voice(by),
                    it.title,
                    text.trim()
                );
                self.deliver(&it.card_id, "Exception sent back", &said, by.clone())
                    .await?;
                Ok(())
            }
            InboxKind::Question => {
                let home = it.home.clone();
                let chain = self.store.read(move |r| r.chain(&home)).await?;
                // Like any owner message: queued if the Chair is live, its session resumed if not.
                let said = format!(
                    "[{} answered your question {:?}]\n{}",
                    model::owner_voice(by),
                    it.title,
                    it.answer
                );
                self.deliver(&chain.chair_id, "Your answer", &said, by.clone()).await?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// What an answer option_id to failure it does: dismissed, or retried when there is something to retry. Pure.
pub fn error_answer(it: &InboxItem, option_id: &str) -> Result<String> {
    if option_id != OPTION_RETRY {
        return Ok("dismissed".to_owned());
    }
    if it.retry.is_none() {
        return Err(Error::refused(NOTHING_TO_RETRY));
    }
    Ok("retried".to_owned())
}

/// What an answer option_id to an exception decides, with the decider's text: accepted, the work goes on as it is; sent
/// back, which must say what its author changes. Pure.
pub fn exception_answer(option_id: &str, text: &str) -> Result<String> {
    let decided = match option_id {
        EXCEPTION_ACCEPT => "Accepted",
        EXCEPTION_REVISE => "Sent back",
        _ => {
            return Err(Error::refused(format!(
                "option {option_id:?}: an exception is answered {EXCEPTION_ACCEPT} or {EXCEPTION_REVISE}"
            )));
        }
    };
    let text = text.trim();
    if option_id == EXCEPTION_REVISE && text.is_empty() {
        return Err(Error::refused("say in text what its author changes"));
    }
    Ok(format!("{decided} {text}").trim().to_owned())
}

/// The answer to question it: the option option_id names, with text after it, or text alone. Pure.
pub fn question_answer(it: &InboxItem, option_id: &str, text: &str) -> Result<String> {
    let answer = it
        .options
        .iter()
        .filter(|op| op.id == option_id)
        .map(|op| format!("{} {text}", op.name).trim().to_owned())
        .next_back()
        .unwrap_or_else(|| text.to_owned());
    if answer.is_empty() {
        return Err(Error::refused("answer is empty"));
    }
    Ok(answer)
}

/// An exception's options: the work goes on as it is, or its author changes it.
pub const EXCEPTION_ACCEPT: &str = "accept";
pub const EXCEPTION_REVISE: &str = "revise";

/// The options of an exception.
pub fn exception_options() -> Vec<InboxOption> {
    vec![
        option(EXCEPTION_ACCEPT, "Accept: the work goes on as it is"),
        option(EXCEPTION_REVISE, "Send back: its author changes it"),
    ]
}

pub fn option(id: &str, name: &str) -> InboxOption {
    InboxOption {
        id: id.to_owned(),
        name: name.to_owned(),
        ..InboxOption::default()
    }
}

impl Core {
    /// Tells the channels of the owner's items, as stored.
    pub fn tell_inbox(&self, items: &[InboxItem]) {
        for it in items {
            self.notify.inbox_updated(it);
        }
    }
}

/// The options of a held item, as lifting a hold names them; a failure's retry is the same option.
pub const OPTION_RETRY: &str = "retry";

/// Whether the Assistant answered item it for the owner: the answers he may overturn. Pure.
pub fn answered_by_assistant(it: &InboxItem) -> bool {
    it.answered_by
        .as_ref()
        .is_some_and(|by| by.kind == ActorKind::Assistant)
}
