//! The agents, skills and playbooks are written by agents, on the owner's word, and so are the records a change item
//! lets through (an employee, an objective, a grant...). Every write of one leaves a `model::Change`, stored in the
//! transaction that stores the record, so the owner can read who wrote what and what it was before. A write that moves a
//! rail waits for his answer first (rail.rs, change_item.rs).

use model::{Actor, Change, ChangeAction, ChangeKind, Time};
use serde::Serialize;
use store::Tx;

use crate::Result;
use crate::chain_core::new_id;

/// Who wrote a record, and who let the write through when it waited for the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorship {
    pub by: Actor,
    pub approved_by: Option<Actor>,
}

/// Who writes the built-in agents and playbooks when Remit starts.
pub fn the_seed() -> Authorship {
    Authorship {
        by: model::by_remit("seed"),
        approved_by: None,
    }
}

/// Stores the change from before to after, at at, in the transaction tx that writes the record: none for a record that
/// did not exist, and for one that is gone. It stores nothing when the two say the same, so a write that changes
/// nothing, or a start that finds a built-in as it ships, leaves no trace.
pub fn record_change<T: Serialize>(
    tx: &Tx<'_>,
    w: &Authorship,
    at: Time,
    kind: ChangeKind,
    name: &str,
    before: Option<&T>,
    after: Option<&T>,
) -> Result<()> {
    let (b, a) = (doc_of(before)?, doc_of(after)?);
    let fields = library::diff_fields(&b, &a);
    if fields.is_empty() {
        return Ok(());
    }
    tx.put_change(&Change {
        id: new_id("ch_"),
        kind,
        name: name.to_owned(),
        action: action_of(before.is_some(), after.is_some()),
        before: b,
        after: a,
        fields,
        by: w.by.clone(),
        approved_by: w.approved_by.clone(),
        at,
    })?;
    Ok(())
}

/// What a write did to a record that was there before it (existed) and is there after it (exists).
fn action_of(existed: bool, exists: bool) -> ChangeAction {
    if !existed {
        ChangeAction::Created
    } else if !exists {
        ChangeAction::Deleted
    } else {
        ChangeAction::Changed
    }
}

/// A record as the JSON it is stored as, and empty for a record that is not there.
pub fn doc_of<T: Serialize>(record: Option<&T>) -> Result<String> {
    match record {
        None => Ok(String::new()),
        Some(r) => Ok(serde_json::to_string(r)?),
    }
}

/// Whether two copies of a record say the same: no field of theirs differs but the count of writes.
pub fn same<T: Serialize>(a: &T, b: &T) -> Result<bool> {
    Ok(library::diff_fields(&doc_of(Some(a))?, &doc_of(Some(b))?).is_empty())
}
