//! Making a subscription: its settings checked, its place in the log set, a schedule stored as a wait for time.

use model::{Start, StartFrom, Subscription, Time};
use store::Tx;

use crate::timewait::wait_for;
use crate::wait::rfc3339;
use crate::{Error, Result};
use model::ActionKind;
use model::ProcessorTakes;
use model::Takes;

/// Stores sub in tx, a subscription its maker gives whole, starting where `from` says, and returns it as stored (docs/adr/0064,
/// Start position and retention). It is the one path a subscription is made by, so each is made the same way: its settings are
/// checked against its processor (`check_subscription`), its place in the log is set from its start (`start_seq`), and one
/// that keeps a schedule is stored as a wait for time (`wait_for`).
pub fn subscribe(tx: &Tx<'_>, mut sub: Subscription, from: &Start, now: Time) -> Result<Subscription> {
    check_subscription(&sub)?;
    sub.seen_seq = start_seq(tx, &sub, from, now)?;
    if sub.schedule.is_some() {
        return Ok(wait_for(tx, sub)?);
    }
    Ok(tx.put_subscription(sub)?)
}

/// The place in the log sub judges after when it starts at `from`, read in tx at now: the head of the log for now; just
/// before a place, or before the first reading stored at or after a time; just before its key's newest reading for current,
/// so that reading is judged (the head while the key has none). A schedule judges no reading, and has no place.
pub fn start_seq(tx: &store::Reader<'_>, sub: &Subscription, from: &Start, now: Time) -> Result<i64> {
    from.check(sub).map_err(|e| Error::refused(e.to_string()))?;
    if sub.schedule.is_some() {
        return Ok(0);
    }
    match &from.from {
        StartFrom::Seq => {
            let head = tx.last_seq()?;
            if from.seq > head.saturating_add(1) {
                return Err(Error::refused(format!(
                    "the newest place in the log is {head}: a start from {} is past it",
                    from.seq
                )));
            }
            return Ok(from.seq.saturating_sub(1));
        }
        StartFrom::Time => {
            let Some(at) = from.at else {
                return Err(Error::refused("a start from a time names that time"));
            };
            if at > now {
                return Err(Error::refused(format!(
                    "a start from a time judges what was stored since, and {} is still to come (now is {})",
                    rfc3339(at),
                    rfc3339(now)
                )));
            }
            return Ok(tx.seq_before(at)?);
        }
        StartFrom::Current => {
            if let Some(key) = sub.r#match.exact() {
                match tx.last_reading(&key) {
                    Ok(last) => return Ok(last.seq.saturating_sub(1)),
                    Err(store::Error::NotFound) => {}
                    Err(err) => return Err(Error::refused(format!("the newest reading of {key}: {err}"))),
                }
            }
        }
        _ => {} // now, and current on a key with no reading, start at the head below
    }
    Ok(tx.last_seq()?)
}

/// Where a subscription starts judging, as the tools take it (`model::Start`): now, the events stored after it is made;
/// seq, the events from a place in the log on; time, the events stored from a time on (RFC 3339 with an offset).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct StartInput {
    /// now: the events stored after it is made; seq: the events from a place in the log on (a reading's seq, as
    /// read_signals shows it); time: the events stored from a time on.
    pub from: String,
    /// For seq: the first place in the log it judges.
    pub seq: i64,
    /// For time: the first time it judges, RFC 3339 with an offset.
    pub time: String,
}

impl StartInput {
    /// The start it says.
    pub fn start(&self) -> Result<Start> {
        let mut st = Start {
            from: self
                .from
                .trim()
                .parse()
                .map_err(|e: model::UnknownValue| Error::refused(e.to_string()))?,
            seq: self.seq,
            ..Start::default()
        };
        if st.from == StartFrom::Time {
            let at = time::OffsetDateTime::parse(self.time.trim(), &time::format_description::well_known::Rfc3339)
                .map_err(|_| Error::refused("a start from a time takes time, RFC 3339 with an offset"))?;
            st.at = Some(Time(at));
        }
        Ok(st)
    }
}

/// Most act on one reading at a time.
fn takes_one() -> ProcessorTakes {
    ProcessorTakes {
        readings: Takes::One,
        expires: false,
    }
}

/// One reading, or a deadline or a schedule's instant with none.
fn takes_one_or_expiry() -> ProcessorTakes {
    ProcessorTakes {
        readings: Takes::One,
        expires: true,
    }
}

/// A wake reads the newest of all a firing carries, and a deadline fires it too.
fn takes_all() -> ProcessorTakes {
    ProcessorTakes {
        readings: Takes::Many,
        expires: true,
    }
}

/// What every plugin's processor takes: one reading at a time, or none when a deadline fires it (its protocol carries one
/// reading). A plugin's action is checked against it whether the plugin is enabled or not: a bundled plugin's
/// subscription ships before the owner enables it.
pub fn plugin_takes() -> ProcessorTakes {
    takes_one_or_expiry()
}

/// What the in-process processor of kind takes; none for a kind Remit does not do itself.
pub fn builtin_takes(kind: &ActionKind) -> Option<ProcessorTakes> {
    match kind {
        ActionKind::Start | ActionKind::End | ActionKind::Run | ActionKind::Deadline => Some(takes_one_or_expiry()),
        ActionKind::Wake | ActionKind::Workflow => Some(takes_all()),
        ActionKind::Tell | ActionKind::Hire | ActionKind::Assist => Some(takes_one()),
        _ => None,
    }
}

/// What the processor of kind takes; none when no processor Remit or a plugin has does it. A plugin's action is judged
/// by what every plugin's processor takes.
pub fn takes_of(kind: &ActionKind) -> Option<ProcessorTakes> {
    builtin_takes(kind).or_else(|| matches!(kind, ActionKind::Plugin(_)).then(plugin_takes))
}

/// Whether sub can be done: a processor serves its action, and its settings suit what that processor takes
/// (`Subscription::check`). It answers the agent that asked for sub at once, with the reason.
pub fn check_subscription(sub: &Subscription) -> Result<()> {
    let Some(takes) = takes_of(&sub.action.kind) else {
        return Err(Error::refused(format!(
            "no processor does {:?}: the action is not one Remit or an enabled plugin does",
            sub.action.kind.as_str()
        )));
    };
    sub.check(&takes).map_err(|e| Error::refused(e.to_string()))
}

impl crate::chain_core::Core {
    /// Ends subscription id, whose subject is gone or has nothing left to end, with no firing.
    pub async fn drop_subscription(&self, id: &str) -> Result<()> {
        let id = id.to_owned();
        Ok(self.store.update(move |tx| tx.delete_subscription(&id)).await?)
    }
}
