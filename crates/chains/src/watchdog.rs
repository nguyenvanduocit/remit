//! The idle watchdog tells the owner when nothing is happening: when no agent has worked for `Config::idle_alert` while
//! work waits (chains queued, approved changes waiting to land, items in his inbox), a stuck scheduler or a
//! routine that finds nothing would otherwise be noticed only when he looks. It writes one inbox item of kind error, in no
//! chain, which the board shows in its first column and says what waits and what holds it. It is asked of him, so the
//! Assistant reads it first (ADR-0087), moves what it can and dismisses it. It is
//! written once per stretch of idleness: the next Ticks see it open, work moving again withdraws it, and a stretch the
//! owner dismissed it in is not told again. Work moving is a chain's agent beginning a turn (`saw_work`), a chain's agent
//! at work or a submit running when the scheduler looks, or a chain changing. The Assistant's turns are not work moving:
//! it answers what waits for the owner, this item among them, and counting its turn would withdraw the item it answers.

use std::time::Duration;

use model::{Chain, ChainStatus, InboxItem, InboxKind, InboxOption, InboxStatus, Landing, Time};

use crate::admit::{held_by_key_result, waiting};
use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::health::elapsed;
use crate::inbox::put_asked;
use crate::submit::submitting;

/// The home of the watchdog's item: no chain's, so the board shows it among what waits from no chain.
const IDLE_HOME: &str = "remit";

/// What waits while nothing runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct IdleWork {
    queued: usize,
    landings: usize,
    asks: usize,
}

impl IdleWork {
    fn any(self) -> bool {
        self.queued > 0 || self.landings > 0 || self.asks > 0
    }
}

/// Counts what waits: chains queued, the landings waiting in the merge queues (of open, `waiting`), and items open in the
/// owner's inbox besides the watchdog's own (inbox).
fn idle_work_of(chains: &[Chain], open: &[Landing], inbox: &[InboxItem]) -> IdleWork {
    IdleWork {
        landings: waiting(&open.iter().collect::<Vec<_>>()),
        queued: chains.iter().filter(|c| *c.status() == ChainStatus::Queued).count(),
        asks: inbox.iter().filter(|it| it.home != IDLE_HOME).count(),
    }
}

/// d in whole minutes as a duration string: "3h0m", "45m"; empty under a minute.
fn minutes_of(d: Duration) -> String {
    let m = d.as_secs() / 60;
    match (m / 60, m % 60) {
        (0, 0) => String::new(),
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h{m}m"),
    }
}

/// What the watchdog's item says of work w that waits while nothing ran for idle, and of held, why admission holds what
/// is due: its title and its detail.
fn idle_note(w: IdleWork, idle: Duration, held: &[String]) -> (String, String) {
    let mut lines = Vec::new();
    for (count, one, many) in [
        (w.queued, "1 chain is queued", "chains are queued"),
        (
            w.landings,
            "1 approved change waits in the merge queue",
            "approved changes wait in the merge queue",
        ),
        (w.asks, "1 item waits in your inbox", "items wait in your inbox"),
    ] {
        match count {
            0 => {}
            1 => lines.push(format!("- {one}")),
            n => lines.push(format!("- {n} {many}")),
        }
    }
    let idle_for = minutes_of(idle);
    let mut detail = format!(
        "No agent has worked for {idle_for}, and work waits:\n{}",
        lines.join("\n")
    );
    if !held.is_empty() {
        detail.push_str("\n\nWhat holds it:\n- ");
        detail.push_str(&held.join("\n- "));
    }
    detail.push_str(
        "\n\nRemit starts what waits on its own as soon as it may. Look at the Subscriptions tab and the board, then \
         dismiss this: it is not said again until work has moved.",
    );
    (format!("nothing has run for {idle_for} while work waits"), detail)
}

/// The reasons, each once, that admission holds what is due on the Subscriptions tab (held_lines, each line's `held`)
/// and the queued chains that wait on the key result they serve, of landings, every open one.
fn held_reasons(chains: &[Chain], held_lines: &[String], landings: &[Landing]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |why: String| {
        if !why.is_empty() && !out.contains(&why) {
            out.push(why);
        }
    };
    for l in held_lines {
        add(l.clone());
    }
    for c in chains {
        add(held_by_key_result(c, chains, landings, ""));
    }
    out
}

impl Core {
    /// Notes that the agent of home began work at `at`: a chain's agent moves work, the Assistant does not.
    pub fn saw_work(&self, home: &str, at: Time) {
        if !self.moves_work(home) {
            return;
        }
        let mut sh = self.shared();
        if sh.last_work.is_none_or(|last| at > last) {
            sh.last_work = Some(at);
        }
    }

    /// Whether an agent of home at work is work moving: a chain's is, the Assistant's is not (the module's header).
    fn moves_work(&self, home: &str) -> bool {
        !self.is_assistant_home(home)
    }

    /// When work last moved, as the scheduler sees it at now: a chain's agent in a turn or a submit running
    /// is work moving now, and the process's first look counts as work, so a restart starts the count afresh.
    fn idle_since(&self, chains: &[Chain], now: Time) -> Time {
        let working =
            self.all_runs().iter().any(|r| r.busy() && self.moves_work(&r.home)) || chains.iter().any(submitting);
        let mut since = {
            let mut sh = self.shared();
            match sh.last_work {
                Some(last) if !working => last,
                _ => *sh.last_work.insert(now),
            }
        };
        for c in chains {
            if c.updated_at > since {
                since = c.updated_at;
            }
        }
        since
    }

    /// Tells the owner, once per stretch, that nothing has run for `Config::idle_alert` while work waits, and withdraws
    /// what it told him once work moves again or nothing waits; held_lines is why admission holds what is due (the
    /// Subscriptions tab's lines' `held`). Called by Tick, one at a time.
    pub async fn watch_idle(&self, chains: &[Chain], held_lines: &[String], now: Time) {
        if self.cfg.idle_alert.is_zero() {
            return;
        }
        let (open, landings) = match self
            .store
            .read(|r| Ok::<_, store::Error>((r.open_inbox()?, r.open_landings()?)))
            .await
        {
            Ok(read) => read,
            Err(err) => return tracing::error!(%err, "idle watchdog"),
        };
        let told = open.iter().find(|it| it.home == IDLE_HOME);
        let since = self.idle_since(chains, now);
        let w = idle_work_of(chains, &landings, &open);
        let idle = elapsed(since, now);
        if !w.any() || idle < self.cfg.idle_alert {
            if let Some(it) = told {
                self.withdraw_idle(it.clone()).await;
            }
            return;
        }
        if told.is_some() {
            return;
        }
        {
            let mut sh = self.shared();
            if sh.idle_told == Some(since) {
                return;
            }
            sh.idle_told = Some(since);
        }
        let (title, detail) = idle_note(w, idle, &held_reasons(chains, held_lines, &landings));
        tracing::warn!(
            ?since,
            queued = w.queued,
            landings = w.landings,
            inbox = w.asks,
            "nothing has run while work waits"
        );
        let it = InboxItem {
            id: new_id("i_"),
            home: IDLE_HOME.to_owned(),
            kind: InboxKind::Error,
            title,
            detail,
            options: vec![InboxOption {
                id: "dismiss".to_owned(),
                name: "Dismiss".to_owned(),
                ..InboxOption::default()
            }],
            status: InboxStatus::Open,
            created_at: now,
            ..InboxItem::default()
        };
        let door = self.door();
        match self.store.update(move |tx| put_asked(tx, &door, it)).await {
            Ok(it) => {
                self.notify.inbox_updated(&it);
                self.kick(); // the Assistant is told on the Tick, and may move what waits
            }
            Err(err) => tracing::error!(%err, "idle watchdog"),
        }
    }

    /// Cancels the watchdog's open item it: work moved again, or nothing waits any more.
    async fn withdraw_idle(&self, mut it: InboxItem) {
        it.status = InboxStatus::Cancelled;
        it.answer = "work moved again".to_owned();
        match self.store.update(move |tx| tx.put_inbox(it)).await {
            Ok(it) => self.notify.inbox_updated(&it),
            Err(err) => tracing::error!(%err, "idle watchdog"),
        }
    }
}
