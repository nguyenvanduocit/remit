use std::io;

use skills::{FrontmatterError, SkillError};

use crate::Kind;

/// Why an entry cannot be listed or imported. Its text is what the owner reads as a candidate's problem.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no Claude config directory given")]
    NoConfigDir,
    #[error("{0:?}: not a file name")]
    BadName(String),
    #[error("{path} resolves to {target}: outside the Claude config directory")]
    Outside { path: String, target: String },
    #[error("more than {0} bytes: too large")]
    TooBig(usize),
    /// The entry exists and is not a skill: a file, or a directory without SKILL.md.
    #[error("{0}: not a skill")]
    NotASkill(String),
    #[error("{0} is not a regular file")]
    NotRegular(String),
    #[error("{path}: {source}")]
    Io { path: String, source: io::Error },
    #[error("SKILL.md was not taken: {0}")]
    SkillFileNotTaken(String),
    #[error(transparent)]
    Frontmatter(#[from] FrontmatterError),
    #[error("the frontmatter has no name")]
    NoName,
    #[error("the name {0:?} does not make an id of lower-case words joined by dashes")]
    BadId(String),
    #[error("{field}: {problem}")]
    Field { field: &'static str, problem: &'static str },
    #[error(
        "disallowedTools cannot be mapped: the library lists the tools an agent may use, so write them in tools and import again"
    )]
    DisallowedTools,
    #[error("skill {name:?}: SKILL.md: {source}")]
    Header { name: String, source: FrontmatterError },
    #[error(transparent)]
    Invalid(#[from] SkillError),
    /// What reading one entry failed with, said of that entry.
    #[error("{kind} {name:?}: {source}")]
    Entry {
        kind: Kind,
        name: String,
        source: Box<Error>,
    },
}

impl Error {
    /// The error under the entry it is said of: what a caller matches on.
    pub fn reason(&self) -> &Error {
        match self {
            Error::Entry { source, .. } => source.reason(),
            other => other,
        }
    }

    /// Whether a file or directory the path names is not there.
    pub(crate) fn is_not_found(&self) -> bool {
        matches!(self.reason(), Error::Io { source, .. } if source.kind() == io::ErrorKind::NotFound)
    }

    pub(crate) fn io(path: &std::path::Path, source: io::Error) -> Self {
        Error::Io {
            path: path.display().to_string(),
            source,
        }
    }
}
