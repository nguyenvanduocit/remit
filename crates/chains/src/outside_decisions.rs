//! The decisions people made outside Remit (with a customer, on a call), stored as chain decisions of kind outside
//! (ADR-0051 Time and accountability): every later brief in the chain lists them by id, so work that rests on one can
//! cite it.

use model::{Decision, DecisionKind, Time};

use crate::Result;
use crate::chain_core::Core;
use crate::wait::clock;

/// What a brief says of the decisions people made for its chain, ds, oldest first: each by its id, so the work that rests
/// on one cites it; "" for none. Pure.
pub fn decided_brief(ds: &[Decision]) -> String {
    if ds.is_empty() {
        return String::new();
    }
    let mut b = String::from(
        "\n\nDECISIONS PEOPLE MADE FOR THIS CHAIN (outside Remit; cite one by its id when your work rests on it):",
    );
    for d in ds {
        // with its date: a brief is read on later days
        b.push_str(&format!(
            "\n- {} ({}): {}",
            d.id,
            clock(d.created_at, Time::default()),
            d.text
        ));
    }
    b
}

impl Core {
    /// `decided_brief` of the decisions people made for chain chain_id.
    pub async fn chain_decided(&self, chain_id: &str) -> Result<String> {
        let id = chain_id.to_owned();
        let ds = self
            .store
            .read(move |r| r.decisions_of_kind(&id, &DecisionKind::Outside))
            .await?;
        Ok(decided_brief(&ds))
    }
}
