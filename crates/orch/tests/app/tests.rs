//! The chain core driven as its callers drive it: a channel's actions on a real store, with fake sessions and ports, and
//! what the core told the channels recorded.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use wire::app::{ConnectionView, ToolSource};

use async_trait::async_trait;
use model::{
    Card, CardStatus, Chain, ChainStatus, DecisionKind, Edge, Employee, Event, EventKind, InboxItem, Origin, Time,
    by_owner,
};
use source::{Caller, Extension, PluginRef, PollResult, ScriptInput, Status};
use time::macros::datetime;

use orch::{Bundled, Config, CreateChain, Deps, Orchestrator, PluginHost, Request, Sources};
use wire::app::ChainRead;

/// What the channels were told, by kind, and the views pushed to them (views.rs).
#[derive(Default)]
pub(crate) struct Told(Mutex<Vec<String>>, Mutex<Pushed>);

/// The views pushed, in the order they were pushed.
#[derive(Default, Clone)]
pub(crate) struct Pushed {
    pub boards: Vec<wire::app::Board>,
    pub standings: Vec<wire::app::ChainStanding>,
    pub agents: Vec<wire::app::AgentStanding>,
    /// The revision of each push.
    pub revs: Vec<i64>,
}

impl Told {
    fn push(&self, s: String) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push(s);
    }
    pub(crate) fn all(&self) -> Vec<String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl orch::chains::Notifier for Told {
    fn chain_updated(&self, c: &ChainRead) {
        self.push(format!("chain {} {}", c.chain.id, c.chain.status()));
    }

    fn chain_deleted(&self, chain_id: &str) {
        self.push(format!("chain {chain_id} deleted"));
    }

    fn card_updated(&self, c: &Card) {
        self.push(format!("card {} {}", c.id, c.status()));
    }

    fn help_updated(&self, h: &model::Help) {
        self.push(format!("help {} {}", h.id, h.state));
    }

    fn edge_added(&self, e: &Edge) {
        self.push(format!("edge {}", e.id));
    }

    fn decision_added(&self, d: &model::Decision) {
        self.push(format!("decision {}", d.kind));
    }

    fn event_added(&self, e: &Event) {
        self.push(format!("event {} added", e.id));
    }

    fn event_updated(&self, e: &Event) {
        self.push(format!("event {} {}", e.id, e.status));
    }

    fn inbox_updated(&self, it: &InboxItem) {
        self.push(format!("inbox {} {}", it.id, it.status));
    }

    fn employee_updated(&self, e: &Employee) {
        self.push(format!("employee {}", e.id));
    }

    fn client_key_updated(&self, k: &model::ClientKey) {
        self.push(format!("client key {} {}", k.id, k.rev));
    }

    fn client_key_deleted(&self, id: &str) {
        self.push(format!("client key {id} deleted"));
    }

    fn worker_key_deleted(&self, id: &str) {
        self.push(format!("worker key {id} deleted"));
    }

    fn draft_updated(&self, d: &model::Draft) {
        self.push(format!("draft {} saved {}", d.id, d.rev));
    }

    fn draft_deleted(&self, d: &model::Draft) {
        self.push(format!("draft {} deleted", d.id));
    }

    fn health_updated(&self, h: &wire::app::Health) {
        self.push(format!("health {} {}", h.ok, h.rev));
    }

    fn playbook_updated(&self, p: &model::Playbook) {
        self.push(format!("playbook_updated {} {}", p.id, p.rev));
    }

    fn playbook_deleted(&self, id: &str) {
        self.push(format!("playbook_deleted {id}"));
    }

    fn agent_updated(&self, a: &model::Agent) {
        self.push(format!("agent_updated {} {}", a.id, a.rev));
    }

    fn agent_deleted(&self, id: &str) {
        self.push(format!("agent_deleted {id}"));
    }

    fn skill_deleted(&self, name: &str) {
        self.push(format!("skill_deleted {name}"));
    }
}

impl orch::pipeline::notify::Notifier for Told {}

impl orch::capability::notify::Notifier for Told {
    fn skill_updated(&self, s: &wire::app::ListedSkill) {
        self.push(format!("skill_updated {} {} files", s.name, s.files.len()));
    }
}

impl orch::crew::notify::Notifier for Told {
    fn retrospective_added(&self, r: &wire::app::RetrospectiveRead) {
        self.push(format!("retrospective {} by {}", r.retrospective.id, r.by_name));
    }
}

impl orch::Notifier for Told {
    fn board_updated(&self, b: &wire::app::Board) {
        let mut p = self.pushed();
        p.revs.push(b.rev);
        p.boards.push(b.clone());
    }

    fn chain_standing_updated(&self, s: &wire::app::ChainStanding) {
        let mut p = self.pushed();
        p.revs.push(s.rev);
        p.standings.push(s.clone());
    }

    fn agent_standing_updated(&self, a: &wire::app::AgentStanding) {
        let mut p = self.pushed();
        p.revs.push(a.rev);
        p.agents.push(a.clone());
    }
}

impl Told {
    fn pushed(&self) -> std::sync::MutexGuard<'_, Pushed> {
        self.1.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Ports no test reaches: each answers as an empty world would.
pub(crate) struct Nothing;

impl agent::Launcher for Nothing {
    fn start(
        &self,
        _spec: agent::Spec,
        _h: Arc<dyn agent::Handler>,
    ) -> agent::BoxFuture<'_, Result<Box<dyn agent::Session>, agent::Error>> {
        Box::pin(async { Err(agent::Error::TurnFailed("no agent runs in these tests".to_owned())) })
    }
}

/// The source a release step reaches in these tests, and its credential.
pub(crate) const REGISTRY: &str = "registry";
pub(crate) const REGISTRY_TOKEN: &str = "tok_registry_secret";

#[async_trait]
impl Sources for Nothing {
    fn source_ids(&self) -> Vec<String> {
        Vec::new()
    }
    async fn lend(
        &self,
        _caller: &Caller,
        _source: &str,
        _connection: &str,
        _call: &str,
        _effect: grant::Effect,
    ) -> source::Result<(PluginRef, BTreeMap<String, String>)> {
        Ok(Default::default())
    }
    fn use_extensions(&self, _root: &Path, _exts: &[Extension]) {}
    /// One source a release step reaches, `registry`, its credential the proxy's (shipping_release.rs).
    async fn injection(&self, source: &str, _connection: &str) -> source::Result<source::InjectSpec> {
        if source != REGISTRY {
            return Err(source::Error::msg(format!("no source {source} here")));
        }
        Ok(source::InjectSpec {
            base: "https://registry.example".to_owned(),
            headers: BTreeMap::from([("Authorization".to_owned(), format!("Bearer {REGISTRY_TOKEN}"))]),
            secrets: vec![REGISTRY_TOKEN.to_owned()],
            ..source::InjectSpec::default()
        })
    }
    async fn catalog(&self) -> source::Result<Vec<ToolSource>> {
        Ok(Vec::new())
    }
    fn blank(&self, _source: &str) -> Option<(ToolSource, ConnectionView)> {
        None
    }
    async fn status(&self) -> source::Result<Vec<Status>> {
        Ok(Vec::new())
    }
    fn invalidate(&self, _source: &str, _connection: &str) {}
    async fn ready_tools(&self, _caps: &[&str]) -> source::Result<Vec<String>> {
        Ok(Vec::new())
    }
    fn doing(&self, _tool: &str) -> String {
        String::new()
    }
    async fn run_script(&self, _c: &Caller, _s: &str, _n: &str, _i: ScriptInput) -> source::Result<String> {
        Ok(String::new())
    }
    async fn poll(
        &self,
        _caller: &Caller,
        _source: &str,
        _connection: &str,
        _query: &str,
        _cursor: &str,
        _limit: usize,
    ) -> source::Result<PollResult> {
        Ok(PollResult::default())
    }
}

impl PluginHost for Nothing {
    fn plugin(&self, _spec: &plugin::Spec) -> Arc<dyn plugin_sdk::Plugin> {
        unreachable_plugin()
    }
    fn stop(&self, _id: &str) {}
    fn close(&self) {}
}

impl Bundled for Nothing {
    fn lookup(&self, _id: &str) -> Option<Arc<dyn plugin_sdk::Plugin>> {
        None
    }
    fn ids(&self) -> Vec<String> {
        Vec::new()
    }
}

fn unreachable_plugin() -> Arc<dyn plugin_sdk::Plugin> {
    panic!("no plugin runs in these tests")
}

/// Sessions of a quiet agent: each starts, and each prompt waits until its turn is cancelled or its session closed. What
/// they were asked is recorded, in order. Each session's handler is kept by card, for a test to speak as its process, and
/// its background tasks have written what `tasks` holds, by task id. The model and effort each card's last session started
/// on are kept by card.
#[derive(Default)]
pub(crate) struct Agents(
    Arc<Mutex<Vec<String>>>,
    Mutex<BTreeMap<String, Arc<dyn agent::Handler>>>,
    Arc<Mutex<BTreeMap<String, String>>>,
    Mutex<BTreeMap<String, (String, String)>>,
);

impl Agents {
    pub(crate) fn all(&self) -> Vec<String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Waits until what the sessions were asked holds said; a test fails after 5s.
    pub(crate) async fn until(&self, said: &str) {
        for _ in 0..250 {
            if self.all().iter().any(|s| s == said) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("not asked {said:?}: {:?}", self.all());
    }

    /// The handler card_id's session was started with, to speak as its process.
    pub(crate) fn handler(&self, card_id: &str) -> Arc<dyn agent::Handler> {
        let held = self.1.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(held.get(card_id).expect("a session started for the card"))
    }
}

impl agent::Launcher for Agents {
    fn start(
        &self,
        spec: agent::Spec,
        h: Arc<dyn agent::Handler>,
    ) -> agent::BoxFuture<'_, Result<Box<dyn agent::Session>, agent::Error>> {
        let said = Arc::clone(&self.0);
        let tasks = Arc::clone(&self.2);
        self.1
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(spec.card_id.clone(), Arc::clone(&h));
        self.3
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(spec.card_id.clone(), (spec.model.clone(), spec.effort.clone()));
        Box::pin(async move {
            let s = Quiet {
                id: format!("s_{}", spec.card_id),
                card: spec.card_id,
                said,
                tasks,
                ended: tokio::sync::Notify::new(),
                _h: h,
            };
            s.say(format!("start {}", s.card));
            let s: Box<dyn agent::Session> = Box::new(s);
            Ok(s)
        })
    }
}

struct Quiet {
    id: String,
    card: String,
    said: Arc<Mutex<Vec<String>>>,
    tasks: Arc<Mutex<BTreeMap<String, String>>>,
    ended: tokio::sync::Notify,
    /// Held while the session lives, as the AI runtime's session holds its handler.
    _h: Arc<dyn agent::Handler>,
}

impl Quiet {
    fn say(&self, s: String) {
        self.said.lock().unwrap_or_else(PoisonError::into_inner).push(s);
    }
}

impl agent::Session for Quiet {
    fn id(&self) -> &str {
        &self.id
    }
    fn prompt(&self, _prompt: model::Prompt) -> agent::BoxFuture<'_, Result<model::StopReason, agent::Error>> {
        self.say(format!("prompt {}", self.card));
        Box::pin(async move {
            self.ended.notified().await;
            Ok(model::StopReason::Cancelled)
        })
    }
    fn steer(&self, _prompt: model::Prompt) -> agent::BoxFuture<'_, Result<bool, agent::Error>> {
        Box::pin(async { Ok(false) })
    }
    fn cancel(&self) -> agent::BoxFuture<'_, Result<(), agent::Error>> {
        self.say(format!("cancel {}", self.card));
        self.ended.notify_one();
        Box::pin(async { Ok(()) })
    }
    fn close(&self) {
        self.say(format!("close {}", self.card));
        self.ended.notify_one();
    }
    fn context_lost(&self) -> bool {
        false
    }
    fn move_to(&self, _account: String) -> agent::BoxFuture<'_, Result<(), agent::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn stop_task(&self, task_id: String) -> agent::BoxFuture<'_, Result<(), agent::Error>> {
        self.say(format!("stop task {task_id}"));
        Box::pin(async { Ok(()) })
    }
    fn task_output(&self, task_id: String) -> agent::BoxFuture<'_, Result<wire::runtime::TaskOutput, agent::Error>> {
        let text = self
            .tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&task_id)
            .cloned();
        Box::pin(async move {
            let text = text.ok_or_else(|| agent::Error::TurnFailed(format!("No task found with ID: {task_id}")))?;
            Ok(wire::runtime::TaskOutput {
                total_bytes: u64::try_from(text.len()).unwrap_or(u64::MAX),
                text,
                truncated: false,
            })
        })
    }
}

/// A rig whose sessions are agents'.
pub(crate) fn rig_of_agents(agents: &Arc<Agents>) -> Rig {
    rig_with(|_| {}, agents.clone(), Arc::new(|| NOW))
}

pub(crate) const NOW: Time = Time(datetime!(2026-11-10 9:00 +7));

pub(crate) struct Rig {
    pub(crate) o: Arc<Orchestrator>,
    pub(crate) told: Arc<Told>,
    _dir: tempfile::TempDir,
}

pub(crate) fn rig() -> Rig {
    rig_with(|_| {}, Arc::new(Nothing), Arc::new(|| NOW))
}

/// A rig whose configuration edit changes.
pub(crate) fn rig_of(edit: impl FnOnce(&mut Config)) -> Rig {
    rig_with(edit, Arc::new(Nothing), Arc::new(|| NOW))
}

/// A rig whose configuration edit changes, whose sessions launcher starts and whose time clock says.
pub(crate) fn rig_with(edit: impl FnOnce(&mut Config), launcher: Arc<dyn agent::Launcher>, clock: orch::Clock) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(store::storetest::open(&dir.path().join("remit.db")).unwrap());
    let told = Arc::new(Told::default());
    let mut cfg = Config {
        core: orch::chains::Config {
            work_root: dir.path().join("work"),
            assistant_dir: dir.path().join("assistant"),
            secret_key_file: dir.path().join("secret.key"),
            key_backup_dir: dir.path().join("keys"),
            ..orch::chains::Config::default()
        },
        ..Config::default()
    };
    edit(&mut cfg);
    let deps = Deps {
        store,
        notify: told.clone(),
        launcher,
        sources: Arc::new(Nothing),
        plugins: Arc::new(Nothing),
        bundled: Arc::new(Nothing),
        clock,
    };
    let o = Orchestrator::new(cfg, deps);
    Rig { o, told, _dir: dir }
}

pub(crate) fn owner(id: &str) -> Request {
    Request {
        id: id.to_owned(),
        by: by_owner("owner", "web"),
    }
}

impl Rig {
    pub(crate) async fn create(&self, req: &str, intent: &str) -> Chain {
        let input = CreateChain {
            intent: intent.to_owned(),
            ..CreateChain::default()
        };
        self.o.create_chain(owner(req), input).await.unwrap()
    }

    async fn chain(&self, id: &str) -> Chain {
        let id = id.to_owned();
        self.o.store.read(move |r| r.chain(&id)).await.unwrap()
    }

    /// The messages ever given to a card, queued or taken by its session: what a test reads when the session may have
    /// taken one already.
    async fn prompts(&self, card: &str) -> Vec<Event> {
        let card = card.to_owned();
        let events = self.o.store.read(move |r| r.card_events(&card, 1000)).await.unwrap();
        events.into_iter().filter(|e| e.kind() == EventKind::Prompt).collect()
    }

    async fn decisions(&self, chain: &Chain) -> Vec<DecisionKind> {
        let id = chain.id.clone();
        let view = self.o.store.view(id).await.unwrap();
        view.decisions.into_iter().map(|d| d.kind).collect()
    }
}

// The owner's chain is queued with its own task objective, starts on the Tick its creation runs, and its Chair is
// handed the opening.
#[tokio::test]
async fn a_chain_the_owner_asks_for_starts_with_its_chair_briefed() {
    let agents = Arc::new(Agents::default());
    let r = rig_of_agents(&agents);
    let ch = r.create("r1", "Fix the login page\nit times out").await;
    assert_eq!(*ch.status(), ChainStatus::Running, "{ch:?}");
    assert_eq!(ch.title, "Fix the login page");
    assert_eq!(ch.kind, model::ChainKind::Work);
    let serves = ch.serves.clone().unwrap();
    let objective = r.o.store.read(move |r| r.objective(&serves.objective)).await.unwrap();
    assert!(objective.task && objective.outcome == ch.goal, "{objective:?}");
    let chair =
        r.o.store
            .read({
                let id = ch.chair_id.clone();
                move |r| r.card(&id)
            })
            .await
            .unwrap();
    assert_eq!((chair.origin, chair.home.as_str()), (Origin::Chair, ch.id.as_str()));
    let opening = r.prompts(&chair.id).await;
    assert_eq!(opening.len(), 1, "{opening:?}");
    assert_eq!(opening.first().map(|e| e.priority), Some(model::PRIORITY_OPENING));
    agents.until(&format!("prompt {}", chair.id)).await;
    assert!(
        r.told.all().contains(&format!("chain {} running", ch.id)),
        "{:?}",
        r.told.all()
    );
}

// Pause cuts the chain's turns and holds it; resume runs it again and tells its Chair.
#[tokio::test]
async fn pause_holds_the_chain_and_resume_tells_the_chair() {
    let agents = Arc::new(Agents::default());
    let r = rig_of_agents(&agents);
    let ch = r.create("r1", "Ship it").await;
    agents.until(&format!("prompt {}", ch.chair_id)).await;
    let before = r.prompts(&ch.chair_id).await.len();
    r.o.core.pause(owner("r2"), ch.id.clone(), String::new()).await.unwrap();
    assert_eq!(*r.chain(&ch.id).await.status(), ChainStatus::Paused);
    agents.until(&format!("cancel {}", ch.chair_id)).await;
    // the cut turn's end queues its continuation, held while the chain is paused
    for _ in 0..500 {
        if r.prompts(&ch.chair_id)
            .await
            .iter()
            .any(|e| e.title == "Continue after the pause")
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    r.o.core
        .resume(owner("r3"), ch.id.clone(), "go on".to_owned())
        .await
        .unwrap();
    let resumed = r.chain(&ch.id).await;
    assert_eq!(*resumed.status(), ChainStatus::Running);
    // the turn the pause cut goes on first, then the Chair reads that the chain resumed
    let prompts = r.prompts(&ch.chair_id).await;
    let titles: Vec<&str> = prompts.iter().skip(before).map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["Continue after the pause", "Resume the chain"], "{prompts:?}");
    let kinds = r.decisions(&resumed).await;
    assert!(
        kinds.contains(&DecisionKind::Pause) && kinds.contains(&DecisionKind::Resume),
        "{kinds:?}"
    );
}

// The owner stopping an agent with a why: the agent stops, its Chair is told the why and not to restart it on its own, and
// the agent's stream says who stopped it and that the Chair was told. A why given after the stop reaches the Chair too.
#[tokio::test]
async fn a_stop_tells_the_chair_why_and_not_to_restart_it() {
    let r = rig();
    let ch = r.create("r1", "Fix the tests").await;
    let worker = {
        let mut built = Card::default();
        built.id = "c_worker".to_owned();
        built.home = ch.id.clone();
        built.name = "Tester".to_owned();
        built.origin = Origin::Hired;
        built.session_id = "s1".to_owned();
        built.created_at = NOW;
        built.updated_at = NOW;
        built.set_status(CardStatus::Idle, NOW);
        built
    };
    r.o.store
        .update(move |tx| tx.put_card(worker).map(|_| ()))
        .await
        .unwrap();
    let why = "it rewrites the wrong module";
    let card =
        r.o.core
            .stop_card(owner("r2"), "c_worker".to_owned(), why.to_owned())
            .await
            .unwrap();
    assert_eq!(card.status(), &CardStatus::Stopped);
    let told = r.prompts(&ch.chair_id).await;
    assert!(
        told.iter()
            .any(|e| e.text.contains(why) && e.text.contains("Do not restart it")),
        "{told:?}"
    );
    let stream = r.o.store.read(|s| s.card_events("c_worker", 100)).await.unwrap();
    let line = format!("The owner stopped Tester: {why}. The Chair was told.");
    assert!(
        stream.iter().any(|e| e.kind() == EventKind::System && e.text == line),
        "{stream:?}"
    );
    let later = "the module moved";
    r.o.core
        .explain_stop(owner("r3"), "c_worker".to_owned(), later.to_owned())
        .await
        .unwrap();
    let told = r.prompts(&ch.chair_id).await;
    assert!(told.iter().any(|e| e.text.contains(later)), "{told:?}");
}
