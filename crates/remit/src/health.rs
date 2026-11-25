//! `/healthz`: 200 while the database answers, the scheduler has ticked within `STALLED_AFTER` and the AI runtime is
//! connected, 503 with the reason otherwise. A supervisor or an uptime check calls it, so it takes no sign-in; it says
//! the commit, the last tick and a reason that names no path or credential. `verdict` is pure; the handler reads.

use std::time::Duration;

use model::Time;
use serde::Serialize;

/// How often the scheduler ticks. A scheduler that has not got through one in three is stalled.
pub const TICK_EVERY: Duration = Duration::from_secs(60);
const STALLED_AFTER: time::Duration = time::Duration::minutes(3);
/// How long a probe waits for the database.
pub const PING_FOR: Duration = Duration::from_secs(2);

/// Which commit the binary was built from (build.rs): "unknown" when it was built outside a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    pub revision: String,
    /// The tree had uncommitted changes.
    pub modified: bool,
}

/// The build of the running binary.
pub fn this_build() -> Build {
    Build {
        revision: env!("REMIT_REVISION").to_owned(),
        modified: env!("REMIT_MODIFIED") == "true",
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    pub ok: bool,
    pub revision: String,
    pub modified: bool,
    pub scheduler_tick_at: Option<Time>,
    pub tick_age_seconds: Option<i64>,
    pub runtime: RuntimeBody,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// The AI runtime beside the core: whether it is connected, the build it runs, and how often it restarted.
#[derive(Debug, Serialize, PartialEq)]
pub struct RuntimeBody {
    pub connected: bool,
    pub revision: String,
    pub restarts: u64,
}

/// What /healthz answers, from what was read: whether the database answered, the last tick (none before the first), the
/// runtime's state and the time now.
pub fn verdict(
    db_answers: bool,
    last_tick: Option<Time>,
    runtime: &runtimelink::State,
    now: Time,
    build: &Build,
) -> Body {
    let mut problems = Vec::new();
    if !db_answers {
        problems.push("database does not answer".to_owned());
    }
    let (mut at, mut age_secs) = (None, None);
    if let Some(last_tick) = last_tick {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the difference of two clock readings, far inside OffsetDateTime's range"
        )]
        let age = now.0 - last_tick.0;
        at = Some(Time(last_tick.0.to_offset(time::UtcOffset::UTC)));
        age_secs = Some(age.whole_seconds());
        if age > STALLED_AFTER {
            problems.push(format!(
                "scheduler last ticked {}s ago, more than {}s",
                age.whole_seconds(),
                STALLED_AFTER.whole_seconds()
            ));
        }
    } else {
        problems.push("scheduler has not ticked yet".to_owned());
    }
    if !runtime.connected {
        problems.push("the AI runtime is not connected".to_owned());
    }
    Body {
        ok: problems.is_empty(),
        revision: build.revision.clone(),
        modified: build.modified,
        scheduler_tick_at: at,
        tick_age_seconds: age_secs,
        runtime: RuntimeBody {
            connected: runtime.connected,
            revision: runtime.revision.clone(),
            restarts: runtime.restarts,
        },
        reason: problems.join("; "),
    }
}
