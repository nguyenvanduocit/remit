//! How Remit follows what it landed (ADR-0114, ADR-0057's pattern): a sensor, a signal and a subscription. The sense is a
//! plugin's: the sensor kind landing of the plugin forge asks the forge, through the service Remit lends it for the call
//! (forge_service.rs in reach), for the checks its pipeline ran on each push a chain's landing rode and on the base
//! before it, and writes them as a reading of chain:<id>.sensor.landing.<landing> whenever they differ from what it wrote
//! last. A subscription that ships with Remit runs record-landing-checks on each: the one place that records the checks
//! on a landing and reverts a push that broke a green base (landing.rs `verdict_on`). The periodic look
//! (`sync_landings`) keeps each chain's sensor in step, runs every queue with a landing ready, reverts a stage whose
//! guardrail breached while it bakes, and runs the release of every repository with something to release (ADR-0116).

use std::collections::BTreeSet;

use bundled::{FORGE_PLUGIN, LANDING_KIND, LandingSettings, LandingWatch};
use model::{
    Chain, Checks, ForeignJson, LANDING_SENSOR, Landing, LandingState, RevertCause, Sensor, Time, by_remit,
    chain_sensor_key, landing_sensor_id, parse_landing_sensor_key,
};
use serde::Deserialize;

use crate::Shipping;
use crate::folder::landed_in_place;
use crate::land::{approve, approved_text};
use crate::landing::{Verdict, requeued, revertible, verdict_on};
use crate::queue::{QUEUE, queue_cause};
use chains::apply::Records;
use chains::chain_core::new_id;
use chains::ingest::SENSOR_AGENT;
use chains::live::LandingWrite;
use chains::{Error, Result};
use pipeline::processing::Delivery;

/// How long after it landed a landing's pipeline is followed: one that has said nothing by then runs none.
const FOLLOW_FOR: time::Duration = time::Duration::DAY;

/// What a reading of the landing sense says: the checks on the push and on the base before it, none while the forge has
/// said nothing.
#[derive(Debug, Clone, Default, Deserialize)]
struct ChecksValue {
    #[serde(default)]
    push: Option<Checks>,
    #[serde(default)]
    onto: Option<Checks>,
}

/// Whether l's pipeline is still followed at now: it is on its base, landed in the last day, and its push has not
/// settled.
fn followed(l: &Landing, now: Time) -> bool {
    l.on_base()
        && l.reverts.is_none()
        && l.push.is_some()
        && l.landed_at.is_some_and(|at| now.0 < at.0.saturating_add(FOLLOW_FOR))
        && verdict_on(l.checks.as_ref(), l.base_checks.as_ref()) == Verdict::Wait
}

/// The landing sensor ch's followed landings call for; none when none is followed. It has no period: it is read when
/// something asks (`follow_chain_landings`).
fn landing_sensor_of(ch: &Chain, landings: &[Landing], now: Time) -> Option<Sensor> {
    let mut watches: Vec<LandingWatch> = landings
        .iter()
        .filter(|l| followed(l, now))
        .map(|l| LandingWatch {
            landing: l.id.clone(),
            host: l.host().to_owned(),
            project: l.project().to_owned(),
            push: l.push.clone().unwrap_or_default(),
            onto: l.onto.clone().unwrap_or_default(),
        })
        .collect();
    if watches.is_empty() {
        return None;
    }
    watches.sort_by(|a, b| a.landing.cmp(&b.landing));
    let settings = serde_json::to_string(&LandingSettings { landings: watches }).ok()?;
    Some(Sensor {
        id: landing_sensor_id(&ch.id),
        kind: model::plugin_name(FORGE_PLUGIN, LANDING_KIND),
        name: LANDING_SENSOR.to_owned(),
        chain: ch.id.clone(),
        signal: chain_sensor_key(&ch.id, LANDING_SENSOR),
        settings: ForeignJson::parse(&settings).ok(),
        by: by_remit(SENSOR_AGENT),
        created_at: now,
        updated_at: now,
        ..Sensor::default()
    })
}

impl Shipping {
    /// Puts in its queue each landing whose gates approved it already, moves each round of attempts on (round.rs), runs every queue with a landing ready or checking or one gone back that is looked for on its base,
    /// keeps each chain's landing sensor in step and has it read, reverts each stage whose guardrail breached while it
    /// bakes, and runs the releases (release_run.rs).
    pub async fn sync_landings(&self) {
        let Ok(judged) = self.store.read(|r| r.open_landings()).await else {
            return;
        };
        self.approve_judged(&judged).await;
        self.judge_rounds().await;
        let Ok(open) = self.store.read(|r| r.open_landings()).await else {
            return;
        };
        let queues: BTreeSet<model::QueueKey> = open
            .iter()
            .filter(|l| {
                matches!(
                    l.state,
                    LandingState::Ready | LandingState::Combining | LandingState::Checking
                )
            })
            .filter_map(|l| l.queue.clone())
            .collect();
        for q in queues {
            self.kick_queue(q);
        }
        let Ok(chains) = self.store.read(|r| r.chains()).await else {
            return;
        };
        self.kick_looked_for(&chains).await;
        for ch in chains {
            if tokio::time::timeout(chains::forges::FORGE_TIMEOUT, self.follow_landings(&ch))
                .await
                .is_err()
            {
                tracing::warn!(chain = %ch.id, "landing sense: the forge did not answer in time");
            }
        }
        self.revert_breached_stages().await;
        self.sync_releases().await;
    }

    /// Makes ready each landing of open that waits for gates which approved its change already: one migration 0119
    /// carried over with its verdicts, or one judged before its gates changed. A landing moved meanwhile is left as it
    /// is: its write is over the revision read here.
    async fn approve_judged(&self, open: &[Landing]) {
        for l in open.iter().filter(|l| l.state == LandingState::Judging) {
            let id = l.chain_id.clone();
            let Ok((approvals, chain)) = self
                .store
                .read(move |r| Ok::<_, store::Error>((r.approvals(&id)?, r.chain(&id)?)))
                .await
            else {
                continue;
            };
            let Ok(round) = self.round_of(&chain).await else {
                continue;
            };
            let mut ready = l.clone();
            let now = self.now();
            if !approve(&mut ready, &approvals, self.cfg.require_approvals, round.as_ref(), now) {
                continue;
            }
            ready.updated_at = now;
            if let Err(err) = self.commit_in_folder(&chain, &mut ready).await {
                // it stays judging: the next look commits it again
                tracing::warn!(landing = %l.id, %err, "a landing in place was not committed");
                continue;
            }
            let mut rec = Records::default();
            if let Err(err) = landed_in_place(&chain, &ready, &mut rec) {
                tracing::debug!(landing = %l.id, %err, "a landing in place was not recorded as landed");
                continue;
            }
            let tell = format!(
                "Version {} of {} carries every verdict it needs. {}",
                ready.version.number,
                ready.repo,
                approved_text(&ready)
            );
            let (chain, repo, q) = (ready.chain_id.clone(), ready.repo.clone(), ready.queue.clone());
            rec.landings.push(LandingWrite {
                landing: ready,
                cause: queue_cause(),
            });
            match self.write_on_chain(&chain, &repo, tell, rec).await {
                Ok(()) => {
                    if let Some(q) = q {
                        self.kick_queue(q);
                    }
                }
                Err(err) => tracing::debug!(landing = %l.id, %err, "an approved landing was not put in its queue"),
            }
        }
    }

    /// Has chain id's landing sensor read now (`follow_landings`).
    pub async fn follow_chain_landings(&self, id: &str) {
        let key = id.to_owned();
        let Ok(ch) = self.store.read(move |r| r.chain(&key)).await else {
            return;
        };
        if tokio::time::timeout(chains::forges::FORGE_TIMEOUT, self.follow_landings(&ch))
            .await
            .is_err()
        {
            tracing::warn!(chain = id, "landing sense: the forge did not answer in time");
        }
    }

    /// Runs the queue of each landing gone back that is looked for on its base (`queue::looked_for`), so one merged
    /// outside the queue is found landed though nothing else in its queue moves.
    async fn kick_looked_for(&self, chains: &[Chain]) {
        let now = self.now();
        let mut queues = BTreeSet::new();
        for ch in chains {
            let id = ch.id.clone();
            let Ok(landings) = self.store.read(move |r| r.chain_landings(&id)).await else {
                continue;
            };
            queues.extend(
                landings
                    .iter()
                    .filter(|l| crate::queue::looked_for(l, &landings, now))
                    .filter_map(|l| l.queue.clone()),
            );
        }
        for q in queues {
            self.kick_queue(q);
        }
    }

    /// Makes ch's landing sensor the one its followed landings call for (made, changed or ended), and has it read now.
    async fn follow_landings(&self, ch: &Chain) {
        if let Err(err) = self.follow_landings_now(ch).await {
            tracing::warn!(chain = %ch.id, %err, "landing sense");
        }
    }

    async fn follow_landings_now(&self, ch: &Chain) -> Result<()> {
        let id = ch.id.clone();
        let landings = self.store.read(move |r| r.chain_landings(&id)).await?;
        let want = landing_sensor_of(ch, &landings, self.now());
        let id = ch.id.clone();
        let have = match self.store.read(move |r| r.sensor_named(&id, LANDING_SENSOR)).await {
            Ok(s) => Some(s),
            Err(store::Error::NotFound) => None,
            Err(err) => return Err(err.into()),
        };
        let want = match (want, have) {
            (None, Some(have)) => {
                let id = have.id.clone();
                self.store.update(move |tx| tx.delete_sensor(&id)).await?;
                self.notify.sensor_deleted(&have.id);
                return Ok(());
            }
            (None, None) => return Ok(()),
            (Some(want), Some(have)) if have.kind == want.kind && have.settings == want.settings => have,
            (Some(mut want), Some(have)) => {
                (want.created_at, want.rev, want.cursor) = (have.created_at, have.rev, have.cursor);
                self.pipeline.put_sensor(want).await?
            }
            (Some(want), None) => self.pipeline.put_sensor(want).await?,
        };
        self.pipeline.run_sensor(&want.id).await
    }

    /// The effect of record-landing-checks on a reading of the landing sense: the checks recorded on the landing, and
    /// when the push broke a green base (`verdict_on`) every landing it carried reverted, those that rode it with
    /// others queued again to be bisected. Idempotent: checks as recorded change nothing, and a landing already being
    /// taken back is not reverted twice.
    pub async fn fire_record_landing_checks(&self, d: &Delivery) -> Result<()> {
        let Some(r) = d.readings.first() else {
            return self
                .pipeline
                .done_saying(d, "the firing carries no reading, so nothing was recorded")
                .await;
        };
        let Some((_, landing)) = parse_landing_sensor_key(&r.key) else {
            let why = format!("{} is no landing's checks, so nothing was recorded", r.key);
            return self.pipeline.done_saying(d, &why).await;
        };
        let said: ChecksValue = r
            .value
            .as_ref()
            .and_then(|v| serde_json::from_value(v.0.clone()).ok())
            .unwrap_or_default();
        let id = landing.to_owned();
        let l = match self.store.read(move |r| r.landing(&id)).await {
            Err(store::Error::NotFound) => {
                return self
                    .pipeline
                    .done_saying(d, "the landing is gone, so nothing was recorded")
                    .await;
            }
            l => l?,
        };
        if l.checks == said.push && l.base_checks == said.onto {
            return self.pipeline.done_saying(d, "the checks are as recorded").await;
        }
        let mut next = l.clone();
        (next.checks, next.base_checks, next.updated_at) = (said.push.clone(), said.onto.clone(), d.now);
        let rec = Records {
            landings: vec![LandingWrite {
                landing: next.clone(),
                cause: queue_cause(),
            }],
            ..Records::default()
        };
        self.commit(None, None, rec).await?;
        let Some(q) = next.queue.clone() else {
            return self.pipeline.done_saying(d, "a landing in place has no pipeline").await;
        };
        let queue = self.store.read(move |r| r.queue_landings(&q)).await?;
        if verdict_on(said.push.as_ref(), said.onto.as_ref()) == Verdict::Stands {
            self.kick_release(&next.repo); // what it carried may be released now
        }
        if verdict_on(said.push.as_ref(), said.onto.as_ref()) != Verdict::Revert || !revertible(&next, &queue) {
            let what = format!(
                "{} checks on the push: {}",
                next.id,
                said.push.as_ref().map_or("none yet", |c| c.state.as_str())
            );
            return self.pipeline.done_saying(d, &what).await;
        }
        let failed = said.push.as_ref().map(|c| c.failed.clone()).unwrap_or_default();
        self.revert_push(&next, &queue, &failed).await?;
        self.pipeline
            .done_saying(
                d,
                &format!(
                    "the pipeline failed on {} where the base passed: reverted",
                    next.push.as_deref().unwrap_or_default()
                ),
            )
            .await
    }

    /// Reverts every landing the push l rode carried, of queue (every landing of l's), as the pipeline on it failed
    /// (failed names the check); when it carried more than one, each is queued again as a new ready copy, so the next
    /// batches find which broke it.
    async fn revert_push(&self, l: &Landing, queue: &[Landing], failed: &str) -> Result<()> {
        let rode: Vec<&Landing> = queue
            .iter()
            .filter(|x| x.push.is_some() && x.push == l.push && revertible(x, queue))
            .collect();
        let why = format!(
            "the forge's pipeline failed on the push that carried it ({}) where the base before it passed",
            if failed.is_empty() { "a check" } else { failed }
        );
        let alone = rode.len() <= 1;
        if let (false, Some(q)) = (alone, &l.queue) {
            self.queue_run(q, |r| r.k = crate::landing::halved(rode.len()));
        }
        for x in &rode {
            match self.queue_revert(x, RevertCause::Pipeline, by_remit(QUEUE), &why).await {
                Ok(_) | Err(Error::Refused(_)) => {}
                Err(err) => return Err(err),
            }
            if !alone {
                let copy = requeued(x, &new_id("ln_"), self.now());
                let rec = Records {
                    landings: vec![LandingWrite {
                        cause: queue_cause(),
                        landing: copy,
                    }],
                    ..Records::default()
                };
                self.commit(None, None, rec).await?;
            }
        }
        Ok(())
    }

    /// Reverts each landed stage whose guardrail breaches now while it bakes, as the chain that landed it reads it.
    async fn revert_breached_stages(&self) {
        let Ok(all) = self.store.read(|r| r.landings()).await else {
            return;
        };
        let now = self.now();
        for l in all.iter().filter(|l| revertible(l, &all)) {
            let Some(stage) = crate::stage::stage_of(l).filter(|s| !s.undo && now < s.bake_until) else {
                continue;
            };
            let Ok(chain) = self.chain(&l.chain_id).await else {
                continue;
            };
            if self.breached(&chain, &stage.guardrail, now).await != Ok(true) {
                continue;
            }
            let why = format!(
                "guardrail {} breached ({}) while the stage {:?} baked",
                stage.guardrail.key.as_str(),
                stage.guardrail.breach,
                stage.exposure
            );
            if let Err(err) = self
                .queue_revert(l, RevertCause::Guardrail, by_remit(QUEUE), &why)
                .await
            {
                tracing::warn!(landing = %l.id, %err, "stage: a breached stage was not reverted");
            }
        }
    }
}
