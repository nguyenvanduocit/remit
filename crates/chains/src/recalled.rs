//! What an employee's sessions recalled in a chain (ADR-0082): read from the fact memory.recalled, which crew's
//! search_memory records, by read_chain and the coach.

use model::Reading;
use serde::Serialize;

use crate::Result;
use crate::chain_core::Core;
use crate::fact::{fact_value_of, keys};

/// What one card's sessions recalled in a chain: the ids of the memories, each once, in the order first recalled.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Recalled {
    pub card: String,
    pub employee: String,
    pub memories: Vec<String>,
}

/// How many of the newest recalls a chain's are looked for among: far more than an instance makes in months.
const RECALLS_READ: usize = 5000;

/// What the sessions of chain chain_id recalled, per card in the order each first recalled, from readings of
/// memory.recalled, newest first. Pure.
fn recalled_in(readings: &[Reading], chain_id: &str) -> Vec<Recalled> {
    let mut out: Vec<Recalled> = Vec::new();
    for r in readings.iter().rev() {
        let Ok(v) = fact_value_of(r) else { continue };
        if v.chain != chain_id {
            continue;
        }
        let at = match out.iter().position(|c| c.card == v.card) {
            Some(at) => at,
            None => {
                out.push(Recalled {
                    card: v.card.clone(),
                    employee: v.employee.clone(),
                    memories: Vec::new(),
                });
                out.len().saturating_sub(1)
            }
        };
        let Some(c) = out.get_mut(at) else { continue };
        for id in v.memories {
            if !c.memories.contains(&id) {
                c.memories.push(id);
            }
        }
    }
    out
}

impl Core {
    /// What the sessions of chain chain_id recalled, per card, read from the newest `RECALLS_READ` recalls of every chain;
    /// true with them when that many were read, so older ones may be left out.
    pub async fn recalled_of(&self, chain_id: &str) -> Result<(Vec<Recalled>, bool)> {
        let limit = i64::try_from(RECALLS_READ).unwrap_or(i64::MAX);
        let rs = self
            .store
            .read(move |r| r.newest_readings(&keys::memory_recalled(), limit))
            .await?;
        Ok((recalled_in(&rs, chain_id), rs.len() == RECALLS_READ))
    }
}
