//! Releases' shell (ADR-0116): one runner per repository at a time, in memory, so one release of a repository runs at
//! once. A pass reads the repository's landings and releases and does what the rule (release.rs) says: it bakes the one
//! that bakes, runs again the one its steps left waiting, or starts the next. A release's steps run in name order in a
//! fresh checkout of its commit, each in a jail of its own, through the `StepRunner` the server gives (the injecting
//! proxy to the connections the procedure names, through the grants its repository holds); on the repository this
//! server runs, `install_release` is one more step. A breach (a step that fails, the forge's checks gone red, a guard
//! breached while it bakes) is taken back in code with no turn: the `back` steps run (or the previous release is
//! installed here), every landing it carried is reverted at the head of its queue, and once the reverts land they are
//! released like any landing. Each step is recorded before the next acts outside, so a restart runs the idempotent
//! procedure again on the recorded commit.

use std::collections::{BTreeSet, HashMap};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::PoisonError;
use std::time::Duration;

use async_trait::async_trait;
use model::{
    Actor, DecisionKind, Landing, Release, ReleaseLift, ReleasePause, ReleaseState, ReleaseStep, RevertCause, by_remit,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use telemetry::Child;

use crate::Shipping;
use crate::install::PREVIOUS;
use crate::landing::{Verdict, revertible, verdict_on};
use crate::queue::queue_cause;
use crate::release::{
    Connection, Own, Procedure, StepOutcome, bake_until, breach_of, guards_of, next_k, next_release, out_of, own_of,
    procedure, reaim, step_env, step_outcome,
};
use crate::release_trace::release_span;
use crate::stage::instance_repository;
use chains::apply::Records;
use chains::chain_core::new_id;
use chains::fact::{fact_write, keys};
use chains::git::{RepoRemote, git, path_str, short, tail};
use chains::live::LandingWrite;
use chains::step::Decided;
use chains::{Error, Result};
use tracing::Instrument;

/// Who Remit acts as, in what a release records.
pub(crate) const RELEASER: &str = "release";
/// Wall time of one step: a wasm build, or the forge's workflow a step waits for, takes tens of minutes.
const STEP_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// Processor time of one step.
const STEP_CPU: Duration = Duration::from_secs(30 * 60);
/// Bytes kept of each of a step's stdout and stderr.
const STEP_OUTPUT: usize = 64 << 10;
/// Bytes of what a step's output is cut to in its record.
const STEP_KEPT: usize = 4000;
/// Bytes of data a step may hold: a build needs room.
const STEP_MEMORY: u64 = 4 << 30;
/// The step install_release is recorded as, on the repository this server runs.
const INSTALL_STEP: &str = "install_release";

/// The port a release's step reaches the outside through (ADR-0116). The server fills it: a step with no connection
/// runs in the jail with no network; one with connections behind the injecting proxy, each through the grant its
/// repository holds, the proxy's authorities written to `home` (the step's HOME, writable). An error is a step that
/// could not be run (no grant, no credential, no jail): the release waits and is tried again, nothing is taken back.
#[async_trait]
pub trait StepRunner: Send + Sync {
    async fn run_step(
        &self,
        repo: &str,
        connections: &[Connection],
        home: &Path,
        spec: jail::Spec,
    ) -> Result<jail::RunResult, String>;
}

/// Where a release published is told to the outside (ADR-0119: Datadog's deployment event). The server fills it; it never
/// waits and never fails the release: what it cannot tell, it lets go.
pub trait Deployments: Send + Sync {
    /// rel was published (its version is set, its steps recorded) at `now`, carrying `own`: what it carries of the
    /// chains' own work, as shipping chose it.
    fn published(&self, rel: &Release, own: &Own, now: model::Time);
}

/// What one repository's runner holds in memory: whether it runs, whether it was kicked while it ran, the size of its
/// next release (`next_k`), and whether its next release is at the newest landing (its releases' pause was lifted). A
/// restart starts again from one.
#[derive(Debug, Clone, Copy)]
pub struct ReleaseRun {
    running: bool,
    again: bool,
    k: usize,
    at_head: bool,
}

impl Default for ReleaseRun {
    fn default() -> Self {
        Self {
            running: false,
            again: false,
            k: 1,
            at_head: false,
        }
    }
}

/// The repositories' release runners, by repository.
pub type ReleaseRuns = std::sync::Mutex<HashMap<String, ReleaseRun>>;

/// What `fact:repo.release` says: a release published, went live, published nothing new or was taken back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReleaseFact {
    pub repository: String,
    pub release: String,
    pub version: String,
    pub commit: String,
    pub landings: Vec<String>,
    /// The release's state, as its kind names it.
    pub state: String,
}

/// A fresh directory of a release's run (its checkout, its HOME and its out file), removed when it is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> std::io::Result<Self> {
        let p = std::env::temp_dir().join(new_id("remit-release-"));
        std::fs::DirBuilder::new().mode(0o700).create(&p)?;
        std::fs::DirBuilder::new().mode(0o700).create(p.join("home"))?;
        Ok(Self(p))
    }
    fn checkout(&self) -> PathBuf {
        self.0.join("checkout")
    }
    fn home(&self) -> PathBuf {
        self.0.join("home")
    }
    fn out(&self) -> PathBuf {
        self.0.join("home").join("release-out.json")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// revert_release: the release to take back and why.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevertReleaseInput {
    /// the release's id (rl_…), as the repository's releases and a landing's release name it
    pub release: String,
    /// why it is taken back, in the owner's words: the Chairs whose changes it carried read it
    pub why: String,
}

/// pause_releases and resume_releases: the repository and why.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleasePauseInput {
    /// the repository, by its catalog id as a chain's repositories and landings name it
    pub repo: String,
    /// why, in the owner's words: recorded with the pause
    pub why: String,
}

/// The step names of a procedure, as its record shows them: the file's name within `.remit/release/`.
fn step_name(path: &str) -> String {
    path.strip_prefix(crate::gate::RELEASE_DIR).unwrap_or(path).to_owned()
}

/// The jail spec of the step at path of a checkout, with env, its HOME at home.
fn step_spec(checkout: &str, home: &str, path: &str, env: Vec<String>) -> jail::Spec {
    let mut read_only = jail::system_paths();
    // git runs what .git names (hooks, fsmonitor) when the server uses the checkout: a step does not write it
    read_only.push(format!("{checkout}/.git"));
    jail::Spec {
        argv: vec![format!("{checkout}/{path}")],
        dir: checkout.to_owned(),
        writable: vec![home.to_owned()],
        env,
        read_only,
        timeout: STEP_TIMEOUT,
        max_output: STEP_OUTPUT,
        memory_bytes: STEP_MEMORY,
        cpu: STEP_CPU,
        ..jail::Spec::default()
    }
}

impl Shipping {
    /// Has repo's release runner run now: started when none runs, told to go again when one does.
    pub fn kick_release(&self, repo: &str) {
        {
            let mut runs = self.releases.lock().unwrap_or_else(PoisonError::into_inner);
            let run = runs.entry(repo.to_owned()).or_default();
            if run.running {
                run.again = true;
                return;
            }
            run.running = true;
        }
        let Some(me) = self.me() else { return };
        let key = repo.to_owned();
        if !self.background(move |_| async move { me.run_releases(&key).await }) {
            self.release_run(repo, |r| r.running = false);
        }
    }

    fn release_run<T>(&self, repo: &str, edit: impl FnOnce(&mut ReleaseRun) -> T) -> T {
        let mut runs = self.releases.lock().unwrap_or_else(PoisonError::into_inner);
        edit(runs.entry(repo.to_owned()).or_default())
    }

    /// Kicks the runner of every repository with a landing on its base no release carried, or a release open.
    pub async fn sync_releases(&self) {
        let Ok((landings, releases)) = self
            .store
            .read(|r| Ok::<_, store::Error>((r.landings()?, r.releases()?)))
            .await
        else {
            return;
        };
        let repos: BTreeSet<&str> = landings
            .iter()
            .filter(|l| l.on_base() && l.release.is_none())
            .map(|l| l.repo.as_str())
            .chain(releases.iter().filter(|r| r.open()).map(|r| r.repo.as_str()))
            .collect();
        for repo in repos {
            self.kick_release(repo);
        }
    }

    /// Runs passes of repo until one does nothing.
    async fn run_releases(&self, repo: &str) {
        loop {
            self.release_run(repo, |r| r.again = false);
            loop {
                if self.is_stopping() {
                    break;
                }
                match self.release_pass(repo).await {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(err) => {
                        tracing::warn!(repo, %err, "release: the pass stopped; the next sync runs it again");
                        break;
                    }
                }
            }
            let again = self.release_run(repo, |r| {
                if !r.again {
                    r.running = false;
                }
                r.again
            });
            if !again {
                return;
            }
        }
    }

    /// One pass of repo: the release that bakes is read, the one whose steps wait runs again, or the next starts.
    /// Whether it moved anything, so the runner passes again at once; a release left waiting did not.
    async fn release_pass(&self, repo: &str) -> Result<bool, String> {
        let key = repo.to_owned();
        let (landings, releases, pause) = self
            .store
            .read(move |r| {
                Ok::<_, store::Error>((r.repo_landings(&key)?, r.repo_releases(&key)?, r.release_pause(&key)?))
            })
            .await
            .map_err(|e| e.to_string())?;
        let held = pause.as_ref().is_some_and(ReleasePause::holds);
        if let Some(open) = releases.iter().find(|r| r.open()) {
            return match open.state {
                ReleaseState::Baking => self.bake(open.clone(), &landings).await,
                // a commit the forge's checks failed is not released, however long its steps wait
                _ if red(open, &landings) => self
                    .take_back(open.clone(), &landings, by_remit(RELEASER), RED, RevertCause::Guardrail)
                    .await
                    .map(|()| true),
                _ if held => Ok(false),
                _ => self.run_procedure(open.clone(), &landings, None).await,
            };
        }
        if held {
            return Ok(false);
        }
        // one base is released: the one the repository's newest landing went to
        let Some(base) = landings
            .iter()
            .filter(|l| l.on_base())
            .max_by_key(|l| l.landed_at)
            .map(|l| l.base.clone())
        else {
            return Ok(false);
        };
        let on_base: Vec<Landing> = landings.into_iter().filter(|l| l.base == base).collect();
        let (k, at_head) = self.release_run(repo, |r| (r.k, r.at_head));
        let Some(due) = next_release(&on_base, &releases, k, at_head, self.now()) else {
            return Ok(false);
        };
        let top = due.top.clone();
        let clone = self.clone_of(&top);
        git(&clone, &["fetch", "-q", "origin"]).await?;
        let commit = top.commit.clone().unwrap_or_default();
        let declared = procedure_at(&clone, &commit).await?;
        let now = self.now();
        let previous = releases
            .iter()
            .find(|r| matches!(r.state, ReleaseState::Live | ReleaseState::Baking))
            .map(|r| r.commit.clone());
        let rel = Release {
            id: new_id("rl_"),
            repo: repo.to_owned(),
            host: top.host.clone(),
            project: top.project.clone(),
            base,
            commit,
            top: top.id.clone(),
            previous,
            created_at: now,
            ..Release::default()
        };
        let carried = carried_by(&due.carried, &rel.id, now);
        self.release_run(repo, |r| r.at_head = false);
        match declared {
            // the repository declares no release: what lands is its customer's once the forge's checks stand on it
            Ok(None) => {
                let rel = self.write_release(rel, carried.clone(), false).await?;
                self.went_live(rel, &carried).await?;
                Ok(true)
            }
            Ok(Some(proc)) => {
                let rel = self.write_release(rel, carried, false).await?;
                let key = repo.to_owned();
                let landings = self
                    .store
                    .read(move |r| r.repo_landings(&key))
                    .await
                    .map_err(|e| e.to_string())?;
                self.run_procedure(rel, &landings, Some(proc)).await
            }
            // stored, its procedure read again by the next pass, which says why it waits and to whom
            Err(_) => self.write_release(rel, carried, false).await.map(|_| true),
        }
    }

    /// The clone of l's repository Remit holds.
    fn clone_of(&self, l: &Landing) -> String {
        path_str(&self.clone_dir(&RepoRemote {
            host: l.host.clone(),
            project: l.project.clone(),
        }))
    }

    /// Stores rel and the landings it carries moved with it, saying so in `fact:repo.release` when fact, and gives it as
    /// stored.
    async fn write_release(&self, rel: Release, landings: Vec<Landing>, fact: bool) -> Result<Release, String> {
        let mut rec = Records {
            releases: vec![rel.clone()],
            landings: landings
                .into_iter()
                .map(|landing| LandingWrite {
                    landing,
                    cause: queue_cause(),
                })
                .collect(),
            ..Records::default()
        };
        if fact {
            let id = rel.id.clone();
            let carried = self
                .store
                .read(move |r| r.release_landings(&id))
                .await
                .map_err(|e| e.to_string())?;
            let v = ReleaseFact {
                repository: rel.repo.clone(),
                release: rel.id.clone(),
                version: rel.version.clone(),
                commit: rel.commit.clone(),
                landings: carried.iter().map(|l| l.id.clone()).collect(),
                state: rel.state.kind().to_owned(),
            };
            rec.readings
                .push(fact_write(keys::repo_release(), &v, by_remit(RELEASER), "").map_err(|e| e.to_string())?);
        }
        self.commit(None, None, rec).await.map_err(|e| e.to_string())?;
        let id = rel.id.clone();
        self.store
            .read(move |r| r.release(&id))
            .await
            .map_err(|e| e.to_string())
    }

    /// Runs rel's procedure (proc, or as its commit declares it) from its first step, the run replacing the steps of the
    /// one before, and records what came of it: published (baking, or live when nothing guards it), nothing new to
    /// publish, waiting to run again, or breached and taken back. Whether it moved on from releasing.
    async fn run_procedure(&self, rel: Release, landings: &[Landing], proc: Option<Procedure>) -> Result<bool, String> {
        let span = release_span("run", &rel, &own_of(landings, &rel.id));
        self.run_procedure_steps(rel, landings, proc).instrument(span).await
    }

    /// `run_procedure`, in the span of its trace.
    async fn run_procedure_steps(
        &self,
        mut rel: Release,
        landings: &[Landing],
        proc: Option<Procedure>,
    ) -> Result<bool, String> {
        let clone = path_str(&self.clone_dir(&RepoRemote {
            host: rel.host.clone(),
            project: rel.project.clone(),
        }));
        let proc = match proc {
            Some(p) => p,
            None => {
                if let Err(err) = git(&clone, &["fetch", "-q", "origin"]).await {
                    return self
                        .wait_release(rel, format!("the clone could not fetch: {err}"))
                        .await;
                }
                match procedure_at(&clone, &rel.commit).await? {
                    Ok(Some(p)) => p,
                    Ok(None) => {
                        return self
                            .wait_release(rel, "its commit declares no release procedure".to_owned())
                            .await;
                    }
                    Err(why) => return self.reaim_or_wait(rel, landings, &why).await,
                }
            }
        };
        let carried: Vec<&Landing> = landings
            .iter()
            .filter(|l| l.release.as_deref() == Some(rel.id.as_str()))
            .collect();
        rel.guards = guards_of(proc.guard.as_ref(), &carried);
        rel.steps.clear();
        rel.waits.clear();
        rel.tries = rel.tries.saturating_add(1);
        let scratch = Scratch::new().map_err(|e| format!("no scratch directory: {e}"))?;
        let checkout = path_str(&scratch.checkout());
        git(&clone, &["worktree", "add", "-q", "--detach", &checkout, &rel.commit]).await?;
        let ran = self.run_steps(&mut rel, &proc, &proc.steps, &scratch, None).await;
        git(&clone, &["worktree", "remove", "--force", &checkout]).await.ok();
        match ran? {
            StepOutcome::Retry => {
                let waits = rel.steps.last().map(|s| tail(&s.output, 300)).unwrap_or_default();
                self.wait_release(rel, waits).await
            }
            StepOutcome::Breach => {
                let why = rel
                    .steps
                    .last()
                    .map(|s| match s.exit {
                        Some(code) => {
                            format!("step {} exited {code}: {}", s.name, tail(&s.output, 600))
                        }
                        None => format!("step {} did not end: {}", s.name, tail(&s.output, 600)),
                    })
                    .unwrap_or_default();
                self.take_back(rel, landings, by_remit(RELEASER), &why, RevertCause::Guardrail)
                    .await
                    .map(|()| true)
            }
            StepOutcome::Passed => {
                let text = std::fs::read_to_string(scratch.out()).ok();
                self.published(rel, &text).await
            }
        }
    }

    /// rel's procedure does not parse (why): it is re-aimed at what landed since, which may fix it (`reaim`), or waits
    /// saying why, the chains whose landings changed `.remit/release/` told once.
    async fn reaim_or_wait(&self, mut rel: Release, landings: &[Landing], why: &str) -> Result<bool, String> {
        let on_base: Vec<Landing> = landings.iter().filter(|l| l.base == rel.base).cloned().collect();
        let more = reaim(&on_base, self.now());
        if let Some(top) = more.last() {
            (rel.top, rel.commit) = (top.id.clone(), top.commit.clone().unwrap_or_default());
            rel.waits.clear();
            let carried = carried_by(&more, &rel.id, self.now());
            self.write_release(rel, carried, false).await?;
            return Ok(true);
        }
        let waits = format!("its .remit/release/ does not parse: {why}");
        if rel.waits != waits {
            let carried: Vec<Landing> = landings
                .iter()
                .filter(|l| l.release.as_deref() == Some(rel.id.as_str()))
                .cloned()
                .collect();
            let changed: Vec<Landing> = carried
                .iter()
                .filter(|l| l.version.paths.iter().any(|p| p.starts_with(crate::gate::RELEASE_DIR)))
                .cloned()
                .collect();
            let told = if changed.is_empty() { carried } else { changed };
            let text = format!(
                "{} cannot be released: {waits}. Land a version that fixes it; the release waits for it.",
                rel.repo
            );
            self.tell_release(&rel, &told, &text).await;
        }
        self.wait_release(rel, waits).await
    }

    /// Records rel as waiting to run again, for waits: it did not move on.
    async fn wait_release(&self, mut rel: Release, waits: String) -> Result<bool, String> {
        tracing::info!(release = %rel.id, waits, "release: it runs again at the next sync");
        rel.waits = waits;
        self.write_release(rel, Vec::new(), false).await.map(|_| false)
    }

    /// Runs paths, steps of proc, one after another in the checkout of scratch, recording each on rel (back: its way
    /// back, with why it was breached), and stops at the first that does not pass.
    async fn run_steps(
        &self,
        rel: &mut Release,
        proc: &Procedure,
        paths: &[String],
        scratch: &Scratch,
        breached: Option<&str>,
    ) -> Result<StepOutcome, String> {
        let (checkout, home, out) = (
            path_str(&scratch.checkout()),
            path_str(&scratch.home()),
            path_str(&scratch.out()),
        );
        let into_back = breached.is_some();
        for path in paths {
            let step = ReleaseStep {
                name: step_name(path),
                started_at: self.now(),
                ..ReleaseStep::default()
            };
            if into_back {
                rel.back.push(step);
            } else {
                rel.steps.push(step);
            }
            rel.rev = self.write_release(rel.clone(), Vec::new(), false).await?.rev;
            // The step runs in a span of its own, named by its release, which it is told of (ADR-0119).
            let child = Child::open(path, &rel.id);
            let env = child.tell(step_env(rel, &home, &out, breached));
            let spec = step_spec(&checkout, &home, path, env);
            let ran = match self.steps.as_ref() {
                Some(steps) => {
                    steps
                        .run_step(&rel.repo, &proc.connections, &scratch.home(), spec)
                        .instrument(child.span().clone())
                        .await
                }
                None => Err("this server runs no release step".to_owned()),
            };
            let (exit, timed_out, output) = match ran {
                Ok(r) => (Some(r.exit_code), r.timed_out, format!("{}\n{}", r.stderr, r.stdout)),
                Err(why) => (None, false, why),
            };
            match exit {
                Some(code) => child.ended(code, timed_out),
                None => child.failed(),
            }
            let outcome = step_outcome(exit, timed_out);
            let list = if into_back { &mut rel.back } else { &mut rel.steps };
            if let Some(s) = list.last_mut() {
                (s.exit, s.ended_at) = (exit, Some(self.now()));
                s.output = tail(output.trim(), STEP_KEPT);
            }
            if outcome != StepOutcome::Passed {
                return Ok(outcome);
            }
        }
        Ok(StepOutcome::Passed)
    }

    /// What follows rel's steps passing, its out file's text read: nothing new (its landings wait for the next
    /// release), or a publish: installed here first on the repository this server runs (a refused install waits and
    /// runs again), then baking under its guards, or live at once with none. Whether it moved on from releasing.
    async fn published(&self, mut rel: Release, text: &Option<String>) -> Result<bool, String> {
        let (out, note) = out_of(text.as_deref());
        let now = self.now();
        rel.waits = note.unwrap_or_default();
        let id = rel.id.clone();
        let carried = self
            .store
            .read(move |r| r.release_landings(&id))
            .await
            .map_err(|e| e.to_string())?;
        if !out.released {
            (rel.state, rel.ended_at) = (ReleaseState::Unchanged, Some(now));
            let back: Vec<Landing> = carried
                .into_iter()
                .map(|mut l| {
                    (l.release, l.updated_at) = (None, now);
                    l
                })
                .collect();
            self.write_release(rel, back, true).await?;
            return Ok(true);
        }
        (rel.version, rel.links) = (out.version, out.links);
        tracing::Span::current().record("version", rel.version.as_str());
        if instance_repository(&self.cfg.update_repo, &rel.host, &rel.project)
            && let Err(why) = self.install_here(&mut rel).await
        {
            return self
                .wait_release(rel, format!("this server did not install it: {why}"))
                .await;
        }
        if let Some(deployments) = &self.deployments {
            deployments.published(&rel, &own_of(&carried, &rel.id), self.now());
        }
        rel.bake_until = bake_until(&rel.guards, self.now());
        if rel.bake_until.is_none() {
            self.went_live(rel, &carried).await?;
            return Ok(true);
        }
        rel.state = ReleaseState::Baking;
        let rel = self.write_release(rel, Vec::new(), true).await?;
        let text = format!(
            "{} was released as {}{}: it carries {}. It bakes until {} under {}: a breach takes it back, and you are \
             told.",
            rel.repo,
            if rel.version.is_empty() {
                short(&rel.commit)
            } else {
                &rel.version
            },
            links_of(&rel),
            carried_words(&carried),
            rel.bake_until.map(chains::wait::rfc3339).unwrap_or_default(),
            rel.guards
                .iter()
                .map(|g| g.signal.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.tell_release(&rel, &carried, &text).await;
        Ok(true)
    }

    /// Installs rel on this server, the last step of a release of the repository it runs, recorded as a step of it;
    /// the refusal when it was not (no -update-restart, an update under way). This server restarts under it a moment
    /// later: a restart before rel is stored runs its procedure again, and installs the same release.
    async fn install_here(&self, rel: &mut Release) -> Result<(), String> {
        let mut step = ReleaseStep {
            name: INSTALL_STEP.to_owned(),
            started_at: self.now(),
            ..ReleaseStep::default()
        };
        let said = self.install_release(&rel.version).await;
        step.ended_at = Some(self.now());
        let refused = said.as_ref().err().cloned();
        (step.exit, step.output) = match said {
            Ok(text) => (Some(0), text),
            Err(why) => (None, why),
        };
        rel.steps.push(step);
        refused.map_or(Ok(()), Err)
    }

    /// Reads rel while it bakes: taken back on a breach (a guard's reading since it published meets its condition, or
    /// the forge's checks on its commit failed where the base before passed), live once its bake ended healthy (at once
    /// with no bake).
    async fn bake(&self, rel: Release, landings: &[Landing]) -> Result<bool, String> {
        let now = self.now();
        let since = rel
            .steps
            .iter()
            .filter_map(|s| s.ended_at)
            .max()
            .unwrap_or(rel.created_at);
        let mut newest = Vec::with_capacity(rel.guards.len());
        for g in &rel.guards {
            let key = g.signal.clone();
            newest.push(match self.store.read(move |r| r.last_reading(&key)).await {
                Ok(r) => Some(r),
                Err(store::Error::NotFound) => None,
                Err(err) => return Err(err.to_string()),
            });
        }
        let breach =
            breach_of(&rel.guards, &newest, since, now).or_else(|| red(&rel, landings).then(|| RED.to_owned()));
        if let Some(why) = breach {
            self.take_back(rel, landings, by_remit(RELEASER), &why, RevertCause::Guardrail)
                .await?;
            return Ok(true);
        }
        if rel.bake_until.is_some_and(|until| now < until) {
            return Ok(false);
        }
        let id = rel.id.clone();
        let carried = self
            .store
            .read(move |r| r.release_landings(&id))
            .await
            .map_err(|e| e.to_string())?;
        self.went_live(rel, &carried).await?;
        Ok(true)
    }

    /// rel baked healthy (or had nothing to bake under): its landings' chains marked live, then rel live, the next
    /// release one landing bigger. The chains come first so that a release the owner sees live has its chains live, and so
    /// that a crash between the two leaves rel baking, where the next round does both again (a chain already marked live
    /// for its landing is marked once).
    async fn went_live(&self, rel: Release, carried: &[Landing]) -> Result<(), String> {
        let span = release_span("live", &rel, &own_of(carried, &rel.id));
        self.went_live_steps(rel, carried).instrument(span).await
    }

    /// `went_live`, in the span of its trace.
    async fn went_live_steps(&self, mut rel: Release, carried: &[Landing]) -> Result<(), String> {
        let note = format!(
            "live in {}{}",
            if rel.version.is_empty() {
                short(&rel.commit)
            } else {
                &rel.version
            },
            links_of(&rel)
        );
        for chain in own_of(carried, &rel.id).chains {
            if let Err(err) = self.mark_live(chain, by_remit(RELEASER), &note).await {
                tracing::warn!(chain, release = %rel.id, %err, "release: a chain it carried was not marked live");
            }
        }
        (rel.state, rel.ended_at) = (ReleaseState::Live, Some(self.now()));
        let rel = self.write_release(rel, Vec::new(), true).await?;
        self.release_run(&rel.repo, |r| r.k = next_k(r.k, &rel.state));
        self.tell_release(&rel, carried, &format!("{} is {note}.", rel.repo))
            .await;
        Ok(())
    }

    /// Takes rel back, by by, for why: recorded taken back, every landing it carried reverted at the head of its queue
    /// (cause), then its way back run: on the repository this server runs, the previous release installed when rel
    /// was installed here (one never installed leaves this server as it is); elsewhere its `back` steps. Once the
    /// reverts land, the next release carries them forward.
    async fn take_back(
        &self,
        rel: Release,
        landings: &[Landing],
        by: Actor,
        why: &str,
        cause: RevertCause,
    ) -> Result<(), String> {
        let span = release_span("take_back", &rel, &own_of(landings, &rel.id));
        self.take_back_steps(rel, landings, by, why, cause)
            .instrument(span)
            .await
    }

    /// `take_back`, in the span of its trace.
    async fn take_back_steps(
        &self,
        mut rel: Release,
        landings: &[Landing],
        by: Actor,
        why: &str,
        cause: RevertCause,
    ) -> Result<(), String> {
        rel.state = ReleaseState::TakenBack {
            by: by.clone(),
            why: why.to_owned(),
        };
        rel.ended_at = Some(self.now());
        let mut rel = self.write_release(rel, Vec::new(), true).await?;
        self.release_run(&rel.repo, |r| r.k = next_k(r.k, &rel.state));
        let carried: Vec<&Landing> = landings
            .iter()
            .filter(|l| l.release.as_deref() == Some(rel.id.as_str()))
            .collect();
        for l in carried.iter().filter(|l| revertible(l, landings)) {
            if let Err(err) = self.queue_revert(l, cause, by.clone(), why).await {
                tracing::warn!(landing = %l.id, release = %rel.id, %err, "release: a landing it carried was not reverted");
            }
        }
        let owned: Vec<Landing> = carried.into_iter().cloned().collect();
        self.tell_release(
            &rel,
            &owned,
            &format!(
                "{} release {} was taken back: {why}\nThe landings it carried of yours are reverted at the head of the \
                 queue; fix the cause and land a new version.",
                rel.repo,
                if rel.version.is_empty() {
                    short(&rel.commit)
                } else {
                    &rel.version
                }
            ),
        )
        .await;
        if !instance_repository(&self.cfg.update_repo, &rel.host, &rel.project) {
            return self.run_back(rel, why).await;
        }
        if !installed(&rel) {
            return Ok(());
        }
        let mut step = ReleaseStep {
            name: format!("{INSTALL_STEP} {PREVIOUS}"),
            started_at: self.now(),
            ..ReleaseStep::default()
        };
        let said = self.install_release(PREVIOUS).await;
        step.ended_at = Some(self.now());
        (step.exit, step.output) = match said {
            Ok(text) => (Some(0), text),
            Err(why) => (None, why),
        };
        rel.back.push(step);
        self.write_release(rel, Vec::new(), false).await.map(|_| ())
    }

    /// Runs the `back` steps of rel's procedure, read at its commit, with why it was breached.
    async fn run_back(&self, mut rel: Release, why: &str) -> Result<(), String> {
        let clone = path_str(&self.clone_dir(&RepoRemote {
            host: rel.host.clone(),
            project: rel.project.clone(),
        }));
        let Ok(Some(proc)) = procedure_at(&clone, &rel.commit).await? else {
            return Ok(()); // no way back it can read: the release forward of the reverts is it
        };
        if proc.back.is_empty() {
            return Ok(()); // no way back declared: the release forward of the reverts is it
        }
        let scratch = Scratch::new().map_err(|e| format!("no scratch directory: {e}"))?;
        let checkout = path_str(&scratch.checkout());
        git(&clone, &["worktree", "add", "-q", "--detach", &checkout, &rel.commit]).await?;
        let ran = self.run_steps(&mut rel, &proc, &proc.back, &scratch, Some(why)).await;
        git(&clone, &["worktree", "remove", "--force", &checkout]).await.ok();
        ran?;
        self.write_release(rel, Vec::new(), false).await.map(|_| ())
    }
    /// Tells the Chair of each chain whose landing of carried rel carries text, as a release decision of its chain.
    async fn tell_release(&self, rel: &Release, carried: &[Landing], text: &str) {
        for chain in own_of(carried, &rel.id).chains {
            let chair = self.chair_of(chain).await;
            let rec = Records {
                decisions: vec![Decided {
                    kind: DecisionKind::Release,
                    text: chains::chain_core::first_line(text, 300),
                    why: format!("release {}", rel.id),
                    card_id: chair,
                    by: by_remit(RELEASER),
                }],
                ..Records::default()
            };
            if let Err(err) = self.write_on_chain(chain, &rel.repo, text.to_owned(), rec).await {
                tracing::warn!(chain, release = %rel.id, %err, "release: a chain it carried was not told");
            }
        }
    }

    /// Takes release input.release back on the word of by (the owner, the Assistant for him, or a Chair whose landing
    /// it carries): it runs the way back a breach runs, at once, recorded with who and why.
    pub async fn revert_release(&self, by: Actor, input: RevertReleaseInput) -> Result<Release> {
        let why = input.why.trim().to_owned();
        if why.is_empty() {
            return Err(Error::refused(
                "why is required: the Chairs whose changes it carried read it",
            ));
        }
        let id = input.release.trim().to_owned();
        let rel = self.store.read(move |r| r.release(&id)).await?;
        if !matches!(rel.state, ReleaseState::Baking | ReleaseState::Live) {
            return Err(Error::refused(format!(
                "release {} is {}: only one that published (baking or live) is taken back",
                rel.id,
                rel.state.kind()
            )));
        }
        let repo = rel.repo.clone();
        let landings = self.store.read(move |r| r.repo_landings(&repo)).await?;
        let id = rel.id.clone();
        self.take_back(rel, &landings, by, &why, RevertCause::Overturn)
            .await
            .map_err(Error::Refused)?;
        Ok(self.store.read(move |r| r.release(&id)).await?)
    }

    /// Holds repository input.repo's releases on the word of by: no release starts while it holds (one baking bakes on),
    /// and landing goes on. Refused for a repository Remit does not know: none of its chains works in it and nothing
    /// landed on it, so a misspelt name holds nothing while it reads as held.
    pub async fn pause_releases(&self, by: Actor, input: ReleasePauseInput) -> Result<ReleasePause> {
        let (repo, why) = (input.repo.trim().to_owned(), input.why.trim().to_owned());
        if repo.is_empty() || why.is_empty() {
            return Err(Error::refused(
                "name the repository and say why: the pause is recorded with both",
            ));
        }
        let key = repo.clone();
        let known = self
            .store
            .read(move |r| {
                Ok::<_, store::Error>(
                    !r.repo_landings(&key)?.is_empty()
                        || r.chains()?.iter().any(|c| c.repos.iter().any(|w| w.repo == key)),
                )
            })
            .await?;
        if !known {
            return Err(Error::refused(format!(
                "Remit knows no repository {repo}: name it as a chain's repository is named"
            )));
        }
        let p = ReleasePause {
            repo,
            by,
            why,
            at: self.now(),
            lifted: None,
        };
        self.write_pause(p.clone()).await?;
        Ok(p)
    }

    /// Stores p, pulled or lifted, and has the owner's view told.
    async fn write_pause(&self, p: ReleasePause) -> Result<()> {
        let rec = Records {
            release_pauses: vec![p],
            ..Records::default()
        };
        self.commit(None, None, rec).await.map(|_| ())
    }

    /// Lifts the pause of repository input.repo's releases on the word of by: the next release is at its newest landing.
    /// Says who paused it and why.
    pub async fn resume_releases(&self, by: Actor, input: ReleasePauseInput) -> Result<String> {
        let repo = input.repo.trim().to_owned();
        let why = input.why.trim().to_owned();
        if why.is_empty() {
            return Err(Error::refused("say why: the lift is recorded with it"));
        }
        let key = repo.clone();
        let Some(mut held) = self
            .store
            .read(move |r| r.release_pause(&key))
            .await?
            .filter(ReleasePause::holds)
        else {
            return Err(Error::refused(format!("the releases of {repo} are not paused")));
        };
        held.lifted = Some(ReleaseLift {
            by,
            why,
            at: self.now(),
        });
        self.write_pause(held.clone()).await?;
        self.release_run(&repo, |r| r.at_head = true);
        self.kick_release(&repo);
        Ok(format!(
            "The releases of {repo} resume at its newest landing (paused by {} {}: {}).",
            chains::outbox::actor_words(&held.by),
            chains::wait::rfc3339(held.at),
            held.why
        ))
    }
}

/// Why a release whose commit the forge's checks failed is taken back.
const RED: &str = "the forge's checks failed on its commit where the base before it passed";

/// Whether the forge's checks failed on rel's commit (its top landing's push) where the base before it passed.
fn red(rel: &Release, landings: &[Landing]) -> bool {
    landings
        .iter()
        .find(|l| l.id == rel.top)
        .is_some_and(|l| verdict_on(l.checks.as_ref(), l.base_checks.as_ref()) == Verdict::Revert)
}

/// The procedure `.remit/release/` declares at commit of the clone at dir (none when it declares no step), or why it does
/// not parse; the outer error is git's.
async fn procedure_at(dir: &str, commit: &str) -> Result<Result<Option<Procedure>, String>, String> {
    let listed = git(
        dir,
        &[
            "ls-tree",
            "-r",
            "-z",
            "--full-tree",
            commit,
            "--",
            crate::gate::RELEASE_DIR,
        ],
    )
    .await?;
    let entries: Vec<(String, bool)> = listed
        .split('\0')
        .filter_map(|e| {
            let (head, path) = e.split_once('\t')?;
            Some((path.to_owned(), head.starts_with("100755 ")))
        })
        .collect();
    let read = |name: &str| {
        let path = format!("{}{name}", crate::gate::RELEASE_DIR);
        let there = entries.iter().any(|(p, _)| *p == path);
        async move {
            if !there {
                return Ok(String::new());
            }
            git(dir, &["cat-file", "blob", &format!("{commit}:{path}")]).await
        }
    };
    let connections = read(crate::release::CONNECTIONS_FILE).await?;
    let guard = read(crate::release::GUARD_FILE).await?;
    Ok(procedure(&entries, &connections, &guard))
}

/// Whether rel was installed on this server: its install step passed.
fn installed(rel: &Release) -> bool {
    rel.steps.iter().any(|s| s.name == INSTALL_STEP && s.exit == Some(0))
}

/// landings, each moved into release id at now.
fn carried_by(landings: &[&Landing], id: &str, now: model::Time) -> Vec<Landing> {
    landings
        .iter()
        .map(|l| {
            let mut l = (*l).clone();
            (l.release, l.updated_at) = (Some(id.to_owned()), now);
            l
        })
        .collect()
}

/// " (link, link)" of rel's links; empty with none.
fn links_of(rel: &Release) -> String {
    if rel.links.is_empty() {
        String::new()
    } else {
        format!(" ({})", rel.links.join(", "))
    }
}

/// The landings carried, as a Chair reads them.
fn carried_words(carried: &[Landing]) -> String {
    carried
        .iter()
        .map(|l| format!("{} version {} ({})", l.repo, l.version.number, l.version.title))
        .collect::<Vec<_>>()
        .join("; ")
}
