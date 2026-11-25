//! Accounts are the Claude Code logins agents run on (ADR-0040). Each is a config dir of its own and its own limits. A card
//! runs on one account (`Card::account`) and moves to another that can run agents when its own is out (failover, wait): the
//! sessions' transcripts are shared between the accounts' config dirs, so the session goes on where it left off. Whether
//! an account can run agents is what `outages` knows of it (health.rs). Which account a card runs on is decided here,
//! once (`account_for`); the config dirs themselves are the shell's (`AccountDirs`).

use std::collections::HashMap;
use std::sync::Arc;

use model::{Account, AccountKind, Card, DEFAULT_ACCOUNT};

use crate::chain_core::Core;
use crate::plugin_accounts::order_accounts;
use crate::request::Service;
use crate::{Done, Error, Request, Result};

/// The account every server has before any is read: the machine's own login.
pub fn default_account() -> Account {
    Account {
        id: DEFAULT_ACCOUNT.to_owned(),
        ..Account::default()
    }
}

/// What an account can take now, as what is known of it says (health.rs's `Outages::standing`), the worst first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Standing {
    /// It cannot run agents: it failed, and has not answered since.
    Out,
    /// It runs agents, and the provider warns it is near its limit.
    Near,
    /// It runs agents, with room.
    Free,
}

/// The account a card runs on: its own, while no other stands better (one with room before one near its limit, so a card
/// leaves an account before its limit rejects its turns); else the best standing that can run agents, the one with the
/// fewest agents at work among those, the oldest of two as busy. With none able, its own (or the oldest, for a card with
/// none, or whose account is gone): its session fails to start there and the card waits for an account (failover).
/// accounts is never empty: the default account stays.
pub fn choose_account(
    own: &str,
    accounts: &[String],
    standing: &HashMap<String, Standing>,
    load: &HashMap<String, usize>,
) -> String {
    let of = |id: &str| standing.get(id).copied().unwrap_or(Standing::Out);
    let top = accounts.iter().map(|id| of(id)).max().unwrap_or(Standing::Out);
    let listed = accounts.iter().any(|a| a == own);
    if listed && of(own) != Standing::Out && of(own) == top {
        return own.to_owned();
    }
    let busy = |id: &str| load.get(id).copied().unwrap_or(0);
    let mut best: Option<&String> = None;
    for id in accounts {
        if of(id) == top && top != Standing::Out && best.is_none_or(|b| busy(id) < busy(b)) {
            best = Some(id);
        }
    }
    match best {
        Some(b) => b.clone(),
        None if listed => own.to_owned(),
        None => accounts.first().cloned().unwrap_or_else(|| DEFAULT_ACCOUNT.to_owned()),
    }
}

impl Core {
    /// Reads the accounts the owner made, at start. A database that cannot be read leaves the default account, which every
    /// server has, and says so: nothing else could start either.
    pub async fn load_accounts(&self) {
        let mut accounts = match self.store.read(|r| r.accounts()).await {
            Ok(accounts) if !accounts.is_empty() => accounts,
            got => {
                let err = got.err().map(|e: store::Error| e.to_string()).unwrap_or_default();
                tracing::error!(err, "read the accounts: the default account alone runs agents");
                vec![default_account()]
            }
        };
        match self.derived_accounts().await {
            Ok(derived) => accounts.extend(derived),
            Err(err) => tracing::error!(%err, "read the plugins' accounts: agents run on the logins alone"),
        }
        order_accounts(&mut accounts);
        self.shared().accounts = accounts;
    }

    /// Makes sure every account has its config dir, at start, and that the default account is known by its login's
    /// address: a dir restored from a backup, or whose link to the transcripts was lost, is made ready again. An account
    /// that cannot be prepared is logged, and agents run on the others.
    pub async fn prepare_accounts(&self) {
        let Some(dirs) = self.cfg.accounts.clone() else { return };
        for a in self.account_list() {
            if let Err(err) = dirs.prepare(&a.id).await {
                tracing::error!(account = a.id, %err, "prepare the config dir of an account");
            }
        }
        self.name_default_account(dirs.as_ref()).await;
    }

    /// Keeps on the default account the address the machine's login is signed in as, read at start: it is the one account
    /// Remit does not sign in itself, so no sign-in recorded who it is. A login with no address to name (signed out, an API
    /// key) leaves the account as it was; a restart reads the login again.
    async fn name_default_account(&self, dirs: &dyn crate::account_signin::AccountDirs) {
        let email = match dirs.email(DEFAULT_ACCOUNT).await {
            Ok(email) => email,
            Err(why) => {
                tracing::info!(%why, "the default account's login names no address");
                return;
            }
        };
        let _accounts = self.account_mu.lock().await;
        for mut a in self.account_list() {
            if a.id != DEFAULT_ACCOUNT || a.email == email {
                continue;
            }
            a.email.clone_from(&email);
            if let Err(err) = self.remember_account(a).await {
                tracing::error!(%err, "keep the default account's address");
                return;
            }
            self.notify.health_updated(&self.health());
        }
    }

    /// The accounts, the oldest first (the default account is always the first).
    pub fn account_list(&self) -> Vec<Account> {
        self.shared().accounts.clone()
    }

    /// Makes accounts the accounts in use, in their order; the caller holds `account_mu`.
    pub fn set_accounts(&self, accounts: Vec<Account>) {
        self.shared().accounts = accounts;
    }

    /// The account card c runs on now: the one `choose_account` chooses from where the accounts stand and how many agents
    /// work on each.
    pub fn account_for(&self, c: &Card) -> String {
        let ids: Vec<String> = self.account_list().into_iter().map(|a| a.id).collect();
        let now = self.now();
        let standing: HashMap<String, Standing> = ids
            .iter()
            .map(|id| (id.clone(), self.outages.standing(id, now)))
            .collect();
        let mut load: HashMap<String, usize> = HashMap::new();
        for c in self.all_runs().iter().map(|r| r.card_now()) {
            if crate::spawn::spawned(&c) {
                continue; // a child card runs on its parent's account
            }
            let n = load.entry(c.account).or_default();
            *n = n.saturating_add(1);
        }
        choose_account(&c.account, &ids, &standing, &load)
    }

    /// Has card c's run on the account `account_for` chooses and stores it on the card before a session is tried there, so
    /// a failure of that session is charged to the account it ran on (failover).
    pub async fn bind_account(&self, c: Card) -> Result<Card> {
        let _place = self.place_mu.lock().await;
        let account = self.account_for(&c);
        if account == c.account {
            return Ok(c);
        }
        self.update_card(&c.id, move |c| c.account.clone_from(&account)).await
    }

    /// Keeps a to run agents on; the caller holds `account_mu`. It is out of use until it answers a probe (Check again, or
    /// the probe a sign-in ends with), so no card is placed on a login nobody has confirmed.
    pub async fn keep_account(&self, a: Account) -> Result<()> {
        let id = a.id.clone();
        self.remember_account(a).await?;
        self.account_failed(
            &id,
            &wire::runtime::Failure {
                category: wire::runtime::FailureCategory::Access,
                title: "not signed in yet".to_owned(),
                resets_at: None,
            },
        );
        Ok(())
    }

    /// Stores a and has the accounts in use list it in place of the one with its id; the caller holds `account_mu`.
    pub async fn remember_account(&self, a: Account) -> Result<()> {
        let stored = a.clone();
        self.store.update(move |tx| tx.put_account(&stored)).await?;
        let mut sh = self.shared();
        sh.accounts.retain(|x| x.id != a.id);
        sh.accounts.push(a);
        order_accounts(&mut sh.accounts);
        Ok(())
    }

    /// What the owner calls account id: its address, else its id.
    pub fn account_label(&self, id: &str) -> String {
        self.account_by_id(id).map_or_else(|| id.to_owned(), |a| a.label())
    }

    /// RemoveAccount is the owner pressing Remove on an account.
    pub async fn remove_account_by(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        self.act_done(
            req,
            "RemoveAccount",
            move |o| async move { o.remove_account(&id).await },
        )
        .await
    }

    /// Stops using the account id and signs it out: no card is placed on it, a card on it moves to another at its next
    /// session, and its config dir and login are gone from the machine. The default account stays: it is the login of this
    /// machine, and every account reads its transcripts.
    pub async fn remove_account(&self, id: &str) -> Result<()> {
        let _accounts = self.account_mu.lock().await;
        let Some(a) = self.account_by_id(id) else {
            return Err(Error::refused(format!("no account {id:?}: list_accounts shows them")));
        };
        if a.kind() == AccountKind::Plugin {
            return Err(Error::refused(format!(
                "this account is the connection {} of the plugin {}: remove the connection's credential, or disable the plugin",
                a.connection, a.plugin
            )));
        }
        let gone = id.to_owned();
        match self.store.update(move |tx| tx.delete_account(&gone)).await {
            Ok(()) => {}
            Err(store::Error::DefaultAccount) => {
                return Err(Error::refused(
                    "the default account stays: it is the login of this machine, and every account reads its transcripts",
                ));
            }
            Err(e) => return Err(e.into()),
        }
        self.shared().accounts.retain(|a| a.id != id);
        self.outages.forget(id);
        self.notify.health_updated(&self.health());
        let Some(dirs) = self.cfg.accounts.clone() else {
            return Ok(());
        };
        dirs.discard(id)
            .await
            .map_err(|e| Error::refused(format!("account {id} is removed, but signing it out failed: {e}")))
    }
}
