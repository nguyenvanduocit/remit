//! A repository's release through the whole application (ADR-0116): what lands is released by the procedure its
//! `.remit/release/` declares, bakes under its guard, and a breach is taken back by code (the way back run, every
//! landing it carried reverted, the reverts released forward) with no agent's turn. The steps run in a fake runner that
//! records what each was given; nothing reaches a real forge or registry, and `remit update` is `true`.

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use model::{
    Checks, ChecksState, DecisionKind, Gate, Grant, GrantHolder, HolderKind, Landing, LandingState, Reading, Release,
    ReleaseGuard, ReleaseState, SignalKey, by_owner,
};
use shipping::release::Connection;
use shipping::release_run::{ReleasePauseInput, StepRunner};
use shipping::remit_facts::{Instance, InstanceHealth};

use crate::shipping_land::{Landed, chain_seeded, sh};
use crate::tests::{NOW, REGISTRY, REGISTRY_TOKEN};

const GUARD: &str = "fact:remit.health";
/// The repository the test server runs (-update-repo), the one `chain_seeded` makes.
const HERE: &str = "github.com/OWNER/remit";

/// Runs no program: records each step's spec and exits as told, in order (0 once told nothing), a step that exits 0
/// saying it published version 1.0.0.
#[derive(Default)]
struct Steps {
    ran: Mutex<Vec<jail::Spec>>,
    exits: Mutex<VecDeque<i32>>,
}

impl Steps {
    fn exiting(exits: &[i32]) -> Arc<Self> {
        Arc::new(Self {
            exits: Mutex::new(exits.iter().copied().collect()),
            ..Self::default()
        })
    }
    fn ran(&self) -> Vec<jail::Spec> {
        self.ran.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

#[async_trait]
impl StepRunner for Steps {
    async fn run_step(
        &self,
        _repo: &str,
        _c: &[Connection],
        home: &Path,
        spec: jail::Spec,
    ) -> Result<jail::RunResult, String> {
        let exit = self
            .exits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or(0);
        if exit == 0 {
            std::fs::write(
                home.join("release-out.json"),
                r#"{"released": true, "version": "1.0.0"}"#,
            )
            .unwrap();
        }
        self.ran.lock().unwrap_or_else(PoisonError::into_inner).push(spec);
        Ok(jail::RunResult {
            stdout: format!("exited {exit}"),
            stderr: String::new(),
            exit_code: exit,
            timed_out: false,
        })
    }
}

fn step(dir: &Path, name: &str) {
    let path = dir.join(".remit/release").join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn guard(dir: &Path, line: &str) {
    std::fs::write(dir.join(".remit/release/guard"), format!("{line}\n")).unwrap();
}

/// A repository whose procedure publishes in one step and takes back in one, guarded as guard says (none: no guard
/// file), run by steps, on a server configure sets up.
async fn releasing(
    steps: &Arc<Steps>,
    guard_line: Option<String>,
    configure: impl FnOnce(&mut orch::Config),
) -> Landed {
    let runner: Arc<dyn StepRunner> = steps.clone();
    chain_seeded(
        |seed| {
            step(seed, "10-publish");
            step(seed, "back/10-yank");
            if let Some(line) = &guard_line {
                guard(seed, line);
            }
        },
        |cfg| {
            cfg.release_steps = orch::ReleaseSteps::Given(runner);
            configure(cfg);
        },
        hello,
    )
    .await
}

/// The chain's change: a file of greeting.
fn hello(wt: &Path) {
    std::fs::write(wt.join("hello.txt"), "hello\n").unwrap();
}

fn baked() -> Option<String> {
    Some(format!("{GUARD} 60 value.healthy == false"))
}

/// This server runs the repository; with updates, `remit update` is `true` in releases.
fn this_server(cfg: &mut orch::Config, updates: Option<&Path>) {
    cfg.core.update_repo = HERE.to_owned();
    if let Some(releases) = updates {
        cfg.core.releases_dir = releases.to_path_buf();
        cfg.updater = Some(orch::Updater {
            program: PathBuf::from("/usr/bin/true"),
            lock: releases.join("update.lock"),
            log: releases.join("update.log"),
            ..orch::Updater::default()
        });
    }
}

impl Landed {
    /// The releases once done says of them, oldest first; a test fails after 15 s.
    async fn releases_until(&self, done: impl Fn(&[Release]) -> bool) -> Vec<Release> {
        for _ in 0..150 {
            let rs = self.r.o.store.read(|r| r.releases()).await.unwrap();
            if done(&rs) {
                return rs;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!(
            "the releases never got there: {:?}",
            self.r.o.store.read(|r| r.releases()).await.unwrap()
        )
    }

    /// fact:remit.health says healthy, read now.
    async fn health(&self, healthy: bool) {
        let reading = Reading {
            key: SignalKey(GUARD.to_owned()),
            at: NOW,
            value: Some(serde_json::json!({ "healthy": healthy }).into()),
            ..Reading::default()
        };
        self.r.o.store.update(move |tx| tx.add_reading(reading)).await.unwrap();
    }

    /// The forge's checks passed on landing's push, recorded on it as its landing sensor's reading has them recorded,
    /// and the releases run.
    async fn checks_pass(&self, landing: &str) {
        let id = landing.to_owned();
        self.r
            .o
            .store
            .update(move |tx| {
                let mut l = tx.landing(&id)?;
                l.checks = Some(Checks {
                    state: ChecksState::Passed,
                    failed: String::new(),
                });
                tx.put_landing(l)
            })
            .await
            .unwrap();
        self.r.o.shipping.sync_releases().await;
    }

    /// The chain's change approved, landed and its checks passed: what a release starts from.
    pub(crate) async fn landed_and_green(&self) -> Landing {
        let judging = self.land().await;
        self.approve(Gate::Reviewer).await;
        let landed = self
            .until(|l| l.id == judging.id, |l| l.state == LandingState::Landed)
            .await;
        self.checks_pass(&landed.id).await;
        landed
    }

    /// The one release, once done says of it.
    pub(crate) async fn release_until(&self, done: impl Fn(&Release) -> bool) -> Release {
        let rs = self
            .releases_until(|rs| rs.len() == 1 && rs.first().is_some_and(&done))
            .await;
        rs.into_iter().next().expect("one release")
    }
}

// The rails of a release: one bakes at a time, a breach while it bakes runs its way back and reverts what it carried,
// and the revert is released forward, all by code (the rig's launcher starts no session, so no agent could act); a step
// is given the release's variables alone.
#[tokio::test]
async fn a_breached_release_runs_back_reverts_and_releases_forward_with_no_turn() {
    let steps = Steps::exiting(&[]);
    let t = releasing(&steps, baked(), |_| {}).await;
    let landed = t.landed_and_green().await;
    let first = t.release_until(|r| r.state == ReleaseState::Baking).await;
    assert_eq!(Some(first.commit.clone()), landed.commit, "it releases what landed");
    assert_eq!(first.version, "1.0.0");
    t.r.o.shipping.sync_releases().await;
    assert_eq!(
        t.r.o.store.read(|r| r.releases()).await.unwrap().len(),
        1,
        "no release starts while one bakes"
    );

    // a step holds the release's variables and nothing of the server's
    let ran = steps.ran();
    let publish = ran.first().unwrap();
    assert!(
        publish
            .argv
            .first()
            .is_some_and(|a| a.ends_with(".remit/release/10-publish")),
        "{:?}",
        publish.argv
    );
    let names: Vec<&str> = publish
        .env
        .iter()
        .filter_map(|v| v.split_once('='))
        .map(|(k, _)| k)
        .collect();
    assert_eq!(
        names,
        [
            "PATH",
            "HOME",
            "TMPDIR",
            "LANG",
            "REMIT_RELEASE_REPOSITORY",
            "REMIT_RELEASE_COMMIT",
            "REMIT_RELEASE_PREVIOUS",
            "REMIT_RELEASE_OUT",
            // and, where the server traces, the work it runs for (ADR-0119): TRACEPARENT and OTEL_RESOURCE_ATTRIBUTES,
            // ids and no credential, as childenv's test holds them; this rig installs no subscriber, so none here
        ],
        "the server's environment is not a step's"
    );
    let scratch = Path::new(publish.writable.first().unwrap()).parent().unwrap();
    assert!(
        Path::new(&publish.dir).starts_with(scratch),
        "a step works in its own checkout"
    );

    // the guard breaches: Remit takes the release back by itself
    t.health(false).await;
    t.r.o.shipping.sync_releases().await;
    let first = t
        .release_until(|r| {
            matches!(r.state, ReleaseState::TakenBack { .. })
                && r.back.iter().all(|s| s.ended_at.is_some())
                && !r.back.is_empty()
        })
        .await;
    assert!(
        matches!(&first.state, ReleaseState::TakenBack { why, .. } if why.contains(GUARD)),
        "{:?}",
        first.state
    );
    let yank = steps.ran().into_iter().nth(1).expect("the way back ran");
    assert!(
        yank.argv
            .first()
            .is_some_and(|a| a.ends_with(".remit/release/back/10-yank")),
        "{:?}",
        yank.argv
    );
    assert!(
        yank.env
            .iter()
            .any(|v| v.starts_with("REMIT_RELEASE_BREACHED=") && v.contains(GUARD)),
        "the way back is told why"
    );
    let reverted = t
        .until(
            |l| l.id == landed.id,
            |l| matches!(l.state, LandingState::Reverted { .. }),
        )
        .await;
    let LandingState::Reverted { revert, .. } = &reverted.state else {
        unreachable!()
    };
    let revert = t.until(|l| l.id == *revert, |l| l.state == LandingState::Landed).await;
    assert_eq!(
        sh(&t.origin, &["rev-parse", "main^{tree}"]),
        sh(&t.origin, &["rev-parse", &format!("{}^{{tree}}", t.base)]),
        "the base holds the change no more"
    );

    // the revert landed and health came back: it is released forward
    t.health(true).await;
    t.checks_pass(&revert.id).await;
    let forward = t
        .releases_until(|rs| rs.len() == 2 && rs.iter().any(|r| r.state == ReleaseState::Baking))
        .await;
    let forward = forward.iter().find(|r| r.state == ReleaseState::Baking).unwrap();
    assert_eq!(
        forward.commit,
        revert.commit.clone().unwrap(),
        "the release forward carries the revert"
    );
    assert_eq!(
        forward.previous, None,
        "what the customer has is what was before the one taken back"
    );
}

// A change to the release procedure is a rail: it waits for a security verdict as a declared rail does.
#[tokio::test]
async fn a_change_to_the_release_procedure_needs_security() {
    let t = chain_seeded(|_| {}, |_| {}, |wt| step(wt, "10-publish")).await;
    let judging = t.land().await;
    assert!(
        judging.version.gates.contains(&Gate::Security),
        "{:?}",
        judging.version.gates
    );
}

// B1: a release of this server's repository whose steps failed never installed, so taking it back installs nothing
// here: only what it carried is reverted.
#[tokio::test]
async fn a_release_taken_back_before_it_installed_installs_nothing() {
    let steps = Steps::exiting(&[1]);
    let releases = tempfile::tempdir().unwrap();
    let dir = releases.path().to_path_buf();
    let t = releasing(&steps, baked(), move |cfg| this_server(cfg, Some(&dir))).await;
    t.landed_and_green().await;
    let taken = t
        .release_until(|r| matches!(r.state, ReleaseState::TakenBack { .. }))
        .await;
    assert!(
        taken.back.iter().all(|s| !s.name.starts_with("install_release")),
        "nothing was installed, so nothing is installed back: {:?}",
        taken.back
    );
}

// B2: a release of this server's repository with no guard goes live once it is installed, and the next can start.
#[tokio::test]
async fn a_release_of_this_server_with_no_guard_goes_live_once_installed() {
    let steps = Steps::exiting(&[]);
    let releases = tempfile::tempdir().unwrap();
    let dir = releases.path().to_path_buf();
    let t = releasing(&steps, None, move |cfg| this_server(cfg, Some(&dir))).await;
    t.landed_and_green().await;
    let live = t.release_until(|r| r.state == ReleaseState::Live).await;
    assert!(
        live.steps
            .iter()
            .any(|s| s.name == "install_release" && s.exit == Some(0)),
        "{:?}",
        live.steps
    );
}

// A release this server cannot install (it was not started with -update-restart) waits saying so, and is never
// published as baking or live.
#[tokio::test]
async fn a_refused_install_waits_and_never_goes_live() {
    let steps = Steps::exiting(&[]);
    let t = releasing(&steps, baked(), |cfg| this_server(cfg, None)).await;
    t.landed_and_green().await;
    let waiting = t
        .release_until(|r| r.state != ReleaseState::Releasing || !r.waits.is_empty())
        .await;
    assert_eq!(waiting.state, ReleaseState::Releasing, "{waiting:?}");
    assert!(waiting.waits.contains("-update-restart"), "{}", waiting.waits);
}

// A step that cannot reach the outside (exit 75) runs once per sync, and the release keeps the last attempt's steps
// with a count of its tries rather than every attempt's.
#[tokio::test]
async fn a_step_that_cannot_reach_the_outside_runs_once_a_sync_and_keeps_one_attempt() {
    let steps = Steps::exiting(&[75, 75]);
    let t = releasing(&steps, baked(), |_| {}).await;
    t.landed_and_green().await;
    let once = t.release_until(|r| !r.waits.is_empty()).await;
    assert_eq!(steps.ran().len(), 1, "one run in one sync");
    assert_eq!(once.steps.len(), 1, "{:?}", once.steps);
    t.r.o.shipping.sync_releases().await;
    let twice = t
        .release_until(|r| r.rev > once.rev && r.steps.iter().all(|s| s.ended_at.is_some()))
        .await;
    assert_eq!(steps.ran().len(), 2);
    assert_eq!(twice.steps.len(), 1, "{:?}", twice.steps);
}

// A repository that declares no release: what lands is its customer's once its checks stand, so the chain is marked
// live by code and nothing parks it.
#[tokio::test]
async fn a_repository_with_no_procedure_has_its_chains_live_once_their_checks_stand() {
    let t = chain_seeded(
        |_| {},
        |_| {},
        |wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap(),
    )
    .await;
    t.landed_and_green().await;
    let live = t.release_until(|r| r.state == ReleaseState::Live).await;
    assert!(live.steps.is_empty(), "{:?}", live.steps);
    let key = chains::live::live_key(&t.chain.id);
    let reading = t.r.o.store.read(move |r| r.last_reading(&key)).await;
    assert!(reading.is_ok(), "the chain is live: {reading:?}");
}

// A procedure that does not parse holds its release, saying why, and the chain that changed it hears it.
#[tokio::test]
async fn a_procedure_that_does_not_parse_waits_and_tells_its_chain() {
    let steps = Steps::exiting(&[]);
    let t = releasing(&steps, Some(format!("{GUARD} soon value.healthy == false")), |_| {}).await;
    t.landed_and_green().await;
    let held = t.release_until(|r| !r.waits.is_empty()).await;
    assert_eq!(held.state, ReleaseState::Releasing);
    assert!(held.waits.contains("guard"), "{}", held.waits);
    assert!(steps.ran().is_empty(), "nothing ran");
    let id = t.chain.id.clone();
    let told =
        t.r.o
            .store
            .read(move |r| r.decisions_of_kind(&id, &DecisionKind::Release))
            .await
            .unwrap();
    assert!(!told.is_empty(), "the chain is told");
}

/// A server whose health is always healthy.
struct Healthy;

#[async_trait]
impl Instance for Healthy {
    async fn health(&self, _last_tick: Option<model::Time>) -> InstanceHealth {
        InstanceHealth {
            healthy: true,
            ..InstanceHealth::default()
        }
    }
}

// The guard of a release that bakes is watched: fact:remit.health is recorded every round while it bakes, not only
// when it changes.
#[tokio::test]
async fn health_is_recorded_every_round_while_a_release_bakes_under_it() {
    let t = chain_seeded(|_| {}, |cfg| cfg.instance = Some(Arc::new(Healthy)), hello).await;
    let baking = Release {
        id: "rl_baking".to_owned(),
        repo: "remit".to_owned(),
        state: ReleaseState::Baking,
        guards: vec![ReleaseGuard {
            signal: SignalKey(GUARD.to_owned()),
            breach: "value.healthy == false".to_owned(),
            ..ReleaseGuard::default()
        }],
        ..Release::default()
    };
    t.r.o.store.update(move |tx| tx.put_release(baking)).await.unwrap();
    let count = || async {
        let key = SignalKey(GUARD.to_owned());
        t.r.o
            .store
            .read(move |r| r.newest_readings(&key, 10))
            .await
            .unwrap()
            .len()
    };
    for round in 1..=2 {
        t.r.o.shipping.record_remit_facts(NOW);
        let mut n = 0;
        for _ in 0..50 {
            n = count().await;
            if n == round {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(n, round, "round {round} records the health");
    }
}

// A release step reaches a connection only through a grant its repository holds, through grant's one rule, and writes
// to it only through an irreversible one; the credential is the proxy's.
#[tokio::test]
async fn a_release_step_writes_only_through_an_irreversible_grant_of_its_repository() {
    let t = chain_seeded(|_| {}, |_| {}, hello).await;
    let reach = t.r.o.reach.clone();
    let named = [(REGISTRY.to_owned(), "default".to_owned())];
    assert!(reach.release_way("remit", &named).await.is_err(), "no grant: no way");
    for (effect, writes) in [("read", false), ("compensable", false), ("irreversible", true)] {
        let g = Grant {
            holder: GrantHolder {
                kind: HolderKind::Repository,
                id: "remit".to_owned(),
            },
            source: REGISTRY.to_owned(),
            connection: "default".to_owned(),
            effect: effect.to_owned(),
            granted_by: by_owner("owner", "web"),
            granted_at: NOW,
        };
        t.r.o.store.update(move |tx| tx.put_grant(&g)).await.unwrap();
        let way = reach.release_way("remit", &named).await.unwrap();
        let [up] = way.upstreams.as_slice() else {
            panic!("one upstream for one connection");
        };
        assert_eq!(up.writes, writes, "{effect}");
        assert!(
            up.headers.iter().any(|(_, v)| v.contains(REGISTRY_TOKEN)),
            "the credential is the proxy's"
        );
        assert!(
            !way.scrubbed(&format!("echo {REGISTRY_TOKEN}")).contains(REGISTRY_TOKEN),
            "its output is scrubbed"
        );
    }
}

// Two releases stored in the same millisecond are read in the order they were made, the newest first.
#[tokio::test]
async fn releases_made_in_one_instant_are_read_in_the_order_they_were_made() {
    let t = chain_seeded(|_| {}, |_| {}, hello).await;
    for id in ["rl_b", "rl_a"] {
        let r = Release {
            id: id.to_owned(),
            repo: "remit".to_owned(),
            created_at: NOW,
            ..Release::default()
        };
        t.r.o.store.update(move |tx| tx.put_release(r)).await.unwrap();
    }
    let newest_first = t.r.o.store.read(|r| r.repo_releases("remit")).await.unwrap();
    assert_eq!(newest_first.first().map(|r| r.id.as_str()), Some("rl_a"));
    let oldest_first = t.r.o.store.read(|r| r.releases()).await.unwrap();
    assert_eq!(oldest_first.first().map(|r| r.id.as_str()), Some("rl_b"));
}

// Releases are paused for a repository Remit knows: a name it has never seen is refused.
#[tokio::test]
async fn a_pause_of_a_repository_remit_does_not_know_is_refused() {
    let t = chain_seeded(|_| {}, |_| {}, hello).await;
    let pause = |repo: &str| ReleasePauseInput {
        repo: repo.to_owned(),
        why: "hold until Monday".to_owned(),
    };
    assert!(
        t.r.o
            .shipping
            .pause_releases(by_owner("owner", "web"), pause("no-such-repo"))
            .await
            .is_err()
    );
    assert!(
        t.r.o
            .shipping
            .pause_releases(by_owner("owner", "web"), pause("remit"))
            .await
            .is_ok(),
        "the chain's repository is known"
    );
}
