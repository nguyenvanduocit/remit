//! A duel's marks the core reads (duel.rs in crew): who sends a side the pick, and the note at the head of a dueling
//! card's report. Pure.

use crate::Result;
use crate::chain_core::Core;
use model::{Actor, Card, Duel};

/// Who sends a dueling card the pick: its employee reads it, and its turn on it goes to no one (turn_end.rs).
pub fn duel_by() -> Actor {
    model::by_remit("duel")
}

/// What the Chair reads at the head of a report of card c held by duel d.
pub fn duel_note(d: &Duel, c: &Card) -> String {
    format!(
        " — card {} is in a duel with card {}: a judge picks one of the two reports once both have reported, and the winner's then waits for accept_work",
        c.id,
        d.other(&c.id).card
    )
}

impl Core {
    /// The duel card c is a side of while it stands: no judge yet and not ended. A report of c's then waits for the judge,
    /// not the Chair.
    pub async fn held_by_duel(&self, c: &Card) -> Result<Option<Duel>> {
        let home = c.home.clone();
        let duels = self.store.read(move |r| r.chain_duels(&home)).await?;
        Ok(duels
            .into_iter()
            .find(|d| d.side(&c.id).is_some() && d.judge.is_empty() && d.ended.is_empty()))
    }
}
