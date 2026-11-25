//! The signal backbone (docs/adr/0014): everything that wakes a chain, starts one or places an event is the firing of a
//! subscription to a signal. These are its stored records; the rules that judge them are pure and live in the signal
//! crate.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, ChainKind, ChainOrigin, Duration, EVENT_KEY_PREFIX, ForeignJson, Invalid, Time};

/// Names a signal as "<source>:<name>": `event:gitlab`, `chain:<id>.sensor.error-rate`, `chain:<id>.landing.<repo>`. The
/// source is the adapter that turns what it receives into readings; everything after the reading is shared. A newtype,
/// it is written as its text.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SignalKey(pub String);

impl SignalKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The kind of the signal, given by its family.
    pub fn kind(&self) -> SignalKind {
        if stream_key_prefixes().iter().any(|p| self.0.starts_with(p)) {
            SignalKind::Stream
        } else {
            SignalKind::State
        }
    }
}

impl fmt::Display for SignalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a subscription listens to: a filter over keys, in the syntax of SQLite's GLOB (docs/adr/0049): `*` matches any
/// run of characters, dots and colons included, `?` one character, `[abc]`, `[a-z]` and `[^a-z]` one character of a
/// class. A pattern with none of them is one exact key. The store filters with GLOB and the router with `matches`: one
/// syntax.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SignalPattern(pub String);

impl SignalPattern {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether key is one the pattern lets through.
    pub fn matches(&self, key: &SignalKey) -> bool {
        glob_match(&self.0, &key.0)
    }

    /// The one key the pattern names, when it has no wildcard.
    pub fn exact(&self) -> Option<SignalKey> {
        (!self.0.contains(['*', '?', '['])).then(|| SignalKey(self.0.clone()))
    }
}

impl fmt::Display for SignalPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Begins the key of every fact Remit records of its own state: fact:<noun>.<verb> (ADR-0035).
pub const FACT_KEY_PREFIX: &str = "fact:";

text_enum! {
    /// What a signal's readings are (docs/adr/0064): a stream's are occurrences, each its own (an event, a fact); a
    /// state's each replace the one before as its current value (a landing's state, what a sensor reads). The kind
    /// sets where a subscription may start and what the log keeps of it.
    pub enum SignalKind {
        Stream = "stream",
        State = "state",
    }
}

/// Begin the keys of the signals that are streams; every other signal is a state. The store reads them to forget by
/// kind, so the families are named here once.
pub fn stream_key_prefixes() -> [&'static str; 2] {
    [EVENT_KEY_PREFIX, FACT_KEY_PREFIX]
}

text_enum! {
    /// Where a new subscription starts judging the log (docs/adr/0064, Start position and retention).
    #[derive(Default)]
    pub enum StartFrom {
        /// The readings stored after it is made.
        #[default]
        Now = "now",
        /// The readings from place `seq` in the log on.
        Seq = "seq",
        /// The readings stored from `at` on.
        Time = "time",
        /// One state's newest reading, then each change.
        Current = "current",
    }
}

/// Where a subscription that is made starts judging, which its maker names: it is applied once, when the subscription is
/// stored, and is not kept (the subscription's `seen_seq` is where it stands).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Start {
    pub from: StartFrom,
    /// For `StartFrom::Seq`: the first place in the log judged.
    pub seq: i64,
    /// For `StartFrom::Time`: the first time judged.
    pub at: Option<Time>,
}

impl Start {
    /// Whether sub can start here: a schedule judges no reading and starts now; current names one exact key of a state,
    /// whose newest reading it judges, so a pattern or a stream is refused; a place in the log is one or later; a time is
    /// named. That the place and the time are not past the log's head and now, the caller checks against the store.
    pub fn check(&self, sub: &Subscription) -> Result<(), Invalid> {
        if sub.schedule.is_some() {
            if self.from != StartFrom::Now {
                return Err(Invalid::new(format!(
                    "a schedule acts at its own instants and judges no reading: it starts now, not from {:?}",
                    self.from.as_str()
                )));
            }
            return Ok(());
        }
        match &self.from {
            StartFrom::Now => {}
            StartFrom::Seq if self.seq < 1 => {
                return Err(Invalid::new(
                    "a start from a place in the log names that place, 1 or later",
                ));
            }
            StartFrom::Seq => {}
            StartFrom::Time if self.at.is_none() => return Err(Invalid::new("a start from a time names that time")),
            StartFrom::Time => {}
            StartFrom::Current => {
                let Some(key) = sub.r#match.exact() else {
                    return Err(Invalid::new(format!(
                        "current judges one signal's newest reading, and {} is a pattern of many: start it from now, a place in the log or a time",
                        sub.r#match
                    )));
                };
                if key.kind() != SignalKind::State {
                    return Err(Invalid::new(format!(
                        "current judges a state's newest reading, and {key} is a stream of occurrences: start it from now, a place in the log or a time"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// GLOB: the whole of s against the pattern, case sensitive. A `[` with no closing `]` matches nothing, as in SQLite.
fn glob_match(p: &str, s: &str) -> bool {
    let mut p = p;
    let mut s = s;
    while let Some(first) = p.chars().next() {
        match first {
            '*' => {
                let rest = p.trim_start_matches('*');
                if rest.is_empty() {
                    return true;
                }
                return s
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain([s.len()])
                    .any(|i| glob_match(rest, &s[i..]));
            }
            '?' => {
                let Some(c) = s.chars().next() else { return false };
                p = &p[1..];
                s = &s[c.len_utf8()..];
            }
            '[' => {
                let Some(c) = s.chars().next() else { return false };
                let Some((true, rest)) = glob_class(&p[1..], c) else {
                    return false;
                };
                p = rest;
                s = &s[c.len_utf8()..];
            }
            _ => {
                let Some(rest) = s.strip_prefix(first) else {
                    return false;
                };
                p = &p[first.len_utf8()..];
                s = rest;
            }
        }
    }
    s.is_empty()
}

/// Reads a character class up to its `]` (the text after the `[`): whether r is in it and what follows it; none when the
/// class is not closed. A `]` first in the class is a member, and `-` between two characters is a range.
fn glob_class(p: &str, r: char) -> Option<(bool, &str)> {
    let (negate, mut p) = match p.strip_prefix('^') {
        Some(rest) => (true, rest),
        None => (false, p),
    };
    let mut first = true;
    let mut is_in = false;
    while let Some(c) = p.chars().next() {
        p = &p[c.len_utf8()..];
        if c == ']' && !first {
            return Some((is_in != negate, p));
        }
        first = false;
        let mut ahead = p.chars();
        if let (Some('-'), Some(hi)) = (ahead.next(), ahead.next())
            && hi != ']'
        {
            is_in |= (c..=hi).contains(&r);
            p = ahead.as_str();
            continue;
        }
        is_in |= r == c;
    }
    None
}

/// One value of a signal at one revision.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Reading {
    pub key: SignalKey,
    /// Counts the key's readings from 1. The write that stores a reading gives it, so two readings of a key never share
    /// one.
    pub rev: i64,
    /// The reading's place in the one log of every stored reading (docs/adr/0049): strictly increasing across keys, so a
    /// subscription's cursor is a seq. Zero before it is stored.
    pub seq: i64,
    /// When the log took it (docs/adr/0064): never a time its writer names, so a source cannot move a window; a
    /// producer's own time is in `value`.
    pub at: Time,
    pub cause: Cause,
    /// What the reading is about within its key (a pull request, a chain, a customer); empty: the key itself.
    pub subject: String,
    /// The part of Remit that wrote it, set by the code that wrote it and never read from a payload.
    pub writer: Writer,
    /// What the signal said, JSON the subscriptions' conditions read as `value`. An event signal's is an EventValue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<ForeignJson>,
    /// What makes a writer's retry the same reading: unique within its key when set (the store's index holds the readings
    /// that have one). An event signal's is the signal's name and the writer's own id for the event, or else a hash of its
    /// body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedupe: Option<String>,
}

text_enum! {
    /// The kind of writer a reading came from (docs/adr/0064).
    #[derive(Default)]
    pub enum WriterKind {
        /// A sensor Remit runs: a poll, a script, a plugin's sensor kind.
        #[default]
        Sensor = "sensor",
        /// A post to an event signal's HTTP door.
        Door = "door",
        /// `remit emit`, through the door.
        Cli = "cli",
        /// An agent's emit_event tool.
        Agent = "agent",
        /// A plugin's processor, emitting what it answered.
        Plugin = "plugin",
        /// A change of Remit's own state (ADR-0035).
        Fact = "fact",
        /// A chain's record: its landings, its live mark.
        Record = "record",
        /// The time a subscription waits for.
        Schedule = "schedule",
    }
}

/// Who wrote a reading: its kind and, for a sensor the sensor's id, for a plugin the plugin's, for a fact its noun, for a
/// chain's record the chain, for a schedule the subscription's, for an agent its card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Writer {
    pub kind: WriterKind,
    pub id: String,
}

/// What made a reading: who acted and, when the reading stems from one, the chain. A subscription of a chain does not
/// hear readings its own chain caused. `chain_id` is set only for a reading an agent of that chain caused through a tool;
/// what the forge's pipeline says of a landed commit is about the chain and is not its doing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Cause {
    pub actor: Actor,
    pub chain_id: String,
}

text_enum! {
    /// Who a subscription belongs to.
    #[derive(Default)]
    pub enum OwnerKind {
        /// A chain made it, to wake itself; it ends with the chain.
        #[default]
        Chain = "chain",
        /// Remit or the owner: a routine.
        Remit = "remit",
    }
}

/// Who a subscription belongs to: for a chain its id, for an event signal its name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SubscriptionOwner {
    pub kind: OwnerKind,
    pub id: String,
}

/// What a firing does: one of Remit's own processors, or a plugin's. It reads and writes as its text. Not a `text_enum!`:
/// a plugin's processors are an open set, named by the plugins the owner applies (docs/adr/0049), so a contract carries it
/// as text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum ActionKind {
    /// Resume the owning chain's Chair, with `why`.
    #[default]
    Wake,
    /// Queue a chain from `start`.
    Start,
    /// Tell the owning chain's Chair what the readings said.
    Tell,
    /// Create the card of `role` for what the reading names (ADR-0035).
    Hire,
    /// End the goal check of the chain `subject` names (ADR-0035).
    End,
    /// Run the job `job` from Remit's closed list (ADR-0035).
    Run,
    /// Tell the Chair that help `subject` was not answered in time (ADR-0062).
    Deadline,
    /// Put to the Assistant the owner's inbox item the reading names, to decide first (ADR-0062).
    Assist,
    /// Resume the step of a workflow run that `workflow` names, which waits on it (ADR-0073).
    Workflow,
    /// A plugin's processor, given what the action's `params` say.
    Plugin(PluginAction),
}

/// Begins the action of a subscription that a plugin's processor does: `plugin:<id>.<name>` (docs/adr/0049).
pub const PLUGIN_ACTION_PREFIX: &str = "plugin:";

/// The name the sensor kind or the processor `name` of the plugin `id` goes by in a record: `plugin:<id>.<name>`.
pub fn plugin_name(id: &str, name: &str) -> String {
    format!("{PLUGIN_ACTION_PREFIX}{id}.{name}")
}

/// The processor `<name>` of the plugin `<id>`, as its text `plugin:<id>.<name>`: made only from the two, or read from a
/// text that begins so.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginAction(String);

impl PluginAction {
    pub fn new(plugin: &str, processor: &str) -> Self {
        Self(plugin_name(plugin, processor))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether it is a processor of the plugin id.
    pub fn of(&self, id: &str) -> bool {
        self.0
            .strip_prefix(PLUGIN_ACTION_PREFIX)
            .and_then(|rest| rest.strip_prefix(id))
            .is_some_and(|rest| rest.starts_with('.'))
    }
}

impl ActionKind {
    /// The texts of Remit's own processors: what a contract that carries this action must accept besides a plugin's.
    pub const VALUES: &'static [&'static str] = &[
        "wake", "start", "tell", "hire", "end", "run", "deadline", "assist", "workflow",
    ];

    /// The action as it is stored.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Wake => "wake",
            Self::Start => "start",
            Self::Tell => "tell",
            Self::Hire => "hire",
            Self::End => "end",
            Self::Run => "run",
            Self::Deadline => "deadline",
            Self::Assist => "assist",
            Self::Workflow => "workflow",
            Self::Plugin(p) => p.as_str(),
        }
    }
}

impl std::str::FromStr for ActionKind {
    type Err = crate::UnknownValue;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(match text {
            "wake" => Self::Wake,
            "start" => Self::Start,
            "tell" => Self::Tell,
            "hire" => Self::Hire,
            "end" => Self::End,
            "run" => Self::Run,
            "deadline" => Self::Deadline,
            "assist" => Self::Assist,
            "workflow" => Self::Workflow,
            plugin
                if plugin
                    .strip_prefix(PLUGIN_ACTION_PREFIX)
                    .is_some_and(|rest| rest.contains('.')) =>
            {
                Self::Plugin(PluginAction(plugin.to_owned()))
            }
            other => {
                return Err(crate::UnknownValue {
                    name: "ActionKind",
                    text: other.to_owned(),
                    values: Self::VALUES,
                });
            }
        })
    }
}

impl fmt::Display for ActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<ActionKind> for String {
    fn from(action: ActionKind) -> Self {
        action.as_str().to_owned()
    }
}

impl TryFrom<String> for ActionKind {
    type Error = crate::UnknownValue;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

/// The chain an `ActionKind::Start` queues.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StartSpec {
    pub title: String,
    pub goal: String,
    /// The kind of chain it queues (ADR-0113).
    pub kind: ChainKind,
    pub origin: ChainOrigin,
    /// A workflow script the chain begins with: its Chair takes its first turn when the run ends (ADR-0073).
    pub script: String,
    /// What `script` reads as args.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<ForeignJson>,
}

/// What a subscription does when it fires. Two actions do the same when they are equal: the same kind, the
/// same words and job, the same start and the same parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SubscriptionAction {
    pub kind: ActionKind,
    /// For a wake, what the Chair will check then and why it cannot be checked now: the owner reads it on the
    /// Subscriptions tab.
    pub why: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<StartSpec>,
    /// The card a hire creates (goal-judge, goal-recheck). Hire only.
    pub role: String,
    /// The job a run does, one of Remit's own. Run only.
    pub job: String,
    /// What an end ends, as the code that stored the subscription named it: a chain's id for its goal check. End only.
    pub subject: String,
    /// What a tell, a start or a run reads of the reading that fires it: an expression in the condition's language for
    /// each parameter the action takes (`chain`, `summary`, `quote`, `author`), by name.
    pub with: BTreeMap<String, String>,
    /// What a plugin's processor is given besides the reading: a JSON object valid against the processor's params
    /// schema. Only for an action `plugin:<id>.<name>`.
    pub params: String,
    /// The step of a run a firing resumes. Workflow only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<crate::WorkflowRef>,
}

text_enum! {
    /// How the readings that hold become firings over time (docs/adr/0064). Unset reads as each.
    #[derive(Default)]
    pub enum FlowPolicy {
        /// Every reading that holds fires alone, at once: the default, also when no policy is named.
        #[default]
        Each = "each",
        /// Every reading fires alone, no sooner than `period` after the firing before; none is lost.
        Delay = "delay",
        /// At most one firing per `period`, carrying the readings held of one key and subject.
        Batch = "batch",
        /// Once readings stop for `period`, or `max_wait` has passed, the newest of each key and subject fires.
        Last = "last",
    }
}

/// A subscription's flow policy. Its default is each.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Flow {
    pub policy: FlowPolicy,
    /// For delay, batch, last.
    pub period: Duration,
    /// Bounds last: the newest reading fires at the latest this long after the first it collapsed. Zero: `period`.
    pub max_wait: Duration,
}

text_enum! {
    /// What a subscription does with a firing while its last is still at work (docs/adr/0064). Unset reads as queue.
    #[derive(Default)]
    pub enum Overlap {
        /// Every firing is done, one at a time, in order: the default, also when none is named.
        #[default]
        Queue = "queue",
        /// While a run it started is at work or a firing waits, one more waits and the rest are skipped.
        BufferOne = "buffer_one",
    }
}

/// When a subscription acts with no reading (docs/adr/0064): a Chair's check-back, a routine, a shipped job, a deadline
/// Remit keeps. Its next instant is the subscription's `due_at`; each instant that comes fires it, caused scheduled,
/// under its schedule_key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Schedule {
    /// How it repeats: five fields in the server's time zone, or `@every 6h`. Empty: it acts once, at `at`.
    pub cron: String,
    /// The one instant a schedule that does not repeat acts at; none for one that repeats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Time>,
    /// How late an instant may still fire: one that came longer ago (the server was down) is skipped, missed_catchup,
    /// and counted. Zero: no limit, one firing catches up whatever was missed.
    pub catchup_window: Duration,
    /// When it last fired; none before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Time>,
}

impl Schedule {
    /// Whether the schedule acts at one instant and is over.
    pub fn once(&self) -> bool {
        self.cron.is_empty()
    }
}

/// Begins the key a subscription's own schedule fires under: schedule:<subscription id>.
pub const SCHEDULE_KEY_PREFIX: &str = "schedule:";

/// The key the firings of subscription sub_id's schedule are recorded under: each at the revision of its instant, in Unix
/// milliseconds, so a retry fires an instant once and the board shows what time caused it. Nothing writes a reading of
/// it.
pub fn schedule_key(sub_id: &str) -> SignalKey {
    SignalKey(format!("{SCHEDULE_KEY_PREFIX}{sub_id}"))
}

text_enum! {
    /// How many readings one firing of a processor carries.
    #[derive(Default)]
    pub enum Takes {
        #[default]
        One = "one",
        Many = "many",
    }
}

/// What a processor declares it takes (docs/adr/0064): how many readings a firing carries, and whether it can be fired by
/// a deadline with none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessorTakes {
    pub readings: Takes,
    pub expires: bool,
}

text_enum! {
    /// Why a subscription's health stops it (docs/adr/0064): the firing that failed for good, or the processor it names.
    #[derive(Default)]
    pub enum HoldReason {
        #[default]
        FiringFailed = "firing_failed",
        ProcessorMissing = "processor_missing",
    }
}

text_enum! {
    /// Whether a subscription acts, as its owner set it or its own end left it (docs/adr/0064). Its default is active, so
    /// a subscription made in code acts unless it is made paused.
    #[derive(Default)]
    pub enum Control {
        /// It judges what comes and fires.
        #[default]
        Active = "",
        /// Its owner paused it: it judges nothing until he resumes it.
        Paused = "paused",
        /// It is over (its deadline came, its one firing fired, a routine ran its last run): kept to be read.
        Ended = "ended",
    }
}

/// Names one firing: the subscription and the key and revision it fired on. The same on every retry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FiringKey {
    pub subscription: String,
    pub key: SignalKey,
    pub rev: i64,
}

// The key as text is the request id its delivery is given (docs/adr/0064), which a processor keys an effect outside the
// store by, so a firing delivered again after a crash or a failed attempt is known as the same one. Nothing reads it back.
impl fmt::Display for FiringKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.subscription, self.key, self.rev)
    }
}

/// A subscription's health stopping it in place (docs/adr/0064): a firing failed for good, and the subscription, its
/// cursor and every firing after it wait behind that one until its owner retries it or skips it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Hold {
    pub reason: HoldReason,
    /// The last error, in the processor's words.
    pub detail: String,
    pub firing: FiringKey,
    pub since: Time,
}

text_enum! {
    /// Why a reading judged fires nothing (docs/adr/0064).
    #[derive(Default)]
    pub enum SkipReason {
        /// Its condition is false on the reading.
        #[default]
        ConditionFalse = "condition_false",
        /// Its condition cannot be read on the reading: counted as false, with the error.
        ConditionError = "condition_error",
        /// The chain that owns the subscription caused the reading.
        OwnChain = "own_chain",
        /// Flow last fired a newer reading of the same key and subject in its place.
        Collapsed = "collapsed",
        /// Overlap buffer_one: a firing waits already.
        Overlap = "overlap",
        /// A scheduled instant that came longer ago than the subscription catches up.
        MissedCatchup = "missed_catchup",
        /// Its screen judged the reading not to need a chain, and the decider was sure enough to act (ADR-0118).
        Screened = "screened",
    }
}

/// Counts what became of what a subscription judged (docs/adr/0064): the firings done and failed, the readings skipped by
/// reason, and the firings found stored already.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Tally {
    pub fired: i64,
    pub failed: i64,
    pub condition_false: i64,
    pub condition_error: i64,
    pub own_chain: i64,
    pub collapsed: i64,
    pub overlap: i64,
    pub missed_catchup: i64,
    pub screened: i64,
    pub duplicate: i64,
}

impl Tally {
    /// Counts one reading skipped for reason.
    pub fn count(&mut self, reason: &SkipReason) {
        let counter = match reason {
            SkipReason::ConditionFalse => &mut self.condition_false,
            SkipReason::ConditionError => &mut self.condition_error,
            SkipReason::OwnChain => &mut self.own_chain,
            SkipReason::Collapsed => &mut self.collapsed,
            SkipReason::Overlap => &mut self.overlap,
            SkipReason::MissedCatchup => &mut self.missed_catchup,
            SkipReason::Screened => &mut self.screened,
        };
        *counter = counter.saturating_add(1);
    }
}

/// A reading a subscription judged that fired nothing, and why: what answers "why did it not fire" (docs/adr/0064).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NearMiss {
    pub subscription_id: String,
    pub seq: i64,
    pub key: SignalKey,
    pub reason: SkipReason,
    /// For condition_error, the error.
    pub detail: String,
    /// For screened, the judgment its screen made of the reading (ADR-0118): the decider's answer the drop acted on.
    pub judgment: String,
    pub at: Time,
}

/// The most characters of a subscription's screen: a decider reads it beside every reading it screens.
pub const SCREEN_CHARS_MAX: usize = 600;

/// Who wants a signal, on what condition, and what happens then.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub owner: SubscriptionOwner,
    /// The pattern of the keys it listens to (docs/adr/0049).
    pub r#match: SignalPattern,
    /// What it does, in a line of the owner's words: its line on the Subscriptions tab is titled by it. A subscription to
    /// an event signal has one; the others are titled by their kind.
    pub note: String,
    /// A CEL condition over a reading (`value`, `header`, `now`, `chain`, `cause`); empty: every reading holds.
    pub expr: String,
    /// For a start: when a reading deserves a chain, in plain words a decider judges true or false of one reading
    /// (ADR-0118, point 2). A reading it judges false with enough certainty is skipped (`SkipReason::Screened`), but one
    /// in twenty fires anyway to learn whether it was right. Empty: every reading that holds fires.
    pub screen: String,
    /// How its readings become firings over time.
    pub flow: Flow,
    /// What it does with a firing while its last is at work.
    pub overlap: Overlap,
    /// A deadline: when it comes the subscription ends and, when nothing else fired it then, fires once expired.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<Time>,
    pub action: SubscriptionAction,
    /// When it acts with no reading; none for one that listens to `match`. A subscription has one of the two.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
    /// The next instant it acts at with no new reading: the earliest of its schedule's next instant, its deadline and its
    /// flow's release; none when there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_at: Option<Time>,
    /// The seq of the last reading the subscription judged, whether it fired or not: a reading is judged once.
    pub seen_seq: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_fired: Option<Time>,
    pub control: Control,
    /// Its health stopping it in place, with the firing that failed; none while it is healthy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<Hold>,
    pub tally: Tally,
    /// A subscription that ships with Remit, as a built-in playbook does: its settings start as the code gives them and
    /// follow Remit until the owner changes them, when his version is kept (`edited`) until he resets it. It is never
    /// deleted.
    pub builtin: bool,
    /// A builtin subscription whose settings are the owner's version, not what ships.
    pub edited: bool,
    /// Who made it or last changed it.
    pub by: Actor,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts its stored changes.
    pub rev: i64,
}

impl Subscription {
    /// The instant the subscription acts at with no new reading: `due_at`, or its deadline when that comes first or
    /// `due_at` is not counted yet (one just made); none when it has neither. A deadline is always a due time, so no
    /// maker of a subscription copies it.
    pub fn due(&self) -> Option<Time> {
        match (self.due_at, self.until) {
            (Some(due), Some(until)) => Some(due.min(until)),
            (due, until) => due.or(until),
        }
    }

    /// Whether the subscription can be done by a processor that takes t (docs/adr/0064): it refuses a policy a processor
    /// of one reading cannot carry, a deadline or a schedule for a processor that cannot fire with no reading, a flow with
    /// no period, and a subscription with neither or both of a pattern and a schedule. The one validator of a
    /// subscription's settings, run where one is made or changed; that its action has a processor, and that its condition
    /// compiles, the caller checks.
    pub fn check(&self, t: &ProcessorTakes) -> Result<(), Invalid> {
        if self.r#match.0.is_empty() == self.schedule.is_none() {
            return Err(Invalid::new(
                "a subscription listens to a key or a pattern of keys, or keeps a schedule: one of the two",
            ));
        }
        if !self.screen.is_empty() {
            if self.action.kind != ActionKind::Start || self.schedule.is_some() {
                return Err(Invalid::new(
                    "a screen is for a start on what is heard: it judges each reading for whether it deserves a chain",
                ));
            }
            if self.flow.policy == FlowPolicy::Batch {
                return Err(Invalid::new(
                    "a screen judges one reading, and a batch fires on many: give the start another flow, or no screen",
                ));
            }
            if self.screen.chars().count() > SCREEN_CHARS_MAX {
                return Err(Invalid::new(format!(
                    "a screen is a statement of at most {SCREEN_CHARS_MAX} characters: a decider reads it with every reading"
                )));
            }
        }
        if let Some(schedule) = &self.schedule {
            return self.check_schedule(schedule, t);
        }
        let flow = &self.flow;
        match &flow.policy {
            FlowPolicy::Each => {
                if flow.period != Duration::ZERO || flow.max_wait != Duration::ZERO {
                    return Err(Invalid::new(
                        "flow each fires every reading at once: it takes no period",
                    ));
                }
            }
            FlowPolicy::Delay | FlowPolicy::Batch | FlowPolicy::Last => {
                if flow.period <= Duration::ZERO {
                    return Err(Invalid::new(format!(
                        "flow {} needs a period longer than zero",
                        flow.policy
                    )));
                }
                if flow.max_wait < Duration::ZERO || flow.max_wait != Duration::ZERO && flow.policy != FlowPolicy::Last
                {
                    return Err(Invalid::new(format!(
                        "a longest wait is for flow last only, and never below zero; flow {} takes none",
                        flow.policy
                    )));
                }
            }
        }
        if flow.policy == FlowPolicy::Batch && t.readings != Takes::Many {
            return Err(Invalid::new(format!(
                "flow batch carries every reading held in one firing, and {} takes one reading at a time: use delay to space them out, or last to keep only the newest",
                self.action.kind
            )));
        }
        if self.until.is_some() && !t.expires {
            return Err(Invalid::new(format!(
                "a deadline fires the subscription with no reading when it comes, and {} cannot be done without one",
                self.action.kind
            )));
        }
        Ok(())
    }

    /// Whether the schedule is sound for a processor that takes t: it repeats or names its one instant, and it fires with
    /// no reading, so it carries no condition, no flow and no deadline of its own.
    fn check_schedule(&self, sch: &Schedule, t: &ProcessorTakes) -> Result<(), Invalid> {
        let flow = &self.flow;
        let why = if sch.once() && sch.at.is_none() {
            "a schedule that does not repeat names the instant it acts at"
        } else if !sch.once() && sch.at.is_some() {
            "a schedule repeats by its cron or acts once at its instant, not both"
        } else if sch.catchup_window < Duration::ZERO {
            "a schedule's catch-up window is never below zero"
        } else if !self.expr.is_empty() {
            "a schedule fires at its instants, which carry no value for a condition to read"
        } else if flow.policy != FlowPolicy::Each || flow.period != Duration::ZERO || flow.max_wait != Duration::ZERO {
            "a schedule fires each of its instants: it takes no flow"
        } else if self.until.is_some() {
            "a schedule ends at its last instant: it takes no deadline"
        } else if !t.expires {
            return Err(Invalid::new(format!(
                "a schedule fires with no reading, and {} cannot be done without one",
                self.action.kind
            )));
        } else {
            return Ok(());
        };
        Err(Invalid::new(why))
    }

    /// Whether the subscription is a chain's wake on a signal, not a time: it waits through its window and ends by its own
    /// firing, its deadline or the chain's end.
    pub fn watch(&self) -> bool {
        self.action.kind == ActionKind::Wake && self.schedule.is_none()
    }
}

/// Records that a subscription decided to act on the readings of a key up to revision `rev`, and where that stands
/// (docs/adr/0064). For an expiry, `rev` is the deadline's instant, and for an instant of a schedule that instant, under
/// the subscription's schedule_key, in Unix milliseconds. (subscription_id, key, rev) is the firing's key (`id`): a firing
/// done is stored with its effect in one write, so a retry or a restart acts once; one whose processor failed is stored
/// pending with its attempts until it is done or fails for good.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Firing {
    pub subscription_id: String,
    /// Who the subscription belonged to when it fired: a chain's subscriptions end with the chain or at their first
    /// firing, and what an owner fired (a chain's wakes in a day) is counted by it all the same.
    pub owner: SubscriptionOwner,
    pub key: SignalKey,
    /// What the firing did. A chain's tell and wake firings share its owner, and only the wakes are counted against its
    /// day.
    pub action: ActionKind,
    pub rev: i64,
    /// The places in the log of the readings the firing covers, ascending; none for an expiry or a schedule's instant.
    pub seqs: Vec<i64>,
    pub at: Time,
    /// What made it: readings, the subscription's deadline that came with no reading to fire it, or an instant of its
    /// schedule.
    pub cause: FiringCause,
    pub status: FiringStatus,
    /// How many times its processor was given it.
    pub attempts: i64,
    /// When a pending one is tried again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_attempt: Option<Time>,
    /// What the last attempt that failed said.
    pub last_error: String,
    /// Why a pending firing's delivery waits without failing (admission, the start quota, the last run still at work), as
    /// its last attempt found it: no attempt is counted, and the next pass tries it again.
    pub waiting: String,
    /// What became of it, in words.
    pub outcome: String,
    /// The chain it acted on: the chain it woke, told, started or ended the check of. None for a firing that did nothing
    /// or ran a job.
    pub chain: String,
    /// The judgment the subscription's screen made of the reading it fired on (ADR-0118); empty when no screen judged it.
    pub screen: String,
    /// The agent it hired.
    pub card: String,
}

impl Firing {
    /// The key that names the firing.
    pub fn id(&self) -> FiringKey {
        FiringKey {
            subscription: self.subscription_id.clone(),
            key: self.key.clone(),
            rev: self.rev,
        }
    }
}

text_enum! {
    /// Where a firing stands (docs/adr/0064).
    #[derive(Default)]
    pub enum FiringStatus {
        /// Recorded and not done yet: a delivery worker hands it to its processor, again at `next_attempt` after a
        /// failure.
        #[default]
        Pending = "pending",
        /// Its effect is done.
        Done = "done",
        /// It failed for good: its subscription is held on it until its owner retries or skips it.
        Failed = "failed",
        /// Its owner gave it up (skip_firing): its readings are judged and nothing was done.
        Skipped = "skipped",
        /// It will never be done: its subscription is gone, or what it was decided on changed.
        Cancelled = "cancelled",
    }
}

text_enum! {
    /// What made a firing: readings, its subscription's deadline with none, or an instant of its schedule.
    #[derive(Default)]
    pub enum FiringCause {
        #[default]
        Reading = "reading",
        Expired = "expired",
        Scheduled = "scheduled",
    }
}
