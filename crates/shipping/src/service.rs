//! The shipping crate's service (ADR-0096): the core it runs on, and what only this crate holds. It reads as
//! the core does (`Deref`), so its code says `self.store` as the core's does.

use std::ops::Deref;
use std::sync::{Arc, Weak};

use chains::Core;
use chains::request::Service;

/// The services below shipping, as the application made them.
pub struct Below {
    pub reach: Arc<reach::Reach>,
    pub pipeline: Arc<pipeline::Pipeline>,
    pub capabilities: Arc<capability::Capabilities>,
    pub crew: Arc<crew::Crew>,
}

/// What shipping runs outside, as the application made it: the checks of a land, the install of a release on this
/// server, a release's steps, and what this server knows of itself.
#[derive(Default)]
pub struct Outside {
    pub checks: Option<chains::git::CheckRunner>,
    pub updater: Option<crate::install::Updater>,
    pub steps: Option<Arc<dyn crate::release_run::StepRunner>>,
    pub instance: Option<Arc<dyn crate::remit_facts::Instance>>,
    pub deployments: Option<Arc<dyn crate::release_run::Deployments>>,
}

pub struct Shipping {
    pub core: Arc<Core>,
    /// The service itself, for the work it starts on tasks of their own.
    me: Weak<Self>,
    /// The checks a submit runs on a change (`CheckRunner`); none runs `run_checks`.
    pub checks: Option<chains::git::CheckRunner>,
    /// How this server installs a release on itself (-update-restart, install.rs): the last step of a release of the
    /// repository it runs; none: nothing installs one here.
    pub updater: Option<crate::install::Updater>,
    /// How a release's steps run and reach their connections (release_run.rs); none: no release step runs here.
    pub steps: Option<Arc<dyn crate::release_run::StepRunner>>,
    /// What this server knows of itself: its health by `/healthz`'s rule (remit_facts.rs); none records no
    /// `fact:remit.*`.
    pub instance: Option<Arc<dyn crate::remit_facts::Instance>>,
    /// Where a release published is told to the outside (release_run.rs `Deployments`); none tells nobody.
    pub deployments: Option<Arc<dyn crate::release_run::Deployments>>,
    /// The merge queues' runners, in memory: one runs a queue at a time, and holds the size of its next batch
    /// (queue.rs).
    pub queues: crate::queue::QueueRuns,
    /// The repositories' release runners, in memory: one runs a repository's releases at a time, and holds the size
    /// of its next release (release_run.rs).
    pub releases: crate::release_run::ReleaseRuns,
    /// The pid of the last `remit update` install_release started (install.rs), held across its check and its start so
    /// two calls cannot both pass before the update takes its own lock.
    pub installing: tokio::sync::Mutex<Option<u32>>,
    /// What the recording of this server's own facts remembers (remit_facts.rs).
    pub remit: crate::remit_facts::RemitFacts,
    /// The services below shipping it asks: reach for a judge's sources, the pipeline for sensors and a firing's outcome,
    /// capability for what an authoring chain wrote, the crew for objectives.
    pub reach: Arc<reach::Reach>,
    pub pipeline: Arc<pipeline::Pipeline>,
    pub capabilities: Arc<capability::Capabilities>,
    pub crew: Arc<crew::Crew>,
}

impl Shipping {
    /// The service over core, with what it runs outside and the services below it.
    pub fn new(core: Arc<Core>, outside: Outside, below: Below) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            core,
            me: me.clone(),
            checks: outside.checks,
            updater: outside.updater,
            steps: outside.steps,
            instance: outside.instance,
            deployments: outside.deployments,
            queues: crate::queue::QueueRuns::default(),
            releases: crate::release_run::ReleaseRuns::default(),
            installing: tokio::sync::Mutex::new(None),
            remit: crate::remit_facts::RemitFacts::default(),
            reach: below.reach,
            pipeline: below.pipeline,
            capabilities: below.capabilities,
            crew: below.crew,
        })
    }

    /// The service as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }
}

impl Deref for Shipping {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl Service for Shipping {
    fn core(&self) -> &Core {
        &self.core
    }
}
