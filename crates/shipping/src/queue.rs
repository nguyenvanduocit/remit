//! The merge queue's shell (ADR-0114): one runner per queue (a repository's host, path and base) at a time, in memory, so
//! one writer moves a base. A pass reads the queue, takes the next batch (landing.rs), combines each landing on the base
//! the forge holds as one squashed commit made by the account the forge's CLI is logged in as, checks the combined tree
//! in the jail and pushes it through the `Lander`. Each step is recorded before the next acts outside: the batch is
//! recorded combining before it is combined, and checking, with the commits and the push it makes, before the push, so a
//! restart finds a push that reached the base and records it landed, and puts any other back in the queue. What the
//! queue records is caused by Remit, in no chain, so a chain's own watches on its landings hear it. A landing whose work a
//! commit on the base already carries (merged, squashed or picked outside the queue) is recorded landed at that commit,
//! both before it would go back and, for one gone back, each time the base moves. A conflict, an empty change, a red
//! check on a landing alone or a refused push goes back to its Chair, saying why; a red batch of more goes back to the
//! queue and the next batch is half as big.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, PoisonError};

use forge::Account;
use model::{Cause, InboxItem, InboxKind, InboxStatus, Landing, LandingState, QueueKey, ReturnReason, by_remit};

use crate::Shipping;
use crate::direct_push::DirectPush;
use crate::landing::{Lander, Pushed, grown, halved, next_batch, taken_back};
use crate::repo::slug;
use crate::repochecks::{run_checks, the_jail};
use chains::Result;
use chains::apply::Records;
use chains::chain_core::{first_line, new_id};
use chains::fact::{FactValue, fact_write, keys};
use chains::git::{RepoRemote, git, path_str, repo_command, short, tail};
use chains::live::LandingWrite;
use chains::step::{ChainCmd, WorkCmd};

/// Who the queue acts as, in what it records.
pub(crate) const QUEUE: &str = "queue";

/// What the queue records is caused by Remit, in no chain: a chain's watches on its own landings hear it.
pub(crate) fn queue_cause() -> Cause {
    Cause {
        actor: by_remit(QUEUE),
        chain_id: String::new(),
    }
}

/// What one queue's runner holds in memory: whether it runs, whether it was kicked while it ran, the size of its next
/// batch, and the base commit its landings gone back were last looked for on. A restart starts again from one, and looks
/// for them again.
#[derive(Debug, Clone)]
pub struct QueueRun {
    running: bool,
    again: bool,
    pub(crate) k: usize,
    looked_on: Option<String>,
}

impl Default for QueueRun {
    fn default() -> Self {
        Self {
            running: false,
            again: false,
            k: 1,
            looked_on: None,
        }
    }
}

/// The queues' runners, by queue.
pub type QueueRuns = std::sync::Mutex<HashMap<QueueKey, QueueRun>>;

/// What combining a landing on a commit gives.
enum Combined {
    /// The commit it became.
    Commit(String),
    /// It conflicts in paths, as said.
    Conflict { paths: Vec<String>, said: String },
    /// It changes nothing there.
    Empty,
}

/// How what landed reached the base: pushed by the queue, or found there, brought by a commit made outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reached {
    Pushed,
    Found,
}

/// One landing of a batch, as it was combined.
struct Part {
    landing: Landing,
    commit: String,
}

impl Shipping {
    /// Has q's runner run a pass now: started when none runs, told to go again when one does.
    pub fn kick_queue(&self, q: QueueKey) {
        {
            let mut runs = self.queues.lock().unwrap_or_else(PoisonError::into_inner);
            let run = runs.entry(q.clone()).or_default();
            if run.running {
                run.again = true;
                return;
            }
            run.running = true;
        }
        let Some(me) = self.me() else { return };
        let key = q.clone();
        if !self.background(move |_| async move { me.run_queue(key).await }) {
            self.queue_run(&q, |r| r.running = false);
        }
    }

    pub(crate) fn queue_run<T>(&self, q: &QueueKey, edit: impl FnOnce(&mut QueueRun) -> T) -> T {
        let mut runs = self.queues.lock().unwrap_or_else(PoisonError::into_inner);
        edit(runs.entry(q.clone()).or_default())
    }

    /// Runs passes of q until nothing in it is ready, or a pass fails (the next sync runs it again).
    async fn run_queue(&self, q: QueueKey) {
        loop {
            self.queue_run(&q, |r| r.again = false);
            let went = loop {
                if self.is_stopping() {
                    break Ok(());
                }
                match self.pass(&q).await {
                    Ok(true) => {}
                    Ok(false) => break Ok(()),
                    Err(err) => break Err(err),
                }
            };
            if let Err(err) = went {
                tracing::warn!(queue = %q.named(), %err, "queue: the pass stopped; the next sync runs it again");
            }
            let again = self.queue_run(&q, |r| {
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

    /// One pass of q: the landings a crash left checking settled, then the next batch landed or sent back. Whether there
    /// was a batch.
    async fn pass(&self, q: &QueueKey) -> Result<bool, String> {
        let key = q.clone();
        let queue = self
            .store
            .read(move |r| r.queue_landings(&key))
            .await
            .map_err(|e| e.to_string())?;
        let clone = path_str(&self.clone_dir(&RepoRemote {
            host: q.host.clone(),
            project: q.project.clone(),
        }));
        git(&clone, &["fetch", "-q", "origin"]).await?;
        let head = git(&clone, &["rev-parse", &format!("origin/{}", q.base)]).await?;
        if self.queue_run(q, |r| r.looked_on.as_deref() != Some(head.as_str())) {
            self.find_returned(q, &clone, &queue, &head).await?;
            self.queue_run(q, |r| r.looked_on = Some(head));
        }
        let combining: Vec<Landing> = queue
            .iter()
            .filter(|l| l.state == LandingState::Combining)
            .map(|l| Landing {
                state: LandingState::Ready,
                ..l.clone()
            })
            .collect();
        if !combining.is_empty() {
            // a pass stopped while it combined: nothing of it reached the forge
            self.write_queue_moves(combining).await?;
            return Ok(true);
        }
        let checking: Vec<&Landing> = queue.iter().filter(|l| l.state == LandingState::Checking).collect();
        if !checking.is_empty() {
            self.settle_checking(q, &clone, &checking).await?;
            return Ok(true);
        }
        let held = self.held_stages(&queue).await;
        let k = self.queue_run(q, |r| r.k);
        let batch = next_batch(&queue, k, &held);
        if batch.is_empty() {
            return Ok(false);
        }
        self.land_batch(q, &clone, &queue, batch).await?;
        Ok(true)
    }

    /// The ready stages of queue the rollout rule holds now: they wait, and the queue goes on past them.
    async fn held_stages(&self, queue: &[Landing]) -> Vec<String> {
        let mut held = Vec::new();
        for l in queue
            .iter()
            .filter(|l| l.state == LandingState::Ready && l.version.plan.is_some())
        {
            let vetted = match (self.chain(&l.chain_id).await, l.version.plan.as_ref()) {
                (Ok(chain), Some(plan)) => self.vet_stage(&chain, &l.host, &l.project, plan).await,
                (Err(err), _) => Err(err),
                (Ok(_), None) => Ok(()),
            };
            if let Err(err) = vetted {
                tracing::info!(landing = %l.id, %err, "queue: the stage waits");
                held.push(l.id.clone());
            }
        }
        held
    }

    /// Settles the landings a pass left checking when it stopped: landed when the base the forge holds carries their
    /// push, back in the queue otherwise.
    async fn settle_checking(&self, q: &QueueKey, clone: &str, checking: &[&Landing]) -> Result<(), String> {
        let f = self.forge_on(&q.host).map_err(|e| e.to_string())?;
        let at = forge_head(&f, q).await?;
        let now = self.now();
        let mut landed = Vec::new();
        let mut back = Vec::new();
        for l in checking {
            let mut l = (*l).clone();
            let on_base = match &l.push {
                Some(push) => is_ancestor(clone, push, &at).await,
                None => false,
            };
            l.updated_at = now;
            if on_base {
                l.state = LandingState::Landed;
                l.landed_at = Some(now);
                landed.push(l);
            } else {
                l.state = LandingState::Ready;
                back.push(l);
            }
        }
        self.write_queue_moves(back).await?;
        self.record_landed(q, landed, Reached::Pushed).await
    }

    /// Records landed each landing of queue gone back to its Chair that is looked for (`looked_for`) and that a commit on
    /// the base at head carries now: the owner merged it outside the queue after it came back.
    async fn find_returned(&self, q: &QueueKey, clone: &str, queue: &[Landing], head: &str) -> Result<(), String> {
        let now = self.now();
        let mut found = Vec::new();
        for l in queue.iter().filter(|l| looked_for(l, queue, now)) {
            if let Some(commit) = carried_by(clone, l, head).await {
                found.push(landed_at(l.clone(), commit, now));
            }
        }
        self.record_landed(q, found, Reached::Found).await
    }

    /// Combines batch on the base the forge holds, checks it and pushes it, recording each step.
    async fn land_batch(
        &self,
        q: &QueueKey,
        clone: &str,
        queue: &[Landing],
        batch: Vec<Landing>,
    ) -> Result<(), String> {
        let f = self.forge_on(&q.host).map_err(|e| e.to_string())?;
        let who = self
            .pusher_on(&q.host)
            .await
            .map_err(|e| format!("who the queue pushes as: {e}"))?;
        let onto = forge_head(&f, q).await?;
        if git(clone, &["cat-file", "-e", &format!("{onto}^{{commit}}")])
            .await
            .is_err()
        {
            git(clone, &["fetch", "-q", "origin", &q.base]).await?;
        }
        let now = self.now();
        let combining: Vec<Landing> = batch
            .into_iter()
            .map(|mut l| {
                l.state = LandingState::Combining;
                l.updated_at = now;
                l
            })
            .collect();
        let batch = self.write_queue_moves(combining).await?;
        let mut top = onto.clone();
        let mut parts: Vec<Part> = Vec::new();
        let mut returned: Vec<Landing> = Vec::new();
        let mut found: Vec<Landing> = Vec::new();
        for l in batch {
            let (reason, said) = match self.combine(clone, &top, &l, queue, &who).await? {
                Combined::Commit(c) => {
                    top.clone_from(&c);
                    parts.push(Part { landing: l, commit: c });
                    continue;
                }
                Combined::Conflict { paths, said } if l.reverts.is_some() => {
                    // a revert that conflicts takes back what landed on the change after it first, newest first
                    self.revert_later(queue, &l, &paths, &said).await?;
                    return Ok(());
                }
                Combined::Conflict { said, .. } => (ReturnReason::Conflict, said),
                Combined::Empty => (
                    ReturnReason::Empty,
                    "combined with the base it changes nothing, and no commit on the base since it forked brought it"
                        .to_owned(),
                ),
            };
            match carried_by(clone, &l, &onto).await {
                Some(commit) => found.push(landed_at(l, commit, now)),
                None => returned.push(l.returned(reason, said, now)),
            }
        }
        self.return_to_chairs(returned).await?;
        self.record_landed(q, found, Reached::Found).await?;
        if parts.is_empty() {
            return Ok(());
        }
        let batch_id = new_id("lb_");
        let checking: Vec<Landing> = parts
            .iter()
            .map(|p| {
                let mut l = p.landing.clone();
                l.state = LandingState::Checking;
                (l.batch, l.onto) = (Some(batch_id.clone()), Some(onto.clone()));
                (l.commit, l.push) = (Some(p.commit.clone()), Some(top.clone()));
                (l.checks, l.base_checks) = (None, None);
                l.updated_at = now;
                l
            })
            .collect();
        let checking = self.write_queue_moves(checking).await?;
        if let Err(output) = self.check_combined(q, clone, &onto, &top, &checking).await {
            return self.checks_failed(q, checking, &output).await;
        }
        let lander = DirectPush { forge: f };
        match lander.land(clone, q, &onto, &top).await? {
            Pushed::Landed => {
                let landed: Vec<Landing> = checking
                    .into_iter()
                    .map(|mut l| {
                        l.state = LandingState::Landed;
                        l.landed_at = Some(self.now());
                        l.updated_at = self.now();
                        l
                    })
                    .collect();
                let n = landed.len();
                self.queue_run(q, |r| r.k = grown(r.k.max(n)));
                self.record_landed(q, landed, Reached::Pushed).await
            }
            Pushed::Moved => {
                let back = checking.into_iter().map(|mut l| {
                    l.state = LandingState::Ready;
                    l
                });
                self.write_queue_moves(back.collect()).await.map(|_| ())
            }
            Pushed::Refused(why) => self.refused(q, checking, &why).await,
        }
    }

    /// Combines l on top: its change from where it forked (a revert: its target's change backwards) merged with top, as
    /// one commit on top made by who.
    async fn combine(
        &self,
        clone: &str,
        top: &str,
        l: &Landing,
        queue: &[Landing],
        who: &Account,
    ) -> Result<Combined, String> {
        let (base, theirs, co_authors) = if let Some(target) = l.reverted_one() {
            let Some(commit) = queue.iter().find(|t| t.id == target).and_then(|t| t.commit.clone()) else {
                return Ok(Combined::Conflict {
                    paths: Vec::new(),
                    said: format!("the landing it reverts, {target}, is not on its queue's base"),
                });
            };
            (commit.clone(), format!("{commit}^"), Vec::new())
        } else {
            let v = &l.version;
            let forked = fork_point(clone, l).await?;
            let authors = git(clone, &["log", "--format=%an <%ae>", &format!("{forked}..{}", v.from)]).await?;
            let mine = format!("{} <{}>", who.name, who.email);
            let mut co: Vec<String> = authors
                .split('\n')
                .filter(|a| !a.is_empty() && *a != mine)
                .map(str::to_owned)
                .collect();
            co.sort();
            co.dedup();
            (forked, v.from.clone(), co)
        };
        let tree = match merge_trees(clone, &base, top, &theirs).await? {
            Merged::Clean(tree) => tree,
            Merged::Conflict(paths) => {
                let said = format!(
                    "combined with {} at {} it conflicts in: {}",
                    l.base,
                    short(top),
                    paths.join(", ")
                );
                return Ok(Combined::Conflict { paths, said });
            }
        };
        if tree == tree_of(clone, top).await? {
            return Ok(Combined::Empty);
        }
        let serves = self
            .chain(&l.chain_id)
            .await
            .ok()
            .and_then(|c| c.serves)
            .map(|s| format!("{}/{}", s.objective, s.key_result))
            .unwrap_or_default();
        let message = crate::landing::commit_message(l, &serves, &co_authors);
        let mut cmd = repo_command(
            clone,
            "git",
            &["commit-tree", "--no-gpg-sign", "-p", top, "-m", &message, &tree],
        );
        cmd.envs([
            ("GIT_AUTHOR_NAME", who.name.as_str()),
            ("GIT_AUTHOR_EMAIL", who.email.as_str()),
            ("GIT_COMMITTER_NAME", who.name.as_str()),
            ("GIT_COMMITTER_EMAIL", who.email.as_str()),
        ]);
        let made = cmd.output().await.map_err(|err| format!("git commit-tree: {err}"))?;
        if !made.status.success() {
            return Err(format!(
                "git commit-tree: {}: {}",
                made.status,
                tail(&String::from_utf8_lossy(&made.stderr), 400)
            ));
        }
        Ok(Combined::Commit(
            String::from_utf8_lossy(&made.stdout).trim().to_owned(),
        ))
    }

    /// Runs the repository's checks on top, the batch combined on onto, in a worktree of the clone of its own: what
    /// failed, when something did. A landing alone on the base it forked from was checked as it is by land, so its tree
    /// is not checked again; a version carried over from a merge request names no fork point, so its tree is.
    async fn check_combined(
        &self,
        q: &QueueKey,
        clone: &str,
        onto: &str,
        top: &str,
        batch: &[Landing],
    ) -> Result<(), String> {
        if let [one] = batch
            && one.reverts.is_none()
            && one.version.forked == onto
        {
            return Ok(());
        }
        let dir = path_str(
            &self
                .cfg
                .work_root
                .join("landing")
                .join(slug(&format!("{}-{}-{}", q.host, q.project, q.base), 80)),
        );
        if Path::new(&dir).exists() {
            git(clone, &["worktree", "remove", "--force", &dir]).await.ok();
        }
        git(clone, &["worktree", "prune"]).await.ok();
        git(clone, &["worktree", "add", "--detach", "--force", &dir, top]).await?;
        let mut paths: Vec<String> = batch.iter().flat_map(|l| l.version.paths.clone()).collect();
        paths.sort();
        paths.dedup();
        let runner = self.checks.clone().unwrap_or_else(|| run_checks(the_jail()));
        let ran = runner(dir.clone(), paths).await;
        git(clone, &["worktree", "remove", "--force", &dir]).await.ok();
        match ran.iter().find(|c| !c.ok) {
            Some(c) => Err(format!("{} failed:\n{}", c.command, c.output)),
            None => Ok(()),
        }
    }

    /// A batch whose combined tree failed its checks: one alone goes back to its Chair; more go back to the queue, and
    /// the next batch is half as big, so the one that broke it is found alone.
    async fn checks_failed(&self, q: &QueueKey, batch: Vec<Landing>, output: &str) -> Result<(), String> {
        let now = self.now();
        if batch.len() == 1 {
            let back: Vec<Landing> = batch
                .into_iter()
                .map(|l| l.returned(ReturnReason::Checks, output.to_owned(), now))
                .collect();
            return self.return_to_chairs(back).await;
        }
        let n = batch.len();
        self.queue_run(q, |r| r.k = halved(n));
        let back: Vec<Landing> = batch
            .into_iter()
            .map(|mut l| {
                l.state = LandingState::Ready;
                l.updated_at = now;
                l
            })
            .collect();
        self.write_queue_moves(back).await.map(|_| ())
    }

    /// A push the forge refused: each landing goes back to its Chair with the forge's words, and the Assistant is asked
    /// what to do about a base Remit may not push to.
    async fn refused(&self, q: &QueueKey, batch: Vec<Landing>, why: &str) -> Result<(), String> {
        let now = self.now();
        let chain = batch.first().map(|l| l.chain_id.clone()).unwrap_or_default();
        let back: Vec<Landing> = batch
            .into_iter()
            .map(|l| l.returned(ReturnReason::Refused, why.to_owned(), now))
            .collect();
        self.return_to_chairs(back).await?;
        let chair = self.chain(&chain).await.map(|c| c.chair_id).unwrap_or_default();
        let item = InboxItem {
            id: new_id("i_"),
            home: chain.clone(),
            card_id: chair,
            kind: InboxKind::Question,
            title: format!("{} refused Remit's push to {}", q.host, q.named()),
            detail: format!(
                "Remit lands every change by pushing it to the base itself (ADR-0114). The forge refused the push:\n\n{}\n\n\
                 Decide how the account Remit pushes as may push to {} (a protected branch rule that allows it, or \
                 another base), or tell the Chair to keep the change in its worktree until then.",
                first_line(why, 600),
                q.base
            ),
            status: InboxStatus::Open,
            created_at: now,
            ..InboxItem::default()
        };
        let rec = Records {
            asked: vec![item],
            ..Records::default()
        };
        self.commit(None, None, rec)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// What landed, as it reached the base: each landing recorded, its Chair told, the facts that it landed written, the
    /// chains whose open work touches the same paths told, its worktree brought up to it, a stage's guardrail watched, and
    /// a revert's target taken back.
    async fn record_landed(&self, q: &QueueKey, landed: Vec<Landing>, reached: Reached) -> Result<(), String> {
        if landed.is_empty() {
            return Ok(());
        }
        let now = self.now();
        let mut targets = Vec::new();
        for l in &landed {
            let Some(id) = l.reverted_one().map(str::to_owned) else {
                continue;
            };
            if let Ok(t) = self.store.read(move |r| r.landing(&id)).await {
                targets.push(taken_back(t, l, now));
            }
        }
        let mut by_chain: BTreeMap<String, Vec<Landing>> = BTreeMap::new();
        for l in landed.iter().cloned().chain(targets) {
            by_chain.entry(l.chain_id.clone()).or_default().push(l);
        }
        for (chain_id, ls) in by_chain {
            let mut rec = Records::default();
            let mut told = Vec::new();
            for l in ls.iter().filter(|l| l.state == LandingState::Landed) {
                let commit = l.commit.clone().unwrap_or_default();
                let v = FactValue {
                    chain: chain_id.clone(),
                    landing: l.id.clone(),
                    repo: l.repo.clone(),
                    base: l.base.clone(),
                    commit: commit.clone(),
                    paths: l.version.paths.clone(),
                    ..FactValue::default()
                };
                let by = by_remit(QUEUE);
                if l.reverts.is_none() {
                    rec.readings
                        .push(fact_write(keys::chain_landed(), &v, by.clone(), &chain_id).map_err(|e| e.to_string())?);
                }
                rec.readings
                    .push(fact_write(keys::repo_landed(), &v, by, &chain_id).map_err(|e| e.to_string())?);
                let what = if reached == Reached::Found {
                    format!(
                        "Found {} version {} on {} at {}, landed outside the queue: {}.",
                        l.repo,
                        l.version.number,
                        l.base,
                        short(&commit),
                        l.version.title
                    )
                } else if l.reverts.is_none() {
                    format!(
                        "Landed {} version {} on {} as {}: {}.",
                        l.repo,
                        l.version.number,
                        l.base,
                        short(&commit),
                        l.version.title
                    )
                } else {
                    format!("Reverted on {} as {}: {}.", l.base, short(&commit), l.version.title)
                };
                rec.decisions.push(crate::land::decided(
                    &self.chair_of(&chain_id).await,
                    what.clone(),
                    l.version.description.clone(),
                ));
                told.push(what);
            }
            let repo = ls.first().map(|l| l.repo.clone()).unwrap_or_default();
            rec.landings = ls
                .iter()
                .map(|l| LandingWrite {
                    landing: l.clone(),
                    cause: queue_cause(),
                })
                .collect();
            let tell = if told.is_empty() {
                String::new()
            } else if reached == Reached::Found {
                told.join("\n")
            } else {
                format!(
                    "{}\nThe forge's pipeline on it is followed: a failure where the base before it passed is reverted \
                     by Remit, and you are told.",
                    told.join("\n")
                )
            };
            self.write_on_chain(&chain_id, &repo, tell, rec)
                .await
                .map_err(|e| e.to_string())?;
        }
        for l in &landed {
            self.after_landed(q, l).await;
        }
        self.kick();
        Ok(())
    }

    /// What follows a landing outside its record: the landing sense follows its pipeline, a stage's guardrail is watched,
    /// the chain's worktree is brought up to it, and chains whose open work touches the same paths are told.
    async fn after_landed(&self, q: &QueueKey, l: &Landing) {
        self.follow_chain_landings(&l.chain_id).await;
        self.watch_stage(l).await;
        if l.reverts.is_none() {
            self.bring_up(l).await;
        }
        self.tell_overlapping(q, l).await;
    }

    /// Merges the commit l landed as into its chain's worktree when the worktree still holds the version l is and nothing
    /// more, so the next version is measured from the base after it; the Chair merges it itself otherwise.
    async fn bring_up(&self, l: &Landing) {
        let Ok(chain) = self.chain(&l.chain_id).await else {
            return;
        };
        let Some(rw) = chain.repos.iter().find(|rw| rw.repo == l.repo) else {
            return;
        };
        let head = git(&rw.path, &["rev-parse", "HEAD"]).await.unwrap_or_default();
        let clean = git(&rw.path, &["status", "--porcelain"])
            .await
            .is_ok_and(|s| s.is_empty());
        let Some(commit) = l.commit.as_deref().filter(|_| head == l.version.from && clean) else {
            return;
        };
        let merged = async {
            let who = self.pusher(rw).await.map_err(|e| e.to_string())?;
            git(&rw.path, &["fetch", "-q", "origin", &l.base]).await?;
            let (name, email) = (format!("user.name={}", who.name), format!("user.email={}", who.email));
            git(
                &rw.path,
                &["-c", &name, "-c", &email, "merge", "-q", "--no-edit", commit],
            )
            .await
        };
        if let Err(err) = merged.await {
            git(&rw.path, &["merge", "--abort"]).await.ok();
            tracing::warn!(landing = %l.id, %err, "queue: the worktree was not brought up to what landed");
        }
    }

    /// Tells the Chair of each other chain with a landing on its way in q whose paths overlap l's that the base moved
    /// under it.
    async fn tell_overlapping(&self, q: &QueueKey, l: &Landing) {
        let key = q.clone();
        let Ok(queue) = self.store.read(move |r| r.queue_landings(&key)).await else {
            return;
        };
        let mut told: Vec<String> = Vec::new();
        for other in queue
            .iter()
            .filter(|o| o.state.open() && o.chain_id != l.chain_id && o.reverts.is_none())
        {
            let shared: Vec<&String> = other
                .version
                .paths
                .iter()
                .filter(|p| l.version.paths.contains(p))
                .collect();
            if shared.is_empty() || told.contains(&other.chain_id) {
                continue;
            }
            told.push(other.chain_id.clone());
            let text = format!(
                "{} landed on {} as {} and changed paths your version {} also changes: {}. Merge origin/{} into your \
                 worktree and land again if it changes what your change means; the queue returns your version if the two \
                 conflict.",
                l.version.title,
                l.base,
                short(l.commit.as_deref().unwrap_or_default()),
                other.version.number,
                shared.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "),
                l.base
            );
            if let Err(err) = self
                .write_on_chain(&other.chain_id, &other.repo, text, Records::default())
                .await
            {
                tracing::warn!(chain = %other.chain_id, %err, "queue: an overlapping chain was not told");
            }
        }
    }

    /// A revert that conflicts with what landed after its target: each landing after the target on the base whose paths
    /// overlap the conflict's (paths) is reverted first, newest first, and the revert waits behind them.
    async fn revert_later(
        &self,
        queue: &[Landing],
        revert: &Landing,
        paths: &[String],
        conflict: &str,
    ) -> Result<(), String> {
        let Some(target) = revert.reverted_one().and_then(|id| queue.iter().find(|t| t.id == id)) else {
            return Ok(());
        };
        let mut later: Vec<&Landing> = queue
            .iter()
            .filter(|l| crate::landing::revertible(l, queue) && l.landed_at > target.landed_at && l.id != target.id)
            .filter(|l| l.version.paths.iter().any(|p| paths.contains(p)))
            .collect();
        later.sort_by_key(|l| std::cmp::Reverse(l.landed_at));
        if later.is_empty() {
            let back = revert
                .clone()
                .returned(ReturnReason::Conflict, conflict.to_owned(), self.now());
            return self.return_to_chairs(vec![back]).await;
        }
        let why = format!(
            "the revert of {:?} conflicts with it ({conflict}), so it is taken back first",
            target.version.title
        );
        for l in later {
            self.queue_revert(l, model::RevertCause::Conflict, by_remit(QUEUE), &why)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Landings sent back to their Chairs, each told why.
    async fn return_to_chairs(&self, back: Vec<Landing>) -> Result<(), String> {
        for l in back {
            let why = match &l.state {
                LandingState::Returned { reason, output } => Some((*reason, output.clone())),
                _ => None,
            };
            let tell = match &why {
                Some((reason, output)) => format!(
                    "{} version {} came back from the queue ({reason}): {}\nFix it in the worktree and land again.",
                    l.repo,
                    l.version.number,
                    tail(output, 1500)
                ),
                None => String::new(),
            };
            let chair = self.chair_of(&l.chain_id).await;
            let mut rec = Records::default();
            if let Some((reason, _)) = &why {
                rec.decisions.push(crate::land::decided(
                    &chair,
                    format!(
                        "{} version {} came back from the queue: {reason}.",
                        l.repo, l.version.number
                    ),
                    String::new(),
                ));
            }
            rec.landings.push(LandingWrite {
                cause: queue_cause(),
                landing: l.clone(),
            });
            self.write_on_chain(&l.chain_id, &l.repo, tell, rec)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Stores moves of landings the Chair hears nothing of (back in the queue, checking), and gives them as stored.
    async fn write_queue_moves(&self, moved: Vec<Landing>) -> Result<Vec<Landing>, String> {
        if moved.is_empty() {
            return Ok(moved);
        }
        let rec = Records {
            landings: moved
                .iter()
                .map(|l| LandingWrite {
                    landing: l.clone(),
                    cause: queue_cause(),
                })
                .collect(),
            ..Records::default()
        };
        self.commit(None, None, rec).await.map_err(|e| e.to_string())?;
        let ids: Vec<String> = moved.iter().map(|l| l.id.clone()).collect();
        self.store
            .read(move |r| ids.iter().map(|id| r.landing(id)).collect::<store::Result<Vec<_>>>())
            .await
            .map_err(|e| e.to_string())
    }

    /// The Chair of chain id; "" when the chain is gone.
    pub(crate) async fn chair_of(&self, id: &str) -> String {
        self.chain(id).await.map(|c| c.chair_id).unwrap_or_default()
    }

    /// Stores rec on chain id, telling its Chair tell (none when empty) when the chain still has repo open and runs; a
    /// landing outlives its chain, so on a chain that is gone or left the repository rec is stored alone.
    pub(crate) async fn write_on_chain(&self, id: &str, repo: &str, tell: String, rec: Records) -> Result<()> {
        let holds = self
            .chain(id)
            .await
            .is_ok_and(|c| c.repos.iter().any(|rw| rw.repo == repo));
        if !holds {
            return self.commit(None, None, rec).await.map(|_| ());
        }
        let cmd = ChainCmd::Work(WorkCmd {
            repo: repo.to_owned(),
            edit: Arc::new(|_| Ok(())),
            tell,
        });
        self.apply(id, cmd, rec).await.map(|_| ())
    }
}

/// Where q's base stands on its forge, as the forge says.
async fn forge_head(f: &Arc<dyn chains::forges::Forge>, q: &QueueKey) -> Result<String, String> {
    let r = forge::Repo {
        host: q.host.clone(),
        project: q.project.clone(),
    };
    match tokio::time::timeout(chains::forges::FORGE_TIMEOUT, f.branch_head(&r, &q.base)).await {
        Ok(Ok(at)) => Ok(at),
        Ok(Err(err)) => Err(format!("{} did not say where {} stands: {err}", f.name(), q.base)),
        Err(_) => Err(format!("{} did not say where {} stands in time", f.name(), q.base)),
    }
}

/// The commit l's version is measured from, in Remit's clone at clone: the base commit its version forked from, or, for a
/// version carried over from a merge request that names none, where its commit meets its queue's base.
pub(crate) async fn fork_point(clone: &str, l: &Landing) -> Result<String, String> {
    if !l.version.forked.is_empty() {
        return Ok(l.version.forked.clone());
    }
    git(clone, &["merge-base", &l.version.from, &format!("origin/{}", l.base)]).await
}

async fn is_ancestor(dir: &str, a: &str, b: &str) -> bool {
    git(dir, &["merge-base", "--is-ancestor", a, b]).await.is_ok()
}

/// How long after a landing went back the queue looks for it on its base: long enough for the owner to merge it outside
/// the queue, and no longer, so what was given up is not looked for at each move of the base for ever.
const LOOK_FOR: time::Duration = time::Duration::WEEK;

/// Whether l is looked for on its base each time the base moves, at now: it went back for a conflict or for changing
/// nothing less than `LOOK_FOR` ago, and it is the newest version of its chain's change to its repository among
/// landings. Its chain may have finished: what it says of the landing is still to be made true. Pure.
pub(crate) fn looked_for(l: &Landing, landings: &[Landing], now: model::Time) -> bool {
    matches!(
        l.state,
        LandingState::Returned {
            reason: ReturnReason::Conflict | ReturnReason::Empty,
            ..
        }
    ) && l.reverts.is_none()
        && now.0 < l.updated_at.0.saturating_add(LOOK_FOR)
        && !landings
            .iter()
            .any(|o| o.chain_id == l.chain_id && o.repo == l.repo && o.version.number > l.version.number)
}

/// l recorded landed at commit, a commit on its base the queue did not push.
fn landed_at(mut l: Landing, commit: String, now: model::Time) -> Landing {
    l.state = LandingState::Landed;
    l.commit = Some(commit);
    (l.batch, l.onto, l.push) = (None, None, None);
    (l.checks, l.base_checks) = (None, None);
    l.landed_at = Some(now);
    l.updated_at = now;
    l
}

/// The commit on the base's first-parent line up to head that brought l's change there outside the queue, in Remit's
/// clone at clone: the one that merged or fast-forwarded its commit, or else the first since it forked whose tree holds
/// all of its change (a squash, a cherry-pick, a rebase); none for a revert, a version that changes nothing, and a change
/// no commit brought. Looking that fails is logged and finds none: the landing goes back as it would have, and the queue
/// goes on.
async fn carried_by(clone: &str, l: &Landing, head: &str) -> Option<String> {
    match look_for(clone, l, head).await {
        Ok(found) => found,
        Err(err) => {
            tracing::warn!(landing = %l.id, %err, "queue: could not look for the landing on its base");
            None
        }
    }
}

async fn look_for(clone: &str, l: &Landing, head: &str) -> Result<Option<String>, String> {
    if l.reverts.is_some() {
        return Ok(None);
    }
    let from = &l.version.from;
    if is_ancestor(clone, from, head).await {
        let line = git(
            clone,
            &[
                "rev-list",
                "--first-parent",
                "--ancestry-path",
                "--reverse",
                &format!("{from}..{head}"),
            ],
        )
        .await?;
        let Some(first) = line.lines().next() else {
            return Ok(Some(from.clone()));
        };
        // its commit sits on the line itself (a fast-forward), or the first commit after it on the line merged it
        let parent = git(clone, &["rev-parse", &format!("{first}^1")]).await?;
        return Ok(Some(if parent == *from {
            from.clone()
        } else {
            first.to_owned()
        }));
    }
    let forked = fork_point(clone, l).await?;
    if tree_of(clone, &forked).await? == tree_of(clone, from).await? {
        return Ok(None);
    }
    let line = git(
        clone,
        &["rev-list", "--first-parent", "--reverse", &format!("{forked}..{head}")],
    )
    .await?;
    for c in line.lines() {
        if let Merged::Clean(tree) = merge_trees(clone, &forked, c, from).await?
            && tree == tree_of(clone, c).await?
        {
            return Ok(Some(c.to_owned()));
        }
    }
    Ok(None)
}

/// What merging theirs into ours from base gives, without a worktree.
enum Merged {
    /// The tree it makes.
    Clean(String),
    /// The paths it conflicts in.
    Conflict(Vec<String>),
}

async fn merge_trees(clone: &str, base: &str, ours: &str, theirs: &str) -> Result<Merged, String> {
    let out = repo_command(
        clone,
        "git",
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--merge-base",
            base,
            ours,
            theirs,
        ],
    )
    .output()
    .await
    .map_err(|err| format!("git merge-tree: {err}"))?;
    let printed = String::from_utf8_lossy(&out.stdout).into_owned();
    match out.status.code() {
        Some(0) => Ok(Merged::Clean(
            printed.lines().next().unwrap_or_default().trim().to_owned(),
        )),
        Some(1) => Ok(Merged::Conflict(
            printed
                .lines()
                .skip(1)
                .take_while(|p| !p.is_empty())
                .map(str::to_owned)
                .collect(),
        )),
        _ => Err(format!(
            "git merge-tree: {}: {}",
            out.status,
            tail(&String::from_utf8_lossy(&out.stderr), 400)
        )),
    }
}

async fn tree_of(clone: &str, commit: &str) -> Result<String, String> {
    git(clone, &["rev-parse", &format!("{commit}^{{tree}}")]).await
}
