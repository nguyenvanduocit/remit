//! What a save of a library record (a playbook, an agent, a skill) is made over. The store writes a record only over the
//! revision it was read at, so the save names which one that is, and it is decided in the transaction that writes.

use chains::error::unmoved;
use chains::{Error, Result};

/// What a save writes over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// Nothing: the record is new, and an id in use is refused, never overwritten.
    New,
    /// The revision its writer read: a record written since is refused (`unmoved`), and so is one gone.
    Read(i64),
    /// Whatever is stored, if anything: its writer replaces it on purpose (an import that replaces).
    Stored,
}

/// How a refusal of a save names the record: what it is, its id, and the tools that make a new one and reread one.
pub struct Named<'a> {
    pub what: &'static str,
    pub id: &'a str,
    pub create: &'static str,
    pub update: &'static str,
    pub reread: &'static str,
}

impl Over {
    /// The revision a save is written over, the record stored being at stored (none: there is none), or why it is
    /// refused. Pure.
    pub fn rev(self, stored: Option<i64>, n: &Named<'_>) -> Result<i64> {
        match (self, stored) {
            (Over::New, Some(_)) => Err(Error::refused(format!(
                "{} {:?} already exists: pick another id, or change it with {}",
                n.what, n.id, n.update
            ))),
            (Over::Read(_), None) => Err(Error::refused(format!(
                "no {} {:?}: {} makes a new one",
                n.what, n.id, n.create
            ))),
            (Over::Read(read), Some(now)) => {
                unmoved(n.what, n.id, read, now, n.reread)?;
                Ok(now)
            }
            (Over::Stored, Some(now)) => Ok(now),
            (Over::New | Over::Stored, None) => Ok(0),
        }
    }
}
