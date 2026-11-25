//! The draft merge requests Remit opened before it landed changes itself (ADR-0114): migration 0119 turned each open one
//! into a judging landing and 0123 kept the merge request, to be closed on its forge once, saying where its change
//! went. This runs at start; a merge request closed is forgotten in the same step, so a second run closes nothing twice,
//! and one the forge cannot close now is tried at the next start. Built to be deleted with the last carried merge
//! request.

use forge::Repo;

use crate::Shipping;
use chains::forges::FORGE_TIMEOUT;
use chains::{Error, Result};

/// What the closed merge request says.
const CLOSING: &str = "Remit lands every change itself now (ADR-0114): this change waits in Remit's merge queue as a \
                       landing, judged by its gates and pushed to the base once they approve. This draft is closed.";

impl Shipping {
    /// Closes each merge request migration 0123 kept on its forge, and forgets it: how many were closed.
    pub async fn close_carried_mrs(&self) -> Result<usize> {
        let carried = self.store.read(|r| r.carried_mrs()).await?;
        let mut closed = 0usize;
        for mr in carried {
            let f = match self.forge_on(&mr.host) {
                Ok(f) => f,
                Err(err) => {
                    tracing::warn!(url = %mr.url, %err, "carried MR: no forge to close it on");
                    continue;
                }
            };
            let r = Repo {
                host: mr.host.clone(),
                project: mr.project.clone(),
            };
            match tokio::time::timeout(FORGE_TIMEOUT, f.close_mr(&r, mr.iid, CLOSING)).await {
                Ok(Ok(was_open)) => {
                    closed = closed.saturating_add(usize::from(was_open));
                    let landing = mr.landing.clone();
                    self.store.update(move |tx| tx.forget_carried_mr(&landing)).await?;
                }
                Ok(Err(err)) => {
                    tracing::warn!(url = %mr.url, %err, "carried MR: not closed; the next start tries again")
                }
                Err(_) => {
                    return Err(Error::refused(format!(
                        "{} did not answer the close of {} in time; the next start tries again",
                        f.name(),
                        mr.url
                    )));
                }
            }
        }
        Ok(closed)
    }
}
