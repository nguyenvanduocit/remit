//! The subscriptions a chain's step stores with the chain: the wake subscriptions it waits on, and the ones Remit makes
//! for it (a goal check's deadline). They are written in the transaction that stores the chain, so a wait and the state it
//! belongs to are one fact.

use std::collections::HashMap;

use model::{Start, StartFrom, Time};
use store::Tx;

use crate::Result;
use crate::goal::owned_by;
use crate::step::Transition;
use crate::subscribe::subscribe;
use crate::wait::{wait_owner, wakes_by, wakes_in};
use crate::watch::wake_start;
use wire::app::CheckBack;

/// Stores the subscriptions of Remit's own that t makes for its chain, and ends the ones it ends: a chain that is closed
/// out ends every one it holds (`owned_by`) and every sensor it reads through.
fn put_own_subs(tx: &Tx<'_>, t: &Transition, now: Time) -> Result<()> {
    if t.closed_out_by.is_some() {
        tx.delete_sensors_of(&t.next.id)?;
        tx.delete_subscriptions_of(&owned_by(&t.next.id))?;
        return Ok(());
    }
    for id in &t.end_subs {
        tx.delete_subscription(id)?;
    }
    let from_now = Start {
        from: StartFrom::Now,
        ..Start::default()
    };
    for s in &t.subs {
        subscribe(tx, s.clone(), &from_now, now)?;
    }
    Ok(())
}

/// Stores the change t makes to the wake subscriptions of its chain, in the transaction that stores the chain, and
/// returns what the chain waits for after it. A chain that is finished waits for nothing: its watches end.
pub fn put_waits(tx: &Tx<'_>, t: &Transition, now: Time) -> Result<HashMap<String, CheckBack>> {
    let owner = wait_owner(&t.next.id);
    if t.end_waits || t.closed_out_by.is_some() || !t.end_watches.is_empty() {
        for w in wakes_in(tx.subscriptions_of(&owner)?).iter() {
            if t.ends(&w.sub) {
                tx.delete_subscription(&w.sub.id)?;
            }
        }
    }
    if let Some(s) = &t.new_wait {
        subscribe(tx, s.clone(), &wake_start(s), now)?;
    }
    put_own_subs(tx, t, now)?;
    Ok(wakes_by(tx.subscriptions_of(&owner)?))
}
