//! The owner is a mentor too (ADR-0062): m_owner, in the pool as any mentor, whom a Chair may bring onto a card. He
//! mentors the Assistant always: help_owner is his standing, accepted help on the Assistant's card, so a code the owner had
//! made for m_owner signs in as that mentor; he talks to the Assistant from its panel.

use model::{Help, HelpState, Mentor, MentorStatus};

use crate::Result;
use crate::chain_core::Core;
use crate::commit::OWNER_HELP_ID;

/// The owner as a mentor.
pub const OWNER_MENTOR_ID: &str = "m_owner";

impl Core {
    /// Makes m_owner and his help on the Assistant's card a, when they are not there: every start runs it, and a start
    /// that finds them writes nothing.
    pub async fn ensure_owner_mentor(&self, a: &model::Assistant) -> Result<()> {
        let (now, a) = (self.now(), a.clone());
        self.store
            .update(move |tx| -> Result<()> {
                match tx.mentor(OWNER_MENTOR_ID) {
                    Err(store::Error::NotFound) => {
                        tx.put_mentor(Mentor {
                            id: OWNER_MENTOR_ID.to_owned(),
                            label: "the owner".to_owned(),
                            strengths: "his own taste, his decisions and what he wants the product to be".to_owned(),
                            status: MentorStatus::Active,
                            created_at: now,
                            ..Mentor::default()
                        })?;
                    }
                    Ok(_) => {}
                    Err(e) => return Err(e.into()),
                }
                match tx.help(OWNER_HELP_ID) {
                    Err(store::Error::NotFound) => {
                        tx.put_help(Help {
                            id: OWNER_HELP_ID.to_owned(),
                            chain: a.id,
                            card: a.card_id,
                            mentor: OWNER_MENTOR_ID.to_owned(),
                            problem: "the owner mentors his Assistant".to_owned(),
                            state: HelpState::Accepted,
                            offered_at: now,
                            deadline: now,
                            answered_at: Some(now),
                            ..Help::default()
                        })?;
                        Ok(())
                    }
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.into()),
                }
            })
            .await
    }
}
