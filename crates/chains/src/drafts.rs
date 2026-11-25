//! The owner's drafts: a message he has written in an agent's composer and not sent, kept as he types it, so that every
//! page and every channel shows the same words and a reload loses none. A draft is sent by its id (Prompt), which ends
//! it in the transaction that sends it; the owner deleting it ends it too.

use std::sync::Arc;

use model::Draft;

use crate::chain_core::Core;
use crate::request::Service;
use crate::request::{Done, Request};
use crate::{Error, Result};

impl Core {
    /// Keeps the owner's words as he types them, made on the draft at `base_rev` (0: a draft the page starts). The channel
    /// makes the draft's id, so saving it again (a retry, the same words twice) is saving the same draft. A save made on
    /// a copy another page has saved over since, or a page's first save in an agent's composer that holds a draft
    /// already, saves nothing: it is refused with the draft as it is now (`Error::DraftMoved`), and
    /// the page goes on from that, so no page's words replace another's unseen.
    pub async fn save_draft(self: &Arc<Self>, req: Request, d: Draft, base_rev: i64) -> Result<Draft> {
        self.act(req, "SaveDraft", move |o| async move {
            let mut d = d;
            d.mentor.clear();
            o.keep_draft(d, base_rev).await
        })
        .await
    }

    /// Keeps d for its writer (`Draft::mentor`), which the caller set from who acts, never from the channel's input.
    pub async fn keep_draft(&self, d: Draft, base_rev: i64) -> Result<Draft> {
        check_draft(&d)?;
        self.check_draft_is_of_its_chain(&d).await?;
        self.check_draft_attachments(&d).await?;
        let key = d.id.clone();
        match self.store.read(move |r| r.draft(&key)).await {
            Ok(held) if held.mentor != d.mentor => return Err(draft_not_yours(&d.id)),
            Ok(_) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
        let home = d.home.clone();
        self.in_home(&home, || async {
            let now = self.now();
            let saved = self
                .store
                .update(move |tx| {
                    let mut d = d;
                    if tx.draft_ended(&d.id)? {
                        return Err(store::Error::DraftEnded);
                    }
                    let live = tx.drafts(&d.home)?;
                    if let Some(held) = moved_past(&live, &d, base_rev) {
                        return Ok(Err(held.clone()));
                    }
                    d.updated_at = now;
                    d.rev = 0;
                    tx.put_draft(&d).map(Ok)
                })
                .await?
                .map_err(|current| Error::DraftMoved(Box::new(current)))?;
            self.notify.draft_updated(&saved);
            Ok(saved)
        })
        .await
    }

    /// Ends a draft the owner no longer wants. One sent or deleted already is left as it is.
    pub async fn delete_draft(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        self.act_done(req, "DeleteDraft", move |o| async move {
            o.end_draft(id, String::new()).await
        })
        .await
    }

    /// Ends draft id when writer (a mentor's id; empty: the owner) wrote it.
    pub async fn end_draft(&self, id: String, writer: String) -> Result<()> {
        let key = id.clone();
        let d = match self.store.read(move |r| r.draft(&key)).await {
            Err(store::Error::NotFound) => return Ok(()),
            other => other?,
        };
        if d.mentor != writer {
            return Err(draft_not_yours(&id));
        }
        self.in_home(&d.home, || async {
            let now = self.now();
            // the latest copy: words saved since are deleted with it
            let ended = self
                .store
                .update(move |tx| match tx.draft(&id) {
                    Err(store::Error::NotFound) => Ok(None),
                    Err(err) => Err(err),
                    Ok(d) => {
                        tx.end_drafts(std::slice::from_ref(&d), now)?;
                        Ok(Some(d))
                    }
                })
                .await?;
            if let Some(d) = ended {
                self.notify.draft_deleted(&d);
            }
            Ok(())
        })
        .await
    }

    /// Writer's draft id names (a mentor's id; empty: the owner's): refused when it was sent or deleted already.
    pub async fn draft_to_send(&self, id: &str, writer: &str) -> Result<Draft> {
        let key = id.to_owned();
        let d = match self.store.read(move |r| r.draft(&key)).await {
            Err(store::Error::NotFound) => return Err(Error::Store(store::Error::DraftEnded)),
            other => other?,
        };
        if d.mentor != writer {
            return Err(draft_not_yours(id));
        }
        Ok(d)
    }

    /// Refuses a message to another chain's agent.
    async fn check_draft_is_of_its_chain(&self, d: &Draft) -> Result<()> {
        let id = d.card_id.clone();
        let c = self
            .store
            .read(move |r| r.card(&id))
            .await
            .map_err(|err| Error::refused(format!("agent {}: {err}", d.card_id)))?;
        if c.home != d.home {
            return Err(Error::refused(format!("agent {} is not this chain's", d.card_id)));
        }
        Ok(())
    }
}

/// Refuses a writer a draft of another's, in the same words as one that does not exist.
fn draft_not_yours(id: &str) -> Error {
    Error::refused(format!("draft {id} is not yours"))
}

/// Refuses a draft with no id, nothing written, and one that does not say which agent it is to.
fn check_draft(d: &Draft) -> Result<()> {
    let refuse = |why: &str| Err(Error::refused(why));
    if d.id.is_empty() {
        return refuse("a draft needs an id");
    }
    if d.home.is_empty() {
        return refuse("a draft is in a chain, or for the Assistant");
    }
    if d.text.trim().is_empty() && d.attachments.is_empty() {
        return refuse("a draft holds words or attachments: delete it instead");
    }
    if d.card_id.is_empty() {
        return refuse("a message is to an agent");
    }
    Ok(())
}

/// The draft d's save would replace unseen, when there is one: d's own at another revision than the one it was made on,
/// or, for a page's first save in an agent's composer, the draft that composer holds already.
fn moved_past<'a>(live: &'a [Draft], d: &Draft, base_rev: i64) -> Option<&'a Draft> {
    let mut live = live.iter().filter(|x| x.mentor == d.mentor);
    if let Some(own) = live.clone().find(|x| x.id == d.id) {
        return (own.rev != base_rev).then_some(own);
    }
    live.find(|x| x.card_id == d.card_id)
}
