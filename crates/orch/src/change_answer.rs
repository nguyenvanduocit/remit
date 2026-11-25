//! The owner's answer to a change item (ADR-0062): Apply stores the write it asks through the writer of its kind, in every
//! domain, and Not now stores nothing. Raising the item is the core's (change_item.rs in chains).

use crate::orch::Orchestrator;
use crate::overturn_window::answerable;
use capability::agents::put_agent;
use capability::library::put_skill;
use capability::library::with_skill;
use capability::playbooks::prepared_playbook;
use capability::playbooks::put_playbook;
use capability::rail::agent_rail;
use capability::rail::skill_rail;
use chains::Result;
use chains::chain_core::optional;
use chains::change_item::Applied;
use chains::change_item::CHANGE_APPLY;
use chains::change_item::CHANGE_NOT_NOW;
use chains::change_item::Tell;
use chains::change_item::kind_words;
use chains::changes::Authorship;
use chains::changes::record_change;
use chains::error::Error;
use chains::outbox::by_remit;
use chains::owner_acts::ACT_APPLY;
use chains::owner_acts::for_owner;
use model::Actor;
use model::ChangeKind;
use model::InboxItem;
use model::InboxKind;
use model::InboxStatus;
use model::PendingChange;
use model::Sensor;
use pipeline::sensor::reads_as_sensor;
use pipeline::sensor_settings::sensor_rail;
use reach::credentials::answered;
use store::Tx;

/// The record of a change, made ready before its transaction: what a write of its kind prepares reads the library,
/// which a transaction cannot wait on.
pub(crate) enum Prepared {
    Playbook(model::Playbook),
    Agent(model::Agent),
    Skill(model::Skill),
    /// A kind whose apply reads only the pending change.
    Plain,
}

/// Whether item, which an Apply lets through, gives a chain a capability: the chain's own writer stores that.
fn capability_applied(item: &InboxItem) -> bool {
    item.change.as_ref().is_some_and(|c| c.kind == ChangeKind::Capability)
}

/// What storing an answer to a change item came to.
struct Answered {
    /// The item, closed.
    closed: InboxItem,
    /// What the agent that asked is told.
    result: String,
    /// Tells the channels of what was stored, once it is committed.
    tell: Option<Tell>,
    /// The record changed while the item waited, so nothing was stored.
    moved: bool,
}

impl Orchestrator {
    /// The owner's answer to a change item: Apply stores the write as it was asked, and only if the record is still the
    /// one it was made against; Not now stores nothing. Either way the agent that asked is told. Who may answer is
    /// `for_owner`'s, checked before anything is prepared or applied: the owner, or the Assistant for him (ADR-0062).
    pub(crate) async fn answer_change(&self, item_id: &str, option_id: &str, by: &Actor) -> Result<()> {
        for_owner(ACT_APPLY, by)?;
        if option_id != CHANGE_APPLY && option_id != CHANGE_NOT_NOW {
            return Err(Error::refused(format!(
                "option {option_id:?}: a change is answered {CHANGE_APPLY} or {CHANGE_NOT_NOW}"
            )));
        }
        let applied = self.applied_item(item_id, option_id).await?;
        if let Some(item) = applied.as_ref().filter(|it| capability_applied(it)) {
            // a chain's lifecycle has one writer, so the chain's own writer stores it, not this transaction
            return self.capabilities.apply_capability(item.clone(), by).await;
        }
        let prepared = match applied {
            Some(InboxItem {
                change: Some(pending), ..
            }) => self.prepared_change(&pending).await?,
            _ => Prepared::Plain,
        };
        let skill = match &prepared {
            Prepared::Skill(s) => Some(s.clone()),
            _ => None,
        };
        let o = self.me().ok_or(Error::Stopping)?;
        let (id, option, by) = (item_id.to_owned(), option_id.to_owned(), by.clone());
        let update = self
            .store
            .update(move |tx| o.store_answer(tx, &id, &option, &by, prepared));
        let out = match skill {
            // the library is laid out for the skill before it is stored, as any write of one is
            Some(s) => self.capabilities.change_skills(with_skill(s), update).await?,
            None => update.await?,
        };
        if let Some(tell) = out.tell {
            tell(self);
        }
        self.tell_inbox(std::slice::from_ref(&out.closed));
        if let Err(err) = self
            .deliver(
                &out.closed.card_id,
                "The owner answered a change",
                &out.result,
                by_remit(),
            )
            .await
        {
            tracing::warn!(item = %item_id, %err, "tell the author what the owner answered");
        }
        if out.moved {
            return Err(Error::ChangeMoved);
        }
        Ok(())
    }

    /// Stores, in tx, the answer option to the change item item_id: the item closed, and on Apply the write it asks
    /// (prepared before the transaction). What the author is told and the channels are told is in what it returns.
    fn store_answer(
        &self,
        tx: &Tx<'_>,
        item_id: &str,
        option_id: &str,
        by: &Actor,
        prepared: Prepared,
    ) -> Result<Answered> {
        let mut item = tx
            .inbox_item(item_id)
            .map_err(|e| Error::refused(format!("item {item_id}: {e}")))?;
        let Some(pending) = item.change.clone().filter(|_| item.kind == InboxKind::Change) else {
            return Err(Error::refused(format!("item {item_id} is not a change")));
        };
        if !answerable(&item, self.now()) {
            return Err(Error::refused(format!("item {item_id} is {}", item.status)));
        }
        let label = format!("{} {}", kind_words(&pending.kind), pending.name);
        if option_id == CHANGE_NOT_NOW {
            return Ok(Answered {
                closed: tx.put_inbox(answered(item, by.clone(), self.now(), "not now"))?,
                result: format!(
                    "[change result · not now] {label} was not let through for the owner: nothing was written. Ask \
                     what is wanted instead, or go on without it."
                ),
                tell: None,
                moved: false,
            });
        }
        let w = Authorship {
            by: model::by_agent(&item.card_id),
            approved_by: Some(by.clone()),
        };
        match self.apply_change(tx, &pending, prepared, &w)? {
            Applied::Moved => {
                // Closing the item is the answer; the caller is told, after this commits, that nothing was written.
                item.status = InboxStatus::Cancelled;
                "the record changed while this waited".clone_into(&mut item.answer);
                Ok(Answered {
                    closed: tx.put_inbox(item)?,
                    result: format!(
                        "[change result · not applied] {label} changed while it waited for an answer, so what was \
                         read is no longer what it would have been: nothing was written. Read it again and ask again if it \
                         is still needed."
                    ),
                    tell: None,
                    moved: true,
                })
            }
            Applied::Written(tell) => Ok(Answered {
                closed: tx.put_inbox(answered(item, by.clone(), self.now(), "applied"))?,
                result: format!(
                    "[change result · applied] {label} was let through for the owner: it was written as you asked."
                ),
                tell,
                moved: false,
            }),
        }
    }
    /// The write an answer lets through, made ready before its transaction (`Prepared`): a playbook, an agent or a
    /// skill as its writes accept it, which may read the library; any other kind as it is pending.
    async fn prepared_change(&self, pending: &PendingChange) -> Result<Prepared> {
        Ok(match pending.kind {
            ChangeKind::Playbook => Prepared::Playbook(
                prepared_playbook(serde_json::from_str(&pending.record)?)
                    .map_err(|e| Error::refused(format!("the write is no longer one the playbooks accept: {e}")))?,
            ),
            ChangeKind::Agent => Prepared::Agent(
                self.capabilities
                    .prepared_agent(serde_json::from_str(&pending.record)?)
                    .await
                    .map_err(|e| {
                        Error::refused(format!("the write is no longer one the agent definitions accept: {e}"))
                    })?,
            ),
            ChangeKind::Skill => Prepared::Skill(
                self.capabilities
                    .prepared_skill(serde_json::from_str(&pending.record)?)
                    .await
                    .map_err(|e| Error::refused(format!("the write is no longer one the library accepts: {e}")))?,
            ),
            _ => Prepared::Plain,
        })
    }
    /// Stores the write pending in tx, with its change, if its record is still as it was when the item was opened
    /// (`Applied::Moved` when it is not, and nothing is stored). prepared is the record `prepared_change` made of it. The
    /// channels are told by the tell it returns, once the transaction is committed.
    pub(crate) fn apply_change(
        &self,
        tx: &Tx<'_>,
        pending: &PendingChange,
        prepared: Prepared,
        w: &Authorship,
    ) -> Result<Applied> {
        let at = self.now();
        match prepared {
            Prepared::Playbook(mut p) => {
                // a playbook moves no rail (ADR-0113): one an earlier build held is written as asked
                let cur = optional(tx.playbook(&p.id))?;
                p.rev = cur.as_ref().map_or(0, |c| c.rev);
                let p = put_playbook(tx, w, at, p)?;
                return Ok(Applied::Written(Some(Box::new(move |o| o.notify.playbook_updated(&p)))));
            }
            Prepared::Agent(mut a) => {
                let cur = optional(tx.agent(&a.id))?;
                if agent_rail(cur.as_ref(), &a) != pending.fields {
                    return Ok(Applied::Moved);
                }
                a.rev = cur.as_ref().map_or(0, |c| c.rev); // its fields were checked against cur, in tx
                let a = put_agent(tx, w, at, a)?;
                return Ok(Applied::Written(Some(Box::new(move |o| o.notify.agent_updated(&a)))));
            }
            Prepared::Skill(mut s) => {
                let cur = optional(tx.skill(&s.name))?;
                if skill_rail(cur.as_ref(), &s) != pending.fields {
                    return Ok(Applied::Moved);
                }
                s.rev = cur.as_ref().map_or(0, |c| c.rev); // its fields were checked against cur, in tx
                let s = put_skill(tx, w, at, s)?;
                let notify = std::sync::Arc::clone(&self.notify);
                return Ok(Applied::Written(Some(Box::new(move |_| {
                    notify.skill_updated(&capability::library::listed_skill(&s));
                }))));
            }
            Prepared::Plain => {}
        }
        match pending.kind {
            ChangeKind::Promote => self.crew.apply_promote(tx, pending, w),
            ChangeKind::Forget => self.crew.apply_forget(tx, pending, w),
            ChangeKind::Playbook | ChangeKind::Agent | ChangeKind::Skill => Err(Error::refused(format!(
                "a {} change is applied as prepared_change makes it ready",
                kind_words(&pending.kind)
            ))),
            ChangeKind::Grant => Ok(if self.reach.apply_grant(tx, pending, w)? {
                Applied::Moved
            } else {
                Applied::Written(None)
            }),
            ChangeKind::Sensor => self.apply_sensor(tx, pending, w),
            ChangeKind::EventSignal => Err(Error::refused(
                "an event signal's settings move no rail, so no change of one waits (a sensor's do: a sensor change)",
            )),
            ChangeKind::Extension => self.reach.apply_extension(tx, pending, w),
            ChangeKind::Objective => self.crew.apply_objective(tx, pending, w),
            ChangeKind::Employee => self.crew.apply_employee(tx, pending, w),
            ChangeKind::Capability => Err(Error::refused(
                "a capability is given by the chain's own writer (apply_capability), never in this transaction",
            )),
            ChangeKind::KeyResult => Err(Error::refused(
                "a key result that refines another is written at once by whoever answers for its parent, so no change of one waits (ADR-0115)",
            )),
            ChangeKind::Project => Err(Error::refused(
                "a project moves no rail, so no change of one waits (ADR-0108)",
            )),
        }
    }
    /// Stores the sensor a change item asks to write, if what it reads is still what the owner was shown.
    fn apply_sensor(&self, tx: &Tx<'_>, pending: &PendingChange, w: &Authorship) -> Result<Applied> {
        let mut next: Sensor = serde_json::from_str(&pending.record)?;
        let cur = optional(tx.sensor(&next.id))?;
        // it was there when asked, and is gone
        if cur.is_none() && pending.fields.first().is_none_or(|f| !f.before.is_empty()) {
            return Ok(Applied::Moved);
        }
        if sensor_rail(cur.as_ref(), &next)? != pending.fields {
            return Ok(Applied::Moved);
        }
        if let Some(c) = &cur {
            next.cursor = if reads_as_sensor(c, &next) {
                c.cursor.clone()
            } else {
                String::new()
            };
        }
        (next.by, next.updated_at) = (w.by.clone(), self.now());
        record_change(
            tx,
            w,
            self.now(),
            ChangeKind::Sensor,
            &next.name,
            cur.as_ref(),
            Some(&next),
        )?;
        let saved = tx.put_sensor(next)?;
        Ok(Applied::Written(Some(Box::new(move |o| {
            o.notify.sensor_updated(&saved)
        }))))
    }
    /// The item an answer lets through: the change item item_id when the answer is Apply, else none. What the item is
    /// and whether it is still open is checked again where the answer is stored.
    async fn applied_item(&self, item_id: &str, option_id: &str) -> Result<Option<InboxItem>> {
        if option_id != CHANGE_APPLY {
            return Ok(None);
        }
        let id = item_id.to_owned();
        let item = self
            .store
            .read(move |r| r.inbox_item(&id))
            .await
            .map_err(|e| Error::refused(format!("item {item_id}: {e}")))?;
        Ok(Some(item))
    }
}
