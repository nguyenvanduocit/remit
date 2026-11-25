//! What the orchestrator asks of the server's Datadog exporter (ADR-0119): where a chain's screen links to, and what a
//! page reported failing, sent on as the page's own, naming only a chain its session may name.

use wire::app::PageFault;

use crate::Orchestrator;

/// The server's Datadog exporter, as the orchestrator uses it.
pub trait DatadogExport: Send + Sync {
    /// Where the owner reads what chain did, each under its label: only while what Remit sends reaches Datadog, none
    /// otherwise.
    fn chain_links(&self, chain: &str) -> Vec<(&'static str, String)>;

    /// Sends faults, what a page found failing, as that page's errors; `session` is its core's name, empty when it is not
    /// one.
    fn page_faults(&self, session: &str, faults: &[PageFault]);
}

impl Orchestrator {
    /// Whether the owner's page may name chain on its faults: a chain there is, read from the store (no chain is loaded).
    pub async fn owner_names(&self, chain: &str) -> bool {
        let id = chain.to_owned();
        self.store.read(move |r| r.chain(&id)).await.is_ok()
    }

    /// Whether the page of worker key key_id may name chain on its faults: one of its mentor's helps is on that chain,
    /// as the store holds them; never what the page said it shows.
    pub async fn mentor_names(&self, key_id: &str, chain: &str) -> bool {
        self.mentor_helps(key_id)
            .await
            .is_ok_and(|helps| helps.iter().any(|h| h.chain == chain))
    }

    /// Sends what failed in a page to Datadog: the faults of a link report kept for the first time, its core named
    /// `session` (empty: not one).
    pub fn page_faults(&self, session: &str, faults: &[PageFault]) {
        if let Some(dd) = &self.datadog
            && !faults.is_empty()
        {
            dd.page_faults(session, faults);
        }
    }
}
