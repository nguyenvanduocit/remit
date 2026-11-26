use wire::app::CardProcesses;

use crate::chain_core::Core;
use crate::session::CANCEL_WAIT;
use crate::{Error, Result};

impl Core {
    /// Reads the live process tree from the runtime that owns this card's session.
    pub async fn card_processes(&self, card_id: &str) -> Result<CardProcesses> {
        let Some(run) = self.run_of(card_id).filter(|r| r.open()) else {
            return Ok(CardProcesses::default());
        };
        let root = run.root.as_ref().unwrap_or(&run);
        let session = root.state().sess.clone().filter(|_| root.open());
        let processes = match session {
            Some(session) => tokio::time::timeout(CANCEL_WAIT, session.processes())
                .await
                .map_err(|_| Error::refused("the runtime did not answer"))?
                .map_err(|e| Error::refused(e.to_string()))?,
            None => Vec::new(),
        };
        if !run.open() || !root.open() || !self.current(&run) {
            return Ok(CardProcesses::default());
        }
        Ok(CardProcesses {
            shared: run.root.is_some(),
            processes,
        })
    }
}
