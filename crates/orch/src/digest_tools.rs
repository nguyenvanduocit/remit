//! The Assistant's digest tools: how the owner gets a digest of the last day or week in its panel, at the time he says, by
//! telling it. Each is the orchestrator's own action (digest.rs), taken as the Assistant for him, so the digest's
//! subscription shows on the Subscriptions tab at once with who set it. A digest only reads and posts, so the Assistant
//! may set and stop it.

use std::sync::Arc;

use model::Actor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::orch::Orchestrator;
use chains::tools::{Effect, NoInput, Tools, as_json};
use chains::wait::rfc3339;
use pipeline::automation::DigestPeriod;

/// A digest as set_digest takes it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetDigestInput {
    /// day: what happened over the 24 hours before each posting; week: over the 7 days before it
    period: DigestPeriod,
    /// when to post it: five fields (minute hour day-of-month month day-of-week) in the server's time zone, e.g. '0 18 * * *' for every day at 18:00 or '0 9 * * 1' for Mondays at 09:00; never more often than once an hour
    cron: String,
}

/// A digest, by its period.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DigestPeriodInput {
    /// day or week
    period: DigestPeriod,
}

/// A digest the owner gets, as list_digests shows it.
#[derive(Serialize)]
struct DigestRecord {
    period: &'static str,
    cron: String,
    /// When it is next posted, RFC 3339; empty while it is paused or held.
    next: String,
    paused: bool,
}

impl Orchestrator {
    /// Gives the Assistant, acting as by, the tools that set, list and stop the owner's digests.
    pub(crate) fn add_digest_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        t.serving(self).add(
            "list_digests", Effect::Reads,
            "The digests the owner gets in your panel: each period (day or week), when it is posted (its cron), when it is next posted, and whether it is paused.",
            |o, _: NoInput| async move {
                let all: Vec<DigestRecord> = o
                    .digests()
                    .await?
                    .into_iter()
                    .map(|(p, s)| DigestRecord {
                        period: p.label(),
                        cron: s.schedule.map(|sch| sch.cron).unwrap_or_default(),
                        next: s.due_at.map(rfc3339).unwrap_or_default(),
                        paused: s.control == model::Control::Paused,
                    })
                    .collect();
                as_json(&all)
            },
        );
        let b = by.clone();
        t.serving(self).add(
            "set_digest", Effect::Writes,
            "Post a digest in your panel at the times the owner tells you: the chains that finished over the last day or week and how each ended (done, waiting to ship, not done), what you decided for him and what he overturned. Remit writes it from what it recorded, with no model call, so it reads nothing in and asks you nothing. One digest per period: setting one that exists gives it the new time. It shows on the Subscriptions tab.",
            move |o, i: SetDigestInput| {
                let by = b.clone();
                async move {
                    let s = o.set_digest(i.period, &i.cron, by).await?;
                    Ok(format!(
                        "The digest of {} is posted in your panel at cron {}; the next at {}.",
                        i.period.label(),
                        i.cron.trim(),
                        s.due_at.map(rfc3339).unwrap_or_default()
                    ))
                }
            },
        );
        t.serving(self).add(
            "stop_digest", Effect::Writes,
            "Stop posting the digest of a period, when the owner no longer wants it. The digests already posted stay in your panel.",
            |o, i: DigestPeriodInput| async move {
                o.stop_digest(i.period).await?;
                Ok(format!("The digest of {} is no longer posted.", i.period.label()))
            },
        );
    }
}
