//! The jobs of a run that act on the one chain an event names (ADR-0036): mark its change live, and read the checks its
//! forge's pipeline ran on what it landed (ADR-0114). The chain is found as a tell finds it (`target_chain`), a finished
//! chain included: what it landed is followed after the Chair is done. Marking live does its act before the firing is
//! stored, so a crash between the two does it again; reading the checks is stored first and done in the background, and
//! both are idempotent (the forge's answer is recorded only where it differs from what is kept).

use crate::Pipeline;
use crate::action_params::{PARAM_CHAIN, PARAM_SUMMARY, params_of};
use crate::automation::{JOB_MARK_LIVE, JOB_SYNC_LANDING};
use crate::processing::Delivery;
use crate::tell::{door_by, target_chain};
use chains::chain_core::first_line;
use chains::{Error, Result};

impl Pipeline {
    /// The effect of a run of a job that takes a chain, on the one reading its firing covers.
    pub(crate) async fn fire_chain_job(&self, d: &Delivery) -> Result<()> {
        let sub = &d.sub;
        let job = sub.action.job.as_str();
        let prm = params_of(&sub.action, d.reading()?, d.now);
        let Some(marks) = prm.marks(PARAM_CHAIN).filter(|m| !m.is_empty()) else {
            return self
                .done_saying(d, &format!("its chain expression gave nothing, so {job} did nothing"))
                .await;
        };
        let (chains, landings) = self
            .store
            .read(|r| Ok::<_, store::Error>((r.chains()?, r.landings()?)))
            .await?;
        let chain_id = match target_chain(&chains, &landings, &marks) {
            Ok(c) => c.id.clone(),
            Err(why) => return self.done_saying(d, &format!("{why}, so {job} did nothing")).await,
        };
        match job {
            JOB_MARK_LIVE => {
                let summary = prm.text(PARAM_SUMMARY).unwrap_or_default();
                if let Err(err) = self.mark_live(&chain_id, door_by(), &summary).await {
                    let why = format!("the change of its chain could not be recorded as live: {err}");
                    return self.done_saying(d, &why).await;
                }
                let said = format!(
                    "{}: recorded the change of {chain_id} as live",
                    first_line(&summary, 300)
                );
                self.done_saying(d, &said).await
            }
            JOB_SYNC_LANDING => {
                // The forge is asked in the background: a slow forge would hold the subscription's later firings past
                // their deadline. A crash before it answers loses nothing that stays lost: the next look at the landings
                // records it.
                let said = format!("asking the forge about the checks on what {chain_id} landed, and recording them");
                self.done_saying(d, &said).await?;
                if let Ok(a) = self.actions() {
                    self.background(move |_| async move { a.follow_chain_landings(&chain_id).await });
                }
                Ok(())
            }
            other => Err(Error::refused(format!(
                "subscription {} runs no job {other:?} on a chain",
                sub.id
            ))),
        }
    }
}
