//! A standalone home is a home that is no chain: the Assistant's. What is recorded for it (messages, events, drafts) is
//! stored and pushed one change after the other, under the writer of the home (`Core::standalone_mu`), as a
//! chain's is under its chain's lock.

use std::future::Future;

use model::{Card, Origin};

use crate::apply::{Applied, Records};
use crate::chain_core::Core;
use crate::{Error, Result};

/// What starts the id of the Assistant, which is the home of its card.
pub const ASSISTANT_ID_PREFIX: &str = "as_";

/// Whether card c works in a standalone home: it is the Assistant's, in no chain.
pub fn standalone_card(c: &Card) -> bool {
    c.origin == Origin::Assistant
}

/// Whether home is the Assistant's rather than a chain's.
pub fn is_standalone_home(home: &str) -> bool {
    home.starts_with(ASSISTANT_ID_PREFIX)
}

impl Core {
    /// Stores a change of what is recorded for a standalone home (w, the messages to it included) under its writer, and
    /// hands the messages over.
    pub async fn change_home(&self, w: Records) -> Result<Applied> {
        let a = {
            let _writer = self.standalone_mu.lock().await;
            self.commit(None, None, w).await?
        };
        self.hand_over(&a).await;
        Ok(a)
    }

    /// Does f under the lock under which the changes of home (a chain or the Assistant) are stored and pushed, sends
    /// included: a save, a deletion and a send of one draft are stored, and pushed, one after the
    /// other, so no channel is told of a save after the send that ended its draft. A deleted chain refuses.
    pub async fn in_home<T, F, Fut>(&self, home: &str, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        if is_standalone_home(home) {
            let _writer = self.standalone_mu.lock().await;
            return f().await;
        }
        let cs = self.state(home).await?;
        let held = cs.held.lock().await;
        if held.gone {
            return Err(Error::ChainDeleted);
        }
        f().await
    }
}
