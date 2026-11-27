//! A landing is one version of a chain's change on one repository on its way onto the base branch, and what became of it
//! (ADR-0114): Remit judges it, combines it with the base in the repository's queue, checks the combined tree, pushes it
//! and reverts it when what it landed breaks. A release carries what landed to its customer (ADR-0116). A chain lands
//! and releases without bound, so landings and releases are records of their own.

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, Checks, Duration, Gate, SignalKey, Time};

/// One version of a chain's change to one repository, from its gates to its repository's base and, maybe, back
/// (ADR-0114). An entity of its own, as a chain lands without bound; the queue of a repository and base is its landings
/// that are ready, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Landing {
    pub id: String,
    pub chain_id: String,
    /// The repository as the chain opened it (`RepoWork::repo`).
    pub repo: String,
    /// The queue it waits in: its forge's host, its path there and the branch it lands on. None for a version of a folder
    /// worked in place (ADR-0137): its files are already where they land, so it has no queue, batch or push.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<QueueKey>,
    pub version: LandingVersion,
    /// The Chair's rating of this version, for what the owner reads first; the queue does not order by it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// Why, in one line.
    pub priority_why: String,
    /// A revert: what it takes back, and why. None for a chain's own change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverts: Option<Revert>,
    pub state: LandingState,
    /// The batch it was last combined and checked in; none before one took it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<String>,
    /// The base commit that batch was combined on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onto: Option<String>,
    /// Its commit on the base once it landed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The head of the push that carried it: the forge's pipeline runs on that head.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push: Option<String>,
    /// How the forge's checks on `push`, and on `onto` (the base before it), stand as last read; none while the forge has
    /// said nothing of them, or runs none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<Checks>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_checks: Option<Checks>,
    /// The release that carried it to its customer (ADR-0116); none until one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// Where it stood on a forge before Remit landed changes itself: the merge request's URL of a version carried over
    /// from one (migration 0119); none for a landing Remit made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// When the version was recorded.
    pub created_at: Time,
    /// When its gates all approved it: its place in the queue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready_at: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landed_at: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverted_at: Option<Time>,
    pub updated_at: Time,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

impl Landing {
    /// The forge host of its queue; empty for a version of a folder worked in place, which no queue code reads.
    pub fn host(&self) -> &str {
        self.queue.as_ref().map_or("", |q| q.host.as_str())
    }

    /// Its repository's path on its forge, as `host`.
    pub fn project(&self) -> &str {
        self.queue.as_ref().map_or("", |q| q.project.as_str())
    }

    /// The branch it lands on, as `host`.
    pub fn base(&self) -> &str {
        self.queue.as_ref().map_or("", |q| q.base.as_str())
    }

    /// Whether its change is on the base now: landed and not taken back.
    pub fn on_base(&self) -> bool {
        self.state == LandingState::Landed
    }

    /// The landing back with its chain instead of landing, for reason, with what said so.
    #[must_use]
    pub fn returned(mut self, reason: ReturnReason, output: String, now: Time) -> Self {
        self.state = LandingState::Returned { reason, output };
        self.updated_at = now;
        self
    }

    /// The landing it takes back, when it is a revert.
    pub fn reverted_one(&self) -> Option<&str> {
        self.reverts.as_ref().map(|r| r.landing.as_str())
    }

    /// The landing it is taking back now, when it is a revert still on its way: once it lands, the landing it took back
    /// says so itself (`LandingState::Reverted`).
    pub fn taking_back(&self) -> Option<&str> {
        self.reverted_one().filter(|_| self.state.open())
    }
}

/// What a landing is a version of, fixed when the Chair lands it: the worktree commit its checks passed, the base commit it
/// forked from, the patch its verdicts are of, the gates that patch needs and the verdicts that approved it, the employees
/// whose work it is, the commit message, the paths it touches and the canary stage it lands as.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LandingVersion {
    /// Counts the chain's versions of its change on this repository: 1 for the first.
    pub number: i64,
    /// The worktree commit.
    pub from: String,
    /// The base commit `from`'s change is measured from; empty for a version carried over from a merge request, whose
    /// change is measured from where `from` meets its queue's base (`shipping::queue::fork_point`).
    pub forked: String,
    /// The patch id of `from`'s change against `forked` (`shipping::gate::patch_id`): a verdict is of it.
    pub change: String,
    /// The gates `change` needs (`required_gates`).
    pub gates: Vec<Gate>,
    /// The verdicts that approved `change`, by id, one for each of `gates`; none while it is judged.
    pub verdicts: Vec<String>,
    /// The employees whose accepted work makes it up, by id.
    pub by: Vec<String>,
    /// The landed commit's subject and body, the Chair's.
    pub title: String,
    pub description: String,
    /// The paths the change touches, as checked.
    pub paths: Vec<String>,
    /// The judgments a decider made of the change when it was recorded (ADR-0118, point 4: how risky it is, whether it
    /// alters a rail), by id: what its outcome is recorded on once the landing is live or taken back.
    pub judged: Vec<String>,
    /// The effort its change reviewers run at, when a decider found it risky enough to raise them above their usual: `high`,
    /// `xhigh` or `max`. Only raises: a judge never runs below what it would have.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// The canary stage it lands as; none for a change that lands to everyone the base reaches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<StagePlan>,
}

/// Where a landing stands. A version is judged, ready once every gate it needs approves its patch (an attempt's once its
/// round picks it, ADR-0115), then taken into a batch, combined onto the base and checked; it lands, or goes back to its
/// Chair; a landed one may be reverted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum LandingState {
    /// Its gates have not all approved it yet.
    #[default]
    Judging,
    /// Every gate approved it, and its chain is an attempt of a round (`Chain::attempt`): it waits for the round's pick
    /// (`pick_attempt`), which alone puts it in the queue.
    Contending,
    /// Every gate approved it: it waits in its repository's queue.
    Ready,
    /// A batch is applying it onto the base.
    Combining,
    /// The checks run on the batch it was combined into, or its push is under way.
    Checking,
    /// It is on the base.
    Landed,
    /// It went back to its Chair, for reason, with what said so.
    Returned { reason: ReturnReason, output: String },
    /// It was taken off the base by the landing `revert`, for cause, on the word of `by`, for why.
    Reverted {
        revert: String,
        cause: RevertCause,
        by: Actor,
        why: String,
    },
}

crate::json_tagged!(LandingState, "kind");

impl LandingState {
    /// Whether the landing is still on its way: judged, waiting for its round's pick or in the queue, combined or checked.
    pub const fn open(&self) -> bool {
        matches!(
            self,
            Self::Judging | Self::Contending | Self::Ready | Self::Combining | Self::Checking
        )
    }

    /// Its kind, as the store's `state` column and the owner read it.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Judging => "judging",
            Self::Contending => "contending",
            Self::Ready => "ready",
            Self::Combining => "combining",
            Self::Checking => "checking",
            Self::Landed => "landed",
            Self::Returned { .. } => "returned",
            Self::Reverted { .. } => "reverted",
        }
    }
}

/// What a revert takes back, what took it back, who and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revert {
    /// The landing it takes back.
    pub landing: String,
    pub cause: RevertCause,
    pub by: Actor,
    #[serde(default)]
    pub why: String,
}

text_enum! {
    /// How soon the owner should read a change, as the Chair rates it when it lands it: it orders what he reads, never
    /// the queue.
    pub enum Priority {
        High = "high",
        Medium = "medium",
        Low = "low",
    }
}

text_enum! {
    /// Why a version went back to its Chair instead of landing.
    pub enum ReturnReason {
        /// It conflicts with the base it was combined onto: the queue never resolves a conflict.
        Conflict = "conflict",
        /// The checks failed on it alone, combined onto the base.
        Checks = "checks",
        /// Combined onto the base it changes nothing, and no commit on the base since it forked brought it.
        Empty = "empty",
        /// A newer version of the same repository's change replaced it before it landed.
        Replaced = "replaced",
        /// The forge refused the push, saying why (a protected base Remit's account may not push to).
        Refused = "refused",
        /// Its chain was deleted before it landed: the owner, or the Assistant for him, took the work back.
        Withdrawn = "withdrawn",
        /// It was an attempt of a round whose judge picked another attempt, or whose pick of it the owner overturned
        /// (`pick_attempt`, ADR-0115).
        NotPicked = "not_picked",
    }
}

text_enum! {
    /// What took a landing back.
    pub enum RevertCause {
        /// The forge's pipeline on the push that carried it failed while the base before it passed.
        Pipeline = "pipeline",
        /// A guardrail of its stage breached while it baked, or the release that carried it breached (ADR-0116).
        Guardrail = "guardrail",
        /// The owner, or the Assistant or its Chair on his word, took it back (`revert_landing`, `revert_release`).
        Overturn = "overturn",
        /// A revert of an earlier landing conflicted with it, so it was taken back first.
        Conflict = "conflict",
    }
}

/// What a landing declares when it is a stage of a canary (ADR-0050): who the change reaches, what guards it, how long it
/// bakes and how it is taken back. A stage that is itself the way back (`undo`) declares only what it brings back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StagePlan {
    /// Who the landed change reaches: "0%, dark behind flag checkout_v2", "10% of shops", "100%".
    pub exposure: String,
    /// The signal that says the stage hurts, and the condition on its reading that says so (the breach).
    pub guardrail: SignalKey,
    pub breach: String,
    /// How long it is watched before the next stage on the repository may land.
    pub bake: Duration,
    /// The act that takes this stage back: the flag value to set, or the revert.
    pub rollback: String,
    /// This stage takes an earlier one back: it lands while a stage bakes and while a guardrail is breached.
    pub undo: bool,
}

/// The queue of one repository and base branch: one writer lands there at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QueueKey {
    pub host: String,
    pub project: String,
    pub base: String,
}

impl QueueKey {
    /// As the owner reads it: host/project → base.
    pub fn named(&self) -> String {
        format!("{}/{} → {}", self.host, self.project, self.base)
    }
}

/// One release of a repository (ADR-0116): its procedure run on a landed commit, which carries every landing between
/// the commit released before and this one (each names it, `Landing::release`), then watched while it bakes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Release {
    pub id: String,
    /// The repository's catalog id, as in RepoWork.
    pub repo: String,
    /// Its forge's host and path there, and the branch it releases from.
    pub host: String,
    pub project: String,
    pub base: String,
    /// The commit released.
    pub commit: String,
    /// The landing whose commit it released: the newest it carries.
    pub top: String,
    /// The commit released before it; none for the repository's first release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    pub state: ReleaseState,
    /// The version the procedure reported it published.
    pub version: String,
    /// Where its customer reaches it, as the procedure reported.
    pub links: Vec<String>,
    /// Each step of its procedure's last run, in order, with what it said: a run again (after a step that could not
    /// reach the outside, or an install refused) replaces the one before.
    pub steps: Vec<ReleaseStep>,
    /// How many times its procedure ran.
    pub tries: u32,
    /// Each step of its way back (`.remit/release/back/`) that ran when it was taken back.
    pub back: Vec<ReleaseStep>,
    /// What it bakes under: the repository's guard and the stage plan of each landing it carries.
    pub guards: Vec<ReleaseGuard>,
    /// What holds it before its steps run or pass, in words: the outside was not reachable (a step exited 75), or a
    /// connection it names is not granted or has no credential. Empty while nothing holds it; it is tried again at the
    /// next sync.
    pub waits: String,
    pub created_at: Time,
    /// Until when it bakes: none for a release with nothing to watch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bake_until: Option<Time>,
    /// When it went live, published nothing, or was taken back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Time>,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

impl Release {
    /// Whether it is still on its way: its steps run, or it bakes.
    pub const fn open(&self) -> bool {
        matches!(self.state, ReleaseState::Releasing | ReleaseState::Baking)
    }
}

impl ReleaseState {
    /// Its kind, as `fact:repo.release` and the owner read it.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Releasing => "releasing",
            Self::Baking => "baking",
            Self::Live => "live",
            Self::Unchanged => "unchanged",
            Self::TakenBack { .. } => "taken_back",
        }
    }
}

/// One step of a release's procedure as it ran: its file's name, how it exited (none when it could not be started),
/// what it printed, scrubbed and cut, and when.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReleaseStep {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    pub output: String,
    pub started_at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Time>,
}

/// A guardrail a release bakes under: the signal, the condition on its reading that says the release hurts, and how
/// long it is watched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReleaseGuard {
    pub signal: SignalKey,
    pub breach: String,
    pub bake: Duration,
}

/// Where a release stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum ReleaseState {
    /// Its procedure's steps run, or wait to run again (`Release::waits`).
    #[default]
    Releasing,
    /// It published and is watched until it bakes.
    Baking,
    /// It baked healthy: its landings are live.
    Live,
    /// Its procedure said the commit carries nothing new to publish: its landings wait for the release that does.
    Unchanged,
    /// It was taken back on the word of `by`, for why: a breach (by Remit), or `revert_release`.
    TakenBack { by: Actor, why: String },
}

crate::json_tagged!(ReleaseState, "kind");

/// A repository's releases held (ADR-0116): pulled by the owner, or the Assistant for him, recorded with who and why,
/// and kept once lifted, with who lifted it and why, until the next pull replaces it. Landing goes on; once it is
/// lifted, the next release is at the base's newest landing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReleasePause {
    /// The repository's catalog id, as in RepoWork.
    pub repo: String,
    pub by: Actor,
    pub why: String,
    pub at: Time,
    /// Who lifted it, why and when; none while it holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifted: Option<ReleaseLift>,
}

impl ReleasePause {
    /// Whether it holds the repository's releases now: it is not lifted.
    pub const fn holds(&self) -> bool {
        self.lifted.is_none()
    }
}

/// The lift of a release pause: who, why and when.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReleaseLift {
    pub by: Actor,
    pub why: String,
    pub at: Time,
}
