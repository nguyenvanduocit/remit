//! What the core calls on the runtime (`wire::runtime::to_runtime`). Each answers at once: what waits on Claude or a
//! person comes later as an entry or a call of the runtime's own.

use std::sync::Arc;

use link::{ErrorCode, Failure};
use model::Prompt;
use runtimelink::Link;
use wire::Ack;
use wire::runtime::{Email, PermissionAnswer, SessionSpec, SignIn, Steered, TaskOutput, TranscriptChunk, to_runtime};

use crate::accounts::Accounts;
use crate::convert;
use crate::runtime::Runtime;
use crate::session::Session;

pub(crate) struct Calls<L> {
    pub(crate) r: Arc<Runtime<L>>,
}

fn failed(why: impl std::fmt::Display) -> Failure {
    Failure::new(ErrorCode::Failed, why.to_string())
}

impl<L: Link> Calls<L> {
    fn session(&self, id: &str) -> Result<Arc<Session<L>>, Failure> {
        self.r
            .session(id)
            .ok_or_else(|| Failure::new(ErrorCode::NotFound, format!("this AI runtime holds no session {id}")))
    }

    fn accounts(&self) -> Result<&dyn Accounts, Failure> {
        self.r
            .cfg
            .accounts
            .as_deref()
            .ok_or_else(|| failed("this AI runtime holds no accounts"))
    }
}

impl<L: Link> to_runtime::Handler for Calls<L> {
    /// A stopped card has no live session to ask, so the transcript is keyed by `cwd` and Claude's session id.
    async fn session_transcript(
        &self,
        cwd: String,
        resume_id: String,
        offset: i64,
    ) -> Result<TranscriptChunk, Failure> {
        let dir = self
            .r
            .cfg
            .transcripts
            .as_ref()
            .ok_or_else(|| failed("this AI runtime is not told where Claude Code writes its transcripts"))?;
        dir.part(&cwd, &resume_id, offset).await.map_err(failed)
    }

    async fn account_email(&self, account: String) -> Result<Email, Failure> {
        Ok(match self.accounts()?.email(&account).await {
            Ok(email) => Email {
                email,
                refused: String::new(),
            },
            Err(e) => Email {
                email: String::new(),
                refused: e.to_string(),
            },
        })
    }

    async fn session_processes(&self, session_id: String) -> Result<Vec<wire::app::SessionProcess>, Failure> {
        let Some(session) = self.r.session(&session_id).and_then(|s| s.live().ok()) else {
            return Ok(Vec::new());
        };
        session.processes().await.map_err(failed)
    }

    async fn session_context(&self, session_id: String) -> Result<model::SessionContext, Failure> {
        self.session(&session_id)?.context().await.map_err(failed)
    }

    async fn session_task_output(&self, session_id: String, task_id: String) -> Result<TaskOutput, Failure> {
        self.session(&session_id)?.task_output(task_id).await.map_err(failed)
    }

    /// Starts opening the session the core asks for, once, and answers at once: how the open ended is the session's
    /// opened entry. The same session id asked again (its answer lost) opens nothing more. The open runs under the
    /// runtime's life, not the call's, and never holds this call's queue. A spec the runtime cannot take fails the
    /// call: nothing is held for it and nothing starts.
    async fn session_open(&self, spec: SessionSpec) -> Result<Ack, Failure> {
        let id = spec.session_id.clone();
        let spec = convert::agent_spec(spec).map_err(|e| Failure::new(ErrorCode::InvalidArgument, e))?;
        let (s, new) = self.r.hold(&id);
        if new {
            tokio::spawn(Arc::clone(&s).send());
            tokio::spawn(s.open(spec));
        }
        Ok(Ack {})
    }

    async fn session_prompt(&self, session_id: String, prompt_id: String, prompt: Prompt) -> Result<Ack, Failure> {
        self.session(&session_id)?.prompt(prompt_id, prompt).map_err(failed)?;
        Ok(Ack {})
    }

    async fn session_steer(&self, session_id: String, steer_id: String, prompt: Prompt) -> Result<Steered, Failure> {
        self.session(&session_id)?.steer(steer_id, prompt).await.map_err(failed)
    }

    async fn session_cancel(&self, session_id: String) -> Result<Ack, Failure> {
        self.session(&session_id)?.cancel().map_err(failed)?;
        Ok(Ack {})
    }

    /// Answers once the session's process has taken the ask; the task's end follows as its task entry.
    async fn session_stop_task(&self, session_id: String, task_id: String) -> Result<Ack, Failure> {
        self.session(&session_id)?.stop_task(task_id).await.map_err(failed)?;
        Ok(Ack {})
    }

    async fn session_move(&self, session_id: String, account: String) -> Result<Ack, Failure> {
        self.session(&session_id)?.move_to(account).await.map_err(failed)?;
        Ok(Ack {})
    }

    /// Starts closing the session and answers at once: its exit entry follows once its process group has exited, and
    /// a session still opening gives its open up. A session the runtime no longer holds is gone already.
    async fn session_kill(&self, session_id: String) -> Result<Ack, Failure> {
        if let Some(s) = self.r.session(&session_id) {
            s.close();
        }
        Ok(Ack {})
    }

    async fn permission_answered(
        &self,
        session_id: String,
        request_id: String,
        answer: PermissionAnswer,
    ) -> Result<Ack, Failure> {
        if let Some(s) = self.r.session(&session_id) {
            s.answer(&request_id, answer);
        }
        Ok(Ack {})
    }

    async fn account_prepare(&self, account: String) -> Result<Ack, Failure> {
        self.accounts()?.prepare(&account).await.map_err(failed)?;
        Ok(Ack {})
    }

    /// Starts the sign-in `login_id`, once, and answers with its link; its end is reported by `account_login_ended`.
    async fn account_sign_in(&self, login_id: String, account: String) -> Result<SignIn, Failure> {
        let accounts = self.accounts()?;
        let start = async { accounts.sign_in(&account).await.map(Arc::from) };
        let login = self.r.sign_in(&login_id, start).await.map_err(failed)?;
        Ok(SignIn {
            url: login.url().to_owned(),
        })
    }

    async fn account_login_code(&self, login_id: String, code: String) -> Result<Ack, Failure> {
        let login = self
            .r
            .login(&login_id)
            .ok_or_else(|| failed("no sign-in is waiting for a code: start it again"))?;
        login.code(&code).await.map_err(failed)?;
        Ok(Ack {})
    }

    /// Abandons the sign-in `login_id`; one that ended already is left as it ended.
    async fn account_login_cancel(&self, login_id: String) -> Result<Ack, Failure> {
        if let Some(login) = self.r.login(&login_id) {
            login.cancel();
        }
        Ok(Ack {})
    }

    async fn account_sign_out(&self, account: String) -> Result<Ack, Failure> {
        self.accounts()?.discard(&account).await.map_err(failed)?;
        Ok(Ack {})
    }
}
