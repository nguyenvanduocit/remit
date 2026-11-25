//! The Tick (schedule.rs in chains holds what it calls): what wakes, fires, starts and stands still, on a timer and on a
//! kick. It runs every crate's part of a Tick, so it is the orchestrator's (ADR-0096).

use tokio_util::sync::CancellationToken;

use chains::Error;
use chains::admit::horizons_of;
use chains::schedule::ready_to_run;

use crate::orch::Orchestrator;
use chains::wait::wakes_by;

impl Orchestrator {
    /// Starts what can start, asks the providers that failed whether they answer again, judges the signals and delivers
    /// what fired, cuts short the turns of agents that have fallen silent (`cut_silent_turns`) and reminds the Chairs of
    /// chains where nothing moves (`unstick`). While no provider can run agents it creates and starts nothing (admission
    /// holds it) and reminds no chain. The scheduler (`run`) calls it on a timer and when a change kicks it.
    pub async fn tick(&self) {
        let _one = self.tick_mu.lock().await;
        let now = self.now();
        self.probe_due(now);
        self.notify.mentors_moved(); // "active 5m ago" ages with the clock, not only with a heartbeat
        self.wake_waiting(now).await;
        self.take_effect_due(now).await; // what the Assistant decided whose overturn window passed
        let no_agents = self.no_agents();
        self.pipeline.tick_signals(now).await; // what a firing queues starts on the Tick its delivery kicks
        self.pipeline.poll_sensors(now).await;
        match self.store.read(|r| r.chains()).await {
            Ok(chains) => self.measure_capacity(&chains).await, // once a Tick, before its starts weigh it
            Err(err) => tracing::error!(%err, "capacity not measured"),
        }
        self.start_ready().await;
        // the chains as this Tick's starts left them
        match self
            .store
            .read(|r| Ok::<_, store::Error>((r.chains()?, r.chain_subscriptions()?)))
            .await
        {
            Ok((chains, waiting)) => {
                self.cut_silent_turns(&chains, now).await;
                if no_agents.is_empty() {
                    // a chain that stands still because no agent can run is not the chain's to answer for
                    self.unstick(&chains, now).await;
                }
                let held = self.pipeline.lay_out_board(&chains, now).await;
                self.watch_idle(&chains, &held, now).await;
                self.count_workload(&chains, &wakes_by(waiting), now);
            }
            Err(err) => tracing::error!(%err, "tick"),
        }
        self.count_unreadable();
        self.shipping.record_remit_facts(now); // after the Tick got through, which its health reads
    }
    /// Starts the chains ready to run (`ready_to_run`) that admission lets in, in the queue's order: the Tick's part that
    /// starts chains, which a new chain runs at once rather than waiting on a whole Tick (its sensors, its
    /// signals). Each start holds `slot_mu`, so it runs beside a Tick: a chain already started is not started again.
    pub(crate) async fn start_ready(&self) {
        match self
            .store
            .read(|r| Ok::<_, store::Error>((r.chains()?, r.objectives()?)))
            .await
        {
            Ok((chains, objectives)) => {
                self.run_ready(&ready_to_run(&chains, &horizons_of(&objectives))).await;
            }
            Err(err) => tracing::error!(%err, "chains ready to run not read"),
        }
    }

    /// Offers admission what `ready_to_run` found, in its order. One it holds back waits for the next Tick.
    async fn run_ready(&self, ids: &[String]) {
        for id in ids {
            let started = {
                let _slot = self.slot_mu.lock().await;
                self.start_chain(id).await
            };
            match started {
                Ok(()) | Err(Error::NotAdmitted(_)) => {}
                Err(err) => tracing::error!(chain = %id, %err, "start chain"),
            }
        }
    }
    /// The scheduler: it resumes what a restart cut off, then ticks when a change kicks it and at least every interval,
    /// until stop is cancelled. A Tick that has begun runs to its end: stop says when to stop ticking, not to leave a
    /// chain half started.
    pub async fn run(&self, interval: std::time::Duration, stop: &CancellationToken) {
        self.resume_interrupted().await;
        let shipping = std::sync::Arc::clone(&self.shipping);
        self.background(move |_| async move {
            // migration 0119's carried draft merge requests, closed once on their forge (ADR-0114)
            match shipping.close_carried_mrs().await {
                Ok(0) => {}
                Ok(n) => tracing::info!(closed = n, "carried draft MRs closed"),
                Err(err) => tracing::warn!(%err, "carried draft MRs: the next start tries again"),
            }
        });
        loop {
            self.tick().await;
            let sleep = self.pipeline.sleep_for(self.now(), interval).await;
            tokio::select! {
                () = stop.cancelled() => return,
                () = tokio::time::sleep(sleep) => {}
                () = self.kicks.notified() => {}
            }
        }
    }
}
