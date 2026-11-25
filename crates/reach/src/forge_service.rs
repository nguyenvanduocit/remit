//! What Remit answers a plugin granted `forge.read` (ADR-0057, ADR-0072 decision 7, ADR-0114) is decided here and nowhere
//! else. Today it is the one question a landing sense needs, the checks the forge's pipeline ran on a commit Remit landed,
//! answered only for the commits that the settings of an unpaused plugin sensor name: Remit set that sensor for a chain's
//! landing. The forge's login stays with Remit, and the service has no method that merges, opens or pushes. The seam for the
//! control not built is the manifest's capabilities (ADR-0057, Revisit when).

use bundled::{FORGE_PLUGIN, LANDING_KIND, LandingSettings};
use forge::Repo;
use link::{ErrorCode, Failure};
use serde_json::Value;

use crate::Reach;
use crate::sources::Bound;
use chains::forges::FORGE_TIMEOUT;

/// The forge service the bundled forge plugin is granted (plugins.rs `BundledPlugins`).
pub(crate) struct ForgeLent(pub(crate) Bound);

#[async_trait::async_trait]
impl plugin_sdk::Forge for ForgeLent {
    async fn checks(&self, host: String, project: String, sha: String) -> Result<Value, Failure> {
        let o = self
            .0
            .get()
            .map_err(|e| Failure::new(ErrorCode::Unavailable, e.to_string()))?;
        o.lent_checks(&host, &project, &sha).await
    }
}

impl Reach {
    /// The checks the pipeline of project on host ran on commit sha, when a landing sensor's settings name that commit.
    pub(crate) async fn lent_checks(&self, host: &str, project: &str, sha: &str) -> Result<Value, Failure> {
        let sensors = self
            .store
            .read(|r| r.sensors())
            .await
            .map_err(|e| Failure::new(ErrorCode::InternalError, e.to_string()))?;
        // Only the forge plugin's own sensor kind: another plugin's settings could name any commit.
        let kind = model::plugin_name(FORGE_PLUGIN, LANDING_KIND);
        let named = sensors
            .iter()
            .filter(|s| !s.paused && s.kind == kind)
            .filter_map(|s| serde_json::from_value::<LandingSettings>(s.settings.as_ref()?.0.clone()).ok())
            .any(|st| {
                st.landings
                    .iter()
                    .any(|w| w.host == host && w.project == project && (w.push == sha || w.onto == sha))
            });
        if !named {
            return Err(Failure::new(
                ErrorCode::Forbidden,
                "the settings of this sensor do not name that commit",
            ));
        }
        let fg = self
            .forge_on(host)
            .map_err(|e| Failure::new(ErrorCode::Unavailable, e.to_string()))?;
        let repo = Repo {
            host: host.to_owned(),
            project: project.to_owned(),
        };
        let checks = match tokio::time::timeout(FORGE_TIMEOUT, fg.checks(&repo, sha)).await {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(Failure::new(ErrorCode::Failed, e.to_string())),
            Err(_) => {
                return Err(Failure::new(
                    ErrorCode::Timeout,
                    format!("{host} did not answer in {}s", FORGE_TIMEOUT.as_secs()),
                ));
            }
        };
        serde_json::to_value(checks).map_err(|e| Failure::new(ErrorCode::InternalError, e.to_string()))
    }
}
