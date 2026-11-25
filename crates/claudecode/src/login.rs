use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{Notify, oneshot, watch};

use crate::{Auth, Error};

/// How to sign a Claude Code config dir in.
#[derive(Debug, Clone, Default)]
pub struct LoginOptions {
    /// The config dir the login is made for, and how to run claude on it.
    pub auth: Auth,
    /// How long to wait for the link; `None` is thirty seconds.
    pub link_wait: Option<Duration>,
    /// How long the login may stay open for its code; `None` is fifteen minutes.
    pub lifetime: Option<Duration>,
}

const LINK_WAIT: Duration = Duration::from_secs(30);
const LOGIN_LIFETIME: Duration = Duration::from_secs(15 * 60);

/// One `claude auth login` in progress: the process that makes a login in a config dir. It prints a link the owner opens,
/// where Anthropic signs him in and shows a code he gives back (`code`); when his browser can reach this machine the
/// process takes the answer itself and ends signed in. The process is the only holder of the login's secrets; the link
/// and the code pass through it and nothing here keeps them.
pub struct Login {
    url: String,
    stdin: tokio::sync::Mutex<ChildStdin>,
    cancel: Arc<Notify>,
    done: watch::Receiver<bool>,
    /// How the process ended; `None` when it signed in. Set before done turns true.
    err: Arc<Mutex<Option<String>>>,
}

/// Ends a login whose start was given up before it gave its link.
struct CancelOnDrop(Option<Arc<Notify>>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            cancel.notify_one();
        }
    }
}

/// Runs `claude auth login` for the config dir and returns once it has printed the link, which `url` gives. The process
/// lives for the options' lifetime or until it ends; dropping the future before it returns ends it.
pub async fn start_login(o: LoginOptions) -> Result<Login, Error> {
    let mut cmd = o.auth.command("login", &["--claudeai"])?;
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(Error::Start)?;
    let (Some(stdin), Some(stdout), Some(stderr)) = (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err(Error::Start(procgroup::Error::Io(std::io::Error::other(
            "claude has no stdio pipes",
        ))));
    };
    let cancel = Arc::new(Notify::new());
    let err = Arc::new(Mutex::new(None));
    let tail = Arc::new(LastLines::default());
    let (link_tx, link_rx) = oneshot::channel();
    let (done_tx, done) = watch::channel(false);
    let lifetime = o.lifetime.unwrap_or(LOGIN_LIFETIME);
    tokio::spawn({
        let (cancel, err, tail) = (Arc::clone(&cancel), Arc::clone(&err), Arc::clone(&tail));
        async move {
            let link = Mutex::new(Some(link_tx));
            let ended = tokio::select! {
                status = async {
                    tokio::join!(scan(stdout, &link, &tail), scan(stderr, &link, &tail));
                    child.wait().await
                } => match status {
                    Ok(s) if s.success() => None,
                    Ok(s) => Some(s.to_string()),
                    Err(e) => Some(e.to_string()),
                },
                () = tokio::time::sleep(lifetime) => Some("the sign-in stayed open past its lifetime".to_string()),
                () = cancel.notified() => Some("the sign-in was cancelled".to_string()),
            };
            // Dropping the child, if not yet reaped, ends its group.
            drop(child);
            *err.lock().unwrap_or_else(PoisonError::into_inner) =
                ended.map(|why| format!("claude auth login: {why}: {}", tail.joined()));
            done_tx.send(true).ok();
        }
    });
    let mut give_up = CancelOnDrop(Some(Arc::clone(&cancel)));
    let url = tokio::select! {
        link = link_rx => link.ok(),
        () = tokio::time::sleep(o.link_wait.unwrap_or(LINK_WAIT)) => return Err(Error::NoLink(tail.joined())),
    };
    let Some(url) = url else {
        // Both outputs ended with no link: the process is ending, and says why.
        let mut ended = done.clone();
        ended.wait_for(|d| *d).await.ok();
        let why = err.lock().unwrap_or_else(PoisonError::into_inner).clone();
        return Err(Error::LoginEnded(why.unwrap_or_else(|| tail.joined())));
    };
    give_up.0 = None;
    Ok(Login {
        url,
        stdin: tokio::sync::Mutex::new(stdin),
        cancel,
        done,
        err,
    })
}

/// Reads one output of the process a line at a time: the first https link goes to link, and the last lines are kept to
/// say why it ended.
async fn scan(r: impl AsyncRead + Unpin, link: &Mutex<Option<oneshot::Sender<String>>>, tail: &LastLines) {
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        tail.add(line);
        if let Some(at) = line.find("https://")
            && let Some(url) = line[at..].split_whitespace().next()
            && let Some(sender) = link.lock().unwrap_or_else(PoisonError::into_inner).take()
        {
            // The caller that stopped waiting for the link no longer needs it.
            sender.send(url.to_string()).ok();
        }
    }
}

impl Login {
    /// The link the owner opens to sign in.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Gives the process the code the sign-in page showed the owner, which it waits for when his browser cannot reach
    /// this machine. The code is single-use and leaves here for the process's input and nowhere else.
    pub async fn code(&self, code: &str) -> Result<(), Error> {
        let code = code.trim();
        if code.is_empty() || code.contains(['\r', '\n']) {
            return Err(Error::BadCode);
        }
        if *self.done.borrow() {
            return Err(Error::SignInEnded);
        }
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(format!("{code}\n").as_bytes())
            .await
            .map_err(Error::GiveCode)?;
        stdin.flush().await.map_err(Error::GiveCode)
    }

    /// Resolves when the process ended: signed in, or not (`err`).
    pub fn done(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut done = self.done.clone();
        async move {
            done.wait_for(|d| *d).await.ok();
        }
    }

    /// Why the login ended without signing in; `None` when it signed in or has not ended.
    pub fn err(&self) -> Option<String> {
        self.err.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Ends the process, signed in or not.
    pub fn cancel(&self) {
        self.cancel.notify_one();
    }
}

/// The last few lines a process wrote: what an error says of it.
#[derive(Default)]
struct LastLines {
    lines: Mutex<Vec<String>>,
}

impl LastLines {
    fn add(&self, line: &str) {
        let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        lines.push(line.to_string());
        let extra = lines.len().saturating_sub(4);
        lines.drain(..extra);
    }

    fn joined(&self) -> String {
        self.lines.lock().unwrap_or_else(PoisonError::into_inner).join(" | ")
    }
}
