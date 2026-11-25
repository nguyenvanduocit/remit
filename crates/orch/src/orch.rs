//! The orchestrator is the application (ADR-0096): the chain core and one service per domain crate, each over the same
//! core, put together once. It implements what the core and the pipeline hand up (`Domains`, `Actions`) by calling the
//! crate that does each, and it keeps what only the whole application can lay out: the views the channels read.

use std::ops::Deref;
use std::sync::{Arc, Weak};

use capability::Capabilities;
use chains::Core;
use chains::request::Service;
use crew::Crew;
use keys::Keys;
use pipeline::Pipeline;
use reach::Reach;
use shipping::Shipping;
use store::Store;

use crate::notify::Notifier;
use chains::ports::{Bundled, PluginHost, Sources};

pub use chains::Clock;

/// How the application is configured: the core's settings and what only a domain crate uses.
#[derive(Clone, Default)]
pub struct Config {
    pub core: chains::Config,
    /// The models memory is searched by meaning and judged with (crew); none: by words and links.
    pub memory_models: Option<Arc<dyn crew::memory::MemoryModels>>,
    /// Where the customers' bugs are read (the pipeline's list_issues); none: no Jira.
    pub jira: Option<Arc<pipeline::jira::Jira>>,
    /// The checks a submit runs (shipping); none runs `run_checks`.
    pub checks: Option<chains::git::CheckRunner>,
    /// How this server installs a release on itself (shipping's install.rs), the last step of a release of the
    /// repository it runs; none: nothing installs one here.
    pub updater: Option<shipping::install::Updater>,
    /// How a release's steps run (ADR-0116); none: no release step runs here.
    pub release_steps: crate::release_steps::ReleaseSteps,
    /// What this server knows of itself (shipping's `fact:remit.*`); none records none.
    pub instance: Option<Arc<dyn shipping::remit_facts::Instance>>,
    /// The Datadog exporter (ADR-0119): where a chain's screen links to, and where a page's faults go; none: nowhere.
    pub datadog: Option<Arc<dyn crate::DatadogExport>>,
    /// Where a release published is told to Datadog as a deployment (ADR-0119); none tells nobody.
    pub deployments: Option<Arc<dyn shipping::release_run::Deployments>>,
}

/// What the application is made of besides its configuration: one value per dependency, each a port its owner fills.
#[derive(Clone)]
pub struct Deps {
    pub store: Arc<Store>,
    pub notify: Arc<dyn Notifier>,
    pub launcher: Arc<dyn agent::Launcher>,
    pub sources: Arc<dyn Sources>,
    pub plugins: Arc<dyn PluginHost>,
    pub bundled: Arc<dyn Bundled>,
    pub clock: Clock,
}

pub struct Orchestrator {
    pub core: Arc<Core>,
    pub reach: Arc<Reach>,
    pub pipeline: Arc<Pipeline>,
    pub capabilities: Arc<Capabilities>,
    pub keys: Arc<Keys>,
    pub crew: Arc<Crew>,
    pub shipping: Arc<Shipping>,
    /// The channels, told of every kind of change, the views' included (`Watched`).
    pub notify: Arc<dyn Notifier>,
    /// The Datadog exporter (ADR-0119): where a chain's screen links to, and where a page's faults go; none: nowhere.
    pub(crate) datadog: Option<Arc<dyn crate::DatadogExport>>,
    /// The orchestrator itself, for the work it starts on tasks of their own.
    me: Weak<Self>,
    /// What the channels read, laid out again as what it is made of moves (views.rs).
    pub(crate) views: crate::views::Views,
}

impl Orchestrator {
    /// The application over its configuration and its ports: the core, every domain's service over it, and the views,
    /// which start computing at once.
    pub fn new(cfg: Config, d: Deps) -> Arc<Self> {
        let views = crate::views::Views::new((d.clock)());
        let marks = Arc::clone(&views.marks);
        let o = Arc::new_cyclic(|me: &Weak<Self>| {
            let watched = Arc::new(crate::views_watched::Watched { inner: d.notify, marks });
            let domains: Weak<dyn chains::Domains> = me.clone();
            let actions: Weak<dyn pipeline::actions::Actions> = me.clone();
            let core_told: Arc<dyn chains::Notifier> = watched.clone();
            let pipeline_told: Arc<dyn pipeline::notify::Notifier> = watched.clone();
            let capability_told: Arc<dyn capability::notify::Notifier> = watched.clone();
            let crew_told: Arc<dyn crew::notify::Notifier> = watched.clone();
            let core = Core::new(
                cfg.core,
                chains::chain_core::Deps {
                    store: d.store,
                    notify: core_told,
                    launcher: d.launcher,
                    sources: d.sources,
                    plugins: d.plugins,
                    bundled: d.bundled,
                    clock: d.clock,
                },
                domains,
            );
            let reach = Reach::new(Arc::clone(&core));
            let pipeline = Pipeline::new(Arc::clone(&core), pipeline_told, cfg.jira, actions);
            let capabilities = Capabilities::new(Arc::clone(&core), capability_told);
            let keys = Keys::new(Arc::clone(&core));
            let crew = Crew::new(
                Arc::clone(&core),
                crew_told,
                cfg.memory_models,
                (Arc::clone(&pipeline), Arc::clone(&capabilities), Arc::clone(&keys)),
            );
            let steps: Option<Arc<dyn shipping::release_run::StepRunner>> = match cfg.release_steps {
                crate::release_steps::ReleaseSteps::None => None,
                crate::release_steps::ReleaseSteps::Jailed(runner) => {
                    Some(Arc::new(crate::release_steps::JailedSteps {
                        reach: Arc::clone(&reach),
                        runner,
                    }))
                }
                crate::release_steps::ReleaseSteps::Given(steps) => Some(steps),
            };
            let shipping = Shipping::new(
                Arc::clone(&core),
                shipping::Outside {
                    checks: cfg.checks,
                    updater: cfg.updater,
                    steps,
                    instance: cfg.instance,
                    deployments: cfg.deployments,
                },
                shipping::Below {
                    reach: Arc::clone(&reach),
                    pipeline: Arc::clone(&pipeline),
                    capabilities: Arc::clone(&capabilities),
                    crew: Arc::clone(&crew),
                },
            );
            Self {
                core,
                reach,
                pipeline,
                capabilities,
                keys,
                crew,
                shipping,
                notify: watched,
                datadog: cfg.datadog,
                me: me.clone(),
                views,
            }
        });
        o.start_views();
        o
    }

    /// The orchestrator as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }
}

impl Deref for Orchestrator {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl Service for Orchestrator {
    fn core(&self) -> &Core {
        &self.core
    }
}
