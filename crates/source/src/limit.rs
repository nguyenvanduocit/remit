//! How many rows, documents or items an agent may ask a tool for. Every tool that takes a count reads it here, so a count of
//! zero, one past the ceiling and a negative one mean the same thing whichever tool is asked.

use crate::error::{Result, fail};

/// The count a tool reads when an agent gives none, and the most it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub default: usize,
    pub max: usize,
}

impl Limit {
    #[must_use]
    pub const fn new(default: usize, max: usize) -> Self {
        Self { default, max }
    }

    /// The count to read for `asked`, the input named `what`: the default for zero (the agent left it out), `asked` from 1 to
    /// the ceiling, and refused otherwise, so an agent is told the range rather than given less than it asked for unsaid.
    pub fn of(self, what: &str, asked: i64) -> Result<usize> {
        match usize::try_from(asked) {
            Ok(0) => Ok(self.default),
            Ok(n) if n <= self.max => Ok(n),
            _ => fail!(
                "{what} {asked} is out of range: use 1 to {} (0 for {})",
                self.max,
                self.default
            ),
        }
    }
}
