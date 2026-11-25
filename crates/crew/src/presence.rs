//! A mentor's presence is which of their codes has a web app session open: kept in memory only, by key, and gone with a
//! restart, when each key's stored last use stands in for it. Where a mentor stands for the Chair and the owner is read
//! off it (`Presence::words`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use model::{Time, WorkerKey};

use crate::Crew;
use crate::employee_standing::span;
use chains::health::elapsed;

/// Where a mentor stands with the app: in it now (`in_app_now`), or when they were last active (`last_active`: when a
/// code of theirs last opened a session; none while none has). `open` is each code with a session open, by key id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presence {
    pub in_app_now: bool,
    pub last_active: Option<Time>,
    pub open: HashSet<String>,
}

/// The presence of a mentor who holds the codes keys, whose codes hold online sessions (by key id, a count), at now.
/// When a code was made says nothing of the mentor: the owner made it. Pure.
pub(crate) fn presence_of(keys: &[WorkerKey], online: &HashMap<String, usize>, now: Time) -> Presence {
    let mut p = Presence::default();
    for k in keys {
        if let Some(used) = k.last_used_at
            && used <= now
            && p.last_active.is_none_or(|last| used > last)
        {
            p.last_active = Some(used);
        }
    }
    p.open = online.iter().filter(|(_, n)| **n > 0).map(|(k, _)| k.clone()).collect();
    p.in_app_now = !p.open.is_empty();
    p
}

impl Presence {
    /// p as the Chair and the owner read it at now: the one owner of these words. Pure.
    pub fn words(&self, now: Time) -> String {
        if self.in_app_now {
            "in the app now".to_owned()
        } else if let Some(last) = self.last_active {
            format!("active {} ago", span(elapsed(last, now)))
        } else {
            "no code used yet".to_owned()
        }
    }
}

/// A session of a mentor's key, open until it is dropped.
pub struct DeviceSocket {
    o: Option<Arc<Crew>>,
    mentor: String,
    key: String,
}

impl Drop for DeviceSocket {
    fn drop(&mut self) {
        if let Some(o) = self.o.take() {
            o.device_offline(&self.mentor, &self.key);
        }
    }
}

impl Crew {
    /// Records that a session of key `key_id` of mentor `mentor_id` opened; dropping what it returns records that it
    /// closed.
    pub fn device_online(&self, mentor_id: &str, key_id: &str) -> DeviceSocket {
        {
            let mut sh = self.shared();
            let n = sh
                .online
                .entry(mentor_id.to_owned())
                .or_default()
                .entry(key_id.to_owned())
                .or_default();
            *n = n.saturating_add(1);
        }
        self.notify.mentors_moved();
        DeviceSocket {
            o: self.me(),
            mentor: mentor_id.to_owned(),
            key: key_id.to_owned(),
        }
    }

    /// A session of the key closed.
    fn device_offline(&self, mentor_id: &str, key_id: &str) {
        {
            let mut sh = self.shared();
            if let Some(keys) = sh.online.get_mut(mentor_id) {
                let n = keys.entry(key_id.to_owned()).or_default();
                *n = n.saturating_sub(1);
                if *n == 0 {
                    keys.remove(key_id);
                }
            }
        }
        self.notify.mentors_moved();
    }

    /// Where mentor `mentor_id`, who holds keys, stands with the app now.
    pub fn presence_now(&self, mentor_id: &str, keys: &[WorkerKey]) -> Presence {
        let now = self.now();
        let online = self.shared().online.get(mentor_id).cloned().unwrap_or_default();
        presence_of(keys, &online, now)
    }
}
