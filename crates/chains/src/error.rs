//! The errors of the orchestrator's actions and its signal pipeline, sentinels and refusals alike, in one type.

/// Why an orchestrator action did not do what it was asked.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An entry to running that admission holds (`admit`): the text says what holds it, for the owner (`why_not`).
    #[error("not admitted: {0}")]
    NotAdmitted(String),
    /// A change asked for once the server has begun to shut down.
    #[error("the server is shutting down: try again once it is back")]
    Stopping,
    #[error("the chain was deleted")]
    ChainDeleted,
    /// The firing is settled already: done (its effect was done then), or given up. Its effect is not done twice.
    #[error("the firing is stored already")]
    FiredAlready,
    /// A processor found nothing for its firing to act on (what the reading names is gone or done already): the firing
    /// is done, saying so.
    #[error("the firing has nothing to act on")]
    NothingToDo,
    /// A start refused while the last chain its subscription started is at work: the text names that chain.
    #[error("the routine's last run is still at work ({0})")]
    RoutineAtWork(String),
    /// A firing's failure no attempt mends (no processor does its action, the action is refused): its subscription is
    /// held at once, for the reason.
    #[error("{1}")]
    Permanent(model::HoldReason, String),
    /// A refusal with its reason, as the asker reads it.
    /// An event of a signal no one made: a signal is made, by telling the Assistant, before anyone writes it. The door
    /// answers it as it answers a key that does not open.
    #[error("no event signal has this name: the owner makes one by telling the Assistant")]
    NoEventSignal,
    /// A reading of a paused signal, or from a paused sensor, whoever writes it: nothing is stored. The door answers it
    /// as it answers a key that does not open.
    #[error("the signal is paused: it takes no reading until the owner resumes it")]
    SignalPaused,
    #[error("{0}")]
    Refused(String),
    /// A write by an agent that moves a rail (change_item.rs): nothing was stored, and the item asks the owner. It is an
    /// error so that a caller that does not know about it cannot take the write for done.
    #[error(
        "this write moves a rail, so nothing was written: it waits in the owner's inbox ({}), where the Assistant decides it for him, and you are told the answer",
        .0.title
    )]
    WaitingForOwner(Box<model::InboxItem>),
    /// An Apply on a change whose record is not the one it was made against: the item is closed and the agent was told.
    #[error("the record changed since the agent asked: the item is closed and the agent was told")]
    ChangeMoved,
    /// An update an agent made from a read of a record that a write has come after (the owner's, another agent's):
    /// nothing was written, and the words say how to go on.
    #[error("{0}")]
    RecordMoved(Box<Moved>),
    /// A save of the owner's words made on a copy of his draft that another page has saved over since: nothing was
    /// saved, and the draft as it is now goes back for the page to go on from.
    #[error("the draft changed on another page since this one read it: nothing was saved")]
    DraftMoved(Box<model::Draft>),
    /// The failure the first asking of a request answered, given again to a retry of it (request.rs): its words, the
    /// kind a channel acts on, and its detail.
    #[error("{why}")]
    Answered {
        kind: crate::ErrorKind,
        why: String,
        detail: Option<Box<model::FailureDetail>>,
    },
    #[error(transparent)]
    Store(#[from] store::Error),
    #[error(transparent)]
    Telemetry(#[from] store::TelemetryError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn refused(why: impl Into<String>) -> Self {
        Self::Refused(why.into())
    }

    /// Whether the store refused a write over a revision that is no longer the stored one.
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Store(store::Error::Conflict))
    }
}

/// A record an update was made from an older read of: what it is, which, the revision the update was made from, the one
/// stored, and the tool that reads it as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub what: &'static str,
    pub id: String,
    pub read: i64,
    pub now: i64,
    pub reread: &'static str,
}

impl std::fmt::Display for Moved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            what,
            id,
            read,
            now,
            reread,
        } = self;
        write!(
            f,
            "{what} {id} changed since the read this update was made from (rev {read}; it is at rev {now} now), so nothing was written: {reread} it again, make your change to what it holds now, and send that with rev {now}"
        )
    }
}

/// Ok when an update made from revision read of the record is made over that revision, stored now; else the conflict.
pub fn unmoved(what: &'static str, id: &str, read: i64, now: i64, reread: &'static str) -> Result<()> {
    if read == now {
        return Ok(());
    }
    Err(Error::RecordMoved(Box::new(Moved {
        what,
        id: id.to_owned(),
        read,
        now,
        reread,
    })))
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
