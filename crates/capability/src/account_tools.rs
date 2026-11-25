//! The Assistant's account tool: it reads which Claude accounts agents run on, so it can say why work waits. Adding an
//! account signs a login in and removing one signs it out: those are the owner's rails, buttons on the Accounts page, and
//! no agent lifts them (account_signin.rs).

use std::sync::Arc;

use serde_json::Value;

use crate::Capabilities;
use chains::Result;
use chains::tools::NoInput;
use chains::tools::{Effect, Tools, as_json};

pub(crate) const LIST_ACCOUNTS: &str = "list_accounts";

pub(crate) const LIST_ACCOUNTS_DESCRIPTION: &str = "The Claude accounts agents run on, the oldest first, each with the address it signed in as, \
whether it can run agents now and, while it cannot, what it failed with (a limit, not signed in). Agents are placed on the accounts that can run \
them, and an agent whose account is out goes on another. The owner adds and removes accounts on the Accounts page; you cannot.";

impl Capabilities {
    /// Gives the Assistant `list_accounts`.
    pub fn add_account_tools(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            LIST_ACCOUNTS,
            Effect::Reads,
            LIST_ACCOUNTS_DESCRIPTION,
            |o, _: NoInput| async move { as_json(&o.list_accounts()?) },
        );
    }

    /// What `list_accounts` answers: each account as Health reads it.
    pub(crate) fn list_accounts(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.health().accounts)?)
    }
}
