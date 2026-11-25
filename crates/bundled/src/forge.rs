//! The forge plugin senses what Remit landed (ADR-0057, ADR-0114). Remit lands every change itself, through its merge queue;
//! the sensor kind `landing` asks Remit, through the forge it is granted (`forge.read`), for the checks the forge's pipeline
//! ran on each commit a landing pushed and on the commit the base stood at before it, and writes them as a reading of the
//! landing when they differ from what it wrote last. The plugin holds no login: the forge's is the machine's and stays with
//! Remit, which answers only for the commits the settings name, and which alone records what the checks mean for a chain.

use std::collections::BTreeMap;

use plugin_sdk::{Call, Definition, Emitter, Error, Reading, Sensing, SensorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The id the plugin ships under.
pub const FORGE_PLUGIN: &str = "forge";

/// The name of the sensor kind the plugin adds.
pub const LANDING_KIND: &str = "landing";

/// One landing a sensor follows: the landing it is (the word its readings are written under), and the host, project and two
/// commits the forge's pipeline runs on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LandingWatch {
    /// the landing's id, the word its readings are written under
    pub landing: String,
    /// the host of the forge
    pub host: String,
    /// the project on the forge: GitLab's group/project, GitHub's owner/name
    pub project: String,
    /// the commit the queue pushed
    pub push: String,
    /// the commit the base stood at before it
    pub onto: String,
}

/// The settings of a landing sensor: the landings it follows. Remit keeps them in step with the landings of the chain the
/// sensor belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LandingSettings {
    /// the landings to follow
    pub landings: Vec<LandingWatch>,
}

/// The bundled plugin that senses landings.
pub(crate) fn plugin() -> Definition {
    Definition {
        label: "Landings".into(),
        capability: "data".into(),
        sensor_kinds: vec![SensorKind::new(
            LANDING_KIND,
            "The checks the forge's pipeline ran on what a chain landed",
            sense,
        )],
        ..Default::default()
    }
}

/// Asks for the checks of each landing of the settings and writes those that are not the ones written last: the cursor is one
/// JSON object, the value last written for each landing, so two landings do not overwrite each other's. Checks that cannot
/// be read are told at the end and do not stop the others, so one forge that is down does not silence another's.
async fn sense(call: Call, s: Sensing<LandingSettings>, out: Emitter) -> Result<(), Error> {
    // A cursor that is no object is none: every landing is written once.
    let last: BTreeMap<String, String> = serde_json::from_str(&s.cursor).unwrap_or_default();
    let mut kept: BTreeMap<String, String> = BTreeMap::new();
    let mut failed = Vec::new();
    for w in &s.settings.landings {
        let before = last.get(&w.landing).cloned().unwrap_or_default();
        kept.insert(w.landing.clone(), before.clone());
        let value = match checks(&call, w).await {
            Ok(v) => v,
            Err(e) => {
                failed.push(format!("{}: {e}", w.landing));
                continue;
            }
        };
        if value == before {
            continue;
        }
        out.reading(Reading {
            key_suffix: w.landing.clone(),
            value: serde_json::from_str(&value)?,
            dedupe: String::new(),
        })
        .await?;
        kept.insert(w.landing.clone(), value);
        out.cursor(&serde_json::to_string(&kept)?).await?;
    }
    if failed.is_empty() {
        return Ok(());
    }
    Err(failed.join("\n").into())
}

/// What a reading of `w` says: the checks on the commit pushed and on the one it was landed onto, each null while the
/// pipeline has run none, as one JSON object whose keys come in one order, so two answers that say the same are the same text.
async fn checks(call: &Call, w: &LandingWatch) -> Result<String, Error> {
    let push = call.checks(&w.host, &w.project, &w.push).await?;
    let onto = call.checks(&w.host, &w.project, &w.onto).await?;
    let value: BTreeMap<&str, Value> = BTreeMap::from([("onto", onto), ("push", push)]);
    Ok(serde_json::to_string(&value)?)
}
