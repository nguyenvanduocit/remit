//! A plugin provides accounts (ADR-0071): each connection of it whose credential the owner typed is an account agents run
//! on, beside the logins he signed in. The connection owns that fact, so the account is derived here and stored nowhere
//! (`plugin_accounts`, `use_plugin_accounts`). Such an account has a config dir of its own that nobody signs in, and what
//! authenticates a session on it is the variables its plugin gives for the connection, asked when the session opens
//! (`open_session`) and held by the session alone. Choosing an account, the breaker, the probe and failover do not know the
//! difference: a plugin's account is an account.

use std::sync::Arc;

use grant::{Decision, Effect, Refusal};
use model::{Account, AccountKind};
use source::{Action, Caller, Extension, Form, Status};

use crate::chain_core::Core;
use crate::{Error, Result};
use plugin::Spec;
use plugin_sdk::Plugin;
use source::PluginRef;

/// Who Remit is to the gate when it opens a session on a plugin's account: the Caller of that lend, in no chain.
pub const ACCOUNT_AGENT: &str = "account";

/// The accounts the connections are: one for each connection that lacks no variable, of a plugin that provides accounts,
/// by id.
pub fn plugin_accounts(plugins: &[String], connections: &[Status]) -> Vec<Account> {
    let mut out: Vec<Account> = connections
        .iter()
        .filter(|c| c.missing.is_empty() && plugins.contains(&c.source))
        .map(|c| model::plugin_account(&c.source, &c.connection))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Puts accounts in the order agents are placed by: the logins, the oldest first, then the plugins' accounts by id, so of
/// two accounts as busy the owner's own login takes the work.
pub fn order_accounts(accounts: &mut [Account]) {
    accounts.sort_by(
        |x, y| match (x.kind() == AccountKind::Plugin, y.kind() == AccountKind::Plugin) {
            (true, true) => x.id.cmp(&y.id),
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => x.created_at.cmp(&y.created_at),
        },
    );
}

impl Core {
    /// Makes exts, the enabled extensions, the plugins whose connections are accounts, in place of the ones before.
    pub fn use_account_plugins(&self, exts: &[Extension]) {
        self.shared().account_plugins = exts.iter().filter(|e| e.accounts).map(|e| e.id.clone()).collect();
    }

    /// Reads the accounts the plugins' connections are now.
    pub async fn derived_accounts(&self) -> Result<Vec<Account>> {
        let plugins = self.shared().account_plugins.clone();
        if plugins.is_empty() {
            return Ok(Vec::new());
        }
        let connections = self.sources.status().await.map_err(|e| Error::refused(e.to_string()))?;
        Ok(plugin_accounts(&plugins, &connections))
    }

    /// Makes the accounts in use follow the plugins' connections, after an extension or a connection's values changed. An
    /// account that appears gets its config dir and is out of use until it answers a probe, which is asked at once, so no
    /// card is placed on a credential nobody has confirmed. One that went is forgotten and its dir removed; a card on it goes
    /// on another account at its next session (`choose_account`).
    pub async fn use_plugin_accounts(self: &Arc<Self>) {
        let _accounts = self.account_mu.lock().await;
        let next = match self.derived_accounts().await {
            Ok(next) => next,
            Err(e) => {
                tracing::error!(err = %e, "the plugins' accounts were not read: the accounts stay as they were");
                return;
            }
        };
        let (had, mut accounts): (Vec<Account>, Vec<Account>) = self
            .account_list()
            .into_iter()
            .partition(|a| a.kind() == AccountKind::Plugin);
        accounts.extend(next.iter().cloned());
        order_accounts(&mut accounts);
        self.set_accounts(accounts);
        let listed = |of: &[Account], id: &str| of.iter().any(|a| a.id == id);
        let mut changed = false;
        for a in had.iter().filter(|a| !listed(&next, &a.id)) {
            changed = true;
            self.outages.forget(&a.id);
            if let Some(dirs) = &self.cfg.accounts {
                self.discard(dirs.as_ref(), &a.id).await;
            }
        }
        for a in next.iter().filter(|a| !listed(&had, &a.id)) {
            changed = true;
            if let Some(dirs) = &self.cfg.accounts
                && let Err(e) = dirs.prepare(&a.id).await
            {
                tracing::error!(account = a.id, err = %e, "prepare the config dir of a plugin's account");
            }
            let unchecked = wire::runtime::Failure {
                category: wire::runtime::FailureCategory::Access,
                title: "not checked yet".to_owned(),
                resets_at: None,
            };
            self.outages.fail(&a.id, &unchecked, self.now());
            let (o, id) = (Arc::clone(self), a.id.clone());
            self.background(move |ends| async move { o.probe_account(&ends, &id).await });
        }
        if changed {
            self.notify.health_updated(&self.health());
        }
    }

    /// The account id among those in use.
    pub fn account_by_id(&self, id: &str) -> Option<Account> {
        self.account_list().into_iter().find(|a| a.id == id)
    }

    /// Starts the session spec names: the one way a session opens, for a card and a probe. On a plugin's
    /// account it first asks the plugin for the variables the session runs with. A plugin that cannot give them, or gives
    /// a name outside the list, is that account's failure, as a login that was signed out is: the breaker takes the account
    /// out and the card goes on another.
    pub async fn open_session(
        &self,
        mut spec: agent::Spec,
        h: Arc<dyn agent::Handler>,
    ) -> Result<Box<dyn agent::Session>, agent::Error> {
        if let Some(a) = self.account_by_id(&spec.account)
            && a.kind() == AccountKind::Plugin
        {
            spec.account_env = self.account_env(&a).await.map_err(|e| {
                agent::Error::Failure(wire::runtime::Failure {
                    category: wire::runtime::FailureCategory::Access,
                    title: format!(
                        "the plugin {} gave no account for its connection {}: {e}",
                        a.plugin, a.connection
                    ),
                    resets_at: None,
                })
            })?;
        }
        self.launcher.start(spec, h).await
    }

    /// What the plugin of account a gives a session to run with, asked now: the connection is lent through the gate to
    /// Remit as the opener of a session, so a key the owner changed or a token the plugin mints is the one the session
    /// gets, and Remit keeps none.
    async fn account_env(&self, a: &Account) -> Result<agent::AccountEnv, String> {
        let opener = Caller {
            agent: ACCOUNT_AGENT.to_owned(),
            ..Caller::default()
        };
        let (plugin, vars) = self
            .sources
            .lend(&opener, &a.plugin, &a.connection, source::ACCOUNT_CALL, Effect::Read)
            .await
            .map_err(|e| e.to_string())?;
        let env = self
            .plugin_of_ref(&plugin)
            .map_err(|e| e.to_string())?
            .account(vars)
            .await
            .map_err(|f| f.to_string())?;
        agent::parse_account_env(env).map_err(|e| e.to_string())
    }

    /// The gate's verdict for Remit opening a session: a read, by a typed call, of a connection that is an account now.
    /// Nothing else is lent to it.
    pub fn account_verdict(&self, a: &Action) -> Decision {
        if a.effect != Effect::Read || a.form != Form::Tool {
            return Decision::Deny {
                reason: "a session's opener only reads, by a typed call".to_owned(),
                refusal: Refusal::Rail,
            };
        }
        if self
            .account_by_id(&model::plugin_account(&a.source, &a.connection).id)
            .is_none()
        {
            return Decision::Deny {
                reason: format!("{}/{} is no account", a.source, a.connection),
                refusal: Refusal::NoGrant,
            };
        }
        Decision::Allow
    }
}

impl Core {
    /// The running plugin a source names: a bundled one in-process, or the program the owner applied, with its hash.
    pub fn plugin_of_ref(&self, p: &PluginRef) -> Result<Arc<dyn Plugin>> {
        if p.bundled {
            return self
                .bundled
                .lookup(&p.id)
                .ok_or_else(|| Error::refused(format!("no plugin {:?} ships with Remit", p.id)));
        }
        Ok(self.plugins.plugin(&Spec {
            id: p.id.clone(),
            binary: p.binary.clone(),
            args: Vec::new(),
            sha256: p.sha256.clone(),
        }))
    }
}
