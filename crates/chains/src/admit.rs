//! Admission is one rule, at the one place every entry to running passes (`apply_locked`): a queued chain's start, a
//! scheduled wake (the firing of a wake subscription), the owner resuming a paused chain or reopening a finished one, a
//! message waking a waiting chain early.
//!
//! ```text
//! entry                                    provider     headroom                         limit                                          key result
//! queued start                             needed       needed, unless the owner asked   a place, unless the owner waits on the chain   its share, its queue, not stalled, unless the owner asked
//! scheduled wake                           needed       needed                           a place, unless the owner waits on the chain   none
//! owner resume, owner reopen               not needed   not needed                       none                                           none
//! early wake by a message                  not needed   not needed                       none                                           none
//! owner checks back now                    not needed   not needed                       none                                           none
//! a workflow wait's firing (scheduled)     needed       needed                           a place, unless the owner waits on the chain   none
//! a workflow moved by a report or answer   not needed   not needed                       none                                           none
//! a subscription telling a Chair           not needed   not needed                       none                                           none
//! ```
//!
//! The key result a chain serves is the unit the limit is shared by (ADR-0115): a start is charged to it, and takes a place
//! only while the chains serving it run fewer than its `share`, which what its landings came to grows and halves
//! (`fates`), and while fewer of its landings than its share wait in the merge queues (`waiting`, back-pressure: more of
//! its work would only wait too); the queue offers first the starts of key results with no chain running, then the
//! soonest horizon, then the oldest (`queue_of`). A key result whose last `MAX_NUDGES` finished
//! chains made no progress is stalled (`stalled`): it takes no new start until it is lifted. A chain the owner asked for
//! is never held by either, and a wake of a chain at work is no new start.
//!
//! Headroom (ADR-0092): while every account that can run agents is near a limit its provider warned of, what the
//! scheduler starts on its own waits, so the room left goes to the owner's and the Assistant's own turns, which take no
//! entry here. A routine's or an event's start firing waits for it too (signals.rs).
//!
//! The limit (`capacity`, ADR-0115) paces what the scheduler starts on its own: it is measured, not set. Each Tick it grows
//! by one while a start waits on it and nothing presses, and halves under pressure (`Pressure`: the accounts near their
//! limits, failovers, the host's memory or load); it is never below what runs, so it holds what would start and stops
//! nothing. The owner acting, and a message that cannot wait for the Chair's check-back, do not queue behind it. Only the
//! scheduler makes entries that take a place, one at a time (a Tick's starts and a wake's delivery hold `slot_mu`), so two
//! cannot take the last one, and a place is promised to a wake due before a queued start takes it (`promised_places`). What
//! the scheduler starts on its own also waits for a provider that can run agents (Health); the owner acting is not held by
//! it.

use std::collections::{HashMap, HashSet};

use model::{
    ActionKind, Chain, ChainStatus, Firing, KRStatus, KeyResult, Landing, LandingState, OwnerKind, ReturnReason,
    Serves, Time,
};

use crate::chain_core::Core;
use crate::schedule::MAX_NUDGES;
use crate::step::{ChainCmd, WakeCmd};
use crate::{Error, Result};

/// What admission weighs: whether a provider can run agents, the limit (the chains running besides this one, of the
/// measured capacity), and the share of the key result the chain entering serves.
#[derive(Debug, Clone, Default)]
pub struct Room {
    /// Why no provider can run agents (Health.Error); empty while one can.
    pub no_agents: String,
    /// Why the room left on the accounts is kept for the owner's and the Assistant's turns (`no_headroom`); empty while
    /// an account has room.
    pub no_headroom: String,
    /// The chains running besides this one, of the capacity the last Tick measured.
    pub limit: Pool,
    /// What pressed on the capacity at the last Tick, in words (`Pressure::words`); empty when nothing did.
    pub pressure: String,
    /// The chains running that serve the key result of the chain entering, of its share (`share`).
    pub share: Pool,
    /// The landings of that key result's work waiting in the merge queues (`waiting`): its back-pressure.
    pub queued: usize,
    /// Why the key result of the chain entering is stalled (`stalled`); empty when it is not.
    pub stalled: String,
    /// The places in the limit the waiting chains a wake firing still to be delivered runs will take (`promised_places`).
    pub promised: usize,
}

/// A bulkhead: how many of one kind of work are at it, and the most that may be (0: no limit).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pool {
    pub busy: usize,
    pub most: usize,
}

impl Pool {
    fn full(self) -> bool {
        self.most > 0 && self.busy >= self.most
    }
}

/// What presses on the capacity at a Tick (ADR-0115): each field one pressure, the seam where a new one is added.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pressure {
    /// Why every account that can run agents is near a limit its provider warned of (`no_headroom`); empty while one has
    /// room.
    pub accounts: String,
    /// The turns whose account failed (a rate limit, a refusal) and whose card moved on since the last Tick (failover.rs).
    pub failovers: usize,
    /// The host as it was read; none when it cannot be.
    pub host: Option<crate::ports::HostReading>,
    /// The agents' sessions open, which the host's memory is shared among.
    pub sessions: usize,
}

impl Pressure {
    /// What presses, each in words; empty when nothing does. Memory presses when what is free is below twice what one
    /// session holds on average, load when the last minute's is above the cores.
    pub fn words(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.accounts.is_empty() {
            out.push(self.accounts.clone());
        }
        match self.failovers {
            0 => {}
            1 => out.push("a turn's account failed and its card moved on since the last look".to_owned()),
            n => out.push(format!(
                "{n} turns' accounts failed and their cards moved on since the last look"
            )),
        }
        if let Some(h) = &self.host {
            if let Some(per) = h
                .sessions_memory
                .checked_div(u64::try_from(self.sessions).unwrap_or(u64::MAX))
                && h.free_memory < per.saturating_mul(2)
            {
                out.push(format!(
                    "the host has {} of memory free, less than two sessions take ({} each)",
                    bytes_text(h.free_memory),
                    bytes_text(per)
                ));
            }
            if h.cores > 0 && h.load > f64::from(h.cores) {
                out.push(format!("the host's load is {:.1} on {} cores", h.load, h.cores));
            }
        }
        out
    }
}

/// n bytes as the owner reads a size: "812 MB", "3.4 GB".
pub(crate) fn bytes_text(n: u64) -> String {
    const MB: u64 = 1 << 20;
    const GB: u64 = 1 << 30;
    if n >= GB {
        format!("{}.{} GB", n / GB, (n % GB).saturating_mul(10) / GB)
    } else {
        format!("{} MB", n / MB)
    }
}

/// The capacity a Tick measures (ADR-0115) from prev, the chains running, whether a start waits on the limit, and whether
/// anything presses: under pressure it halves; with nothing pressing and a start waiting it grows by one; else it stays.
/// It is never below the chains running, so it stops nothing that runs, and never below 1. Pure.
pub fn capacity(prev: usize, running: usize, waits: bool, pressed: bool) -> usize {
    let next = if pressed {
        prev / 2
    } else if waits {
        prev.saturating_add(1)
    } else {
        prev
    };
    next.max(running).max(1)
}

/// The limit as the last Tick measured it, and what pressed on it then.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capacity {
    /// The chains the scheduler may run at once; none before the first Tick measured it.
    pub limit: Option<usize>,
    /// What pressed, in words; empty when nothing did.
    pub pressure: String,
}

/// What an entry to running takes from the room.
#[derive(Debug, Clone, Copy, Default)]
pub struct Needs {
    /// A provider that can run agents.
    pub agents: bool,
    /// An account with room beyond what the owner's and the Assistant's turns keep (ADR-0092).
    pub headroom: bool,
    /// A place in the limit.
    pub slot: bool,
    /// A place in the share of the key result the chain serves, and that key result not stalled: a new start.
    pub charged: bool,
}

/// The scheduler's own entries take an agent, and a start or a wake takes a place in the limit besides.
const SCHEDULER_NEEDS: Needs = Needs {
    slot: true,
    agents: true,
    headroom: true,
    charged: false,
};

fn not_admitted(why: impl Into<String>) -> Error {
    Error::NotAdmitted(why.into())
}

/// Whether chain c may enter running by cmd, given r; see the table above.
pub fn admit(c: &Chain, cmd: &ChainCmd, r: &Room) -> Result<()> {
    let n = needs_of(c, cmd)?;
    fits(&n, r)?;
    let promised = Pool {
        busy: r.limit.busy.saturating_add(r.promised),
        most: r.limit.most,
    };
    if matches!(cmd, ChainCmd::Start(_)) && n.slot && r.promised > 0 && promised.full() {
        return Err(not_admitted(format!(
            "the free places go first to the {} waiting chains whose time to wake has come",
            r.promised
        )));
    }
    if n.charged && !r.stalled.is_empty() {
        return Err(not_admitted(r.stalled.clone()));
    }
    if n.charged && r.share.full() {
        let running = if r.share.busy == 1 {
            "a chain serving the same key result runs".to_owned()
        } else {
            format!("{} chains serving the same key result run", r.share.busy)
        };
        return Err(not_admitted(format!(
            "{running}, as many as its share ({}): it starts when one finishes; refining the key result \
             (refine_key_result) lets its parts run side by side",
            r.share.most
        )));
    }
    if n.charged && r.share.most > 0 && r.queued >= r.share.most {
        let waits = if r.queued == 1 {
            "a change of the same key result's work waits".to_owned()
        } else {
            format!("{} changes of the same key result's work wait", r.queued)
        };
        return Err(not_admitted(format!(
            "{waits} in the merge queue, as many as its share ({}): it starts as they land",
            r.share.most
        )));
    }
    Ok(())
}

/// What a landing of a key result's work came to: it held on its base, or it was taken back or returned for what the
/// queue found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    Held,
    Taken,
}

/// The share of the limit a key result's chains may hold at once (ADR-0115), from the fates of the landings of its work,
/// oldest first: 1 to begin, one more for each that held, halved (never below 1) for each taken. Work that breaks the
/// base gets less room, work that lands more. Pure.
pub fn share(fates: &[Fate]) -> usize {
    fates.iter().fold(1usize, |s, f| match f {
        Fate::Held => s.saturating_add(1),
        Fate::Taken => (s / 2).max(1),
    })
}

/// The fates of landings, in the order they fell: one that is on its base held when it landed; one reverted was taken
/// when it was; one returned for a conflict or a failed combined check was taken when it went back. A revert is no fate of
/// its own (it is what the landing it takes back came to), and a landing still on its way, or returned for what says
/// nothing of the work (a newer version, the chain leaving), has none yet. Pure.
pub fn fates(landings: &[&Landing]) -> Vec<Fate> {
    let mut fell: Vec<(Time, Fate)> = landings
        .iter()
        .filter(|l| l.reverts.is_none())
        .filter_map(|l| match &l.state {
            LandingState::Landed => l.landed_at.map(|at| (at, Fate::Held)),
            LandingState::Reverted { .. } => Some((l.reverted_at.unwrap_or(l.updated_at), Fate::Taken)),
            LandingState::Returned {
                reason: ReturnReason::Conflict | ReturnReason::Checks,
                ..
            } => Some((l.updated_at, Fate::Taken)),
            _ => None,
        })
        .collect();
    fell.sort_by_key(|(at, _)| *at);
    fell.into_iter().map(|(_, f)| f).collect()
}

/// How many of landings wait in a merge queue: ready, combined or checked, and no revert (a revert is the queue's own
/// work, not the key result's). Pure.
pub fn waiting(landings: &[&Landing]) -> usize {
    landings
        .iter()
        .filter(|l| l.reverts.is_none())
        .filter(|l| {
            matches!(
                l.state,
                LandingState::Ready | LandingState::Combining | LandingState::Checking
            )
        })
        .count()
}

/// What one finished chain did for the key result it serves, as the progress rail reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub chain: String,
    pub title: String,
    pub serves: Serves,
    pub created_at: Time,
    /// It landed work (`landed`) or its goal judge's latest verdict confirmed its key result.
    pub progressed: bool,
}

/// Whether a chain's work landed, of its landings: one of them is on its base. A landing taken back is no progress.
/// Pure.
pub fn landed(landings: &[Landing]) -> bool {
    landings.iter().any(|l| l.reverts.is_none() && l.on_base())
}

/// Why key result id of objective, of its tree, takes no new chain (ADR-0115): it, or a key result it refines, is
/// stalled (`stalled`), so a fresh key result under a stalled one gets no room the stalled one lacks. Pure.
pub fn stall_on(objective: &str, tree: &[KeyResult], id: &str, outcomes: &[Outcome]) -> Option<String> {
    model::lineage(tree, id)
        .into_iter()
        .filter_map(|a| tree.iter().find(|kr| kr.id == a))
        .find_map(|kr| stalled(objective, kr, tree, outcomes))
}

/// Why key result kr of objective, of its tree, is stalled (ADR-0115), or none: the last `MAX_NUDGES` finished chains,
/// of outcomes, that serve it or a key result refining it made no progress, counting those queued since the rail on it
/// was last lifted (`KeyResult::lifted`). Work landed or met below it is progress on it. The words name those chains.
/// Pure.
pub fn stalled(objective: &str, kr: &KeyResult, tree: &[KeyResult], outcomes: &[Outcome]) -> Option<String> {
    let lifted = kr.lifted.as_ref().map(|l| l.at);
    let mut last: Vec<&Outcome> = outcomes
        .iter()
        .filter(|o| {
            o.serves.objective == objective && model::lineage(tree, &o.serves.key_result).contains(&kr.id.as_str())
        })
        .filter(|o| lifted.is_none_or(|at| o.created_at > at))
        .collect();
    last.sort_by_key(|o| std::cmp::Reverse(o.created_at));
    last.truncate(usize::try_from(MAX_NUDGES).unwrap_or(usize::MAX));
    if last.len() < usize::try_from(MAX_NUDGES).unwrap_or(usize::MAX) || last.iter().any(|o| o.progressed) {
        return None;
    }
    let named: Vec<String> = last.iter().map(|o| format!("{} ({})", o.title, o.chain)).collect();
    Some(format!(
        "key result {} is stalled: its last {MAX_NUDGES} chains finished without progress (no work landed, no goal \
         met): {}. It takes no new chain until the Assistant lifts the rail, or the owner asks for one",
        kr.id,
        named.join("; ")
    ))
}

/// Whether r has what n takes: a provider that can run agents, then headroom on its accounts, then a place in the limit.
/// The provider comes before the rest: they free up, and with no provider nothing starts all the same.
pub fn fits(n: &Needs, r: &Room) -> Result<()> {
    if n.agents && !r.no_agents.is_empty() {
        return Err(no_agents(&r.no_agents));
    }
    if n.headroom && !r.no_headroom.is_empty() {
        return Err(no_headroom(&r.no_headroom));
    }
    if n.slot && r.limit.full() {
        return Err(not_admitted(full_words(r.limit.most, &r.pressure)));
    }
    Ok(())
}

/// Why a start waits on a full limit of most, pressure being what pressed on it (empty: nothing): the limit and what set
/// it, as the owner reads them where held starts are shown.
fn full_words(most: usize, pressure: &str) -> String {
    let run = if most == 1 {
        "1 chain runs".to_owned()
    } else {
        format!("{most} chains run")
    };
    if pressure.is_empty() {
        format!("{run}, as many as Remit runs at once now: the limit grows by one each look while nothing presses")
    } else {
        format!("{run}, as many as Remit runs at once now: the limit was halved because {pressure}")
    }
}

/// Whether the owner asked for chain c: himself, or the Assistant on his words. A chain from before Remit recorded who
/// asked is not taken for his.
pub fn owners_ask(c: &Chain) -> bool {
    c.by.as_ref().is_some_and(model::Actor::for_owner)
}

/// Refuses what needs a provider that can run agents when none can; why is what each failed with.
pub fn no_agents(why: &str) -> Error {
    not_admitted(format!("no agent can run: {why}; it starts once one can"))
}

/// Refuses what needs headroom while every account is near its limit; why says each account's.
pub fn no_headroom(why: &str) -> Error {
    not_admitted(format!(
        "{why}: the room left is kept for the owner's and the Assistant's own turns, and this starts once an account has room again"
    ))
}

/// What holds an entry to running, from the error admit gave, for the owner to read.
pub fn why_not(err: &Error) -> String {
    match err {
        Error::NotAdmitted(why) => why.clone(),
        other => other.to_string(),
    }
}

/// Why the start of queued chain c, of chains, waits on the key result it serves: the chains serving it fill its share,
/// as many of its landings (landings holds every chain's) wait in the merge queues, or it is stalled (stalled,
/// `stall_of`; empty when it is not); empty when it waits on none. The board says it on the chain.
pub fn held_by_key_result(c: &Chain, chains: &[Chain], landings: &[Landing], stalled: &str) -> String {
    if *c.status() != ChainStatus::Queued {
        return String::new();
    }
    let of_it = serving(c, chains, landings);
    let room = Room {
        share: share_of(c, chains, &of_it),
        queued: waiting(&of_it),
        stalled: stalled.to_owned(),
        ..Room::default()
    };
    match admit(c, &ChainCmd::Start(crate::step::StartCmd::default()), &room) {
        Ok(()) => String::new(),
        Err(err) => why_not(&err),
    }
}

/// The chains of chains running besides c that serve the key result c serves, of its share, from the landings of its
/// work (of_it, `serving`). Pure.
fn share_of(c: &Chain, chains: &[Chain], of_it: &[&Landing]) -> Pool {
    let busy = chains
        .iter()
        .filter(|o| *o.status() == ChainStatus::Running && o.id != c.id && o.serves.is_some() && o.serves == c.serves)
        .count();
    Pool {
        busy,
        most: share(&fates(of_it)),
    }
}

/// The landings, of landings, of every chain of chains that serves the key result c serves; none when c serves none.
/// Pure.
pub fn serving<'a>(c: &Chain, chains: &[Chain], landings: &'a [Landing]) -> Vec<&'a Landing> {
    let Some(serves) = c.serves.as_ref() else {
        return Vec::new();
    };
    let ids: HashSet<&str> = chains
        .iter()
        .filter(|o| o.serves.as_ref() == Some(serves))
        .map(|o| o.id.as_str())
        .collect();
    landings.iter().filter(|l| ids.contains(l.chain_id.as_str())).collect()
}

/// Which entry cmd is, by what it takes from the room.
fn needs_of(c: &Chain, cmd: &ChainCmd) -> Result<Needs> {
    match cmd {
        ChainCmd::Start(_) => {
            // the scheduler's own; charged to its key result unless the owner asked for it
            return Ok(Needs {
                headroom: !owners_ask(c),
                slot: !c.kind.owner_waits(),
                charged: !owners_ask(c),
                ..SCHEDULER_NEEDS
            });
        }
        ChainCmd::Wake(_) => {
            return Ok(Needs {
                slot: !c.kind.owner_waits(),
                ..SCHEDULER_NEEDS
            });
        }
        ChainCmd::Resume(_) | ChainCmd::CheckNow(_) => return Ok(Needs::default()), // the owner's
        // a wait's firing is the scheduler's own, as a wake is; the owner's answer or stop is his
        ChainCmd::Workflow(w) if w.scheduled => {
            return Ok(Needs {
                slot: !c.kind.owner_waits(),
                ..SCHEDULER_NEEDS
            });
        }
        ChainCmd::Workflow(_) => return Ok(Needs::default()),
        ChainCmd::Send(send) => {
            let reopen = *c.status() == ChainStatus::Done && send.msg.by.for_owner(); // the owner reopens it, or the Assistant for him
            if reopen || *c.status() == ChainStatus::Waiting {
                return Ok(Needs::default()); // or a message wakes it early
            }
        }
        _ => {}
    }
    Err(not_admitted(format!(
        "{cmd} is no entry to running from {}",
        c.status()
    )))
}

/// The waiting chains other than id that a wake firing still to be delivered (pending) is about to run, and that admission
/// lets wake with a slot free: each takes a slot as its delivery comes, ahead of a queued start. Pure.
fn promised_places(chains: &[Chain], pending: &[Firing], id: &str, r: &Room) -> usize {
    let waiting: HashMap<&str, &Chain> = chains
        .iter()
        .filter(|c| *c.status() == ChainStatus::Waiting && c.id != id)
        .map(|c| (c.id.as_str(), c))
        .collect();
    let mut free = r.clone();
    free.limit.most = 0;
    let wake = ChainCmd::Wake(WakeCmd::default());
    let mut promised = HashSet::new();
    for f in pending {
        let Some(c) = waiting.get(f.owner.id.as_str()) else {
            continue;
        };
        if !matches!(f.action, ActionKind::Wake | ActionKind::Workflow) || f.owner.kind != OwnerKind::Chain {
            continue;
        }
        if needs_of(c, &wake).is_ok_and(|n| n.slot) && admit(c, &wake, &free).is_ok() {
            promised.insert(c.id.as_str());
        }
    }
    promised.len()
}

impl Core {
    /// The capacity as the last Tick measured it.
    pub fn capacity(&self) -> Capacity {
        self.shared().capacity.clone()
    }

    /// Measures the capacity for this Tick over chains (`capacity`), from what presses now, and keeps it. The host is
    /// read off the async threads: a look at every process takes a while.
    pub async fn measure_capacity(&self, chains: &[Chain]) {
        let host = match self.cfg.host.clone() {
            Some(h) => tokio::task::spawn_blocking(move || h.reading()).await.ok().flatten(),
            None => None,
        };
        let sessions = self.all_runs().iter().filter(|r| r.state().sess.is_some()).count();
        let failovers = std::mem::take(&mut self.shared().failovers);
        let pressure = Pressure {
            accounts: self.no_headroom(),
            failovers,
            host,
            sessions,
        };
        let words = pressure.words();
        let running = chains.iter().filter(|c| *c.status() == ChainStatus::Running).count();
        let mut sh = self.shared();
        // the first Tick starts from the chains running when the server started
        let prev = sh.capacity.limit.unwrap_or(running).max(1);
        let waits = running >= prev && chains.iter().any(|c| *c.status() == ChainStatus::Queued);
        sh.capacity = Capacity {
            limit: Some(capacity(prev, running, waits, !words.is_empty())),
            pressure: words.join("; "),
        };
    }

    /// What admission weighs for chain id now.
    pub async fn room(&self, id: &str) -> Result<Room> {
        let cap = self.capacity();
        let mut r = Room {
            no_agents: self.no_agents(),
            no_headroom: self.no_headroom(),
            limit: Pool {
                busy: 0,
                most: cap.limit.unwrap_or(1),
            },
            pressure: cap.pressure,
            ..Room::default()
        };
        let entering = id.to_owned();
        let (chains, pending, stall, landings) = self
            .store
            .read(move |s| {
                let chains = s.chains()?;
                let mut landings = Vec::new();
                let mut stall = None;
                if let Some(serves) = chains.iter().find(|c| c.id == entering).and_then(|c| c.serves.clone()) {
                    for c in chains.iter().filter(|c| c.serves.as_ref() == Some(&serves)) {
                        landings.extend(s.chain_landings(&c.id)?);
                    }
                    stall = stall_of(s, &chains, &serves)?;
                }
                Ok::<_, store::Error>((chains, s.pending_firings("")?, stall, landings))
            })
            .await
            .map_err(|e| Error::refused(format!("admission: {e}")))?;
        let me = chains.iter().find(|c| c.id == id);
        r.limit.busy = chains
            .iter()
            .filter(|c| *c.status() == ChainStatus::Running && c.id != id)
            .count();
        if let Some(c) = me {
            let of_it = serving(c, &chains, &landings);
            r.share = share_of(c, &chains, &of_it);
            r.queued = waiting(&of_it);
        }
        r.stalled = stall.unwrap_or_default();
        r.promised = promised_places(&chains, &pending, id, &r);
        Ok(r)
    }
}

/// Why the key result serves names takes no new chain (`stall_on`), of chains as r holds them; none when it does, or
/// no longer stands.
pub fn stall_of(r: &store::Reader<'_>, chains: &[Chain], serves: &Serves) -> store::Result<Option<String>> {
    let tree = r.key_results(&serves.objective)?;
    let of_it = chains
        .iter()
        .filter(|c| c.serves.as_ref().is_some_and(|s| s.objective == serves.objective));
    let outcomes = outcomes_of(r, of_it)?;
    Ok(stall_on(&serves.objective, &tree, &serves.key_result, &outcomes))
}

/// What each finished chain of chains did for the key result it serves, with the verdicts of its goal judge r holds. An
/// attempt of a round (ADR-0115) did what its round did: one attempt lands for all of them, so the others, not picked,
/// are no chains without progress.
pub fn outcomes_of<'a>(r: &store::Reader<'_>, chains: impl Iterator<Item = &'a Chain>) -> store::Result<Vec<Outcome>> {
    let chains: Vec<&Chain> = chains.collect();
    let mut out = Vec::new();
    for c in chains.iter().filter(|c| *c.status() == ChainStatus::Done) {
        let Some(serves) = c.serves.clone() else { continue };
        let approvals = r.approvals(&c.id)?;
        let verdicts: Vec<_> = crate::goal::goal_approvals(&approvals).into_iter().cloned().collect();
        let met = crate::goal::verdict_on(&serves.key_result, &verdicts) == KRStatus::Met;
        let mut landings = Vec::new();
        for mate in chains
            .iter()
            .filter(|o| o.id == c.id || (c.attempt.is_some() && o.attempt == c.attempt))
        {
            landings.extend(r.chain_landings(&mate.id)?);
        }
        out.push(Outcome {
            chain: c.id.clone(),
            title: c.title.clone(),
            serves,
            created_at: c.created_at,
            progressed: met || landed(&landings),
        });
    }
    Ok(out)
}

/// The horizon of each objective (`Horizon.until`), by objective id: what orders the starts of key results (ADR-0115).
pub type Horizons = HashMap<String, Time>;

/// The queued chains among chains in the order they start (ADR-0115): the ones the owner waits on first (they take no
/// place in the limit); then those whose key result has no chain running, so each served key result runs one before any
/// runs two; then by their objective's horizon, soonest first and none last; then the oldest first. Pure.
pub fn queue_of<'a>(chains: &'a [Chain], horizons: &Horizons) -> Vec<&'a Chain> {
    let running: HashSet<&Serves> = chains
        .iter()
        .filter(|c| *c.status() == ChainStatus::Running)
        .filter_map(|c| c.serves.as_ref())
        .collect();
    let horizon = |c: &Chain| c.serves.as_ref().and_then(|s| horizons.get(&s.objective)).copied();
    let mut q: Vec<&Chain> = chains.iter().filter(|c| *c.status() == ChainStatus::Queued).collect();
    q.sort_by(|a, b| {
        let served = |c: &Chain| c.serves.as_ref().is_some_and(|s| running.contains(s));
        b.kind
            .owner_waits()
            .cmp(&a.kind.owner_waits())
            .then(served(a).cmp(&served(b)))
            .then(match (horizon(a), horizon(b)) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then(a.created_at.cmp(&b.created_at))
    });
    q
}

/// The horizons of objectives (`Horizons`).
pub fn horizons_of(objectives: &[model::Objective]) -> Horizons {
    objectives
        .iter()
        .filter_map(|o| o.horizon.until.map(|t| (o.id.clone(), t)))
        .collect()
}
