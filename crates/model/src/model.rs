//! The records of a chain and its agents. A clone shares no memory with the original: a Rust value owns what it holds.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, AttachmentRef, EventStatus, ForeignJson, Loadout, Priority, Serves, Signs, StagePlan, Time};

text_enum! {
    #[derive(Default)]
    pub enum ChainStatus {
        /// Waiting for a slot; the scheduler starts it.
        #[default]
        Queued = "queued",
        Running = "running",
        Paused = "paused",
        /// The Chair holds a wake subscription and comes back when it fires; holds no slot and no session.
        Waiting = "waiting",
        Done = "done",
    }
}

text_enum! {
    /// Who started a chain.
    #[derive(Default)]
    pub enum ChainOrigin {
        #[default]
        Owner = "owner",
        /// Queued by another chain, with propose_chain.
        Proposed = "proposed",
        /// Started by Remit from one of the owner's routines.
        Routine = "routine",
        /// Started by a subscription to an event signal, from the event it names.
        Event = "event",
        /// Started by an outside agent through a client key, which the chain's `by` names.
        Client = "client",
        /// Opened by a judge to check a result.
        Verification = "verification",
        /// Queued by Remit to replay a recorded goal with a change's candidate, which `replay` names (ADR-0069).
        Replay = "replay",
    }
}

/// One case worked by one Chair and the agents it hires.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Chain {
    pub id: String,
    pub title: String,
    pub goal: String,
    /// What code branches on for it (ADR-0113): set once when it is queued, changed by no tool.
    pub kind: ChainKind,
    /// The capabilities it holds: its kind's when it was queued, then what its Chair gave or the Assistant decided.
    pub capabilities: Vec<String>,
    pub origin: ChainOrigin,
    /// The chain that queued it.
    pub proposed_by: String,
    /// The routine that started it.
    pub routine_id: String,
    /// The address of the event that started it (EventRef), when one did.
    pub event_id: String,
    /// The change item whose candidate it replays a recorded goal with (ADR-0069).
    pub replay: String,
    /// Who asked for it; none for chains from before Remit recorded it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<Actor>,
    /// Private, so one path changes it (ADR-0072, section 3): `set_status`, which orch's step alone calls.
    status: ChainStatus,
    pub chair_id: String,
    /// The directory Remit made for it: <work root>/<chain id>.
    pub workdir: String,
    pub summary: String,
    /// Code work, one worktree and branch per repository.
    pub repos: Vec<RepoWork>,
    /// The Chair asked to finish, and the goal judge checks the result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finishing: Option<Finishing>,
    /// Why its last finish had no ruling of the goal judge; unset when the judge ruled on it, or the chain is not
    /// finished. A chain finished before it was recorded reads its outcome from its verdicts alone (ADR-0067).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unjudged: Option<Unjudged>,
    /// The key result it works toward (ADR-0031).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serves: Option<Serves>,
    /// The round (`Round::id`) it is an attempt of, competing with the round's other attempts at the key result it
    /// serves (ADR-0115); none for a chain that competes with none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<String>,
    /// Put away by the owner: out of the lists until it reopens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<Time>,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts the chain's stored changes: each write is made over the revision it read, so none is lost.
    pub rev: i64,
}

impl Chain {
    pub fn status(&self) -> &ChainStatus {
        &self.status
    }

    /// Moves the chain to status: the one way a chain's status changes.
    pub fn set_status(&mut self, status: ChainStatus) {
        self.status = status;
    }
}

text_enum! {
    /// What a chain is to code (ADR-0113): its admission, its gates, its Chair's fixed brief and who may start it. Set
    /// once when the chain is queued; a playbook is reference and decides none of it.
    #[derive(Default)]
    pub enum ChainKind {
        /// An ordinary request.
        #[default]
        Work = "work",
        /// Work small enough that running it is the whole judgement: its changes pass QC alone (ADR-0110).
        Chore = "chore",
        /// Shapes, reviews or checks in on the owner's objectives.
        Objective = "objective",
        /// Writes playbooks, agents and skills from the owner's words.
        Author = "author",
        /// Coaches how the employees learn.
        Coach = "coach",
        /// Reads the outcome figures and tells the owner what changed.
        Analyst = "analyst",
        /// Writes every agent's memory from what people and judges taught it (ADR-0117).
        Librarian = "librarian",
        /// A seeded-fault trial of the judges (ADR-0102).
        Trial = "trial",
    }
}

impl ChainKind {
    /// The gates a chain of this kind passes its changes through: QC alone for a chore, review and QC for every other.
    pub const fn gates(self) -> ChangeGates {
        match self {
            Self::Chore => ChangeGates::Qc,
            Self::Work
            | Self::Objective
            | Self::Author
            | Self::Coach
            | Self::Analyst
            | Self::Librarian
            | Self::Trial => ChangeGates::ReviewAndQc,
        }
    }

    /// Whether the owner waits on its result, so it takes no slot (ADR-0001).
    pub const fn owner_waits(self) -> bool {
        match self {
            Self::Objective | Self::Author => true,
            Self::Work | Self::Chore | Self::Coach | Self::Analyst | Self::Librarian | Self::Trial => false,
        }
    }
}

text_enum! {
    /// Why a chain finished with no ruling of the goal judge on that finish: each reads Not done, saying so (ADR-0067).
    pub enum Unjudged {
        /// The goal check could not run: its judge failed, stopped or ran out of time.
        Failed = "failed",
        /// The judge refused as many finishes as it may; the next one ended the chain.
        Limit = "limit",
        /// The owner ended it, himself or through an agent on his word.
        Owner = "owner",
        /// Remit ended it: no progress after repeated nudges.
        Stalled = "stalled",
        /// Its Chair's word ended it: a verification chain, a trial of the judges the goal judge ruled on once, or goal
        /// checks are off.
        NoCheck = "nocheck",
    }
}

/// A chain's worktree of one repository, on a branch only that chain uses. What it lands are landings of their own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RepoWork {
    /// Catalog id.
    pub repo: String,
    /// Its path on its forge: GitLab's group/project, GitHub's owner/name.
    pub project: String,
    /// The forge's host.
    pub host: String,
    /// The worktree.
    pub path: String,
    pub branch: String,
    /// The branch its changes land on.
    pub base: String,
    /// The submit in progress: one at a time per repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submit: Option<Submit>,
}

/// A submit of a repository's work in progress (`land`): its own checks run on the worktree's head, minutes long, and
/// once they pass it becomes a landing. It is recorded before the checks start, so a restart runs them again and the
/// Chair hears the result once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Submit {
    /// The landed commit's subject and body, the Chair's.
    pub title: String,
    pub description: String,
    /// The Chair's rating of this version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    pub priority_why: String,
    /// The canary stage the version lands as, when it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<StagePlan>,
    pub started_at: Time,
}

/// What a deleted chain leaves on disk to remove: its worktrees, each only while it is on the chain's own branch, and the
/// chain's directory when Remit made it. It is stored with the chain's deletion, so a crash before the files are gone
/// leaves it to the next start, which finishes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Cleanup {
    pub chain_id: String,
    pub repos: Vec<RepoWork>,
    pub dir: String,
}

/// The Chair's request to finish while the goal judge checks the result. The summary waits here for the verdict, so a
/// restart loses nothing. `judge` is the card that checks it; `at` is when its time began (the check began, or the chain
/// resumed from a pause). Once the judge has given its verdict, `approval_id` names that approval: the chain acts on it
/// when the judge's turn has ended, because the judge's usage report arrives with the end of its turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Finishing {
    pub summary: String,
    pub judge: String,
    pub at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    /// A finished chain judged again after its change landed (ADR-0039): it ends with a verdict, no finish.
    pub recheck: bool,
}

/// A judge's verdict: a reviewer's or QC's on one version of a repository's change (identified by its patch id), or the
/// goal judge's on the chain's result as a whole (no repository, no change), with a finding per key result of the goal.
/// A chain's verdicts are records of their own: they grow with every round of review. Who signed one and what it rules
/// have no default, as every approval names both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub chain_id: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub change: String,
    #[serde(default)]
    pub card_id: String,
    #[serde(default)]
    pub name: String,
    pub role: VerdictRole,
    pub verdict: Verdict,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub at: Time,
}

impl Approval {
    /// Whether this verdict shows the result holds: it approves and every item it checked is met. The owner reads it as
    /// Done on a finished chain, and the outcomes count it as a confirmed result (ADR-0016).
    pub fn confirms(&self) -> bool {
        self.verdict == Verdict::Approve && self.findings.iter().all(|f| f.status == FindingStatus::Met)
    }
}

text_enum! {
    /// What a judge rules on what it checked.
    pub enum Verdict {
        Approve = "approve",
        RequestChanges = "request_changes",
    }
}

text_enum! {
    /// Who signs a verdict: a change's gate (`Gate`), or the judge of the goal and of the check it was declared with.
    pub enum VerdictRole {
        Reviewer = "reviewer",
        Qc = "qc",
        Security = "security",
        Judge = "judge",
    }
}

impl VerdictRole {
    /// The change gate a verdict of this role closes; none for a judge's.
    pub const fn gate(self) -> Option<Gate> {
        match self {
            Self::Reviewer => Some(Gate::Reviewer),
            Self::Qc => Some(Gate::Qc),
            Self::Security => Some(Gate::Security),
            Self::Judge => None,
        }
    }
}

text_enum! {
    /// A gate a change passes before it lands: review, QC, and the security review of a change that touches a rail
    /// (ADR-0087). Its text is that of the role that signs it.
    pub enum Gate {
        Reviewer = "reviewer",
        Qc = "qc",
        Security = "security",
    }
}

text_enum! {
    /// The gates a chain's changes pass before they publish, a change that touches a rail adding security (ADR-0110).
    #[derive(Default)]
    pub enum ChangeGates {
        #[default]
        ReviewAndQc = "review-and-qc",
        /// QC alone: work small enough that running it is the whole judgement.
        Qc = "qc",
    }
}

impl ChangeGates {
    /// The gates, in the order they are asked.
    pub const fn gates(self) -> &'static [Gate] {
        match self {
            Self::ReviewAndQc => &[Gate::Reviewer, Gate::Qc],
            Self::Qc => &[Gate::Qc],
        }
    }
}

impl Gate {
    /// The role whose verdict closes this gate.
    pub const fn role(self) -> VerdictRole {
        match self {
            Self::Reviewer => VerdictRole::Reviewer,
            Self::Qc => VerdictRole::Qc,
            Self::Security => VerdictRole::Security,
        }
    }
}

text_enum! {
    /// A Chair's ruling on a worker's report: accepted, or sent back to rework.
    pub enum ReportVerdict {
        Accept = "accept",
        Rework = "rework",
    }
}

/// One item a judge checked of a key result, what it found, and the proof: file:line, command output, or why it could not
/// be checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Finding {
    /// The id of the key result this finding rolls up to: the judge's brief names it, and the tool schema requires it (a
    /// reviewer or QC, whose brief names none, sends it empty).
    pub key_result: String,
    /// The sub-point of that key result the judge checked, or the defect class it asks changes for.
    pub item: String,
    pub status: FindingStatus,
    pub evidence: String,
    /// What an unverified key result waits on; an unverified finding names one, the others none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocker: Option<FindingBlocker>,
    /// Why a reviewer objected, from the closed set; unset when the finding objects to nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<ReasonCode>,
    /// Platform records that corroborate `evidence`; a finding coded evidence_contradicts carries at least one.
    pub refs: Vec<EvidenceRef>,
}

text_enum! {
    /// Why a reviewer objected to a key result or to its evidence: a closed set, so the owner counts objections by kind
    /// and no agent invents a category.
    pub enum ReasonCode {
        /// The outcome is not covered by the key results.
        UncoveredKeyResult = "uncovered_key_result",
        /// No check can show it holds.
        UnverifiableKeyResult = "unverifiable_key_result",
        /// A surrogate or signal the platform does not know.
        UnknownMetric = "unknown_metric",
        /// It states what another key result already states.
        Overlap = "overlap",
        /// Meeting it would breach a stated guardrail.
        GuardrailBreach = "guardrail_breach",
        /// A platform record contradicts the evidence given.
        EvidenceContradicts = "evidence_contradicts",
        /// The way of working forbids it.
        ProcessConstraint = "process_constraint",
        /// The same text as another key result.
        Duplicate = "duplicate",
        /// No tool call of the team stands behind its words: a "could not" its own tools were never tried on, or a result
        /// no call it made produced.
        Untried = "untried",
        /// None of the above, said in the evidence.
        Other = "other",
    }
}

/// Points a finding at a record the platform wrote: a source call, a signal reading or an event, never text an agent
/// wrote. A reference names its kind: one without reads as none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub kind: EvidenceRefKind,
    #[serde(default)]
    pub id: String,
}

impl EvidenceRef {
    /// Whether the reference names the id of a platform record.
    pub fn valid(&self) -> bool {
        !self.id.trim().is_empty()
    }
}

text_enum! {
    /// The kind of platform record an EvidenceRef points at.
    pub enum EvidenceRefKind {
        SourceCall = "source_call",
        Reading = "reading",
        Event = "event",
    }
}

text_enum! {
    /// Why a judge could not check an item: a closed set, so "the team lacks access" is a claim the chain's capabilities
    /// can refute, never free text.
    pub enum FindingBlocker {
        /// It needs a write to a shared store, a landing or a migration.
        Write = "write",
        /// It shows only once deployed.
        Deploy = "deploy",
        /// A product or owner decision.
        Person = "person",
        /// Data or a system the team cannot reach.
        Access = "access",
        /// A watch on a signal rules on it, and its window is open or ended with no reading that met it (ADR-0013).
        Watch = "watch",
    }
}

text_enum! {
    /// What a judge found of an item.
    #[derive(Default)]
    pub enum FindingStatus {
        Met = "met",
        NotMet = "not_met",
        /// The team cannot check it, e.g. it needs production access.
        #[default]
        Unverified = "unverified",
    }
}

text_enum! {
    #[derive(Default)]
    pub enum CardStatus {
        #[default]
        Starting = "starting",
        Running = "running",
        /// Turn ended; can be prompted again.
        Idle = "idle",
        /// Waiting on the owner.
        Permission = "permission",
        Stopped = "stopped",
        Failed = "failed",
        /// A card no provider can run now (each it could run on is out of capacity, signed out or down): its session is
        /// closed and its message kept, and Remit opens it again once a provider can.
        Waiting = "waiting",
    }
}

text_enum! {
    /// Why a card exists: the Chair itself, or hired while the chain ran.
    #[derive(Default)]
    pub enum Origin {
        Chair = "chair",
        #[default]
        Hired = "hired",
        /// A judge, which Remit starts, not the Chair, outside the Chair's team; `Card::judge` names which.
        Judge = "judge",
        /// The Assistant's card, the owner's agent of the whole app, in no chain: he talks to it from any screen and it
        /// acts on his words.
        Assistant = "assistant",
        /// A subagent Claude Code spawned inside another card's process (ADR-0073): a card of the employee its type
        /// names, bound to that process by `Card::spawn`.
        Spawned = "spawned",
    }
}

text_enum! {
    /// Which judge a card is, set where Remit starts it: the goal judge checks a finish against the goal and the check
    /// it was declared with; a duel's judge picks the better of two reports; a second judge re-decides an item the
    /// Assistant answered, blind to its answer (ADR-0103); a round's judge picks one attempt of a round to land (ADR-0115).
    pub enum JudgeKind {
        Goal = "goal",
        Duel = "duel",
        Second = "second",
        Round = "round",
    }
}

/// What a card is to Remit, derived from the card (`CardRole::of`): every branch on who a card is matches this, so a
/// new role or judge is a compile error at each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRole {
    Chair,
    Assistant,
    /// One of the Chair's team.
    Worker,
    /// A subagent Claude Code spawned inside another card's process (ADR-0073).
    Spawned,
    Judge(JudgeKind),
}

impl CardRole {
    /// The role of card c. A judge is named by its kind: every judge Remit starts is made by `Card::judge_of`, which
    /// sets it with the origin, and migration 0114 gave each judge an earlier build started its kind. Origin judge with
    /// no kind names no judge, so it reads as the team's.
    pub fn of(c: &Card) -> Self {
        if let Some(kind) = c.judge {
            return Self::Judge(kind);
        }
        match c.origin {
            Origin::Chair => Self::Chair,
            Origin::Assistant => Self::Assistant,
            Origin::Spawned => Self::Spawned,
            Origin::Hired | Origin::Judge => Self::Worker,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

/// The owner's agent of the whole app: one conversation he holds with it from every screen, in no chain. It reads what
/// Remit holds and acts on his words through the orchestrator's own actions. Its one card is the agent it runs, and that
/// card's stream is the conversation; a new session of it starts with a recap of the conversation so far. Its id is the
/// home of its card and of what is recorded about it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Assistant {
    pub id: String,
    pub card_id: String,
    pub created_at: Time,
}

/// One agent: one Claude Code session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    /// Where it works: its chain's id, or the Assistant's. What is recorded about it (its events, session init, the
    /// owner's drafts and inbox items for it) carries the same home, and its changes go through that home's writer.
    pub home: String,
    /// The id of the Agent it runs: a plain agent when none has that id.
    pub agent: String,
    /// The id of the Employee at work in it; empty on the cards Remit starts for itself.
    pub employee: String,
    /// The gate its verdict closes, fixed at hire from its agent.
    pub signs: Signs,
    /// What its sessions hold, copied from its agent at hire; none on the cards Remit starts itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loadout: Option<Loadout>,
    pub name: String,
    pub origin: Origin,
    /// Which judge it is, on a card Remit started as one (origin judge); none on every other card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge: Option<JudgeKind>,
    pub why: String,
    pub brief: String,
    /// What the agent hands over.
    pub deliverable: String,
    /// How the Chair accepts it.
    pub criteria: Vec<String>,
    /// Its last report waits for the Chair's acceptance.
    pub awaiting_review: bool,
    /// The id of the last report it sent its Chair, which the watchdog's answers are tied to (ADR-0118): the verdict on
    /// the card proves those of this report and no other. None before its first report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    pub model: String,
    /// How long its model thinks before it acts (`provider::Effort`); empty runs at Claude Code's default.
    pub effort: String,
    /// The id of the Account its sessions run on; empty until its first session starts.
    pub account: String,
    pub mode: String,
    pub cwd: String,
    /// Private, so one path changes it: `set_status`, which keeps `ended_at` with it.
    status: CardStatus,
    /// When its work last ended (it went idle, stopped or failed); none while it works, and on a card that ended before
    /// this was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<Time>,
    /// Whom its permission prompt waits on, while it is in status permission; none in every other status. Private, so
    /// it changes with the status: `ask_permission` sets both, `set_status` to any other status clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_wait: Option<PermissionWait>,
    pub session_id: String,
    pub activity: String,
    pub plan: Vec<PlanEntry>,
    pub ctx_used: i64,
    pub ctx_size: i64,
    pub turns: i64,
    /// Messages waiting for its next turn: its queue's count, filled in by the store, never stored.
    pub queued: i64,
    /// Final message of the last finished turn.
    pub output: String,
    pub error: String,
    /// Its work is being handed to another employee (ADR-0059); none otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handover: Option<Handover>,
    /// The process it runs inside, when Claude Code spawned it there; none otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spawn: Option<Spawn>,
    /// Its model was chosen by code, not by its Chair or its agent (ADR-0118); none otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chosen: Option<Chosen>,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

/// A card whose model code chose, when neither its Chair nor its agent named one (ADR-0118): it climbs a tier when its
/// turn fails or its gate sends its work back twice.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Chosen {
    /// The judgment whose outcome the card's work records; none when the climb chose it: the judgment was scored on the
    /// card the climb replaced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judgment: Option<String>,
}

impl CardStatus {
    /// Whether a card in this status is at its work: starting, in a turn, asking a permission, or waiting for a provider
    /// to open it again. A status this build cannot read is not.
    pub fn works(&self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Permission | Self::Waiting)
    }
}

impl Card {
    /// A card of a judge of kind: Remit starts it outside the Chair's team, and its kind decides what it does.
    pub fn judge_of(kind: JudgeKind) -> Self {
        Self {
            origin: Origin::Judge,
            judge: Some(kind),
            ..Self::default()
        }
    }

    pub fn status(&self) -> &CardStatus {
        &self.status
    }

    /// When its work last ended; none while it works.
    pub fn ended_at(&self) -> Option<Time> {
        self.ended_at
    }

    /// Whom its permission prompt waits on; none while it waits on none.
    pub fn permission_wait(&self) -> Option<&PermissionWait> {
        self.permission_wait.as_ref()
    }

    /// Moves the card to status permission at now, its prompt waiting on wait: the one way a card comes to wait on one.
    pub fn ask_permission(&mut self, wait: PermissionWait, now: Time) {
        self.set_status(CardStatus::Permission, now);
        self.permission_wait = Some(wait);
    }

    /// Moves the card to status at now: the one way a card's status changes. Going to work clears when its work ended;
    /// going idle, stopped or failed from work records now as that time, and an end after an end (an idle card stopped)
    /// keeps the first. A status this build cannot read leaves it as it was.
    pub fn set_status(&mut self, status: CardStatus, now: Time) {
        if status.works() {
            self.ended_at = None;
        } else if matches!(status, CardStatus::Idle | CardStatus::Stopped | CardStatus::Failed)
            && self.ended_at.is_none()
        {
            self.ended_at = Some(now);
        }
        if status != CardStatus::Permission {
            self.permission_wait = None;
        }
        self.status = status;
    }

    /// Puts back a status and the end time it was held with, read from the card before a change: how a closed card that
    /// reached an end state stays there when an edit still unwinding would revive it (orch's commit).
    pub fn restore_status(&mut self, status: CardStatus, ended_at: Option<Time>) {
        if status != CardStatus::Permission {
            self.permission_wait = None;
        }
        self.status = status;
        self.ended_at = ended_at;
    }

    /// Whether the card is one of the Chair's agents, the ones it hired. The Chair itself and the cards Remit starts (the
    /// judges) are not: the Chair does not name them, route to them, read them or stop them.
    pub fn on_team(&self) -> bool {
        self.origin == Origin::Hired
    }
}

/// An action a channel asked for on someone's behalf, kept with its first answer: the same action asked again (by its
/// id) is answered the same, and done once. The store keeps it in columns of its own, not as a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    pub id: String,
    /// The action asked for: the channel's call.
    pub method: String,
    pub by: Actor,
    pub done: bool,
    /// What it returned, when it was done without error.
    pub answer: Option<ForeignJson>,
    /// Why it failed.
    pub error: String,
    /// The kind of its failure as the contract spells it (`NOT_FOUND`, `ENDED`, `UNAVAILABLE`, `CONFLICT`, `FAILED`), so a retry is
    /// answered with the code the first asking had; empty for one done without error, or recorded before kinds were.
    pub error_kind: String,
    /// What a channel acts on beyond the kind of its failure (`FailureDetail`), so a retry is answered with it too.
    pub error_detail: Option<ForeignJson>,
    pub at: Time,
}

/// What a channel acts on beyond the kind of a failure (`link::Failure::detail`), kept with the request it answered so a
/// retry is answered with it too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum FailureDetail {
    /// A draft's save made on a copy another page has saved over since: the draft as it is now, which the page goes on
    /// from (`CONFLICT`).
    DraftConflict { current: Draft },
}

crate::json_tagged!(FailureDetail, "kind");

/// Whom a card's permission prompt waits on (ADR-0059): its chain's Chair, the owner's inbox (where the Assistant decides
/// it for him, or he does when the Assistant is the asker), or, once the time limit passed, the Chair `who` (its card id)
/// as the driver, whose decision it is alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum PermissionWait {
    Chair,
    Owner,
    Driver { who: String },
}

crate::json_tagged!(PermissionWait, "on");

/// What binds a spawned card to the process of the card that spawned it (ADR-0073): it holds no session of its own, and
/// ends when that process does.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Spawn {
    /// The card whose session spawned it.
    pub parent: String,
    /// Claude Code's task id of the subagent, its agent id: what `stop_task` names.
    pub task: String,
    /// The parent's tool call that spawned it, which links to it on the parent card.
    pub tool_call: String,
}

/// A card's work on its way to another employee (ADR-0059): who takes it (a standing employee, or an agent hired anew for
/// it), why, who moved it, and until when the leaving session may write its note. Set from the Chair's hand_over until
/// the new card is made, so a restart in between finishes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Handover {
    pub employee: String,
    pub agent: String,
    pub why: String,
    pub by: Actor,
    pub until: Time,
    /// The climb (ADR-0118) the move is; none: a Chair's hand-over, whose card is hired as any other.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub climb: Option<Climb>,
}

/// Where a card is rehired when it climbs: the model one tier up, at the effort it had, and what raised it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Climb {
    pub model: String,
    pub cause: ClimbCause,
}

/// What raised a card, which decides what the climb proves of the model it chose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClimbCause {
    /// Its turn failed: a broken turn says little of the model, so the choice is scored unknown.
    #[default]
    Failed,
    /// Its gate asked for changes to its work twice: the model was too weak for the work, so the choice is scored wrong.
    SentBack,
}

/// Keeps one edge per pair of cards, whichever way it points: an edge shows that two agents work together (one builds
/// on, reviews or answers the other), not each message between them.
pub fn relationships(edges: &[Edge]) -> Vec<&Edge> {
    let mut seen = HashSet::new();
    edges
        .iter()
        .filter(|e| {
            let pair = if e.from <= e.to {
                (&e.from, &e.to)
            } else {
                (&e.to, &e.from)
            };
            seen.insert(pair)
        })
        .collect()
}

text_enum! {
    #[derive(Default)]
    pub enum EdgeKind {
        #[default]
        Handoff = "handoff",
        Hire = "hire",
        Feedback = "feedback",
        Question = "question",
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Edge {
    pub id: String,
    pub chain_id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub label: String,
    pub created_at: Time,
}

text_enum! {
    #[derive(Default)]
    pub enum EventKind {
        /// agent_message_chunk, joined.
        #[default]
        Message = "message",
        /// agent_thought_chunk, joined.
        Thought = "thought",
        /// tool_call, updated by tool_call_update.
        Tool = "tool",
        /// What was sent to the agent, and by whom.
        Prompt = "prompt",
        /// A turn ended.
        Stop = "stop",
        /// Lifecycle notes and errors.
        System = "system",
        /// Work the server runs for a card between its turns (a submit): running, then completed or failed.
        Job = "job",
        /// Work the agent's own process runs in the background (a Monitor, a background shell), outliving the turn that
        /// started it: running, then completed, failed or stopped.
        Task = "task",
    }
}

/// One entry in a card's stream. Its kind is its status's (`Event::kind`), so the two cannot disagree. A readable
/// format writes them as two texts, `kind` and `status` (`json_paired!`); postcard writes the status as its variant.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", default, rename_all = "camelCase")]
pub struct Event {
    pub id: i64,
    pub home: String,
    pub card_id: String,
    pub status: EventStatus,
    pub tool_call_id: String,
    pub tool_kind: String,
    /// Claude Code's name for the tool a tool call used (Bash, Skill, mcp__<server>__<tool>); empty on a call recorded
    /// before names were, and on the events that are no tool call.
    pub tool_name: String,
    pub title: String,
    pub text: String,
    /// Who sent a prompt, or caused a system note; none for what the agent streamed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<Actor>,
    /// Orders the prompts queued for a card: the higher is read first, then the older. Not shown, and kept in a column.
    #[serde(skip)]
    pub priority: i64,
    /// What the event's kind keeps beside its text, as the code that records it writes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ForeignJson>,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

crate::json_paired!(Event, status, "kind", "status");

impl Event {
    /// What kind of entry it is.
    pub const fn kind(&self) -> EventKind {
        self.status.kind()
    }
}

/// The priority of a session's opening message, the highest: a session reads it first, and it is not one of the messages
/// a card shows as waiting for it (`Card::queued`), since nobody sent it to a session already open.
pub const PRIORITY_OPENING: i64 = 3;

/// The tool calls of one card that share a tool, counted: what the Outcomes page is computed from, so the events
/// themselves are never loaded for it. `name` and `title` are as the call's event records them; `title` is kept only where
/// it says more than the name (a Skill's, and any call recorded before names were).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolTally {
    pub name: String,
    pub title: String,
    pub card_id: String,
    pub calls: i64,
    pub failed: i64,
}

text_enum! {
    /// What kind of thing happened in a chain.
    #[derive(Default)]
    pub enum DecisionKind {
        /// The chain started.
        #[default]
        Seed = "seed",
        /// An agent was hired.
        Hire = "hire",
        /// Work was passed from one agent to another.
        Route = "route",
        /// An agent's report was accepted.
        Accept = "accept",
        /// An agent was sent back to rework its report.
        Rework = "rework",
        /// The owner was asked a question.
        Ask = "ask",
        /// The Chair took back its question: what blocked it is cleared.
        Withdraw = "withdraw",
        /// An agent was stopped.
        Stop = "stop",
        /// An agent's session was started again.
        Restart = "restart",
        /// The chain was paused.
        Pause = "pause",
        /// The chain was resumed.
        Resume = "resume",
        /// A wait began: the Chair subscribed to a time or a watch, or waits on people's replies alone; the kind keeps
        /// its wire and history name.
        Subscribe = "check_back",
        /// The Chair came back, as planned, early, or woken by a message.
        Wake = "wake",
        /// The goal judge checked a finish against the goal.
        Goal = "goal",
        /// The goal check could not run: the chain finished without it.
        GoalFailed = "goal_failed",
        /// The chain finished because nothing moved after repeated nudges.
        Stall = "stall",
        /// The chain finished.
        Finish = "finish",
        /// A finished chain was reopened.
        Reopen = "reopen",
        /// A finished chain was put away.
        Archive = "archive",
        /// An archived chain came back into the lists.
        Unarchive = "unarchive",
        /// A repository was opened for the chain's work.
        Repo = "repo",
        /// A reviewer or QC judged a change.
        Verdict = "verdict",
        /// A submit ended, or a landing moved: ready, landed, returned or reverted.
        Landing = "landing",
        /// The change went live: the owner said so, or a deploy's event told the door.
        Live = "live",
        /// A playbook was written.
        Playbook = "playbook",
        /// An agent definition was written.
        Agent = "agent",
        /// A skill was imported, written or removed.
        Library = "library",
        /// A chain queued another chain.
        Propose = "propose",
        /// The chain was given a capability its kind did not start with.
        Capability = "capability",
        /// The chain was titled.
        Title = "title",
        /// The analyst's Chair told the owner something about how the agents do.
        Insight = "insight",
        /// The Chair allowed or refused an agent's request to do something risky.
        Permission = "permission",
        /// Something told the chain's Chair: an event a subscription tells, a key result retired.
        Event = "event",
        /// An outside agent holding a client key started the chain, or told or cancelled one of its agents.
        Client = "client",
        /// A person accepted or declined the task their card offered.
        Offer = "offer",
        /// The Chair handed a card's work to another employee on a new card.
        Handover = "handover",
        /// A person decided something outside Remit (with a customer, on a call), told in their hand-in.
        Outside = "outside",
        /// The Chair started a workflow run (ADR-0073); its hires are decisions of their own.
        Workflow = "workflow",
        /// A time passed: a mentor did not answer the Chair's call for help by its deadline.
        Deadline = "deadline",
        /// The Chair brought a mentor onto a card, or the mentor accepted or declined.
        Help = "help",
        /// The Chair ended a person's work: an offer withdrawn, or accepted work given a window to hand in.
        Cancel = "cancel",
        /// A person disputed a send-back or a verdict, or the owner ruled on it.
        Dispute = "dispute",
        /// The Assistant decided the owner's inbox item `item` for him, or left it to him.
        Assist = "assist",
        /// The owner overturned decision `overturns`: the Assistant's, a duel judge's pick, or a round judge's.
        Overturn = "overturn",
        /// A second judge, re-deciding inbox item `item` blind to the Assistant's answer, decided otherwise (ADR-0103).
        Dissent = "dissent",
        /// A judge picked one of two cards of different lineages that worked the same brief.
        Duel = "duel",
        /// A round's judge picked the attempt whose version lands of a round of competing attempts (`pick_attempt`).
        Pick = "pick",
        /// The coach changed how employees learn: shared a lesson with a lineage, asked for a contest, set the duels'
        /// share.
        Learning = "learning",
        /// A release carried one of the chain's landings to its customer, went live, or was taken back (ADR-0116).
        Release = "release",
        /// The chain's project was set: by the owner, the Assistant or the Chair (ADR-0108).
        Project = "project",
    }
}

/// Something that happened in a chain and why.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Decision {
    pub id: String,
    pub chain_id: String,
    pub kind: DecisionKind,
    /// Who did it; none for decisions from before Remit recorded it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<Actor>,
    pub text: String,
    pub why: String,
    pub card_id: String,
    /// The inbox item an assist decided or left to the owner.
    pub item: String,
    /// The id of the assist or the duel an overturn overturned.
    pub overturns: String,
    /// An assist's answer: the option of its item it took, by id; empty when it answered in words or left the item. An
    /// overturn's: the option of the answer it overturned. What the deciders' picks are read against (ADR-0118, point 5).
    /// `stance` stays beside it: it is that option's meaning (approve or decline) for the kinds whose option ids say
    /// nothing of it, kept so the feed and the coach read it without reading the item.
    pub option: String,
    /// An assist's answer: whether it let through what its item asks or turned it down (ADR-0103). Neither for an answer
    /// in words (a question's) and for every other kind of decision.
    pub stance: Stance,
    pub created_at: Time,
}

text_enum! {
    /// Whether an answer lets through what an item asks (a permission allowed, a change applied, an exception accepted, a
    /// failure or a held firing retried) or turns it down; neither when it answers in words.
    #[derive(Default)]
    pub enum Stance {
        #[default]
        Neither = "",
        Approve = "approve",
        Decline = "decline",
    }
}

/// An agent a playbook suggests for its kind of work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Seed {
    pub agent: String,
    pub name: String,
    pub brief: String,
    pub model: String,
    pub effort: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Playbook {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The kind of work it is for (parse_group): the column it stands in on the Playbooks page. A built-in playbook's is
    /// code.
    pub group: String,
    /// The playbook's text: how this kind of work has gone well, read by whoever chooses to (ADR-0113).
    pub brief: String,
    pub seeds: Vec<Seed>,
    /// Shipped with Remit: can be reset to its original, not deleted.
    pub builtin: bool,
    /// Written since it shipped, on the owner's word: a restart keeps it instead of rewriting it.
    pub edited: bool,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

/// A message the owner or a mentor has written to an agent and not sent, kept as he types it, so that a reload, a closed tab or a second
/// one neither loses nor splits them. Its channel makes its id, so saving it again is saving the same draft. What it is
/// about is fixed when it is first saved; a save changes its words. Sending it ends it in the transaction of the send, and
/// so does the owner deleting it: a save that arrives after is refused, and the draft does not come back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Draft {
    pub id: String,
    pub home: String,
    pub text: String,
    /// A message's attachments, in the order they show (their bytes are stored apart).
    pub attachments: Vec<AttachmentRef>,
    /// The card it is to.
    pub card_id: String,
    /// Who writes it: a mentor's id; empty for the owner's. A writer reads, saves and sends only their own (ADR-0089).
    pub mentor: String,
    pub updated_at: Time,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

/// One chain as the store holds it: its records, which orch lays out as the chain's view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredChain {
    pub chain: Chain,
    pub cards: Vec<Card>,
    pub edges: Vec<Edge>,
    pub decisions: Vec<Decision>,
    /// The owner's, not sent, oldest first.
    pub drafts: Vec<Draft>,
    /// Its runs, each with its steps, oldest first.
    pub workflows: Vec<crate::Workflow>,
    /// What the coach wrote after each of its finishes, oldest first.
    pub retrospectives: Vec<crate::Retrospective>,
}

/// What Claude Code says a session loaded, from the init message it sends at the start of every turn, as it says it.
/// Remit does not choose most of it: every agent loads the server user's own Claude Code settings, skills, plugins and
/// MCP servers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionInit {
    pub session_id: String,
    /// Claude Code's.
    pub version: String,
    pub model: String,
    pub permission_mode: String,
    pub output_style: String,
    pub tools: Vec<String>,
    #[serde(rename = "mcpServers")]
    pub mcp_servers: Vec<MCPServerState>,
    pub skills: Vec<String>,
    pub plugins: Vec<Plugin>,
    pub plugin_errors: Vec<PluginError>,
    pub agents: Vec<String>,
    pub slash_commands: Vec<String>,
}

/// A card's session's newest [`SessionInit`], which replaces the one before, and when Remit heard it. The store keeps it
/// by card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InitReport {
    pub init: SessionInit,
    pub at: Time,
}

/// What fills a session's context, as Claude Code counts it for `/context`. `parts` add up to `total` but for those not
/// sent (`ContextPart::kind`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionContext {
    pub session_id: String,
    pub model: String,
    /// Tokens in the context, the window's size and where the conversation is compacted.
    pub total: u64,
    pub size: u64,
    pub compact_at: u64,
    pub parts: Vec<ContextPart>,
    /// The instruction files read (CLAUDE.md and its kin), `source` saying whose.
    pub memory_files: Vec<ContextItem>,
    pub skills: Vec<ContextItem>,
    pub agents: Vec<ContextItem>,
    /// `source` is the tool's server.
    pub mcp_tools: Vec<ContextItem>,
    /// The conversation's tool calls and their results, by tool.
    pub tool_calls: Vec<ContextItem>,
}

/// A count of a card's session's context and when it was counted: as the session went to rest, which replaces the count
/// before and the store keeps by card, or now, while it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextCount {
    pub context: SessionContext,
    pub at: Time,
}

/// One part of a context. `kind`: `used` is sent with every request, `deferred` is listed by name until loaded,
/// `buffer` is kept free for compaction, `free` is left.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextPart {
    pub id: String,
    pub name: String,
    pub tokens: u64,
    pub kind: String,
}

/// One thing in a part of a context and its tokens; a `deferred` one is listed by name until loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextItem {
    pub name: String,
    pub source: String,
    pub tokens: u64,
    pub deferred: bool,
}

/// What Remit started a card's session with, recorded as it started the session's process: each start (a resume too)
/// replaces the one before. `given.opening` is the session's first message, recorded as it is sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionLaunch {
    pub card_id: String,
    pub home: String,
    pub session_id: String,
    pub at: Time,
    /// The process resumed the session rather than opening it.
    pub resumed: bool,
    pub given: ContextGiven,
}

/// What Remit gave a session as it started it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextGiven {
    /// The library's skills it holds, by name.
    pub skills: Vec<String>,
    /// Its Claude Code tools; none is every tool.
    pub claude_tools: Option<Vec<String>>,
    /// The settings layers it reads.
    pub setting_sources: Vec<String>,
    /// What it names that the library could not give.
    pub missing: Vec<GivenMissing>,
    /// What Remit's MCP server listed for it.
    pub tools: Vec<GivenTool>,
    /// Its chain's capabilities, as the catalog said them.
    pub capabilities: Vec<GivenCapability>,
    /// Never asked for permission.
    pub allowed_tools: Vec<String>,
    pub deny_rules: Vec<String>,
    /// The owner's principles file its brief points to; empty for none.
    pub principles: String,
    /// The session's first message as sent; none until it is sent, or when the session was opened before Remit
    /// recorded it.
    pub opening: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GivenMissing {
    pub kind: String,
    pub name: String,
    pub why: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GivenTool {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GivenCapability {
    pub id: String,
    pub label: String,
    pub gives: String,
}

/// An MCP server a session has and how it stands: connected, failed, needs-auth, pending.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MCPServerState {
    pub name: String,
    pub status: String,
    /// Where its definition came from: sdk (Remit), plugin, or a settings scope.
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Plugin {
    pub name: String,
    pub path: String,
    pub version: String,
}

/// A plugin that did not load, or loaded without a part of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginError {
    pub plugin: String,
    pub r#type: String,
    pub message: String,
}
