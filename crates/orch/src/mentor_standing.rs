//! A mentor's standing is what the owner reads of a mentor beside their record: whether they are in the app
//! (presence.rs, worded by `Presence::words`), the codes they work from, and how many helps of theirs are open. Presence is
//! in memory and moves as their sessions open and close, which is no stored change of the mentor, so the standing is a view
//! (views.rs) with the views' revision, as an agent's standing is, computed again whenever a mentor's record, codes, sessions
//! or helps move (`touch_mentors`).

use std::collections::HashMap;

use model::{Mentor, Time, WorkerKey};
use wire::app::{MentorStanding, WorkerKeyLine};

use crate::orch::Orchestrator;
use crate::views::Computed;
use chains::Result;
use crew::presence::Presence;

/// The standing of mentor m, who holds the codes keys, stands with their apps as p says and holds open of helps, at now.
/// Pure.
fn mentor_standing_of(m: &Mentor, keys: &[WorkerKey], p: &Presence, open: usize, now: Time) -> MentorStanding {
    let mut keys: Vec<&WorkerKey> = keys.iter().collect();
    keys.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
    let keys = keys
        .into_iter()
        .map(|k| {
            let open = p.open.contains(&k.id);
            WorkerKeyLine {
                id: k.id.clone(),
                note: k.note.clone(),
                open,
                state: if open { "open" } else { "closed" }.to_owned(),
                created_at: k.created_at,
                last_used_at: k.last_used_at,
            }
        })
        .collect();
    MentorStanding {
        mentor: m.id.clone(),
        in_app_now: p.in_app_now,
        last_active: p.last_active,
        presence: p.words(now),
        keys,
        open_helps: open,
        rev: 0,
    }
}

impl Orchestrator {
    /// The standing of every mentor of the pool, removed ones included, oldest added first, pushing what changed.
    pub(crate) async fn mentors_of(&self) -> Result<Vec<MentorStanding>> {
        let mut comp = self.views.compute.lock().await;
        let (pool, keys, open) = self
            .store
            .read(|r| {
                let pool = r.mentors()?;
                let keys = r.all_worker_keys()?;
                let mut open = HashMap::new();
                for m in &pool {
                    let n = r.mentor_helps(&m.id)?.iter().filter(|h| h.open()).count();
                    open.insert(m.id.clone(), n);
                }
                Ok::<_, store::Error>((pool, keys, open))
            })
            .await?;
        let mut held: HashMap<&str, Vec<WorkerKey>> = HashMap::new();
        for k in &keys {
            held.entry(k.mentor.as_str()).or_default().push(k.clone());
        }
        let now = self.now();
        let mut out = Vec::with_capacity(pool.len());
        for m in &pool {
            let theirs = held.get(m.id.as_str()).map_or(&[][..], Vec::as_slice);
            let p = self.crew.presence_now(&m.id, theirs);
            let open = open.get(&m.id).copied().unwrap_or_default();
            out.push(self.put_mentor(&mut comp, mentor_standing_of(m, theirs, &p, open, now)));
        }
        Ok(out)
    }

    /// Keeps s as the mentor's standing, pushed at a new revision when it changed; comp is held.
    fn put_mentor(&self, comp: &mut Computed, mut s: MentorStanding) -> MentorStanding {
        let was = comp.mentors.get(&s.mentor);
        s.rev = was.map_or(0, |w| w.rev);
        if let Some(was) = was
            && *was == s
        {
            return s;
        }
        comp.rev = comp.rev.saturating_add(1);
        s.rev = comp.rev;
        comp.mentors.insert(s.mentor.clone(), s.clone());
        self.notify.mentor_standing_updated(&s);
        s
    }
}
