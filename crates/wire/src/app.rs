//! The contract between the owner's channels (the Vue shell, the CLI, the daemon) and the server, from
//! `web/src/rpc/define.ts`. Field names are the ones the Vue shell and the stored records use. Times are RFC 3339 strings as
//! the server writes them. `rev` on a record counts its stored changes: of two copies, the higher wins.

use model::{
    AccountKind, ActionKind, Actor, Agent, Assistant, Attachment, AttachmentRef, Card, Chain, ChainStatus, Change,
    ChangeKind, ClientKey, ContextCount, ContextGiven, Control, Decision, Draft, Edge, Employee, Event, EventKind,
    EventSignal, EventVia, Extension, Feedback, FeedbackKind, Finding, FindingStatus, ForeignJson, Help, Hold,
    InboundEvent, InboxItem, InitReport, Landing, Memory, Mentor, NearMiss, ObjectiveView, OutcomeRange, OutcomeReport,
    OutcomeWindow, Playbook, Project, Release, ReleasePause, Retrospective, Sensor, Source, Teaching, Time,
    VerdictRole, Where, Workflow,
};
use serde::{Deserialize, Serialize};

use crate::Ack;

/// A chain's watch on a signal, as the owner's pages read it: a reading that meets `condition` wakes its Chair, else the
/// deadline `until` does.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Watch {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    pub until: Time,
    pub why: String,
}

/// When and why a chain's Chair comes back: its earliest wake subscription, a time or the deadline of a watch. `watch` is
/// the watch that ends first, when the chain holds any.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckBack {
    pub at: Time,
    pub why: String,
    pub set_at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch: Option<Watch>,
}

/// A chain as a channel reads it: the stored chain and when its Chair comes back, which the server works out from the
/// chain's wake subscriptions (ADR-0107).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainRead {
    pub chain: Chain,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_back: Option<CheckBack>,
}

/// What a line of the board is, in the owner's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionKind {
    /// A waiting chain's Chair coming back at a time.
    #[default]
    CheckBack,
    /// A waiting chain's Chair waking when a signal's reading meets a condition, else at its deadline.
    Watch,
    /// One of the owner's routines: it starts a chain.
    Routine,
    /// A subscription to an event signal's events: it tells a Chair, starts a chain or runs a job for each event.
    Event,
    /// Something Remit does on its own: a hire that answers a fact, a job on a clock, a goal check's deadline (ADR-0035).
    Automation,
}

/// Where a line stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionState {
    /// Its time or its signal has not come.
    #[default]
    Waiting,
    /// Its time has come and admission lets it go: the next Tick acts.
    Due,
    /// A firing of it failed for good, or admission or a silent source holds it; `held` says why.
    Held,
    /// Its chain, routine or event signal is paused: it keeps its time and waits for it again once resumed.
    Paused,
    /// A routine's last run has begun: its chain is queued or at work.
    Underway,
    /// It has no next time: a routine that ran its last run.
    Over,
}

/// The part of the board a line sits in, in the order the owner looks at them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionGroup {
    /// Held or due: it should have acted by now.
    Attention,
    /// Waiting for its time or its signal, or under way.
    #[default]
    Coming,
    /// Paused, or with no next time.
    Idle,
}

/// One line of the Subscriptions tab.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionLine {
    pub id: String,
    pub kind: SubscriptionKind,
    /// The chain's or the routine's title, or what the automation or event subscription does.
    pub title: String,
    pub chain_id: String,
    /// An event signal's id: the record its page shows.
    pub event_signal_id: String,
    pub source: SignalSource,
    pub key: String,
    pub condition: String,
    /// When an event deserves a chain, as a decider judges it (`Subscription::screen`): a start's, empty for any other.
    pub screen: String,
    /// When it is next due; none when it is not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Time>,
    /// What the Chair will check; what a routine does and how often; what a subscription does with an event.
    pub what: String,
    pub state: SubscriptionState,
    pub group: SubscriptionGroup,
    /// Why it is held: the firing that failed and its error, admission, or a silent source.
    pub held: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_fired: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_read: Option<Time>,
    pub control: Control,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<Hold>,
    /// What the delivery of its next firing waits on.
    pub waiting: String,
    pub near_misses: Vec<NearMiss>,
}

/// A kind of signal Remit supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSource {
    /// A key no source writes.
    #[default]
    #[serde(rename = "")]
    None,
    /// Time itself: a subscription's own schedule, schedule:<subscription>.
    Clock,
    /// What comes in, whichever way: event:<name>.
    Event,
    /// How a chain's change stands on its way onto a base: chain:<id>.landing.<repository> (ADR-0114).
    Landing,
    /// A chain's change is live: chain:<id>.live.
    Live,
    /// The state a script sensor of a chain reads: chain:<id>.sensor.<name> (ADR-0047).
    Sensor,
    /// What Remit records of its own state: fact:<noun>.<verb> (ADR-0035).
    Fact,
}

impl SignalSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Clock => "clock",
            Self::Event => "event",
            Self::Landing => "landing",
            Self::Live => "live",
            Self::Sensor => "sensor",
            Self::Fact => "fact",
        }
    }
}

/// One source as the owner reads it: what its signals say, who listens to them, and how much of it there is now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalSourceLine {
    /// The address of the line: source:<source>.
    pub id: String,
    pub source: SignalSource,
    pub title: String,
    /// The shape of its keys.
    pub pattern: String,
    /// What a reading says and what writes it.
    pub what: String,
    /// Who may subscribe to it, and to do what.
    pub who: String,
    /// Its keys with a reading or a subscription.
    pub signals: usize,
    /// The lines of the Subscriptions tab that listen to it.
    pub subscriptions: usize,
    /// When the newest reading of any of its signals came; none for time, and while none came.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_read: Option<Time>,
}

/// A sensor of a signal that has stopped reading, and why.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilentSensor {
    pub sensor_id: String,
    pub why: String,
}

/// One signal of the Signals tab, with its newest reading.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalLine {
    pub id: String,
    pub key: String,
    /// The source that writes it; none for a key none writes.
    pub source: SignalSource,
    pub title: String,
    /// The newest reading; 0 while it has none.
    pub rev: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Time>,
    pub cause: String,
    /// How many lines of the Subscriptions tab listen to it.
    pub subscribers: usize,
    /// Why its source is not being read, while it is silent.
    pub silent: String,
    pub silent_sensors: Vec<SilentSensor>,
    pub event_signal_id: String,
}

/// One firing, in words.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiringLine {
    /// The subscription that fired: a line of the Subscriptions tab, while it exists.
    pub subscription_id: String,
    /// The chain it acted on.
    pub chain_id: String,
    /// The agent it hired, in `chain_id`.
    pub card_id: String,
    pub at: Time,
    /// Whose subscription fired.
    pub title: String,
    /// The signal it fired on; none for the clock.
    pub key: String,
    pub action: ActionKind,
    pub outcome: String,
    /// Its deadline came with no reading to fire it.
    pub expired: bool,
}

/// Everything the Signals and Subscriptions tabs list, in the order to look at it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalBoard {
    /// Every source of signals Remit supports, in `signal_source_specs`' order.
    pub sources: Vec<SignalSourceLine>,
    pub subscriptions: Vec<SubscriptionLine>,
    pub signals: Vec<SignalLine>,
    /// Signals with a reading or a listener that are not listed.
    pub signals_more: usize,
    pub held: usize,
    /// Counts the changes of the board since this process started: of two, a channel keeps the higher.
    pub rev: i64,
}

/// One reading, as the owner reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingLine {
    pub rev: i64,
    pub at: Time,
    pub cause: String,
    /// What the signal said, as text, cut at `VALUE_SHOWN` characters.
    pub value: String,
    /// `value` is the start of a longer one.
    pub truncated: bool,
    /// For an event, the address of what became of it.
    pub event_id: String,
}

/// One row of the Signals table: a reading of the signal `key`, or a firing of a subscription on it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalRow {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading: Option<ReadingLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firing: Option<FiringLine>,
}

/// What one opened line of the board adds: the newest readings and firings, newest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalDetail {
    pub title: String,
    /// The signal; none for a subscription to the clock.
    pub key: String,
    pub readings: Vec<ReadingLine>,
    pub firings: Vec<FiringLine>,
}

/// The workload as a Tick counted it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workload {
    pub running: usize,
    /// The chains the scheduler runs at once, measured each Tick from what presses (ADR-0115).
    pub capacity: usize,
    /// What pressed on the capacity at the last Tick, in words; empty when nothing did.
    pub pressure: String,
    pub queued: usize,
    /// Chains that wait: on a check-back, or on their mentors alone.
    pub waiting: usize,
    /// The earliest check-back of the chains that wait; none when none holds one, as a wait on mentors alone does not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_check_back: Option<Time>,
    /// Counts the Ticks since this process started: of two, a channel keeps the higher.
    pub rev: i64,
}

/// What a card shows of the moment, pushed as it changes and not stored yet: its activity and how full its context is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardShown {
    pub id: String,
    pub home: String,
    pub rev: i64,
    pub activity: String,
    pub ctx_used: i64,
    pub ctx_size: i64,
}

/// What an event's data (`Event::data`) holds that a channel and the server's typing of it (`chains::activity`) read,
/// each key absent where the event's kind keeps none: a tool call's input and output, a turn's length, where the owner
/// was, a permission prompt's request and answer. A background task's event carries Claude Code's `task_id` and task
/// `kind`, the `tool` call that started it (Monitor, Bash) and the `command` it runs, whether what it writes can be read
/// (`readable`, through `card_task_output`) and whether the owner can end it from its card (`stoppable`, through
/// `card_task_stop`), and once it ended the end of what it wrote (`output`). Other keys a kind keeps are read where that
/// kind is typed (a turn's row, a block, the marks on a reply).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<ForeignJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placed: Option<Placed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stoppable: Option<bool>,
    /// The attachments a message holds, read with `attachment_get`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<AttachmentRef>>,
}

/// Where the owner was as he wrote to the Assistant, and the server's words for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    pub r#where: Where,
    pub label: String,
}

/// What kind of work a tool call did, as the stream groups and counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolGroup {
    Read,
    Edit,
    Execute,
    Search,
    Fetch,
    Remit,
    Other,
}

/// Where a tool call stands as a channel shows it: running until it has ended (`model::ToolStatus` is where it stands as
/// Claude Code reports it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    Running,
    Completed,
    Failed,
}

/// An event of an agent's work as the server types it, for a channel to render without reading the tool's raw input
/// or title (`chains::activity`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum Activity {
    /// A tool call: its group, the name it is shown by (the program a command runs, the tool), what it did in words,
    /// and where it stands.
    Tool {
        group: ToolGroup,
        name: String,
        summary: String,
        status: ToolState,
    },
    /// A message the agent sent another through a tool that took it: `to` is the card (none: its Chair).
    Sent {
        #[serde(skip_serializing_if = "Option::is_none")]
        to: Option<String>,
        kind: String,
        label: String,
        text: String,
    },
    /// Where a turn ended, how long it took when that was recorded, and its row: what it touched, its failures and
    /// refusals; none on a turn that ended before rows were recorded.
    TurnEnd {
        #[serde(skip_serializing_if = "Option::is_none")]
        ms: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn: Option<TurnSummary>,
    },
    /// A permission prompt asked, until it is decided: its id, the command that matters in it, and who is asked.
    Asked { id: String, request: String, of: Asker },
    /// A permission prompt decided: its id (none when a rule refused it before it was asked), the command that matters
    /// in what was asked, and whether it was allowed.
    Decided {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        request: String,
        allowed: bool,
    },
    /// A block the agent showed (`show_block`, ADR-0091): its parts as it drew them, with the picks as they stand.
    Block { block: Block },
    /// An agent's reply that people marked (`turn_teach`, ADR-0093): each one's mark as it stands, one per person.
    Marked { marks: Vec<ReplyMark> },
    /// A session break someone asked for, the owner's Start over, as against one a full context or a lost session
    /// brought: the conversation shown begins at the newest one.
    StartedOver,
}

model::json_tagged!(Activity, "type");

/// One person's mark on an agent's reply as it stands (ADR-0093): their newer mark on it replaces it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyMark {
    /// The person who marked it: a mentor's id, the owner's `m_owner`.
    pub mentor: String,
    /// Who acted: the owner, or the mentor through their help.
    pub by: Actor,
    pub mark: TurnMark,
    /// Why, or the correct reply; empty when they said nothing.
    pub words: String,
    pub at: String,
}

/// What one turn did, as its row tells it: what it touched, each told once with how many times, and its failures and
/// refusals kept apart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSummary {
    /// In the order it first touched each; one that failed every time is only a failure.
    pub touched: Vec<Touched>,
    /// In the order they came.
    pub failures: Vec<TurnFailure>,
    /// The turn had more events than the server read: the row tells the first of them.
    #[serde(default)]
    pub partial: bool,
}

/// One thing a turn touched: the tool's group, the name it is shown by, what it touched, and how many calls touched it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Touched {
    pub group: ToolGroup,
    pub name: String,
    pub summary: String,
    pub count: i64,
}

/// A step of a turn that did not go through; `event_id` is the event that says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnFailure {
    pub event_id: i64,
    pub kind: FailureKind,
    pub name: String,
    #[serde(default)]
    pub summary: String,
}

/// How a step did not go through: a tool call that failed, or a call refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Failed,
    Refused,
}

/// What an agent draws in its stream with `show_block` (ADR-0091): a title and the parts the app draws with its own
/// components. Checked once at the tool (`orch` `check_block`) and trusted after.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BlockSpec {
    /// What the block is about, in a few words.
    pub title: String,
    pub parts: Vec<BlockPart>,
}

/// One part of a block. A new kind is a new variant: stored blocks keep reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(remote = "Self", rename_all = "snake_case")]
#[schemars(tag = "type")]
pub enum BlockPart {
    /// Options the owner may pick from: one, or several when `many`. `picked` holds option ids: the agent's own pick
    /// from the start, which it goes ahead with; the owner's once he changed it.
    Choice {
        id: String,
        label: String,
        options: Vec<BlockOption>,
        #[serde(default)]
        many: bool,
        picked: Vec<String>,
    },
    /// Numbers drawn as bars or lines: one value of each series per label of `x`.
    Chart {
        id: String,
        label: String,
        kind: ChartKind,
        x: Vec<String>,
        series: Vec<ChartSeries>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
}

model::json_tagged!(BlockPart, "type");

/// An option of a choice; `detail` is a line that compares it with the others.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BlockOption {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// How a chart draws its series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Line,
}

/// One named series of a chart, a value for each label of its `x`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<f64>,
}

/// A block as stored and shown: the spec, its choices' picks as they stand, and who changed them last and when (none:
/// they are still the agent's).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub spec: BlockSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picked_by: Option<Actor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picked_at: Option<String>,
}

/// Who a permission prompt is asked of first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asker {
    Chair,
    Owner,
}

/// An event as a channel reads it: the stored event, and what the server types it as (`chains::activity`), for a channel
/// to render without reading the tool's raw input or title. The steps a channel shows as such have one (a tool call, a
/// message sent through a tool, a turn's end, a permission prompt asked or decided, a block, a marked reply, the owner
/// starting over); the rest are shown by their kind.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRead {
    pub event: Event,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<Activity>,
}

/// One streamed chunk, sent to browsers as it arrives; the stored event catches up in batches. `offset` is where `text`
/// goes in the event's text: how long that text was before it, in UTF-16 code units (a JavaScript string's length), so a
/// channel holding the event at another length knows it missed a chunk, or has this one already.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventDelta {
    pub id: i64,
    pub home: String,
    pub card_id: String,
    pub kind: EventKind,
    pub text: String,
    pub offset: usize,
}

/// A decision as a chain's stream reads it: the record, and whether it is an outcome of the chain's work (paused or
/// resumed, an agent stopped, finished or reopened, a landing landed, sent back or taken back, the change live,
/// archived or unarchived), which the stream shows where it happened, with who and why (ADR-0081). The server works
/// `outcome` out; it is never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRead {
    pub decision: Decision,
    pub outcome: bool,
}

/// A landing as a chain's screen reads it (ADR-0114, ADR-0116): the record, the release that carried it to its
/// customer as that release stands now (none until one did), and the revert taking it back now, when one is on its way
/// (`Landing::taking_back`), so the landing it takes back finds it without a rule of the page's. The server reads both
/// for each landing it pushes (`chains::landings::landing_read`), and pushes each landing a release carries again when
/// the release moves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandingRead {
    pub landing: Landing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<Release>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taking_back: Option<String>,
}

/// One repository as its queue and Releases views read it (ADR-0114, ADR-0116), laid out by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoView {
    /// Its catalog id, as in RepoWork.
    pub repo: String,
    /// A queue for each base it lands on, by base.
    pub queues: Vec<LandingQueue>,
    /// Its newest releases, at most `RELEASES_SHOWN`, the newest first.
    pub releases: Vec<ReleaseRead>,
    /// Its last release pause, held or lifted (ADR-0116); none when releases were never paused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause: Option<ReleasePause>,
}

/// The queue of one repository and base: the batch being combined and checked, then what is ready in the order the
/// queue takes it, then what landed, went back or was reverted in the last `QUEUE_ENDED_SHOWN_HOURS`, the latest first.
/// A version still judged is not in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandingQueue {
    pub base: String,
    pub landings: Vec<Landing>,
}

/// A release with the landings it carried, the oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRead {
    pub release: Release,
    pub landings: Vec<Landing>,
}

/// How many of a repository's releases its view holds: the newest.
pub const RELEASES_SHOWN: usize = 20;

/// How long a landing that ended (landed, went back, reverted) stays in its queue's view.
pub const QUEUE_ENDED_SHOWN_HOURS: i64 = 24;

/// A draft as a channel saves it: the server adds when and its revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftInput {
    pub id: String,
    pub home: String,
    pub text: String,
    /// A message's attachments, put first with `attachment_put`.
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
    /// The card it is to.
    pub card_id: String,
}

/// The draft a channel saves, as the store keeps it until the server stamps who writes it, when and its revision.
impl From<DraftInput> for Draft {
    fn from(d: DraftInput) -> Self {
        Self {
            id: d.id,
            home: d.home,
            text: d.text,
            attachments: d.attachments,
            card_id: d.card_id,
            ..Self::default()
        }
    }
}

/// Whether agents can run, read off what is known of each account.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub ok: bool,
    /// What each account failed with while none can run agents; empty while one can.
    pub error: String,
    /// Every account, the oldest first.
    pub accounts: Vec<AccountHealth>,
    /// When an account last answered or failed; none until one has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<Time>,
    /// The stored records this build cannot read, left out of every list it shows (ADR-0107): a newer build wrote them,
    /// so none but after a rollback. Each is left as it is for a build that reads it.
    pub unreadable: u32,
    /// Counts the changes since this process started: of two, a channel keeps the higher.
    pub rev: i64,
}

/// Whether one account can run agents, and what it last failed with while it cannot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountHealth {
    pub id: String,
    /// What it is, and so whether the owner can remove it.
    pub kind: AccountKind,
    /// The address it signed in as; empty while no address is known, as for the default account whose login names none.
    pub email: String,
    /// A plugin account's: the plugin that provides it.
    pub plugin: String,
    /// And the connection of that plugin it is.
    pub connection: String,
    pub ok: bool,
    /// What the account failed with.
    pub error: String,
    /// When the limit it hit resets, or, while it takes work, the limit the provider warns is near; none when it did not
    /// say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<Time>,
    /// The limit the provider warns is near, in the owner's words ("Weekly limit"), while it takes work; empty while none is.
    pub near_limit: String,
    /// How much of that limit is used, from 0 to 1, while one is near; none otherwise or when the provider did not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utilization: Option<f64>,
}

/// A skill as a channel lists it: its files by path and size, not their text, which may be long and is read one file at
/// a time (`skill_file`) as the owner opens it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedSkill {
    pub name: String,
    pub description: String,
    pub group: String,
    /// By path.
    pub files: Vec<SkillFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    pub rev: i64,
}

/// One file of a skill, by its path inside the skill and its size in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillFile {
    pub path: String,
    pub size: usize,
}

/// The text of one file of a skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillFileText {
    pub text: String,
}

/// Where a variable's value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceVarState {
    Unset,
    /// Stored by Remit.
    Set,
    /// Set by the process environment or an env file: `default` only.
    Environment,
}

/// A tool of a source and what it answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceTool {
    pub name: String,
    pub what: String,
}

/// One variable of a connection. `value` is filled for a plain variable that is set, and never for a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceVariable {
    pub name: String,
    pub what: String,
    pub secret: bool,
    pub optional: bool,
    pub address: bool,
    pub state: SourceVarState,
    /// Empty for a secret, and for a variable that is not set.
    pub value: String,
}

/// One connection of a source: what it reaches, whether it can be read, and its variables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionView {
    pub name: String,
    pub description: String,
    pub configured: bool,
    /// What `missing` says it still needs.
    pub missing: Vec<String>,
    /// Why its stored values cannot be read (a sealed value that does not open): its variables are then unknown. Empty
    /// when they can be.
    pub problem: String,
    #[serde(rename = "variables")]
    pub vars: Vec<SourceVariable>,
}

/// One source as the Tools page and `list_sources` show it. It carries no secret: a secret variable says only whether it is
/// set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSource {
    pub id: String,
    pub label: String,
    pub capability: String,
    pub tools: Vec<SourceTool>,
    pub connections: Vec<ConnectionView>,
}

/// Everything that draws one chain: its records, where each of its agents stands, and where it stands.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainView {
    pub chain: ChainRead,
    pub cards: Vec<Card>,
    pub edges: Vec<Edge>,
    pub decisions: Vec<DecisionRead>,
    /// The owner's, not sent, oldest first.
    pub drafts: Vec<Draft>,
    /// The mentors' helps on its cards, oldest offer first.
    pub helps: Vec<Help>,
    /// Its runs, each with its steps, oldest first.
    pub workflows: Vec<Workflow>,
    pub agents: Vec<AgentStanding>,
    pub verification: Verification,
    /// The chain's, as the board has it: an archived chain, which the snapshot leaves out, has it here.
    pub standing: ChainStanding,
    /// What was decided in it since the owner last viewed it (ADR-0079): read once, as viewing it moves his mark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub away: Option<Away>,
    /// The coach's retrospective of each finish, oldest first (ADR-0082).
    pub retrospectives: Vec<RetrospectiveRead>,
    /// Its landings, each with the release that carried it, the oldest first (ADR-0114, ADR-0116).
    pub landings: Vec<LandingRead>,
    /// Where Datadog shows what it did (ADR-0119); empty while nothing is sent there.
    pub datadog: Vec<DatadogLink>,
}

/// One place in Datadog the owner opens from a chain's screen: what it shows, in a word, and its address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatadogLink {
    pub label: String,
    pub url: String,
}

/// What was decided in a chain since the owner last viewed it, oldest first: the newest `AWAY_SHOWN`, and how many more
/// there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Away {
    /// When he last viewed the chain.
    pub since: Time,
    pub lines: Vec<AwayLine>,
    pub more: usize,
}

/// One decision made while the owner was away: who made it and its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwayLine {
    pub at: Time,
    pub who: Actor,
    pub text: String,
}

/// The coach's retrospective of a chain's finish as a channel reads it (ADR-0082): with the coach's name, which lives on a
/// chain of its own the page does not hold, resolved when it is shown and never stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrospectiveRead {
    pub retrospective: Retrospective,
    pub by_name: String,
}

/// How something judged reads at a glance: `label`, in the colour `tone` names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckLook {
    pub label: String,
    pub tone: CheckTone,
}

/// The colour of how something judged reads (`CheckLook`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckTone {
    Needs,
    Done,
    Running,
    Failed,
}

/// Where a gate stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateState {
    Approved,
    /// An approval of an earlier version of the change than the one it has now.
    Stale,
    Changes,
    /// The goal judge at work.
    Running,
    /// Nobody asked yet.
    #[default]
    None,
}

/// One gate a chain's result passes (`key`: review | qc | security | goal), as the owner reads it: its state and line,
/// who stands behind it (the first name, and how many more), and `rounds`, every verdict it has given, oldest first, true
/// for an approval. `watch` is the goal judge's card while it works.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateView {
    pub key: String,
    pub label: String,
    pub state: GateState,
    pub line: String,
    pub who: String,
    pub rounds: Vec<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch: Option<String>,
}

/// An item a verdict judges differently from the round before it by the same judge: `from` is its status then (none when
/// that round did not list it), `to` its status now (none when this round no longer lists it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Move {
    pub item: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<FindingStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<FindingStatus>,
}

/// One verdict as the owner reads it: the agent that gave it, what it means (`check`), the line it opens with
/// (`headline`) and the proof behind it (`rest`, `more` lines of it), the findings, each with its evidence, and which of
/// them moved since the round before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerdictView {
    pub id: String,
    pub card_id: String,
    pub check: CheckLook,
    pub headline: String,
    pub rest: String,
    pub more: usize,
    pub findings: Vec<Finding>,
    pub moves: Vec<Move>,
    pub at: Time,
}

/// One agent's judgement as it stands: its latest verdict, and its earlier rounds, newest first. `card_id` is the agent
/// that gave them. `repo` is named only when the chain changes several repositories and the judge's name does not already
/// say which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Judge {
    pub key: String,
    pub card_id: String,
    pub role: VerdictRole,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    pub latest: VerdictView,
    pub earlier: Vec<VerdictView>,
}

/// How one chain's result has been judged: the same gates for every chain, then who judged, in the order the owner reads
/// them: the goal check first, the one verdict on the whole chain, then the reviewers and the QCs of its changes, each
/// newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub chain_id: String,
    pub gates: Vec<GateView>,
    pub judges: Vec<Judge>,
    pub rev: i64,
}

/// Where a chain stands on a board: the owner's columns, then a mentor's (orch's `mentor_board`), whose Finished is the
/// owner's word too. Ordered as a mentor's board looks at them: asked, helping, finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    /// A mentor is asked to help one of its cards and has not answered.
    Asked,
    /// A mentor helps one of its cards.
    Helping,
    /// Something waits for the owner: an open item only he can settle (ADR-0087).
    Needs,
    /// Running, or paused.
    Running,
    /// Its Chair checks back later, or a mentor's message wakes it.
    Waiting,
    /// A chain whose change is merged and whose Chair watches a signal until the effect is verified (ADR-0013): it takes
    /// no slot and is not idle.
    Watching,
    Queued,
    /// Finished, however it ended: Done, Waiting to ship or Not done, as its badge says.
    Finished,
}

/// One column of the board: its chains in order and, in the column that waits for the owner, what waits for him about
/// the chains and from none of them there is (the files a deleted chain left, the idle watchdog), as inbox item ids. What
/// an agent in no chain asks (the Assistant) is not here: it waits with that agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardColumn {
    pub key: Column,
    pub label: String,
    /// What fills this column and who puts it there, said while it holds nothing.
    pub empty: String,
    pub chains: Vec<String>,
    pub items: Vec<String>,
}

/// Every chain on the owner's board, column by column, each in the order it is looked at, and what waits for him there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub columns: Vec<BoardColumn>,
    /// How many chains are on no column: the owner put them away (`chains_archived` reads them).
    pub archived: usize,
    /// How much waits for the owner: the chains and the items of the column that waits for him.
    pub needs_you: usize,
    /// Everything that waits for the owner, in the one order every surface shows (ADR-0078).
    pub waiting: Vec<Wait>,
    pub rev: i64,
}

/// One thing that waits for the owner: an open inbox item only he can settle (ADR-0087). `id` is the inbox item's id;
/// `label` is the server's word for its kind; `unseen`: it began after the owner last viewed its home (ADR-0079).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wait {
    pub id: String,
    pub kind: WaitKind,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    /// Who it is about, in words.
    pub who: String,
    pub ask: String,
    pub since: Time,
    pub unseen: bool,
}

/// What kind of thing waits: an inbox item's kind (a question, a permission, a failure, a change, a credential, a login,
/// an exception, a held subscription).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitKind {
    Asks,
    Approve,
    Failed,
    Apply,
    Key,
    SignIn,
    Decide,
    Held,
}

/// Where one chain stands for the owner, as its board card and its page say it: the badge (`text`, in the colour `tone`
/// names), whether it works right now, what of it waits for him, when it checks back, its place in the queue and why it
/// stands where it stands (`why`), which a queued chain's Overview and an event's chains say.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStanding {
    pub chain_id: String,
    pub text: String,
    /// The column's, or paused: a paused chain sits in the Running column but does not work; in the column that waits
    /// for the owner, needs, or failed when its first wait is a failure.
    pub tone: ChainTone,
    pub working: bool,
    /// When its Chair checks back; none while it holds no check-back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_back_at: Option<Time>,
    /// From 1, while it is queued.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<usize>,
    /// How its result has been judged; none while nobody has judged it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<CheckLook>,
    /// Why it stands where it stands: in the column that waits for the owner, what the agent asks or the Chair's reason
    /// for its rating; while it waits, why its Chair checks back; while its start is held, why. None where its column
    /// says enough.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// Its first wait in the board's order; none while nothing of it waits for the owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<Wait>,
    /// How many of the board's waits are its own.
    pub waits: usize,
    /// It finished after the owner last viewed it (ADR-0079).
    pub unseen: bool,
    /// What the owner can ask of it.
    pub asks: Vec<ChainAsk>,
    /// Who it came from, as the owner reads it.
    pub from: String,
    /// The id of its project, its objective's (ADR-0108); none when it has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub rev: i64,
}

/// The colour of a chain's standing: its column's (a finished chain's: done, or not done), paused, or in the column that
/// waits for the owner, needs, or failed when its first wait is a failure (ADR-0078).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainTone {
    #[default]
    Running,
    Waiting,
    Watching,
    Queued,
    Done,
    /// A finished chain the goal judge did not confirm and that waits on nothing of the owner's (ADR-0067).
    #[serde(rename = "notdone")]
    NotDone,
    Paused,
    /// Its first wait holds something up for the owner.
    Needs,
    /// Its first wait is a failure.
    Failed,
}

/// Something the owner can ask of a chain as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainAsk {
    Pause,
    Resume,
    Finish,
    Archive,
    Unarchive,
    Delete,
}

/// Where one agent stands for the owner, wherever it shows: its badge (`label`, in the colour `tone` names) and, in a
/// line, what it is doing right now (`doing`, in the colour `doing_tone` names). `waiting` is the open inbox items with
/// it (their ids), pinned under its stream: those that wait on the owner, and those the Assistant decides for him. An
/// item stays with the agent it names, and one that names no agent of the chain waits with the Chair.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStanding {
    pub card_id: String,
    pub home: String,
    pub label: String,
    pub tone: AgentTone,
    pub doing: String,
    pub doing_tone: DoingTone,
    /// What its work is while it runs: the step of its plan in progress, else its brief's title; empty while it does not
    /// run, or holds neither. Its line reads `work`, then the time since `since`, then `doing`.
    pub work: String,
    /// Since when it stands as its line says: its turn's start while it runs, the oldest thing waiting on the owner with
    /// it while one waits; none when the server knows no start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<Time>,
    /// Its time counts as work (it may warm): it runs, held up by nothing known. A wait with a known reason (the owner,
    /// the Chair, a provider, a paused chain) never warms.
    pub warms: bool,
    pub waiting: Vec<String>,
    /// Its place in the agent tree: the agents that hang under it, the other earlier agents it works with besides the one
    /// it hangs under and, on the Chair, the cards that stand beside it, each oldest first; and its branch.
    pub children: Vec<String>,
    pub with: Vec<String>,
    pub peers: Vec<String>,
    pub branch: Branch,
    /// What the owner can ask of it.
    pub asks: Vec<Ask>,
    /// What stopping it does, in the server's words: what the owner reads before he stops it. Empty when he may not.
    pub stop_says: String,
    /// Whether it has a memory to read (`card_memory`): an employee's card, or the Assistant's.
    pub remembers: bool,
    /// None while it is not in a turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_message: Option<OnMessage>,
    /// What a message sent now does, in the server's words, when it is not read at once ("Waits for the current turn
    /// (+2 queued)"); the composer shows it in its button row (ADR-0080).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_send: Option<String>,
    pub rev: i64,
}

/// Every seat at work now, in the server's order: what waits on the owner first, then what runs, the longest at it
/// first, then the rest that wait.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtWork {
    pub seats: Vec<Seat>,
    pub rev: i64,
}

/// One agent at work or waiting, wherever it sits: its card and home, what it is (`kind`), the chain's title (empty for
/// the Assistant), its name and employee, its badge, its line (`work`, the time since `since`, `doing`), whether that
/// time counts as work (`warms`), the seats under it at work too (`helpers`, card ids, which end with it), whether the
/// owner may stop it, and what a stop does (`stop_says`, the server's words, empty when he may not).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seat {
    pub card_id: String,
    pub home: String,
    pub kind: SeatKind,
    pub chain_title: String,
    pub name: String,
    pub employee: String,
    pub label: String,
    pub tone: AgentTone,
    pub work: String,
    pub doing: String,
    pub doing_tone: DoingTone,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    pub warms: bool,
    pub helpers: Vec<String>,
    pub stop: bool,
    pub stop_says: String,
}

/// What a seat is: the Assistant, a chain's Chair, or any other agent of a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeatKind {
    Assistant,
    Chair,
    Agent,
}

/// An employee's runs, newest first, at most `RUNS_SHOWN` (`more`: older ones are left out), and over all of them how
/// many failed and how many wait (a permission asked, a provider out, a report awaiting the Chair's acceptance).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployeeRuns {
    pub employee: String,
    pub runs: Vec<Run>,
    pub more: bool,
    pub failures: i64,
    pub waiting: i64,
}

/// One card an employee held: its chain and the chain's title, the card, the first line of what it was asked (the
/// chain's goal for a Chair, its brief otherwise), how it ended (`card_outcome`: accepted, sent back, failed, stopped,
/// open), where the ask came from (`source`: the chain's origin for a Chair, "chair" for an agent the Chair hired, "agent"
/// for a subagent another agent spawned, "remit" for a judge), when it started and when its work last ended (`ended`:
/// none while it works, and on a card that ended before end times were recorded).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub chain_id: String,
    pub chain_title: String,
    pub card_id: String,
    pub request: String,
    pub outcome: String,
    pub source: String,
    pub waiting: bool,
    /// Its card is at its work now (`CardStatus::works`).
    pub live: bool,
    pub started: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<Time>,
}

/// Something the owner can ask of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ask {
    /// A message to its open session, read once its turn ends.
    Message,
    /// A message put into its turn in progress (`Delivery::Steer`), offered while a turn runs in a session that takes
    /// input.
    Steer,
    /// A message that resumes its closed session (and reopens a finished chain).
    Resume,
    /// The turn in progress ended.
    Interrupt,
    /// Its session ended for good.
    Stop,
    /// A new session, its own having gone.
    Restart,
}

/// How a message the owner sends meets a turn in progress (ADR-0080): `queue` is read after it (or now when none runs),
/// `steer` goes into it (queued when it ended meanwhile), `interrupt` ends it and is read next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Queue,
    Steer,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTone {
    Needs,
    Failed,
    Waiting,
    Running,
    Done,
    Idle,
}

/// An agent nothing is known of yet stands idle.
impl Default for AgentTone {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoingTone {
    Busy,
    Needs,
    Waiting,
    Idle,
    Done,
    Error,
}

/// An agent nothing is known of yet does nothing.
impl Default for DoingTone {
    fn default() -> Self {
        Self::Idle
    }
}

/// An agent and everything that hangs under it in the tree: how many agents hang under it, all the way down, how many
/// turns they took with it, and whether one of them is at work (the turns still climb).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub agents: i64,
    pub turns: i64,
    pub working: bool,
}

/// The message an agent in a turn is on: the newest it took to open a turn (`event_id`), and whether it has begun to
/// answer it. A receipt under that message says it is being read, and under every other message it took that it was
/// read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnMessage {
    pub event_id: i64,
    pub answering: bool,
    /// When its session took it: the start of the turn.
    pub at: Time,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub added: i64,
    pub removed: i64,
    pub diff: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
}

/// A landing's change as the queue lands it: the files from the base it forked from to the version the gates approved,
/// read from Remit's clone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandingDiff {
    pub landing_id: String,
    pub from: String,
    pub files: Vec<FileChange>,
}

/// A named set of tools a chain holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityView {
    pub id: String,
    pub label: String,
    pub gives: String,
}

/// The end of what a background shell or a Monitor has written: its last 8 KiB (`text`), all it wrote (`total_bytes`), and
/// whether `text` starts partway through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOutput {
    pub text: String,
    pub total_bytes: u64,
    pub truncated: bool,
}

/// What one card's agent has in its context, for the owner to understand it. It is read when the owner asks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardContext {
    pub card_id: String,
    /// What Remit gave the card's session as it started its process; none before Remit recorded a start of it.
    pub given: Option<ContextGiven>,
    /// The session's newest init; none until the session's first turn reports one.
    pub loaded: Option<InitReport>,
    /// Counted now while the session runs, else the count of when it last went to rest; none before either.
    pub context: Option<ContextCount>,
    /// Why the running session could not count it now; the stored count, when there is one, stands in. Empty when it was
    /// counted now, or no session runs.
    pub context_error: String,
}

/// A card's telemetry. Latencies are milliseconds, zero with no request; times are milliseconds since the Unix epoch.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTelemetry {
    pub requests: u64,
    pub errors: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
    pub latency_p50_ms: u64,
    pub latency_p95_ms: u64,
    /// Time to the first token.
    pub ttft_p50_ms: u64,
    /// By model and effort as each request reported them, most requests first.
    pub models: Vec<ModelUse>,
    /// By time spent, most first.
    pub tools: Vec<ToolUse>,
    /// Newest first.
    pub recent_errors: Vec<ApiError>,
    pub first_at_ms: i64,
    pub last_at_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUse {
    pub model: String,
    /// Empty when the requests reported none.
    pub effort: String,
    pub requests: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUse {
    pub name: String,
    pub runs: u64,
    pub failed: u64,
    pub total_ms: u64,
    pub p95_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub at_ms: i64,
    pub model: String,
    pub status: String,
    pub error: String,
}

/// A page of archived chains, newest change first, with where they stand. `next` is where the page after it starts:
/// empty when there is none.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChainPage {
    /// Finished, so none holds a wake.
    pub chains: Vec<ChainRead>,
    pub standings: Vec<ChainStanding>,
    pub next: String,
}

/// How many of a chain's newest events `chain_events` answers, and the most the client core keeps of the open chain as
/// more are pushed: what its Live feed shows (ADR-0095). An agent's older events are read by `card_events`.
pub const CHAIN_EVENTS_SHOWN: usize = 600;

/// How many events one page of an agent's stream holds (`card_events`): the newest, or the newest before an event. The
/// client core keeps one page per page the owner asked for, however long the agent goes on (ADR-0095).
pub const CARD_EVENTS_PAGE: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    pub events: Vec<EventRead>,
    pub truncated: bool,
}

/// The analyst chain (the one whose capabilities include outcomes): its status, when its last run began (its start or
/// its last wake) and when it checks back next, while it waits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeAnalyst {
    pub chain_id: String,
    pub status: ChainStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<Time>,
}

/// The Outcomes page: the report as of its `at`, and the analyst that reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcomes {
    pub report: OutcomeReport,
    /// None when no analyst was ever started.
    pub analyst: Option<OutcomeAnalyst>,
}

/// The Judgments page (ADR-0118), laid out on the server in the owner's words: each question Remit asks its deciders,
/// the questions asked last first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Judgments {
    pub questions: Vec<JudgmentQuestion>,
}

/// One question: what it is for, how it does in sentences and figures, its latest mistakes, and what only someone tuning
/// it reads, under its details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgmentQuestion {
    /// Its id and version, `memory.same_claim@1`.
    pub key: String,
    /// What it is for: `Finding duplicate memories`.
    pub title: String,
    /// Whether it has settled: the level of certainty it decides alone at is measured, or still the first guess. The
    /// figures say the rest.
    pub summary: String,
    /// Which model answers it, why that one, and which answer beside it.
    pub answered_by: String,
    pub figures: Vec<JudgmentFigure>,
    /// Its latest used answers that proved wrong, the newest first.
    pub mistakes: Vec<JudgmentMistake>,
    pub details: JudgmentDetails,
}

/// One figure of a question: its name and what it means, its value, and what the value is over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgmentFigure {
    pub label: String,
    pub hint: String,
    pub value: String,
    pub over: String,
}

/// An answer the code used whose pick proved wrong, said as what happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgmentMistake {
    pub id: String,
    /// "Said two memories were the same lesson; the librarian kept them apart".
    pub text: String,
    /// How sure it was, and what that made of it: "was 93% sure".
    pub sure: String,
    /// When it proved wrong.
    pub at: Time,
    /// The chain it was asked in, and the card it was about, where the owner opens it; empty when none.
    pub home: String,
    pub card: String,
}

/// What only someone tuning a question reads: its facts in words, and how each model stands on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgmentDetails {
    pub facts: Vec<JudgmentFact>,
    /// Every model asked it, the one that answers it first, then the best Brier score first.
    pub deciders: Vec<DeciderStanding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgmentFact {
    pub label: String,
    pub value: String,
}

/// How well one model has answered one question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeciderStanding {
    /// Its name, `Jev 1.14`, and its pinned model id.
    pub name: String,
    pub decider: String,
    /// What it does for the question: "Answers it" or "Measured beside it".
    pub role: String,
    pub judgments: i64,
    /// Of them, those whose pick proved right or wrong, and those whose outcome can no longer be known.
    pub outcomes: i64,
    pub unknown: i64,
    /// 0 is perfect and 0.25 a coin's; none without an outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brier: Option<f64>,
    /// 1 separates right from wrong perfectly, 0.5 not at all; none until one of each.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auc: Option<f64>,
    pub cost_usd: f64,
}

/// Who a channel signed in as (ADR-0089): the owner, or a mentor with a code he had made for them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Who {
    #[default]
    Owner,
    Mentor {
        mentor_id: String,
        name: String,
        key_id: String,
    },
}

model::json_tagged!(Who, "kind");

/// The columns of the boards whose cards arrive one by one (subscriptions, feedback, playbooks, agents, skills, tool
/// sources), each board's in the order they stand: a card stands in the column its record names (`SubscriptionLine.group`,
/// `Feedback.kind`, `Playbook.group`, `Agent.group`, `ListedSkill.group`, `ToolSource.capability`). An icon is a name of the
/// channels' shared icon set (the web app's `Icon.vue`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Boards {
    pub subscriptions: Vec<SubscriptionColumn>,
    /// Every kind of subscription, in the order the kinds are offered to narrow the board by.
    pub subscription_kinds: Vec<SubscriptionKindSaid>,
    pub feedback: Vec<FeedbackColumn>,
    /// One column per kind of work the playbooks are for, `general` last.
    pub playbooks: Vec<GroupColumn>,
    /// One column per kind of work the agents are for, `general` last.
    pub agents: Vec<GroupColumn>,
    /// One column per kind of work the skills are for, `general` last.
    pub skills: Vec<GroupColumn>,
    /// One column per capability the sources give, in the order the sources come.
    pub sources: Vec<GroupColumn>,
    pub rev: i64,
}

/// A column of the Subscriptions board: what happens to what stands in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionColumn {
    pub key: SubscriptionGroup,
    pub label: String,
    pub icon: String,
    /// What puts a subscription here, said while it holds none.
    pub empty: String,
}

/// A kind of subscription as the owner reads it: `plural` names the kind among the others ("Check-backs"), `noun` one
/// line of it ("Check-back"), `says` what one does ("Checks back on a chain"). `starts_hidden`: the board leaves it out
/// until the owner asks for it (the automations Remit runs on its own).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionKindSaid {
    pub kind: SubscriptionKind,
    pub plural: String,
    pub noun: String,
    pub says: String,
    pub icon: String,
    pub starts_hidden: bool,
}

/// A column of the Feedback board: one kind of feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackColumn {
    pub key: FeedbackKind,
    pub label: String,
    pub icon: String,
    /// What an agent tells to land here, said while it holds none.
    pub empty: String,
}

/// A column of a Crew board: a group its cards' records name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupColumn {
    pub key: String,
    pub label: String,
    pub icon: String,
    /// What stands here, said while the search leaves it none.
    pub empty: String,
}

/// Everything the app shows, as it is now; live changes reach a channel through the Notifier after it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Who the channel signed in as: what else it holds is what they see.
    pub who: Who,
    /// Newest first, archived ones left out (`archived_chains` pages them).
    pub chains: Vec<ChainRead>,
    /// The open items.
    pub inbox: Vec<InboxItem>,
    /// Where every chain stands for the owner.
    pub board: Board,
    pub standings: Vec<ChainStanding>,
    pub playbooks: Vec<Playbook>,
    /// The library: the agents the owner defined, and the skills they can hold.
    pub agents: Vec<Agent>,
    pub skills: Vec<ListedSkill>,
    /// Remit's own tools for reading the outside world, each with its connections.
    pub sources: Vec<ToolSource>,
    /// The catalog a playbook's and a chain's capabilities are named from.
    pub capabilities: Vec<CapabilityView>,
    pub health: Health,
    pub workload: Workload,
    pub signals: SignalBoard,
    /// The columns of the boards whose cards arrive one by one (boards.rs).
    pub boards: Boards,
    /// The analyst's newest, newest first.
    pub insights: Vec<Decision>,
    /// The owner's Assistant; none in a mentor's snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant: Option<AssistantView>,
    pub assistant_feed: AssistantFeed,
    /// The newest events with what their subscriptions did, newest first, without what arrived.
    pub events: Vec<EventRecord>,
    /// The newest first, without their keys.
    pub event_signals: Vec<EventSignal>,
    /// Every sensor, by id.
    pub sensors: Vec<Sensor>,
    /// The newest first, without their values.
    pub client_keys: Vec<ClientKey>,
    /// Every plugin source, disabled or not, the bundled first.
    pub extensions: Vec<Extension>,
    /// What agents told the organisation, the newest first.
    pub feedback: Vec<Feedback>,
    /// Every objective with where each key result stands.
    pub objectives: Vec<ObjectiveView>,
    /// Every project, archived ones too, by name (ADR-0108).
    pub projects: Vec<Project>,
    /// Oldest first.
    pub employees: Vec<EmployeeRead>,
    /// The pool, removed ones included, oldest added first.
    pub mentors: Vec<Mentor>,
    /// Where each mentor stands, in the pool's order.
    pub mentor_standings: Vec<MentorStanding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorGone {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSignalGone {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientKeyGone {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionGone {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientKeyValue {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSignalKey {
    pub key: String,
}

/// One event with its firings. Its id is its address, `event:<signal>#<rev>`: the page of the event and the chain it
/// started name it. `event` is what arrived; the lists leave it out (`event_get` gives it), since a body can be large.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRecord {
    pub id: String,
    pub signal: String,
    pub rev: i64,
    pub at: Time,
    pub via: EventVia,
    /// What the writer sent, whole; none in a list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<InboundEvent>,
    /// The subscriptions' acts on it, oldest first, each with what became of it in words. None says that no
    /// subscription held for it.
    pub firings: Vec<FiringLine>,
}

/// What the owner viewed (ADR-0079): a home (a chain, the Assistant's) up to its event `event_id`, or the Assistant's
/// feed as it shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum SeenSubject {
    Home { home: String, event_id: i64 },
    AssistantFeed,
}

model::json_tagged!(SeenSubject, "kind");

/// One decision of the Assistant's for the owner, whether it answered the item for him (so he may overturn it) or left
/// it to him, a second judge's dissent from its answer, and his overturn of it when he made one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedLine {
    pub decision: Decision,
    pub answered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dissent: Option<Decision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overturn: Option<Decision>,
    /// While its answer waits out the overturn window: when it takes effect (ADR-0101).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub takes_effect_at: Option<Time>,
}

/// The Assistant's decisions for the owner, newest first, and how many came since he last looked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssistantFeed {
    pub lines: Vec<FeedLine>,
    pub unseen: i64,
}

/// Everything that draws the Assistant: its record, its card as it is now, where it stands, and the owner's unsent words
/// to it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssistantView {
    pub assistant: Assistant,
    pub card: Card,
    pub agent: AgentStanding,
    pub drafts: Vec<Draft>,
}

/// A `work` chain the owner asks for: what he wants, in his words; `project` the id of the project his board is narrowed
/// to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewChain {
    pub intent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// What streamed in a home and was not sent was let go, as the session fell too far behind: what shows the home reads it
/// again (ADR-0095).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeReset {
    pub home: String,
}

/// The server applied the session's `homes_shown` of `seq`, which named this home: what streams in the home after this
/// mark on its stream is pushed as it happens, and what came before it may predate the session showing the home again
/// (ADR-0095 decision 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeShown {
    pub home: String,
    pub seq: u64,
}

/// What a page's core counted on the link over a minute (ADR-0095 decision 12), kept in the server's telemetry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkReport {
    /// The page's own random name: with `minute`, it names the report, so one sent again is kept once.
    pub page: String,
    /// How many reports the page sent before this one.
    pub minute: i64,
    /// The core's clock when it sent the report, in milliseconds since 1970: the server moves the delays onto its own
    /// clock by it.
    pub sent_at: i64,
    pub frames: Vec<FrameTally>,
    pub repairs: Vec<RepairTally>,
    /// None when no event was pushed in the minute.
    pub delays: Option<Delays>,
    /// What failed in the page over the minute (ADR-0119), at most `PAGE_FAULTS_MAX` of them.
    #[serde(default)]
    pub faults: Vec<PageFault>,
}

/// The most faults one link report carries: past it, a minute's faults are counted on the ones it holds.
pub const PAGE_FAULTS_MAX: usize = 32;
/// The most characters of a fault's message and of where it happened that are kept.
pub const PAGE_FAULT_TEXT_MAX: usize = 500;

/// text as a fault keeps it: its first `PAGE_FAULT_TEXT_MAX` characters.
pub fn fault_text(text: &str) -> String {
    text.chars().take(PAGE_FAULT_TEXT_MAX).collect()
}

/// One kind of failure a page met over a minute, counted: the same message at the same place in the same chain is one
/// fault however often it happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageFault {
    pub kind: FaultKind,
    /// What failed, as the browser or the failed call said it, cut at `PAGE_FAULT_TEXT_MAX`.
    pub message: String,
    /// Where: the script and line of an uncaught error, the method of a failed call; empty when the browser said none.
    pub at: String,
    /// The chain the tab showed; empty when it showed none.
    pub chain: String,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FaultKind {
    /// An error the page's code threw and nothing caught (`window.onerror`).
    Uncaught,
    /// A promise rejected with nothing to handle it (`unhandledrejection`).
    Rejection,
    /// A call to the server that failed for a reason other than the link being down.
    Call,
}

/// The answers or pushes of one name the core decoded in a minute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameTally {
    pub kind: FrameKind,
    /// The call's method or the push's name.
    pub name: String,
    pub count: i64,
    pub bytes: i64,
    pub decode_us: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameKind {
    Answer,
    Push,
}

/// How often a home was read again in a minute, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairTally {
    pub reason: RepairReason,
    /// Empty when what failed names no home.
    pub home: String,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepairReason {
    /// The server let go of the home's stream (`home_reset`).
    Reset,
    /// A push this core could not decode was dropped.
    Decode,
}

/// How long after events happened the core held them, on the core's clock, over a minute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delays {
    pub count: i64,
    pub p50_ms: i64,
    pub p95_ms: i64,
    pub max_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainGone {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftGone {
    pub id: String,
    pub home: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventGone {
    pub id: i64,
    pub home: String,
    pub card_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookGone {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentGone {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillGone {
    pub name: String,
}

/// An employee as a channel reads it: the record, how the server words it (`Employee::shown` and its tag), and the face
/// the server derives (ADR-0107).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployeeRead {
    pub employee: Employee,
    pub shown: String,
    pub tag: String,
    pub face: Face,
}

/// An employee's face, which the server derives and a channel only draws (ADR-0093): the shape and hue of its lineage
/// (the template it was hired from), so siblings read as one family wherever they show, and an accessory of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Face {
    pub shape: FaceShape,
    /// Degrees on the colour wheel, 0 to 359; a channel picks lightness and chroma from its theme.
    pub hue: i64,
    pub accessory: FaceAccessory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceShape {
    Circle,
    Square,
    Hexagon,
    Triangle,
    Diamond,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceAccessory {
    Antenna,
    Halo,
    Visor,
    Spark,
    Leaf,
    Bow,
}

/// What a person teaches by marking an agent's reply (ADR-0093).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnMark {
    Helpful,
    NotHelpful,
    /// The reply was wrong; the words given are the correct reply.
    Correct,
}

/// Where a mentor stands for the owner: whether they are in the app now, when they were last active, those as their
/// presence words them, the keys they work from, oldest made first, and their helps still open.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MentorStanding {
    pub mentor: String,
    pub in_app_now: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active: Option<Time>,
    pub presence: String,
    pub keys: Vec<WorkerKeyLine>,
    pub open_helps: usize,
    pub rev: i64,
}

/// One code a mentor works from: its id (never its value), the owner's note of what it is for, whether a session of it is
/// open now and that in the owner's words (`state`: "open" or "closed"), when it was made and when it last opened a
/// session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerKeyLine {
    pub id: String,
    pub note: String,
    pub open: bool,
    pub state: String,
    pub created_at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<Time>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerKeyValue {
    pub key: String,
}

/// What one agent's memory holds, as its screen shows it (ADR-0117): what it recalls now, then what it once held and
/// stopped recalling (superseded or retracted, with when and why), and the teachings its memory learned from, the latest
/// first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MemoryBook {
    pub memories: Vec<Memory>,
    pub teachings: Vec<Teaching>,
}

/// How a lineage (the template its employees were hired from) stands in one kind of work: its Bradley–Terry rating on the
/// Elo scale (1500 is even), from duels judged, won of them. Few duels make a noisy rating, so it is always read with its
/// count.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageRating {
    pub template: String,
    pub work: String,
    pub rating: i64,
    pub duels: u32,
    pub won: u32,
}

/// What the outside said of one chain's accepted work.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutsideResult {
    /// Every outside signal on the chain was in its favour.
    #[default]
    Held,
    /// Landed, then the owner took it back: he found what reviewer and QC had approved.
    Corrected,
    /// Taken back by the forge's pipeline, a guardrail or a revert it conflicted with, found not met by the goal judge,
    /// or its reading broke.
    Failed,
}

/// One chain's result for the lineages whose accepted work it holds, why in words, and when the outside said it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutsideEvidence {
    pub chain: String,
    pub result: OutsideResult,
    pub why: String,
    pub at: Time,
}

/// How a lineage stands in one kind of work: its score from the outside's results (held, corrected and failed, each with
/// its evidence, newest first), whether those are enough to stand on (settled), and its duel rating beside them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LineageStanding {
    pub template: String,
    pub work: String,
    /// The share of its chains the outside upheld, 0 to 100, a corrected one as half; 50 with none.
    pub score: i64,
    pub held: u32,
    pub corrected: u32,
    pub failed: u32,
    /// The outside decided on at least `MIN_OUTSIDE` of its chains.
    pub settled: bool,
    /// Its duel rating, which only breaks ties; 0 duels before it fought one.
    pub duel: LineageRating,
    pub evidence: Vec<OutsideEvidence>,
}

/// Where lineages stand in one kind of work: each lineage's standing from the outside with its duel rating, best first;
/// why the kind of work is unsettled, so a hire there runs on two lineages (none once settled); the judge's agreement with
/// the owner over its newest picks; and why duels there are paused (none while they go on).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkDuels {
    pub work: String,
    pub standings: Vec<LineageStanding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsettled: Option<String>,
    pub judged: u32,
    pub overturned: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<String>,
}

/// What the way in reads at `GET /auth/config` before anyone has come in: the ways in this server offers beside the
/// owner key (ADR-0105).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInConfig {
    /// The Firebase project the owner signs in with Google through; none: Google sign-in is off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<FirebaseWeb>,
}

/// What the page sets Firebase up with. None of it is a secret: Firebase shows it to every page that signs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirebaseWeb {
    pub api_key: String,
    pub auth_domain: String,
    pub project_id: String,
}

/// What the page posts to `/auth/google`: the Firebase ID token Google's sign-in handed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSignIn {
    pub id_token: String,
}

/// The answer to a Google sign-in that lets the owner in: the credential every session of this browser carries, and the
/// account it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSignedIn {
    pub credential: String,
    pub email: String,
}

link::contract! {
    capability: "app";
    reads {
        app_snapshot() -> Snapshot, why = "reads the snapshot a channel starts from";
        /// A provider's probe may take two minutes (`orch` `PROBE_TIMEOUT`): the answer waits longer than that.
        health_check() -> Health, why = "probes the providers and reads their health: a replay probes again and changes no record", timeout_ms = 150000;
        chain_get(id: String) -> ChainView, why = "reads one chain's view";
        chain_events(id: String) -> EventPage, why = "reads a chain's events";
        /// The homes (a chain's id, the Assistant's) the session shows: the pushes of a home that stream (its events,
        /// what its agents show of the moment) reach a session that named it, and no other (ADR-0095). A session that
        /// never names any is pushed them all. Each home named is pushed `home_shown` with `seq` on its stream once the
        /// server applied it: a channel reads a home it newly names after that mark, and drops what streamed there before.
        homes_shown(homes: Vec<String>, seq: u64) -> Ack, why = "says which homes this session shows: it changes no record, and a replay sets the same";
        /// `after` is the `next` of the page shown (empty: the newest).
        chains_archived(after: String) -> ChainPage, why = "reads a page of archived chains";
        event_get(id: String) -> EventRecord, why = "reads one event's view";
        signal_get(address: String) -> SignalDetail, why = "reads the readings and firings of one signal or subscription";
        /// `pattern` is a GLOB over keys (`fact:*`, `*`), newest first.
        signal_rows(pattern: String, limit: i64) -> Vec<SignalRow>, why = "reads the rows of the Signals table";
        skill_file(name: String, path: String) -> SkillFileText, why = "reads one file of a skill";
        changes_get(kind: ChangeKind, name: String) -> Vec<Change>, why = "reads the changes of one record";
        /// With `before` (an event's id; 0: none), the newest of those older than it.
        card_events(card_id: String, before: i64) -> EventPage, why = "reads a card's events";
        card_context(card_id: String) -> CardContext, why = "reads what a card's session holds";
        card_telemetry(card_id: String) -> CardTelemetry, why = "reads what a card's agent did, as its sessions exported it";
        /// Live from the agent's process while the task runs, as kept at its end after.
        card_task_output(event_id: i64) -> TaskOutput, why = "reads what a background task of a card wrote";
        landing_diff(landing_id: String) -> LandingDiff, why = "reads a landing's diff";
        /// Every repository with a landing open or ended in the last day, or a release, each as `repo_get` reads it.
        repo_views() -> Vec<RepoView>, why = "reads every repository's queue and releases";
        event_signal_key(id: String) -> EventSignalKey, why = "reads an event signal's door key for the owner to copy (logged, never stored)";
        client_key(id: String) -> ClientKeyValue, why = "reads a client key's value for the owner to copy (logged, never stored)";
        worker_key(id: String) -> WorkerKeyValue, why = "reads a person's code for the owner to pass on (logged, never stored)";
        /// Fails when the window does not fit twice in the range.
        outcomes_get(last: OutcomeRange, window: OutcomeWindow) -> Outcomes, why = "reads the outcomes report";
        judgments_get() -> Judgments, why = "reads how each decider stands on each question";
        employee_memory(id: String) -> MemoryBook, why = "reads what one employee remembers";
        /// What the agent of a card remembers where it works: its employee's memory, or the Assistant's. Fails for a card
        /// with none (`AgentStanding::remembers`).
        card_memory(card_id: String) -> MemoryBook, why = "reads what one agent remembers";
        /// What a mentor taught, as their own memory keeps it.
        mentor_memory(id: String) -> MemoryBook, why = "reads what one mentor taught";
        duel_standings() -> Vec<WorkDuels>, why = "reads where duels between lineages stand in every kind of work";
        /// Lays out the chains whose seats were not laid out since the server started, then answers.
        at_work() -> AtWork, why = "reads every seat at work now";
        employee_runs(id: String) -> EmployeeRuns, why = "reads one employee's runs across chains";
        /// An attachment a message or a draft holds, by the id it was put with.
        attachment_get(id: String) -> Attachment, why = "reads one attachment's bytes";
        /// `repo` is the repository's catalog id; `repo_updated` brings each change of the view.
        repo_get(repo: String) -> RepoView, why = "reads one repository's queue and releases";
    }
    // A change of one chain says `order = id`: the owner's changes of a chain run in the order he made them, whatever their
    // method (a resume sent right after a pause is done after it), as orch's `act_on_chain` does them.
    changes {
        /// The owner pressing Add account: the answer is the login item that holds its link and takes its code.
        account_add() -> InboxItem;
        /// No agent is placed on it, its login is signed out and its config dir removed. The default account stays.
        account_remove(id: String) -> Ack;
        chain_create(input: NewChain) -> Chain;
        chain_pause(id: String, why: String) -> Ack, order = id;
        chain_resume(id: String, why: String) -> Ack, order = id;
        /// Wakes a waiting chain's Chair now instead of at its check-back time.
        chain_check_back_now(id: String) -> Ack, order = id;
        chain_finish(id: String, summary: String) -> Ack, order = id;
        /// Only a finished chain: archiving hides it, deleting removes it and its worktrees for good.
        chain_archive(id: String, why: String) -> Ack, order = id;
        chain_unarchive(id: String, why: String) -> Ack, order = id;
        /// Removes the chain's worktrees and directory before it answers, which takes minutes on a large repository.
        chain_delete(id: String) -> Ack, order = id, timeout_ms = 300000;
        /// Sends the owner's message draft to its agent as `delivery` says, and ends the draft with the send.
        card_prompt(draft_id: String, delivery: Delivery) -> EventRead;
        /// Takes back a queued message no agent has read yet: its words return to the card's message draft, answered.
        /// Refused once it is read or steered, and while that draft holds unsent words.
        card_prompt_withdraw(event_id: i64) -> Draft;
        /// The owner or a mentor of the card marks the agent's reply `event_id` (a message event): helpful, not helpful
        /// (`words`: why, may be empty) or correct (`words`: the correct reply). It is recorded as the teaching
        /// `fact:teach.reply`, which the agent's memory learns from and the coach reads. A mentor marks a reply on a card
        /// they help.
        turn_teach(event_id: i64, mark: TurnMark, words: String) -> Ack;
        card_cancel(card_id: String) -> Ack;
        card_restart(card_id: String) -> Ack;
        /// Ends an agent's session for good; `why` (may be empty) reaches its Chair, which is told not to start its work
        /// again on its own, and the agent's stream.
        card_stop(card_id: String, why: String) -> Card;
        /// Says why a stopped agent was stopped, after the stop: its Chair is told and its stream says it.
        card_stop_explain(card_id: String, why: String) -> Ack;
        /// Ends the background task of event `event_id` (a background shell, a Monitor); the agent goes on and is told.
        card_task_stop(event_id: i64) -> Ack;
        /// The owner picks `picked` (option ids) in the choice `part` of the block event `event_id` (ADR-0091): the block
        /// shows his pick, and its card is told in one queued message by him, which replaces one from this block it has
        /// not read yet. Refused for a part that is no choice of the block, an id that is no option of it, and a card that
        /// takes no message.
        block_pick(event_id: i64, part: String, picked: Vec<String>) -> EventRead;
        /// Sends the owner's message draft to the Assistant as `delivery` says, with where he is; ends the draft with the
        /// send.
        assistant_prompt(draft_id: String, r#where: Where, delivery: Delivery) -> EventRead;
        /// Keeps the owner's words as he types, made on the draft at `base_rev` (0: one the page starts); answers the
        /// draft as stored. A save made on a copy another page has saved over since, or a page's first save in a box that
        /// holds a draft already, saves nothing: `CONFLICT`, with `FailureDetail::draft_conflict` holding the draft as it is.
        draft_save(draft: DraftInput, base_rev: i64) -> Draft;
        draft_delete(id: String) -> Ack;
        /// Starts a routine's chain now instead of at its time; one run at a time.
        routine_run(id: String) -> Chain;
        routine_pause(id: String) -> Ack;
        /// A routine that repeats waits for its next time: the times it missed are not run.
        routine_resume(id: String) -> Ack;
        event_signal_pause(id: String) -> Ack;
        event_signal_resume(id: String) -> Ack;
        client_key_pause(id: String) -> Ack;
        client_key_resume(id: String) -> Ack;
        extension_disable(id: String) -> Ack;
        extension_enable(id: String) -> Ack;
        /// The owner opening a piece of feedback: it stops waiting to be read.
        feedback_read(id: String) -> Feedback;
        /// Starts the analyst when none is open, else asks its Chair to run now; one at a time.
        analyst_run() -> Chain;
        inbox_answer(item_id: String, option_id: String, text: String) -> Ack;
        /// The owner overturns an answer the Assistant gave (`word`: what he decides instead) or a duel judge's pick.
        overturn(decision_id: String, word: String) -> Ack;
        /// The owner viewed `subject`: its mark moves forward to it, never back. Made only for a real view (the page
        /// visible and focused, the newest line in sight), ADR-0079.
        seen_mark(subject: SeenSubject) -> Ack;
        /// Stops a workflow run for good, saying why: its cards at work stop, its waits end, its question is withdrawn.
        workflow_stop(workflow_id: String, why: String) -> Ack;
        /// A mentor taking on a help offered to them: the card's agent and the Chair are told. Only a mentor calls it.
        help_accept(help_id: String) -> Help;
        /// A mentor saying no to a help offered to them; `why` reaches the Chair. Only a mentor calls it.
        help_decline(help_id: String, why: String) -> Help;
    }
    idempotent {
        /// What the page's core counted on the link in a minute (ADR-0095 decision 12).
        link_report(report: LinkReport) -> Ack, why = "a report sent again is kept once, by its page and minute";
        /// Keeps an attachment's bytes (base64, at most `ATTACHMENT_MAX_BYTES`) for a draft of `home` to hold by `id`, which the
        /// channel makes.
        attachment_put(id: String, home: String, attachment: Attachment) -> Ack, why = "an id put again keeps the bytes it was first put with";
    }
    pushes {
        chain_updated(chain: ChainRead);
        chain_deleted(gone: ChainGone);
        card_updated(card: Card);
        card_shown(shown: CardShown);
        edge_added(edge: Edge);
        decision_added(decision: DecisionRead);
        /// A landing of the chain as last stored, or with the release that carried it as that release moved.
        landing_updated(landing: LandingRead);
        /// A repository's queue and releases, whole, whenever a landing or a release of it is written. The owner's alone.
        repo_updated(view: RepoView);
        /// A workflow run with its steps, as last stored.
        workflow_updated(workflow: Workflow);
        /// A new insight of the analyst's; its chain also gets it as `decision_added`.
        insight_added(insight: Decision);
        /// The Assistant's feed, whole, whenever it moves.
        assistant_feed_updated(feed: AssistantFeed);
        draft_updated(draft: Draft);
        draft_deleted(gone: DraftGone);
        event_added(event: EventRead);
        event_updated(event: EventRead);
        event_deleted(gone: EventGone);
        event_delta(delta: EventDelta);
        health_updated(health: Health);
        workload_updated(workload: Workload);
        signal_board_updated(board: SignalBoard);
        /// The columns of the boards whose cards arrive one by one, whole, whenever they move.
        boards_updated(boards: Boards);
        /// An event as last stored, with its firings: a new event comes with none.
        event_read(event: EventRecord);
        event_signal_updated(event_signal: EventSignal);
        event_signal_deleted(gone: EventSignalGone);
        sensor_updated(sensor: Sensor);
        sensor_deleted(gone: SensorGone);
        client_key_updated(key: ClientKey);
        client_key_deleted(gone: ClientKeyGone);
        extension_updated(extension: Extension);
        extension_deleted(gone: ExtensionGone);
        feedback_updated(feedback: Feedback);
        objective_updated(view: ObjectiveView);
        /// A project as last stored (ADR-0108). The owner's alone.
        project_updated(project: Project);
        employee_updated(employee: EmployeeRead);
        mentor_updated(mentor: Mentor);
        help_updated(help: Help);
        inbox_updated(item: InboxItem);
        playbook_updated(playbook: Playbook);
        playbook_deleted(gone: PlaybookGone);
        agent_updated(agent: Agent);
        agent_deleted(gone: AgentGone);
        skill_updated(skill: ListedSkill);
        skill_deleted(gone: SkillGone);
        source_updated(source: ToolSource);
        board_updated(board: Board);
        chain_standing_updated(standing: ChainStanding);
        agent_standing_updated(agent: AgentStanding);
        /// The at-work view, whole, whenever it moves. The owner's alone.
        at_work_updated(at_work: AtWork);
        mentor_standing_updated(mentor: MentorStanding);
        verification_updated(verification: Verification);
        /// The coach's retrospective of a chain's finish.
        retrospective_added(retrospective: RetrospectiveRead);
        home_reset(reset: HomeReset);
        home_shown(mark: HomeShown);
    }
}

impl Push {
    /// The home whose stream carries this push (ADR-0072 decision 3): the chain or the Assistant's home its record lives in,
    /// so a home's pushes keep their order; none: the feed, for every record that lives in no one home.
    pub fn home(&self) -> Option<&str> {
        Some(match self {
            Self::chain_updated { chain } => &chain.chain.id,
            Self::card_updated { card } => &card.home,
            Self::card_shown { shown } => &shown.home,
            Self::agent_standing_updated { agent } => &agent.home,
            Self::edge_added { edge } => &edge.chain_id,
            Self::decision_added { decision } => &decision.decision.chain_id,
            Self::landing_updated { landing } => &landing.landing.chain_id,
            Self::verification_updated { verification } => &verification.chain_id,
            Self::retrospective_added { retrospective } => &retrospective.retrospective.chain_id,
            Self::workflow_updated { workflow } => &workflow.home,
            Self::help_updated { help } => &help.chain,
            Self::draft_updated { draft } => &draft.home,
            Self::draft_deleted { gone } => &gone.home,
            Self::event_added { event } | Self::event_updated { event } => &event.event.home,
            Self::event_deleted { gone } => &gone.home,
            Self::event_delta { delta } => &delta.home,
            Self::home_reset { reset } => &reset.home,
            Self::home_shown { mark } => &mark.home,
            _ => return None,
        })
    }

    /// The home of a push that streams while its agents work, which only the sessions showing that home are pushed
    /// (ADR-0095 decision 7); none for every other push (the board, the inbox, where each chain stands).
    pub fn streamed_home(&self) -> Option<&str> {
        match self {
            Self::event_added { event } | Self::event_updated { event } => Some(&event.event.home),
            Self::event_deleted { gone } => Some(&gone.home),
            Self::event_delta { delta } => Some(&delta.home),
            Self::card_shown { shown } => Some(&shown.home),
            Self::home_reset { reset } => Some(&reset.home),
            Self::home_shown { mark } => Some(&mark.home),
            _ => None,
        }
    }
}
