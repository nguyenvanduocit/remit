//! The owner's settings are one row of the settings table (migration 76), one typed document.

use model::Settings;
use rusqlite::params;

use crate::store::{Error, Reader, Result, Tx, doc, over_read_rev};

/// The id of the one row the owner's settings are.
const SETTINGS_ID: &str = "owner";

impl Reader<'_> {
    /// The owner's settings: the default Settings when none is stored.
    pub fn settings(&self) -> Result<Settings> {
        match self.one("SELECT doc FROM settings WHERE id=?", [SETTINGS_ID]) {
            Err(Error::NotFound) => Ok(Settings::default()),
            other => other,
        }
    }
}

impl Tx<'_> {
    /// Writes s over the revision it carries (0: none stored yet) and returns it at its new revision. `Error::Conflict`
    /// when the stored settings are at another revision: a change made since would otherwise be lost.
    pub fn put_settings(&self, mut s: Settings) -> Result<Settings> {
        s.rev = s.rev.saturating_add(1);
        s.rev = self.put_doc(
            concat!(
                "INSERT INTO settings(id, doc) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("settings"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![SETTINGS_ID, doc(&s)?],
        )?;
        Ok(s)
    }
}
