//! A Datadog site (`datadoghq.com`, `datadoghq.eu`, `us5.datadoghq.com`, ...): where its OTLP intake takes what Remit
//! sends, and where the owner reads it. Its name is the Datadog source's `DATADOG_SITE`, read by the source's rule
//! (`source::datadog_site`). Pure.

use std::fmt;

/// What the intake takes, each on its own path and under its own size (docs.datadoghq.com/opentelemetry/setup/
/// intake_endpoint, read 2026-11-19: 5.1 MiB uncompressed per logs or metrics request, 15 MiB per traces request).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signal {
    Logs,
    Metrics,
    Traces,
}

impl Signal {
    pub const ALL: [Signal; 3] = [Signal::Logs, Signal::Metrics, Signal::Traces];

    pub(crate) fn path(self) -> &'static str {
        match self {
            Signal::Logs => "/v1/logs",
            Signal::Metrics => "/v1/metrics",
            Signal::Traces => "/v1/traces",
        }
    }

    /// The most bytes one request carries: the intake's limit, which the batching measures an upper bound against.
    pub(crate) fn request_limit(self) -> usize {
        match self {
            // 5.1 MiB
            Signal::Logs | Signal::Metrics => 5_347_737,
            Signal::Traces => 15 << 20,
        }
    }
}

/// A Datadog site, its name checked to be host labels and nothing that moves the address (a path, a port, a login).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site(String);

/// Why a name is not a site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotASite(pub String);

impl fmt::Display for NotASite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} is not a Datadog site such as datadoghq.com, datadoghq.eu or us5.datadoghq.com",
            self.0
        )
    }
}

impl std::error::Error for NotASite {}

impl Site {
    /// The site a connection's DATADOG_SITE names: datadoghq.com for none.
    pub fn parse(named: &str) -> Result<Site, NotASite> {
        source::datadog_site(named)
            .map(Site)
            .ok_or_else(|| NotASite(named.trim().to_owned()))
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    /// Where the site's OTLP intake is: `https://otlp.<site>`, each signal on its path below it, on every site (US1, US3,
    /// US5, EU, AP1, AP2, UK1 and the government sites alike, as the docs' per-site values say).
    pub(crate) fn intake(&self) -> String {
        format!("https://otlp.{}", self.0)
    }

    /// Where the site's API is, the host the Datadog source reads with.
    pub(crate) fn api(&self) -> String {
        format!("https://api.{}", self.0)
    }

    /// The host the owner reads the site on: `app.` before the sites that are a bare domain (datadoghq.com,
    /// datadoghq.eu, ddog-gov.com), the site itself for the others (us5.datadoghq.com).
    fn app(&self) -> String {
        if self.0.split('.').count() == 2 {
            format!("https://app.{}", self.0)
        } else {
            format!("https://{}", self.0)
        }
    }

    /// Where the owner reads what chain did: its traces, its logs and its errors, each filtered to it.
    pub fn chain_links(&self, chain: &str) -> Vec<(&'static str, String)> {
        let query = urlescape::query_value(&format!("@{}:{chain}", telemetry::attr::CHAIN_ID));
        let app = self.app();
        vec![
            ("traces", format!("{app}/apm/traces?query={query}")),
            ("logs", format!("{app}/logs?query={query}")),
            ("errors", format!("{app}/error-tracking?query={query}")),
        ]
    }
}
