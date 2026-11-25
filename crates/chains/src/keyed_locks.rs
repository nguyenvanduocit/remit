//! One lock per key (a card), held for as long as its guard lives: an entry lives while it is held or waited for. A
//! waiter that gives up (its future dropped, as a cancelled caller's is) leaves the lock to the next. `sessions` holds
//! a card's session opening (dispatch, open) and its stop for good each one step, taken
//! before the chain's lock, so the stop's change, stored under that lock, hands its one message to the Chair, never to
//! the card it stops.
//!
//! One line per key (a chain): `KeyedQueue` gives a call its place when the call is made, before it awaits anything,
//! and its turn once every call placed before it on that key has gone. A lock taken further in orders calls by whichever
//! reaches it first, which is not the order they were made in when one path awaits more on the way.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{OwnedMutexGuard, watch};

#[derive(Default)]
pub struct KeyedLocks {
    locks: Arc<Mutex<HashMap<String, Entry>>>,
}

struct Entry {
    lock: Arc<tokio::sync::Mutex<()>>,
    /// Holding it or waiting for it.
    users: usize,
}

/// Holds a key's lock until it is dropped.
pub struct KeyGuard {
    _held: OwnedMutexGuard<()>,
    _user: User,
}

/// One holder or waiter of a key's lock: its drop forgets the key once nobody uses it.
struct User {
    locks: Arc<Mutex<HashMap<String, Entry>>>,
    id: String,
}

impl Drop for User {
    fn drop(&mut self) {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(e) = locks.get_mut(&self.id) {
            e.users = e.users.saturating_sub(1);
            if e.users == 0 {
                locks.remove(&self.id);
            }
        }
    }
}

impl KeyedLocks {
    /// Holds id's lock until the guard is dropped. Dropping the future while it waits leaves the lock.
    pub async fn lock(&self, id: &str) -> KeyGuard {
        let lock = {
            let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
            let e = locks.entry(id.to_owned()).or_insert_with(|| Entry {
                lock: Arc::default(),
                users: 0,
            });
            e.users = e.users.saturating_add(1);
            Arc::clone(&e.lock)
        };
        let user = User {
            locks: Arc::clone(&self.locks),
            id: id.to_owned(),
        };
        KeyGuard {
            _held: lock.lock_owned().await,
            _user: user,
        }
    }
}

/// The calls made on each key, in the order they were made (the module's `KeyedQueue`).
#[derive(Default)]
pub struct KeyedQueue {
    lines: Arc<Mutex<HashMap<String, Line>>>,
}

/// The places of one key's line: it lives while one is taken.
struct Line {
    /// The number the next place takes.
    next: u64,
    /// The places taken and not yet gone, in the order taken.
    places: VecDeque<u64>,
    /// The place whose turn it is: the first of places.
    front: watch::Sender<u64>,
}

/// A call's place in its key's line, taken until it is dropped: its drop gives the turn to the next.
pub struct Place {
    lines: Arc<Mutex<HashMap<String, Line>>>,
    key: String,
    number: u64,
    front: watch::Receiver<u64>,
}

impl KeyedQueue {
    /// Takes the next place in key's line, at once.
    pub fn join(&self, key: &str) -> Place {
        let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        let line = lines.entry(key.to_owned()).or_insert_with(|| Line {
            next: 0,
            places: VecDeque::new(),
            front: watch::channel(0).0,
        });
        let number = line.next;
        line.next = number.wrapping_add(1);
        line.places.push_back(number);
        Place {
            lines: Arc::clone(&self.lines),
            key: key.to_owned(),
            number,
            front: line.front.subscribe(),
        }
    }
}

impl Place {
    /// Returns once every place taken before this one on its key has gone.
    pub async fn turn(&mut self) {
        let me = self.number;
        // the line, and so its sender, lives while this place is in it: the wait ends only at this place's turn
        self.front.wait_for(|front| *front == me).await.ok();
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(line) = lines.get_mut(&self.key) else {
            return;
        };
        line.places.retain(|n| *n != self.number);
        match line.places.front() {
            None => {
                lines.remove(&self.key);
            }
            Some(&first) => {
                line.front.send_if_modified(|front| {
                    let moved = *front != first;
                    *front = first;
                    moved
                });
            }
        }
    }
}
