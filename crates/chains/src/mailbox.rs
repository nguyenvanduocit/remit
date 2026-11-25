//! A session's door: it wakes the session's loop when a message is queued for its card (the messages themselves wait in
//! the store, outbox.rs), and it is the only owner of whether the session is still open. A message handed to a closed
//! mailbox is not lost: it stays queued, and the sender opens a new session for it.

use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::sync::Notify;

#[derive(Default)]
pub struct Mailbox {
    state: Mutex<Door>,
    bell: Notify,
}

#[derive(Default)]
struct Door {
    /// A message was queued since the loop last looked.
    rung: bool,
    closed: bool,
}

impl Mailbox {
    fn door(&self) -> MutexGuard<'_, Door> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tells the loop a message waits; false when the mailbox is closed.
    pub fn ring(&self) -> bool {
        let mut d = self.door();
        if d.closed {
            return false;
        }
        d.rung = true;
        self.bell.notify_one();
        true
    }

    /// Waits until the mailbox is rung (since the loop last looked) or closed; false once closed. One loop waits on a
    /// mailbox: `notify_one` keeps its permit for it when it is not waiting yet.
    pub async fn wait(&self) -> bool {
        loop {
            {
                let mut d = self.door();
                if d.closed {
                    return false;
                }
                if d.rung {
                    d.rung = false;
                    return true;
                }
            }
            self.bell.notified().await;
        }
    }

    /// Closes the mailbox; false when it was closed already.
    pub fn close(&self) -> bool {
        let mut d = self.door();
        if d.closed {
            return false;
        }
        d.closed = true;
        self.bell.notify_one();
        true
    }

    pub fn is_closed(&self) -> bool {
        self.door().closed
    }
}
