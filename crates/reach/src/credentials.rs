//! A source's credentials are typed by the owner into the write-only field of an inbox item: a value never passes through an
//! agent's context. A secret is sealed into the store, and the only code that reads it back is the source store
//! (source_store.rs), which hands it to the source that uses it: not a push to a channel, a decision, an event, a transcript
//! or a log carries it. A plain variable (an address, a user) is stored as typed and shown on the Tools page.
//!
//! The limit: an agent runs as the same OS user as this server, so it can read what the server reads (the database, the key
//! file). Sealing protects the database file and its backups, not the host from an agent that tries.
//!
//! The key the values are sealed with is read from its file at each use, never cached: a key file that was restored or
//! fixed is used at once, and a failure is never remembered. Each sealed value records the id of the key that sealed it
//! (`secrets::key_id`), and the key is backed up under that id before anything is sealed under it, so a value whose key is
//! missing or another names the key it needs and where its backup is, and is kept until that key is back: nothing is ever
//! cleared for a key.

use std::sync::LazyLock;
use wire::app::{ConnectionView, SourceVarState, SourceVariable, ToolSource};

use model::{Actor, Assistant, InboxItem, InboxKind, InboxOption, InboxStatus, SourceConnection, Time};
use regex::Regex;
use schemars::JsonSchema;
use serde::Deserialize;
use source::DEFAULT_CONNECTION;
use store::SourceValue;

use crate::Reach;
use chains::chain_core::new_id;
use chains::owner_acts::{ACT_CREDENTIAL, for_owner};
use chains::{Error, Result};

/// The answers of a credential item besides a value: the owner removes what is stored.
pub(crate) const REMOVE_VALUE_OPTION: &str = "remove-value";
pub(crate) const REMOVE_CONNECTION_OPTION: &str = "remove-connection";

static CONNECTION_NAME: LazyLock<Regex> = LazyLock::new(|| {
    #[expect(clippy::expect_used, reason = "a constant pattern a test compiles")]
    Regex::new(r"^[a-z][a-z0-9-]{0,31}$").expect("the connection name pattern compiles")
});

/// The name of the variable a sealed value belongs to, so a value moved to another variable, connection or source does
/// not open there.
pub(crate) fn secret_aad(src: &str, connection: &str, variable: &str) -> String {
    format!("{src}/{connection}/{variable}")
}

/// The name a request gives a connection: empty and "default" are the connection the environment serves, any other is a
/// word the owner can say.
pub(crate) fn connection_name(name: &str) -> Result<String> {
    if name.is_empty() || name == DEFAULT_CONNECTION {
        return Ok(DEFAULT_CONNECTION.to_owned());
    }
    if !CONNECTION_NAME.is_match(name) {
        return Err(Error::refused(format!(
            "connection name {name:?} is not valid: use lowercase letters, digits and dashes, starting with a letter, at most 32 characters, like prod or eu-west (\"default\" is the one the environment serves)"
        )));
    }
    Ok(name.to_owned())
}

/// What the Assistant or a Chair asks for: a source, the connection (empty: default), a line that says what a new
/// connection reaches, and the variables to ask for (none: what the connection still lacks). It takes names, never a value.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceCredentialsInput {
    /// the id of the source, as list_sources names it (mongo, bytebase, grafana...)
    pub source: String,
    /// the name of the connection: a lowercase word like prod or eu-west; empty is the default connection, the one the
    /// server's environment serves
    #[serde(default)]
    pub connection: String,
    /// for a new named connection, one line that says what it reaches and holds, for the agents that must choose between
    /// connections
    #[serde(default)]
    pub description: String,
    /// the variables to ask the owner for; none asks for what the connection still lacks; naming one that is set asks for
    /// a new value
    #[serde(default)]
    pub variables: Vec<String>,
}

impl Reach {
    /// Run at start: backs the key file up, and says in the log which stored sealed values the key file does not open,
    /// because it is missing or holds another key; each of those refuses when it is used, with what to do. A key file that
    /// is there and cannot be read, or backed up, stops the start: nothing would be sealed under it.
    pub async fn check_secret_key(&self) -> Result<()> {
        let file = &self.cfg.secret_key_file;
        if file.as_os_str().is_empty() {
            return Ok(());
        }
        let ids = self.store.read(|r| r.sealed_key_ids()).await?;
        let key = match secrets::load_key(file) {
            Err(e) if e.is_not_found() => {
                for id in &ids {
                    tracing::error!(key = %id, file = %file.display(), r#do = %self.restore_key(id), "sealed values cannot be opened: the key file is missing");
                }
                return Ok(());
            }
            other => other.map_err(|e| Error::refused(e.to_string()))?,
        };
        secrets::back_up_key(&key, &self.cfg.key_backup_dir).map_err(|e| Error::refused(e.to_string()))?;
        let have = secrets::key_id(&key);
        for id in ids.iter().filter(|id| **id != have) {
            tracing::error!(key = %id, file_key = %have, file = %file.display(), r#do = %self.restore_key(id), "sealed values cannot be opened: they were sealed under another key");
        }
        Ok(())
    }

    /// The owner's answer to credential item item: "Remove the stored value" or "Remove this connection" when option_id
    /// names one, else text, the value he typed.
    pub async fn answer_credential(&self, item: &str, option_id: &str, text: &str, by: &Actor) -> Result<()> {
        if option_id == REMOVE_VALUE_OPTION || option_id == REMOVE_CONNECTION_OPTION {
            return self.remove_credential(item, option_id, by).await;
        }
        self.set_credential(item, text, by).await
    }

    /// Stores the value the owner typed for the variable a credential item asks for, and closes the item: in one
    /// transaction the value is stored (sealed when it is a secret) and the item is answered with an empty answer. The
    /// value is stored as typed, never trimmed. Only the owner enters it: an agent that was told one, the Assistant
    /// included, cannot.
    pub(crate) async fn set_credential(&self, item_id: &str, value: &str, by: &Actor) -> Result<()> {
        for_owner(ACT_CREDENTIAL, by)?;
        if value.trim().is_empty() {
            return Err(Error::refused("the credential is empty: type its value into the field"));
        }
        let id = item_id.to_owned();
        let item = self
            .store
            .read(move |r| r.inbox_item(&id))
            .await
            .map_err(|e| Error::refused(format!("item {item_id}: {e}")))?;
        let aad = secret_aad(&item.source, &item.connection, &item.variable);
        let stored = if item.secret {
            let key = self
                .sealing_key()
                .await
                .map_err(|e| Error::refused(format!("the credential cannot be sealed: {e}")))?;
            let sealed =
                secrets::seal(&key, &aad, value.as_bytes()).map_err(|e| Error::refused(format!("seal {aad}: {e}")))?;
            SourceValue::Secret(sealed)
        } else {
            SourceValue::Plain(value.to_owned())
        };
        let (id, by_, now) = (item_id.to_owned(), by.clone(), self.now());
        let closed = self
            .store
            .update(move |tx| -> Result<InboxItem> {
                let open = open_credential_item(tx, &id)?;
                tx.put_source_value(&open.source, &open.connection, &open.variable, &stored)?;
                Ok(tx.put_inbox(answered(open, by_, now, ""))?)
            })
            .await?;
        self.source_changed(&item.source, &item.connection).await;
        self.notify.inbox_updated(&closed);
        Ok(())
    }

    /// The owner's answer "Remove the stored value" or "Remove this connection" to a credential item: what is stored goes,
    /// and the environment (for default) or nothing (for a named connection) answers from then on. Only the owner removes.
    pub(crate) async fn remove_credential(&self, item_id: &str, option_id: &str, by: &Actor) -> Result<()> {
        for_owner(ACT_CREDENTIAL, by)?;
        let (id, option, by_, now) = (item_id.to_owned(), option_id.to_owned(), by.clone(), self.now());
        let (item, closed) = self
            .store
            .update(move |tx| -> Result<(InboxItem, Vec<InboxItem>)> {
                let item = open_credential_item(tx, &id)?;
                let answer = match option.as_str() {
                    REMOVE_VALUE_OPTION => {
                        tx.delete_source_value(&item.source, &item.connection, &item.variable)?;
                        "removed the stored value"
                    }
                    REMOVE_CONNECTION_OPTION => {
                        if item.connection == DEFAULT_CONNECTION {
                            return Err(Error::refused(
                                "the default connection cannot be removed: remove its stored values one by one",
                            ));
                        }
                        tx.delete_source_connection(&item.source, &item.connection)?;
                        "removed the connection"
                    }
                    other => return Err(Error::refused(format!("no answer {other:?} to a credential item"))),
                };
                let mut closed = Vec::new();
                for it in tx.open_inbox()? {
                    if it.kind != InboxKind::Credential || it.source != item.source || it.connection != item.connection
                    {
                        continue;
                    }
                    if it.id != id && option == REMOVE_VALUE_OPTION {
                        continue;
                    }
                    let it = if it.id == id {
                        answered(it, by_.clone(), now, answer)
                    } else {
                        InboxItem {
                            status: InboxStatus::Cancelled, // the connection it asked for is gone
                            ..it
                        }
                    };
                    closed.push(tx.put_inbox(it)?);
                }
                Ok((item, closed))
            })
            .await?;
        self.source_changed(&item.source, &item.connection).await;
        self.tell_inbox(&closed);
        Ok(())
    }

    /// Drops what the sources built from a connection's old values and tells the channels how the source stands.
    pub(crate) async fn source_changed(&self, src: &str, connection: &str) {
        self.sources.invalidate(src, connection);
        if let Some(o) = self.me() {
            o.core.use_plugin_accounts().await;
            // an MCP server not reached yet is reached once its connection may be (mcp_servers.rs)
            let src = src.to_owned();
            tokio::spawn(async move { o.pin_mcp(&src).await });
        }
        self.tell_source(src).await;
    }

    /// Tells the channels a source as it stands now.
    pub(crate) async fn tell_source(&self, id: &str) {
        match self.sources.catalog().await {
            Ok(views) => views
                .iter()
                .filter(|v| v.id == id)
                .for_each(|v| self.notify.source_updated(v)),
            Err(err) => tracing::warn!(source = %id, %err, "the channels were not told how a source stands"),
        }
    }

    /// Makes a connection when it is new and raises one credential item for each variable it needs, plain ones first so a
    /// secret's item can say where it is sent. An item that already waits for the same variable is kept, not raised twice.
    /// The items are the Assistant's: no chain asked.
    pub(crate) async fn request_source_credentials(&self, input: SourceCredentialsInput) -> Result<Vec<InboxItem>> {
        let connection = connection_name(&input.connection)?;
        let (sv, cv) = self.connection_view(&input.source, &connection).await?;
        let names = if input.variables.is_empty() {
            cv.missing.clone()
        } else {
            input.variables.clone()
        };
        let mut asked = Vec::new();
        for name in &names {
            let Some(v) = cv.vars.iter().find(|v| v.name == *name) else {
                return Err(Error::refused(format!(
                    "{} has no variable {name:?}: it reads {}",
                    sv.id,
                    var_names(&cv.vars)
                )));
            };
            asked.push(v.clone());
        }
        asked.sort_by_key(|v| v.secret);
        let assistant = self.the_assistant()?;
        let description = if connection == DEFAULT_CONNECTION {
            String::new()
        } else {
            connection_description(&input, &connection, &cv)?
        };
        let now = self.now();
        let (sv_, connection_) = (sv.clone(), connection.clone());
        let raised = self
            .store
            .update(move |tx| -> Result<Vec<InboxItem>> {
                if connection_ != DEFAULT_CONNECTION {
                    tx.put_source_connection(&SourceConnection {
                        source: sv_.id.clone(),
                        name: connection_.clone(),
                        description,
                    })?;
                }
                let open = tx.open_inbox()?;
                let mut raised = Vec::new();
                for v in &asked {
                    if open.iter().any(|it| {
                        it.kind == InboxKind::Credential
                            && it.source == sv_.id
                            && it.connection == connection_
                            && it.variable == v.name
                    }) {
                        continue;
                    }
                    raised.push(tx.put_inbox(credential_item(&assistant, now, &sv_, &connection_, &cv, v))?);
                }
                Ok(raised)
            })
            .await?;
        self.tell_inbox(&raised);
        self.tell_source(&sv.id).await;
        Ok(raised)
    }

    /// The connection as it stands, or a blank one with what a new connection needs.
    async fn connection_view(&self, src: &str, connection: &str) -> Result<(ToolSource, ConnectionView)> {
        let views = self
            .sources
            .catalog()
            .await
            .map_err(|e| Error::refused(e.to_string()))?;
        let Some(v) = views.into_iter().find(|v| v.id == src) else {
            return Err(Error::refused(format!(
                "no source {src:?}: the ones that exist are {}",
                self.sources.source_ids().join(", ")
            )));
        };
        if let Some(c) = v.connections.iter().find(|c| c.name == connection) {
            let c = c.clone();
            return Ok((v, c));
        }
        let mut cv = self
            .sources
            .blank(src)
            .map(|(_, cv)| cv)
            .ok_or_else(|| Error::refused(format!("no source {src:?}")))?;
        connection.clone_into(&mut cv.name);
        Ok((v, cv))
    }
}

/// The item of that id, which has to be a credential request that still waits.
pub(crate) fn open_credential_item(tx: &store::Tx<'_>, item_id: &str) -> Result<InboxItem> {
    let item = tx
        .inbox_item(item_id)
        .map_err(|e| Error::refused(format!("item {item_id}: {e}")))?;
    if item.kind != InboxKind::Credential {
        return Err(Error::refused(format!("item {item_id} is not a credential request")));
    }
    if item.status != InboxStatus::Open {
        return Err(Error::refused(format!("item {item_id} is {}", item.status)));
    }
    Ok(item)
}

/// The item closed by the owner's answer, which says what he did and never a value.
pub fn answered(item: InboxItem, by: Actor, now: Time, answer: &str) -> InboxItem {
    InboxItem {
        status: InboxStatus::Answered,
        answered_at: Some(now),
        answered_by: Some(by),
        answer: answer.to_owned(),
        ..item
    }
}

fn var_names(vars: &[SourceVariable]) -> String {
    vars.iter().map(|v| v.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// The line that says what a named connection reaches: the one the request gives, else the one it has. A new connection
/// needs one, so agents that must choose between connections can.
fn connection_description(input: &SourceCredentialsInput, connection: &str, cv: &ConnectionView) -> Result<String> {
    let mut description = input.description.trim().to_owned();
    if description.is_empty() {
        description.clone_from(&cv.description);
    }
    if description.is_empty() {
        return Err(Error::refused(format!(
            "connection {connection} is new: give a one-line description of what it reaches and holds, so agents can choose between connections"
        )));
    }
    Ok(description)
}

/// Asks the owner for one variable of a connection. It names the source, the connection, the variable and where a secret
/// will be sent, never a value. What he may take away is offered as an answer.
pub(crate) fn credential_item(
    assistant: &Assistant,
    now: Time,
    sv: &ToolSource,
    connection: &str,
    cv: &ConnectionView,
    v: &SourceVariable,
) -> InboxItem {
    let mut options = Vec::new();
    if v.state == SourceVarState::Set {
        options.push(InboxOption {
            id: REMOVE_VALUE_OPTION.to_owned(),
            name: "Remove the stored value".to_owned(),
            ..InboxOption::default()
        });
    }
    if connection != DEFAULT_CONNECTION {
        options.push(InboxOption {
            id: REMOVE_CONNECTION_OPTION.to_owned(),
            name: "Remove this connection".to_owned(),
            ..InboxOption::default()
        });
    }
    InboxItem {
        id: new_id("i_"),
        home: assistant.id.clone(),
        card_id: assistant.card_id.clone(),
        kind: InboxKind::Credential,
        title: format!("Credential for {}/{connection}: {}", sv.id, v.name),
        detail: credential_detail(sv, connection, cv, v),
        options,
        status: InboxStatus::Open,
        created_at: now,
        source: sv.id.clone(),
        connection: connection.to_owned(),
        variable: v.name.clone(),
        secret: v.secret,
        ..InboxItem::default()
    }
}

/// What the owner reads under a credential request: what the value is for and, for a secret, where it is sent, so a
/// connection pointed somewhere else cannot ask for his token for the old place.
fn credential_detail(sv: &ToolSource, connection: &str, cv: &ConnectionView, v: &SourceVariable) -> String {
    let mut detail = format!(
        "The {} connection {connection} needs {} ({}) before an agent can read it.",
        sv.label, v.what, v.name
    );
    if v.secret {
        detail.push_str(&format!(
            " It is sent to {}. Type it into the field of this item: it is stored encrypted, and no agent sees it.",
            destination_of(sv, cv)
        ));
    } else {
        detail.push_str(" Type it into the field of this item: it is stored as typed and shown on the Tools page.");
    }
    detail
}

/// Where a connection sends what it holds: the address it has, the one it will have once its address variable is set, or,
/// for a source whose address is inside the value or fixed by the source, that.
fn destination_of(sv: &ToolSource, cv: &ConnectionView) -> String {
    match cv.vars.iter().find(|v| v.address) {
        Some(v) if v.state != SourceVarState::Unset && !v.value.is_empty() => v.value.clone(),
        Some(v) => format!(
            "the address in {}, which is not set yet (answer its item first)",
            v.name
        ),
        None => format!("the address inside the value, or the one {} fixes", sv.label),
    }
}
