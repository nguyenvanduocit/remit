use model::{Account, AccountKind, DEFAULT_ACCOUNT};
use rusqlite::params;

use crate::store::{Error, Reader, Result, Tx, doc, ms};

impl Tx<'_> {
    /// Stores the login a, replacing the one with its id. A plugin's account is refused: its connection owns that fact,
    /// and a copy here would outlive it.
    pub fn put_account(&self, a: &Account) -> Result<()> {
        if a.kind() == AccountKind::Plugin {
            return Err(Error::PluginAccount);
        }
        self.exec(
            "INSERT INTO accounts(id, created_at, doc) VALUES(?, ?, ?) ON CONFLICT(id) DO UPDATE SET created_at=excluded.created_at, doc=excluded.doc",
            params![a.id, ms(a.created_at), doc(a)?],
        )?;
        Ok(())
    }

    /// Removes the account id, and the default account never: it is the machine's own login and holds the sessions'
    /// transcripts.
    pub fn delete_account(&self, id: &str) -> Result<()> {
        if id == DEFAULT_ACCOUNT {
            return Err(Error::DefaultAccount);
        }
        self.exec("DELETE FROM accounts WHERE id=?", [id])?;
        Ok(())
    }
}

impl Reader<'_> {
    /// Every account, the oldest first, the default account before any made the same millisecond.
    pub fn accounts(&self) -> Result<Vec<Account>> {
        self.list(
            "SELECT doc FROM accounts ORDER BY created_at, id = 'default' DESC, id",
            [],
        )
    }
}
