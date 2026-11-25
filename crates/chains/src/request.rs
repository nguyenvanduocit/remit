//! The application layer: what a channel (the web app today, a command line or a chat tomorrow) may ask of Remit on the
//! owner's behalf is the public methods that take a Request, and what it may read is the public reads. A channel adapter
//! authenticates its user, turns its calls into these and pushes what the Notifier tells it; it decides nothing, so every
//! channel behaves the same.

use std::future::Future;
use std::sync::{Arc, PoisonError};

use model::{Actor, ActorKind, FailureDetail, ForeignJson, Time};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::chain_core::Core;
use crate::{Error, ErrorKind, Result};

/// One action a channel asks for: who asks, as the channel signed him in, and the id the channel gave the action. A
/// channel gives each action of its user an id of its own and sends that id again with every retry of it (an answer that
/// never came: a timeout, a lost connection), so that Remit does the action once and answers each retry as it answered
/// the first.
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub id: String,
    pub by: Actor,
}

/// The answer of an action that answers nothing but whether it was done: `{}` on the wire.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {}

/// How long an action is remembered: far past the longest any channel waits before asking again.
const REQUESTS_KEPT: time::Duration = time::Duration::days(7);

/// What answers again an action a restart cut off: whether it was done is not known, so it is not done twice.
pub const INTERRUPTED: &str =
    "the server restarted while this was being done, so it may not have been: look, and ask again if it was not";

/// An answer as recorded: the JSON the action returned, or its failure.
type Answer = std::result::Result<ForeignJson, Failure>;

/// A failure as a retry is answered with it: its words, its kind and its detail.
#[derive(Debug, Clone)]
struct Failure {
    kind: ErrorKind,
    why: String,
    detail: Option<Box<FailureDetail>>,
}

impl Failure {
    fn of(err: &Error) -> Self {
        Self {
            kind: err.kind(),
            why: err.to_string(),
            detail: err.detail().map(Box::new),
        }
    }

    fn failed(why: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Failed,
            why: why.into(),
            detail: None,
        }
    }
}

/// An action being done: the same request asked meanwhile waits for its answer.
pub struct Inflight {
    method: String,
    answer: watch::Sender<Option<Answer>>,
}

/// A service over the core: the core itself, each domain crate's service, the orchestrator (ADR-0096). An action runs
/// through `act` or `once` on whichever service it belongs to, and is handed that service.
pub trait Service: Send + Sync + Sized + 'static {
    /// The core the service runs on.
    fn core(&self) -> &Core;

    /// Does the action a channel asked for (method names it) once per request id, and answers every request with that
    /// id as it answered the first. Only the owner acts through a channel.
    fn act<T, F, Fut>(
        self: &Arc<Self>,
        req: Request,
        method: &'static str,
        act: F,
    ) -> impl Future<Output = Result<T>> + Send
    where
        F: FnOnce(Arc<Self>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: Serialize + DeserializeOwned + Send + 'static,
    {
        self.once(req, method, move |by| owners(by, method), act)
    }

    /// `act` for an action that answers nothing but whether it was done.
    fn act_done<F, Fut>(
        self: &Arc<Self>,
        req: Request,
        method: &'static str,
        act: F,
    ) -> impl Future<Output = Result<Done>> + Send
    where
        F: FnOnce(Arc<Self>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.act(req, method, move |o| async move { act(o).await.map(|()| Done {}) })
    }

    /// `act_done` for an action on chain chain_id, done in the order the calls on that chain were made: the call takes its
    /// place in the chain's line when it is made, before anything is awaited, so a resume sent right after a pause is done
    /// after it, whichever of the two would reach the chain's lock first. A call answered without being done (a retry of
    /// a request already done) gives its place up.
    fn act_on_chain<F, Fut>(
        self: &Arc<Self>,
        req: Request,
        method: &'static str,
        chain_id: String,
        act: F,
    ) -> impl Future<Output = Result<Done>> + Send + use<Self, F, Fut>
    where
        F: FnOnce(Arc<Self>, String) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let mut place = self.core().chain_order.join(&chain_id);
        let o = Arc::clone(self);
        async move {
            o.act_done(req, method, move |o| async move {
                place.turn().await;
                act(o, chain_id).await
            })
            .await
        }
    }

    /// `act` for an action whose asker is gated by allowed: it does the action once per request id and answers every
    /// request with that id as it answered the first: an id already done gets that answer, its error included; one being
    /// done waits for it. The action runs on the orchestrator's tracker to its end whatever happens to the channel (a
    /// closed tab, a lost network): dropping this future does not cut it short, and Drain waits for it. Once the server is
    /// draining nothing new starts.
    fn once<T, F, Fut>(
        self: &Arc<Self>,
        req: Request,
        method: &str,
        allowed: impl FnOnce(&Actor) -> Result<()>,
        act: F,
    ) -> impl Future<Output = Result<T>> + Send
    where
        F: FnOnce(Arc<Self>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: Serialize + DeserializeOwned + Send + 'static,
    {
        let gate = allowed(&req.by);
        let method = method.to_owned();
        let svc = Arc::clone(self);
        async move { once(svc, req, &method, gate, act).await }
    }
}

impl Service for Core {
    fn core(&self) -> &Core {
        self
    }
}

/// `Service::once`, with allowed's ruling on the asker already made.
async fn once<S, T, F, Fut>(svc: Arc<S>, req: Request, method: &str, allowed: Result<()>, act: F) -> Result<T>
where
    S: Service,
    F: FnOnce(Arc<S>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
    T: Serialize + DeserializeOwned + Send + 'static,
{
    if req.id.is_empty() {
        return Err(Error::refused(format!("{method}: a request needs an id")));
    }
    allowed?;
    // The id is claimed before the store is read, so a retry arriving meanwhile waits for this one.
    let f = match svc.core().claim(&req.id, method)? {
        Claim::Waits(mut answer) => {
            // the other asking does the action: what this one holds for it (its place in a chain's line) goes now,
            // or the two could wait on each other
            drop(act);
            let got = answer
                .wait_for(Option::is_some)
                .await
                .map_err(|_| Error::refused(INTERRUPTED))?
                .clone()
                .unwrap_or_else(|| Err(Failure::failed(INTERRUPTED)));
            return answer_of(got);
        }
        Claim::Mine(f) => Claimed {
            svc: Arc::clone(&svc),
            id: req.id.clone(),
            f,
            settled: false,
        },
    };
    let id = req.id.clone();
    let first = svc.core().store.read(move |r| r.request(&id)).await;
    match first {
        Ok(first) if first.method != method => {
            let err = Error::refused(format!(
                "request {} was for {}, not {method}: each action has an id of its own",
                first.id, first.method
            ));
            f.settle(Err(Failure::of(&err)));
            return Err(err);
        }
        Ok(first) => {
            let answer = recorded_answer(&first);
            f.settle(answer.clone());
            return answer_of(answer);
        }
        Err(store::Error::NotFound) => {}
        Err(err) => {
            let err = Error::from(err);
            f.settle(Err(Failure::of(&err)));
            return Err(err);
        }
    }
    let started = model::Request {
        id: req.id.clone(),
        method: method.to_owned(),
        by: req.by.clone(),
        at: svc.core().now(),
        ..model::Request::default()
    };
    if let Err(err) = svc.core().store.update(move |tx| tx.start_request(&started)).await {
        let err = Error::from(err);
        f.settle(Err(Failure::of(&err)));
        return Err(err);
    }
    let o = Arc::clone(&svc);
    let (id, logged) = (req.id.clone(), method.to_owned());
    let done = svc.core().acting.spawn(async move {
        let v = act(Arc::clone(&o)).await;
        let answer: Answer = match &v {
            Ok(v) => serde_json::to_value(v)
                .map(ForeignJson)
                .map_err(|e| Failure::failed(e.to_string())),
            Err(err) => Err(Failure::of(err)),
        };
        let (ok, err_text, err_kind, err_detail) = match &answer {
            Ok(a) => (Some(a.clone()), String::new(), "", None),
            Err(e) => (None, e.why.clone(), e.kind.as_str(), detail_raw(e.detail.as_deref())),
        };
        let finished = id.clone();
        let kept = o
            .core()
            .store
            .update(move |tx| {
                tx.finish_request(&finished, ok.as_ref(), &err_text, err_kind, err_detail.as_ref())
            })
            .await;
        if let Err(err) = kept {
            tracing::error!(request = %id, method = %logged, %err, "request's answer not kept: asked again, it is answered as cut off");
        }
        f.settle(answer);
        v
    });
    done.await.map_err(|e| Error::refused(format!("{method}: {e}")))?
}

/// What answers those waiting on an action that ended without an answer: it panicked, or the call doing it was dropped
/// before the action began.
const CUT_OFF: &str =
    "this was cut off before it answered, so it may not have been done: look, and ask again if it was not";

/// A request id claimed for its action, from the claim until the action is answered (`settle`). Dropped unanswered, it
/// answers that the action was cut off (`CUT_OFF`), so neither those waiting on it nor a retry of the id wait forever.
struct Claimed<S: Service> {
    svc: Arc<S>,
    id: String,
    f: Arc<Inflight>,
    settled: bool,
}

impl<S: Service> Claimed<S> {
    /// Ends the action with answer: whoever waits for it reads it, and the id is the store's to answer again.
    fn settle(mut self, answer: Answer) {
        self.settled = true;
        self.svc.core().settle(&self.id, &self.f, answer);
    }
}

impl<S: Service> Drop for Claimed<S> {
    fn drop(&mut self) {
        if !self.settled {
            self.svc.core().settle(&self.id, &self.f, Err(Failure::failed(CUT_OFF)));
        }
    }
}

impl Core {
    /// Claims request id for method, or gives the answer of the action already doing it.
    fn claim(&self, id: &str, method: &str) -> Result<Claim> {
        let mut rq = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
        if rq.draining {
            return Err(Error::Stopping);
        }
        if let Some(f) = rq.inflight.get(id) {
            if f.method != method {
                return Err(Error::refused(format!(
                    "request {id} is being done for {}, not {method}: each action has an id of its own",
                    f.method
                )));
            }
            return Ok(Claim::Waits(f.answer.subscribe()));
        }
        let f = Arc::new(Inflight {
            method: method.to_owned(),
            answer: watch::Sender::new(None),
        });
        rq.inflight.insert(id.to_owned(), Arc::clone(&f));
        Ok(Claim::Mine(f))
    }

    /// Ends request id's action with answer: whoever waits for it reads it, and the id is the store's to answer again.
    fn settle(&self, id: &str, f: &Inflight, answer: Answer) {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .inflight
            .remove(id);
        f.answer.send_replace(Some(answer));
    }

    /// Refuses new actions and waits for the ones being done, or until stop is cancelled.
    pub async fn drain(&self, stop: &CancellationToken) -> Result<()> {
        self.requests.lock().unwrap_or_else(PoisonError::into_inner).draining = true;
        self.acting.close();
        tokio::select! {
            () = self.acting.wait() => Ok(()),
            () = stop.cancelled() => Err(Error::refused("actions still being done at the deadline")),
        }
    }

    /// Ends what a previous process left of the actions it was doing (it cannot answer them) and forgets those old
    /// enough that no channel asks again, and with them the drafts ended as long ago: they are remembered only to refuse
    /// a save of them that a channel sends late.
    pub async fn settle_requests(&self) -> Result<()> {
        let before = self.now().0.checked_sub(REQUESTS_KEPT).map_or_else(Time::default, Time);
        self.store
            .update(move |tx| {
                tx.end_requests(INTERRUPTED)?;
                tx.forget_requests(before)?;
                tx.forget_drafts(before)
            })
            .await?;
        Ok(())
    }
}

enum Claim {
    /// This call does the action.
    Mine(Arc<Inflight>),
    /// Another does it: its answer, when it comes.
    Waits(watch::Receiver<Option<Answer>>),
}

/// The owner's actions are gated to the owner.
fn owners(by: &Actor, method: &str) -> Result<()> {
    if by.kind == ActorKind::Owner {
        return Ok(());
    }
    Err(Error::refused(format!(
        "{method}: only the owner acts through a channel"
    )))
}

/// The answer an action recorded already gives again.
fn recorded_answer(first: &model::Request) -> Answer {
    if !first.done {
        return Err(Failure::failed(INTERRUPTED));
    }
    if !first.error.is_empty() {
        return Err(Failure {
            kind: ErrorKind::parse(&first.error_kind),
            why: first.error.clone(),
            detail: first
                .error_detail
                .as_ref()
                .and_then(|d| serde_json::from_value(d.0.clone()).ok())
                .map(Box::new),
        });
    }
    first.answer.clone().ok_or_else(|| Failure::failed(INTERRUPTED))
}

fn answer_of<T: DeserializeOwned>(answer: Answer) -> Result<T> {
    let raw = answer.map_err(|f| Error::Answered {
        kind: f.kind,
        why: f.why,
        detail: f.detail,
    })?;
    Ok(serde_json::from_value(raw.0)?)
}

/// A failure's detail as it is kept with its request; one that does not encode is kept as none, and logged.
fn detail_raw(detail: Option<&FailureDetail>) -> Option<ForeignJson> {
    let d = detail?;
    serde_json::to_value(d)
        .map(ForeignJson)
        .inspect_err(|err| tracing::error!(%err, "a failure's detail not kept: a retry is answered without it"))
        .ok()
}
