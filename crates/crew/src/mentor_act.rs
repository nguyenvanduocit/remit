//! How a mentor's device acts (ADR-0062): the code it signed in with is checked, the help it acts on is theirs, and the
//! act is done once per request id under the help's lock.

use std::future::Future;
use std::sync::Arc;

use model::{Actor, Help, Mentor, WorkerKey, by_mentor};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use chains::request::{Request, Service};
use chains::{Error, Result};

use crate::Crew;
use crate::help::{WorkerRequest, help_not_yours};

/// What a page holding a revoked code is told.
const CODE_REVOKED: &str = "this code was revoked: ask the owner for a new one";

impl Crew {
    /// The key key_id, refused as revoked when it is gone.
    pub async fn device_key(&self, key_id: &str) -> Result<WorkerKey> {
        let id = key_id.to_owned();
        match self.store.read(move |r| r.worker_key(&id)).await {
            Err(store::Error::NotFound) => Err(Error::refused(CODE_REVOKED)),
            k => Ok(k?),
        }
    }

    /// Help help_id when device key k may read it: its mentor's.
    async fn worker_help(&self, k: &WorkerKey, help_id: &str) -> Result<Help> {
        let id = help_id.to_owned();
        match self.store.read(move |r| r.help(&id)).await {
            Ok(h) if h.mentor == k.mentor => Ok(h),
            Ok(_) | Err(store::Error::NotFound) => Err(help_not_yours(help_id)),
            Err(err) => Err(err.into()),
        }
    }

    /// The help of key key_id's mentor on card card_id, the newest: refused when they help it in none.
    pub async fn help_on_card(&self, key_id: &str, card_id: &str) -> Result<(WorkerKey, Help)> {
        let k = self.device_key(key_id).await?;
        let (card, mentor) = (card_id.to_owned(), k.mentor.clone());
        let helps = self.store.read(move |r| r.card_helps(&card)).await?;
        let h = helps
            .into_iter()
            .filter(|h| h.mentor == mentor)
            .max_by_key(|h| h.offered_at)
            .ok_or_else(|| Error::refused(format!("you help no agent {card_id}")))?;
        Ok((k, h))
    }

    /// Does a mentor's act on a help of theirs once per request id, as `by_mentor(help)`. A revoked code and another
    /// mentor's help are refused before anything is done; the act decides on the help as stored under its lock
    /// (`help_locks`), so two pages of the mentor's, or the help's deadline, never both decide.
    pub async fn worker_act<T, F, Fut>(
        self: &Arc<Self>,
        req: &WorkerRequest,
        method: &str,
        args: &impl Serialize,
        act: F,
    ) -> Result<T>
    where
        F: FnOnce(Arc<Self>, Mentor, Help) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: Serialize + DeserializeOwned + Send + 'static,
    {
        let k = self.device_key(&req.key_id).await?;
        let h = self.worker_help(&k, &req.help_id).await?;
        let by = by_mentor(&h.id);
        mentor_once(self, &k, &req.id, by, method, args, move |o| async move {
            let _held = o.help_locks.lock(&h.id).await;
            let id = h.id.clone();
            let (live, m) = o
                .store
                .read(move |r| -> store::Result<_> {
                    let live = r.help(&id)?;
                    let m = r.mentor(&live.mentor)?;
                    Ok((live, m))
                })
                .await?;
            act(Arc::clone(&o), m, live).await
        })
        .await
    }
}

/// Does an act of key key_id's mentor once per request id, as `by`: the id is the page's, namespaced by its key, and
/// the method names the act and its arguments, so the same id asked for other work is refused rather than answered
/// with the first act's answer.
pub async fn mentor_once<S, T, F, Fut>(
    svc: &Arc<S>,
    k: &WorkerKey,
    id: &str,
    by: Actor,
    method: &str,
    args: &impl Serialize,
    act: F,
) -> Result<T>
where
    S: Service,
    F: FnOnce(Arc<S>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
    T: Serialize + DeserializeOwned + Send + 'static,
{
    let sum = Sha256::digest(serde_json::to_vec(args)?);
    let r = Request {
        id: format!("worker:{}:{id}", k.id),
        by,
    };
    let method = format!("{method}#{}", hex::encode(sum.get(..8).unwrap_or_default()));
    svc.once(r, &method, |_| Ok(()), act).await
}
