//! What the core and its shell say to each other with no transport in it (ADR-0072 decision 10, ADR-0095 decision 5):
//! the models a page draws, their rows and how fresh each is, the core's state, what the owner should notice, what the
//! page asks (`Ask`, the shell's half of `Input`) and what it is told (`Told`, the shell's half of `Output`). tsgen
//! writes this module's TypeScript for the page (`web/src/core/generated/replica`), so it holds only shapes tsgen reads.

use link::{Failure, Millis, RefusalCode};
use model::InboxKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wire::app::FaultKind;

/// A model's key for a row: the server's id of its record, as text (an event's number too). A model of one record
/// holds it under the empty key.
pub type Key = String;

/// Names a shell's call, so its answer finds it.
pub type Ticket = u64;

/// What a page draws, one per kind of record the core keeps in step: the catalog's (the whole app), the open chain's,
/// the Assistant's, the open event's, the archived pages read, and the owner's typed words the server does not have
/// yet. A page shows a model (`Ask::Show`) and is told its rows, in the model's order, until it hides it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "model", content = "card", rename_all = "camelCase")]
pub enum Model {
    /// Who the page signed in as, the owner or a mentor (one record, ADR-0089).
    Who,
    Chains,
    Inbox,
    /// Where each chain stands, by chain id.
    Standings,
    Board,
    Playbooks,
    Agents,
    Skills,
    Sources,
    Capabilities,
    Health,
    Workload,
    /// Every seat at work now (`AtWork`, one record), read on each (re)connect of the owner's page and kept live by pushes.
    AtWork,
    Signals,
    /// The columns of the boards whose cards arrive one by one (`Boards`, one record).
    Boards,
    Insights,
    /// The newest events (`EventRecord`) with their firings, newest first.
    Events,
    EventSignals,
    Sensors,
    ClientKeys,
    Extensions,
    Feedback,
    Objectives,
    /// Every project, archived ones too, by name (ADR-0108).
    Projects,
    Employees,
    Mentors,
    /// Where each mentor stands, by mentor id.
    MentorStandings,
    /// The open chain itself (one record).
    OpenChain,
    Cards,
    Edges,
    Decisions,
    ChainDrafts,
    /// The open chain's workflow runs, each with its steps, oldest first.
    Workflows,
    Helps,
    /// Where each agent of the open chain stands, by card id.
    AgentStandings,
    /// How the open chain's result was judged (one record).
    Verification,
    /// What happened in the open chain since the owner last viewed it (one record, or none), as it was opened.
    Away,
    /// The coach's retrospectives of the open chain, oldest first.
    Retrospectives,
    /// The open chain's landings, each with the release that carried it, oldest first (ADR-0114, ADR-0116).
    Landings,
    /// Where Datadog shows what the open chain did, as it was opened (ADR-0119): none while nothing is sent there.
    DatadogLinks,
    /// Every event of the open chain, oldest first.
    ChainEvents,
    /// One card's events of the open chain, oldest first: those of the chain's answer and pushes, or its whole stream
    /// once asked for.
    CardEvents(Key),
    /// The Assistant (one record), its card (one record) and where it stands (one record).
    Assistant,
    AssistantCard,
    AssistantAgent,
    AssistantDrafts,
    /// The Assistant's stream: its conversation, oldest first.
    AssistantEvents,
    /// The Assistant's feed, sent whole (one record).
    AssistantFeed,
    /// The event open on screen (one record).
    OpenEvent,
    /// The archived chains of the pages read, newest change first as read.
    Archived,
    /// The repository open (`Ask::OpenRepo`): its queues, releases and pause as the server lays them out (one
    /// `RepoView`, ADR-0114, ADR-0116), once read.
    Repo,
    /// Every repository with something in its queues or releases (`Ask::OpenRepos`), by catalog id, in the server's
    /// order.
    Repos,
    /// What the owner typed in a draft that the server does not have yet, by draft id (`DraftInput`).
    Typed,
    /// The owner's sends the server has not taken yet, oldest first, by the id of their row (`Pending`).
    Outbox,
}

/// A send of the owner's drafts the server has not taken yet (ADR-0094): shown at once where its drafts were, kept
/// while the link is down and sent when it is back, with the same request id, so it acts once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub id: Key,
    /// Where the row shows: the page's name for the box it was sent from.
    pub at: Key,
    /// The words it sends, as the page showed them when the owner sent them.
    pub text: String,
    /// The drafts it sends: they leave their box while it is here, and leave the page once it went.
    pub drafts: Vec<Key>,
    pub status: PendingStatus,
    /// Why it did not go, when `status` is `failed`; empty otherwise.
    pub why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PendingStatus {
    /// No session: it goes out when the connection is back.
    Waiting,
    /// Asked of the server a moment ago.
    Going,
    /// Asked of the server and not answered for a while (`outbox::SLOW`): the page says it is sending.
    Slow,
    /// It did not go: `Ask::Resend` tries again, `Ask::Unsend` puts its words back in their box.
    Failed,
    /// The server took it: the row says so for a moment (`outbox::SENT_SHOWN`), then goes.
    Sent,
}

/// What the core keeps across a reload of the page (ADR-0094): the sends the server has not taken yet, so a page that
/// reloads (a new build, the owner's F5) sends them still, with the request ids they were given. The shell stores it
/// as it is told it (`Told::Keep`) and hands it back before it connects (`Ask::Restore`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    pub sends: Vec<KeptSend>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptSend {
    /// The row as it shows.
    pub row: Pending,
    /// The call it sends, as the shell wrote it.
    pub call: Value,
    /// Its request id, kept for every try whose answer never came.
    pub request: Key,
    /// The server answered `request`: Try again takes a new one.
    pub answered: bool,
    /// What the owner typed in its drafts that the server does not have yet (`DraftInput`), with the revision it was
    /// typed over: saved before the send goes.
    pub typed: Vec<KeptDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptDraft {
    pub draft: Value,
    pub base_rev: i64,
}

/// A row of a model: a record, as the server's JSON for its type, under its key.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    pub id: Key,
    pub record: Value,
}

/// One change to the rows of a model the page shows; a batch of them is applied in order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "change", rename_all = "camelCase")]
pub enum Change {
    /// The row takes this record, where it is; a new row goes before `before`, or last when none.
    Put {
        model: Model,
        id: Key,
        #[serde(skip_serializing_if = "Option::is_none")]
        before: Option<Key>,
        record: Value,
    },
    /// A streamed chunk: the text of the event the row holds (`EventRead`'s `event`) grows by it.
    Append {
        model: Model,
        id: Key,
        text: String,
    },
    Drop {
        model: Model,
        id: Key,
    },
}

/// How current a model the page shows is (ADR-0095 decision 11), for the page to draw as the link's banner says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Freshness {
    /// Current, and kept so by pushes since `as_of` (the shell's clock, in milliseconds).
    Live { as_of: Millis },
    /// What shows is from `since` (the shell's clock): the link went, or reading it again failed.
    Stale { since: Millis, reason: StaleReason },
    /// Being read (on a reconnect, an opening, or its home let go by the server): what shows, if anything, is from
    /// before.
    Repairing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StaleReason {
    /// No session.
    Offline,
    /// The session is up, but reading what shows again failed: it is tried again.
    Behind,
}

/// The link as the owner has to know it (`link.ts`): no session, the snapshot on its way (what shows is from before),
/// the snapshot failed and is asked for again (what shows may be old), or live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Offline,
    Syncing,
    Stale,
    Live,
}

/// The core's own state a shell shows, sent whole each time it changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub phase: Phase,
    /// The latest reason the link is not live; empty when there is none.
    pub why: String,
    /// What the page says of the link, ready to show (ADR-0094); none while everything shown is current.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<Banner>,
    /// Why an action that changes something cannot be taken now (no session: a message still goes, later); empty when
    /// it can.
    pub blocked: String,
    /// The server turned this page away for good (`bad_credential`: the shell shows the way in) or until it loads the
    /// build the server serves (`new_bundle`); it is not dialled again until `Input::Connect`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<RefusalCode>,
    /// The first snapshot is in.
    pub loaded: bool,
    /// The chain being opened, until it shows; empty when none is.
    pub opening: Key,
    /// The open chain's answer held its newest events only.
    pub truncated: bool,
    /// The cards whose stream goes back further than the events held: `LoadEarlier` reads the page before.
    pub earlier: Vec<Key>,
    /// The Assistant's stream has been answered.
    pub assistant_loaded: bool,
    /// The event being opened, until it shows.
    pub event_opening: Key,
    /// Counts the snapshots that forgot the archived pages read: what shows them reads them again.
    pub archived_generation: u64,
    /// The first archived page has been read since the last snapshot.
    pub archived_read: bool,
    /// A page after those read is there to read.
    pub archived_more: bool,
    /// Every repository has been read since they were opened or the last snapshot (the one open is read once its
    /// record is there: `repo_get` answers a view for any repository).
    pub repos_read: bool,
}

/// What the page says of the link: its words, how it looks, the one thing the owner can do about it, and what a screen
/// reader is told.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Banner {
    pub tone: BannerTone,
    /// What it says. `{since}` stands for `since`, which the shell writes in the owner's own clock.
    pub text: String,
    /// When the link went (the shell's clock, in milliseconds): what shows is from then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<Millis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<BannerAction>,
    /// How many of the owner's messages wait for the link, and when they go.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waits: Option<String>,
    /// What a screen reader is told, once for each change: the words without the countdown.
    pub announce: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BannerTone {
    /// The session is up and what shows is loading.
    Loading,
    /// The session is up but what shows could not be loaded: it may be old.
    Stale,
    /// No session: what shows is from when the link went, or the page must be reloaded or signed in again.
    Down,
    /// The link is back and what shows is current: said for a moment (`banner::BACK_SHOWN`).
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BannerAction {
    /// Dial now (`Ask::Connect`).
    TryNow,
    /// Load the page again: the server runs another build.
    Reload,
}

/// What the owner should notice without watching; the shell words it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "notice", rename_all = "camelCase")]
pub enum Notice {
    ChainDone {
        title: String,
    },
    /// An agent finished its turn; its report went to the Chair.
    TurnFinished {
        name: String,
    },
    InboxOpened {
        kind: InboxKind,
        title: String,
    },
    /// A duel was judged or overturned: where duels stand moved.
    DuelsMoved,
    /// Something the owner asked for did not happen.
    CouldNot {
        what: What,
        failure: Failure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum What {
    OpenChain,
    LoadStream,
    LoadEarlier,
    LoadAssistant,
    LoadAtWork,
    OpenEvent,
    ReadArchived,
    ReadRepos,
    SaveDraft,
    DeleteDraft,
}

/// A place's actions, one at a time: asking the same `key` again after an answer that never came (`TIMEOUT`,
/// `DISCONNECTED`) is a retry of that action, with its request id; anything else is a new action.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Retry {
    pub place: String,
    pub key: String,
}

/// One of a page's asks, of the core its tab shares with the browser's other tabs (ADR-0095 decision 4). What a tab
/// opens (a chain, an event, the archived pages) and the models it shows are its own; what it calls is answered to it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "ask", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Ask {
    Connect,
    /// The page draws `model`: it is told the model whole, then each change to its rows and to its freshness.
    Show {
        model: Model,
    },
    /// The page no longer draws `model`: it is told nothing more of it.
    Hide {
        model: Model,
    },
    Call {
        ticket: Ticket,
        #[serde(default)]
        retry: Option<Retry>,
        call: Value,
    },
    OpenChain {
        id: Key,
    },
    LoadStream {
        card: Key,
    },
    LoadEarlier {
        card: Key,
    },
    LoadAssistant,
    OpenEvent {
        id: Key,
    },
    CloseEvent,
    ReadArchived,
    /// Opens repository `repo` (its catalog id); empty closes it.
    OpenRepo {
        repo: Key,
    },
    /// Opens every repository, or closes them.
    OpenRepos {
        open: bool,
    },
    Write {
        draft: Value,
    },
    Discard {
        id: Key,
    },
    Send {
        ticket: Ticket,
        ids: Vec<Key>,
        call: Value,
        at: Key,
        text: String,
    },
    Resend {
        id: Key,
    },
    Unsend {
        id: Key,
    },
    Restore {
        kept: Kept,
    },
    /// Whether the page is out of the owner's sight (`document.hidden`): what the owner should notice is said where he
    /// looks, in a tab he sees, or in every tab when he sees none.
    Hidden {
        hidden: bool,
    },
    /// Something failed in the page that nothing handled (ADR-0119): it goes up in the minute's link report, with the
    /// chain the tab shows.
    Fault {
        kind: FaultKind,
        message: String,
        /// Where: the script and line the browser named, or empty.
        #[serde(default)]
        at: String,
    },
}

/// One thing the page is told, in the order the core said it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "told", rename_all = "camelCase")]
pub enum Told {
    /// A model the page shows, whole, and how fresh it is: as it is when shown, or as read again.
    Model {
        model: Model,
        rows: Vec<Row>,
        status: Freshness,
    },
    /// What changed in the rows of the models the page shows, in order.
    Rows {
        changes: Vec<Change>,
    },
    /// A model the page shows is now this fresh.
    Status {
        model: Model,
        status: Freshness,
    },
    State {
        state: State,
    },
    /// The call of this ticket answered `value`.
    Answer {
        ticket: Ticket,
        value: Value,
    },
    /// The call of this ticket failed.
    Failed {
        ticket: Ticket,
        failure: Failure,
    },
    Notice {
        notice: Notice,
    },
    /// What to keep across a reload, in place of what was kept before.
    Keep {
        kept: Kept,
    },
}
