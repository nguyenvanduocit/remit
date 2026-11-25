//! Landing's main path through the whole application (ADR-0114): the Chair lands a version, its gates' verdicts let it
//! into the queue, the queue lands it on the base itself, and a revert takes it back. The forge is a bare repository on
//! disk the fake forge reads its branches from; nothing reaches a real forge.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chains::forges::fake::FakeForge;
use chains::git::CheckRunner;
use chains::step::replace_with;
use model::{
    Approval, Card, Chain, Gate, Landing, LandingState, RepoWork, ReturnReason, RevertCause, Signs, Verdict, by_owner,
};
use shipping::gate::VerdictInput;
use shipping::land::LandInput;
use shipping::revert::RevertInput;

use crate::tests::{Rig, owner, rig_of};

const PROJECT: &str = "OWNER/remit";
const HOST: &str = "github.com";

/// A chain with the repository open in a worktree of Remit's clone, its forge a bare repository whose main declares
/// crates/auth a rail, and the change edit makes there committed.
pub(crate) struct Landed {
    pub(crate) r: Rig,
    pub(crate) origin: PathBuf,
    pub(crate) chain: Chain,
    pub(crate) chair: Card,
    pub(crate) base: String,
    _dir: tempfile::TempDir,
}

async fn chain_with_change(edit: impl FnOnce(&Path)) -> Landed {
    chain_seeded(|_| {}, |_| {}, edit).await
}

/// As chain_with_change, with seed adding to the base's first commit and configure to the application's configuration.
pub(crate) async fn chain_seeded(
    seed_with: impl FnOnce(&Path),
    configure: impl FnOnce(&mut orch::Config),
    edit: impl FnOnce(&Path),
) -> Landed {
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origin.git");
    sh(dir.path(), &["init", "-q", "--bare", "-b", "main", &s(&origin)]);
    sh(dir.path(), &["clone", "-q", &s(&origin), "seed"]);
    let seed = dir.path().join("seed");
    std::fs::create_dir_all(seed.join(".remit")).unwrap();
    std::fs::write(seed.join(shipping::gate::RAILS_FILE), "crates/auth\n").unwrap();
    std::fs::create_dir_all(seed.join("crates/auth")).unwrap();
    std::fs::write(seed.join("crates/auth/lib.rs"), "pub fn sign_in() {}\n").unwrap();
    std::fs::write(seed.join("README"), "remit\n").unwrap();
    seed_with(&seed);
    sh(&seed, &["add", "-A"]);
    sh(&seed, &["commit", "-q", "-m", "base"]);
    sh(&seed, &["push", "-q", "origin", "HEAD:main"]);
    let base = sh(&seed, &["rev-parse", "HEAD"]);
    let repos = dir.path().join("repos");
    let clone = repos.join(HOST).join(PROJECT);
    std::fs::create_dir_all(clone.parent().unwrap()).unwrap();
    sh(dir.path(), &["clone", "-q", &s(&origin), &s(&clone)]);

    let forge = Arc::new(FakeForge::default());
    forge.backed_by(&s(&origin));
    let f: Arc<dyn chains::forges::Forge> = forge;
    let passes: CheckRunner = Arc::new(|_dir, _changed| Box::pin(async { Vec::new() }));
    let r = rig_of(move |cfg| {
        cfg.core.forges.insert(HOST.to_owned(), f);
        cfg.core.require_approvals = true;
        cfg.core.repos_dir = repos;
        cfg.checks = Some(passes);
        configure(cfg);
    });
    let chain = r.create("r_land", "land it").await;
    let wt = dir.path().join("wt");
    sh(
        &clone,
        &["worktree", "add", "-q", "-b", "feature/mc-land", &s(&wt), "origin/main"],
    );
    edit(&wt);
    sh(&wt, &["add", "-A"]);
    sh(&wt, &["commit", "-q", "-m", "the change"]);
    let rw = RepoWork {
        repo: "remit".to_owned(),
        project: PROJECT.to_owned(),
        host: HOST.to_owned(),
        path: s(&wt),
        branch: "feature/mc-land".to_owned(),
        base: "main".to_owned(),
        ..RepoWork::default()
    };
    r.o.shipping
        .record_work(&chain.id, "remit", replace_with(rw), Vec::new())
        .await
        .unwrap();
    let id = chain.chair_id.clone();
    let chair = r.o.store.read(move |r| r.card(&id)).await.unwrap();
    Landed {
        r,
        origin,
        chain,
        chair,
        base,
        _dir: dir,
    }
}

impl Landed {
    pub(crate) async fn land(&self) -> Landing {
        let input = LandInput {
            title: "feat: say hello".to_owned(),
            priority: "high".to_owned(),
            priority_why: "the greeting waits on it".to_owned(),
            description: "Adds a greeting.".to_owned(),
            ..LandInput::default()
        };
        self.r.o.shipping.land(&self.chair, input).await.unwrap();
        self.until(|l| l.reverts.is_none(), |l| l.state == LandingState::Judging)
            .await
    }

    /// The chain's landing pick says, once done says it is so; a test fails after 30 s.
    pub(crate) async fn until(&self, pick: impl Fn(&Landing) -> bool, done: impl Fn(&Landing) -> bool) -> Landing {
        for _ in 0..300 {
            let id = self.chain.id.clone();
            let ls = self.r.o.store.read(move |r| r.chain_landings(&id)).await.unwrap();
            if let Some(l) = ls.into_iter().rev().find(|l| pick(l))
                && done(&l)
            {
                return l;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the landing never got there")
    }

    /// A judge of gate on the chain records its approve through record_verdict, as its tool does.
    pub(crate) async fn approve(&self, gate: Gate) {
        let signs = match gate {
            Gate::Reviewer => Signs::Review,
            Gate::Qc => Signs::Qc,
            Gate::Security => Signs::Security,
        };
        let mut judge = Card::default();
        judge.id = format!("c_{gate}");
        judge.home.clone_from(&self.chain.id);
        judge.name = gate.to_string();
        judge.signs = signs;
        let stored = self.r.o.store.update(move |tx| tx.put_card(judge)).await.unwrap();
        let input: VerdictInput = serde_json::from_value(serde_json::json!({
            "verdict": "approve",
            "notes": "read the diff and ran it",
            "findings": [{"keyResult": "", "item": "the change", "status": "met", "evidence": "hello.txt:1"}],
        }))
        .unwrap();
        self.r.o.shipping.record_verdict(&stored, input).await.unwrap();
    }

    fn origin_main(&self) -> String {
        sh(&self.origin, &["rev-parse", "main"])
    }
}

// The main path: a version waits for its gates, enters the queue on the last approve, and the queue lands it on the base
// itself as one commit on where the base stood, made by the forge's account and tied to its record.
#[tokio::test]
async fn an_approved_change_lands_on_the_base_by_itself() {
    let t = chain_with_change(|wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap()).await;
    let judging = t.land().await;
    assert_eq!(judging.version.gates, vec![Gate::Reviewer, Gate::Qc]);
    assert_eq!(judging.version.paths, vec!["hello.txt".to_owned()]);
    t.approve(Gate::Reviewer).await;
    assert_eq!(t.origin_main(), t.base, "nothing lands on one gate of two");
    t.approve(Gate::Qc).await;
    let landed = t
        .until(|l| l.id == judging.id, |l| l.state == LandingState::Landed)
        .await;
    assert_eq!(
        Some(t.origin_main()),
        landed.commit,
        "the base moved to the landed commit"
    );
    assert_eq!(
        sh(&t.origin, &["rev-parse", "main^"]),
        t.base,
        "one commit on where the base stood"
    );
    let message = sh(&t.origin, &["log", "-1", "--format=%an%n%B", "main"]);
    assert!(message.starts_with("Remit\nfeat: say hello"), "{message}");
    assert!(message.contains(&format!("Remit-Landing: {}", landed.id)), "{message}");
    assert_eq!(
        landed.version.verdicts.len(),
        2,
        "the verdicts that let it in are named"
    );

    // the owner takes it back: a revert lands ahead of everything, and the base holds the change no more
    let why = "the greeting is wrong".to_owned();
    let revert =
        t.r.o
            .shipping
            .revert_landing(
                by_owner("owner", "web"),
                RevertInput {
                    landing: landed.id.clone(),
                    why,
                },
            )
            .await
            .unwrap();
    let taken = t
        .until(
            |l| l.id == landed.id,
            |l| matches!(l.state, LandingState::Reverted { .. }),
        )
        .await;
    assert!(
        matches!(&taken.state, LandingState::Reverted { revert: r, cause: RevertCause::Overturn, .. } if *r == revert.id),
        "it says what took it back: {:?}",
        taken.state
    );
    assert_eq!(
        sh(&t.origin, &["rev-parse", "main^{tree}"]),
        sh(&t.origin, &["rev-parse", &format!("{}^{{tree}}", t.base)]),
        "the revert leaves the base as it was"
    );
}

// A change to a path the repository's .remit/rails names does not land on review and QC alone: it waits for a security
// verdict on the same change.
#[tokio::test]
async fn a_change_to_a_rail_waits_for_security() {
    let t =
        chain_with_change(|wt| std::fs::write(wt.join("crates/auth/lib.rs"), "pub fn sign_in() { open() }\n").unwrap())
            .await;
    let judging = t.land().await;
    assert!(
        judging.version.gates.contains(&Gate::Security),
        "{:?}",
        judging.version.gates
    );
    t.approve(Gate::Reviewer).await;
    t.approve(Gate::Qc).await;
    let still = t.until(|l| l.id == judging.id, |_| true).await;
    assert_eq!(
        still.state,
        LandingState::Judging,
        "a rail change lands without security"
    );
    t.approve(Gate::Security).await;
    t.until(|l| l.id == judging.id, |l| l.state == LandingState::Landed)
        .await;
}

// A version carried over waiting for its gates (migration 0119's open MR) whose verdicts were recorded before it became a
// landing enters the queue on the next sync and lands, with no verdict given again.
#[tokio::test]
async fn a_carried_version_its_gates_approved_lands_on_the_next_sync() {
    let t = chain_with_change(|wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap()).await;
    let judging = t.land().await;
    for gate in [Gate::Reviewer, Gate::Qc] {
        let a = Approval {
            id: format!("ap_{gate}"),
            chain_id: t.chain.id.clone(),
            repo: judging.repo.clone(),
            change: judging.version.change.clone(),
            card_id: format!("c_{gate}"),
            name: gate.to_string(),
            role: gate.role(),
            verdict: Verdict::Approve,
            notes: "approved on the MR".to_owned(),
            findings: Vec::new(),
            at: judging.created_at,
        };
        t.r.o.store.update(move |tx| tx.put_approval(&a)).await.unwrap();
    }
    t.r.o.shipping.sync_landings().await;
    let landed = t
        .until(|l| l.id == judging.id, |l| l.state == LandingState::Landed)
        .await;
    assert_eq!(
        Some(t.origin_main()),
        landed.commit,
        "the base moved to the landed commit"
    );
    assert_eq!(
        landed.version.verdicts.len(),
        2,
        "the carried verdicts are the ones named"
    );
}

// A version carried over ready (migration 0119's approved open MR) knows no fork point: it is where its commit meets its
// queue's base. Its files read from there, and the queue lands it on a base that moved meanwhile, keeping both changes.
#[tokio::test]
async fn a_carried_version_with_no_fork_point_shows_its_files_and_lands() {
    let t = chain_with_change(|wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap()).await;
    let judging = t.land().await;
    let elsewhere = t.origin.parent().unwrap().join("elsewhere");
    sh(
        t.origin.parent().unwrap(),
        &["clone", "-q", &s(&t.origin), &s(&elsewhere)],
    );
    std::fs::write(elsewhere.join("other.txt"), "other\n").unwrap();
    sh(&elsewhere, &["add", "-A"]);
    sh(&elsewhere, &["commit", "-q", "-m", "meanwhile"]);
    sh(&elsewhere, &["push", "-q", "origin", "HEAD:main"]);
    let mut carried = Landing {
        state: LandingState::Ready,
        ..judging.clone()
    };
    carried.version.forked = String::new();
    carried.version.paths = Vec::new();
    carried.version.verdicts = vec!["ap_reviewer".to_owned(), "ap_qc".to_owned()];
    t.r.o.store.update(move |tx| tx.put_landing(carried)).await.unwrap();

    let diff = t.r.o.shipping.landing_diff(&judging.id).await.unwrap();
    let files: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, vec!["hello.txt"], "its files, from where it forked");

    t.r.o.shipping.kick_queue(judging.queue());
    let landed = t
        .until(|l| l.id == judging.id, |l| l.state == LandingState::Landed)
        .await;
    assert_eq!(
        Some(t.origin_main()),
        landed.commit,
        "the base moved to the landed commit"
    );
    let tree = sh(&t.origin, &["ls-tree", "--name-only", "main"]);
    assert!(
        tree.contains("hello.txt") && tree.contains("other.txt"),
        "both changes are on the base: {tree}"
    );
}

// A draft MR carried over by migration 0123 is closed once on its forge, the fake one here, saying where its change
// went, and forgotten: the next start closes nothing twice.
#[tokio::test]
async fn a_carried_draft_mr_is_closed_once_on_its_forge() {
    let forge = Arc::new(FakeForge::default());
    forge.open_mr(7);
    let f: Arc<dyn chains::forges::Forge> = forge.clone();
    let r = rig_of(move |cfg| {
        cfg.core.forges.insert(HOST.to_owned(), f);
    });
    let mr = store::CarriedMr {
        landing: "ln_carried".to_owned(),
        host: HOST.to_owned(),
        project: PROJECT.to_owned(),
        iid: 7,
        url: "https://github.com/OWNER/remit/pull/7".to_owned(),
    };
    r.o.store.update(move |tx| tx.carry_mr(&mr)).await.unwrap();

    assert_eq!(r.o.shipping.close_carried_mrs().await.unwrap(), 1);
    let calls = forge.calls();
    assert!(
        calls.len() == 1 && calls[0].starts_with("close_mr 7 Remit lands every change itself"),
        "{calls:?}"
    );
    assert!(r.o.store.read(|r| r.carried_mrs()).await.unwrap().is_empty());
    assert_eq!(r.o.shipping.close_carried_mrs().await.unwrap(), 0, "closed once");
    assert_eq!(forge.calls().len(), 1, "the forge is not asked again");
}

// Deleting a chain takes its work back: a version of it waiting in the queue is withdrawn with it and never lands.
#[tokio::test]
async fn a_deleted_chains_ready_landing_is_withdrawn_and_never_lands() {
    let t = chain_with_change(|wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap()).await;
    let judging = t.land().await;
    // ready as its gates' approves leave it, with the queue not yet run on it
    let ready = Landing {
        state: LandingState::Ready,
        ..judging.clone()
    };
    t.r.o.store.update(move |tx| tx.put_landing(ready)).await.unwrap();
    t.r.o
        .core
        .finish(owner("r_finish"), t.chain.id.clone(), "done".to_owned())
        .await
        .unwrap();
    t.r.o.core.delete(owner("r_delete"), t.chain.id.clone()).await.unwrap();

    let id = judging.id.clone();
    let withdrawn = t.r.o.store.read(move |r| r.landing(&id)).await.unwrap();
    assert!(
        matches!(
            &withdrawn.state,
            LandingState::Returned {
                reason: ReturnReason::Withdrawn,
                ..
            }
        ),
        "{:?}",
        withdrawn.state
    );
    t.r.o.shipping.kick_queue(withdrawn.queue());
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(t.origin_main(), t.base, "a withdrawn landing reached the base");
    }
}

// The owner squash-merged the change outside the queue and the base then changed the same lines: combining it again
// conflicts, yet its work is on the base, so the queue records it landed at the squash, pushes nothing and sends nothing
// back.
#[tokio::test]
async fn a_change_squashed_outside_the_queue_is_found_landed() {
    let t = chain_with_change(|wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap()).await;
    let judging = t.land().await;
    let squash = t.push_elsewhere("hello.txt", "hello\n", "squashed: say hello");
    t.push_elsewhere("hello.txt", "hello, world\n", "say more");
    let main = t.origin_main();
    t.approve(Gate::Reviewer).await;
    t.approve(Gate::Qc).await;
    let landed = t.until(|l| l.id == judging.id, |l| !l.state.open()).await;
    assert_eq!(landed.state, LandingState::Landed, "{:?}", landed.state);
    assert_eq!(landed.commit, Some(squash), "landed at the commit that brought it");
    assert_eq!(t.origin_main(), main, "the queue pushed nothing");
}

// A version that came back for a conflict before the owner merged it outside the queue is found landed on the next sync,
// though nothing else in its queue moves and its chain finished: the queue looks again for what it sent back.
#[tokio::test]
async fn a_returned_change_merged_outside_the_queue_is_found_landed() {
    let t = chain_with_change(|wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap()).await;
    let judging = t.land().await;
    let returned = judging.clone().returned(
        ReturnReason::Conflict,
        "combined with main it conflicts in: hello.txt".to_owned(),
        judging.created_at,
    );
    t.r.o.store.update(move |tx| tx.put_landing(returned)).await.unwrap();
    t.r.o
        .core
        .finish(owner("r_finish"), t.chain.id.clone(), "done".to_owned())
        .await
        .unwrap();
    let id = t.chain.id.clone();
    let chain = t.r.o.store.read(move |r| r.chain(&id)).await.unwrap();
    assert_eq!(*chain.status(), model::ChainStatus::Done);
    let squash = t.push_elsewhere("hello.txt", "hello\n", "squashed: say hello");
    t.push_elsewhere("hello.txt", "hello, world\n", "say more");
    t.r.o.shipping.sync_landings().await;
    let landed = t
        .until(|l| l.id == judging.id, |l| l.state == LandingState::Landed)
        .await;
    assert_eq!(landed.commit, Some(squash), "landed at the commit that brought it");
}

impl Landed {
    /// Someone outside Remit commits file as text on the forge's main, saying message: the commit.
    fn push_elsewhere(&self, file: &str, text: &str, message: &str) -> String {
        let dir = self.origin.parent().unwrap();
        let elsewhere = dir.join("elsewhere");
        if !elsewhere.exists() {
            sh(dir, &["clone", "-q", &s(&self.origin), &s(&elsewhere)]);
        }
        sh(&elsewhere, &["pull", "-q", "--ff-only", "origin", "main"]);
        std::fs::write(elsewhere.join(file), text).unwrap();
        sh(&elsewhere, &["add", "-A"]);
        sh(&elsewhere, &["commit", "-q", "-m", message]);
        sh(&elsewhere, &["push", "-q", "origin", "HEAD:main"]);
        sh(&elsewhere, &["rev-parse", "HEAD"])
    }
}

pub(crate) fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Runs git in dir as Ann and gives what it printed; a test fails when git does.
#[expect(
    clippy::disallowed_methods,
    reason = "the tests make their fixture repositories with git as a person would, outside the code under test"
)]
pub(crate) fn sh(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Ann")
        .env("GIT_AUTHOR_EMAIL", "ann@example.com")
        .env("GIT_COMMITTER_NAME", "Ann")
        .env("GIT_COMMITTER_EMAIL", "ann@example.com")
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
