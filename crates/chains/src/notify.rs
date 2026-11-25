//! The kinds of change the chain core makes, told to the channels (ADR-0096). A channel adapter (the web app's today)
//! implements it, with the Notifier of each crate above that adds the kinds of change only that crate makes, and pushes
//! each change to whoever follows it there. The core calls it while it holds the locks that order its changes, so a
//! method returns at once: it may queue, never wait, and never calls back into the core.

use model::{Card, ClientKey, Decision, Draft, Edge, Employee, Event, InboxItem};

use wire::app::ChainRead;

/// Tells the channels what changed, once it is stored and in the order it was stored: one method per kind of change,
/// carrying the record as stored.
/// `Watched` (views_watched.rs), the Notifier orch calls, forwards every method: a method added here is forwarded there too.
pub trait Notifier: Send + Sync {
    fn chain_updated(&self, c: &ChainRead);
    fn chain_deleted(&self, chain_id: &str);
    fn card_updated(&self, c: &Card);
    fn edge_added(&self, e: &Edge);
    fn decision_added(&self, d: &Decision);
    fn event_added(&self, e: &Event);
    fn event_updated(&self, e: &Event);
    fn inbox_updated(&self, it: &InboxItem);
    /// An employee, as last stored.
    fn employee_updated(&self, e: &Employee);
    fn client_key_updated(&self, k: &ClientKey);
    fn client_key_deleted(&self, id: &str);
    /// A mentor's device key, deleted: the server closes its sessions.
    fn worker_key_deleted(&self, id: &str);
    /// The owner's draft, as saved (drafts.rs).
    fn draft_updated(&self, _d: &Draft) {}
    /// The owner's draft, ended: deleted by him (a send ends its drafts with the send).
    fn draft_deleted(&self, _d: &Draft) {}
    /// What the owner reads of whether agents can run, each time it changes (health.rs).
    fn health_updated(&self, _h: &wire::app::Health) {}
    /// A chain's views moved with nothing of it pushed: a verdict moved its verification and its board card's chip. Only
    /// the views mark it (views_watched.rs).
    fn views_moved(&self, _chain_id: &str) {}
    /// A landing as last stored, or with the release that carried it as that release was last stored (commit.rs).
    fn landing_updated(&self, _l: &wire::app::LandingRead) {}
    /// A landing or a release of repository repo was written: its queue and releases moved. Only the views mark it.
    fn repo_moved(&self, _repo: &str) {}
    /// A mentor's standing moved with nothing pushed: a device key used, a heartbeat aged. Only the views mark it.
    fn mentors_moved(&self) {}
    /// What a card shows of the moment, not stored yet (stream.rs).
    fn card_shown(&self, _s: &wire::app::CardShown) {}
    /// Text an agent streams, not stored yet, as it comes (stream.rs).
    fn event_delta(&self, _d: &wire::app::EventDelta) {}
    /// An event, as last stored, deleted.
    fn event_deleted(&self, _e: &Event) {}
    /// A mentor of the pool, as last stored.
    fn mentor_updated(&self, _m: &model::Mentor) {}
    /// A mentor's help on a card, as last stored: its chain and its mentor's devices show it.
    fn help_updated(&self, _h: &model::Help) {}
    /// A run of a script with its steps, as last stored (ADR-0073).
    fn workflow_updated(&self, _run: &model::Workflow) {}
    /// The columns of the boards whose cards arrive one by one, as the views computed them again (boards.rs).
    fn boards_updated(&self, _b: &wire::app::Boards) {}
    /// The workload, as each Tick counts it (schedule.rs).
    fn workload_updated(&self, _w: &wire::app::Workload) {}
    /// The at-work view, whole, whenever it moves (atwork.rs).
    fn at_work_updated(&self, _v: &wire::app::AtWork) {}
    /// An event signal, as last stored (event_signal_keys.rs).
    fn event_signal_updated(&self, _es: &model::EventSignal) {}
    /// An event signal, deleted with its key and sensors.
    fn event_signal_deleted(&self, _id: &str) {}
    /// A sensor, as last stored (sensor_settings.rs).
    fn sensor_updated(&self, _s: &model::Sensor) {}
    /// A sensor, deleted.
    fn sensor_deleted(&self, _id: &str) {}
    /// A source with its tools and connections, as it stands now (credentials.rs).
    fn source_updated(&self, _s: &wire::app::ToolSource) {}
    /// An extension as the owner sees it, as it stands now (extension_state.rs).
    fn extension_updated(&self, _e: &model::Extension) {}
    /// An extension that is no longer installed.
    fn extension_deleted(&self, _id: &str) {}
    /// A playbook, as last stored (playbooks.rs).
    fn playbook_updated(&self, _p: &model::Playbook) {}
    /// A playbook, deleted.
    fn playbook_deleted(&self, _id: &str) {}
    /// An agent definition, as last stored (agents.rs).
    fn agent_updated(&self, _a: &model::Agent) {}
    /// An agent definition, deleted.
    fn agent_deleted(&self, _id: &str) {}
    /// A skill of the library, deleted.
    fn skill_deleted(&self, _name: &str) {}
    /// A piece of feedback an agent sent, or the owner read (feedback.rs).
    fn feedback_updated(&self, _f: &model::Feedback) {}
    /// An insight the analyst's Chair recorded (analyst.rs `record_insight`): the Outcomes page lists it.
    fn insight_added(&self, _d: &Decision) {}
    /// An objective as stored, with where its key results stand (objective_change.rs `tell_objective`).
    fn objective_updated(&self, _v: &model::ObjectiveView) {}
    /// A project as last stored (ADR-0108): each write of the Assistant's project tools tells it once it commits.
    fn project_updated(&self, _p: &model::Project) {}
}
