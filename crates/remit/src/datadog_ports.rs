//! What the Datadog exporter is given by the server (ADR-0119): the account it sends as, which is the Datadog source's
//! default connection; and what the orchestrator asks of it, the chain links it lays out and a page's faults.

use datadog::{Login, Site};
use orch::shipping::release::Own;

/// The Datadog source's default connection: its key (`DATADOG_API_KEY`), which the owner fills through the write-only field
/// of an inbox item and no agent sees, and its site (`DATADOG_SITE`, datadoghq.com when empty), read at each send. One
/// connection is the one owner of both, for the agents' reads and for what Remit sends.
pub struct Account(pub source::Set);

impl datadog::Account for Account {
    async fn login(&self) -> Result<Option<Login>, String> {
        let read = self
            .0
            .read(
                "datadog",
                source::DEFAULT_CONNECTION,
                &["DATADOG_API_KEY", "DATADOG_SITE"],
            )
            .await
            .map_err(|e| e.to_string())?;
        let [key, site] = read.as_slice() else {
            return Err("the Datadog connection did not answer both names".to_owned());
        };
        if key.trim().is_empty() {
            return Ok(None);
        }
        let site = Site::parse(site).map_err(|e| format!("DATADOG_SITE: {e}"))?;
        Ok(Some(Login {
            site,
            api_key: key.trim().to_owned(),
        }))
    }
}

/// The exporter as the orchestrator uses it: the chain links of a chain's screen, and where a page's faults go.
pub struct Exporter(pub datadog::Datadog);

impl orch::DatadogExport for Exporter {
    fn chain_links(&self, chain: &str) -> Vec<(&'static str, String)> {
        self.0.chain_links(chain)
    }

    fn page_faults(&self, session: &str, faults: &[wire::app::PageFault]) {
        self.0.page_faults(session, faults);
    }
}

/// The deployments of the exporter, for the releases the server publishes (ADR-0119).
pub struct Deployments(pub datadog::Datadog);

impl orch::shipping::release_run::Deployments for Deployments {
    fn published(&self, rel: &model::Release, own: &Own, now: model::Time) {
        self.0.deployed(deployment_of(rel, own, now));
    }
}

/// What Datadog is told of a release published: its service is its repository's project name (the last part of its
/// path), the repository and commit are what Datadog's suspect commits read, and its tags name the release, then the
/// chains it carries and then their landings, as shipping chose them (`own`), so the owner finds in Datadog the deployment
/// a chain's change went out in; what a cap on the tags cuts is the landings first. The agents' tools do not search
/// deployments by chain (`datadog_events` has no `chain`). Pure.
fn deployment_of(rel: &model::Release, own: &Own, now: model::Time) -> datadog::Deployment {
    use telemetry::attr;
    let tags = std::iter::once(format!("{}:{}", attr::RELEASE_ID, rel.id))
        .chain(own.chains.iter().map(|c| format!("{}:{c}", attr::CHAIN_ID)))
        .chain(own.landings.iter().map(|l| format!("{}:{}", attr::LANDING_ID, l.id)))
        .collect();
    let nanos = |t: model::Time| t.0.unix_timestamp_nanos();
    datadog::Deployment {
        id: rel.id.clone(),
        service: rel.project.rsplit('/').next().unwrap_or_default().to_lowercase(),
        version: rel.version.clone(),
        repository_url: format!("https://{}/{}", rel.host, rel.project),
        commit: rel.commit.clone(),
        started_at: nanos(rel.steps.iter().map(|s| s.started_at).min().unwrap_or(rel.created_at)),
        finished_at: nanos(rel.steps.iter().filter_map(|s| s.ended_at).max().unwrap_or(now)),
        tags,
    }
}

#[cfg(test)]
mod tests {
    use model::{Landing, Release, ReleaseStep, Revert, Time};
    use orch::shipping::release::own_of;

    use super::*;

    #[test]
    fn a_deployment_names_the_release_and_each_chain_it_carries_once() {
        let rel = Release {
            id: "rl_1".into(),
            host: "github.com".into(),
            project: "Acme/Billing".into(),
            commit: "abc".into(),
            version: "1.4.0".into(),
            steps: vec![ReleaseStep::default()],
            ..Release::default()
        };
        let landing = |id: &str, chain: &str, reverts: Option<Revert>| Landing {
            id: id.into(),
            chain_id: chain.into(),
            release: Some("rl_1".into()),
            reverts,
            ..Landing::default()
        };
        let carried = [
            landing("ld_1", "ch_a", None),
            landing("ld_2", "ch_a", None),
            landing(
                "ld_3",
                "ch_b",
                Some(Revert {
                    landing: "ld_1".into(),
                    cause: model::RevertCause::Guardrail,
                    by: model::by_remit("release"),
                    why: String::new(),
                }),
            ),
        ];
        let d = deployment_of(&rel, &own_of(&carried, "rl_1"), Time::default());
        assert_eq!(d.service, "billing");
        assert_eq!(d.repository_url, "https://github.com/Acme/Billing");
        assert_eq!(
            d.tags,
            [
                "remit.release_id:rl_1",
                "remit.chain_id:ch_a",
                "remit.landing_id:ld_1",
                "remit.landing_id:ld_2"
            ]
        );
    }
}
