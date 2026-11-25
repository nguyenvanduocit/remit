//! The climb (ADR-0118, point 1): the recovery built before the risk. A card whose model code chose
//! (`Card::chosen`) may have been given less than its work needs, so when its turn breaks, or its gate sends its work back
//! twice (`shipping::gate::CLIMB_AFTER_ROUNDS`), its work is handed to the same employee on a new card one tier higher, as
//! a hand-over Remit makes (`hand_over_by`), recorded as a decision with why. It never goes above the catalog's strongest,
//! and a card whose model its Chair or its agent named is theirs to move. The hand-over's own commit scores the choice
//! (wrong only when the gate caused it: a broken turn says little of the model) and tells the Chair, so a failure has one
//! owner: when Remit takes the card the Assistant is not handed a Retry, and when it cannot, the Assistant is, as it
//! always was.

use std::sync::Arc;

use model::{Card, CardRole, Climb, ClimbCause, Origin, Retry, by_remit};

use crate::chain_core::Core;
use crate::handover::HandOverInput;
use crate::inbox::{Cause, Failed};
use crate::{Error, Result};

/// Whether a failure of this cause is one a stronger model could have avoided: the agent's own turn broke or it fell
/// silent. A provider's limit, a session that would not open, a process that ended or a restart say nothing of the model.
pub const fn climbs_after(cause: &Cause) -> bool {
    matches!(cause, Cause::Harness | Cause::Silent)
}

impl Core {
    /// What follows a failed turn of card, whose error is f: a worker whose model code chose is raised a tier by Remit,
    /// and the failure is Remit's to answer; every other card's failure is reported for the owner's Assistant, with
    /// retry, as it always was. When Remit cannot raise the card, the failure is reported the same way.
    pub async fn respond_to_failed_turn(self: &Arc<Self>, card: &Card, f: &Failed, retry: Option<Retry>) {
        if CardRole::of(card) != CardRole::Worker || !climbs_after(&f.cause) || !self.can_climb(card) {
            self.report_error(card, f, retry).await;
            return;
        }
        let (o, card, f) = (Arc::clone(self), card.clone(), f.clone());
        let why = format!("it failed: {}", f.text);
        self.background(move |_| async move {
            if let Err(err) = o.climb_card(&card.id, &why, ClimbCause::Failed).await {
                log_unraised(&card.id, &err);
                o.report_error(&card, &f, retry).await;
            }
        });
    }

    /// Raises, in the background, each of the cards ids, whose gate sent their work back for why.
    pub fn climb_cards(self: &Arc<Self>, ids: Vec<String>, why: &str) {
        for id in ids {
            let (o, why) = (Arc::clone(self), why.to_owned());
            self.background(move |_| async move {
                if let Err(err) = o.climb_card(&id, &why, ClimbCause::SentBack).await {
                    log_unraised(&id, &err);
                }
            });
        }
    }

    /// Raises card one tier, for why, when it is one the climb applies to and the catalog holds a higher tier. Nothing
    /// when it is not: a card that is not hired, whose model nothing chose, already handed over, or at the top tier.
    ///
    /// # Errors
    /// Reading the card or its chain fails, or the card was not moved: the hiring rules refuse the new card (its
    /// employee retired), or it is being handed over already.
    pub async fn climb_card(self: &Arc<Self>, card_id: &str, why: &str, cause: ClimbCause) -> Result<()> {
        let id = card_id.to_owned();
        let card = self.store.read(move |r| r.card(&id)).await?;
        let Some(to) = self.tier_above(&card) else {
            return Ok(());
        };
        let chain = self.chain(&card.home).await?;
        let chair_id = chain.chair_id.clone();
        let chair = self.store.read(move |r| r.card(&chair_id)).await?;
        let input = HandOverInput {
            card_id: card.id.clone(),
            employee: card.employee.clone(),
            agent: String::new(),
            why: why.to_owned(),
        };
        self.hand_over_by(&chair, input, by_remit("climb"), Some(Climb { model: to, cause }))
            .await?;
        // the hand-over may have been refused as it finished: the card is then as it was
        let id = card.id.clone();
        let after = self.store.read(move |r| r.card(&id)).await?;
        if after.handover.is_none() && !self.handed_over(&after).await? {
            return Err(Error::refused("the hiring rules refused the new card"));
        }
        Ok(())
    }

    /// Whether card is one the climb can raise now.
    pub fn can_climb(&self, card: &Card) -> bool {
        self.tier_above(card).is_some()
    }

    /// The model one tier above card's, when card is a hired one whose model code chose and is not handed over.
    fn tier_above(&self, card: &Card) -> Option<String> {
        if card.origin != Origin::Hired || card.chosen.is_none() || card.handover.is_some() || card.employee.is_empty()
        {
            return None;
        }
        let catalog = self.cfg.models.as_ref()?;
        let tier = catalog.lookup(&card.model)?.tier;
        catalog.above(tier).map(|m| m.id.clone())
    }
}

/// Logs that card was not raised: a refusal is a rule at work (a second climb of a card two verdicts raised at once), not
/// a fault.
fn log_unraised(card: &str, err: &Error) {
    if matches!(err, Error::Refused(_)) {
        tracing::debug!(%card, %err, "a card was not raised");
    } else {
        tracing::error!(%card, %err, "raise a card one tier");
    }
}
