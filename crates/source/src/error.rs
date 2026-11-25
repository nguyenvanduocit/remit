//! The one error of this crate: text an agent may read, or a system's refusal a source reacts to.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// A future that can be sent between threads, as every port of this crate answers.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What an implementation of a port (the store, the policy, the runner, the plugins) fails with: its text is what is shown.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Why a call failed. Its text is safe for an agent to read once the gate has redacted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A failure said in words.
    Message(String),
    /// A system answered with a status that is not a success.
    Status(StatusError),
}

impl Error {
    /// The error that says `text`.
    pub fn msg(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }

    /// The status code when the system refused the request, so a source can sign in again on 401 or say a rate limit hit.
    #[must_use]
    pub const fn status(&self) -> Option<&StatusError> {
        match self {
            Self::Status(s) => Some(s),
            Self::Message(_) => None,
        }
    }

    /// This error with `lead` in front of its text: "lead: text".
    #[must_use]
    pub fn context(self, lead: &str) -> Self {
        Self::Message(format!("{lead}: {self}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(m) => f.write_str(m),
            Self::Status(s) => s.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Message(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self::Message(s.to_owned())
    }
}

/// A system's refusal: it answered, with a status that is not a success. The text is what the agent reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusError {
    pub system: String,
    pub code: u16,
    /// "429 Too Many Requests"
    pub status: String,
    /// The start of the answer, credentials removed.
    pub body: String,
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.system, self.status, self.body)
    }
}

/// `Err(Error::Message(format!(...)))`, the shape of every refusal in words.
macro_rules! fail {
    ($($arg:tt)*) => {
        Err($crate::error::Error::Message(format!($($arg)*)))
    };
}
pub(crate) use fail;

/// `Error::Message(format!(...))`.
macro_rules! err {
    ($($arg:tt)*) => {
        $crate::error::Error::Message(format!($($arg)*))
    };
}
pub(crate) use err;

/// The result of this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
