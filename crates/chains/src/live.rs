//! The record adapter turns what Remit itself learns about a chain into the readings of the signals the chain may watch
//! (ADR-0013): chain:<id>.landing.<repository> says how its change stands on its way onto the base (ADR-0114), and
//! chain:<id>.live says the change is live. Each cause names who acted. What the forge says (checks that settle on a
//! landed commit) and a deploy the owner or his system reports are about the chain and not its doing, so they name no
//! chain and its own watch hears them; a version its land submits and a verdict its gate gives are its own acts, named
//! with the chain, so only the watches of the chains that serve the same objective hear them (ADR-0055).
//!
//! "Live" has one writer, `mark_live`, and three callers: the Assistant's mark_live on the owner's word, the release of a
//! repository for each chain whose change it carried, once it baked healthy (shipping's release_run.rs, by code), and a
//! rule of an event signal that does live (chain_jobs.rs), which names the chain by its marks. Marking live lifts no rail and lands nothing: it is a fact
//! recorded with who said it.

use std::collections::BTreeMap;

use model::{
    Actor, Approval, CHAIN_KEY_PREFIX, Cause, Checks, ForeignJson, Landing, LandingState, SignalKey, Time, Verdict,
    Writer, WriterKind,
};
use serde::{Deserialize, Serialize};

use crate::apply::Records;
use crate::chain_core::Core;
use crate::chain_core::first_line;
use crate::fact::{FactValue, fact_write, keys};
use crate::ingest::Write;
use crate::outbox::actor_words;
use crate::step::{ChainCmd, MarkLiveCmd};
use crate::{Error, Result};

/// What begins every key of chain id's own records.
pub fn own_key_prefix(id: &str) -> String {
    format!("{CHAIN_KEY_PREFIX}{id}.")
}

/// The signal that says chain id's change is live.
pub fn live_key(id: &str) -> SignalKey {
    SignalKey(format!("{}live", own_key_prefix(id)))
}

/// The signal that says how chain id's change to repo stands on its way onto the base.
pub fn landing_key(id: &str, repo: &str) -> SignalKey {
    SignalKey(format!("{}landing.{repo}", own_key_prefix(id)))
}

/// The writer of the readings of chain id's record: its landings and its live mark.
pub fn record_writer(id: &str) -> Writer {
    Writer {
        kind: WriterKind::Record,
        id: id.to_owned(),
    }
}

/// What a reading of chain:<id>.live says.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct LiveValue {
    at: Time,
    by: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
    /// When the chain's newest landing reached its base: the landing this reading marks live.
    #[serde(skip_serializing_if = "Option::is_none")]
    landed: Option<Time>,
}

/// What a reading of chain:<id>.landing.<repository> says: the landing, its state, the version it lands, the commit it
/// landed as and the forge's checks on it, the latest verdict of each gate on that version's change and who made the
/// version (ADR-0055). One reading carries it all, so one condition reads them together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandingValue {
    id: String,
    state: LandingState,
    version: i64,
    from: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    checks: Option<Checks>,
    #[serde(skip_serializing_if = "Option::is_none")]
    landed_at: Option<Time>,
    /// By gate, the latest on the version's change; {} when none.
    verdicts: BTreeMap<&'static str, GateVerdict>,
    /// The employees who made the version; [] when none.
    by: Vec<String>,
}

/// A gate's latest verdict on the version a landing lands, and who gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct GateVerdict {
    verdict: Verdict,
    name: String,
    at: Time,
}

/// A landing a change stores, and who moved it: the change's transaction writes the reading of it when what it says
/// moved (`landing_moved`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LandingWrite {
    pub landing: Landing,
    pub cause: Cause,
}

/// What the reading of landing l says, with the verdicts among approvals on its change: of each change gate (gate.rs)
/// the last in store order. Pure.
pub fn landing_value_of(l: &Landing, approvals: &[Approval]) -> LandingValue {
    let mut verdicts = BTreeMap::new();
    for a in approvals {
        let Some(gate) = a.role.gate() else {
            continue;
        };
        if a.repo == l.repo && !l.version.change.is_empty() && a.change == l.version.change {
            verdicts.insert(
                gate.as_str(),
                GateVerdict {
                    verdict: a.verdict,
                    name: a.name.clone(),
                    at: a.at,
                },
            );
        }
    }
    LandingValue {
        id: l.id.clone(),
        state: l.state.clone(),
        version: l.version.number,
        from: l.version.from.clone(),
        commit: l.commit.clone().unwrap_or_default(),
        checks: l.checks.clone(),
        landed_at: l.landed_at,
        verdicts,
        by: l.version.by.clone(),
    }
}

/// The reading the move of landing l from was to now makes, by cause: one when the two differ, none when they are the
/// same. The one rule for when the reading is written, whoever moved it (land, the queue, the forge's checks, a
/// verdict). A revert is the queue's own landing and says nothing of a chain's change: none. Pure.
pub fn landing_moved(
    l: &Landing,
    was: Option<&LandingValue>,
    now: &LandingValue,
    cause: &Cause,
) -> Result<Option<Write>> {
    if l.reverts.is_some() {
        return Ok(None);
    }
    let value = serde_json::to_string(now)?;
    if let Some(was) = was
        && serde_json::to_string(was)? == value
    {
        return Ok(None);
    }
    Ok(Some(Write {
        key: landing_key(&l.chain_id, &l.repo),
        cause: cause.clone(),
        value: Some(ForeignJson::parse(&value)?),
        writer: record_writer(&l.chain_id),
        ..Write::default()
    }))
}

/// When the newest of landings, a chain's, reached its base and stays there; none when none does. Pure.
pub fn landed_at(landings: &[Landing]) -> Option<Time> {
    landings
        .iter()
        .filter(|l| l.reverts.is_none() && l.on_base())
        .filter_map(|l| l.landed_at)
        .max()
}

impl Core {
    /// Whether the newest landing of chain ch was marked live already, under the chain's lock: a change already marked
    /// live for that landing is marked once (a retry, or the same deploy reported twice). A mark it cannot read refuses
    /// the command: taken as unmarked, it would be marked twice.
    pub async fn observe_mark_live(&self, mut c: MarkLiveCmd, ch: &model::Chain) -> Result<MarkLiveCmd> {
        c.already = self.marked_live(&ch.id).await?;
        Ok(c)
    }

    /// When the newest landing of chain id reached its base (`landed_at`), as stored.
    pub async fn chain_landed_at(&self, id: &str) -> Result<Option<Time>> {
        let id = id.to_owned();
        let landings = self.store.read(move |r| r.chain_landings(&id)).await?;
        Ok(landed_at(&landings))
    }

    /// Says the newest landing of chain id was marked live.
    pub async fn marked_live(&self, id: &str) -> Result<bool> {
        let Some(landed) = self.chain_landed_at(id).await? else {
            return Ok(false);
        };
        let key = live_key(id);
        let last = match self.store.read(move |r| r.last_reading(&key)).await {
            Ok(last) => last,
            Err(store::Error::NotFound) => return Ok(false),
            Err(err) => return Err(err.into()),
        };
        let v: LiveValue = serde_json::from_value(last.value.as_ref().map_or(serde_json::Value::Null, |j| j.0.clone()))
            .map_err(|e| Error::refused(format!("the live mark of chain {id}: {e}")))?;
        Ok(v.landed == Some(landed))
    }

    /// Says the change of chain id is live, as by (who said so) and with note: a reading of chain:<id>.live that its
    /// watch hears, a decision the owner reads, and the fact fact:chain.marked-live, which has the goal judge look again
    /// (ADR-0067). A change already marked live for its landing is not marked again.
    pub async fn mark_live(&self, chain_id: &str, by: Actor, note: &str) -> Result<()> {
        let v = LiveValue {
            at: self.now(),
            by: actor_words(&by),
            note: first_line(note, 300),
            landed: self
                .chain_landed_at(chain_id)
                .await
                .map_err(|e| Error::refused(format!("chain {chain_id:?}: {e}")))?,
        };
        let r = Write {
            key: live_key(chain_id),
            cause: Cause {
                actor: by.clone(),
                chain_id: String::new(),
            },
            value: Some(ForeignJson(serde_json::to_value(&v)?)),
            writer: record_writer(chain_id),
            ..Write::default()
        };
        let fact = fact_write(
            keys::chain_marked_live(),
            &FactValue {
                chain: chain_id.to_owned(),
                ..FactValue::default()
            },
            by.clone(),
            chain_id,
        )?;
        // The next tick answers the fact, as it answers fact:chain.landed: a mark made by an event signal's rule is made
        // inside a firing's delivery.
        let cmd = ChainCmd::MarkLive(MarkLiveCmd {
            by,
            note: note.to_owned(),
            already: false,
        });
        self.apply(
            chain_id,
            cmd,
            Records {
                readings: vec![r, fact],
                ..Records::default()
            },
        )
        .await?;
        self.kick();
        Ok(())
    }
}
