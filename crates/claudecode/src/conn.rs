use std::collections::HashMap;
use std::future::Future;
use std::io::{self, Write};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{Notify, mpsc, oneshot, watch};

use claudehooks::{HookInput, HookOutput};

use crate::Error;
use crate::options::{Options, Request};

/// Where the process's standard error, and the lines of its output that are not JSON, are written.
pub type Stderr = Arc<Mutex<dyn Write + Send>>;

/// What kind of message the process wrote, by its `type`. A kind this crate does not know is kept, with its name, and
/// passes as any other: Claude Code adds kinds between versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    System,
    Assistant,
    User,
    StreamEvent,
    Result,
    RateLimitEvent,
    Unknown(String),
}

impl Kind {
    fn of(name: &str) -> Kind {
        match name {
            "system" => Kind::System,
            "assistant" => Kind::Assistant,
            "user" => Kind::User,
            "stream_event" => Kind::StreamEvent,
            "result" => Kind::Result,
            "rate_limit_event" => Kind::RateLimitEvent,
            other => Kind::Unknown(other.to_string()),
        }
    }
}

/// One message the process wrote, other than the control channel's: its kind and subtype, and the message whole, as
/// written, for the reader to decode what it needs of it.
#[derive(Debug)]
pub struct Message {
    pub kind: Kind,
    pub subtype: String,
    pub raw: Box<RawValue>,
}

/// The process asking whether it may use a tool (`can_use_tool`), in the protocol's own fields.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub input: Map<String, Value>,
    pub tool_use_id: String,
    pub agent_id: String,
    pub blocked_path: String,
    pub decision_reason: String,
    pub title: String,
    pub display_name: String,
    pub description: String,
}

/// The answer to a `PermissionRequest`: allowed, or denied with a message the model reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    Allow,
    Deny { message: String },
}

/// Receives what the process sends. `message` is called from one task, in the order the messages were written, and never
/// holds up the control channel: a slow `message` delays the messages after it, not an interrupt. `can_use_tool` and
/// `hook` run on a task of their own for each request, once every message written before the request has been handed
/// to `message` (a subagent's request comes after the task that spawned it); the future is dropped when the process
/// withdraws the request (`control_cancel_request`) or ends.
pub trait Handler: Send + Sync + 'static {
    fn message(&self, m: Message);
    fn can_use_tool(&self, req: &PermissionRequest) -> impl Future<Output = Permission> + Send;
    /// Answers a hook of an event the process was started with (`Options::hooks`).
    fn hook(&self, input: HookInput) -> impl Future<Output = HookOutput> + Send;
}

/// A hook the process runs through this client (`hook_callback`): its input, in Claude Code's hook fields.
#[derive(Deserialize)]
struct HookCallback {
    input: HookInput,
}

/// Where a user message goes when a turn is running: into the running turn, after it, or when the turn ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Now,
    Next,
    Later,
}

/// The end of what a background shell or a Monitor has written so far, as Claude Code keeps it (`get_task_output`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TaskOutput {
    /// Its last 8 KiB.
    pub output: String,
    /// All it wrote.
    pub total_bytes: u64,
    /// `output` starts partway through.
    pub truncated: bool,
}

/// A message to the model. `uuid`, when set, comes back on the result of the turn that answers it
/// (`user_message_uuids`). `priority` `None` is Claude Code's default.
#[derive(Debug, Clone, Default)]
pub struct UserMessage {
    pub text: String,
    /// Read after the text, in this order.
    pub images: Vec<UserImage>,
    pub uuid: Option<String>,
    pub priority: Option<Priority>,
}

/// An image in a message: its MIME type (`image/png`, `image/jpeg`, `image/gif`, `image/webp`) and its bytes, base64.
#[derive(Debug, Clone, Default)]
pub struct UserImage {
    pub media_type: String,
    pub data: String,
}

/// How a process ended other than cleanly.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExitError {
    #[error("claude exited: {0}")]
    Status(ExitStatus),
    /// Close ended it, with every process it started.
    #[error("claude was stopped")]
    Stopped,
    #[error("wait for claude: {0}")]
    Wait(Arc<io::Error>),
}

/// One running Claude Code process. Dropping it ends the process and every process it started, as `close` does.
pub struct Conn {
    shared: Arc<Shared>,
    done: watch::Receiver<bool>,
    init_response: Box<RawValue>,
}

struct Shared {
    stdin: tokio::sync::Mutex<ChildStdin>,
    next_id: AtomicU64,
    stop: Notify,
    state: Mutex<State>,
}

type Reply = Result<Box<RawValue>, String>;

#[derive(Default)]
struct State {
    /// Control requests sent, by request id: dropping a sender fails its request as closed.
    pending: HashMap<String, oneshot::Sender<Reply>>,
    /// Permission requests and hooks being answered, by request id: dropping a sender withdraws its request.
    asked: HashMap<String, oneshot::Sender<()>>,
    /// The child PID is valid until the wait can reap it.
    pid: Option<u32>,
    /// Stdout reached its end and the process was reaped.
    read_done: bool,
    /// How the process ended, once it has.
    ended: Option<Result<(), ExitError>>,
    /// The errors of the last result that was an error.
    failed: Vec<String>,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn write(&self, v: &impl Serialize) -> Result<(), Error> {
        let mut line = serde_json::to_vec(v)?;
        line.push(b'\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(&line).await.map_err(Error::Write)?;
        stdin.flush().await.map_err(Error::Write)
    }
}

/// A frame this client writes, by its `type`.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Outgoing<'a> {
    User {
        message: UserContent<'a>,
        parent_tool_use_id: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        uuid: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority: Option<Priority>,
    },
    ControlRequest {
        request_id: &'a str,
        request: &'a Request,
    },
    ControlResponse {
        response: Response<'a>,
    },
    /// Not a control request: Claude Code answers it with a `control_response` of its `request_id` all the same.
    UpdateEnvironmentVariables {
        request_id: &'a str,
        variables: OAuthToken<'a>,
    },
}

/// The one variable of `update_environment_variables` this client sets. Claude Code takes it only in a fixed set
/// (`CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_SESSION_ACCESS_TOKEN`, `CLAUDE_CODE_SESSIONS_OAUTH_TOKEN`) and refuses the
/// rest, so the frame carries only it.
#[derive(Serialize)]
struct OAuthToken<'a> {
    #[serde(rename = "CLAUDE_CODE_OAUTH_TOKEN")]
    token: &'a str,
}

#[derive(Serialize)]
struct UserContent<'a> {
    role: &'static str,
    content: Vec<Block<'a>>,
}

/// A content block of a user message, as the Messages API names it.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block<'a> {
    Text { text: &'a str },
    Image { source: ImageSource<'a> },
}

#[derive(Serialize)]
struct ImageSource<'a> {
    r#type: &'static str,
    media_type: &'a str,
    data: &'a str,
}

#[derive(Serialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
enum Response<'a> {
    Success { request_id: &'a str, response: Value },
    Error { request_id: &'a str, error: String },
}

#[derive(Serialize)]
#[serde(tag = "behavior", rename_all = "snake_case")]
enum PermissionResult<'a> {
    Allow {
        #[serde(rename = "updatedInput")]
        updated_input: &'a Map<String, Value>,
        #[serde(rename = "toolUseID")]
        tool_use_id: &'a str,
    },
    Deny {
        message: &'a str,
        #[serde(rename = "toolUseID")]
        tool_use_id: &'a str,
    },
}

/// What `deliver` hands over next: a message, or the go-ahead of a permission request whose messages before it are
/// handed over.
enum Queued {
    Message(Message),
    Ask(oneshot::Sender<()>),
}

/// What is read of every line: enough to route it.
#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    request_id: String,
    request: Option<Box<RawValue>>,
    response: Option<ControlResponse>,
}

#[derive(Deserialize)]
struct ControlResponse {
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    request_id: String,
    response: Option<Box<RawValue>>,
    #[serde(default)]
    error: String,
}

/// What a result says of its errors.
#[derive(Deserialize)]
struct ResultErrors {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    errors: Vec<String>,
}

impl Conn {
    /// Starts a process with o, hands what it sends to h, and initializes it. The process ends with `close`, when the
    /// `Conn` is dropped, or on its own, and `done` says when. Dropping the future before it returns ends the process.
    pub async fn start<H: Handler>(o: &Options, h: Arc<H>, stderr: Option<Stderr>) -> Result<Conn, Error> {
        let (program, own_args) = o.command.split_first().ok_or(Error::NoCommand)?;
        let args: Vec<String> = own_args.iter().cloned().chain(o.args()?).collect();
        let mut cmd = procgroup::command(program, &args);
        if !o.cwd.is_empty() {
            cmd.current_dir(&o.cwd);
        }
        let extra = [
            "CLAUDE_CODE_ENTRYPOINT=sdk-rust".to_string(),
            "CLAUDE_CODE_EMIT_SESSION_STATE_EVENTS=1".to_string(),
        ];
        crate::set_env(&mut cmd, extra.iter().chain(&o.env));
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        cmd.stderr(if stderr.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        let mut child = cmd.spawn().map_err(Error::Start)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(Error::Start(procgroup::Error::Io(io::Error::other(
                "claude has no stdio pipes",
            ))));
        };
        if let (Some(pipe), Some(log)) = (child.stderr.take(), stderr.clone()) {
            tokio::spawn(copy_stderr(pipe, log));
        }
        let shared = Arc::new(Shared {
            stdin: tokio::sync::Mutex::new(stdin),
            next_id: AtomicU64::new(0),
            stop: Notify::new(),
            state: Mutex::new(State {
                pid: child.id(),
                ..State::default()
            }),
        });
        let (done_tx, done) = watch::channel(false);
        let (queue, received) = mpsc::unbounded_channel();
        tokio::spawn(deliver(Arc::clone(&h), received, done_tx));
        tokio::spawn(run(Arc::clone(&shared), h, child, stdout, queue, stderr));
        let mut conn = Conn {
            shared,
            done,
            init_response: RawValue::NULL.to_owned(),
        };
        match conn.request(&o.initialize()).await {
            Ok(resp) => {
                let taken =
                    serde_json::from_str(resp.get()).map_err(|e| Error::Initialize(Box::new(Error::Answer(e))))?;
                if let Some(why) = o.unheld_hooks(&taken) {
                    // dropping conn ends the process
                    return Err(Error::Initialize(Box::new(Error::Unguarded(why))));
                }
                conn.init_response = resp;
                Ok(conn)
            }
            Err(Error::Closed { .. } | Error::Write(_)) => {
                // It ended before it answered, and may have said why in a result first (a session to resume that it
                // does not have): that result is the error.
                conn.done().await;
                let said = std::mem::take(&mut conn.shared.state().failed);
                Err(Error::Initialize(Box::new(Error::Closed { said })))
            }
            Err(e) => Err(Error::Initialize(Box::new(e))),
        }
    }

    /// The PID of the OS child Remit started, valid until it begins to be reaped.
    pub fn child_pid(&self) -> Option<u32> {
        self.shared.state().pid
    }

    /// What the process answered the initialize request with.
    pub fn init_response(&self) -> &RawValue {
        &self.init_response
    }

    /// Resolves once the process has ended and every message it wrote has been handed over, to how it ended: `None` for
    /// a clean exit. It does not borrow the `Conn`, so it can be awaited on a task of its own while the `Conn` lives on.
    pub fn done(&self) -> impl Future<Output = Option<ExitError>> + Send + 'static {
        let mut done = self.done.clone();
        let shared = Arc::clone(&self.shared);
        async move {
            // The sender is dropped only after it sent true.
            done.wait_for(|ended| *ended).await.ok();
            shared.state().ended.clone().and_then(Result::err)
        }
    }

    /// How the process ended, once `done` resolved: `None` for a clean exit or a process still running.
    pub fn err(&self) -> Option<ExitError> {
        self.shared.state().ended.clone().and_then(Result::err)
    }

    /// Ends the process and every process it started.
    pub fn close(&self) {
        self.shared.stop.notify_one();
    }

    /// Writes a message to the model.
    pub async fn send(&self, m: &UserMessage) -> Result<(), Error> {
        let frame = Outgoing::User {
            message: UserContent {
                role: "user",
                content: std::iter::once(Block::Text { text: &m.text })
                    .chain(m.images.iter().map(|i| Block::Image {
                        source: ImageSource {
                            r#type: "base64",
                            media_type: &i.media_type,
                            data: &i.data,
                        },
                    }))
                    .collect(),
            },
            parent_tool_use_id: None,
            uuid: m.uuid.as_deref(),
            priority: m.priority,
        };
        self.shared.write(&frame).await
    }

    /// Ends the turn that is running.
    pub async fn interrupt(&self) -> Result<(), Error> {
        self.request(&Request::Interrupt).await.map(drop)
    }

    /// Ends one task the process runs (a subagent, a workflow run, a background shell); the turn that started it goes on,
    /// and the task's notification says it stopped.
    pub async fn stop_task(&self, task_id: &str) -> Result<(), Error> {
        let task_id = task_id.to_string();
        self.request(&Request::StopTask { task_id }).await.map(drop)
    }

    /// Reads the end of what one task of the process has written (a background shell, a Monitor), with no turn of the
    /// agent's. The process keeps it until it exits, after the task ended too.
    pub async fn task_output(&self, task_id: &str) -> Result<TaskOutput, Error> {
        let task_id = task_id.to_string();
        let raw = self.request(&Request::GetTaskOutput { task_id }).await?;
        serde_json::from_str(raw.get()).map_err(Error::Answer)
    }

    /// Reads what fills the session's context, as Claude Code counts it for `/context`.
    pub async fn context_usage(&self) -> Result<crate::ContextUsage, Error> {
        let raw = self.request(&Request::GetContextUsage).await?;
        serde_json::from_str(raw.get()).map_err(Error::Answer)
    }

    /// Changes the model of the next request.
    pub async fn set_model(&self, model: &str) -> Result<(), Error> {
        let model = model.to_string();
        self.request(&Request::SetModel { model }).await.map(drop)
    }

    /// Changes the permission mode.
    pub async fn set_permission_mode(&self, mode: &str) -> Result<(), Error> {
        let mode = mode.to_string();
        self.request(&Request::SetPermissionMode { mode }).await.map(drop)
    }

    /// Has the process reach the model with OAuth access token `token` from its next request on, the one in flight
    /// excepted: Claude Code puts it in its environment, where it comes before the login of its config dir, and drops the
    /// token it had cached. The process, its session and the turn in progress go on. The token carries no refresh token:
    /// once it expires, a request fails until a fresher one is set.
    pub async fn set_oauth_token(&self, token: &str) -> Result<(), Error> {
        let id = self.next_id();
        let frame = Outgoing::UpdateEnvironmentVariables {
            request_id: &id,
            variables: OAuthToken { token },
        };
        self.answered(&id, &frame).await.map(drop)
    }

    /// Sends a control request and waits for its response or the end of the process. Dropping the future forgets the
    /// request.
    async fn request(&self, req: &Request) -> Result<Box<RawValue>, Error> {
        let id = self.next_id();
        let frame = Outgoing::ControlRequest {
            request_id: &id,
            request: req,
        };
        self.answered(&id, &frame).await
    }

    fn next_id(&self) -> String {
        format!(
            "req_{}",
            self.shared.next_id.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
        )
    }

    /// Writes frame, whose `request_id` is id, and waits for the `control_response` of id or the end of the process.
    /// Dropping the future forgets it.
    async fn answered(&self, id: &str, frame: &Outgoing<'_>) -> Result<Box<RawValue>, Error> {
        let (reply, replied) = oneshot::channel();
        {
            let mut state = self.shared.state();
            if state.read_done {
                return Err(Error::closed());
            }
            state.pending.insert(id.to_string(), reply);
        }
        let _forget = Forget {
            shared: &self.shared,
            id,
        };
        self.shared.write(frame).await?;
        match replied.await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(refusal)) => Err(Error::Refused(refusal)),
            Err(_) => Err(Error::closed()),
        }
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.close();
    }
}

/// Removes a control request from the pending ones when its caller stops waiting.
struct Forget<'a> {
    shared: &'a Shared,
    id: &'a str,
}

impl Drop for Forget<'_> {
    fn drop(&mut self) {
        self.shared.state().pending.remove(self.id);
    }
}

/// Reads stdout to its end, then reaps the process: on its own, or once `close` asked, by ending its group. Then it fails
/// what still waits on the process, and drops the queue so `deliver` ends.
async fn run<H: Handler>(
    shared: Arc<Shared>,
    h: Arc<H>,
    mut child: procgroup::Child,
    stdout: impl AsyncRead + Unpin,
    queue: mpsc::UnboundedSender<Queued>,
    stderr: Option<Stderr>,
) {
    let reading = read(&shared, &h, stdout, &queue, stderr.as_ref());
    tokio::pin!(reading);
    let stopped = tokio::select! {
        () = &mut reading => false,
        () = shared.stop.notified() => true,
    };
    shared.state().pid = None;
    let ended = if stopped {
        // Dropping the child, not yet reaped, kills its whole group; its stdout then reaches its end.
        drop(child);
        reading.await;
        Err(ExitError::Stopped)
    } else {
        let ended = tokio::select! {
            status = child.wait() => match status {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(ExitError::Status(s)),
                Err(e) => Err(ExitError::Wait(Arc::new(e))),
            },
            () = shared.stop.notified() => Err(ExitError::Stopped),
        };
        drop(child);
        ended
    };
    let mut state = shared.state();
    state.ended = Some(ended);
    state.read_done = true;
    // The process that would read their answers has ended: dropping the senders fails the control requests and
    // withdraws the permission requests and hooks.
    state.pending.clear();
    state.asked.clear();
    // queue, a parameter, is dropped after reading on return: deliver ends only once the end is recorded.
}

/// Reads stdout to its end. Control traffic is answered here; every other message is queued for `deliver`, so a slow
/// handler never stops a control response from being read. A line that is not JSON (a warning a dependency prints) is
/// reported on stderr and skipped.
async fn read<H: Handler>(
    shared: &Arc<Shared>,
    h: &Arc<H>,
    stdout: impl AsyncRead + Unpin,
    queue: &mpsc::UnboundedSender<Queued>,
    stderr: Option<&Stderr>,
) {
    let mut r = BufReader::with_capacity(64 << 10, stdout);
    let mut line = Vec::new();
    loop {
        line.clear();
        match r.read_until(b'\n', &mut line).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        match serde_json::from_str::<Envelope>(text) {
            Ok(e) => route(shared, h, queue, e, text),
            Err(_) => log(
                stderr,
                format_args!("claude code wrote a line that is not JSON: {:.200}\n", text),
            ),
        }
    }
}

fn route<H: Handler>(shared: &Arc<Shared>, h: &Arc<H>, queue: &mpsc::UnboundedSender<Queued>, e: Envelope, line: &str) {
    match e.kind.as_str() {
        "control_response" => {
            let Some(r) = e.response else { return };
            let Some(reply) = shared.state().pending.remove(&r.request_id) else {
                return; // not ours, or a second response to one id
            };
            let outcome = if r.subtype == "error" {
                Err(r.error)
            } else {
                Ok(r.response.unwrap_or_else(|| RawValue::NULL.to_owned()))
            };
            // The caller that stopped waiting no longer needs the answer.
            reply.send(outcome).ok();
        }
        "control_request" => answer(shared, h, queue, e.request_id, e.request),
        "control_cancel_request" => {
            // Dropping the sender ends the handler's future.
            shared.state().asked.remove(&e.request_id);
        }
        "keep_alive" => {}
        kind => {
            let kind = Kind::of(kind);
            if kind == Kind::Result
                && let Ok(result) = serde_json::from_str::<ResultErrors>(line)
                && result.is_error
            {
                shared.state().failed = result.errors;
            }
            let Ok(raw) = RawValue::from_string(line.to_string()) else {
                return;
            };
            queue
                .send(Queued::Message(Message {
                    kind,
                    subtype: e.subtype,
                    raw,
                }))
                .ok();
        }
    }
}

/// Answers a control request from the process. A permission request or a hook is the handler's, asked on a task of its
/// own once every message written before it is handed over (`deliver` gives the go-ahead): a subagent's request comes after
/// the task that spawned it. Any other is one this client does not take (it registers no SDK MCP servers or dialogs), and
/// is refused.
fn answer<H: Handler>(
    shared: &Arc<Shared>,
    h: &Arc<H>,
    queue: &mpsc::UnboundedSender<Queued>,
    id: String,
    raw: Option<Box<RawValue>>,
) {
    #[derive(Deserialize)]
    struct Subtype {
        #[serde(default)]
        subtype: String,
    }
    let raw = raw.map(|r| r.get().to_string()).unwrap_or_default();
    let subtype = serde_json::from_str::<Subtype>(&raw)
        .map(|k| k.subtype)
        .unwrap_or_default();
    let h = Arc::clone(h);
    match subtype.as_str() {
        "can_use_tool" => match serde_json::from_str::<PermissionRequest>(&raw) {
            Ok(req) => ask(shared, queue, id, async move {
                let p = h.can_use_tool(&req).await;
                let result = match &p {
                    Permission::Allow => PermissionResult::Allow {
                        updated_input: &req.input,
                        tool_use_id: &req.tool_use_id,
                    },
                    Permission::Deny { message } => PermissionResult::Deny {
                        message,
                        tool_use_id: &req.tool_use_id,
                    },
                };
                serde_json::to_value(result)
            }),
            Err(e) => refuse(shared, id, e.to_string()),
        },
        "hook_callback" => match serde_json::from_str::<HookCallback>(&raw) {
            Ok(cb) => ask(shared, queue, id, async move {
                serde_json::to_value(h.hook(cb.input).await)
            }),
            Err(e) => refuse(shared, id, e.to_string()),
        },
        _ => refuse(shared, id, format!("unsupported control request {subtype:?}")),
    }
}

/// Answers request id with what answering resolves to, on a task of its own, once the messages before it are handed over;
/// the process withdrawing the request, or ending, drops answering.
fn ask(
    shared: &Arc<Shared>,
    queue: &mpsc::UnboundedSender<Queued>,
    id: String,
    answering: impl Future<Output = Result<Value, serde_json::Error>> + Send + 'static,
) {
    let (withdraw, withdrawn) = oneshot::channel();
    {
        let mut state = shared.state();
        if state.read_done {
            return;
        }
        state.asked.insert(id.clone(), withdraw);
    }
    let (go, gone_ahead) = oneshot::channel();
    // A queue whose deliver has ended drops the go-ahead, and the request is never asked.
    queue.send(Queued::Ask(go)).ok();
    let shared = Arc::clone(shared);
    tokio::spawn(async move {
        let asked = async {
            gone_ahead.await.ok()?;
            Some(answering.await)
        };
        let answered = tokio::select! {
            a = asked => a,
            _ = withdrawn => None,
        };
        shared.state().asked.remove(&id);
        // Withdrawn: the process no longer waits for an answer.
        match answered {
            None => {}
            Some(Ok(response)) => {
                let frame = Outgoing::ControlResponse {
                    response: Response::Success {
                        request_id: &id,
                        response,
                    },
                };
                // A process that ended reads nothing.
                shared.write(&frame).await.ok();
            }
            Some(Err(e)) => respond_error(&shared, &id, e.to_string()).await,
        }
    });
}

/// Refuses request id, on a task of its own.
fn refuse(shared: &Arc<Shared>, id: String, error: String) {
    let shared = Arc::clone(shared);
    tokio::spawn(async move { respond_error(&shared, &id, error).await });
}

async fn respond_error(shared: &Shared, id: &str, error: String) {
    let frame = Outgoing::ControlResponse {
        response: Response::Error { request_id: id, error },
    };
    // A process that ended reads nothing.
    shared.write(&frame).await.ok();
}

/// Hands queued messages to the handler, in order, and lets each permission request be asked once the messages before
/// it are handed over, until the queue is dropped and empty; then done.
async fn deliver<H: Handler>(h: Arc<H>, mut received: mpsc::UnboundedReceiver<Queued>, done: watch::Sender<bool>) {
    while let Some(q) = received.recv().await {
        match q {
            Queued::Message(m) => h.message(m),
            // A request withdrawn already has no one waiting for its go-ahead.
            Queued::Ask(go) => drop(go.send(())),
        }
    }
    // Nobody waits on a session no handle refers to.
    done.send(true).ok();
}

async fn copy_stderr(mut pipe: impl AsyncRead + Unpin, log: Stderr) {
    let mut buf = vec![0; 8 << 10];
    while let Ok(n) = pipe.read(&mut buf).await {
        if n == 0 {
            return;
        }
        let mut w = log.lock().unwrap_or_else(PoisonError::into_inner);
        // A log line; the session goes on without it.
        if let Some(chunk) = buf.get(..n) {
            w.write_all(chunk).ok();
        }
    }
}

fn log(stderr: Option<&Stderr>, line: std::fmt::Arguments<'_>) {
    if let Some(log) = stderr {
        let mut w = log.lock().unwrap_or_else(PoisonError::into_inner);
        // A log line; the session goes on without it.
        w.write_fmt(line).ok();
    }
}
