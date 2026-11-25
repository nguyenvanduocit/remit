//! What a card's agent did, as its Claude Code sessions exported it (ADR-0085), kept by the OTLP door and read here for
//! the owner; and what a page's core counted on the link (ADR-0095 decision 12), kept here.

use telemetry::{Record, summarize};
use wire::app::CardTelemetry;

use crate::Result;
use crate::chain_core::Core;

impl Core {
    /// Card `card_id`'s telemetry, summed up; empty when this server keeps none.
    pub async fn card_telemetry(&self, card_id: &str) -> Result<CardTelemetry> {
        let Some(t) = &self.cfg.telemetry else {
            return Ok(CardTelemetry::default());
        };
        Ok(summarize(&t.card_events(card_id.to_owned()).await?))
    }

    /// Keeps one link report's records, once by the trace and span they name, and says whether this kept them: false for
    /// a report kept before, and on a server that keeps no telemetry, which cannot tell a report sent again.
    pub async fn record_link(&self, records: Vec<Record>) -> Result<bool> {
        match &self.cfg.telemetry {
            Some(t) => Ok(t.write_once(records).await?),
            None => Ok(false),
        }
    }
}
