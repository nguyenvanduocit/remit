//! The runtime's way in: a session on `/link` whose hello presents this start's key is admitted to `wire::runtime`'s
//! `to_core` calls and nothing else of the core, and serves them until it closes.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use link::{ErrorCode, Event, Failure, Refusal, RefusalCode};
use wire::Ack;
use wire::runtime::{
    Acked, Hello, HelloReply, Moment, PermissionRequest, RUNTIME_CONTRACT, SessionAck, SessionEntry, Stopping, Told,
    to_core,
};

use crate::convert::Ending;
use crate::launcher::{Launcher, Runtime, State};
use crate::port::{Connection, Link, Peer};
use crate::session::Session;
use crate::{BUSY_REFUSAL, lock};

impl<L: Link> Launcher<L> {
    /// Serves one session of a runtime, whose `hello` the caller took off its events: it comes in only with this start's
    /// key, compared in constant time, and is granted `to_core` alone. Returns once the session has closed.
    pub async fn serve(&self, hello: link::Hello, peer: Peer<L>) {
        let Peer { link, mut events } = peer;
        if !self.inner.key.admits(&hello.credential) {
            tracing::warn!(node = %hello.node.id, "a runtime with another key was turned away");
            link.refuse(Refusal {
                code: RefusalCode::BadCredential,
                reason: "bad key".to_owned(),
            });
            return;
        }
        link.admit(vec![to_core::CAPABILITY.to_owned()]);
        let n = {
            let mut shared = self.shared();
            shared.next_conn = shared.next_conn.wrapping_add(1);
            shared.next_conn
        };
        let calls = Arc::new(Calls {
            l: self.clone(),
            conn: Connection { n, link: link.clone() },
            rt: Mutex::new(None),
        });
        let mut reason = "the session ended".to_owned();
        while let Some(event) = events.recv().await {
            match event {
                Event::Call { id, method, body, .. } => {
                    let calls = Arc::clone(&calls);
                    let link = link.clone();
                    // An action runs to its end whatever happens to the session that asked for it.
                    tokio::spawn(async move {
                        let answer = to_core::serve(calls.as_ref(), &method, &body).await;
                        link.reply(id, answer);
                    });
                }
                Event::Closed(closing) => {
                    reason = format!("{closing:?}");
                    break;
                }
                _ => {}
            }
        }
        if let Some(rt) = calls.runtime() {
            tracing::info!(runtime = %rt.id, reason = %reason, "the AI runtime disconnected");
            self.disconnected(&rt, n, &reason);
        }
    }
}

/// One session of a runtime: what it calls on the core. Until its hello it knows no runtime and serves nothing.
struct Calls<L> {
    l: Launcher<L>,
    conn: Connection<L>,
    /// Set by the hello.
    rt: Mutex<Option<Arc<Runtime<L>>>>,
}

fn not_greeted() -> Failure {
    Failure::new(ErrorCode::Failed, "the AI runtime has not said hello on this session")
}

impl<L: Link> Calls<L> {
    fn runtime(&self) -> Option<Arc<Runtime<L>>> {
        lock(&self.rt).clone()
    }

    /// The session `session_id` of the runtime this connection said hello as; none when the core does not hold it.
    fn session(&self, session_id: &str) -> Result<Option<Arc<Session<L>>>, Failure> {
        let rt = self.runtime().ok_or_else(not_greeted)?;
        Ok(rt.held().sessions.get(session_id).cloned())
    }
}

impl<L: Link> to_core::Handler for Calls<L> {
    /// Takes the runtime that says hello as the one sessions run in, unless another is connected. The same runtime back
    /// after a dropped session keeps its sessions, and is told what of each arrived; a new one ends the sessions of the
    /// one before it.
    async fn runtime_hello(&self, hello: Hello) -> Result<HelloReply, Failure> {
        let l = &self.l;
        if hello.contract != RUNTIME_CONTRACT {
            let refused = format!(
                "the AI runtime speaks contract {} and the core {RUNTIME_CONTRACT}: restart Remit",
                hello.contract
            );
            l.refused(&refused);
            return Ok(HelloReply {
                refused,
                sessions: Vec::new(),
            });
        }
        let now = l.now();
        let (before, acks, lost) = {
            let mut shared = l.shared();
            if let Some(cur) = &shared.rt
                && cur.id != hello.runtime_id
                && cur.held().conn.is_some()
            {
                return Ok(HelloReply {
                    refused: format!("{BUSY_REFUSAL} ({})", cur.id),
                    sessions: Vec::new(),
                });
            }
            let mut before = None;
            let cur = match shared.rt.clone() {
                Some(cur) if cur.id == hello.runtime_id => cur,
                was => {
                    before = was;
                    let cur = Arc::new(Runtime::new(hello.runtime_id.clone()));
                    shared.rt = Some(Arc::clone(&cur));
                    shared.hellos = shared.hellos.wrapping_add(1);
                    cur
                }
            };
            *lock(&self.rt) = Some(Arc::clone(&cur));
            let listed: HashSet<&str> = hello.sessions.iter().map(|h| h.session_id.as_str()).collect();
            let (mut acks, mut lost) = (Vec::new(), Vec::new());
            {
                let mut held = cur.held();
                held.conn = Some(self.conn.clone());
                for (id, s) in &held.sessions {
                    if listed.contains(id.as_str()) {
                        acks.push(SessionAck {
                            session_id: id.clone(),
                            acked: s.acked(),
                        });
                    } else if s.is_opened() {
                        lost.push(Arc::clone(s));
                    }
                }
            }
            shared.state = State {
                connected: true,
                runtime_id: hello.runtime_id.clone(),
                revision: hello.revision.clone(),
                reason: String::new(),
                since: now,
                restarts: shared.state.restarts,
            };
            (before, acks, lost)
        };
        l.bump();
        tracing::info!(runtime = %hello.runtime_id, revision = %hello.revision, sessions = hello.sessions.len(), "the AI runtime said hello");
        if let Some(before) = before {
            let why = format!("the AI runtime {} gave way to {}", before.id, hello.runtime_id);
            l.end(&before, &Ending::Restarted(why));
        }
        for s in lost {
            s.finish(Ending::Unheld);
        }
        Ok(HelloReply {
            refused: String::new(),
            sessions: acks,
        })
    }

    /// Takes a session's entries and answers at once with the last one received; the session hands them to its handler
    /// on its own. Entries of a session the core does not hold (its start gave up, or it ended) are dropped, and
    /// acknowledged so they are not sent again.
    async fn session_events(&self, session_id: String, entries: Vec<SessionEntry>) -> Result<Acked, Failure> {
        let acked = match self.session(&session_id)? {
            Some(s) => s.receive(entries),
            None => entries.iter().map(|e| e.seq).max().unwrap_or_default(),
        };
        Ok(Acked { acked })
    }

    /// Asks the handler of the session's own agent what it must do before its turn ends; nothing for a session the core
    /// does not hold.
    async fn session_stopping(&self, session_id: String) -> Result<Stopping, Failure> {
        let first = match self.session(&session_id)? {
            Some(s) => s.stopping().await.unwrap_or_default(),
            None => String::new(),
        };
        Ok(Stopping { first })
    }

    /// Asks the handler of the agent agent_id what it reads beside moment, once every entry up to after_seq is handed
    /// over; nothing for a session the core does not hold.
    async fn session_moment(
        &self,
        session_id: String,
        agent_id: String,
        after_seq: i64,
        moment: Moment,
    ) -> Result<Told, Failure> {
        let text = match self.session(&session_id)? {
            Some(s) => s.moment(&agent_id, after_seq, moment).await.unwrap_or_default(),
            None => String::new(),
        };
        Ok(Told { text })
    }

    /// Acknowledges at once; the session's handler decides on a task of its own.
    async fn permission_asked(
        &self,
        session_id: String,
        request_id: String,
        request: PermissionRequest,
        after_seq: i64,
    ) -> Result<Ack, Failure> {
        if let Some(s) = self.session(&session_id)? {
            s.ask(request_id, request, after_seq);
        }
        Ok(Ack {})
    }

    /// Ends the sign-in `login_id` the core started in this runtime; an empty error is signed in.
    async fn account_login_ended(&self, login_id: String, error: String) -> Result<Ack, Failure> {
        let login = self.runtime().and_then(|rt| rt.held().logins.get(&login_id).cloned());
        let Some(login) = login else {
            tracing::warn!(login = %login_id, "the AI runtime ended a sign-in the core does not hold");
            return Ok(Ack {});
        };
        login.end(if error.is_empty() { None } else { Some(error) });
        Ok(Ack {})
    }
}
