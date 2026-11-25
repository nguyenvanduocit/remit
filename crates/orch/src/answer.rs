//! Answering an inbox item (inbox.rs in chains keeps the items): the answer goes to what the item's kind belongs to, a
//! permission, a credential, a sign-in, a change, a held firing, a question, so it is the orchestrator's (ADR-0096).

use std::sync::Arc;

use model::{Actor, InboxKind, InboxStatus};

use chains::inbox::{error_answer, exception_answer, question_answer};
use chains::request::{Done, Request, Service};
use chains::{Error, Result};

use chains::owner_acts::may_decide;

use crate::orch::Orchestrator;
use crate::overturn_window::waits_out_window;

impl Orchestrator {
    /// Resolves an inbox item: a permission option, the owner's text for a Chair question, or the value of a
    /// credential.
    pub async fn answer(
        self: &Arc<Self>,
        req: Request,
        item_id: String,
        option_id: String,
        text: String,
    ) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "Answer", move |o| async move {
            o.answer_item(&item_id, &option_id, &text, by).await
        })
        .await
    }

    /// Answers inbox item item_id as by: the owner, or his Assistant deciding for him what `may_decide` lets it. An item
    /// answered here is claimed (open → answered) in one transaction before anything is done on it, so of two deciders
    /// at once one acts and the other is refused; what the answer asks failing puts the item back open.
    pub async fn answer_item(&self, item_id: &str, option_id: &str, text: &str, by: Actor) -> Result<()> {
        let id = item_id.to_owned();
        let mut it = self.store.read(move |r| r.inbox_item(&id)).await?;
        if it.status != InboxStatus::Open {
            return Err(Error::refused(format!("item {item_id} is {}", it.status)));
        }
        may_decide(&it, &by)?;
        if waits_out_window(&it, option_id, &by) {
            return self.decide_for_window(&it, by).await;
        }
        let answer = match it.kind {
            // the ladder's one answer path: the first answer wins and closes this item
            InboxKind::Permission => return self.answer_permission(&it, option_id, text, &by).await,
            // The text is the value: it goes to the sealed store and nowhere else, so nothing here or below records it.
            InboxKind::Credential => return self.reach.answer_credential(item_id, option_id, text, &by).await,
            // The text is the code of a sign-in: it goes to the sign-in process and nowhere else.
            InboxKind::Login => return self.submit_login_code(item_id, text, &by).await,
            InboxKind::Change => return self.answer_change(item_id, option_id, &by).await,
            InboxKind::Held => return self.pipeline.answer_held(it, option_id, text, by).await,
            InboxKind::Error => error_answer(&it, option_id)?,
            InboxKind::Exception => exception_answer(option_id, text)?,
            InboxKind::Question => {
                let answer = question_answer(&it, option_id, text)?;
                if it.workflow.is_some() {
                    // a run's askOwner(): his answer is the step's result, stored with it
                    it.status = InboxStatus::Answered;
                    (it.answered_at, it.answered_by) = (Some(self.now()), Some(by));
                    answer.clone_into(&mut it.answer);
                    return self.answer_step(it, &answer).await;
                }
                answer
            }
        };
        let it = self.claim_answer(item_id, answer, &by).await?;
        if let Err(err) = self.act_on_answer(&it, option_id, text, &by).await {
            self.reopen(item_id).await;
            return Err(err);
        }
        self.notify.inbox_updated(&it);
        Ok(())
    }
}
