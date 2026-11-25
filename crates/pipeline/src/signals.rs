//! The shell of the signal backbone (docs/adr/0014, docs/adr/0064). crates/signal is the pure core: given one
//! subscription, the readings it has not judged and the time, it says what the subscription does, as effects. This module
//! is what surrounds it: it reads the store into that input for each subscription the scheduler's Tick looks at, under
//! `tick_mu`, and hands the effects to the record stage (record.rs), which stores each firing pending with the
//! subscription's progress in one transaction; the deliver stage (deliver.rs) then does each, outside `tick_mu`, and
//! stores its effect with the firing done, so a retry or a restart acts once. A reading enters by ingest (ingest.rs),
//! whose caller kicks the scheduler.
//!
//! To add an action kind, give it a processor (processing.rs) that says what it takes and stores its effect with the
//! firing done (as `fire_start` does for a start through `store_done`, and `fire_wake` through the change's records), and
//! name what it takes from the room. To add a source, normalise what it receives into a Write and ingest it (event.rs does
//! it for an event signal's events); nothing after that is the source's.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use model::{Actor, Chain, Landing, Overlap, OwnerKind, Subscription, Time};
use signal::{ChainFacts, Effect, Input, Screen};
use store::Tx;

use crate::Pipeline;
use crate::automation::JobsAtWork;
use crate::deliver::Deliveries;
use crate::plugin_pipeline::PluginPipeline;
use crate::polls::Polls;
use crate::processing::Delivery;
use crate::routine::{routine_by, start_spec_of, unfinished_run};
use crate::start_event::{start_evidence, start_quota};
use chains::admit::{Needs, fits};
use chains::fact::put_queued_fact;
use chains::ingest::{Door, ShippedSignals};
use chains::lifecycle::{NewChain, with_task_objective};
use chains::screen::Asks;
use chains::{Error, Result};
use wire::app::SignalBoard;

/// Bounds the readings an evaluation of one subscription loads after its place in the log. A pass that does not reach
/// the end leaves the rest unjudged, and the next pass starts where this one stopped.
const UNREAD_PER_PASS: i64 = 1000;

/// What the signal pipeline holds in memory: what the bundled plugins ship subscriptions for, the delivery workers and the shipped jobs at
/// work.
#[derive(Default)]
pub struct SignalState {
    bundled_subs: Mutex<Vec<(String, plugin_sdk::Subscription)>>,
    pub deliveries: Deliveries,
    pub jobs: JobsAtWork,
    /// The board as the last Tick laid it out (`keep_board`).
    pub board: Mutex<SignalBoard>,
    /// What the poller remembers of each sensor it reads (sensor.rs).
    pub sensors: Polls,
    /// What the enabled plugins add to the pipeline (plugin_pipeline.rs).
    pub pipeline: Mutex<Arc<PluginPipeline>>,
}

/// What firing a start takes from the room: a provider that can run agents, with headroom (ADR-0092). The chain it queues
/// takes its slot, and its place in the owner's review, when the scheduler starts it, as every queued chain does.
fn start_needs() -> Needs {
    Needs {
        agents: true,
        headroom: true,
        ..Needs::default()
    }
}

/// What only a chain's own work carries: the branch Remit made for it, and the commits its landings (landings holds
/// every landing, or the chain's) pushed and landed as (ADR-0114), so a forge's event on a commit finds the chain.
pub(crate) fn marks_of(c: &Chain, landings: &[Landing]) -> Vec<String> {
    let mut marks: Vec<String> = c
        .repos
        .iter()
        .filter(|rw| !rw.branch.is_empty())
        .map(|rw| rw.branch.clone())
        .collect();
    for l in landings.iter().filter(|l| l.chain_id == c.id) {
        marks.extend([&l.push, &l.commit].into_iter().flatten().cloned());
    }
    marks
}

/// What an evaluation of one subscription reads, as the store holds it.
struct Read {
    sub: Subscription,
    readings: Vec<model::Reading>,
    chain: Option<ChainFacts>,
    pending: usize,
    running: bool,
}

impl Read {
    fn input<'a>(&'a self, screens: &'a [Screen], now: Time) -> Input<'a> {
        Input {
            sub: &self.sub,
            readings: &self.readings,
            chain: self.chain.as_ref(),
            pending: self.pending,
            running: self.running,
            screens,
            now,
        }
    }
}

/// What an evaluation of sub needs, read in r: the stored readings its pattern lets through that it has not judged
/// (none for one that keeps a schedule), the facts of the chain that owns it, and, for an overlap that asks, how many of
/// its firings wait to be delivered and whether a chain it started is at work.
fn input_of(r: &store::Reader<'_>, sub: Subscription) -> Result<Read> {
    let readings = if sub.schedule.is_none() {
        r.unread_readings(&sub.r#match, sub.seen_seq, UNREAD_PER_PASS)
            .map_err(|e| Error::refused(format!("readings of {}: {e}", sub.r#match)))?
    } else {
        Vec::new()
    };
    let chain = chain_facts_of(r, &sub)?;
    let (mut pending, mut running) = (0, false);
    if sub.overlap == Overlap::BufferOne {
        running = unfinished_run(&r.chains()?, &sub.id).is_some();
        pending = r.pending_firings(&sub.id)?.len();
    }
    Ok(Read {
        sub,
        readings,
        chain,
        pending,
        running,
    })
}

/// What a condition of sub reads of the chain that owns it, as the chain stands now: none for a subscription of no chain,
/// and for one whose chain is gone (it is judged on no facts, and ends with it).
pub(crate) fn chain_facts_of(r: &store::Reader<'_>, sub: &Subscription) -> Result<Option<ChainFacts>> {
    if sub.owner.kind != OwnerKind::Chain {
        return Ok(None);
    }
    match r.chain(&sub.owner.id) {
        Ok(c) => Ok(Some(ChainFacts {
            marks: marks_of(&c, &r.chain_landings(&c.id)?),
            status: *c.status(),
        })),
        Err(store::Error::NotFound) => Ok(None),
        Err(err) => Err(Error::refused(format!("chain {}: {err}", sub.owner.id))),
    }
}

/// The subscriptions an evaluation looks at: those whose time comes by horizon, and those with readings they have not
/// judged.
fn signal_subs(r: &store::Reader<'_>, horizon: Time) -> store::Result<Vec<Subscription>> {
    let mut subs = r.due_subscriptions(horizon)?;
    let due: HashSet<String> = subs.iter().map(|s| s.id.clone()).collect();
    subs.extend(
        r.subscriptions_with_unread()?
            .into_iter()
            .filter(|u| !due.contains(&u.id)),
    );
    Ok(subs)
}

/// What a start queued: the chain, and the run of its script when the start names one.
pub(crate) struct StartQueued {
    pub chain: Chain,
    pub run: Option<model::Workflow>,
}

impl Pipeline {
    /// The subscriptions the bundled plugins ship, by plugin, as last loaded (`load_shipped`).
    pub(crate) fn bundled_subscriptions(&self) -> Vec<(String, plugin_sdk::Subscription)> {
        self.signals
            .bundled_subs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Reads what each bundled plugin ships (its signals, for ingest to weigh what a plugin writes, and its subscriptions,
    /// for the seed): called once at start, before the plugins write and before `seed_subscriptions`.
    pub async fn load_shipped(&self) {
        let mut shipped = Vec::new();
        let mut subs = Vec::new();
        for id in self.bundled.ids() {
            let Some(p) = self.bundled.lookup(&id) else { continue };
            match p.describe().await {
                Ok(m) => {
                    shipped.extend(m.signals.into_iter().map(|s| (id.clone(), s)));
                    subs.extend(m.subscriptions.into_iter().map(|s| (id.clone(), s)));
                }
                Err(err) => tracing::error!(plugin = %id, err = ?err, "describe a bundled plugin"),
            }
        }
        *self.shipped.lock().unwrap_or_else(PoisonError::into_inner) = Arc::new(ShippedSignals(shipped));
        *self.signals.bundled_subs.lock().unwrap_or_else(PoisonError::into_inner) = subs;
    }

    /// The evaluation the Tick makes: the subscriptions whose time has come and those with readings they have not judged
    /// are judged and what they do is recorded; then each subscription with a firing due is given to its delivery worker.
    /// Called with `tick_mu` held; the workers run outside it.
    pub async fn tick_signals(&self, now: Time) {
        match self.store.read(move |r| signal_subs(r, now)).await {
            Ok(subs) => self.judge(subs, now).await,
            Err(err) => tracing::error!(%err, "signals"),
        }
        self.deliver_due(now).await;
    }

    /// How long the scheduler waits after a Tick at now before the next: the interval, or less when a subscription acts
    /// sooner with no new reading (a schedule's instant, a deadline, a flow's release), so that a period is not rounded
    /// up to the interval. What is due already and was not done is held (admission), and a kick or the interval wakes the
    /// scheduler for it.
    pub async fn sleep_for(&self, now: Time, interval: std::time::Duration) -> std::time::Duration {
        let Ok(step) = time::Duration::try_from(interval) else {
            return interval;
        };
        let horizon = now.0.checked_add(step).map_or(now, Time);
        let reads = self
            .store
            .read(move |r| {
                let mut reads = Vec::new();
                for sub in signal_subs(r, horizon)? {
                    reads.push(input_of(r, sub)?);
                }
                Ok::<_, Error>(reads)
            })
            .await;
        let reads = match reads {
            Ok(reads) => reads,
            Err(err) => {
                tracing::error!(%err, "signals: read the input");
                return interval;
            }
        };
        let mut wait = interval;
        for read in &reads {
            for e in signal::evaluate(&read.input(&[], now)) {
                if let Effect::Due(Some(at)) = e
                    && at > now
                {
                    wait = wait.min(chains::health::elapsed(now, at));
                }
            }
        }
        wait
    }

    /// Evaluates each of subs at now and records what the core says of it, in the order of their ids. A subscription
    /// whose input cannot be read is left for the next pass: nothing of it was judged. One with a screen (ADR-0118,
    /// point 2) waits to be evaluated until the decider has been asked about the firings of all of them at once, so the
    /// pass waits for the deciders once (`Core::screen_firings`); the others are recorded as they are read.
    async fn judge(&self, mut subs: Vec<Subscription>, now: Time) {
        subs.sort_by(|a, b| a.id.cmp(&b.id));
        let mut screening: Vec<(Read, Vec<model::Reading>)> = Vec::new();
        for sub in subs {
            let id = sub.id.clone();
            let read = match self.store.read(move |r| input_of(r, sub)).await {
                Ok(read) => read,
                Err(err) => {
                    tracing::error!(subscription = %id, %err, "signals: read the input");
                    continue;
                }
            };
            let asks = if read.sub.screen.is_empty() || self.cfg.decider.is_none() {
                Vec::new()
            } else {
                signal::asks(&read.input(&[], now))
            };
            if asks.is_empty() {
                let effects = signal::evaluate(&read.input(&[], now));
                self.record(&read.sub, &effects, now).await;
            } else {
                screening.push((read, asks));
            }
        }
        let asks = screening
            .iter()
            .map(|(read, readings)| Asks {
                sub: read.sub.clone(),
                readings: readings.clone(),
            })
            .collect();
        let mut screens = self.core.screen_firings(asks).await;
        for (read, _) in screening {
            let screens = screens.remove(&read.sub.id).unwrap_or_default();
            let effects = signal::evaluate(&read.input(&screens, now));
            self.record(&read.sub, &effects, now).await;
        }
    }

    /// The effect of a start: it queues a chain from the subscription's StartSpec, in the transaction that stores the
    /// firing done, under the id the firing records. A firing done already did its effect then: it is not done twice.
    pub(crate) async fn fire_start(&self, d: &Delivery) -> Result<()> {
        fits(&start_needs(), &d.room)?;
        if d.firing.cause == model::FiringCause::Expired {
            return self.done_saying(d, "expired: nothing to start a chain from").await;
        }
        let id = chains::chain_core::new_id("ch_"); // the chain is queued below, under this id
        let done = d.done("started a chain", &id, "");
        let evidence = start_evidence(&d.sub, &d.readings, d.now);
        let ch = self
            .new_start_chain(&d.sub, &id, routine_by(), d.now, &evidence)
            .await?;
        let run = self.start_run(&d.sub, &ch, d.now).await?;
        let (sub, quota) = (d.sub.clone(), self.cfg.event_chains_per_day);
        let q = self
            .store_done(done, move |tx, door| queue_start(tx, door, &sub, ch, run, quota))
            .await?;
        self.start_queued(&q);
        Ok(())
    }

    /// The chain a start of sub queues, titled with the time, made ready to store (its directory made): id is the chain's,
    /// when the firing that starts it has recorded it; empty: a new one.
    pub(crate) async fn new_start_chain(
        &self,
        sub: &Subscription,
        id: &str,
        by: Actor,
        now: Time,
        evidence: &str,
    ) -> Result<Chain> {
        let start = start_spec_of(sub);
        let stamp = now
            .0
            .format(time::macros::format_description!("[day]/[month] [hour]:[minute]"))
            .unwrap_or_default();
        self.new_queued_chain(&NewChain {
            id: id.to_owned(),
            title: format!("{} · {stamp}", start.title),
            goal: format!("{}{evidence}", start.goal),
            kind: start.kind,
            origin: start.origin,
            routine_id: sub.id.clone(),
            by,
            ..NewChain::default()
        })
        .await
    }

    /// The run of the script a start of sub begins its chain ch with (ADR-0073), made ready to store: none when the
    /// start names no script. Its first evaluation runs here, outside the transaction that stores it, as the chain's
    /// directory is made before it; the run has no Chair until the chain starts (workflow_run.rs `start_queued_runs`).
    pub(crate) async fn start_run(&self, sub: &Subscription, ch: &Chain, now: Time) -> Result<Option<model::Workflow>> {
        let start = start_spec_of(sub);
        if start.script.is_empty() {
            return Ok(None);
        }
        self.new_script_run(ch, &start.script, start.args, now)
            .await
            .map(Some)
            .map_err(|e| Error::refused(format!("the run of its script: {e}")))
    }

    /// Tells the channels what a start queued, once it is stored, and kicks the scheduler, which starts the chain as
    /// admission lets it.
    pub(crate) fn start_queued(&self, q: &StartQueued) {
        self.queued(&q.chain);
        if let Some(run) = &q.run {
            self.notify.workflow_updated(run);
        }
        self.kick();
    }
}

/// The effect of a start: stores ch, the chain a start of sub queues, in tx, and run, the run of its script the chain
/// begins with (`start_run`, ADR-0073). It is refused while the last chain the subscription started is at work: a
/// subscription starts one chain at a time, and what it heard meanwhile waits. On an event it is refused too past the
/// subscription's quota of chains a day.
pub(crate) fn queue_start(
    tx: &Tx<'_>,
    door: &Door,
    sub: &Subscription,
    ch: Chain,
    run: Option<model::Workflow>,
    quota: usize,
) -> Result<StartQueued> {
    let chains = tx.chains()?;
    if let Some(run) = unfinished_run(&chains, &sub.id) {
        return Err(Error::RoutineAtWork(format!("{}, {}", run.id, run.status())));
    }
    if sub.schedule.is_none()
        && let Some(why) = start_quota(sub, &chains, door.now, quota)
    {
        return Err(Error::NotAdmitted(why));
    }
    let ch = with_task_objective(tx, ch, model::Accountable::default())?;
    let ch = tx.put_chain(ch)?;
    put_queued_fact(tx, door, &ch)?;
    let run = run.map(|r| tx.put_workflow(r)).transpose()?;
    Ok(StartQueued { chain: ch, run })
}

/// What a script's wait() returns (ADR-0073): why it fired (a reading met its condition, its deadline came, or its time),
/// and the readings that fired it, oldest first.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub(crate) struct WaitResult {
    pub cause: model::FiringCause,
    pub readings: Vec<WaitReading>,
}

/// One reading as a script reads it.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub(crate) struct WaitReading {
    pub key: model::SignalKey,
    pub rev: i64,
    pub at: Time,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<model::ForeignJson>,
}

/// The result of a wait that fired by cause on readings. Pure.
pub(crate) fn wait_result_of(cause: &model::FiringCause, readings: &[model::Reading]) -> WaitResult {
    WaitResult {
        cause: *cause,
        readings: readings
            .iter()
            .map(|r| WaitReading {
                key: r.key.clone(),
                rev: r.rev,
                at: r.at,
                subject: r.subject.clone(),
                value: r.value.clone(),
            })
            .collect(),
    }
}

impl Pipeline {
    /// The effect of a workflow run's wait (ADR-0073): the step that waits on the subscription gets what fired it as its
    /// result, and its run advances, with the firing stored done, in one transaction (workflow_run.rs's
    /// `resume_workflow_step`). A firing that came while its chain waits is a scheduled wake and passes admission as one;
    /// refused, it waits and is tried again.
    pub(crate) async fn fire_workflow(&self, d: &crate::processing::Delivery) -> chains::Result<()> {
        let Some(r) = d.sub.action.workflow.as_ref() else {
            return Err(chains::Error::Permanent(
                model::HoldReason::FiringFailed,
                format!("subscription {} names no workflow step to resume", d.sub.id),
            ));
        };
        let result = model::ForeignJson(serde_json::to_value(wait_result_of(&d.firing.cause, &d.readings))?);
        let fired = d.done(
            &format!("resumed step {} of workflow {}", r.seq, r.run),
            &d.sub.owner.id,
            "",
        );
        self.resume_workflow_step(r, result, fired).await
    }
}
