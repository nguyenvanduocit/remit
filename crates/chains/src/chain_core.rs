//! The core's fields are its dependencies (ADR-0072 decision 5, ADR-0096): the store and its one writer, the channels'
//! notifier, the ports of the crates Remit drives (ports.rs, forges.rs), the clock, what it holds in memory of each chain
//! (`ChainState`) and session, and the application above it (`Domains`). Nothing is package state: two cores in one
//! process share nothing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use model::{Actor, Decision, DecisionKind, Time};
use store::Store;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::apply::ChainState;
use crate::domains::Domains;
use crate::forges::{Forge, ForgeAccounts};
use crate::notify::Notifier;
use crate::ports::{Bundled, PluginHost, Sources};
use crate::recover::Interruption;
use crate::request::Inflight;
use crate::session::Run;
use crate::{Error, Result};

/// Every clock read goes through it, so a test can set the time; it gives the time in the server's offset.
pub type Clock = Arc<dyn Fn() -> Time + Send + Sync>;

/// What the owner configured that the chain core reads.
#[derive(Clone, Default)]
pub struct Config {
    /// Claude Code permission mode for every card, e.g. "auto".
    pub mode: String,
    /// Every chain gets `<work_root>/<chain id>` as its directory.
    pub work_root: PathBuf,
    /// The machine Remit runs on, whose memory and load press on the capacity (admit.rs); none reads none.
    pub host: Option<Arc<dyn crate::ports::Host>>,
    /// How long an agent in a turn may send nothing before its turn is cut short; zero: never.
    pub turn_idle: std::time::Duration,
    /// How many times the same tool call with the same result, or the same error, among a turn's last 8 calls makes the
    /// repetition rail act (repeats.rs); 0: never.
    pub turn_repeats: usize,
    /// How many turns a running chain's agents may take with nothing produced (no plan step completed, no work accepted,
    /// no verdict) before its Chair is reminded, and as many again between reminders until Remit finishes it; for the
    /// Assistant, turns in a row on agents' replies before its messages to agents carry a reminder, then are refused
    /// (spin.rs); 0: never.
    pub spin_turns: usize,
    /// The forge of each host a repository may be on (forges.rs).
    pub forges: HashMap<String, Arc<dyn Forge>>,
    /// Principles: the file of the owner's working principles every agent reads first; empty for none.
    pub principles: String,
    /// Language: the language the owner reads what agents write in; empty leaves it to them.
    pub language: String,
    /// Autonomous: a Chair decides everything but merging, instead of asking the owner.
    pub autonomous: bool,
    /// GoalCheck: finish_chain waits for an independent check of the result against the goal (goal.rs).
    pub goal_check: bool,
    /// JudgeModel: the model every judge runs on (the goal judge, a duel's judge, a second judge, and a card that signs a
    /// gate): the catalog's strongest, so no judge grades an author stronger than itself (ADR-0102); empty keeps
    /// Claude Code's default.
    pub judge_model: String,
    /// ChairModel: the model of the Chair, which chooses every other agent's; empty keeps Claude Code's default.
    pub chair_model: String,
    /// The effort a new chain's Chair runs at (`provider::Effort`); empty keeps Claude Code's default.
    pub chair_effort: String,
    /// How many chains the events of one subscription may start a day (start_event.rs); 0: no limit.
    pub event_chains_per_day: usize,
    /// How often the automation snapshots the database (snapshot.rs); zero: never.
    pub snapshot_every: time::Duration,
    /// The models cards run on (provider.rs); none: no catalog, every model taken as named.
    pub models: Option<provider::Catalog>,
    /// How long no agent may work while work waits before the owner's inbox says so (watchdog.rs);
    /// zero: never.
    pub idle_alert: std::time::Duration,
    /// The transcripts Claude Code writes of the agents' sessions, which teaching reads (teach_memory.rs); none reads
    /// none.
    pub transcripts: Option<Arc<dyn crate::transcript::Transcripts>>,
    /// The deciders Remit's small judgments are asked of (judge.rs, ADR-0118); none: every point runs its System Two.
    pub decider: Option<Arc<dyn judgment::Decider>>,
    /// The key the library's credentials are sealed with, one of the files agents are asked before
    /// reading (policy.rs `own_secrets`); empty: not set.
    pub secret_key_file: PathBuf,
    /// Where the key file's key is backed up, under its id, before anything is sealed under it
    /// (credentials.rs).
    pub key_backup_dir: PathBuf,
    /// Where the programs of the extensions the owner applied are installed (extensions.rs); empty: none
    /// can be proposed.
    pub ext_dir: PathBuf,
    /// The store's file, one of the files agents are asked before reading (`own_secrets`).
    pub database: PathBuf,
    /// Where the holders of client keys keep theirs (`own_secrets`).
    pub client_key_files: Vec<PathBuf>,
    /// The owner's file of source credentials (`own_secrets`).
    pub source_env_file: PathBuf,
    /// The directories of the owner's credentials no agent reads (-secret-dirs, `own_secrets`).
    pub secret_dirs: Vec<PathBuf>,
    /// The config dirs of the accounts agents run on (account_signin.rs); none: none are made.
    pub accounts: Option<Arc<dyn crate::account_signin::AccountDirs>>,
    /// The URL of this server's MCP endpoint, as agents reach it (access.rs).
    pub mcp_url: String,
    /// The URL of this server's OTLP endpoint, as agents reach it to export their telemetry (access.rs, ADR-0085); empty:
    /// agents export none.
    pub otlp_url: String,
    /// What the agents export, kept (telemetry.rs); none: none is kept or read.
    pub telemetry: Option<store::Telemetry>,
    /// Where the library's skills, and the skills that ship with Remit, are laid out for sessions
    /// (library_render.rs); empty: none is, and a skill cannot be saved.
    pub library_dir: PathBuf,
    /// The Claude Code configuration on this machine the library imports from
    /// (library_import.rs).
    pub claude_config_dir: PathBuf,
    /// The model and effort the Assistant runs on (assistant.rs), followed at every start; empty keeps Claude Code's
    /// default.
    pub assistant_model: String,
    pub assistant_effort: String,
    /// The Assistant's working directory, made at the first start.
    pub assistant_dir: PathBuf,
    /// Where Remit keeps its own clone of each repository a chain opens, as `<host>/<project>` (repo.rs).
    pub repos_dir: PathBuf,
    /// A submit publishes only a change a reviewer and a QC approved (submit.rs).
    pub require_approvals: bool,
    /// How long a finished chain keeps its worktrees after it last changed (archive.rs
    /// `collect_worktrees`); zero: always.
    pub keep_worktrees: std::time::Duration,
    /// The repository this instance runs, `owner/name` as the forge's releases name it (-update-repo, ADR-0087); a release
    /// of it installs itself here as its last step (ADR-0116). Empty: none, so no release installs here.
    pub update_repo: String,
    /// The directory of the installed releases, where `remit update` writes `update.json` (ADR-0087); empty: none read.
    pub releases_dir: PathBuf,
}

/// What the core is made of besides its configuration: one value per dependency, each a port its owner fills.
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

/// What the orchestrator holds in memory, behind one lock taken briefly and never across an await.
#[derive(Default)]
pub struct Shared {
    /// By chain id.
    pub chains: HashMap<String, Arc<ChainState>>,
    /// Reminders sent to a stalled chain's Chair, by chain id.
    pub nudges: HashMap<String, i64>,
    /// What the no-progress rail keeps of each running chain, by chain id (spin.rs).
    pub spins: HashMap<String, crate::spin::Spin>,
    /// Cards Recover found mid-turn in running chains: `resume_interrupted` picks them up.
    pub interrupted: Vec<Interruption>,
    /// Cards Recover found being handed over: `resume_interrupted` finishes their hand-overs (handover.rs).
    pub handing_over: Vec<String>,
    /// The session of each card that has one, by card id (session.rs).
    pub runs: HashMap<String, Arc<Run>>,
    /// The card each session's MCP bearer token was given to, by token.
    pub tokens: HashMap<String, String>,
    /// The employees each session was offered as subagent types, and their seat tokens (subagents.rs).
    pub seats: crate::subagents::SeatBook,
    /// When a chain's agent last began work (watchdog.rs `saw_work`); none before the watchdog first looks.
    pub last_work: Option<Time>,
    /// Since when nothing had run when the watchdog last told the owner (watchdog.rs `watch_idle`); none before it told.
    pub idle_told: Option<Time>,
    /// The workload as the last Tick counted it (schedule.rs).
    pub workload: wire::app::Workload,
    /// The capacity as the last Tick measured it (admit.rs).
    pub capacity: crate::admit::Capacity,
    /// The turns whose account failed and whose card moved on since the last Tick (failover.rs), which press on the
    /// capacity.
    pub failovers: usize,
    /// When the last Tick got through, none before the first (schedule.rs `last_tick`).
    pub last_tick: Option<Time>,
    /// The sessions each mentor's keys hold open, by mentor id then key id (presence.rs `device_online`): in memory
    /// only, as a session is.
    pub online: HashMap<String, HashMap<String, usize>>,
    /// The sign-ins in progress, by the id of the account each makes (account_signin.rs).
    pub signing: HashMap<String, crate::account_signin::AccountLogin>,
    /// The enabled plugins whose connections are accounts, by id (plugin_accounts.rs).
    pub account_plugins: Vec<String>,
    /// The accounts agents run on, logins first, never empty (accounts.rs); behind `account_mu` to change.
    pub accounts: Vec<model::Account>,
    /// The cards waiting for an account, by card id (failover.rs).
    pub waits: HashMap<String, crate::failover::WaitingCard>,
    /// The Assistant, once `ensure_assistant` made or read it (assistant.rs). Its changes are stored
    /// under `standalone_mu`, the writer of its home.
    pub assistant: Option<model::Assistant>,
}

/// The actions channels ask for (request.rs).
#[derive(Default)]
pub struct Requests {
    /// By request id.
    pub inflight: HashMap<String, Arc<Inflight>>,
    /// Drain has begun: no action starts.
    pub draining: bool,
}

pub struct Core {
    pub cfg: Config,
    pub store: Arc<Store>,
    pub notify: Arc<dyn Notifier>,
    pub launcher: Arc<dyn agent::Launcher>,
    pub sources: Arc<dyn Sources>,
    pub plugins: Arc<dyn PluginHost>,
    pub bundled: Arc<dyn Bundled>,
    clock: Clock,
    pub shared: Mutex<Shared>,
    pub requests: Mutex<Requests>,
    /// Actions being done (request.rs): Drain waits for them.
    pub acting: TaskTracker,
    /// Session loops and background work (recover.rs's `background`): Shutdown waits for them.
    pub work: TaskTracker,
    /// Cancelled when Shutdown begins: no agent session or background work starts after it.
    pub stopped: CancellationToken,
    /// Cancelled when Shutdown's deadline passes: what is still at work is cut short.
    pub work_ends: CancellationToken,
    /// Changes asking the scheduler for a Tick (schedule.rs's `kick`): one permit stands for every kick until it is taken.
    pub kicks: Notify,
    /// Makes a Tick's starts one at a time, so two cannot take the last slot.
    pub slot_mu: tokio::sync::Mutex<()>,
    /// Who Remit works as on each forge's host (forges.rs).
    pub forge_logins: Mutex<ForgeAccounts>,
    /// Makes Ticks one at a time.
    pub tick_mu: tokio::sync::Mutex<()>,
    /// Makes opening a card's session (dispatch, open) and stopping it for good (stop) one step each, card by card: a
    /// card's lock is taken before its chain's (keyed_locks.rs).
    pub sessions: crate::keyed_locks::KeyedLocks,
    /// The owner's changing calls on each chain, in the order he made them (request.rs's `act_on_chain`).
    pub chain_order: crate::keyed_locks::KeyedQueue,
    /// Makes the hand-over of each card one step, from the Chair's call to the new card (handover.rs).
    pub handovers: crate::keyed_locks::KeyedLocks,
    /// The writer of the standalone home, the Assistant's (homes.rs): its changes are stored and pushed one after the
    /// other, as a chain's are under its lock.
    pub standalone_mu: tokio::sync::Mutex<()>,
    /// Which accounts can run agents; Health is read from it (health.rs).
    pub outages: crate::health::Outages,
    /// Makes the changes to the accounts one at a time, held across what each asks of their config
    /// dirs (account_signin.rs, plugin_accounts.rs, accounts.rs).
    pub account_mu: tokio::sync::Mutex<()>,
    /// Makes choosing a card's account and storing it one step (accounts.rs, failover.rs).
    pub place_mu: tokio::sync::Mutex<()>,
    /// Held by pick_in_block (blocks.rs) from reading a block to storing the pick and its message, so two picks at once
    /// neither lose one another nor queue two messages. One lock for every block: picks are a click each.
    pub pick_mu: tokio::sync::Mutex<()>,
    /// What spreads a waiting card's next look and a retried turn's wait (failover.rs); in tests a spread of
    /// none, so waits last exactly their length.
    pub draw: crate::health::Draw,
    /// The core itself, for the work it starts on tasks of their own (a session's loop).
    me: Weak<Self>,
    /// The application the core hands up to: what a turn's end, a commit, a session and a Tick give the domains (ADR-0096).
    domains: Weak<dyn Domains>,
    /// The open tool-list streams of agents' sessions (tools.rs): a capability given mid-session reaches them.
    pub listeners: crate::tools::Listeners,
    /// What the bundled plugins ship, for ingest to weigh what a plugin writes: loaded once at start (the pipeline's
    /// `load_shipped`).
    pub shipped: Mutex<Arc<crate::ingest::ShippedSignals>>,
}

impl Core {
    /// The core of cfg over d, handing up to domains: the application, which holds it (ADR-0096).
    pub fn new(cfg: Config, d: Deps, domains: Weak<dyn Domains>) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            notify: d.notify,
            me: me.clone(),
            domains,
            outages: crate::health::Outages::default(),
            account_mu: tokio::sync::Mutex::new(()),
            place_mu: tokio::sync::Mutex::new(()),
            pick_mu: tokio::sync::Mutex::new(()),
            draw: if cfg!(any(test, feature = "testing")) {
                Arc::new(|| 0.5)
            } else {
                Arc::new(crate::health::random_unit)
            },
            cfg,
            store: d.store,
            launcher: d.launcher,
            sources: d.sources,
            plugins: d.plugins,
            bundled: d.bundled,
            clock: d.clock,
            shared: Mutex::new(Shared {
                accounts: vec![crate::accounts::default_account()],
                ..Shared::default()
            }),
            requests: Mutex::default(),
            acting: TaskTracker::new(),
            work: TaskTracker::new(),
            stopped: CancellationToken::new(),
            work_ends: CancellationToken::new(),
            kicks: Notify::new(),
            slot_mu: tokio::sync::Mutex::new(()),
            forge_logins: Mutex::default(),
            tick_mu: tokio::sync::Mutex::new(()),
            sessions: crate::keyed_locks::KeyedLocks::default(),
            chain_order: crate::keyed_locks::KeyedQueue::default(),
            handovers: crate::keyed_locks::KeyedLocks::default(),
            standalone_mu: tokio::sync::Mutex::new(()),
            listeners: crate::tools::Listeners::default(),
            shipped: Mutex::default(),
        })
    }

    /// The application the core hands up to; `Stopping` once it is dropped (the server is shutting down).
    pub fn domains(&self) -> Result<Arc<dyn Domains>> {
        self.domains.upgrade().ok_or(Error::Stopping)
    }

    /// The core as its owner holds it; none once it is dropped.
    pub fn me(&self) -> Option<Arc<Self>> {
        self.me.upgrade()
    }

    pub fn now(&self) -> Time {
        (self.clock)()
    }

    /// Deletes the events ids name from the stream and tells the channels; a failure is logged.
    pub async fn delete_events(&self, ids: &[i64]) {
        let keys = ids.to_vec();
        match self.store.update(move |tx| tx.delete_events(&keys)).await {
            Ok(gone) => gone.iter().for_each(|e| self.notify.event_deleted(e)),
            Err::<_, store::Error>(err) => tracing::warn!(?ids, %err, "delete events"),
        }
    }

    /// Stores e as of now, tells the channels, and returns it as stored.
    pub async fn add_event(&self, mut e: model::Event) -> Result<model::Event> {
        let now = self.now();
        (e.created_at, e.updated_at) = (now, now);
        let saved = self.store.update(move |tx| tx.add_event(e)).await?;
        self.notify.event_added(&saved);
        Ok(saved)
    }

    /// Stores note e as `add_event` does, for a caller that reads nothing back: what it says is shown, nothing waits on
    /// it, so a failure to store it is logged.
    pub async fn note(&self, e: model::Event) {
        let (home, card) = (e.home.clone(), e.card_id.clone());
        if let Err(err) = self.add_event(e).await {
            tracing::error!(%home, %card, %err, "note");
        }
    }

    /// What the orchestrator holds in memory. A panic under the lock leaves nothing half-changed that a later reader
    /// could not take as it is, so a poisoned lock is read on.
    pub fn shared(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records a decision of by's in chain chain_id that no change of the chain carries, and tells the channels. A failure
    /// to store it is logged: what was decided is done all the same.
    pub async fn decide(&self, chain_id: &str, kind: DecisionKind, text: &str, why: &str, card_id: &str, by: Actor) {
        let d = Decision {
            id: new_id("d_"),
            chain_id: chain_id.to_owned(),
            kind,
            by: Some(by),
            text: text.to_owned(),
            why: why.to_owned(),
            card_id: card_id.to_owned(),
            created_at: self.now(),
            ..Decision::default()
        };
        let stored = d.clone();
        match self.store.update(move |tx| tx.put_decision(&stored)).await {
            Ok(()) => self.notify.decision_added(&d),
            Err(err) => tracing::error!(chain = %chain_id, %err, "decision"),
        }
    }
}

/// A new id: prefix and twelve random hex digits.
pub fn new_id(prefix: &str) -> String {
    let mut b = [0_u8; 6];
    // The OS's random source does not fail on the systems Remit runs on; were it to, the id is still unique enough
    // for one store's records, which refuse a duplicate.
    getrandom::fill(&mut b).ok();
    let mut out = String::with_capacity(prefix.len().saturating_add(12));
    out.push_str(prefix);
    for byte in b {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The first line of s, trimmed.
pub fn headline(s: &str) -> &str {
    s.split('\n').next().unwrap_or_default().trim()
}

/// The first line of s, cut to limit characters.
pub fn first_line(s: &str, limit: usize) -> String {
    cut(headline(s), limit)
}

/// s cut to limit characters, its last one an ellipsis when it was longer.
pub fn cut(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// s, or d when s says nothing.
pub fn or_default(s: &str, d: &str) -> String {
    if s.trim().is_empty() {
        d.to_owned()
    } else {
        s.to_owned()
    }
}

/// s on one line, cut to limit characters, its last one an ellipsis when it was longer.
pub fn gist(s: &str, limit: usize) -> String {
    cut(&s.split_whitespace().collect::<Vec<_>>().join(" "), limit)
}

/// The record a lookup found, and none when it found none: a write that creates a record has nothing before it.
pub fn optional<T>(found: store::Result<T>) -> Result<Option<T>> {
    match found {
        Ok(record) => Ok(Some(record)),
        Err(store::Error::NotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// n as a page's size: shown when n names none, at most most.
pub fn page_size(n: i64, shown: usize, most: usize) -> usize {
    if n <= 0 {
        return shown;
    }
    clamp(n).min(most)
}

/// n, a count an agent gave, as a usize: none when it is negative.
pub fn clamp(n: i64) -> usize {
    usize::try_from(n.max(0)).unwrap_or(usize::MAX)
}
