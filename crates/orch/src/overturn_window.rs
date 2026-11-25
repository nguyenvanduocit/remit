//! The overturn window (ADR-0101). The Assistant decides every item put to it at once (ADR-0087), but an Apply it gives
//! that lets something on the top rung of the effect ladder happen (`grant::Effect::Irreversible`) takes effect only once
//! the window has passed: until then the item is stored `decided`, the owner sees it with its time and Overturn, and his
//! overturn stops it before anything is written. The agent that asked is told at once and keeps working on what does not
//! need it. The decided item is the whole state: the Tick lets each one take effect once its time has come, through the
//! answer the owner's Apply calls, in the transaction that closes it, so a restart mid-window resumes it and two Ticks
//! never write it twice.

use std::time::Duration;

use grant::Effect;
use model::{Actor, ActorKind, ChangeKind, InboxItem, InboxKind, InboxStatus, PendingChange, Time};

use chains::change_item::{CHANGE_APPLY, kind_words};
use chains::health::after;
use chains::outbox::by_remit;
use chains::owner_acts::may_decide;
use chains::wait::clock;
use chains::{Error, Result};
use reach::grant_access::GrantAsk;

use crate::orch::Orchestrator;

/// How long an answer of the Assistant's on the top rung waits for the owner's overturn: the bake a release waits out
/// before it is called healthy (ADR-0087), the time Remit already gives a person or a signal to catch a bad change.
pub const OVERTURN_WINDOW: Duration = Duration::from_secs(60 * 60);

/// The rung of the effect ladder that applying change lets happen, None for a write that lets no outside effect happen.
/// A grant's is the effect it gives. An extension (an outside MCP server included) runs a program whose tools its author
/// ranked, and the ladder's rule is that the platform ranks a tool, never its author: what the platform did not rank
/// counts as the top rung, and so does a grant whose effect cannot be read. Pure.
fn rung_of(change: &PendingChange) -> Option<Effect> {
    match change.kind {
        ChangeKind::Grant => Some(
            serde_json::from_str::<GrantAsk>(&change.record)
                .ok()
                .and_then(|ask| ask.effect.parse().ok())
                .unwrap_or(Effect::Irreversible),
        ),
        ChangeKind::Extension => Some(Effect::Irreversible),
        _ => None,
    }
}

/// Whether by's answer option_id to item it waits out the overturn window: an Apply by anyone but the owner (the
/// Assistant, deciding for him) that lets something on the top rung happen. His own answer takes effect at once. Pure.
pub(crate) fn waits_out_window(it: &InboxItem, option_id: &str, by: &Actor) -> bool {
    by.kind != ActorKind::Owner
        && it.kind == InboxKind::Change
        && option_id == CHANGE_APPLY
        && it.change.as_ref().and_then(rung_of) == Some(Effect::Irreversible)
}

/// Whether item it may be answered at now: open, or decided with its window passed (the answer taking effect). Pure.
pub(crate) fn answerable(it: &InboxItem, now: Time) -> bool {
    match it.status {
        InboxStatus::Open => true,
        InboxStatus::Decided => it.takes_effect_at.is_some_and(|at| at <= now),
        _ => false,
    }
}

impl Orchestrator {
    /// Stores by's Apply of change item it as decided, taking effect when the window has passed, and tells the agent that
    /// asked: nothing is written yet, and it goes on with what does not need it. The item is claimed in one transaction,
    /// so of two deciders at once one decides and the other is refused.
    pub(crate) async fn decide_for_window(&self, it: &InboxItem, by: Actor) -> Result<()> {
        let (id, now) = (it.id.clone(), self.now());
        let at = after(now, OVERTURN_WINDOW);
        let words = format!("apply, taking effect at {} unless you overturn it", clock(at, now));
        let decided = self
            .store
            .update(move |tx| -> Result<InboxItem> {
                let mut it = tx.inbox_item(&id)?;
                if it.status != InboxStatus::Open {
                    return Err(Error::refused(format!("item {id} is {}", it.status)));
                }
                may_decide(&it, &by)?;
                it.status = InboxStatus::Decided;
                it.answer = words;
                (it.answered_at, it.answered_by, it.takes_effect_at) = (Some(now), Some(by), Some(at));
                Ok(tx.put_inbox(it)?)
            })
            .await?;
        self.tell_inbox(std::slice::from_ref(&decided));
        if decided.card_id.is_empty() {
            return Ok(());
        }
        let label = decided
            .change
            .as_ref()
            .map(|c| format!("{} {}", kind_words(&c.kind), c.name))
            .unwrap_or_else(|| decided.title.clone());
        let said = format!(
            "[change result · decided] {label} was let through for the owner by his Assistant. It cannot be undone once \
             written, so it is written at {} unless he overturns it first: nothing is written yet. Go on with the parts of \
             the work that do not need it; a message starting [change result tells you when it is written, or that he \
             overturned it.",
            clock(at, now)
        );
        if let Err(err) = self
            .deliver(&decided.card_id, "The Assistant decided a change", &said, by_remit())
            .await
        {
            tracing::warn!(item = %decided.id, %err, "tell the author its change waits out the overturn window");
        }
        Ok(())
    }

    /// Lets every decided item whose window has passed by now take effect, as the Assistant's answer that decided it: the
    /// write is stored in the transaction that closes the item, so each takes effect once. One that fails is logged and
    /// stays decided for the next Tick.
    pub(crate) async fn take_effect_due(&self, now: Time) {
        let due = match self.store.read(|r| r.decided_inbox()).await {
            Ok(items) => items.into_iter().filter(|it| answerable(it, now)).collect::<Vec<_>>(),
            Err(err) => {
                tracing::error!(%err, "read the decided inbox items");
                return;
            }
        };
        if due.is_empty() {
            return;
        }
        for it in due {
            let Some(by) = it.answered_by.clone() else {
                tracing::error!(item = %it.id, "a decided item names no decider");
                continue;
            };
            match self.answer_change(&it.id, CHANGE_APPLY, &by).await {
                Ok(()) | Err(Error::ChangeMoved) => {}
                Err(err) => tracing::warn!(item = %it.id, %err, "a decided change did not take effect"),
            }
        }
        self.push_feed().await;
    }
}

/// Stops decided item item_id in tx, the owner overturning it before its window passed: closed with nothing written.
/// None when it is not decided any more (it took effect, so his overturn reaches what followed).
pub(crate) fn stop_decided(tx: &store::Tx<'_>, item_id: &str) -> Result<Option<InboxItem>> {
    let mut it = tx.inbox_item(item_id)?;
    if it.status != InboxStatus::Decided {
        return Ok(None);
    }
    it.status = InboxStatus::Cancelled;
    "overturned by the owner before it took effect".clone_into(&mut it.answer);
    it.takes_effect_at = None;
    Ok(Some(tx.put_inbox(it)?))
}
