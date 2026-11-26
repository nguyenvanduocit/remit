use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use jiff::Timestamp;
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::sync::oneshot;

use claudehooks::input::{ConfigSource, LoadReason, MemoryType, ModelSwitchSource};
use claudehooks::output::{Decision as BlockDecision, PermissionDecision};
use claudehooks::{Event, HookOutput, SpecificOutput};

use model::{Prompt, StopReason, ToolStatus};
use wire::runtime::{
    Failure, Limit, LimitStatus, Moment, Note, NoteKind, PermissionAnswer, PermissionRequest, TASK_SUBAGENT,
    TASK_WORKFLOW, TaskOutput, ToolCall, ToolKind, Usage,
};

use crate::Error;
use crate::agent::{BoxFuture, Exit, Handler, Launcher, SdkMessage, Session, Spec, Update};
use crate::context::session_context;
use crate::failure::{claude_failure, resets_at, time_of};
use crate::options::{by_reference, mcp_config, options, repository_root};
use crate::processes::ProcessSampler;
use crate::refused::{ASKING_MODE, Refusals};
use crate::subagents::Subagents;
use crate::tasks::{OwnedTask, TaskLog, WorkflowLaunch};
use crate::toolcall::{describe, display_path, plan_of};

/// The clock a limit's reset, said in words ("resets 10pm"), is read against.
pub type Clock = Arc<dyn Fn() -> Timestamp + Send + Sync>;

/// The config dir of an account, where its login lives when it is one.
pub type Home = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Where Claude Code keeps the login of a config dir: this machine's store (`claudecode::Store::of`), or in tests a file.
pub type StoreOf = Arc<dyn Fn(&str) -> claudecode::Store + Send + Sync>;

/// How much life a token set on a moved session keeps before it is renewed. Claude Code renews a login when it starts
/// only once its token has less than 5 minutes left, so a renewal asked earlier would find nothing to do.
const RENEW_LEFT: Duration = Duration::from_secs(4 * 60);
/// How long a renewal that failed waits before it is tried again, while the token it would replace still holds.
const RENEW_RETRY: Duration = Duration::from_secs(30);
/// How long the agent waits, after a tool call or as a subagent starts, for what its handler adds to its context
/// (`Handler::on_moment`): past it, the agent goes on with nothing added.
const MOMENT_WAIT: Duration = Duration::from_secs(3);

/// Starts every session as a Claude Code process of its own (`command`), signed in as its account is: `home` names the
/// account's config dir, which holds its login, or for a plugin's account no login, the variables of its spec
/// (`AccountEnv`) signing it in; nothing else carries a credential of the provider. `env` is what else the process is
/// given. A session inherits no secret of the server: its environment is the names claudecode lists, `hide`, the paths in
/// which Remit keeps its secrets (the key file, the database and its copies, the source env files, the process manager's
/// config), which it and everything its shell starts cannot read, and `read_only`, the paths the server runs from (its
/// binary and checkout), read as they are and cannot be changed by it (ADR-0018). A launcher with no path to hide starts
/// no session: isolation is not left to a caller that forgot to ask for it.
pub struct ClaudeLauncher {
    pub command: Vec<String>,
    pub home: Option<Home>,
    pub env: Vec<String>,
    pub hide: Vec<String>,
    pub read_only: Vec<String>,
    pub stderr: Option<claudecode::Stderr>,
    pub now: Clock,
    /// Where the login of an account's config dir is read when a session moves onto that account (`Session::move_to`).
    pub store: StoreOf,
}

/// What Claude Code says when told to resume a session it does not have.
const MISSING_SESSION: &str = "No conversation found with session ID";

impl Launcher for ClaudeLauncher {
    /// Starts a session as spec says: the one `spec.resume` names, or a new one, also when Claude Code no longer has the
    /// one to resume (`context_lost`).
    fn start(&self, spec: Spec, h: Arc<dyn Handler>) -> BoxFuture<'_, Result<Box<dyn Session>, Error>> {
        Box::pin(async move {
            let s = match self.start_on(&spec, &h, &spec.resume).await {
                Err(e) if !spec.resume.is_empty() && missing_session(&e) => {
                    let mut s = self.start_on(&spec, &h, "").await?;
                    s.lost = true;
                    s
                }
                s => s?,
            };
            let session: Box<dyn Session> = Box::new(s);
            Ok(session)
        })
    }
}

/// Says whether a start failed because Claude Code does not have the session it was told to resume.
fn missing_session(e: &Error) -> bool {
    let Error::Claude(claudecode::Error::Initialize(inner)) = e else {
        return false;
    };
    matches!(&**inner, claudecode::Error::Closed { said } if said.iter().any(|s| s.contains(MISSING_SESSION)))
}

impl ClaudeLauncher {
    /// What puts a session on the account of spec: the account's config dir, which holds its login when it is one, then
    /// the variables its plugin gave when it is a plugin's (ADR-0071), which are all that can sign that session in, as no
    /// one signs in to its dir. A launcher with no home leaves Claude Code its own config dir, where the machine's login
    /// is: it starts a login's session there and refuses a plugin account's, which would run on that login if its
    /// variables failed.
    fn account_env(&self, spec: &Spec) -> Result<Vec<String>, Error> {
        let Some(home) = &self.home else {
            if !spec.account_env.is_zero() {
                return Err(Error::PluginAccountWithoutHome(spec.account.clone()));
            }
            return Ok(Vec::new());
        };
        let dir = format!("CLAUDE_CONFIG_DIR={}", home(&spec.account));
        Ok([dir].into_iter().chain(spec.account_env.environ()).collect())
    }

    /// Starts a process on the session resume names, or on a new session of its own id when resume is empty.
    async fn start_on(&self, spec: &Spec, h: &Arc<dyn Handler>, resume: &str) -> Result<ClaudeSession, Error> {
        if self.hide.is_empty() {
            return Err(Error::NoHide);
        }
        let account_env = self.account_env(spec)?;
        let mut o = options(spec, &repository_root(&spec.cwd));
        o.hide.clone_from(&self.hide);
        o.read_only = self.read_only.iter().chain(&spec.read_only).cloned().collect();
        let attachments = tempfile::Builder::new()
            .prefix("remit-attachments-")
            .tempdir()
            .map_err(|e| Error::Prepare(format!("the session's attachment directory: {e}")))?;
        let attachment_path = attachments
            .path()
            .canonicalize()
            .map_err(|e| Error::Prepare(format!("the session's attachment path: {e}")))?;
        o.read_only.push(attachment_path.to_string_lossy().into_owned());
        let (servers, values) = by_reference(&spec.mcp);
        o.mcp_servers = mcp_config(servers);
        o.command.clone_from(&self.command);
        o.env.extend(self.env.iter().cloned().chain(account_env).chain(values));
        if resume.is_empty() {
            o.session_id = new_session_id();
        } else {
            o.resume = resume.to_string();
        }
        let id = if resume.is_empty() {
            o.session_id.clone()
        } else {
            resume.to_string()
        };
        let shared = Arc::new(Shared {
            id: id.clone(),
            h: Arc::clone(h),
            agents: Subagents::new(Arc::clone(h)),
            cwd: spec.cwd.clone(),
            now: Arc::clone(&self.now),
            state: Mutex::new(State::default()),
            conn: OnceLock::new(),
            window: Arc::new(AtomicU64::new(0)),
        });
        let conn = Arc::new(claudecode::Conn::start(&o, Arc::clone(&shared), self.stderr.clone()).await?);
        // Set before the first prompt is sent, so before any tool call the process could ask about.
        shared.conn.set(Arc::downgrade(&conn)).ok();
        let ended = conn.done();
        let watcher = Arc::clone(&shared);
        tokio::spawn(async move {
            let end = ended.await.map(Exit::Process);
            watcher.ended(end.clone());
            watcher.h.on_exit(end);
        });
        // A session signed in by its plugin's variables (ADR-0071) has no login to move onto another's: what signs it in
        // is its environment, which a running process does not change but for the one token below.
        let mover = match &self.home {
            Some(home) if spec.account_env.is_zero() => Some(Mover {
                command: self.command.clone(),
                home: Arc::clone(home),
                store: Arc::clone(&self.store),
                env: self.env.clone(),
            }),
            _ => None,
        };
        Ok(ClaudeSession {
            id,
            lost: false,
            conn,
            shared,
            attachments,
            mover,
            keeping: Mutex::new(None),
            processes: Arc::default(),
        })
    }
}

/// One Claude Code process and the session it runs. Its turns are prompts: a prompt sent is answered by the result that
/// names it (user_message_uuids). A message steered into a running turn is answered with it: the turn ends with the
/// result that names the last message sent into it. A turn the agent takes on its own (a background task's news) runs
/// with no prompt open, and its result settles nothing. Dropping it ends the process.
struct ClaudeSession {
    id: String,
    lost: bool,
    /// The process; the task that keeps a moved session's token fresh holds it only weakly, so dropping the session
    /// still ends it.
    conn: Arc<claudecode::Conn>,
    shared: Arc<Shared>,
    attachments: tempfile::TempDir,
    /// What moves the session onto another account's login; none for a session of a plugin's account.
    mover: Option<Mover>,
    /// The task keeping the token of the account the session last moved to fresh.
    keeping: Mutex<Option<tokio::task::AbortHandle>>,
    processes: Arc<Mutex<ProcessSampler>>,
}

/// What a session needs to take another account's login: how to run Claude Code on that account's config dir and where
/// its login is kept.
struct Mover {
    command: Vec<String>,
    home: Home,
    store: StoreOf,
    env: Vec<String>,
}

impl Mover {
    fn on(&self, account: &str) -> (claudecode::Auth, claudecode::Store) {
        let auth = claudecode::Auth {
            command: self.command.clone(),
            config_dir: (self.home)(account),
            env: self.env.clone(),
        };
        let store = (self.store)(&auth.config_dir);
        (auth, store)
    }
}

impl Drop for ClaudeSession {
    fn drop(&mut self) {
        self.stop_keeping(None);
    }
}

impl ClaudeSession {
    /// Ends the keeping of the last account's token, and starts next's in its place when given.
    fn stop_keeping(&self, next: Option<tokio::task::AbortHandle>) {
        let mut keeping = self.keeping.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(old) = std::mem::replace(&mut *keeping, next) {
            old.abort();
        }
    }
}

/// How long to wait before renewing a token with left of life: until it has `RENEW_LEFT` left, at once when it has less.
fn renew_in(left: Duration) -> Duration {
    left.saturating_sub(RENEW_LEFT)
}

/// Sets a fresh token of the account on the session's process each time the one it has nears its end, until the
/// process ends or the session moves again. A renewal that fails is tried again while the token it would replace holds;
/// once that expires, the session's next request fails at the provider with the account's sign-in error, which is the
/// failure the core already answers by moving the card.
async fn keep_fresh(
    conn: Weak<claudecode::Conn>,
    auth: claudecode::Auth,
    store: claudecode::Store,
    mut token: claudecode::AccessToken,
) {
    loop {
        tokio::time::sleep(renew_in(token.left())).await;
        let fresh = match auth.fresh_token(&store, RENEW_LEFT).await {
            Ok(t) => t,
            Err(_) => {
                tokio::time::sleep(RENEW_RETRY).await;
                continue;
            }
        };
        let Some(conn) = conn.upgrade() else { return };
        if conn.set_oauth_token(fresh.secret()).await.is_err() {
            return; // the process has ended
        }
        token = fresh;
    }
}

/// What the process's messages are read into: the handler, the handlers of the subagents, and what the session has heard
/// so far.
struct Shared {
    id: String,
    h: Arc<dyn Handler>,
    /// The handlers of the subagents, `h` the root.
    agents: Subagents,
    cwd: String,
    now: Clock,
    state: Mutex<State>,
    /// The process, to change its mode from its own hooks; weak, so the process the session drops ends.
    conn: OnceLock<Weak<claudecode::Conn>>,
    /// The context window Claude Code last counted the session against (`/context`), in tokens; zero before its first
    /// count. It is the window the session compacts in, which a turn's `modelUsage` (the model's largest) is not.
    window: Arc<AtomicU64>,
}

#[derive(Default)]
struct State {
    /// The calls the classifier refused in the turn, the one coming back to be asked (`refused`).
    refusals: Refusals,
    /// The prompt being answered; `None` when none is.
    turn: Option<Turn>,
    /// Numbers the turns, so a prompt that stopped waiting clears its own turn and no other.
    turns: u64,
    /// Permission requests waiting for an answer.
    asking: usize,
    /// The tool calls of the session and its subagents that have no result yet, by id, as last reported.
    tools: HashMap<String, ToolCall>,
    /// The subagents whose text streams as deltas: their whole messages repeat it.
    streamed: HashSet<String>,
    /// What the process said of itself: its background tasks.
    tasks: TaskLog,
    /// Whether the process said it is working.
    working: bool,
    /// The session was told to end what it is doing (`cancel`): it starts no tool call until it next goes to work or
    /// to rest, or takes a prompt.
    held: bool,
    /// The context the last top-level request used, in tokens.
    used_total: u64,
    /// The model of that request.
    model: String,
    /// When the limit Claude Code last said rejects requests resets; `None` while none does.
    resets_at: Option<Timestamp>,
}

struct Turn {
    serial: u64,
    /// The uuid of the last message sent into the turn: the result that names it ends it.
    last: String,
    cancelled: bool,
    /// The provider's failure an assistant message of the turn carried.
    failure: Option<Failure>,
    done: oneshot::Sender<Result<StopReason, Error>>,
}

impl Session for ClaudeSession {
    fn id(&self) -> &str {
        &self.id
    }

    fn context_lost(&self) -> bool {
        self.lost
    }

    fn prompt(&self, prompt: Prompt) -> BoxFuture<'_, Result<StopReason, Error>> {
        Box::pin(async move {
            let uuid = new_session_id();
            let (done, ended) = oneshot::channel();
            let serial = {
                let mut state = self.shared.state();
                if state.turn.is_some() {
                    return Err(Error::TurnRunning);
                }
                state.turns = state.turns.wrapping_add(1);
                state.held = false;
                let serial = state.turns;
                state.turn = Some(Turn {
                    serial,
                    last: uuid.clone(),
                    cancelled: false,
                    failure: None,
                    done,
                });
                serial
            };
            // Whatever way this future ends, its turn is no longer open.
            let _clear = ClearTurn {
                shared: &self.shared,
                serial,
            };
            let prompt = self.stage_attachments(prompt, &uuid).await?;
            let m = user_message(prompt, Some(uuid), None);
            self.conn.send(&m).await?;
            ended.await.unwrap_or(Err(Error::Ended(None)))
        })
    }

    /// Puts text into the running turn, which reads it between its steps (priority now); while a permission request waits
    /// for its answer, after it (later), since interrupting that request withdraws it. False when no turn runs.
    fn steer(&self, prompt: Prompt) -> BoxFuture<'_, Result<bool, Error>> {
        Box::pin(async move {
            let uuid = new_session_id();
            let active = self.shared.state().turn.as_ref().map(|turn| turn.serial);
            let Some(serial) = active else { return Ok(false) };
            let prompt = self.stage_attachments(prompt, &uuid).await?;
            let m = {
                let mut state = self.shared.state();
                let asking = state.asking;
                let Some(turn) = state.turn.as_mut().filter(|turn| turn.serial == serial) else {
                    return Ok(false);
                };
                let priority = if asking > 0 {
                    claudecode::Priority::Later
                } else {
                    claudecode::Priority::Now
                };
                turn.last.clone_from(&uuid);
                user_message(prompt, Some(uuid), Some(priority))
            };
            self.conn.send(&m).await?;
            Ok(true)
        })
    }

    /// Ends the running turn, or the work the agent took on its own; its prompt returns `StopReason::Cancelled`. Until
    /// the interrupt lands the agent starts no tool call (`Shared::tool_start`).
    fn cancel(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            {
                let mut state = self.shared.state();
                state.held = true;
                if let Some(turn) = state.turn.as_mut() {
                    turn.cancelled = true;
                }
            }
            Ok(self.conn.interrupt().await?)
        })
    }

    /// Ends the process; `on_exit` follows.
    fn close(&self) {
        self.stop_keeping(None);
        self.conn.close();
    }

    /// Takes a fresh token of account's login (renewing it first when it has little left) and sets it on the running
    /// process, whose next request to the model, inside the turn in progress too, goes out on it; then keeps it fresh.
    fn move_to(&self, account: String) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let Some(mover) = &self.mover else {
                return Err(Error::NotMovable);
            };
            let (auth, store) = mover.on(&account);
            let token = auth.fresh_token(&store, RENEW_LEFT).await?;
            self.conn.set_oauth_token(token.secret()).await?;
            let keeper = tokio::spawn(keep_fresh(Arc::downgrade(&self.conn), auth, store, token));
            self.stop_keeping(Some(keeper.abort_handle()));
            Ok(())
        })
    }

    /// Ends one task of the process; its notification follows, to `on_task`.
    fn stop_task(&self, task_id: String) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move { Ok(self.conn.stop_task(&task_id).await?) })
    }

    fn processes(&self) -> BoxFuture<'_, Result<Vec<wire::app::SessionProcess>, Error>> {
        let conn = Arc::clone(&self.conn);
        let sampler = Arc::clone(&self.processes);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || crate::processes::sample(&conn, &sampler))
                .await
                .map_err(|e| Error::TurnFailed(format!("sample the session's processes: {e}")))?
        })
    }

    fn kill_process(&self, kill_token: String) -> BoxFuture<'_, Result<(), Error>> {
        let conn = Arc::clone(&self.conn);
        let sampler = Arc::clone(&self.processes);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || crate::processes::kill(&conn, &sampler, &kill_token))
                .await
                .map_err(|e| Error::TurnFailed(format!("force kill the session's process: {e}")))?
        })
    }

    /// Counts what fills the context now.
    fn context(&self) -> BoxFuture<'_, Result<model::SessionContext, Error>> {
        Box::pin(async move { Ok(session_context(&self.id, self.conn.context_usage().await?)) })
    }

    /// Reads the end of what one background task of the process has written.
    fn task_output(&self, task_id: String) -> BoxFuture<'_, Result<TaskOutput, Error>> {
        Box::pin(async move {
            let out = self.conn.task_output(&task_id).await?;
            Ok(TaskOutput {
                text: out.output,
                total_bytes: out.total_bytes,
                truncated: out.truncated,
            })
        })
    }
}

/// Clears a prompt's turn when its prompt stops waiting.
struct ClearTurn<'a> {
    shared: &'a Shared,
    serial: u64,
}

impl Drop for ClearTurn<'_> {
    fn drop(&mut self) {
        let mut state = self.shared.state();
        if state.turn.as_ref().is_some_and(|t| t.serial == self.serial) {
            state.turn = None;
        }
    }
}

/// Lowers the count of permission requests waiting, however the request ends.
struct Asking<'a>(&'a Shared);

impl Drop for Asking<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state();
        state.asking = state.asking.saturating_sub(1);
    }
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Puts the process in mode; false when it has ended or refused.
    async fn set_mode(&self, mode: &str) -> bool {
        let Some(conn) = self.conn.get().and_then(Weak::upgrade) else {
            return false;
        };
        conn.set_permission_mode(mode).await.is_ok()
    }

    /// Fails the prompt still open: the process that would answer it has ended.
    fn ended(&self, err: Option<Exit>) {
        let Some(turn) = self.state().turn.take() else {
            return;
        };
        let end = match turn.failure {
            Some(failure) => Error::Failure(failure),
            None => Error::Ended(err),
        };
        // A turn nobody awaits any more has no one to tell.
        turn.done.send(Err(end)).ok();
    }

    /// Reads what Claude Code said of the account's limit (SDKRateLimitEvent) and tells the session's handler: whether it
    /// takes requests, which limit, how much of it is used, and when it resets, in seconds since the epoch as the
    /// provider's anthropic-ratelimit-unified-reset header says. While it rejects requests, its reset is what the turn's
    /// failure says.
    fn rate_limit(&self, raw: &str) {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Event {
            rate_limit_info: Info,
        }
        #[derive(Deserialize, Default)]
        #[serde(default, rename_all = "camelCase")]
        struct Info {
            status: String,
            resets_at: i64,
            rate_limit_type: String,
            utilization: Option<f64>,
        }
        let Ok(Event { rate_limit_info: i }) = serde_json::from_str::<Event>(raw) else {
            return;
        };
        let status = match i.status.as_str() {
            "allowed" => LimitStatus::Allowed,
            "allowed_warning" => LimitStatus::Warning,
            "rejected" => LimitStatus::Rejected,
            _ => return,
        };
        let resets_at = (i.resets_at > 0)
            .then(|| Timestamp::from_second(i.resets_at).ok())
            .flatten();
        self.state().resets_at = resets_at.filter(|_| status == LimitStatus::Rejected);
        self.h.on_limit(Limit {
            status,
            window: i.rate_limit_type,
            utilization: i.utilization,
            resets_at: resets_at.and_then(time_of),
        });
    }

    /// Passes on the text and thinking of an answer as it streams, to the handler of the agent that writes it.
    fn stream_event(&self, raw: &str) {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct StreamEvent {
            parent_tool_use_id: Option<String>,
            agent_id: String,
            event: Event,
        }
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Event {
            r#type: String,
            delta: Delta,
        }
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Delta {
            r#type: String,
            text: String,
            thinking: String,
        }
        let Ok(m) = serde_json::from_str::<StreamEvent>(raw) else {
            return;
        };
        if m.event.r#type != "content_block_delta" {
            return;
        }
        let Some(h) = self.writer(m.parent_tool_use_id.as_deref(), &m.agent_id) else {
            return;
        };
        if !m.agent_id.is_empty() {
            self.state().streamed.insert(m.agent_id);
        }
        match m.event.delta.r#type.as_str() {
            "text_delta" => h.on_update(Update::MessageChunk(m.event.delta.text)),
            "thinking_delta" => h.on_update(Update::ThoughtChunk(m.event.delta.thinking)),
            _ => {}
        }
    }

    /// The handler of the agent that wrote a message: the session's own agent when it names no parent tool call, else the
    /// subagent `agent_id` names; `None` when that subagent has no handler, or a subagent's message names none (Claude
    /// Code forwards a subagent's messages with their `agent_id` only).
    fn writer(&self, parent: Option<&str>, agent_id: &str) -> Option<Arc<dyn Handler>> {
        if parent.is_none() {
            return Some(Arc::clone(&self.h));
        }
        if agent_id.is_empty() {
            return None;
        }
        self.agents.handler(agent_id)
    }

    /// Reads a complete assistant message: the tools it calls (its text and thinking streamed already), the context its
    /// request used, and the error the provider answered with. A subagent's goes to its handler (`subagent`).
    fn assistant(&self, raw: &str) {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Assistant {
            parent_tool_use_id: Option<String>,
            agent_id: String,
            error: String,
            message: ApiMessage,
        }
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct ApiMessage {
            model: String,
            content: Vec<ContentBlock>,
            usage: Option<TokenUsage>,
        }
        let Ok(m) = serde_json::from_str::<Assistant>(raw) else {
            return;
        };
        if m.parent_tool_use_id.is_some() {
            self.subagent(&m.agent_id, m.message.content);
            return;
        }
        if let Some(category) = claude_failure(&m.error) {
            let mut state = self.state();
            let resets = state.resets_at;
            if let Some(turn) = state.turn.as_mut() {
                let text = text_of_blocks(&m.message.content);
                let title = if text.is_empty() { m.error.clone() } else { text };
                let resets_at = resets.or_else(|| resets_at(&title, (self.now)()));
                turn.failure = Some(Failure {
                    category,
                    title,
                    resets_at: resets_at.and_then(time_of),
                });
            }
        }
        if let Some(usage) = &m.message.usage {
            let mut state = self.state();
            state.used_total = usage.total();
            state.model = m.message.model;
        }
        self.tool_calls(&self.h, m.message.content);
    }

    /// Reads a complete message of the subagent `agent_id` into its handler: its text and thinking too, which it does
    /// not stream unless its deltas came (`streamed`).
    fn subagent(&self, agent_id: &str, content: Vec<ContentBlock>) {
        if agent_id.is_empty() {
            return;
        }
        let Some(h) = self.agents.handler(agent_id) else {
            return;
        };
        if !self.state().streamed.contains(agent_id) {
            for b in &content {
                match b.r#type.as_str() {
                    "text" if !b.text.is_empty() => h.on_update(Update::MessageChunk(b.text.clone())),
                    "thinking" if !b.thinking.is_empty() => h.on_update(Update::ThoughtChunk(b.thinking.clone())),
                    _ => {}
                }
            }
        }
        self.tool_calls(&h, content);
    }

    /// Reports the tool calls of a message to `h`: a TodoWrite as the plan.
    fn tool_calls(&self, h: &Arc<dyn Handler>, content: Vec<ContentBlock>) {
        for b in content {
            if b.r#type != "tool_use" {
                continue;
            }
            if b.name == "TodoWrite" {
                h.on_update(Update::Plan(plan_of(&b.input)));
                continue;
            }
            let (kind, title) = describe(&b.name, &b.input, &self.cwd);
            self.report(
                h,
                ToolCall {
                    id: b.id,
                    name: b.name,
                    kind,
                    title,
                    status: ToolStatus::InProgress,
                    input: b.input,
                    output: String::new(),
                },
            );
        }
    }

    /// Reads the results of tool calls from a user message, into the handler of the agent that made them; a Workflow
    /// call's answer also says how to resume the run it launched.
    fn tool_results(&self, raw: &str) {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct User {
            parent_tool_use_id: Option<String>,
            agent_id: String,
            tool_use_result: Value,
            message: UserContent,
        }
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct UserContent {
            content: Value,
        }
        let Ok(m) = serde_json::from_str::<User>(raw) else {
            return;
        };
        if let Ok(launch) = WorkflowLaunch::deserialize(&m.tool_use_result)
            && launch.task_type == TASK_WORKFLOW
        {
            let tasks = self.state().tasks.launched(&launch);
            self.tasks(tasks);
        }
        let Some(h) = self.writer(m.parent_tool_use_id.as_deref(), &m.agent_id) else {
            return;
        };
        // A plain text message is an echo, with nothing to read.
        let Ok(blocks) = serde_json::from_value::<Vec<ContentBlock>>(m.message.content) else {
            return;
        };
        for b in blocks {
            if b.r#type != "tool_result" {
                continue;
            }
            // A call ends with its result: it is reported as it ended and no longer kept. A TodoWrite's, or one of a call
            // not seen, is not reported.
            let Some(mut call) = self.state().tools.remove(&b.tool_use_id) else {
                continue;
            };
            call.output = b.result_text();
            call.status = if b.is_error {
                ToolStatus::Failed
            } else {
                ToolStatus::Completed
            };
            h.on_update(Update::ToolCall(call));
        }
    }

    fn report(&self, h: &Arc<dyn Handler>, call: ToolCall) {
        self.state().tools.insert(call.id.clone(), call.clone());
        h.on_update(Update::ToolCall(call));
    }

    /// Ends the turn it answers, and reports the context the session uses.
    fn result(&self, raw: &str) {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct ResultMessage {
            is_error: bool,
            errors: Vec<String>,
            result: String,
            #[serde(rename = "modelUsage")]
            model_usage: BTreeMap<String, ModelUsage>,
            user_message_uuids: Vec<String>,
        }
        #[derive(Deserialize, Default)]
        #[serde(default, rename_all = "camelCase")]
        struct ModelUsage {
            context_window: u64,
        }
        let Ok(m) = serde_json::from_str::<ResultMessage>(raw) else {
            return;
        };
        // A message handler does not wait on the process: the mode goes back on a task of its own.
        if let Some(mode) = self.state().refusals.end_turn()
            && let Some(conn) = self.conn.get().and_then(Weak::upgrade)
        {
            tokio::spawn(async move {
                // A process that ended has no mode left to set.
                conn.set_permission_mode(&mode).await.ok();
            });
        }
        let (usage, turn) = {
            let mut state = self.state();
            // Claude Code's count once it counted; before, the window of the model the last request used.
            let mut size = self.window.load(Ordering::Relaxed);
            if size == 0 {
                for (name, u) in &m.model_usage {
                    if size == 0 || name.starts_with(&state.model) {
                        size = u.context_window;
                    }
                }
            }
            let usage = Usage {
                used: state.used_total,
                size,
            };
            let answered = state
                .turn
                .as_ref()
                .is_some_and(|t| answers(&m.user_message_uuids, &t.last));
            (usage, if answered { state.turn.take() } else { None })
        };
        // A result that asked the model nothing (a session not found) has no usage to show.
        if !m.model_usage.is_empty() {
            self.h.on_update(Update::Usage(usage));
        }
        let Some(turn) = turn else { return };
        let end = if turn.cancelled {
            Ok(StopReason::Cancelled)
        } else if let Some(failure) = turn.failure {
            Err(Error::Failure(failure))
        } else if m.is_error {
            Err(Error::TurnFailed(
                m.errors.into_iter().chain([m.result]).collect::<Vec<_>>().join(": "),
            ))
        } else {
            Ok(StopReason::EndTurn)
        };
        // A turn nobody awaits any more has no one to tell.
        turn.done.send(end).ok();
    }

    /// Reads what the process says of the session: what it loaded, whether it works, its background tasks, and what
    /// Claude Code does to it beside the agent's work (a retried request, a compaction).
    fn system(&self, subtype: &str, raw: &str) {
        if let Some(text) = system_note(subtype, raw) {
            self.h.on_update(Update::Note(text));
            return;
        }
        let Ok(msg) = serde_json::from_str::<SdkMessage>(raw) else {
            return;
        };
        match subtype {
            "init" => self.h.on_init(msg.init(&self.id)),
            "session_state_changed" => {
                let working = msg.state != "idle";
                let changed = {
                    let mut state = self.state();
                    let changed = working != state.working;
                    state.working = working;
                    if changed {
                        // the cancelled work has ended, or new work began after it
                        state.held = false;
                    }
                    changed
                };
                if changed {
                    self.h.on_working(working);
                    if !working {
                        self.read_context();
                    }
                }
            }
            _ => {
                let tasks = self.state().tasks.read(&msg);
                self.tasks(tasks);
            }
        }
    }

    /// Reads what fills the context, now that the session rests, and reports it; on a task of its own, as a message
    /// handler does not wait on the process. A process that ended, or a Claude Code that cannot say, reports nothing.
    fn read_context(&self) {
        let Some(conn) = self.conn.get().and_then(Weak::upgrade) else {
            return;
        };
        let (id, h, window) = (self.id.clone(), Arc::clone(&self.h), Arc::clone(&self.window));
        tokio::spawn(async move {
            // the one read before stays the newest
            if let Ok(u) = conn.context_usage().await {
                window.store(u.max_tokens, Ordering::Relaxed);
                h.on_update(Update::Usage(Usage {
                    used: u.total_tokens,
                    size: u.max_tokens,
                }));
                h.on_context(session_context(&id, u));
            }
        });
    }

    /// Reports each task to the handler of the agent whose task it is; a subagent's spawner owns it.
    fn tasks(&self, tasks: Vec<OwnedTask>) {
        for t in tasks {
            if t.task.kind == TASK_SUBAGENT {
                self.agents.owns(&t.owner, &t.task.id);
            }
            if let Some(h) = self.agents.handler(&t.owner) {
                h.on_task(t.task);
            }
        }
    }
}

impl claudecode::Handler for Shared {
    /// Reads one message of the process into what the handler hears.
    fn message(&self, m: claudecode::Message) {
        let raw = m.raw.get();
        match m.kind {
            claudecode::Kind::StreamEvent => self.stream_event(raw),
            claudecode::Kind::Assistant => self.assistant(raw),
            claudecode::Kind::User => self.tool_results(raw),
            claudecode::Kind::Result => self.result(raw),
            claudecode::Kind::System => self.system(&m.subtype, raw),
            claudecode::Kind::RateLimitEvent => self.rate_limit(raw),
            claudecode::Kind::Unknown(_) => {}
        }
    }

    /// Hands the request, as the tool call it asks for, to the handler of the agent that asks (`Subagents::asker`). The
    /// first request after a refusal puts the session back in its mode; the refused call come back says why it asks.
    async fn can_use_tool(&self, req: &claudecode::PermissionRequest) -> claudecode::Permission {
        let (seen, closed) = {
            let mut state = self.state();
            state.asking = state.asking.saturating_add(1);
            let closed = state.refusals.close(&req.tool_name, &req.input);
            (state.tools.get(&req.tool_use_id).cloned(), closed)
        };
        let _asking = Asking(self);
        let mut classifier = String::new();
        if let Some(closed) = closed {
            classifier = closed.classifier.unwrap_or_default();
            // Back in its mode before the answer, so the calls after this one meet the classifier again; a process that
            // ended reads no answer either.
            self.set_mode(&closed.mode).await;
        }
        let tool = seen.unwrap_or_else(|| {
            let (kind, title) = describe(&req.tool_name, &req.input, &self.cwd);
            ToolCall {
                id: req.tool_use_id.clone(),
                name: req.tool_name.clone(),
                kind,
                title,
                status: ToolStatus::Pending,
                input: req.input.clone(),
                output: String::new(),
            }
        });
        let asked = PermissionRequest {
            tool,
            reason: req.decision_reason.clone(),
            classifier,
            agent_id: req.agent_id.clone(),
        };
        let asker = self.agents.asker(&req.agent_id);
        match asker.on_permission(asked).await {
            PermissionAnswer::Allow => claudecode::Permission::Allow,
            PermissionAnswer::Deny { message } => claudecode::Permission::Deny { message },
        }
    }

    /// Answers the hooks the session registered (options.rs's `hooks`): the rails Remit holds mid-turn, a refused call
    /// brought back to be asked, and what the owner follows the agent by.
    async fn hook(&self, input: claudehooks::HookInput) -> claudehooks::HookOutput {
        let agent_id = input.common.agent_id.unwrap_or_default();
        match input.event {
            Event::PreToolUse { tool_name, .. } => self.tool_start(&agent_id, &tool_name),
            Event::PermissionDenied {
                tool_name,
                tool_input,
                reason,
                ..
            } => {
                self.refused(&tool_name, &tool_input, &reason, input.common.permission_mode)
                    .await
            }
            Event::ConfigChange { source, file_path } => self.config_change(&agent_id, source, file_path),
            Event::InstructionsLoaded {
                file_path,
                memory_type,
                load_reason,
                ..
            } => {
                let note = Note {
                    kind: NoteKind::InstructionsRead,
                    title: display_path(&file_path, &self.cwd),
                    text: instructions_read(memory_type, load_reason),
                };
                self.note(&agent_id, note);
                HookOutput::default()
            }
            Event::PostModelSwitch(switch) if switch.source == ModelSwitchSource::Auto => {
                let text = format!(
                    "Claude Code switched the model from {} to {} on its own.",
                    switch.from_model, switch.to_model
                );
                self.note(&agent_id, Note::says(NoteKind::ModelSwitched, text));
                HookOutput::default()
            }
            Event::Stop { stop_hook_active, .. } => self.stopping(stop_hook_active).await,
            Event::PostToolUse {
                tool_name, tool_input, ..
            } => {
                let Value::Object(input) = tool_input else {
                    return HookOutput::default();
                };
                let (kind, _) = describe(&tool_name, &input, &self.cwd);
                if !matches!(
                    kind,
                    ToolKind::Read | ToolKind::Edit | ToolKind::Search | ToolKind::Execute
                ) {
                    return HookOutput::default();
                }
                let told = self
                    .moment(&agent_id, Moment::ToolUsed { tool: tool_name, input })
                    .await;
                HookOutput {
                    hook_specific_output: Some(SpecificOutput::PostToolUse {
                        additional_context: told,
                        classifier_context: None,
                        updated_tool_output: None,
                        updated_mcp_tool_output: None,
                    }),
                    ..HookOutput::default()
                }
            }
            Event::SubagentStart {} => HookOutput {
                hook_specific_output: Some(SpecificOutput::SubagentStart {
                    additional_context: self.moment(&agent_id, Moment::Started).await,
                }),
                ..HookOutput::default()
            },
            _ => HookOutput::default(),
        }
    }
}

impl Shared {
    /// What the handler of agent_id (empty: the session's own agent) adds to its context at moment m, waited for
    /// `MOMENT_WAIT` at most; none from a subagent whose spawner gave no handler.
    async fn moment(&self, agent_id: &str, m: Moment) -> Option<String> {
        let h = self.agents.handler(agent_id)?;
        tokio::time::timeout(MOMENT_WAIT, h.on_moment(m))
            .await
            .ok()
            .flatten()
            .filter(|t| !t.is_empty())
    }

    /// Tells the handler of agent_id (empty: the session's own agent) a note of kind with text; a subagent whose spawner
    /// gave no handler has none, and its note is dropped as what it writes is.
    fn note(&self, agent_id: &str, note: Note) {
        if let Some(h) = self.agents.handler(agent_id) {
            h.on_update(Update::Note(note));
        }
    }

    /// Lets agent_id start a call of tool, or refuses it while the session is being cancelled (a pause, a stop, the
    /// owner's interrupt): in every permission mode, so the agent stops mid-turn even before the interrupt lands. The
    /// refusal reaches the model as the call's error, and the owner as a note.
    fn tool_start(&self, agent_id: &str, tool: &str) -> HookOutput {
        if !self.state().held {
            return HookOutput::default();
        }
        self.note(
            agent_id,
            Note::says(NoteKind::Refused, format!("Refused a call of {tool}: {HELD}")),
        );
        HookOutput {
            hook_specific_output: Some(SpecificOutput::PreToolUse {
                permission_decision: Some(PermissionDecision::Deny),
                permission_decision_reason: Some(HELD.to_owned()),
                updated_input: None,
                additional_context: None,
            }),
            ..HookOutput::default()
        }
    }

    /// Keeps a change of the settings files out of the running session: what a session runs with is set when it starts.
    /// A skill installed while it runs still loads.
    fn config_change(&self, agent_id: &str, source: ConfigSource, file: Option<String>) -> HookOutput {
        if source == ConfigSource::Skills {
            return HookOutput::default();
        }
        let file = file.unwrap_or_else(|| format!("{source:?}"));
        let text =
            format!("Kept a change to {file} out of the running session: its settings change only between sessions.");
        self.note(agent_id, Note::says(NoteKind::Refused, text));
        HookOutput {
            decision: Some(BlockDecision::Block),
            reason: Some(SETTINGS_FIXED.to_owned()),
            ..HookOutput::default()
        }
    }

    /// The session's own agent is about to end its turn: what its handler says it must do first keeps it going, in the
    /// same turn. Once only: a stop that follows a kept-going one ends the turn.
    async fn stopping(&self, kept_going: bool) -> HookOutput {
        if kept_going {
            return HookOutput::default();
        }
        let Some(h) = self.agents.handler("") else {
            return HookOutput::default();
        };
        match h.on_stop().await {
            Some(reason) => HookOutput {
                decision: Some(BlockDecision::Block),
                reason: Some(reason),
                ..HookOutput::default()
            },
            None => HookOutput::default(),
        }
    }

    /// A call the auto-mode classifier refused (`PermissionDenied`): the session leaves auto mode and the model is told it
    /// may retry, so the call comes back as a permission request (`refused`). The mode is set before the answer, so the
    /// retry cannot reach the classifier again.
    async fn refused(&self, tool_name: &str, tool_input: &Value, reason: &str, mode: Option<String>) -> HookOutput {
        let no_retry = HookOutput::default();
        let Some(mode) = mode else {
            return no_retry;
        };
        if !self.state().refusals.open(tool_name, tool_input, reason, &mode) {
            return no_retry;
        }
        if !self.set_mode(ASKING_MODE).await {
            self.state().refusals.abandon();
            return no_retry;
        }
        HookOutput {
            hook_specific_output: Some(SpecificOutput::PermissionDenied { retry: Some(true) }),
            ..HookOutput::default()
        }
    }
}

/// Whose instructions file a session read and when, in words: "Project instructions, at the session's start".
fn instructions_read(whose: MemoryType, why: LoadReason) -> String {
    let whose = match whose {
        MemoryType::User => "The user's instructions",
        MemoryType::Project => "Project instructions",
        MemoryType::Local => "Local instructions",
        MemoryType::Managed => "Managed instructions",
        MemoryType::Other => "Instructions",
    };
    let when = match why {
        LoadReason::SessionStart => "at the session's start",
        LoadReason::NestedTraversal => "on entering its directory",
        LoadReason::PathGlobMatch => "for a file its paths match",
        LoadReason::Include => "included by another",
        LoadReason::Compact => "again after a compaction",
        LoadReason::Other => return whose.to_owned(),
    };
    format!("{whose}, {when}")
}

/// Why a tool call is refused while the session is being cancelled, as the model and the owner read it.
const HELD: &str = "Remit is ending this turn: no more tool calls start in it. Stop here.";

/// Why a settings change mid-session is kept out, as the session hears it.
const SETTINGS_FIXED: &str =
    "Remit starts each session with its settings; a change to them applies from the next session.";

/// What a system message of subtype says of the session beside the agent's work, as a note: a request to the model
/// retried (api_retry), the conversation compacted (compact_boundary) or not (a status whose compaction failed). `None`
/// for any other message, and for one that does not read as its subtype.
fn system_note(subtype: &str, raw: &str) -> Option<Note> {
    #[derive(Deserialize)]
    struct Retry {
        attempt: u32,
        max_retries: u32,
        retry_delay_ms: u64,
        error_status: Option<u16>,
        error: String,
    }
    #[derive(Deserialize)]
    struct Compaction {
        compact_metadata: CompactMetadata,
    }
    #[derive(Deserialize)]
    struct CompactMetadata {
        trigger: String,
        pre_tokens: u64,
        post_tokens: Option<u64>,
    }
    #[derive(Deserialize)]
    struct Status {
        compact_result: Option<String>,
        #[serde(default)]
        compact_error: String,
    }
    let (kind, text) = match subtype {
        "api_retry" => {
            let r: Retry = serde_json::from_str(raw).ok()?;
            let status = r.error_status.map(|s| format!(", HTTP {s}")).unwrap_or_default();
            let wait = r.retry_delay_ms.div_ceil(1000);
            (
                NoteKind::Retrying,
                format!(
                    "The request to the model failed ({}{status}); Claude Code tries again in {wait} s, attempt {} of {}.",
                    r.error, r.attempt, r.max_retries
                ),
            )
        }
        "compact_boundary" => {
            let m = serde_json::from_str::<Compaction>(raw).ok()?.compact_metadata;
            let after = m.post_tokens.map(|t| format!(" to {}k", t / 1000)).unwrap_or_default();
            (
                NoteKind::Compacted,
                format!(
                    "Claude Code compacted the conversation ({}): {}k tokens{after}.",
                    m.trigger,
                    m.pre_tokens / 1000
                ),
            )
        }
        "status" => {
            let st: Status = serde_json::from_str(raw).ok()?;
            if st.compact_result.as_deref() != Some("failed") {
                return None;
            }
            (
                NoteKind::CompactFailed,
                format!("Claude Code could not compact the conversation: {}", st.compact_error),
            )
        }
        _ => return None,
    };
    Some(Note::says(kind, text))
}

/// Says whether a result naming uuids answers the message last; a result that names none (an older Claude Code) answers
/// whatever is open.
fn answers(uuids: &[String], last: &str) -> bool {
    uuids.is_empty() || uuids.iter().any(|u| u == last)
}

/// A block of a message's content, in the API's fields.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ContentBlock {
    r#type: String,
    text: String,
    thinking: String,
    id: String,
    name: String,
    input: Map<String, Value>,
    tool_use_id: String,
    is_error: bool,
    content: Value,
}

impl ContentBlock {
    /// The text a tool_result block holds: a string, or its text blocks.
    fn result_text(&self) -> String {
        match &self.content {
            Value::String(s) => s.clone(),
            v @ Value::Array(_) => Vec::<ContentBlock>::deserialize(v)
                .map(|blocks| text_of_blocks(&blocks))
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
}

fn text_of_blocks(blocks: &[ContentBlock]) -> String {
    let texts: Vec<&str> = blocks
        .iter()
        .filter(|b| b.r#type == "text" && !b.text.is_empty())
        .map(|b| b.text.as_str())
        .collect();
    texts.join("\n")
}

/// The usage of one request.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TokenUsage {
    input_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
    output_tokens: u64,
}

impl TokenUsage {
    /// How much of the context the request filled.
    fn total(&self) -> u64 {
        [
            self.cache_creation_input_tokens,
            self.cache_read_input_tokens,
            self.output_tokens,
        ]
        .into_iter()
        .fold(self.input_tokens, u64::saturating_add)
    }
}

fn new_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl ClaudeSession {
    /// Makes the uploaded bytes available to the running session as read-only files.
    async fn stage_attachments(&self, mut prompt: Prompt, uuid: &str) -> Result<Prompt, Error> {
        if prompt.files.is_empty() {
            return Ok(prompt);
        }
        let directory = self.attachments.path().join(uuid);
        let stage_error = |e: std::io::Error| Error::TurnFailed(format!("stage message attachments: {e}"));
        tokio::fs::create_dir(&directory).await.map_err(stage_error)?;
        let mut paths = Vec::new();
        for (index, file) in std::mem::take(&mut prompt.files).into_iter().enumerate() {
            let name = file
                .name
                .chars()
                .map(|c| {
                    if matches!(c, '/' | '\\') || c.is_control() {
                        '_'
                    } else {
                        c
                    }
                })
                .collect::<String>();
            let name = &name[..name.floor_char_boundary(230)];
            let path = directory.join(format!("{index}-{name}"));
            let bytes = STANDARD
                .decode(file.data)
                .map_err(|e| Error::TurnFailed(format!("decode message attachment: {e}")))?;
            tokio::fs::write(&path, bytes).await.map_err(stage_error)?;
            paths.push(path.to_string_lossy().into_owned());
        }
        prompt.text.push_str("\n\nUploaded files (read-only paths): ");
        prompt.text.push_str(&serde_json::json!(paths).to_string());
        Ok(prompt)
    }
}

/// What the agent reads of prompt, as Claude Code takes a message: its words, then its images.
fn user_message(
    prompt: Prompt,
    uuid: Option<String>,
    priority: Option<claudecode::Priority>,
) -> claudecode::UserMessage {
    claudecode::UserMessage {
        text: prompt.text,
        images: prompt
            .images
            .into_iter()
            .map(|i| claudecode::UserImage {
                media_type: i.media_type.as_str().to_owned(),
                data: i.data,
            })
            .collect(),
        uuid,
        priority,
    }
}
