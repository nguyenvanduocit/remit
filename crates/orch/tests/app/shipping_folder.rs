//! A chain's work in place in a folder through the whole application (ADR-0137): the Chair opens the folder, the agents
//! change it, `land` records the version between two checkpoints, its gate's verdict lands it with no queue (as a commit
//! on the folder's own branch when it is a Git repository, ADR-0139), and a checkpoint is restored unless that would
//! overwrite the owner's own edit. No forge, clone or worktree is involved.

use std::path::Path;
use std::time::Duration;

use chains::Error;
use model::{Card, Chain, Gate, Landing, LandingState, Signs};
use shipping::folder::RestoreInput;
use shipping::gate::VerdictInput;
use shipping::land::LandInput;

use crate::shipping_land::sh;
use crate::tests::{Rig, rig_of};

struct Notes {
    r: Rig,
    chain: Chain,
    chair: Card,
    folder: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

async fn chair_of(r: &Rig, chain: &Chain) -> Card {
    let id = chain.chair_id.clone();
    r.o.store.read(move |r| r.card(&id)).await.unwrap()
}

async fn notes() -> Notes {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("plan.txt"), "one\n").unwrap();
    std::fs::write(folder.join(".gitignore"), "*.log\n").unwrap();
    let folders = dir.path().join("folders");
    let r = rig_of(move |cfg| {
        cfg.core.folders_dir = folders;
        cfg.core.require_approvals = true;
    });
    let chain = r.create("r_folder", "write the notes").await;
    let chair = chair_of(&r, &chain).await;
    let folder = std::fs::canonicalize(&folder).unwrap();
    Notes {
        r,
        chain,
        chair,
        folder,
        _dir: dir,
    }
}

impl Notes {
    fn landing_input() -> LandInput {
        LandInput {
            title: "docs: second draft".to_owned(),
            priority: "low".to_owned(),
            priority_why: "nothing waits on the notes".to_owned(),
            description: "Rewrites the plan.".to_owned(),
            ..LandInput::default()
        }
    }

    async fn landing(&self, state: impl Fn(&Landing) -> bool) -> Landing {
        for _ in 0..300 {
            let id = self.chain.id.clone();
            let ls = self.r.o.store.read(move |r| r.chain_landings(&id)).await.unwrap();
            if let Some(l) = ls.into_iter().rev().find(|l| state(l)) {
                return l;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the landing never got there")
    }

    async fn approve(&self) {
        let mut judge = Card::default();
        judge.id = chains::chain_core::new_id("c_");
        judge.home.clone_from(&self.chain.id);
        judge.name = "reviewer".to_owned();
        judge.signs = Signs::Review;
        let stored = self.r.o.store.update(move |tx| tx.put_card(judge)).await.unwrap();
        let input: VerdictInput = serde_json::from_value(serde_json::json!({
            "verdict": "approve",
            "notes": "read the diff between the checkpoints",
            "behaviourEvidence": "read both drafts",
            "findings": [{"keyResult": "", "item": "the plan", "status": "met", "evidence": "plan.txt:1"}],
        }))
        .unwrap();
        self.r.o.shipping.record_verdict(&stored, input).await.unwrap();
    }

    async fn restore(&self, checkpoint: &str) -> chains::Result<String> {
        let input = RestoreInput {
            repo: s(&self.folder),
            checkpoint: checkpoint.to_owned(),
            why: "the second draft is worse".to_owned(),
            ..RestoreInput::default()
        };
        self.r
            .o
            .shipping
            .restore_checkpoint(model::by_agent(&self.chair.id), &self.chain.id, input)
            .await
    }
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// The main path: the folder opens with a checkpoint and keeps no `.git`; a second chain is refused it; a change lands once
// its gate approves, measured from the checkpoint at the open and landed with no queue; a restore refuses to overwrite
// the owner's own edit, and with none writes the folder back.
#[tokio::test(flavor = "multi_thread")]
async fn a_folder_worked_in_place_lands_on_its_gates_and_is_restored_to_a_checkpoint() {
    let t = notes().await;
    let opened = t.r.o.shipping.open_repo(&t.chair, &s(&t.folder)).await.unwrap();
    assert!(opened.in_place(), "an absolute path opens the folder in place");
    assert!(
        !t.folder.join(".git").exists(),
        "Remit never writes a .git into the folder"
    );

    let other = t.r.create("r_folder2", "write them too").await;
    let mut rival = t.chair.clone();
    rival.home.clone_from(&other.id);
    let held = t.r.o.shipping.open_repo(&rival, &s(&t.folder)).await;
    assert!(
        matches!(&held, Err(Error::Refused(why)) if why.contains(&t.chain.id)),
        "another chain holds the folder: {held:?}"
    );

    // the agents change the folder itself
    std::fs::write(t.folder.join("plan.txt"), "two\n").unwrap();
    std::fs::write(t.folder.join("extra.txt"), "new\n").unwrap();
    std::fs::write(t.folder.join("noise.log"), "ignored\n").unwrap();
    t.r.o.shipping.land(&t.chair, Notes::landing_input()).await.unwrap();
    let judging = t.landing(|l| l.state == LandingState::Judging).await;
    assert!(judging.queue.is_none(), "a folder's version belongs to no queue");
    assert_eq!(judging.version.gates, vec![Gate::Reviewer]);
    let mut paths = judging.version.paths.clone();
    paths.sort();
    assert_eq!(
        paths,
        ["extra.txt".to_owned(), "plan.txt".to_owned()],
        "the change is the diff between the checkpoints, and honours .gitignore"
    );

    t.approve().await;
    let landed = t
        .landing(|l| l.id == judging.id && l.state == LandingState::Landed)
        .await;
    assert!(landed.landed_at.is_some());
    assert_eq!(landed.version.verdicts.len(), 1, "the review that landed it is named");
    assert!(landed.commit.is_none(), "a folder that is not Git is committed nowhere");

    // the owner edits a file the restore would overwrite: it refuses, names the file and writes nothing
    std::fs::write(t.folder.join("plan.txt"), "mine\n").unwrap();
    let refused = t.restore(&short(&opened_checkpoint(&opened))).await;
    assert!(
        matches!(&refused, Err(Error::Refused(why)) if why.contains("plan.txt")),
        "{refused:?}"
    );
    assert_eq!(std::fs::read_to_string(t.folder.join("plan.txt")).unwrap(), "mine\n");
    assert!(t.folder.join("extra.txt").exists(), "nothing was written");

    // with the owner's edit put back to what the newest checkpoint holds, the folder goes back to the one it was opened at
    std::fs::write(t.folder.join("plan.txt"), "two\n").unwrap();
    t.restore(&short(&opened_checkpoint(&opened))).await.unwrap();
    assert_eq!(std::fs::read_to_string(t.folder.join("plan.txt")).unwrap(), "one\n");
    assert!(!t.folder.join("extra.txt").exists(), "a file made since is removed");
    assert!(
        t.folder.join("noise.log").exists(),
        "an ignored file is not Remit's to touch"
    );
}

// In a folder that is a Git repository, an approved version is one commit on the branch the folder is on: exactly its
// paths as the checkpoint holds them, the Chair's message, no hook run, and the user's own staged edit of another file
// left staged and unchanged.
#[tokio::test(flavor = "multi_thread")]
async fn a_version_of_a_git_folder_lands_as_a_commit_on_its_branch() {
    let t = notes().await;
    let f = t.folder.as_path();
    std::fs::write(f.join("old.txt"), "old\n").unwrap();
    std::fs::write(f.join("todo.md"), "a\n").unwrap();
    sh(f, &["init", "-q", "-b", "main"]);
    sh(f, &["config", "user.name", "Owner"]);
    sh(f, &["config", "user.email", "owner@example.com"]);
    sh(f, &["add", "-A"]);
    sh(f, &["commit", "-q", "-m", "start"]);
    let hook = f.join(".git/hooks/post-commit");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch {}\n", f.join("hook-ran").display())).unwrap();
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    // the user's own edit, staged before the chain opens the folder: not the chain's change
    std::fs::write(f.join("todo.md"), "a\nb\n").unwrap();
    sh(f, &["add", "todo.md"]);
    let start = sh(f, &["rev-parse", "HEAD"]);

    t.r.o.shipping.open_repo(&t.chair, &s(f)).await.unwrap();
    std::fs::write(f.join("plan.txt"), "two\n").unwrap();
    std::fs::write(f.join("extra.txt"), "new\n").unwrap();
    std::fs::remove_file(f.join("old.txt")).unwrap();
    t.r.o.shipping.land(&t.chair, Notes::landing_input()).await.unwrap();
    let judging = t.landing(|l| l.state == LandingState::Judging).await;
    t.approve().await;
    let landed = t
        .landing(|l| l.id == judging.id && l.state == LandingState::Landed)
        .await;

    let head = sh(f, &["rev-parse", "HEAD"]);
    assert_eq!(landed.commit.as_deref(), Some(head.as_str()), "the landing names its commit");
    assert_eq!(sh(f, &["rev-parse", "HEAD^"]), start, "one commit on the branch the folder is on");
    assert_eq!(sh(f, &["symbolic-ref", "--short", "HEAD"]), "main", "the checkout did not move");
    assert_eq!(sh(f, &["log", "-1", "--format=%s%n%n%b"]), "docs: second draft\n\nRewrites the plan.");
    assert_eq!(
        sh(f, &["show", "--name-status", "--format=", "HEAD"]),
        "A\textra.txt\nD\told.txt\nM\tplan.txt",
        "exactly the version's paths"
    );
    assert_eq!(sh(f, &["show", "HEAD:plan.txt"]), "two");
    assert_eq!(sh(f, &["diff", "--cached", "--name-only"]), "todo.md", "the user's staged edit stays staged");
    assert_eq!(sh(f, &["status", "--porcelain", "--", "plan.txt", "extra.txt", "old.txt"]), "", "the commit matches the folder");
    assert_eq!(std::fs::read_to_string(f.join("todo.md")).unwrap(), "a\nb\n");
    assert!(!f.join("hook-ran").exists(), "no hook runs");
}

fn opened_checkpoint(rw: &model::RepoWork) -> String {
    match &rw.place {
        model::WorkPlace::Folder { opened, .. } => opened.clone(),
        model::WorkPlace::Worktree(_) => panic!("a folder"),
    }
}

fn short(commit: &str) -> String {
    commit.chars().take(8).collect()
}
