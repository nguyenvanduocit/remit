//! The hire and end actions of the signal backbone (ADR-0035). A hire creates the card of a role for what a fact names;
//! an end ends the goal check of a chain whose time is up. Both are Remit's own: a subscription of any other owner holds
//! neither. The roles are a closed list, each with what it takes from admission and the code that builds its card and
//! brief from records Remit reads (a fact carries ids, never words of the work): each role's hire is its group's
//! (goal.rs, recheck.rs).

use model::{Card, ChainStatus, Time};

use crate::Shipping;
use chains::admit::{Needs, fits};
use chains::apply::Records;
use chains::fact::{FactValue, fact_value_of};
use chains::goal::GOAL_CHECK_LIMIT;
use chains::step::{CardEdit, JudgeEnd};
use chains::{Error, Result};
use pipeline::automation::{ROLE_GOAL_JUDGE, ROLE_GOAL_RECHECK};
use pipeline::processing::Delivery;

/// What each role's hire takes from the room: a provider that can run agents. None for a role Remit does not hire.
fn hire_needs(role: &str) -> Option<Needs> {
    match role {
        ROLE_GOAL_JUDGE | ROLE_GOAL_RECHECK => Some(Needs {
            agents: true,
            ..Needs::default()
        }),
        _ => None,
    }
}

/// One hire in the making: the delivery of its firing, and when.
#[derive(Debug, Clone)]
pub struct SignalHire {
    pub d: Delivery,
    pub now: Time,
}

impl SignalHire {
    /// What a step that does the hire stores: the hired card, and the firing with its outcome and the card it hired,
    /// with the subscription's progress.
    pub fn records(&self, outcome: &str, hired: Card) -> Records {
        let fired = self.d.done(outcome, &hired.home, &hired.id);
        Records {
            created: vec![hired],
            fired: Some(fired),
            ..Records::default()
        }
    }

    /// What a step that gives the work to a card it already has stores: edit to that card, and the firing with its
    /// outcome and the card, with the subscription's progress.
    pub fn records_again(&self, outcome: &str, card: &Card, edit: CardEdit) -> Records {
        let fired = self.d.done(outcome, &card.home, &card.id);
        Records {
            cards: vec![edit],
            fired: Some(fired),
            ..Records::default()
        }
    }
}

/// How a hire's step ended: stored once, or nothing to do (`Error::NothingToDo`).
pub(crate) fn after_hire(changed: bool, r: Result<()>) -> Result<()> {
    match r {
        Ok(()) if !changed => Err(Error::NothingToDo),
        other => other,
    }
}

impl Shipping {
    /// What a hire's firing d gives the hire of its role: the firing and the fact its reading holds, once the role is one
    /// a subscription hires and there is room for it.
    async fn signal_hire(&self, d: &Delivery) -> Result<(SignalHire, FactValue)> {
        let sub = &d.sub;
        let r = d.reading()?;
        let v = fact_value_of(r).map_err(|e| Error::refused(format!("the fact {} at {}: {e}", d.firing.key, r.rev)))?;
        let role = sub.action.role.as_str();
        let Some(needs) = hire_needs(role) else {
            return Err(Error::refused(format!(
                "subscription {} hires no role {role:?}",
                sub.id
            )));
        };
        fits(&needs, &self.room("").await?)?;
        let h = SignalHire {
            d: d.clone(),
            now: d.now,
        };
        Ok((h, v))
    }

    /// The effect of a hire: the card of the subscription's role, for what the one reading its firing covers names.
    pub async fn fire_hire(&self, d: &Delivery) -> Result<()> {
        let (h, v) = self.signal_hire(d).await?;
        match d.sub.action.role.as_str() {
            ROLE_GOAL_JUDGE => self.hire_goal_judge(&h, &v).await,
            ROLE_GOAL_RECHECK => self.hire_goal_recheck(&h, &v).await,
            role => Err(Error::refused(format!(
                "subscription {} hires no role {role:?}",
                d.sub.id
            ))),
        }
    }

    /// The effect of an end: the goal check of the chain the subscription names ends, its judge stopped (or, when none
    /// was hired, the chain finishing without a check), for it is past its time. The effect is a state change that holds
    /// when done twice, so it comes before the firing is stored, and a restart between the two ends nothing twice. A
    /// check that is over already ends the subscription; one held for a verification the judge opened, or a paused
    /// chain's, is held and waits.
    pub async fn fire_end(&self, d: &Delivery) -> Result<()> {
        let sub = &d.sub;
        let id = sub.action.subject.clone();
        let ch = match self.store.read(move |r| r.chain(&id)).await {
            Ok(ch) if ch.finishing.is_some() => ch,
            Ok(_) | Err(store::Error::NotFound) => return self.drop_subscription(&sub.id).await,
            Err(err) => return Err(err.into()),
        };
        let Some(finishing) = ch.finishing.clone() else {
            return Ok(());
        };
        if *ch.status() != ChainStatus::Running && !finishing.recheck {
            // a second look at a finished chain runs while it is done
            return Err(Error::NotAdmitted(format!(
                "the chain is {}: its check's time starts again when it runs",
                ch.status()
            )));
        }
        let minutes = GOAL_CHECK_LIMIT.0 / model::Duration::MINUTE.0;
        let why = if finishing.judge.is_empty() {
            let why = format!("no judge was hired within {minutes} minutes");
            self.end_goal_check(&ch.id, "", &why, JudgeEnd::Stopped, Records::default())
                .await?;
            why
        } else {
            let judge = finishing.judge.clone();
            if let Ok(card) = self.store.read(move |r| r.card(&judge)).await {
                let chains = self.store.read(|r| r.chains()).await?;
                if crate::open_verification::awaits_verification(&chains, &card) {
                    return Err(Error::NotAdmitted(
                        "its judge waits for a verification chain, whose own stall rules bound the wait".to_owned(),
                    ));
                }
            }
            let why = format!("it gave no verdict within {minutes} minutes");
            self.me()
                .ok_or(Error::Stopping)?
                .core
                .stop(&finishing.judge, &why, &model::by_remit(""))
                .await?;
            why
        };
        // the end of the check ended the subscription with it; the delete covers a check that had ended
        let sub_id = sub.id.clone();
        self.store_done(
            d.done(&format!("ended the goal check: {why}"), &ch.id, ""),
            move |tx, _| Ok(tx.delete_subscription(&sub_id)?),
        )
        .await
    }
}
