//! The kinds of the orchestrator's errors, as a channel tells one failure from another, and the detail a channel acts on
//! beyond the kind (`model::FailureDetail`).

use model::FailureDetail;

use crate::Error;

/// Why the orchestrator did not do what it was asked, as a channel tells one failure from another: the words of an error
/// are for the owner and change, its kind is what code acts on. The contract names the same kinds (`link::ErrorCode`),
/// carried as the code of the error a call answers with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// No such record: deleted, or never was.
    NotFound,
    /// A draft sent or deleted already.
    Ended,
    /// The server cannot take it now; the same call later may go.
    Unavailable,
    /// What the action would change moved since the channel read it: nothing changed, and the detail holds it as it is.
    Conflict,
    /// Anything else.
    Failed,
}

impl ErrorKind {
    /// The kind a stored spelling names; any other (none recorded) is failed.
    pub fn parse(s: &str) -> Self {
        match s {
            "NOT_FOUND" => Self::NotFound,
            "ENDED" => Self::Ended,
            "UNAVAILABLE" => Self::Unavailable,
            "CONFLICT" => Self::Conflict,
            _ => Self::Failed,
        }
    }

    /// The kind as the contract spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "NOT_FOUND",
            Self::Ended => "ENDED",
            Self::Unavailable => "UNAVAILABLE",
            Self::Conflict => "CONFLICT",
            Self::Failed => "FAILED",
        }
    }
}

impl Error {
    /// The kind of the error, by what it is.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Store(store::Error::DraftEnded) => ErrorKind::Ended,
            Self::Store(store::Error::NotFound) => ErrorKind::NotFound,
            Self::Stopping => ErrorKind::Unavailable,
            Self::DraftMoved(_) | Self::RecordMoved(_) => ErrorKind::Conflict,
            Self::Answered { kind, .. } => *kind,
            _ => ErrorKind::Failed,
        }
    }

    /// What a channel needs beyond the kind to act on the failure; none for most.
    pub fn detail(&self) -> Option<FailureDetail> {
        match self {
            Self::DraftMoved(current) => Some(FailureDetail::DraftConflict {
                current: current.as_ref().clone(),
            }),
            Self::Answered { detail, .. } => detail.as_deref().cloned(),
            _ => None,
        }
    }
}
