//! The Chair's team: the agents of its chain it hired, never the Chair itself nor a card Remit started
//! (`Card::on_team`). A tool that names a card by id names one of them, and a call that named something else is told who
//! the team is, so its retry names an id that works.

use model::Card;

use crate::chain_core::Core;
use crate::{Error, Result};

impl Core {
    /// The card of chain chain_id's team that card_id names, refused for any other card.
    pub async fn team_card(&self, chain_id: &str, card_id: &str) -> Result<Card> {
        let id = card_id.to_owned();
        match self.store.read(move |r| r.card(&id)).await {
            Ok(c) if c.home == chain_id && c.on_team() => Ok(c),
            _ => Err(Error::refused(format!(
                "card {card_id:?} is not an agent of your team{}",
                self.team_text(chain_id).await
            ))),
        }
    }

    /// Names the cards a tool may address by id; "" when they cannot be read.
    async fn team_text(&self, chain_id: &str) -> String {
        let key = chain_id.to_owned();
        let Ok(cards) = self.store.read(move |r| r.cards(&key)).await else {
            return String::new();
        };
        team_text_of(&cards)
    }
}

/// What a refusal says of the team among cards. Pure.
fn team_text_of(cards: &[Card]) -> String {
    let team: Vec<String> = cards
        .iter()
        .filter(|c| c.on_team())
        .map(|c| format!("{} ({})", c.id, c.name))
        .collect();
    if team.is_empty() {
        return "; the team has no agent yet: hire one".to_owned();
    }
    format!("; the team is: {}", team.join(", "))
}
