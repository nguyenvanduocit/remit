use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, Gate, Time, Title, VerdictRole};

text_enum! {
    /// Which gate a card's verdict closes: the reviewer's, the QC's, the security reviewer's (a change that touches a
    /// rail, ADR-0087), or the goal judge's. It is stored on the card at hire, so editing an agent later never takes a
    /// verdict power from a judge in the middle of a chain.
    #[derive(Default)]
    pub enum Signs {
        #[default]
        None = "",
        Review = "review",
        Qc = "qc",
        Security = "security",
        /// Only a definition titled judge signs it (ADR-0032); a judge signs nothing else.
        Goal = "goal",
    }
}

impl Signs {
    /// The change gate a card that signs this closes with its verdict; none for a judge's and for a card that signs none.
    /// What a card signs is fixed when it is hired, from its agent or the role a standing employee is put to work in, never
    /// read from its name.
    pub const fn gate(self) -> Option<Gate> {
        match self {
            Self::Review => Some(Gate::Reviewer),
            Self::Qc => Some(Gate::Qc),
            Self::Security => Some(Gate::Security),
            Self::None | Self::Goal => None,
        }
    }

    /// The role a card that signs this gives its verdicts as: its gate's, or judge for the goal judge; none for a card
    /// that signs none. A card the Chair hires as "judge" is refused (its title): only Remit starts
    /// those judges, and sets what they sign.
    pub const fn verdict_role(self) -> Option<VerdictRole> {
        match self {
            Self::Goal => Some(VerdictRole::Judge),
            Self::Review => Some(VerdictRole::Reviewer),
            Self::Qc => Some(VerdictRole::Qc),
            Self::Security => Some(VerdictRole::Security),
            Self::None => None,
        }
    }
}

/// The system roles Remit starts for itself, which are not employees; no definition may take their ids. The Chair and the
/// judge are templates employees are hired from (ADR-0032), so their ids are free.
pub const RESERVED_AGENT_IDS: [&str; 2] = ["assistant", "probe"];

/// What a worker's sessions hold: the instructions they start with, the Claude Code tools they may use, and the skills
/// they hold from the library. An Agent keeps one as the default of its hires, and a card keeps its own copy, taken when
/// it is hired: what a card holds changes only through that hire's own choices, so an edit to a definition, or its
/// deletion, reaches the agents hired afterwards and nobody already at work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Loadout {
    /// What every session starts with, appended to the system prompt: how it works, what it must always or never do. A
    /// brief is something else: the task a card is given.
    pub instructions: String,
    /// The Claude Code tools it may use. None means every tool; empty means none.
    pub tools: Option<Vec<String>>,
    /// What it holds from the library, by name.
    pub skills: Vec<String>,
}

/// A definition the owner keeps: what an agent of this kind is for and what its hires hold by default. A card runs one
/// (`Card::agent` is its id) and takes a copy of its loadout when it is hired; an id with no definition is a plain agent
/// that holds every Claude Code tool and nothing from the library. The owner makes and changes agents by telling an
/// `author` chain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The kind of work it belongs to (parse_group): the column it stands in on the Agents page. A built-in agent's is
    /// code.
    pub group: String,
    /// Its default; empty uses the worker model.
    pub model: String,
    /// Its default effort; empty uses the hiring Chair's.
    pub effort: String,
    /// What its hires hold by default.
    pub loadout: Loadout,
    /// The gate it closes with a verdict: review and qc for a worker, goal for the title judge only.
    pub signs: Signs,
    /// The title an employee hired from it holds (ADR-0032); unset until the template names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Title>,
    /// Where it came from, when an agent imported it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Shipped with Remit: can be reset to its original, not deleted.
    pub builtin: bool,
    /// Written since it shipped, on the owner's word: a restart keeps it instead of rewriting it.
    pub edited: bool,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

/// Where a library item came from, when the owner had an agent import it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Source {
    /// claude-code: read from the Claude Code configuration on this machine.
    pub kind: String,
    pub name: String,
    /// Of what was read, so a later read can tell it changed.
    pub hash: String,
    pub at: Time,
}

/// Instructions an agent can load when its work calls for them. Remit renders the library's skills into a plugin
/// directory for Claude Code; the files here are the one copy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// The kind of work it is for (parse_group): the column it stands in on the Skills page.
    pub group: String,
    /// A path inside the skill's directory to its text. SKILL.md is required.
    pub files: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    pub rev: i64,
}

/// One place the owner reaches through a source: the same code with another set of credentials (a staging and a
/// production MongoDB). The owner names it, and `description` says what it reaches, for the agents that must choose.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourceConnection {
    pub source: String,
    pub name: String,
    pub description: String,
}

/// The right given to act on one connection of one source with an effect (ADR-0018): the strongest effect granted to its
/// holder on that connection, and who gave it. Effect is one of the names the grant crate ranks; it parses it, the store
/// refuses any other text. What a chain reads through its capabilities is not stored: it is derived.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Grant {
    pub holder: GrantHolder,
    pub source: String,
    pub connection: String,
    pub effect: String,
    pub granted_by: Actor,
    pub granted_at: Time,
}

/// Who holds a grant: a chain by its id, or a repository by its catalog id, for the release that runs as no chain
/// (ADR-0116).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GrantHolder {
    pub kind: HolderKind,
    pub id: String,
}

impl GrantHolder {
    pub fn chain(id: &str) -> Self {
        Self {
            kind: HolderKind::Chain,
            id: id.to_owned(),
        }
    }
}

text_enum! {
    /// What holds a grant.
    #[derive(Default)]
    pub enum HolderKind {
        #[default]
        Chain = "chain",
        Repository = "repository",
    }
}
