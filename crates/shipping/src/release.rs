//! Releases (ADR-0116), the rule, pure: what a repository's procedure in `.remit/release/` is, which landed commit is
//! released next and what it carries, what a step's exit says, what a release bakes under and when that says it hurts,
//! and what a step's environment holds. The shell (release_run.rs) reads the store and the clone, runs the steps and
//! records what this rule says; the tests run the rule without a mock.
//!
//! A release is Remit's code, after every landing, one at a time per repository: no agent and no person is asked
//! whether to release. What is released is decided in the commit (a version bump is part of the change its gates
//! judged); the procedure is a rail (gate.rs `RELEASE_DIR`), written by chains and judged with `security`.

use model::{Duration, Landing, Release, ReleaseGuard, ReleaseState, SignalKey, Time};
use serde::Deserialize;

use crate::gate::RELEASE_DIR;
use crate::landing::{Verdict, verdict_on};

/// What a release carries of the chains' own work: its landings but the reverts (a revert undoes a release, it is no
/// chain's change) and the chains they belong to, each once. Who a release tells, marks live, traces and reports to
/// Datadog is this and nothing else.
#[derive(Debug, Default)]
pub struct Own<'a> {
    pub landings: Vec<&'a Landing>,
    pub chains: std::collections::BTreeSet<&'a str>,
}

/// What release `id` carries of the chains' own work, of `landings`. Pure.
pub fn own_of<'a>(landings: &'a [Landing], id: &str) -> Own<'a> {
    let landings: Vec<&Landing> = landings
        .iter()
        .filter(|l| l.release.as_deref() == Some(id) && l.reverts.is_none())
        .collect();
    let chains = landings.iter().map(|l| l.chain_id.as_str()).collect();
    Own { landings, chains }
}

/// The file naming the connections a procedure's steps may reach, one `<source>/<connection>` a line.
pub const CONNECTIONS_FILE: &str = "connections";
/// The file naming the guardrail every release of the repository bakes under, one line
/// `<signal key> <bake minutes> <breach condition>`.
pub const GUARD_FILE: &str = "guard";
/// The directory of the steps run when a release is taken back.
pub const BACK_DIR: &str = "back/";
/// The exit of a step that says the outside was not reachable (`EX_TEMPFAIL`): tried again, nothing taken back.
pub const TEMPFAIL: i32 = 75;
/// How long after it landed a landing whose push and base the forge has said nothing of is taken as one on a repository
/// whose forge runs no pipeline: a pipeline says it is pending within seconds of a push.
pub const NO_PIPELINE_AFTER: time::Duration = time::Duration::minutes(10);

/// One connection a procedure's steps may reach: a source and the name of one of its connections.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Connection {
    pub source: String,
    pub connection: String,
}

impl Connection {
    /// As the connections file and list_sources name it.
    pub fn place(&self) -> String {
        format!("{}/{}", self.source, self.connection)
    }
}

/// A repository's release procedure, as declared at one commit: its steps and its way back in name order (paths under
/// `.remit/release/`), the connections they reach and the guard every release bakes under.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Procedure {
    pub steps: Vec<String>,
    pub back: Vec<String>,
    pub connections: Vec<Connection>,
    pub guard: Option<ReleaseGuard>,
}

/// The procedure the files under `.remit/release/` declare: entries are every file there, by path from the repository's
/// root, with whether it is executable; connections and guard are the text of those two files ("" when absent). A step
/// is an executable file directly under the directory, a way back one under `back/`; any other file is data a step
/// reads. None when the directory declares no step: the repository has no release. Pure.
pub fn procedure(entries: &[(String, bool)], connections: &str, guard: &str) -> Result<Option<Procedure>, String> {
    let mut p = Procedure::default();
    for (path, executable) in entries {
        let Some(rest) = path.strip_prefix(RELEASE_DIR) else {
            continue;
        };
        if !executable || rest == CONNECTIONS_FILE || rest == GUARD_FILE {
            continue;
        }
        match rest.strip_prefix(BACK_DIR) {
            Some(name) if !name.contains('/') => p.back.push(path.clone()),
            Some(_) => {}
            None if !rest.contains('/') => p.steps.push(path.clone()),
            None => {}
        }
    }
    if p.steps.is_empty() {
        return Ok(None);
    }
    p.steps.sort();
    p.back.sort();
    p.connections = connections_of(connections)?;
    p.guard = guard_of(guard)?;
    Ok(Some(p))
}

/// The lines of text without comments (`#` on) and blanks.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(|l| l.split_once('#').map_or(l, |(before, _)| before).trim())
        .filter(|l| !l.is_empty())
}

/// The connections file: one `<source>/<connection>` a line. Pure.
fn connections_of(text: &str) -> Result<Vec<Connection>, String> {
    let mut out = Vec::new();
    for line in lines(text) {
        let Some((source, connection)) = line.split_once('/').filter(|(s, c)| {
            let name = |x: &str| !x.is_empty() && x.chars().all(|ch| ch.is_ascii_alphanumeric() || "-_.".contains(ch));
            name(s) && name(c)
        }) else {
            return Err(format!(
                "{RELEASE_DIR}{CONNECTIONS_FILE}: {line:?} is not <source>/<connection>, as list_sources names them"
            ));
        };
        let c = Connection {
            source: source.to_owned(),
            connection: connection.to_owned(),
        };
        if !out.contains(&c) {
            out.push(c);
        }
    }
    Ok(out)
}

/// The guard file: one line `<signal key> <bake minutes> <breach condition>`; none when it says nothing. Pure.
fn guard_of(text: &str) -> Result<Option<ReleaseGuard>, String> {
    let mut found = lines(text);
    let Some(line) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(format!(
            "{RELEASE_DIR}{GUARD_FILE}: one line, the guardrail every release bakes under"
        ));
    }
    let bad = |why: &str| format!("{RELEASE_DIR}{GUARD_FILE}: {line:?} {why}");
    let mut parts = line.splitn(3, char::is_whitespace);
    let (Some(key), Some(minutes), Some(breach)) = (parts.next(), parts.next(), parts.next().map(str::trim)) else {
        return Err(bad("is not <signal key> <bake minutes> <breach condition>"));
    };
    let minutes: i64 = minutes
        .parse()
        .ok()
        .filter(|m| (1..=7 * 24 * 60).contains(m))
        .ok_or_else(|| bad("names no bake of 1 minute to a week"))?;
    signal::check(breach).map_err(|e| bad(&format!("has a breach condition that does not parse: {e}")))?;
    Ok(Some(ReleaseGuard {
        signal: SignalKey(key.to_owned()),
        breach: breach.to_owned(),
        bake: Duration(
            time::Duration::minutes(minutes)
                .whole_nanoseconds()
                .try_into()
                .unwrap_or(i64::MAX),
        ),
    }))
}

/// The landings of landings (one repository's, one base's) on their base that no release carried to its customer yet,
/// oldest first. A revert is one: taking a change back reaches the customer by a release too. Pure.
pub fn unreleased(landings: &[Landing]) -> Vec<&Landing> {
    let mut out: Vec<&Landing> = landings.iter().filter(|l| l.on_base() && l.release.is_none()).collect();
    out.sort_by(|a, b| (a.landed_at, a.created_at, &a.id).cmp(&(b.landed_at, b.created_at, &b.id)));
    out
}

/// Whether the forge has settled on l's push so it may be released at now: its checks passed, or failed where the base
/// before it was red already (so the red is not its doing), or the forge has said nothing of it or its base for
/// `NO_PIPELINE_AFTER` (a repository with no pipeline). A push being reverted, or still pending, is not. Pure.
pub fn settled(l: &Landing, now: Time) -> bool {
    match verdict_on(l.checks.as_ref(), l.base_checks.as_ref()) {
        Verdict::Stands => true,
        Verdict::Revert => false,
        Verdict::Wait => {
            l.checks.is_none()
                && l.base_checks.is_none()
                && l.landed_at
                    .is_some_and(|at| at.0.saturating_add(NO_PIPELINE_AFTER) <= now.0)
        }
    }
}

/// What the next release of a repository is: the landing whose commit it releases and every landing it carries, oldest
/// first, ending with that one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due<'a> {
    pub top: &'a Landing,
    pub carried: Vec<&'a Landing>,
}

/// The next release of a repository, of its landings (`all`, every one of its base's) and its releases (newest first):
/// none while one of them is open (it releases or bakes, so one runs at a time); none while the last taken back still
/// has a landing on the base (its reverts land first, so nothing re-publishes what it took back); otherwise at the
/// k-th oldest unreleased landing (at the newest when at_head: the first release of the repository, or the first once a
/// pause is lifted), at least past what a release that published nothing looked at and up to every revert of what the
/// last taken back carried, and at the newest settled one at or below that (`settled`). Pure.
pub fn next_release<'a>(
    all: &'a [Landing],
    releases: &[Release],
    k: usize,
    at_head: bool,
    now: Time,
) -> Option<Due<'a>> {
    if releases.iter().any(Release::open) {
        return None;
    }
    let pending = unreleased(all);
    let last = pending.len().checked_sub(1)?;
    let mut from = 0;
    let mut head = at_head || releases.is_empty();
    match releases.first() {
        Some(r) if r.state == ReleaseState::Unchanged => {
            if let Some(i) = pending.iter().position(|l| l.id == r.top) {
                from = i.checked_add(1)?;
            }
        }
        Some(r) if matches!(r.state, ReleaseState::TakenBack { .. }) => {
            let carried: Vec<&Landing> = all
                .iter()
                .filter(|l| l.release.as_deref() == Some(r.id.as_str()))
                .collect();
            if carried.iter().any(|l| l.on_base() && l.reverts.is_none()) {
                return None;
            }
            let reverts = pending
                .iter()
                .rposition(|l| l.reverted_one().is_some_and(|t| carried.iter().any(|c| c.id == t)));
            if let Some(i) = reverts {
                from = i;
            }
            head = head || reverts.is_none();
        }
        _ => {}
    }
    if from > last {
        return None;
    }
    let want = if head {
        last
    } else {
        from.saturating_add(k.max(1).saturating_sub(1)).min(last)
    };
    let top = (from..=want)
        .rev()
        .find(|&i| pending.get(i).is_some_and(|l| settled(l, now)))?;
    let carried: Vec<&Landing> = pending.get(..=top)?.to_vec();
    Some(Due {
        top: carried.last()?,
        carried,
    })
}

/// The landings a release that never published re-aims at, of all (its base's): every one no release carries up to the
/// newest the forge settled, oldest first; empty while none is settled. A release whose procedure does not parse waits
/// on them, so the change that fixes it is released without anyone asked. Pure.
pub fn reaim(all: &[Landing], now: Time) -> Vec<&Landing> {
    let pending = unreleased(all);
    pending
        .iter()
        .rposition(|l| settled(l, now))
        .and_then(|top| pending.get(..=top))
        .map(<[&Landing]>::to_vec)
        .unwrap_or_default()
}

/// What a step's run says of the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// It exited 0: the next step runs.
    Passed,
    /// The outside was not reachable (exit 75), or Remit could not run it (no grant, no proxy): the release is tried
    /// again at the next sync and nothing is taken back.
    Retry,
    /// Any other failure, a step that ran past its deadline included: the release is breached.
    Breach,
}

/// The outcome of a step that exited with exit (none when Remit could not run it) and ran past its deadline or not.
/// Pure.
pub const fn step_outcome(exit: Option<i32>, timed_out: bool) -> StepOutcome {
    match (exit, timed_out) {
        (_, true) => StepOutcome::Breach,
        (None, false) => StepOutcome::Retry,
        (Some(0), false) => StepOutcome::Passed,
        (Some(TEMPFAIL), false) => StepOutcome::Retry,
        (Some(_), false) => StepOutcome::Breach,
    }
}

/// What a procedure writes to `REMIT_RELEASE_OUT`: whether this commit published anything new, the version it
/// published and where its customer reaches it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Out {
    pub released: bool,
    pub version: String,
    pub links: Vec<String>,
}

impl Default for Out {
    fn default() -> Self {
        Self {
            released: true,
            version: String::new(),
            links: Vec::new(),
        }
    }
}

/// What the procedure said in its out file (none when it wrote none: it released, saying no version). A file that is
/// not that JSON is said so, and counts as a release with no version, as its steps passed. Pure.
pub fn out_of(text: Option<&str>) -> (Out, Option<String>) {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        None => (Out::default(), None),
        Some(t) => match serde_json::from_str::<Out>(t) {
            Ok(out) => (out, None),
            Err(e) => (
                Out::default(),
                Some(format!(
                    "REMIT_RELEASE_OUT is not {{\"released\", \"version\", \"links\"}}: {e}"
                )),
            ),
        },
    }
}

/// What a release of carried bakes under: the repository's guard, and the plan of every stage it carries that is no way
/// back (ADR-0114's optional plan). Pure.
pub fn guards_of(guard: Option<&ReleaseGuard>, carried: &[&Landing]) -> Vec<ReleaseGuard> {
    let mut out: Vec<ReleaseGuard> = guard.into_iter().cloned().collect();
    for plan in carried
        .iter()
        .filter_map(|l| l.version.plan.as_ref())
        .filter(|p| !p.undo)
    {
        let g = ReleaseGuard {
            signal: plan.guardrail.clone(),
            breach: plan.breach.clone(),
            bake: plan.bake,
        };
        if !g.signal.as_str().is_empty() && !out.contains(&g) {
            out.push(g);
        }
    }
    out
}

/// Until when a release published at at under guards bakes: the longest of their bakes; none with no guard. Pure.
pub fn bake_until(guards: &[ReleaseGuard], at: Time) -> Option<Time> {
    guards.iter().map(|g| rollout::bake_end(g.bake, at)).max()
}

/// Why the release breached while it bakes, of each guard's newest reading (none when it has none yet), as read at now:
/// a reading made since the release published (`since`) that meets its guard's breach condition. A condition that
/// cannot be evaluated on a reading says nothing. Pure.
pub fn breach_of(guards: &[ReleaseGuard], newest: &[Option<model::Reading>], since: Time, now: Time) -> Option<String> {
    guards.iter().zip(newest).find_map(|(g, r)| {
        let r = r.as_ref().filter(|r| r.at >= since)?;
        signal::probe(&g.breach, r, now)
            .ok()
            .filter(|breached| *breached)
            .map(|_| {
                format!(
                    "guardrail {} breached ({}) while the release baked",
                    g.signal.as_str(),
                    g.breach
                )
            })
    })
}

/// The environment of a step of rel: its repository (host/project, as the forge names it), its commit and the commit
/// released before it (empty for the first), with scratch its HOME and out the file it writes what it released to; a
/// way back also says why the release was breached. No credential is in it: a connection's is added by the proxy a step
/// reaches it through. Pure.
pub fn step_env(rel: &Release, scratch: &str, out: &str, breached: Option<&str>) -> Vec<String> {
    let mut env = vec![
        "PATH=/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin".to_owned(),
        format!("HOME={scratch}"),
        format!("TMPDIR={scratch}"),
        "LANG=C.UTF-8".to_owned(),
        format!("REMIT_RELEASE_REPOSITORY={}/{}", rel.host, rel.project),
        format!("REMIT_RELEASE_COMMIT={}", rel.commit),
        format!("REMIT_RELEASE_PREVIOUS={}", rel.previous.as_deref().unwrap_or_default()),
        format!("REMIT_RELEASE_OUT={out}"),
    ];
    if let Some(why) = breached {
        env.push(format!("REMIT_RELEASE_BREACHED={}", why.replace(['\n', '\0'], " ")));
    }
    env
}

/// The AIMD size of the next release of a repository after r ended: one more landing after a release that went live,
/// half after one taken back, as the queue's batch (landing.rs). Pure.
pub const fn next_k(k: usize, state: &ReleaseState) -> usize {
    match state {
        ReleaseState::Live => crate::landing::grown(k),
        ReleaseState::TakenBack { .. } => crate::landing::halved(k),
        _ => k,
    }
}

#[cfg(test)]
mod tests;
