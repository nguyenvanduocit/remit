//! Reads what the Claude Code configuration on this machine holds (agents and skills) and turns an entry into a record of
//! the Remit library. It only ever reads, and only inside the configuration directory: a name is a file name, and what a
//! link leads out of the directory to is refused.

mod agent;
mod error;
mod list;
mod read;
mod skill;

use serde::Serialize;

pub use agent::agent;
pub use error::Error;
pub use list::list;
pub use skill::skill;

/// Where an imported record says it came from.
pub const SOURCE_KIND: &str = "claude-code";

/// What a Candidate is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Agent,
    Skill,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Agent => "agent",
            Kind::Skill => "skill",
        }
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One entry of the configuration the owner could import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub kind: Kind,
    /// What the import call takes.
    pub name: String,
    pub description: String,
    /// What the entry would add to the library, in bytes.
    pub size: i64,
    /// Why the import would be refused; empty when it would not.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub problem: String,
}

/// What the library already holds: an imported agent keeps only the names it can resolve.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known {
    pub models: Vec<String>,
    pub skills: Vec<String>,
}

/// What an import left out, so the owner never has to guess what a record is missing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Files of a skill that were not taken.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skip>,
    /// Parts of the entry the record has no place for, as "model opus" or "field color".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unmapped: Vec<String>,
}

/// A file left out, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skip {
    pub what: String,
    pub why: String,
}
