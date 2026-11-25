//! A Claude subscription signed in by its OAuth token (ADR-0129): each connection is an account agents run on, beside the
//! logins the owner signs in on this machine. The owner makes the token himself with `claude setup-token` (it is good for a
//! year) and types it into the write-only field of the connection's credential item; a session on the account runs with it
//! as `CLAUDE_CODE_OAUTH_TOKEN`, which Claude Code reads in place of its config dir's login. The plugin has no tool: it
//! gives nothing to a chain, only the account.

use std::collections::BTreeMap;

use plugin_sdk::{Account, Call, Definition, Error, Var};
use source::CAP_DATA;

/// The variable the token is typed into, and the one Claude Code reads it from.
const TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// The bundled plugin whose connections are Claude subscriptions.
pub(crate) fn plugin() -> Definition {
    Definition {
        label: "Claude subscription".into(),
        // It has no tool, so the capability gives a chain nothing; the catalog asks for one.
        capability: CAP_DATA.into(),
        vars: vec![Var {
            name: TOKEN.into(),
            what: "the OAuth token `claude setup-token` prints after you sign in to the Claude account (sk-ant-oat01-…, good for a year)".into(),
            secret: true,
            ..Default::default()
        }],
        account: Some(Account::new(account)),
        ..Default::default()
    }
}

/// The environment of a session on the connection's account: its token, trimmed of what a paste brings along.
async fn account(call: Call) -> Result<BTreeMap<String, String>, Error> {
    let token = call.var(TOKEN).trim();
    if token.is_empty() {
        return Err(
            format!("the connection has no {TOKEN}: type the token of `claude setup-token` into its item").into(),
        );
    }
    Ok(BTreeMap::from([(TOKEN.to_owned(), token.to_owned())]))
}

#[cfg(test)]
mod tests {
    use plugin_sdk::Plugin;

    use super::*;

    /// The rail of the account: the token the owner typed is what the session runs with, under the one name Claude Code
    /// reads, and nothing else rides along.
    #[tokio::test]
    async fn a_session_runs_with_the_token_the_owner_typed() {
        let vars = BTreeMap::from([(TOKEN.to_owned(), "  sk-ant-oat01-x \n".to_owned())]);
        let env = plugin().account(vars).await.unwrap();
        assert_eq!(env, BTreeMap::from([(TOKEN.to_owned(), "sk-ant-oat01-x".to_owned())]));
    }
}
