//! The gate is the one place a call made to an outside system is checked and shaped (ADR-0018, invariant 3). A form of use
//! (the typed tool of `new_tool`, a program, a script, and Remit's own poll of a source) hands the gate a call and gets back
//! the answer or the error an agent may read. A form with a path around the gate does not exist: what a form adds is how the
//! act is carried out (`Resolved`), never whether it may be.

use std::future::Future;

use grant::{Decision, Effect, Refusal};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use telemetry::attr;
use tracing::Instrument as _;

use crate::api::CALL_TIMEOUT;
use crate::connection::{Resolved, Set};
use crate::error::{BoxError, BoxFuture, Error, Result, err, fail};
use crate::redact::redact_text;
use crate::source::Kind;
use crate::text::{OUTPUT_MAX, limit};
use crate::when::duration_text;

/// Who makes a call: the chain it belongs to and the agent in it. The channel that carries the call names it, because the
/// tool's own input is the agent's to write, and who is calling is not. The default Caller is no one, which a Policy has no
/// grant for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Caller {
    pub chain: String,
    pub agent: String,
}

/// How an act is expressed, and so how near the credential comes to the code the agent wrote (ADR-0018). The tier is how
/// near: T0 a typed tool, T1 a script behind a proxy, T2 a program whose environment holds the credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    /// T0: a typed tool; the credential stays in the backend, and the tool's code keeps it read-only.
    Tool,
    /// T1: a program behind an injecting proxy; the program holds no credential.
    Proxy,
    /// T2: a program whose environment holds the credential.
    Env,
}

impl Form {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Proxy => "proxy",
            Self::Env => "env",
        }
    }
}

impl std::fmt::Display for Form {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a call asks to do: act on one connection of one source with an effect, in a form. The form matters to a policy
/// because it decides how near the credential comes to the agent's code: a program holds the value in its environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub source: String,
    pub connection: String,
    pub effect: Effect,
    pub form: Form,
}

/// Decides whether a caller may take an action. The gate asks it for every call; the orchestrator implements it over the
/// grants a chain holds (`grant::decide`). A deny or an ask carries the reason an agent is told and the kind of refusal.
pub trait Policy: Send + Sync {
    fn decide<'a>(&'a self, who: &'a Caller, a: &'a Action) -> BoxFuture<'a, std::result::Result<Decision, BoxError>>;
}

/// How a call ended: ok it was answered, denied it was refused or waits for the owner, error it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Ok,
    Denied,
    Error,
}

impl Outcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Denied => "denied",
            Self::Error => "error",
        }
    }
}

/// What a Recorder is told about one call: who asked what of which connection, never a secret and never the tool's input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub caller: Caller,
    pub source: String,
    pub connection: String,
    pub tool: String,
    /// None when the call failed before the tool was found.
    pub effect: Option<Effect>,
    pub form: Form,
    /// What the input was without its content, when the tool has something to say: a script's language, size and hash, so
    /// that the record and the agent's tool event can be matched without the record holding the script.
    pub detail: String,
    pub outcome: Outcome,
    /// The kind of refusal when the call was turned away before it ran. A refusal nobody named is the platform's own.
    pub refusal: Option<Refusal>,
}

/// Keeps the account of calls (who asked what, of which connection). The gate tells it once per call, whatever the outcome.
/// It cannot refuse a call: a recorder that cannot store a record reports that itself.
pub trait Recorder: Send + Sync {
    fn record(&self, c: Call) -> BoxFuture<'_, ()>;
}

/// How a gate finds what answers a call.
#[derive(Clone)]
pub(crate) enum Resolution {
    /// A tool, a program, a script or a poll of this source, found at call time against the current store.
    Source(Kind),
    /// A lend of this source's connection to a plugin's own run, with the effect the run has.
    Lend(Kind, Effect),
    /// Nothing answers: the call is refused with this reason, and still recorded once.
    Refused(String),
}

/// The gate of the calls to one source: they all pass the same policy and recorder.
#[derive(Clone)]
pub(crate) struct Gate {
    pub source: String,
    pub how: Resolution,
    pub set: Set,
}

impl Gate {
    async fn resolve(&self, tool: &str, connection: &str) -> Result<Resolved> {
        match &self.how {
            Resolution::Source(k) => self.set.resolve_call(k, tool, connection).await,
            Resolution::Lend(k, effect) => self.set.resolve_lend(k, *effect, connection).await,
            Resolution::Refused(why) => Err(Error::msg(why.clone())),
        }
    }

    /// One call of a tool: the gate's steps (`pass`), and an answer with its secrets removed, headed by the connection's name
    /// and cut. `tool` is the name the agent called, `connection` the one it asked for (empty for the only configured one)
    /// and `input` the tool's arguments. A returned error is safe for an agent to read.
    pub(crate) async fn run(&self, caller: &Caller, tool: &str, connection: &str, input: Value) -> Result<String> {
        let tool_name = tool.to_owned();
        let args = input.clone();
        self.pass(caller, tool, connection, &input, move |res| async move {
            let Some(exec) = res.exec else {
                return fail!("{tool_name} is not a tool");
            };
            let out = exec(args).await?;
            Ok(limit(
                &redact_text(&format!("connection: {}\n{out}", res.label)),
                OUTPUT_MAX,
            ))
        })
        .await
    }

    /// One call through the gate, in this order: find the connection, ask the policy, carry out the act under a deadline;
    /// then tell the recorder, once, whatever the outcome. `act` is what the form answers (a tool's text, a poll's items), its
    /// secrets removed. A returned error is safe for an agent to read.
    pub(crate) async fn pass<T, F, Fut>(
        &self,
        caller: &Caller,
        tool: &str,
        connection: &str,
        input: &Value,
        act: F,
    ) -> Result<T>
    where
        F: FnOnce(Resolved) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut call = Call {
            caller: caller.clone(),
            source: self.source.clone(),
            connection: connection.to_owned(),
            tool: tool.to_owned(),
            effect: None,
            form: Form::Tool,
            detail: String::new(),
            outcome: Outcome::Error,
            refusal: None,
        };
        // The call's span names the chain that made it, so what it runs (a program, a script) is that chain's child.
        let span = tracing::info_span!(
            attr::SOURCE_SPAN,
            { attr::CHAIN_ID } = tracing::field::Empty,
            source = %self.source,
            tool,
        );
        if !caller.chain.is_empty() {
            span.record(attr::CHAIN_ID, caller.chain.as_str());
        }
        let answer = self.attempt(&mut call, input, act).instrument(span).await;
        call.outcome = match &answer {
            Ok(_) => Outcome::Ok,
            Err((outcome, _)) => *outcome,
        };
        if let Some(r) = &self.set.inner.recorder {
            r.record(call).await;
        }
        answer.map_err(|(_, e)| Error::msg(redact_text(&e.to_string())))
    }

    /// Fills `call` with what the resolution finds (the connection's name, the act's effect and form) and answers.
    async fn attempt<T, F, Fut>(
        &self,
        call: &mut Call,
        input: &Value,
        act: F,
    ) -> std::result::Result<T, (Outcome, Error)>
    where
        F: FnOnce(Resolved) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let res = self
            .resolve(&call.tool, &call.connection)
            .await
            .map_err(|e| (Outcome::Error, e))?;
        call.connection.clone_from(&res.connection);
        call.effect = Some(res.effect);
        call.form = res.form;
        if let Some(detail) = &res.detail {
            call.detail = detail(input);
        }
        if let Err((refusal, e)) = self.permit(call, res.effect).await {
            call.refusal = Some(refusal);
            return Err((Outcome::Denied, e));
        }
        match tokio::time::timeout(CALL_TIMEOUT, act(res)).await {
            Ok(Ok(out)) => Ok(out),
            Ok(Err(e)) => Err((Outcome::Error, e)),
            Err(_) => Err((
                Outcome::Error,
                err!(
                    "{}: the call did not finish within {}: narrow it",
                    call.tool,
                    duration_text(CALL_TIMEOUT)
                ),
            )),
        }
    }

    /// Ok when the policy allows the call. Anything but a clear allow refuses it, with the kind of refusal: an error, and with
    /// no policy every call but a read by a typed tool: a program holds the credential's value, so only a policy that names it
    /// may let one run.
    async fn permit(&self, call: &Call, effect: Effect) -> std::result::Result<(), (Refusal, Error)> {
        let where_ = format!("{}/{}", call.source, call.connection);
        let words = grant_words(call, effect);
        let Some(policy) = &self.set.inner.policy else {
            if effect == Effect::Read && call.form == Form::Tool {
                return Ok(());
            }
            return Err((
                Refusal::Rail,
                err!(
                    "{}: refused: the grant ({words}) is missing: this server has no policy that grants anything above a read by a typed tool",
                    call.tool
                ),
            ));
        };
        let action = Action {
            source: call.source.clone(),
            connection: call.connection.clone(),
            effect,
            form: call.form,
        };
        match policy.decide(&call.caller, &action).await {
            Err(e) => Err((
                Refusal::Rail,
                err!(
                    "{}: refused: the grant for {where_} could not be checked: {e}",
                    call.tool
                ),
            )),
            Ok(Decision::Allow) => Ok(()),
            Ok(Decision::Ask { reason, refusal }) => Err((
                refusal,
                err!(
                    "{}: the owner must approve {effect} on {where_} before this runs: ask the Chair to request the grant ({words}){}",
                    call.tool,
                    because(&reason)
                ),
            )),
            Ok(Decision::Deny { reason, refusal }) => Err((
                refusal,
                err!(
                    "{}: refused: the grant ({words}) is missing: ask the Chair to request it{}",
                    call.tool,
                    because(&reason)
                ),
            )),
        }
    }
}

/// The grant a call needs, named the way request_grant asks for it. A program that holds the credential names its form; a
/// typed tool and a script, which holds none, are covered by the read on the connection and name none.
fn grant_words(call: &Call, effect: Effect) -> String {
    let words = format!(
        "source {}, connection {}, effect {effect}",
        call.source, call.connection
    );
    if call.form == Form::Env {
        format!("{words}, form {}", call.form)
    } else {
        words
    }
}

fn because(reason: &str) -> String {
    if reason.is_empty() {
        String::new()
    } else {
        format!(": {reason}")
    }
}
