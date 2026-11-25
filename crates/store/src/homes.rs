//! Homes
//!
//! A home is where cards work: a chain or the Assistant. What is recorded for a card (its events, session init and context, the
//! owner's drafts and inbox items for it, its workflow runs and their steps) names its home, and what only a chain has
//! (edges, decisions, verdicts, retrospectives, seeded faults, duels, helps, second opinions, grants, the
//! sensors its scripts read through) names it as chain_id or chain, and the owner's seen mark on it names it as its
//! subject. A home that goes takes all of it: delete_home is the one way, whoever removes the
//! home.

use crate::store::{Result, Tx};

/// The tables delete_home removes a home's records from, each with its column that names the home. A table that names
/// a home is listed here or in KEPT_PAST_HOME.
pub const HOME_RECORDS: &[(&str, &str)] = &[
    ("approvals", "chain_id"),
    ("cards", "home"),
    ("decisions", "chain_id"),
    ("drafts", "home"),
    ("duels", "chain"),
    ("edges", "chain_id"),
    ("events", "home"),
    // the grants a chain holds; a repository's are kept (`grants.chain_id` is null for them)
    ("grants", "chain_id"),
    ("helps", "chain"),
    ("attachments", "home"),
    ("inbox", "home"),
    ("retrospectives", "chain_id"),
    ("second_opinions", "chain"),
    ("seeded_faults", "chain_id"),
    ("seen", "subject"),
    ("sensors", "chain"),
    ("session_contexts", "home"),
    ("session_launches", "home"),
    ("session_inits", "home"),
    ("workflow_steps", "home"),
    ("workflows", "home"),
];

/// The tables that name a home and keep its rows when it goes, each with why.
pub const KEPT_PAST_HOME: &[(&str, &str)] = &[
    (
        "cleanups",
        "the files a deleted chain leaves, stored with its deletion until they are removed",
    ),
    (
        "feedback",
        "what agents told the organisation is read by the owner long after the chain that sent it",
    ),
    (
        "landings",
        "what reached a repository's base, and what was taken back, is the repository's history and its releases': the queue reverts it from its record, and the outcomes count it",
    ),
    (
        "source_calls",
        "the account of what an agent did to a system outside Remit outlives the chain that did it",
    ),
];

impl Tx<'_> {
    /// Removes everything recorded under home: its cards with their events, queued messages and session inits, the
    /// owner's drafts and inbox items for it, and what only a chain has. Part of a Tx: a home goes with the record that
    /// held it (a chain).
    pub fn delete_home(&self, home: &str) -> Result<()> {
        for (table, column) in HOME_RECORDS {
            self.exec(&format!("DELETE FROM {table} WHERE {column}=?"), [home])?;
        }
        Ok(())
    }
}
