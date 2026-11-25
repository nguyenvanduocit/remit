//! The scheduler keeps work flowing without the owner: queued chains start, as admission lets them.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use model::{Card, CardStatus, Chain, ChainOrigin, ChainStatus, DecisionKind, InboxKind, Time, Unjudged};

use crate::admit::{Horizons, queue_of};
use crate::cap_names::CAP_PROPOSE;
use crate::chain_core::Core;
use crate::health::elapsed;
use crate::inbox::{Cause, Failed};
use crate::outbox::by_remit;
use crate::session::{Run, RunState};
use crate::spin::{Spin, tally};
use crate::step::FinishCmd;
use crate::submit::submitting;
use wire::app::{CheckBack, Workload};

/// w with the chains counted: how many run, are queued and wait, and the earliest wake of those that wait.
fn count_chains(mut w: Workload, chains: &[Chain], wakes: &HashMap<String, CheckBack>) -> Workload {
    for c in chains {
        match c.status() {
            ChainStatus::Running => w.running = w.running.saturating_add(1),
            ChainStatus::Queued => w.queued = w.queued.saturating_add(1),
            ChainStatus::Waiting => {
                w.waiting = w.waiting.saturating_add(1);
                if let Some(wake) = wakes.get(&c.id)
                    && w.next_check_back.is_none_or(|next| wake.at < next)
                {
                    w.next_check_back = Some(wake.at);
                }
            }
            _ => {}
        }
    }
    w
}

/// When t's local day began, in t's offset: the server's (the injected clock gives times in it).
pub fn local_day_start(t: Time) -> Time {
    Time(t.0.replace_time(time::Time::MIDNIGHT))
}

/// The queued chains the scheduler may start now, in the queue's order (`queue_of`, over the objectives' horizons).
/// Admission takes them while it has room; a chain the owner waits on takes no place in the limit, so it starts even
/// when the limit is full.
pub fn ready_to_run(chains: &[Chain], horizons: &Horizons) -> Vec<String> {
    queue_of(chains, horizons).into_iter().map(|c| c.id.clone()).collect()
}

/// Whether a card's agent is in a turn, starting one, or waiting on a permission in one.
pub fn at_work(c: &Card) -> bool {
    matches!(
        c.status(),
        CardStatus::Running | CardStatus::Starting | CardStatus::Permission
    )
}

impl Core {
    /// Why a turn that sent nothing for Config.turn_idle was cut short.
    pub fn silence(&self) -> String {
        format!(
            "the turn was cut short by Remit: the agent had sent nothing for {}",
            seconds_text(self.cfg.turn_idle)
        )
    }

    /// Counts the workload of chains, keeps it with now as when the last Tick got through, and tells the channels.
    pub fn count_workload(&self, chains: &[Chain], wakes: &HashMap<String, CheckBack>, now: Time) {
        let cap = self.capacity();
        let mut w = count_chains(
            Workload {
                capacity: cap.limit.unwrap_or(1),
                pressure: cap.pressure,
                ..Workload::default()
            },
            chains,
            wakes,
        );
        {
            let mut sh = self.shared();
            w.rev = sh.workload.rev.saturating_add(1);
            sh.workload = w.clone();
            sh.last_tick = Some(now);
        }
        self.notify.workload_updated(&w);
    }

    /// The workload as the last Tick counted it.
    pub fn workload(&self) -> Workload {
        self.shared().workload.clone()
    }

    /// When the last Tick got through, none before the first. A Tick that could not read its chains does not count, so a
    /// health probe sees a scheduler that cannot read them as stalled.
    pub fn last_tick(&self) -> Option<Time> {
        self.shared().last_tick
    }

    /// Asks the scheduler for a Tick, after a change that may let a chain start (a slot freed, a chain queued). It never
    /// blocks, and a kick still waiting stands for this one: changes made while a Tick runs fold into one more.
    pub fn kick(&self) {
        self.kicks.notify_one();
    }
}

// A running chain must always have something moving. When nothing has for STALL_AFTER, its Chair is told to move it on;
// after MAX_NUDGES such reminders with nothing moving in between, the chain is finished so it frees its slot. A chain
// whose agents keep taking turns that produce nothing is reminded and finished the same way (spin.rs).
const STALL_AFTER: Duration = Duration::from_secs(10 * 60);
pub(crate) const MAX_NUDGES: i64 = 3;

/// Whether nothing in a chain has moved for quiet: no card is working, besides those silent in their turn (`silent`), and
/// none has changed since.
fn stalled(cards: &[Card], silent: &HashSet<String>, now: Time, quiet: Duration) -> bool {
    if cards.iter().any(|c| at_work(c) && !silent.contains(&c.id)) {
        return false;
    }
    cards
        .iter()
        .map(|c| c.updated_at)
        .max()
        .is_some_and(|last| elapsed(last, now) >= quiet)
}

/// The tools that can move a chain on: land only once it has a repository, propose_chain only for a chain that may
/// propose.
fn next_moves(ch: &Chain) -> Vec<&'static str> {
    let mut moves = vec!["route work", "hire"];
    if !ch.repos.is_empty() {
        moves.push("land");
    }
    if ch.capabilities.iter().any(|c| c == CAP_PROPOSE) {
        moves.push("propose_chain");
    }
    moves
}

/// The agents at work besides the Chair: the Chair answering reminders is not progress.
fn workers<'a>(cards: &'a [Card], chair_id: &str) -> Vec<&'a str> {
    cards
        .iter()
        .filter(|c| c.id != chair_id && at_work(c))
        .map(|c| c.id.as_str())
        .collect()
}

// The liveness rail: an agent in a turn keeps sending its session's updates (its words, its tool calls and their
// progress), even through a long tool. One that has sent nothing for Config.turn_idle is taken for wedged, its process or
// its connection hung, and would otherwise hold its card running, and its chain's slot, for ever. Its turn is cancelled;
// a session still in the turn a grace later is closed. Either way the turn ends failed, which the owner reads in his
// inbox, with a retry, unless the agent ended it on its own as the cancel was on its way (`ended_by_silence`). The clock
// is the time the agent was last heard from, not the turn's length: a turn may last hours. A paused chain is left alone.

/// How long a turn cut short for silence has to end before its session is closed: a live agent ends a cancelled turn
/// within seconds, and the scheduler looks once a minute.
const SILENCE_GRACE: Duration = Duration::from_secs(60);

/// Whether an agent last heard from at heard has sent nothing for limit by now. A zero limit turns the rail off.
fn silent(heard: Time, now: Time, limit: Duration) -> bool {
    !limit.is_zero() && elapsed(heard, now) >= limit
}

/// What the liveness rail does with a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SilenceCut {
    /// The agent was heard from lately, or its turn was cut short less than a grace ago.
    Keep,
    /// The agent has been silent for the limit: its turn is cancelled.
    Cut,
    /// Its turn, cancelled a grace ago, has not ended: its session is closed.
    Close,
}

/// What the turn of an agent last heard from at heard calls for by now, limit being how long it may be silent; cut is
/// when the rail cut the turn short, none when it has not.
fn silence_cut_of(heard: Time, cut: Option<Time>, now: Time, limit: Duration) -> SilenceCut {
    match cut {
        None if silent(heard, now, limit) => SilenceCut::Cut,
        Some(cut) if elapsed(cut, now) >= SILENCE_GRACE => SilenceCut::Close,
        _ => SilenceCut::Keep,
    }
}

/// Whether r's agent is in a turn in which it owes its client word of its work: a turn waiting on a permission's answer
/// waits on the client. s is r's state, held.
fn owes_word(r: &Run, s: &RunState) -> bool {
    s.sess.is_some() && (s.prompt != 0 || s.own.is_some()) && s.permissions.is_empty() && !r.mailbox.is_closed()
}

/// Weighs r's turn by now (`silence_cut_of`) and marks what it decides, under the run's lock, so that a turn that ends
/// meanwhile is not cut, and the end of the turn is recorded once: a cut turn is marked, and its end, recorded by the
/// card's loop, is a failure; a session to close has its mailbox closed, so the loop records nothing of the turn and the
/// rail does, and the id given is the message the turn was on.
fn silence_cut(r: &Run, now: Time, limit: Duration) -> (SilenceCut, i64) {
    let mut s = r.state();
    if !owes_word(r, &s) {
        return (SilenceCut::Keep, 0);
    }
    let cut = silence_cut_of(s.heard, s.silenced, now, limit);
    let mut on = 0;
    match cut {
        SilenceCut::Cut => {
            s.silenced = Some(now);
            if let Some(own) = &s.own {
                own.silence();
            }
        }
        SilenceCut::Close => {
            on = s.own.as_ref().map_or(s.prompt, |own| own.opening_id());
            r.mailbox.close();
        }
        SilenceCut::Keep => {}
    }
    (cut, on)
}

impl Core {
    /// The cards whose agent is silent in its turn by now: they are at work, but not the chain's progress.
    fn silent_cards(&self, now: Time) -> HashSet<String> {
        self.all_runs()
            .into_iter()
            .filter_map(|r| {
                let s = r.state();
                (owes_word(&r, &s) && silent(s.heard, now, self.cfg.turn_idle)).then(|| s.card.id.clone())
            })
            .collect()
    }

    /// Applies the liveness rail to every agent's turn by now, but those of paused chains: a pause is the owner's rail, and
    /// what is left of a chain he paused waits for him.
    pub async fn cut_silent_turns(&self, chains: &[Chain], now: Time) {
        let Some(me) = self.me() else { return };
        let paused: HashSet<&str> = chains
            .iter()
            .filter(|c| *c.status() == ChainStatus::Paused)
            .map(|c| c.id.as_str())
            .collect();
        for r in self.all_runs() {
            if paused.contains(r.home.as_str()) {
                continue;
            }
            match silence_cut(&r, now, self.cfg.turn_idle) {
                (SilenceCut::Cut, _) => {
                    let o = Arc::clone(&me);
                    self.background(move |_| async move {
                        o.cut_turn_of(&r, &model::Cut::SilentTurn).await;
                    });
                }
                (SilenceCut::Close, on) => me.close_silent_session(r, on).await,
                (SilenceCut::Keep, _) => {}
            }
        }
    }

    /// Ends the turn on message on of run r, whose mailbox the rail closed, as failed, then closes its session. The end is
    /// recorded within the Tick, before its stall rail sends the card a reminder that would open the card's next session;
    /// closing the process, which may wait on the wedged agent, is left to the background.
    async fn close_silent_session(self: &Arc<Self>, r: Arc<Run>, on: i64) {
        let why = format!(
            "{}; it did not end the turn when cancelled, so Remit closed its session",
            self.silence()
        );
        match self.store.read(move |s| s.event(on)).await {
            Ok(e) => {
                self.end_open_calls(&r, &e, Some(&why)).await;
                self.end_failed_turn(&r, &e, &Failed::new(Cause::Silent, why), None)
                    .await;
            }
            Err(err) => tracing::error!(home = %r.home, card = %r.card_id(), %err, "end a silent turn"),
        }
        let o = Arc::clone(self);
        self.background(move |_| async move {
            o.end_run(&r, &model::Cut::SilentSession).await;
        });
    }

    /// Reminds the Chair of each running chain where nothing has moved for `STALL_AFTER` to move it on, and finishes one
    /// still standing after `MAX_NUDGES` reminders. A chain waiting on the owner's answer is not stalled, nor one with an
    /// agent waiting for a provider: the Chair cannot make capacity, and the agent goes on by itself once a provider can
    /// run it. An agent's failure waiting in the inbox is not that: the Chair can see it and move on (route, restart,
    /// finish), so it is nudged like any chain where nothing moves. A chain whose agents took `spin_turns` turns with nothing
    /// produced is reminded on the same path, whether they are at work or not (spin.rs).
    pub async fn unstick(&self, chains: &[Chain], now: Time) {
        let items = match self.store.read(|r| r.open_inbox()).await {
            Ok(items) => items,
            Err(err) => {
                tracing::error!(%err, "stall rail");
                return;
            }
        };
        let waiting: HashSet<&str> = items
            .iter()
            .filter(|it| matches!(it.kind, InboxKind::Question | InboxKind::Permission | InboxKind::Change))
            .map(|it| it.home.as_str())
            .collect();
        let silent = self.silent_cards(now);
        for ch in chains {
            if *ch.status() != ChainStatus::Running {
                self.shared().spins.remove(&ch.id); // a wait or a pause: the count starts again when it runs
                continue;
            }
            if ch.chair_id.is_empty() || waiting.contains(ch.id.as_str()) {
                continue;
            }
            let id = ch.id.clone();
            let Ok((cards, decisions)) = self
                .store
                .read(move |r| Ok::<_, store::Error>((r.cards(&id)?, r.decisions(&id)?)))
                .await
            else {
                continue;
            };
            if cards.iter().any(|c| c.status() == &CardStatus::Waiting) {
                continue;
            }
            let now_tally = tally(&cards, &decisions);
            let due = {
                let mut sh = self.shared();
                let spin = sh.spins.entry(ch.id.clone()).or_insert_with(|| Spin::new(now_tally));
                spin.saw(now_tally, self.cfg.spin_turns)
            };
            if let Some(due) = due {
                self.remind(ch, due.n, Standing::Spinning(due.idle)).await;
                continue;
            }
            let working = workers(&cards, &ch.chair_id).iter().any(|id| !silent.contains(*id));
            if submitting(ch) || working {
                self.shared().nudges.remove(&ch.id); // real progress: the reminders start over
                continue;
            }
            if !stalled(&cards, &silent, now, STALL_AFTER) {
                continue;
            }
            let n = {
                let mut sh = self.shared();
                let n = sh.nudges.entry(ch.id.clone()).or_default();
                *n = n.saturating_add(1);
                *n
            };
            self.remind(ch, n, Standing::Still).await;
        }
    }

    /// Sends the Chair of ch, which stands as standing says, reminder n of `MAX_NUDGES` to move it on, or finishes the
    /// chain past the last: the one path both the stall and the no-progress rails take.
    async fn remind(&self, ch: &Chain, n: i64, standing: Standing) {
        if n > MAX_NUDGES {
            let cmd = FinishCmd {
                summary: format!(
                    "Remit finished this chain because {} after {MAX_NUDGES} nudges to the Chair.\n\n{}",
                    standing.why(),
                    ch.summary
                ),
                text: "Finish chain: no progress after repeated nudges.".to_owned(),
                card_id: ch.chair_id.clone(),
                by: by_remit(),
                kind: Some(DecisionKind::Stall),
                unjudged: Some(Unjudged::Stalled),
                ..FinishCmd::default()
            };
            if let Err(err) = self.finish_chain(&ch.id, cmd, crate::apply::Records::default()).await {
                tracing::error!(chain = %ch.id, %err, "finish stalled chain");
            }
            return;
        }
        let mut msg = format!(
            "[Remit: {} (reminder {n} of {MAX_NUDGES})] {}Check list_agents and move the chain on: {}, or finish_chain with what is done and what is left. If nothing moves after the last reminder, Remit finishes the chain.",
            standing.why(),
            standing.advice(),
            next_moves(ch).join(", ")
        );
        let watches = self.watch_list(&ch.id).await;
        if !watches.is_empty() {
            msg.push_str(&format!(
                "\nStill running in the background: {}. That is not progress. subscribe frees the chain's slot until the time you give: the sessions close and with them what they run in the background.",
                watches.join("; ")
            ));
        }
        if let Err(err) = self.deliver(&ch.chair_id, standing.title(), &msg, by_remit()).await {
            tracing::warn!(chain = %ch.id, %err, "nudge stalled chain");
        }
    }
}

/// How a chain the Chair is reminded of stands.
#[derive(Debug, Clone, Copy)]
enum Standing {
    /// Nothing in it has moved for `STALL_AFTER`.
    Still,
    /// Its agents took this many turns and produced nothing (spin.rs).
    Spinning(i64),
}

impl Standing {
    /// What Remit saw, in words.
    fn why(self) -> String {
        match self {
            Self::Still => format!(
                "nothing in this chain has moved for {} minutes",
                STALL_AFTER.as_secs() / 60
            ),
            Self::Spinning(turns) => format!(
                "the agents of this chain took {turns} turns and produced nothing: no step of a plan completed, no work accepted, no verdict"
            ),
        }
    }

    /// What the Chair is to look at first.
    fn advice(self) -> &'static str {
        match self {
            Self::Still => "",
            Self::Spinning(_) => {
                "An agent going round in circles needs another way: read its last report, then give it a narrower brief, hire another, or stop_agent. "
            }
        }
    }

    /// The reminder's title, as the Chair's stream shows it.
    fn title(self) -> &'static str {
        match self {
            Self::Still => "Nudged for standing still",
            Self::Spinning(_) => "Nudged for going round in circles",
        }
    }
}

/// d in whole seconds as a duration string: "10m0s", "1h0m0s", "45s".
pub fn seconds_text(d: std::time::Duration) -> String {
    let s = d.as_secs();
    match (s / 3600, s % 3600 / 60, s % 60) {
        (0, 0, s) => format!("{s}s"),
        (0, m, s) => format!("{m}m{s}s"),
        (h, m, s) => format!("{h}h{m}m{s}s"),
    }
}

/// Whether chain ch is a verification chain. It is the one place the two rails of one read the origin: it has no goal
/// judge, and it opens no chain, so no kind or agent lifts either.
pub fn verifying(ch: &Chain) -> bool {
    ch.origin == ChainOrigin::Verification
}
