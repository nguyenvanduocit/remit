use std::sync::Arc;

use model::{Event, EventStatus, SystemNote};
use wire::app::CardProcesses;

use crate::chain_core::Core;
use crate::outbox::actor_words;
use crate::request::{Done, Request, Service};
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
        let mut processes = match session {
            Some(session) => tokio::time::timeout(CANCEL_WAIT, session.processes())
                .await
                .map_err(|_| Error::refused("the runtime did not answer"))?
                .map_err(|e| Error::refused(e.to_string()))?,
            None => Vec::new(),
        };
        if !run.open() || !root.open() || !self.current(&run) {
            return Ok(CardProcesses::default());
        }
        if run.root.is_some() {
            for process in &mut processes {
                process.kill_token = None;
            }
        }
        Ok(CardProcesses {
            shared: run.root.is_some(),
            processes,
        })
    }

    /// Force kills a process in the card's own runtime session, and records who acted and why.
    pub async fn kill_card_process(
        self: &Arc<Self>,
        req: Request,
        card_id: String,
        kill_token: String,
        why: String,
    ) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "KillCardProcess", move |o| async move {
            let run = o
                .run_of(&card_id)
                .filter(|r| r.open())
                .ok_or_else(|| Error::refused("the agent has no live session"))?;
            if run.root.is_some() {
                return Err(Error::refused("force kill processes from the parent session"));
            }
            let session = run
                .state()
                .sess
                .clone()
                .filter(|_| run.open())
                .ok_or_else(|| Error::refused("the agent has no live session"))?;
            let target = tokio::time::timeout(CANCEL_WAIT, session.processes())
                .await
                .map_err(|_| Error::refused("the runtime did not answer"))?
                .map_err(|e| Error::refused(e.to_string()))?
                .into_iter()
                .find(|p| p.kill_token.as_deref() == Some(kill_token.as_str()))
                .ok_or_else(|| Error::refused("the process selection has expired"))?;
            // A signal may take effect even if the runtime's reply is lost.
            let who = actor_words(&by);
            let why = why.trim();
            let why = if why.is_empty() {
                "Immediate termination requested."
            } else {
                why
            };
            o.add_event(Event {
                home: run.home.clone(),
                card_id,
                status: EventStatus::System(SystemNote::Plain),
                by: Some(by),
                text: format!(
                    "{who} requested force kill for {} (PID {}). Reason: {why}",
                    target.command, target.pid
                ),
                ..Event::default()
            })
            .await?;
            tokio::time::timeout(CANCEL_WAIT, session.kill_process(kill_token))
                .await
                .map_err(|_| Error::refused("the runtime did not confirm the signal"))?
                .map_err(|e| Error::refused(e.to_string()))
        })
        .await
    }
}
