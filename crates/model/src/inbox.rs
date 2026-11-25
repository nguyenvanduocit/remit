use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, ChangeKind, PendingChange, Replay, Time};

text_enum! {
    #[derive(Default)]
    pub enum InboxKind {
        Permission = "permission",
        #[default]
        Question = "question",
        /// A turn or a session failed; the owner retries or dismisses.
        Error = "error",
        /// Asks for the value of one variable of a source's connection. The owner types it into a write-only field; it
        /// reaches the store and nothing else: no answer text, no agent, no decision.
        Credential = "credential",
        /// Asks the owner to sign an account in: it holds the link he opens and a write-only field for the code the page
        /// shows, which reaches the sign-in process and nothing else.
        Login = "login",
        /// Asks the owner to let a write through that moves a rail: a program Remit would start, or a capability that
        /// writes. It is answered with Apply or Not now, and nothing is written before.
        Change = "change",
        /// A key result that could not be served or judged as the objective said: it waits for the owner, who decides.
        Exception = "exception",
        /// A subscription held in place on a firing that failed for good (docs/adr/0064): answered with retry (the firing
        /// is tried again, then the ones after it) or skip (it is given up), the text saying why. Lifting the hold any
        /// other way closes it.
        Held = "held",
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InboxOption {
    pub id: String,
    pub name: String,
    /// allow_once, allow_always, reject_once, reject_always for permissions.
    pub kind: String,
}

text_enum! {
    /// Where an inbox item stands: open until the owner answers it, or until what it waits on goes away.
    #[derive(Default)]
    pub enum InboxStatus {
        #[default]
        Open = "open",
        /// The Assistant decided it for the owner, and what it decided lets something on the top rung of the effect
        /// ladder happen: it takes effect at `takes_effect_at`, unless he overturns it first (ADR-0101).
        Decided = "decided",
        Answered = "answered",
        Cancelled = "cancelled",
    }
}

/// Something only the owner may answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub home: String,
    pub card_id: String,
    pub kind: InboxKind,
    pub title: String,
    pub detail: String,
    pub options: Vec<InboxOption>,
    pub status: InboxStatus,
    pub answer: String,
    pub created_at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answered_by: Option<Actor>,
    /// A decided item's: when the Assistant's answer takes effect, unless the owner overturns it first (ADR-0101).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub takes_effect_at: Option<Time>,
    /// An error's: what the owner's "retry" does; none when nothing can be retried.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<Retry>,
    /// `source`, `connection` and `variable` name the credential variable a credential item asks for; `secret` says its
    /// field is masked, a plain value (an address, a user) being shown as typed.
    pub source: String,
    pub connection: String,
    pub variable: String,
    pub secret: bool,
    /// `account` and `link` are a login item's: the account it signs in, and the page the owner opens to do it.
    pub account: String,
    pub link: String,
    /// The write a change item waits on; no channel is told it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<PendingChange>,
    /// `change.kind`, set when the item is stored: what a channel is told of the write, to word the item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_kind: Option<ChangeKind>,
    /// A change item's replay: its candidate run against the goals Remit recorded, and how it fared (ADR-0069).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay: Option<Replay>,
    /// A held item's: the subscription held. Remit raises it, so it has no card.
    pub subscription: String,
    /// A question's that a run's askOwner() asked: the step his answer settles, not a message to the Chair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<crate::WorkflowRef>,
    /// Nobody decides it for the owner (ADR-0087): the Assistant raised it, so it is not put to the Assistant (the asker
    /// never decides its own), or Remit raised it with no agent to put it to. It waits for him (waiting.rs).
    pub owner_only: bool,
    /// A permission item's: the call it asks for, as the one deciding reads it (orch permission_ask.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionAsk>,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

/// A call an agent asks leave to make, in words: what it does, why it is asked, what about it is risky, the exact call,
/// and once it no longer waits how it ended. Derived from the call and the rules when it is asked, and stored with its
/// item, so every channel reads the same words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionAsk {
    /// The action in words, read off the call itself ("Run psql -h prod-db -c …"), never the agent's description of it.
    pub action: String,
    /// What the agent says the call does, in its own words (its `description`): shown as the agent's, never as the action,
    /// and read for no trait, since an asker may word a call as anything.
    pub said: String,
    /// One sentence: why it is asked rather than run.
    pub reason: String,
    /// What about it is risky, the most serious first.
    pub traits: Vec<PermissionTraitSaid>,
    pub risk: PermissionRiskSaid,
    /// Claude Code's name for the tool.
    pub tool: String,
    /// The exact call: a command as typed, a file's path, else the input as JSON.
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<PermissionOutcome>,
    /// Once it no longer waits, one line of who ended it and how ("Allowed by the Assistant, for you").
    pub decided: String,
}

/// A trait of a call and the words its badge reads: a channel keys its icon on the kind and shows the words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionTraitSaid {
    pub kind: PermissionTrait,
    pub says: String,
}

/// How careful to be and the words its badge reads ("High risk"): a channel keys its hue and the hold on the kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionRiskSaid {
    pub kind: PermissionRisk,
    pub says: String,
}

text_enum! {
    /// What about a call makes it worth a look, read off the call itself.
    #[derive(Default)]
    pub enum PermissionTrait {
        /// It names a production system.
        #[default]
        Production = "production",
        /// What it removes or overwrites does not come back.
        Irreversible = "irreversible",
        /// It writes outside the session's working directory.
        OutsideWorktree = "outside_worktree",
        /// It reaches another machine.
        Network = "network",
        /// It names a secret: a key, a credentials file, a dotenv file.
        Secrets = "secrets",
        /// It asks to run outside the sandbox.
        Unsandboxed = "unsandboxed",
    }
}

text_enum! {
    /// How careful the one deciding should be: high is what cannot be undone, which a channel confirms by a hold.
    #[derive(Default)]
    pub enum PermissionRisk {
        #[default]
        Low = "low",
        Medium = "medium",
        High = "high",
    }
}

text_enum! {
    /// How a permission ask ended; unset while it waits.
    pub enum PermissionOutcome {
        Allowed = "allowed",
        Refused = "refused",
        /// Nobody waits on it any more: its turn ended before anyone decided.
        Withdrawn = "withdrawn",
    }
}

/// What the owner's retry of an agent's failure does: queue the failed turn's message again, or open again a session that
/// failed to open, which then reads the messages that waited for it. Either way the stream's records of the failure are
/// deleted, so the retried work follows what was done, not the error that stopped it. Stored with the error, so it
/// outlives a restart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Retry {
    pub card_id: String,
    /// The event of the message the failed turn was on.
    pub prompt_id: i64,
    /// The events that record the failure.
    pub failure_ids: Vec<i64>,
    pub reopen: bool,
}
