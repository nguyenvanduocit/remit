use serde::{Deserialize, Serialize};

use crate::Time;
use crate::text_enum;

/// The account the machine's own Claude Code login is: the one every server has, which holds the transcripts of every
/// session (the others link to them) and is never removed.
pub const DEFAULT_ACCOUNT: &str = "default";

/// What agents run on: a Claude Code login in a config dir of its own, or a connection of a plugin that provides
/// accounts (ADR-0071). Its id names its config dir. The owner knows a login by `email`, the address it signed in as,
/// and a plugin's by `plugin` and `connection`. Whether an account can run agents now is not stored: it is what its
/// recent turns said. A plugin account is not stored either: it is derived from the plugin's connections, which own
/// that fact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub email: String,
    pub plugin: String,
    pub connection: String,
    pub created_at: Time,
}

text_enum! {
    /// What an account is, and so who adds and removes it.
    pub enum AccountKind {
        /// The machine's own login: it stays.
        Machine = "machine",
        /// A login the owner signed in and can remove.
        Login = "login",
        /// A connection of a plugin: it comes and goes with the connection.
        Plugin = "plugin",
    }
}

/// The account the connection of plugin is. A plugin's id and a connection's name hold no dot, so the id is read one way
/// and is never the id of a login.
pub fn plugin_account(plugin: &str, connection: &str) -> Account {
    Account {
        id: format!("{plugin}.{connection}"),
        plugin: plugin.to_owned(),
        connection: connection.to_owned(),
        ..Account::default()
    }
}

impl Account {
    /// What the account is.
    pub fn kind(&self) -> AccountKind {
        if !self.plugin.is_empty() {
            AccountKind::Plugin
        } else if self.id == DEFAULT_ACCOUNT {
            AccountKind::Machine
        } else {
            AccountKind::Login
        }
    }

    /// What the owner calls the account: a plugin's by its plugin and connection, a login by the address it signed in
    /// as, else by its id (the machine's own login has none while its login names no address).
    pub fn label(&self) -> String {
        if !self.plugin.is_empty() {
            format!("{} · {}", self.plugin, self.connection)
        } else if !self.email.is_empty() {
            self.email.clone()
        } else {
            self.id.clone()
        }
    }
}
