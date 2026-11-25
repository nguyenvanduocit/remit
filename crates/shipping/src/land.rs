//! `land` puts a version of a chain's change on one repository into its queue (ADR-0114): it commits what agents left,
//! runs the repository's checks on the worktree's head and records the version as a landing, judging until every gate
//! the change needs approved its patch id, then ready. The queue (queue.rs) lands it; the Chair hears each step. The
//! checks take minutes, longer than an agent's tool call may wait, so `land` records a submit on the repository and
//! returns, and the submit runs in the background as work of its chain (`chain_work`): pausing or finishing the chain
//! stops it, and a restart runs its checks again (`carry_on_later`), as nothing outside has changed. The submit is a job
//! on the Chair's card, ended with the first line of its result in the write that records the landing and tells the
//! Chair.

use std::sync::Arc;
use std::time::Duration;

use model::{
    Card, Cause, Chain, ChainOrigin, DecisionKind, Duration as Span, Event, EventStatus, ForeignJson, Gate, JobStatus,
    Landing, LandingState, LandingVersion, Priority, RepoWork, ReturnReason, Round, SignalKey, StagePlan, Submit, Time,
    by_agent, by_remit,
};
use serde::Deserialize;
use telemetry::attr;
use tokio_util::sync::CancellationToken;
use tracing::Instrument as _;

use crate::Shipping;
use crate::gate::{RAILS_FILE, base_on, gates_of, patch_id, rails_of};
use crate::repo::foreign_commits;
use crate::repochecks::{run_checks, the_jail};
use crate::round::approved_state;
use chains::apply::Records;
use chains::chain_core::{first_line, new_id};
use chains::gate::missing_approvals;
use chains::git::{Check, git};
use chains::live::LandingWrite;
use chains::review_depth::{Change, Said};
use chains::step::{ChainCmd, Decided, WorkCmd, replace_with};
use chains::submit::{JobData, job_repo};
use chains::{Error, Result};

/// What the Chair lands: the repository, the commit's title and description, its rating of the change, and the canary
/// stage it lands as when it is one.
#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LandInput {
    /// repository as given to open_repo; empty when the chain has exactly one repository open
    #[serde(default)]
    pub repo: String,
    /// the landed commit's subject in the owner's language, conventional-commit style, e.g. 'fix(overview): report an
    /// error when the handler panics'
    pub title: String,
    /// how soon the owner should read this change once it lands: high, medium or low
    #[serde(default)]
    pub priority: String,
    /// one line in the owner's language: what waits on this change or what breaks if it is wrong
    #[serde(default)]
    pub priority_why: String,
    /// the commit's body in the owner's language: what changed and why, evidence (file:line), how it was verified, risks
    #[serde(default)]
    pub description: String,
    /// for a stage of a canary only: who this landing reaches once deployed, "0%, dark behind flag checkout_v2" for the
    /// first stage, then the share it grows to; for undo, what it brings back. Empty lands the change to everyone the base
    /// reaches.
    #[serde(default)]
    pub exposure: String,
    /// for a stage: the signal that says it hurts (chain:<id>.sensor.<name>, or another of this chain's own records)
    #[serde(default)]
    pub guardrail_key: String,
    /// for a stage: the condition on that signal's newest reading that means it hurts, e.g. value.error_rate > 0.02
    #[serde(default)]
    pub breach: String,
    /// for a stage: how long it is watched before the next stage on the repository may land, 1 to 10080
    #[serde(default)]
    pub bake_minutes: i64,
    /// for a stage: the act that takes it back, the flag value to set or the revert
    #[serde(default)]
    pub rollback: String,
    /// this stage takes an earlier one back: it lands while a stage bakes and while a guardrail is breached
    #[serde(default)]
    pub undo: bool,
}

impl LandInput {
    /// The Chair's rating, or why it is none: the owner reads what landed in its order.
    fn rated(&self) -> Result<Priority> {
        let Ok(p) = self.priority.parse::<Priority>() else {
            return Err(Error::refused(format!(
                "priority is {:?}: rate how soon the owner should read this change as high, medium or low",
                self.priority
            )));
        };
        if self.priority_why.trim().is_empty() {
            return Err(Error::refused(
                "priority_why is empty: say in one line what waits on this change or what breaks if it is wrong",
            ));
        }
        Ok(p)
    }

    /// The canary stage it lands as; none when no exposure is named.
    fn plan(&self) -> Option<StagePlan> {
        if self.exposure.trim().is_empty() && !self.undo {
            return None;
        }
        Some(StagePlan {
            exposure: self.exposure.trim().to_owned(),
            guardrail: SignalKey(self.guardrail_key.trim().to_owned()),
            breach: self.breach.trim().to_owned(),
            bake: Span(self.bake_minutes.clamp(0, 10_080).saturating_mul(Span::MINUTE.0)),
            rollback: self.rollback.trim().to_owned(),
            undo: self.undo,
        })
    }
}

/// What the job of a land of repo is called, wherever it shows.
pub fn job_title(repo: &str) -> String {
    format!("Checking · {repo}")
}

/// What starts the message that tells the Chair how a land of repo went.
pub fn result_heading(repo: &str) -> String {
    format!("[land result · {repo}]")
}

/// The job a land of repo is on the Chair's card, as it starts at at.
fn land_job(chain_id: &str, chair_id: &str, repo: &str, at: Time) -> Result<Event> {
    let data = serde_json::to_string(&JobData { repo: repo.to_owned() })?;
    Ok(Event {
        home: chain_id.to_owned(),
        card_id: chair_id.to_owned(),
        status: EventStatus::Job(JobStatus::Running),
        title: job_title(repo),
        data: Some(ForeignJson::parse(&data)?),
        created_at: at,
        updated_at: at,
        ..Event::default()
    })
}

/// The index of the chain's work on the repository named q. q names it as open_repo accepts it (its path on the forge,
/// host/project, or its last element) or as open_repo reported it (its worktree's directory).
pub fn find_work(repos: &[RepoWork], q: &str) -> Option<usize> {
    let q = q.trim_matches('/');
    repos.iter().position(|rw| {
        rw.repo == q
            || rw.project == q
            || format!("{}/{}", rw.host, rw.project) == q
            || last_element(&rw.project) == q
            || (!rw.path.is_empty() && last_element(&rw.path) == q)
    })
}

/// `find_work` for a caller that may leave the repository out: a chain with one repository open means that one.
pub fn work_named(repos: &[RepoWork], q: &str) -> Option<usize> {
    if q.trim().is_empty() && repos.len() == 1 {
        return Some(0);
    }
    find_work(repos, q)
}

/// Which repository the chain has open when the one named is not, so the caller retries with a name that works.
pub fn not_open_text(repos: &[RepoWork], q: &str) -> String {
    if repos.is_empty() {
        return format!("the chain has no repository open: the Chair opens {q:?} with open_repo first");
    }
    let open: Vec<&str> = repos.iter().map(|rw| rw.repo.as_str()).collect();
    format!(
        "the chain has no repository {q:?} open; repo is one of: {}",
        open.join(", ")
    )
}

/// The last element of a slash-separated path.
pub fn last_element(p: &str) -> &str {
    let p = p.trim_end_matches('/');
    match p.rsplit('/').next() {
        Some(last) if !last.is_empty() => last,
        _ => ".",
    }
}

/// Refuses to land the work of a chain a client key started (ADR-0009): it is an experiment.
pub(crate) const CLIENT_LANDS: &str =
    "a chain started through a client key does not land: its work stays in its worktree";

/// Whether a chain may change path. It may not change CI configuration, which runs with the group's or the repository's
/// secrets on any branch (GitLab's pipeline, GitHub's workflows), secrets, which stay out of git, the checks a
/// repository declares, which grade the chain's own change (ADR-0025), or the list of its rails, which says what needs
/// `security`. Everything else under `.remit/` is a chain's to write: its release procedure (`.remit/release/`) as a rail
/// (gate.rs, ADR-0116). Pure.
pub(crate) fn forbidden_path(path: &str) -> bool {
    if path.ends_with(".pem") {
        return true;
    }
    let starts = std::iter::once(0).chain(path.match_indices('/').map(|(i, _)| i.saturating_add(1)));
    starts.filter_map(|i| path.get(i..)).any(|rest| {
        [
            ".gitlab-ci.yml",
            ".gitlab/",
            ".github/workflows/",
            ".remit/checks/",
            "secret/",
        ]
        .iter()
        .any(|p| rest.starts_with(p))
            || rest == RAILS_FILE
            || rest == ".env"
            || rest.starts_with(".env.")
    })
}

/// How long the checks of a land may take: go build, vet and test and the repository's own.
const CHECK_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// A decision of Remit's about a chain's landing: what happened and why, on its Chair.
pub(crate) fn decided(chair_id: &str, text: String, why: String) -> Decided {
    Decided {
        kind: DecisionKind::Landing,
        text,
        why,
        card_id: chair_id.to_owned(),
        by: by_remit(""),
    }
}

impl Shipping {
    /// Starts a land of the chain's work on a repository, named as open_repo accepts it. It is refused while another land
    /// of the same repository runs, when the Chair has not rated the change, and, for a stage, when the rollout rule
    /// refuses it now. The result reaches the Chair as a message starting with `result_heading`.
    pub async fn land(self: &Arc<Self>, chair: &Card, input: LandInput) -> Result<RepoWork> {
        if self.is_stopping() {
            return Err(Error::Stopping);
        }
        let chain = self.chain(&chair.home).await?;
        crate::seeded::trial_land_refusal(&chain)?;
        if chain.origin == ChainOrigin::Client {
            return Err(Error::refused(CLIENT_LANDS));
        }
        let priority = input.rated()?;
        let i = work_named(&chain.repos, &input.repo)
            .ok_or_else(|| Error::Refused(not_open_text(&chain.repos, &input.repo)))?;
        let rw = chain.repos.get(i).cloned().unwrap_or_default();
        let plan = input.plan();
        if let Some(p) = &plan {
            self.vet_stage(&chain, &rw.host, &rw.project, p).await?;
        }
        let start = Submit {
            title: input.title.trim().to_owned(),
            description: input.description.trim().to_owned(),
            priority: Some(priority),
            priority_why: input.priority_why.trim().to_owned(),
            plan,
            started_at: self.now(),
        };
        let repo = rw.repo.clone();
        let job = land_job(&chain.id, &chair.id, &repo, start.started_at)?;
        let refusal = format!("a land of {repo} is already running; its result will come as a message");
        let edit = Arc::new(move |rw: &mut RepoWork| {
            if rw.submit.is_some() {
                return Err(refusal.clone());
            }
            rw.submit = Some(start.clone());
            Ok(())
        });
        let cmd = ChainCmd::Work(WorkCmd {
            repo: repo.clone(),
            edit,
            tell: String::new(),
        });
        let a = self
            .apply(
                &chain.id,
                cmd,
                Records {
                    events: vec![job],
                    ..Records::default()
                },
            )
            .await?;
        self.carry_on_later(&chain.id, &repo).await;
        a.chain
            .repos
            .get(i)
            .cloned()
            .ok_or_else(|| Error::refused(format!("the chain no longer has {repo} open")))
    }

    /// Carries a recorded land on in the background, as work of its chain. Once the server is stopping nothing starts,
    /// and the next start carries it on.
    pub async fn carry_on_later(&self, chain_id: &str, repo: &str) {
        let Some(o) = self.me() else { return };
        let (id, r) = (chain_id.to_owned(), repo.to_owned());
        let me = Arc::clone(&o);
        let started = o
            .core
            .chain_work(chain_id, move |work| async move { me.carry_on(work, &id, &r).await })
            .await;
        if let Err(err) = started
            && !matches!(err, Error::Stopping)
        {
            tracing::error!(chain = %chain_id, %repo, %err, "land");
        }
    }

    /// Why a chain's work was stopped when its chain did it (paused, finished, deleted), or none: then the server is
    /// stopping, and the work is left for the next start.
    async fn held_by(&self, chain_id: &str) -> Option<&'static str> {
        let cs = self.state(chain_id).await.ok()?;
        let held = cs.held.lock().await;
        if held.gone {
            return Some("the chain was deleted");
        }
        match held.chain.status() {
            model::ChainStatus::Paused => Some("the chain was paused"),
            model::ChainStatus::Done => Some("the chain was finished"),
            _ => None,
        }
    }

    /// The job of the land of repo still running in a chain, if there is one.
    async fn running_job(&self, chain_id: &str, repo: &str) -> Result<Option<Event>> {
        let id = chain_id.to_owned();
        let jobs = self.store.read(move |r| r.running_jobs(&id)).await?;
        Ok(jobs.into_iter().find(|e| job_repo(e) == repo))
    }

    /// Takes the recorded land of a chain's repository to its end: the landing recorded, or why not, told to the Chair
    /// in the write that ends the submit and its job.
    async fn carry_on(&self, work: CancellationToken, chain_id: &str, repo: &str) {
        let chain = match self.chain(chain_id).await {
            Ok(c) => c,
            Err(err) => return tracing::error!(chain = %chain_id, %repo, %err, "land"),
        };
        let Some(rw) = chain.repos.iter().find(|rw| rw.repo == repo).cloned() else {
            return;
        };
        let Some(submit) = rw.submit.clone() else { return };
        let checked = tokio::select! {
            r = tokio::time::timeout(CHECK_TIMEOUT, self.version_of_work(&chain, &rw, &submit)) => {
                r.unwrap_or_else(|_| Err(format!("the checks took longer than {CHECK_TIMEOUT:?}")))
            }
            () = work.cancelled() => Err(String::new()),
        };
        let held = if work.is_cancelled() {
            self.held_by(chain_id).await
        } else {
            None
        };
        let mut rec = Records::default();
        let (outcome, status) = match (held, checked) {
            (None, _) if work.is_cancelled() => return, // the server is stopping: the next start carries it on
            (Some(why), _) => {
                rec.decisions.push(decided(
                    &chain.chair_id,
                    format!("Land of {repo} stopped."),
                    why.to_owned(),
                ));
                (
                    format!("Stopped: {why}. Call land again when the chain runs."),
                    JobStatus::Failed,
                )
            }
            (None, Err(err)) => {
                rec.decisions.push(decided(
                    &chain.chair_id,
                    format!("Land of {repo} failed."),
                    first_line(&err, 200),
                ));
                (format!("Failed: {err}"), JobStatus::Failed)
            }
            (None, Ok((next, said))) => match self.queued(&chain, next, &said, &mut rec).await {
                Ok(text) => (text, JobStatus::Completed),
                Err(err) => (format!("Failed: {err}"), JobStatus::Failed),
            },
        };
        match self.running_job(chain_id, repo).await {
            Ok(Some(mut job)) => {
                job.status = EventStatus::Job(status);
                job.text = first_line(&outcome, 300);
                job.updated_at = self.now();
                rec.events.push(job);
            }
            Ok(None) => {}
            Err(err) => {
                return tracing::error!(chain = %chain_id, %repo, %err,
                    "land: its job was not read; the next start runs it again");
            }
        }
        let mut done = rw.clone();
        done.submit = None;
        let cmd = ChainCmd::Work(WorkCmd {
            repo: repo.to_owned(),
            edit: replace_with(done),
            tell: format!("{}\n{outcome}", result_heading(repo)),
        });
        let ready = rec.landings.iter().any(|w| w.landing.state == LandingState::Ready);
        let contends = rec.landings.iter().any(|w| w.landing.state == LandingState::Contending);
        let q = rec.landings.first().map(|w| w.landing.queue());
        if let Err(err) = self.apply(chain_id, cmd, rec).await {
            return tracing::error!(chain = %chain_id, %repo, %err,
                "land: its end was not recorded; the next start runs it again");
        }
        if let (true, Some(q)) = (ready, q) {
            self.kick_queue(q);
        }
        if contends {
            self.judge_rounds().await;
        }
    }

    /// The version rw's worktree holds, checked: what agents left is committed as the account that pushes, the checks and
    /// the path rules pass on it, and its change is measured from where it left the base the forge holds. The landing is
    /// returned unsaved, judging, with what the deciders did to its review (`Core::judge_change`); `queued` says whether
    /// its gates hold it.
    async fn version_of_work(&self, chain: &Chain, rw: &RepoWork, s: &Submit) -> Result<(Landing, Vec<Said>), String> {
        let f = self.forge_on(&rw.host).map_err(|e| e.to_string())?;
        let who = self.pusher(rw).await.map_err(|e| e.to_string())?;
        let dir = rw.path.clone();
        git(&dir, &["add", "-A"]).await?;
        if git(&dir, &["diff", "--cached", "--quiet"]).await.is_err() {
            let (name, email) = (format!("user.name={}", who.name), format!("user.email={}", who.email));
            git(&dir, &["-c", &name, "-c", &email, "commit", "-q", "-m", &s.title]).await?;
        }
        let base = base_on(&f, rw).await?;
        let from = git(&dir, &["rev-parse", "HEAD"]).await?;
        let forked = git(&dir, &["merge-base", &base, &from]).await?;
        if forked == from {
            return Err(format!(
                "the worktree holds nothing beyond {}: nothing to land",
                rw.base
            ));
        }
        let foreign = foreign_commits(&dir, &rw.base).await?;
        if !foreign.is_empty() {
            return Err(format!(
                "the worktree carries commits that are not this chain's work: they are already on another branch of the \
                 repository (a release or hotfix). Drop them (git rebase --onto, or rebuild from origin/{} with \
                 cherry-pick of the chain's own commits), then land again:\n{}",
                rw.base,
                foreign.join("\n")
            ));
        }
        let change = patch_id(&dir, &base, &from).await?;
        if change.is_empty() {
            return Err(format!(
                "the worktree changes nothing beyond {}: nothing to land",
                rw.base
            ));
        }
        let rails = rails_of(&dir, &base).await?;
        let mut gates = gates_of(chain.kind.gates(), &dir, &base, &from, &rails).await?;
        let paths = self
            .pass_rules(&dir, &forked, &from)
            .instrument(tracing::info_span!(attr::CHECKS_SPAN, { attr::CHAIN_ID } = %chain.id))
            .await?;
        let v = self.version_of(&chain.id, rw).await.map_err(|e| e.to_string())?;
        let id = new_id("ln_");
        let chair_id = chain.chair_id.clone();
        let chair = self
            .store
            .read(move |r| r.card(&chair_id))
            .await
            .map_err(|e| e.to_string())?;
        let facts = Change {
            title: &s.title,
            description: &s.description,
            paths: &paths,
            usual_effort: &chair.effort,
        };
        let reviewed = self
            .core
            .judge_change(chain, &id, &facts, gates.contains(&Gate::Security))
            .await;
        if reviewed.security {
            gates.push(Gate::Security);
        }
        let now = self.now();
        let landing = Landing {
            id,
            chain_id: chain.id.clone(),
            repo: rw.repo.clone(),
            host: rw.host.clone(),
            project: rw.project.clone(),
            base: rw.base.clone(),
            version: LandingVersion {
                from,
                forked,
                change,
                gates,
                by: v.by,
                title: s.title.clone(),
                description: s.description.clone(),
                paths,
                judged: reviewed.judged,
                effort: reviewed.effort.map(|e| e.as_str().to_owned()),
                plan: s.plan.clone(),
                ..LandingVersion::default()
            },
            priority: s.priority,
            priority_why: s.priority_why.clone(),
            state: LandingState::Judging,
            created_at: now,
            updated_at: now,
            ..Landing::default()
        };
        Ok((landing, reviewed.said))
    }

    /// Holds the change forked..from to the path rules and the repository's checks, and gives the paths it changes.
    async fn pass_rules(&self, dir: &str, forked: &str, from: &str) -> Result<Vec<String>, String> {
        let changed = git(dir, &["diff", "--name-only", forked, from]).await?;
        let paths: Vec<String> = changed
            .split('\n')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        let bad: Vec<&str> = paths.iter().map(String::as_str).filter(|p| forbidden_path(p)).collect();
        if !bad.is_empty() {
            return Err(format!(
                "these files may not be changed by a chain: {}. Revert them and land again",
                bad.join(", ")
            ));
        }
        let runner = self.checks.clone().unwrap_or_else(|| run_checks(the_jail()));
        let checks: Vec<Check> = runner(dir.to_owned(), paths.clone()).await;
        if let Some(c) = checks.iter().find(|c| !c.ok) {
            return Err(format!("{} failed; fix it and land again:\n{}", c.command, c.output));
        }
        Ok(paths)
    }

    /// Records l, a checked version, as the chain's newest landing on its repository: ready when every gate it needs
    /// approved its change (or none is required), judging otherwise, and the version before it, still on its way, is
    /// replaced. Says what the Chair is told.
    async fn queued(&self, chain: &Chain, mut l: Landing, said: &[Said], rec: &mut Records) -> Result<String, String> {
        let id = chain.id.clone();
        let (approvals, landings) = self
            .store
            .read(move |r| Ok::<_, store::Error>((r.approvals(&id)?, r.chain_landings(&id)?)))
            .await
            .map_err(|e| e.to_string())?;
        let round = self.round_of(chain).await.map_err(|e| e.to_string())?;
        let mine: Vec<&Landing> = landings
            .iter()
            .filter(|x| x.repo == l.repo && x.reverts.is_none())
            .collect();
        l.version.number = i64::try_from(mine.len()).unwrap_or(i64::MAX).saturating_add(1);
        let cause = Cause {
            actor: by_agent(&chain.chair_id),
            chain_id: chain.id.clone(),
        };
        for was in mine.into_iter().filter(|x| {
            matches!(
                x.state,
                LandingState::Judging | LandingState::Contending | LandingState::Ready
            )
        }) {
            let was = was.clone().returned(
                ReturnReason::Replaced,
                format!("version {} replaced it", l.version.number),
                l.created_at,
            );
            rec.landings.push(LandingWrite {
                landing: was,
                cause: cause.clone(),
            });
        }
        let at = l.created_at;
        let approved = approve(&mut l, &approvals, self.cfg.require_approvals, round.as_ref(), at);
        let text = if approved {
            approved_text(&l)
        } else {
            let missing = missing_approvals(&approvals, &l.repo, &l.version.change, &l.version.gates);
            let security = if l.version.gates.contains(&Gate::Security) {
                " It touches a rail (a path the repository declares in .remit/rails, or what a decider judged the change \
                 to alter), so a security reviewer (role \"security\") also judges it: a threat model of what the change \
                 opens, written in its verdict."
            } else {
                ""
            };
            let depth = l.version.effort.as_deref().map_or_else(String::new, |e| {
                format!(" Remit runs the reviewer and QC at {e} effort, as the change was judged risky.")
            });
            format!(
                "Version {} of {} is checked and waits for its gates:\n- {}\nHire a judge for each gate listed (a \
                 reviewer, role \"reviewer\", reviews the diff; a QC, role \"qc\", verifies the behaviour against the \
                 goal); each records its verdict with record_verdict on this version.{security}{depth} {} the moment the \
                 last approves; a commit after it needs land again.",
                l.version.number,
                l.repo,
                missing.join("\n- "),
                if round.is_some() {
                    "It contends in its round"
                } else {
                    "It enters the queue"
                }
            )
        };
        rec.decisions.push(decided(
            &chain.chair_id,
            format!("Land {} version {}: {}.", l.repo, l.version.number, l.version.title),
            first_line(&text, 200),
        ));
        // what a decider did to the review is told only when a judge will run: a version already approved has none
        for s in said.iter().filter(|_| !approved) {
            rec.decisions
                .push(decided(&chain.chair_id, format!("{}.", s.what), s.why.clone()));
        }
        rec.landings.push(LandingWrite { landing: l, cause });
        Ok(text)
    }
}

/// Moves landing l on at at when every gate it needs approved its change (any change, when require is false), with the
/// verdicts that did: whether it moved. It goes where an approved version of its chain goes (`round::approved_state`,
/// round the round its chain is an attempt of): ready in its queue, or, for an attempt, contending for its round's pick.
/// The one rule by which a version leaves judging. Pure.
pub(crate) fn approve(
    l: &mut Landing,
    approvals: &[model::Approval],
    require: bool,
    round: Option<&Round>,
    at: Time,
) -> bool {
    if require && !missing_approvals(approvals, &l.repo, &l.version.change, &l.version.gates).is_empty() {
        return false;
    }
    l.state = approved_state(round);
    if l.state == LandingState::Ready {
        l.ready_at = Some(at);
    }
    l.version.verdicts = approved_by(approvals, l);
    true
}

/// What the Chair hears of its version l once its gates approved it, as `approve` left it.
pub(crate) fn approved_text(l: &Landing) -> String {
    match &l.state {
        LandingState::Contending => format!(
            "Version {} of {} is approved and contends in its round: it enters the queue only if the round's judge \
             picks it, once every attempt has an approved version or has finished. You hear the pick.",
            l.version.number, l.repo
        ),
        LandingState::Returned { output, .. } => {
            format!("Version {} of {} does not land: {output}", l.version.number, l.repo)
        }
        _ => format!(
            "Version {} of {} is in the queue for {}: Remit combines it with the base, checks it and pushes it; you \
             hear when it lands.",
            l.version.number, l.repo, l.base
        ),
    }
}

/// The ids of the verdicts that approve l's change, one per gate it needs: the latest of each.
fn approved_by(approvals: &[model::Approval], l: &Landing) -> Vec<String> {
    l.version
        .gates
        .iter()
        .filter_map(|g| {
            approvals
                .iter()
                .rfind(|a| a.repo == l.repo && a.role == g.role() && a.change == l.version.change)
                .map(|a| a.id.clone())
        })
        .collect()
}
