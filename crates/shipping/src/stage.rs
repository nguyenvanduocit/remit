//! A landing with a plan is a stage of a canary (ADR-0050, ADR-0114): it lands only when the stage before it on its
//! repository has baked and every guardrail reads and holds (crates/rollout's `vet`, asked when the Chair calls land and
//! again when the queue batches it), its Chair watches its guardrail through the bake, and a breach during the bake has
//! it reverted by the queue. The stage before is the newest landed landing with a plan on the same repository, whichever
//! chain landed it.

use model::{Chain, Landing, StagePlan, Time};
use rollout::{Guardrail, Stage};

use crate::Shipping;
use crate::check_rail::CheckDoor;
use chains::wait::rfc3339;
use chains::watch::SubscribeInput;
use chains::{Error, Result};

/// Whether host/project is the repository this instance runs, as `-update-repo` names it the way `gh -R` reads it:
/// owner/name on github.com, or host/owner/name. None is named on an instance not started with it. Pure.
pub fn instance_repository(update_repo: &str, host: &str, project: &str) -> bool {
    let named = update_repo.trim().trim_matches('/');
    if named.is_empty() {
        return false;
    }
    let (h, p) = match named.splitn(3, '/').count() {
        3 => named.split_once('/').unwrap_or_default(),
        _ => ("github.com", named),
    };
    h.eq_ignore_ascii_case(host) && p.eq_ignore_ascii_case(project)
}

/// The stage landing l became, when it is one that landed.
pub(crate) fn stage_of(l: &Landing) -> Option<Stage> {
    let plan = l.version.plan.as_ref()?;
    Some(Stage::landed(plan, &l.chain_id, &l.id, l.landed_at?))
}

impl Shipping {
    /// The stage landed last on repository host/project; none when none has.
    pub(crate) async fn last_stage(&self, host: &str, project: &str) -> Result<Option<Stage>> {
        let (h, p) = (host.to_owned(), project.to_owned());
        let last = self.store.read(move |r| r.last_stage_on(&h, &p)).await?;
        Ok(last.as_ref().and_then(stage_of))
    }

    /// Whether plan, a stage chain lands on host/project, may land now: the chain's check holds every stage but a way
    /// back (ADR-0068), and the rollout rule holds them all. Its guardrail must be one the chain may watch, as its Chair
    /// watches it through the bake.
    pub(crate) async fn vet_stage(&self, chain: &Chain, host: &str, project: &str, plan: &StagePlan) -> Result<()> {
        let now = self.now();
        if !plan.undo {
            let (_, why) = self.check_refusal(chain, CheckDoor::AtStage).await?;
            if !why.is_empty() {
                return Err(Error::Refused(why));
            }
            self.watch_of(chain, &guardrail_watch(plan, now), now)
                .await
                .map_err(|err| Error::refused(format!("the guardrail cannot be watched for the bake: {err}")))?;
        }
        let last = self.last_stage(host, project).await?;
        let answers = self.breached_on(chain, plan, last.as_ref(), now).await;
        rollout::vet(plan, last.as_ref(), now, |g: &Guardrail| {
            answers.iter().find(|(x, _)| x == g).map_or_else(
                || Err(Error::refused("the guardrail was not read")),
                |(_, r)| r.clone().map_err(Error::Refused),
            )
        })
        .map_err(|refusal| Error::refused(refusal.to_string()))
    }

    /// Each guardrail the rule may ask about (the plan's, and the last stage's while that one counts: always for a way
    /// back), with whether its newest reading meets its breach condition now, or why that cannot be said (ADR-0045). The
    /// plan's is read as chain may read it, the last stage's as the chain that landed it may. Read before the rule runs,
    /// as the rule itself reads nothing.
    async fn breached_on(
        &self,
        chain: &Chain,
        plan: &StagePlan,
        last: Option<&Stage>,
        now: Time,
    ) -> Vec<(Guardrail, Result<bool, String>)> {
        let own = Guardrail::of(plan);
        let mut out = Vec::with_capacity(2);
        if !plan.undo && !own.key.as_str().is_empty() {
            let answer = self.breached(chain, &own, now).await;
            out.push((own.clone(), answer));
        }
        if let Some(l) =
            last.filter(|l| !l.undo && (plan.undo || l.guardrail != own) && !l.guardrail.key.as_str().is_empty())
        {
            let answer = match self.chain(&l.by).await {
                Ok(by) => self.breached(&by, &l.guardrail, now).await,
                Err(err) => Err(format!("the chain that landed the stage before, {:?}: {err}", l.by)),
            };
            out.push((l.guardrail.clone(), answer));
        }
        out
    }

    /// Whether g's newest reading, as chain may read it, meets its breach condition now.
    pub(crate) async fn breached(&self, chain: &Chain, g: &Guardrail, now: Time) -> Result<bool, String> {
        self.watchable(chain, &g.key).await.map_err(|err| err.to_string())?;
        let key = g.key.clone();
        let r = match self.store.read(move |r| r.last_reading(&key)).await {
            Err(store::Error::NotFound) => {
                return Err(
                    "the signal has no reading yet: build the signal, or a sensor that reads it, first".to_owned(),
                );
            }
            r => r.map_err(|err| err.to_string())?,
        };
        signal::probe(&g.breach, &r, now).map_err(|err| err.to_string())
    }

    /// Has the Chair of l's chain watch l's guardrail until its bake ends, once l landed as a stage that is no way back.
    pub(crate) async fn watch_stage(&self, l: &Landing) {
        let Some(plan) = l.version.plan.as_ref().filter(|p| !p.undo) else {
            return;
        };
        let Some(at) = l.landed_at else { return };
        let watched = async {
            let chain = self.chain(&l.chain_id).await?;
            let id = chain.chair_id.clone();
            let chair = self.store.read(move |r| r.card(&id)).await?;
            self.subscribe(&chair, &guardrail_watch(plan, at), self.now()).await
        };
        if let Err(err) = watched.await {
            tracing::warn!(landing = %l.id, %err, "stage: the guardrail watch was not set; the queue still reverts a breach");
        }
    }
}

/// The watch on plan's guardrail over the bake of a stage landed at at.
fn guardrail_watch(plan: &StagePlan, at: Time) -> SubscribeInput {
    SubscribeInput {
        key: plan.guardrail.as_str().to_owned(),
        condition: plan.breach.clone(),
        until: rfc3339(rollout::bake_end(plan.bake, at)),
        why: format!(
            "the canary stage {:?} is baking: on a breach Remit reverts it ({})",
            plan.exposure, plan.rollback
        ),
        ..SubscribeInput::default()
    }
}
