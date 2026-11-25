//! Competing attempts' main path through the whole application (ADR-0115): a Chair opens two attempts at a key result
//! whose check is a command, each lands a version its gates approve, neither enters the queue by itself, a round judge is
//! started once both contend, and its pick lands through the queue while the other goes back and never lands. The forge
//! is a bare repository on disk the fake forge reads its branches from; nothing reaches a real forge.

use std::sync::Arc;
use std::time::Duration;

use chains::forges::fake::FakeForge;
use chains::git::CheckRunner;
use chains::step::replace_with;
use model::{
    Card, Chain, ChainStatus, Check, CheckKind, Gate, KeyResult, Landing, LandingState, RepoWork, ReturnReason,
    RoundState, Serves, Signs,
};
use orch::propose::{ProposeInput, ServesInput};
use shipping::gate::VerdictInput;
use shipping::land::LandInput;
use shipping::round::PickAttemptInput;

use crate::shipping_land::{s, sh};
use crate::tests::{Agents, NOW, Rig, rig_with};

const PROJECT: &str = "OWNER/remit";
const HOST: &str = "github.com";

/// The chains' landings, read as stored.
async fn landings_of(r: &Rig, chain: &str) -> Vec<Landing> {
    let id = chain.to_owned();
    r.o.store.read(move |r| r.chain_landings(&id)).await.unwrap()
}

/// What answers: whether so holds, asked again every 100 ms; a test fails after 30 s.
async fn until<T>(what: &str, mut so: impl AsyncFnMut() -> Option<T>) -> T {
    for _ in 0..300 {
        if let Some(t) = so().await {
            return t;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("never: {what}")
}

/// A judge of gate on chain records its approve through record_verdict, as its tool does.
async fn approve(r: &Rig, chain: &Chain, gate: Gate) {
    let mut judge = Card::default();
    judge.id = format!("c_{}_{gate}", chain.id);
    judge.home.clone_from(&chain.id);
    judge.name = gate.to_string();
    judge.signs = match gate {
        Gate::Reviewer => Signs::Review,
        Gate::Qc => Signs::Qc,
        Gate::Security => Signs::Security,
    };
    let stored = r.o.store.update(move |tx| tx.put_card(judge)).await.unwrap();
    let input: VerdictInput = serde_json::from_value(serde_json::json!({
        "verdict": "approve",
        "notes": "read the diff and ran it",
        "findings": [{"keyResult": "", "item": "the change", "status": "met", "evidence": "the file:1"}],
    }))
    .unwrap();
    r.o.shipping.record_verdict(&stored, input).await.unwrap();
}

// The main path: two attempts' approved versions contend instead of landing, the round's judge picks one, and only the
// pick reaches the base; the other goes back to its Chair, not picked.
#[tokio::test]
async fn a_rounds_pick_lands_and_the_other_attempt_never_does() {
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origin.git");
    sh(dir.path(), &["init", "-q", "--bare", "-b", "main", &s(&origin)]);
    sh(dir.path(), &["clone", "-q", &s(&origin), "seed"]);
    let seed = dir.path().join("seed");
    std::fs::write(seed.join("README"), "remit\n").unwrap();
    sh(&seed, &["add", "-A"]);
    sh(&seed, &["commit", "-q", "-m", "base"]);
    sh(&seed, &["push", "-q", "origin", "HEAD:main"]);
    let base = sh(&seed, &["rev-parse", "HEAD"]);
    let clone = dir.path().join("repos").join(HOST).join(PROJECT);
    std::fs::create_dir_all(clone.parent().unwrap()).unwrap();
    sh(dir.path(), &["clone", "-q", &s(&origin), &s(&clone)]);

    let forge = Arc::new(FakeForge::default());
    forge.backed_by(&s(&origin));
    let f: Arc<dyn chains::forges::Forge> = forge;
    let passes: CheckRunner = Arc::new(|_dir, _changed| Box::pin(async { Vec::new() }));
    let repos = dir.path().join("repos");
    let r = rig_with(
        move |cfg| {
            cfg.core.forges.insert(HOST.to_owned(), f);
            cfg.core.require_approvals = true;
            cfg.core.repos_dir = repos;
            cfg.checks = Some(passes);
        },
        Arc::new(Agents::default()),
        Arc::new(|| NOW),
    );

    // the owner's chain, whose Chair opens attempts at a key result refining its own, one with a command check
    let home = r.create("r_home", "make the greeting land").await;
    let home_id = home.id.clone();
    let home = r.o.store.read(move |r| r.chain(&home_id)).await.unwrap();
    let mine = home.serves.clone().unwrap();
    let mut check = Check::of(CheckKind::Command);
    check.repo = "remit".to_owned();
    check.name = "test -f hello.txt".to_owned();
    let kr = KeyResult {
        id: "kr_greet".to_owned(),
        text: "the greeting is in the repository".to_owned(),
        check: Some(check),
        parent: Some(mine.key_result.clone()),
        ..KeyResult::default()
    };
    let serves = Serves {
        objective: mine.objective.clone(),
        key_result: kr.id.clone(),
    };
    // a landing of the key result's work that held earlier: its share is 2, room for two attempts
    let mut earlier = Chain::default();
    earlier.id = "ch_earlier".to_owned();
    earlier.serves = Some(serves.clone());
    earlier.set_status(ChainStatus::Done);
    let held = Landing {
        id: "ln_earlier".to_owned(),
        chain_id: earlier.id.clone(),
        state: LandingState::Landed,
        landed_at: Some(NOW),
        ..Landing::default()
    };
    let objective = mine.objective.clone();
    r.o.store
        .update(move |tx| {
            tx.put_key_result(&objective, kr)?;
            tx.put_chain(earlier)?;
            tx.put_landing(held).map(|_| ())
        })
        .await
        .unwrap();
    let home_chair = home.chair_id.clone();
    let chair = r.o.store.read(move |r| r.card(&home_chair)).await.unwrap();
    let mut attempts = Vec::new();
    for title in ["Greet in English", "Greet in Vietnamese"] {
        let input = ProposeInput {
            title: title.to_owned(),
            goal: "Add hello.txt with a greeting.".to_owned(),
            why: "the greeting waits on it".to_owned(),
            serves: Some(ServesInput {
                objective: serves.objective.clone(),
                key_result: serves.key_result.clone(),
            }),
            attempt: true,
            ..ProposeInput::default()
        };
        let (ch, _) = r.o.propose(&chair, input).await.unwrap();
        assert!(ch.attempt.is_some(), "an attempt names its round");
        attempts.push(ch);
    }
    assert_eq!(attempts[0].attempt, attempts[1].attempt, "both join one round");

    // each starts, works in a worktree of its own and lands a version its gates approve
    let mut started = Vec::new();
    for (i, a) in attempts.iter().enumerate() {
        let id = a.id.clone();
        let ch = until("the attempt starts", async || {
            r.o.tick().await;
            let id = id.clone();
            let ch = r.o.store.read(move |r| r.chain(&id)).await.unwrap();
            (!ch.chair_id.is_empty()).then_some(ch)
        })
        .await;
        let wt = dir.path().join(format!("wt{i}"));
        let branch = format!("feature/attempt-{i}");
        sh(
            &clone,
            &["worktree", "add", "-q", "-b", &branch, &s(&wt), "origin/main"],
        );
        std::fs::write(wt.join("hello.txt"), format!("hello {i}\n")).unwrap();
        std::fs::write(wt.join(format!("attempt{i}.txt")), "mine\n").unwrap();
        sh(&wt, &["add", "-A"]);
        sh(&wt, &["commit", "-q", "-m", "the greeting"]);
        let rw = RepoWork {
            repo: "remit".to_owned(),
            project: PROJECT.to_owned(),
            host: HOST.to_owned(),
            path: s(&wt),
            branch,
            base: "main".to_owned(),
            ..RepoWork::default()
        };
        r.o.shipping
            .record_work(&ch.id, "remit", replace_with(rw), Vec::new())
            .await
            .unwrap();
        let chair_id = ch.chair_id.clone();
        let attempt_chair = r.o.store.read(move |r| r.card(&chair_id)).await.unwrap();
        let input = LandInput {
            title: format!("feat: greet ({i})"),
            priority: "high".to_owned(),
            priority_why: "the greeting waits on it".to_owned(),
            description: "Adds a greeting.".to_owned(),
            ..LandInput::default()
        };
        r.o.shipping.land(&attempt_chair, input).await.unwrap();
        let cid = ch.id.clone();
        until("the version is judged", async || {
            let ls = landings_of(&r, &cid).await;
            ls.into_iter().find(|l| l.state == LandingState::Judging)
        })
        .await;
        approve(&r, &ch, Gate::Reviewer).await;
        approve(&r, &ch, Gate::Qc).await;
        started.push(ch);
    }
    for ch in &started {
        let ls = landings_of(&r, &ch.id).await;
        assert!(
            ls.iter().all(|l| l.state == LandingState::Contending),
            "an attempt's approved version waits for the pick: {:?}",
            ls.iter().map(|l| l.state.kind()).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        sh(&origin, &["rev-parse", "main"]),
        base,
        "nothing lands before the pick"
    );

    // both contend: a round judge is started, and picks the second attempt
    let at = serves.clone();
    let judge_id = until("the round's judge is started", async || {
        let at = at.clone();
        let rounds = r.o.store.read(move |r| r.key_result_rounds(&at)).await.unwrap();
        rounds.into_iter().find_map(|round| match round.state {
            RoundState::Judging { judge } => Some(judge),
            _ => None,
        })
    })
    .await;
    let judge = r.o.store.read(move |r| r.card(&judge_id)).await.unwrap();
    let (other, pick) = (&started[0], &started[1]);
    r.o.shipping
        .pick_attempt(
            &judge,
            PickAttemptInput {
                attempt: pick.id.clone(),
                why: "its check passes and it greets the customer in their language".to_owned(),
            },
        )
        .await
        .unwrap();

    let pid = pick.id.clone();
    let landed = until("the pick lands", async || {
        let ls = landings_of(&r, &pid).await;
        ls.into_iter().find(|l| l.state == LandingState::Landed)
    })
    .await;
    assert_eq!(
        Some(sh(&origin, &["rev-parse", "main"])),
        landed.commit,
        "the pick is on the base"
    );
    let back = landings_of(&r, &other.id).await;
    assert!(
        back.iter().all(|l| matches!(
            l.state,
            LandingState::Returned {
                reason: ReturnReason::NotPicked,
                ..
            }
        )),
        "{:?}",
        back.iter().map(|l| &l.state).collect::<Vec<_>>()
    );
    r.o.shipping.kick_queue(landed.queue());
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let tree = sh(&origin, &["ls-tree", "--name-only", "main"]);
        assert!(
            !tree.contains("attempt0.txt"),
            "the attempt not picked reached the base: {tree}"
        );
    }
    let at = serves.clone();
    let rounds = r.o.store.read(move |r| r.key_result_rounds(&at)).await.unwrap();
    assert!(
        matches!(&rounds[0].state, RoundState::Picked { attempt, why, .. } if *attempt == pick.id && why.contains("check passes")),
        "the judge's reasons stay on the round: {:?}",
        rounds[0].state
    );
}
