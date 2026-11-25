//! Health says whether agents can actually run: on at least one account. It is not kept: it is what outages know, read
//! when it is asked for, so a turn that fails at the provider, a check and a probe all change it and nothing else needs
//! to. It also says how many stored records this build cannot read (ADR-0107), as the Tick last counted them. `claude auth status` reads local files only and reports a revoked login as signed in, so what is known of an
//! account is what a real one-line turn (a probe), or a card's own turn, said.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use model::{Account, Time};
use tokio_util::sync::CancellationToken;
use wire::app::{AccountHealth, Health};
use wire::runtime::{Failure, FailureCategory, Limit, LimitStatus};

use crate::accounts::Standing;
use crate::chain_core::Core;
use crate::chain_core::new_id;

/// Why what Remit starts on its own keeps the last room on the accounts for the owner's and the Assistant's turns
/// (ADR-0092): every account of h that can run agents is near a limit the provider warned of, each said with that limit
/// and how much of it is used. Empty while one has room, and while none can run agents (`no_agents` says that).
pub fn short_of_room(h: &Health) -> String {
    let able: Vec<&AccountHealth> = h.accounts.iter().filter(|a| a.ok).collect();
    if able.is_empty() || able.iter().any(|a| a.near_limit.is_empty()) {
        return String::new();
    }
    let each: Vec<String> = able
        .iter()
        .map(|a| {
            let name = if a.email.is_empty() { &a.id } else { &a.email };
            match a.utilization {
                Some(u) => format!("{name}: {} {:.0}% used", a.near_limit, u * 100.0),
                None => format!("{name}: {}", a.near_limit),
            }
        })
        .collect();
    format!("every account is near its limit ({})", each.join("; "))
}

/// A draw in [0, 1): what spreads retries and cooldowns (jitter).
pub type Draw = Arc<dyn Fn() -> f64 + Send + Sync>;

/// A uniform draw in [0, 1) from the OS's random source; 0.5 (no spread) were it to fail.
pub fn random_unit() -> f64 {
    let mut b = [0_u8; 4];
    if getrandom::fill(&mut b).is_err() {
        return 0.5;
    }
    f64::from(u32::from_le_bytes(b)) / 4_294_967_296.0
}

/// What is known of whether each account (by its id) can run agents: that it failed, and why, until it answers again.
/// It is the one owner of that fact: a card takes a turn only while some account is in use (unreachable), the scheduler
/// starts nothing while none is (admit), and the owner reads it as Health. An account that never failed is presumed to
/// work.
///
/// An account that failed is not used again when its cooldown ends, only asked: `claim_probes` says which, and a turn of
/// a line (probe) says whether it answers, so the owner's work does not pay for finding out.
///
/// It is a circuit breaker per account. A limit or a signed-out account opens it at once: they last, and each retry of
/// one is a failed turn. A service failure (overloaded, a server error) opens it only when the account's recent turns
/// fail at a rate (trips): one blip among turns it answers is not an outage, and the card that met it tries again
/// (retryService). A probe that fails keeps it open. Its cooldowns are spread (draw), so accounts and cards that failed
/// together are not all asked again at the same moment.
pub struct Outages {
    state: Mutex<OutageState>,
    /// Spreads each cooldown (spread); none: cooldowns are exact.
    draw: Option<Draw>,
}

#[derive(Default)]
struct OutageState {
    down: HashMap<String, Outage>,
    /// The accounts a probe is running on.
    probing: HashSet<String>,
    /// Each account's turns within `BREAKER_WINDOW`, oldest first: what trips weighs.
    turns: HashMap<String, Vec<TurnOutcome>>,
    /// When an account last answered or failed; none until one has.
    seen: Option<Time>,
    /// What `Health::rev` counts.
    changes: i64,
    /// What `Health::unreadable` says: the store's count when the Tick last read it.
    unreadable: u32,
    /// What the provider last said of each account's limit.
    limits: HashMap<String, Limit>,
}

/// Whether, at now, what the provider said of a limit says it is near: a warning past `NEAR_LIMIT` (or one that says no
/// use), or a rejection, until the limit resets.
fn near(l: &Limit, now: Time) -> bool {
    let warned = match l.status {
        LimitStatus::Allowed => false,
        LimitStatus::Warning => l.utilization.is_none_or(|u| u >= NEAR_LIMIT),
        LimitStatus::Rejected => true,
    };
    warned && l.resets_at.is_none_or(|at| at > now)
}

/// How much of a limit the provider warns of is used before the pool puts new work on an account with room instead
/// (`Standing::Near`). The provider warns early (a quarter of a week's limit), and a warning so far from the limit is no
/// reason to move: it orders work, it refuses none.
const NEAR_LIMIT: f64 = 0.9;

/// How one turn on an account ended: answered, or failed by the provider's service.
#[derive(Clone, Copy)]
struct TurnOutcome {
    at: Time,
    failed: bool,
}

struct Outage {
    /// When to ask the provider again.
    retry_at: Time,
    /// What it failed with.
    fault: Failure,
}

// The breaker opens on a service failure once, within BREAKER_WINDOW, at least BREAKER_FAILURES of an account's turns
// failed and they are at least half of its turns.
const BREAKER_WINDOW: Duration = Duration::from_secs(5 * 60);
const BREAKER_FAILURES: usize = 3;
/// The most turns kept of an account: a rate needs no more.
const BREAKER_TURNS: usize = 64;

/// How long a probe may take before it counts as failed.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// Whether turns, an account's turns within the window, open its breaker.
fn trips(turns: &[TurnOutcome]) -> bool {
    let failed = turns.iter().filter(|t| t.failed).count();
    failed >= BREAKER_FAILURES && failed.saturating_mul(2) >= turns.len()
}

/// d moved by a draw u in [0, 1) to between three quarters and five quarters of itself: jitter, so that what failed
/// together is not tried again together.
pub fn spread(d: Duration, u: f64) -> Duration {
    Duration::try_from_secs_f64(d.as_secs_f64() * (0.75 + 0.5 * u)).unwrap_or(d)
}

/// How long an account that failed with f is left alone before it is asked again. A service that is down comes back
/// soon; a limit or a signed-out account lasts, and each retry of one is a failed turn.
pub fn cooldown(f: &Failure) -> Duration {
    if f.category == FailureCategory::Service {
        Duration::from_secs(2 * 60)
    } else {
        Duration::from_secs(15 * 60)
    }
}

/// t, d later.
pub fn after(t: Time, d: Duration) -> Time {
    time::Duration::try_from(d)
        .ok()
        .and_then(|d| t.0.checked_add(d))
        .map_or(t, Time)
}

/// How long from `from` to `to`; zero when `to` is not later.
pub fn elapsed(from: Time, to: Time) -> Duration {
    let nanos =
        to.0.unix_timestamp_nanos()
            .saturating_sub(from.0.unix_timestamp_nanos());
    u64::try_from(nanos).map_or(Duration::ZERO, Duration::from_nanos)
}

impl Default for Outages {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            draw: Some(Arc::new(random_unit)),
        }
    }
}

impl Outages {
    fn state(&self) -> MutexGuard<'_, OutageState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records that a turn on account name failed with f at now, and says whether that changes what Health reads: a
    /// limit or a signed-out account opens the breaker, a service failure counts toward the rate that does (trips).
    pub fn fail(&self, name: &str, f: &Failure, now: Time) -> bool {
        let mut s = self.state();
        s.saw(now);
        if f.category == FailureCategory::Service && !s.record(name, true, now) {
            return false;
        }
        self.open(&mut s, name, f, now)
    }

    /// Records that a probe of account name failed with f at now: the breaker opens, or stays open for another
    /// cooldown. It says whether that changes what Health reads.
    pub fn trip(&self, name: &str, f: &Failure, now: Time) -> bool {
        let mut s = self.state();
        s.saw(now);
        self.open(&mut s, name, f, now)
    }

    /// Opens name's breaker for f's cooldown, spread, from now, and says whether that changes what Health reads. An
    /// outage already running longer is kept: a service blip does not end a limit.
    fn open(&self, s: &mut OutageState, name: &str, f: &Failure, now: Time) -> bool {
        s.probing.remove(name);
        let mut cool = cooldown(f);
        if let Some(d) = &self.draw {
            cool = spread(cool, d());
        }
        let retry_at = after(now, cool);
        let was = s.down.get(name).map(|cur| (cur.retry_at, cur.fault.title.clone()));
        if was.as_ref().is_some_and(|(at, _)| *at > retry_at) {
            return false;
        }
        s.down.insert(
            name.to_owned(),
            Outage {
                retry_at,
                fault: f.clone(),
            },
        );
        if was.is_some_and(|(_, title)| title == f.title) {
            return false;
        }
        s.changes = s.changes.saturating_add(1);
        true
    }

    /// Records that account name answered a turn at now: its outage is over. It says whether that changes what Health
    /// reads.
    pub fn up(&self, name: &str, now: Time) -> bool {
        let mut s = self.state();
        s.saw(now);
        s.record(name, false, now);
        s.probing.remove(name);
        if s.down.remove(name).is_none() {
            return false;
        }
        s.changes = s.changes.saturating_add(1);
        true
    }

    /// Whether a card may start on account name: it has not failed, or has answered since.
    pub fn usable(&self, name: &str) -> bool {
        !self.state().down.contains_key(name)
    }

    /// Where account name stands at now, for the pool: out of use, near its limit as the provider said, or with room.
    pub fn standing(&self, name: &str, now: Time) -> Standing {
        let s = self.state();
        if s.down.contains_key(name) {
            Standing::Out
        } else if s.limits.get(name).is_some_and(|l| near(l, now)) {
            Standing::Near
        } else {
            Standing::Free
        }
    }

    /// Records what the provider said of account name's limit at now, and says whether that changes what Health reads.
    pub fn limit(&self, name: &str, l: Limit, now: Time) -> bool {
        let mut s = self.state();
        s.saw(now);
        let shown = |r: Option<&Limit>| r.filter(|r| near(r, now)).cloned();
        let changed = shown(s.limits.get(name)) != shown(Some(&l));
        s.limits.insert(name.to_owned(), l);
        if changed {
            s.changes = s.changes.saturating_add(1);
        }
        changed
    }

    /// The failure that took account name out of use; none while it may be used.
    pub fn fault(&self, name: &str) -> Option<Failure> {
        self.state().down.get(name).map(|o| o.fault.clone())
    }

    /// The accounts that failed, whose cooldown has ended at now, and that no probe is running on, in name order. Each
    /// is marked as being probed, until its probe says whether it answers (up or fail).
    pub fn claim_probes(&self, now: Time) -> Vec<String> {
        let mut s = self.state();
        let mut names: Vec<String> = s
            .down
            .iter()
            .filter(|(name, o)| now >= o.retry_at && !s.probing.contains(*name))
            .map(|(name, _)| name.clone())
            .collect();
        names.sort();
        s.probing.extend(names.iter().cloned());
        names
    }

    /// Drops what is known of account name, which is removed: Health lists one account fewer, which is a change.
    pub fn forget(&self, name: &str) {
        let mut s = self.state();
        s.down.remove(name);
        s.turns.remove(name);
        s.probing.remove(name);
        s.limits.remove(name);
        s.changes = s.changes.saturating_add(1);
    }

    /// Records that the store now counts n records it cannot read, and says whether that changes what the owner reads.
    pub fn unreadable(&self, n: u32) -> bool {
        let mut s = self.state();
        if s.unreadable == n {
            return false;
        }
        s.unreadable = n;
        s.changes = s.changes.saturating_add(1);
        true
    }

    /// What the owner reads of accounts, each in its turn: agents can run while one is in use, and when none is, why
    /// not.
    pub fn health(&self, accounts: &[Account], now: Time) -> Health {
        let s = self.state();
        let mut failed = Vec::new();
        let list: Vec<AccountHealth> = accounts
            .iter()
            .map(|a| {
                let mut h = AccountHealth {
                    id: a.id.clone(),
                    kind: a.kind(),
                    email: a.email.clone(),
                    plugin: a.plugin.clone(),
                    connection: a.connection.clone(),
                    ok: true,
                    error: String::new(),
                    resets_at: None,
                    near_limit: String::new(),
                    utilization: None,
                };
                if let Some(o) = s.down.get(&a.id) {
                    failed.push(format!("{}: {}", a.label(), o.fault.title));
                    h.ok = false;
                    h.error = o.fault.title.clone();
                    h.resets_at = o.fault.resets_at;
                } else if let Some(l) = s.limits.get(&a.id).filter(|l| near(l, now)) {
                    h.near_limit = limit_words(&l.window);
                    h.utilization = l.utilization;
                    h.resets_at = l.resets_at;
                }
                h
            })
            .collect();
        let ok = failed.len() < accounts.len();
        Health {
            ok,
            error: if ok { String::new() } else { failed.join("; ") },
            accounts: list,
            checked_at: s.seen,
            unreadable: s.unreadable,
            rev: s.changes,
        }
    }
}

impl OutageState {
    fn saw(&mut self, now: Time) {
        if self.seen.is_none_or(|seen| now > seen) {
            self.seen = Some(now);
        }
    }

    /// Adds a turn on account name that ended at now, failed or answered, and says whether the account's turns now trip
    /// its breaker. Turns older than the window are dropped.
    fn record(&mut self, name: &str, failed: bool, now: Time) -> bool {
        let turns = self.turns.entry(name.to_owned()).or_default();
        turns.push(TurnOutcome { at: now, failed });
        turns.retain(|t| elapsed(t.at, now) < BREAKER_WINDOW);
        let extra = turns.len().saturating_sub(BREAKER_TURNS);
        turns.drain(..extra);
        trips(turns)
    }
}

/// Receives a probe's one turn: it keeps what the agent says, and asks for nothing.
#[derive(Default)]
struct ProbeHandler {
    text: Mutex<String>,
}

impl ProbeHandler {
    fn text(&self) -> String {
        self.text.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl agent::Handler for ProbeHandler {
    fn on_update(&self, u: agent::Update) {
        if let agent::Update::MessageChunk(c) = u {
            self.text.lock().unwrap_or_else(PoisonError::into_inner).push_str(&c);
        }
    }

    fn on_permission(
        &self,
        _req: wire::runtime::PermissionRequest,
    ) -> agent::BoxFuture<'_, wire::runtime::PermissionAnswer> {
        Box::pin(async {
            wire::runtime::PermissionAnswer::Deny {
                message: "A probe asks for nothing: it only answers.".to_owned(),
            }
        })
    }

    fn on_init(&self, _init: model::SessionInit) {}

    /// Its session ends with its one turn, and its tasks with it.
    fn on_task(&self, _t: wire::runtime::Task) {}

    fn on_working(&self, _working: bool) {}

    fn on_exit(&self, _end: Option<agent::Exit>) {}
}

impl Core {
    /// Whether agents can run, as is known now.
    pub fn health(&self) -> Health {
        self.outages.health(&self.account_list(), self.now())
    }

    /// Why the provider cannot run agents now, in what it failed with; empty while it can. It is what admission weighs
    /// (room), so the scheduler starts nothing while it is not empty.
    pub fn no_agents(&self) -> String {
        let h = self.health();
        if h.ok { String::new() } else { h.error }
    }

    /// Why what the scheduler starts on its own waits so that the owner's and the Assistant's turns keep the last room on
    /// the accounts (`short_of_room`); empty while an account has room. Admission weighs it beside `no_agents`.
    pub fn no_headroom(&self) -> String {
        short_of_room(&self.health())
    }

    /// Tells the channels what Health reads now.
    fn health_changed(&self) {
        self.notify.health_updated(&self.health());
    }

    /// Records how many records the store found it cannot read, and tells the channels when that moved. The Tick calls
    /// it, having read every chain.
    pub fn count_unreadable(&self) {
        if self.outages.unreadable(self.store.unreadable()) {
            self.health_changed();
        }
    }

    /// Records that account name failed with f, and tells the channels when that changes what the owner reads.
    pub fn account_failed(&self, name: &str, f: &Failure) {
        if self.outages.fail(name, f, self.now()) {
            self.health_changed();
        }
    }

    /// Records that the probe of account name failed with f, and tells the channels when that changes what the owner
    /// reads.
    pub fn probe_failed(&self, name: &str, f: &Failure) {
        if self.outages.trip(name, f, self.now()) {
            self.health_changed();
        }
    }

    /// Records what the provider said of account name's limit, and tells the channels when that changes what the owner
    /// reads.
    pub fn account_limit(&self, name: &str, l: wire::runtime::Limit) {
        if self.outages.limit(name, l, self.now()) {
            self.health_changed();
        }
    }

    /// Records that account name answered, and tells the channels when that changes what the owner reads.
    pub fn account_up(&self, name: &str) {
        if self.outages.up(name, self.now()) {
            self.health_changed();
        }
    }

    /// Asks every account for one turn on its fastest model, together, and says whether agents can run: while one
    /// answers. Cancelling stop cuts the probes short, and they decide nothing.
    pub async fn check_agents(&self, stop: &CancellationToken) -> Health {
        if let Some(me) = self.me() {
            let mut asking = tokio::task::JoinSet::new();
            for a in self.account_list() {
                let (me, stop) = (Arc::clone(&me), stop.clone());
                asking.spawn(async move { me.probe_account(&stop, &a.id).await });
            }
            while asking.join_next().await.is_some() {}
        }
        self.health()
    }

    /// Asks each account that failed, once its cooldown has ended, whether it answers. It does so in the background: a
    /// turn can take minutes, and the Tick that asked must not.
    pub fn probe_due(&self, now: Time) {
        let Some(me) = self.me() else { return };
        for id in self.outages.claim_probes(now) {
            let me = Arc::clone(&me);
            self.background(move |work| async move { me.probe_account(&work, &id).await });
        }
    }

    /// Runs one probe on the account's fastest model and records whether it answered. What waited for an account
    /// starts at once (kick) rather than at the next Tick. A probe cut short (stop) decides nothing.
    pub async fn probe_account(&self, stop: &CancellationToken, account: &str) {
        let model = self.probe_model();
        let answered = tokio::select! {
            () = stop.cancelled() => return,
            r = self.probe(account, &model) => r,
        };
        match answered {
            Ok(()) => self.account_up(account),
            Err(err) => {
                let f = match err {
                    agent::Error::Failure(f) => f,
                    other => Failure {
                        category: FailureCategory::Service,
                        title: other.to_string(),
                        resets_at: None,
                    },
                };
                tracing::error!(account, err = %f, "agent health check failed");
                self.probe_failed(account, &f);
            }
        }
        self.kick();
    }

    /// The model an account is checked on: the fastest. With no catalog it is haiku, as it always was.
    fn probe_model(&self) -> String {
        self.cfg
            .models
            .as_ref()
            .and_then(provider::Catalog::fastest)
            .map_or_else(|| "haiku".to_owned(), |m| m.id.clone())
    }

    /// One turn of a line on account and model, in a scratch directory: whether the agent answers.
    async fn probe(&self, account: &str, model: &str) -> Result<(), agent::Error> {
        let dir = std::env::temp_dir().join(new_id("remit-probe-"));
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| agent::Error::Prepare(format!("the probe's directory: {e}")))?;
        let text = tokio::time::timeout(
            PROBE_TIMEOUT,
            self.run_once(
                "probe",
                account,
                &dir.to_string_lossy(),
                model,
                "Reply with exactly: ok",
            ),
        )
        .await;
        // a scratch directory: one left behind is cleared with the temp dir
        tokio::fs::remove_dir_all(&dir).await.ok();
        let text = text.map_err(|_| agent::Error::TurnFailed("the probe had no answer within 2m".to_owned()))??;
        if text.is_empty() {
            return Err(agent::Error::TurnFailed("the agent answered with nothing".to_owned()));
        }
        Ok(())
    }

    /// Runs one turn in a throwaway session outside any chain, on account, and returns what the agent said.
    async fn run_once(
        &self,
        id: &str,
        account: &str,
        dir: &str,
        model: &str,
        prompt: &str,
    ) -> Result<String, agent::Error> {
        let h = Arc::new(ProbeHandler::default());
        let spec = agent::Spec {
            card_id: id.to_owned(),
            cwd: dir.to_owned(),
            model: model.to_owned(),
            account: account.to_owned(),
            ..agent::Spec::default()
        };
        let sess = self.open_session(spec, h.clone()).await?;
        let answered = sess.prompt(prompt.into()).await;
        sess.close();
        answered?;
        Ok(h.text())
    }
}

/// A provider's limit window in the owner's words; a warning that names no window is still a limit he should see.
fn limit_words(window: &str) -> String {
    match window {
        "five_hour" => "5-hour limit".to_owned(),
        "seven_day" => "Weekly limit".to_owned(),
        "seven_day_opus" => "Weekly Opus limit".to_owned(),
        "" => "Usage limit".to_owned(),
        other => {
            let words = other.replace('_', " ");
            let mut c = words.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        }
    }
}
