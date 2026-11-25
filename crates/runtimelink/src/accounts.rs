//! The accounts' config dirs and logins, which live in the AI runtime: what the orchestrator asks of them is asked over
//! the link. `sign_in` returns a `Login`, which the binary hands the orchestrator: the link knows no application layer.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use link::{ErrorCode, Failure};
use tokio_util::sync::CancellationToken;
use wire::runtime::to_runtime;

use crate::launcher::{CallError, Launcher, Runtime};
use crate::port::Link;
use crate::{Error, new_id};

/// How long a call a sign-in makes on the owner's behalf (his code, his cancel) may take.
const LOGIN_CALL_WAIT: Duration = Duration::from_secs(30);

pub struct Accounts<L> {
    pub l: Launcher<L>,
}

impl<L: Link> Accounts<L> {
    /// Makes the config dir of account `id` ready in the runtime.
    pub async fn prepare(&self, id: &str) -> Result<(), Error> {
        self.l
            .on_runtime(to_runtime::client::account_prepare(id.to_owned()))
            .await
            .map(|_| ())
            .map_err(Error::from)
    }

    /// Starts signing account `id` in, in the runtime, and returns once it gave the link. The login ends when the runtime
    /// says so (`account_login_ended`), or with the runtime.
    pub async fn sign_in(&self, id: &str) -> Result<Login<L>, Error> {
        let rt = self.l.connected().await;
        let state = Arc::new(LoginState {
            l: self.l.clone(),
            rt: Arc::clone(&rt),
            id: new_id(),
            url: OnceLock::new(),
            done: CancellationToken::new(),
            err: OnceLock::new(),
        });
        self.l.hold_login(&rt, &state)?;
        let login = Login(Arc::clone(&state));
        // Given up before the answer, the runtime may have started it with the reply lost: it is cancelled.
        let mut cancel = CancelOnDrop(Some(Arc::clone(&state)));
        let sign_in = to_runtime::client::account_sign_in(state.id.clone(), id.to_owned());
        match self.l.call(&rt, sign_in).await {
            Ok(link) => {
                cancel.0 = None;
                state.url.set(link.url).ok();
                Ok(login)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// The address account `id` is signed in as, as the runtime reads it.
    pub async fn email(&self, id: &str) -> Result<String, Error> {
        let email = self
            .l
            .on_runtime(to_runtime::client::account_email(id.to_owned()))
            .await?;
        if !email.refused.is_empty() {
            return Err(Error::Refused(email.refused));
        }
        Ok(email.email)
    }

    /// Signs account `id` out and removes its config dir, in the runtime.
    pub async fn discard(&self, id: &str) -> Result<(), Error> {
        self.l
            .on_runtime(to_runtime::client::account_sign_out(id.to_owned()))
            .await
            .map(|_| ())
            .map_err(Error::from)
    }
}

/// A sign-in in progress in the runtime, known there by the id the core minted. It holds no secret: the link and the
/// code pass through it to the runtime's login process.
pub struct Login<L>(Arc<LoginState<L>>);

pub(crate) struct LoginState<L> {
    l: Launcher<L>,
    rt: Arc<Runtime<L>>,
    id: String,
    url: OnceLock<String>,
    /// Cancelled when the sign-in has ended.
    done: CancellationToken,
    /// Why it ended without signing in; none when it signed in. Set before `done`.
    err: OnceLock<Option<String>>,
}

impl<L: Link> LoginState<L> {
    /// Ends the sign-in with `err` (none: signed in), once.
    pub(crate) fn end(&self, err: Option<String>) {
        if self.err.set(err).is_ok() {
            self.done.cancel();
            self.l.drop_login(&self.rt, &self.id);
        }
    }

    async fn cancel(&self) {
        let cancel = self
            .l
            .call(&self.rt, to_runtime::client::account_login_cancel(self.id.clone()));
        let failed = match tokio::time::timeout(LOGIN_CALL_WAIT, cancel).await {
            Ok(Ok(_)) => return,
            Ok(Err(e)) => Error::from(e),
            Err(_) => timed_out(),
        };
        tracing::warn!(login = %self.id, error = %failed, "cancel a sign-in in the AI runtime");
    }
}

fn timed_out() -> Error {
    Error::Link(Failure::new(
        ErrorCode::Timeout,
        "the AI runtime did not answer in time",
    ))
}

impl<L: Link> Login<L> {
    /// The link the owner opens to sign in.
    pub fn url(&self) -> &str {
        self.0.url.get().map_or("", String::as_str)
    }

    /// Gives the sign-in the code the owner copied from its page.
    pub async fn code(&self, code: String) -> Result<(), Error> {
        let s = &self.0;
        let give =
            s.l.call(&s.rt, to_runtime::client::account_login_code(s.id.clone(), code));
        match tokio::time::timeout(LOGIN_CALL_WAIT, give).await {
            Ok(answer) => answer.map(|_| ()).map_err(Error::from),
            Err(_) => Err(timed_out()),
        }
    }

    /// Returns once the sign-in has ended.
    pub async fn done(&self) {
        self.0.done.cancelled().await;
    }

    /// Why the sign-in ended without signing in: none when it signed in, or has not ended yet.
    pub fn err(&self) -> Option<&str> {
        self.0.err.get().and_then(Option::as_deref)
    }

    /// Abandons the sign-in; `done` follows once the runtime has ended it.
    pub async fn cancel(&self) {
        self.0.cancel().await;
    }
}

struct CancelOnDrop<L: Link>(Option<Arc<LoginState<L>>>);

impl<L: Link> Drop for CancelOnDrop<L> {
    fn drop(&mut self) {
        if let Some(state) = self.0.take() {
            state.l.drop_login(&state.rt, &state.id);
            tokio::spawn(async move { state.cancel().await });
        }
    }
}

impl<L: Link> Launcher<L> {
    /// Makes `login` one of `rt`'s before the runtime is asked to start it, so its end finds it.
    fn hold_login(&self, rt: &Runtime<L>, login: &Arc<LoginState<L>>) -> Result<(), Error> {
        let _shared = self.shared();
        if rt.gone.is_cancelled() {
            return Err(CallError::Gone(rt.id.clone()).into());
        }
        rt.held().logins.insert(login.id.clone(), Arc::clone(login));
        Ok(())
    }

    fn drop_login(&self, rt: &Runtime<L>, id: &str) {
        rt.held().logins.remove(id);
    }
}
