//! An account is added by the owner in one step that passes through no agent: he presses Add account, and the server makes a
//! config dir for it and runs the sign-in there (`AccountDirs::sign_in`), putting its link in an inbox item
//! (`InboxKind::Login`) with a write-only field. The owner opens the link, signs in to Anthropic, and types the code its page
//! shows into the field (`submit_login_code`), which gives it to the sign-in process and to nothing else. When his browser
//! runs on the server's machine the process finishes by itself and the field is never used. When it ends signed in, the
//! server asks whose login it is and keeps the account by that address; a login that did not finish leaves nothing behind.
//! Signing the same address in again replaces its account, which is how one whose login ran out is signed in anew.

use std::sync::Arc;

use async_trait::async_trait;
use model::{Account, Actor, DEFAULT_ACCOUNT, InboxItem, InboxKind, InboxStatus};
use tokio_util::sync::CancellationToken;

use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::request::Service;
use crate::{Error, Request, Result};

/// What the shell does for accounts that the orchestrator cannot: make an account's config dir, sign its login in, say
/// whose login it is and take it away. The orchestrator never reads a login.
#[async_trait]
pub trait AccountDirs: Send + Sync {
    /// Makes the config dir of account id, ready for a login and for the sessions' shared transcripts. Called again for an
    /// account that exists, it changes nothing.
    async fn prepare(&self, id: &str) -> Result<(), agent::Error>;
    /// Starts signing account id in: the process that makes the login, which gives the link the owner opens.
    async fn sign_in(&self, id: &str) -> Result<Arc<dyn Login>, agent::Error>;
    /// The address account id is signed in as; an error when it is not signed in with a Claude account.
    async fn email(&self, id: &str) -> Result<String, agent::Error>;
    /// Signs account id out and removes its config dir. The default account is never discarded.
    async fn discard(&self, id: &str) -> Result<(), agent::Error>;
}

/// A sign-in in progress (`agent::Login`): the link the owner opens, the code its page shows that he gives back, and
/// whether it ended signed in. It holds the login's secrets; nothing of the orchestrator's does.
#[async_trait]
pub trait Login: Send + Sync {
    fn url(&self) -> String;
    async fn code(&self, code: &str) -> Result<(), agent::Error>;
    /// Resolves when the sign-in ended.
    fn done(&self) -> agent::BoxFuture<'static, ()>;
    /// Why it ended without signing in; `None` when it signed in. Read after `done`.
    fn err(&self) -> Option<String>;
    fn cancel(&self);
}

#[async_trait]
impl AccountDirs for agent::ClaudeAccounts {
    async fn prepare(&self, id: &str) -> Result<(), agent::Error> {
        agent::ClaudeAccounts::prepare(self, id)
    }
    async fn sign_in(&self, id: &str) -> Result<Arc<dyn Login>, agent::Error> {
        Ok(Arc::new(agent::ClaudeAccounts::sign_in(self, id).await?))
    }
    async fn email(&self, id: &str) -> Result<String, agent::Error> {
        agent::ClaudeAccounts::email(self, id).await
    }
    async fn discard(&self, id: &str) -> Result<(), agent::Error> {
        agent::ClaudeAccounts::discard(self, id).await
    }
}

#[async_trait]
impl Login for agent::Login {
    fn url(&self) -> String {
        agent::Login::url(self).to_owned()
    }
    async fn code(&self, code: &str) -> Result<(), agent::Error> {
        Ok(agent::Login::code(self, code).await?)
    }
    fn done(&self) -> agent::BoxFuture<'static, ()> {
        Box::pin(agent::Login::done(self))
    }
    fn err(&self) -> Option<String> {
        agent::Login::err(self)
    }
    fn cancel(&self) {
        agent::Login::cancel(self);
    }
}

/// A sign-in in progress: the item the owner answers it in and the process.
#[derive(Clone)]
pub struct AccountLogin {
    pub item: String,
    pub login: Arc<dyn Login>,
}

/// What the owner reads under a login item: what to do with its link, and what to give back.
const LOGIN_DETAIL: &str = "Open the sign-in page, sign in to the Claude account to add, and type the code the page shows into the field of this item. \
If your browser runs on the server's machine, signing in finishes by itself. The code goes to the sign-in and nowhere else: no agent sees it.";

impl Core {
    /// AddAccount is the owner pressing Add account: it starts adding an account and returns the inbox item that holds
    /// the link of its sign-in.
    pub async fn add_account(self: &Arc<Self>, req: Request) -> Result<InboxItem> {
        self.act(req, "AddAccount", |o| async move { o.sign_in_new_account().await })
            .await
    }

    /// Makes a config dir for a new account and starts its sign-in there. One sign-in runs at a time: while it is open,
    /// asking again returns its item.
    async fn sign_in_new_account(self: &Arc<Self>) -> Result<InboxItem> {
        let _accounts = self.account_mu.lock().await;
        let running = self.shared().signing.values().next().map(|r| r.item.clone());
        if let Some(item) = running {
            return Ok(self.store.read(move |r| r.inbox_item(&item)).await?);
        }
        let Some(dirs) = self.cfg.accounts.clone() else {
            return Err(Error::refused("this server has no way to sign an account in"));
        };
        let assistant = self.the_assistant()?;
        let id = new_id("a_");
        let login = self.start_sign_in(dirs.as_ref(), &id).await?;
        let item = InboxItem {
            id: new_id("i_"),
            home: assistant.id,
            card_id: assistant.card_id,
            kind: InboxKind::Login,
            title: "Add a Claude account".to_owned(),
            detail: LOGIN_DETAIL.to_owned(),
            status: InboxStatus::Open,
            created_at: self.now(),
            account: id.clone(),
            link: login.url(),
            ..InboxItem::default()
        };
        let item = match self.store.update(move |tx| tx.put_inbox(item)).await {
            Ok(item) => item,
            Err(e) => {
                login.cancel();
                self.discard(dirs.as_ref(), &id).await;
                return Err(e.into());
            }
        };
        self.shared().signing.insert(
            id.clone(),
            AccountLogin {
                item: item.id.clone(),
                login: Arc::clone(&login),
            },
        );
        self.notify.inbox_updated(&item);
        let (o, item_id, finishing) = (Arc::clone(self), item.id.clone(), Arc::clone(&login));
        let started = self.background(move |ends| async move {
            o.finish_sign_in(&ends, dirs.as_ref(), &id, &item_id, finishing.as_ref())
                .await;
        });
        if !started {
            // The server began to stop between the start and now: the sign-in ends with it, as finish_sign_in ends it.
            login.cancel();
        }
        Ok(item)
    }

    /// Makes the config dir of account id and starts the sign-in in it; a dir whose sign-in did not start is removed.
    async fn start_sign_in(&self, dirs: &dyn AccountDirs, id: &str) -> Result<Arc<dyn Login>> {
        if let Err(e) = dirs.prepare(id).await {
            self.discard(dirs, id).await;
            return Err(Error::refused(format!("make the config dir of the new account: {e}")));
        }
        match dirs.sign_in(id).await {
            Ok(login) => Ok(login),
            Err(e) => {
                self.discard(dirs, id).await;
                Err(Error::refused(format!("start the sign-in: {e}")))
            }
        }
    }

    /// Takes account id's config dir and login off the machine, and logs what could not be removed: the account it
    /// belonged to is not kept either way.
    pub async fn discard(&self, dirs: &dyn AccountDirs, id: &str) {
        if let Err(e) = dirs.discard(id).await {
            tracing::error!(account = id, err = %e, "remove the config dir of an account that was not kept");
        }
    }

    /// Waits for the sign-in of the new account id to end and closes its item: answered when it signed in, and then the
    /// account is kept by its address and asked whether it answers; cancelled, with why, when it did not, and its config
    /// dir is removed. A server that stops ends it.
    async fn finish_sign_in(
        &self,
        ends: &CancellationToken,
        dirs: &dyn AccountDirs,
        id: &str,
        item_id: &str,
        login: &dyn Login,
    ) {
        let mut failure = tokio::select! {
            () = login.done() => login.err(),
            () = self.stopped.cancelled() => {
                login.cancel();
                Some("the server stopped".to_owned())
            }
            () = ends.cancelled() => {
                login.cancel();
                Some("the server stopped".to_owned())
            }
        };
        let mut kept = None;
        if failure.is_none() {
            match self.keep_signed_in(dirs, id).await {
                Ok(a) => kept = Some(a),
                Err(e) => failure = Some(e.to_string()),
            }
        }
        if failure.is_some() {
            self.discard(dirs, id).await;
        }
        self.shared().signing.remove(id);
        let wanted = item_id.to_owned();
        let mut item = match self.store.read(move |r| r.inbox_item(&wanted)).await {
            Ok(item) => item,
            Err(e) => {
                tracing::error!(account = id, item = item_id, err = %e, "close the sign-in item");
                return;
            }
        };
        item.status = InboxStatus::Answered;
        item.answered_at = Some(self.now());
        match (&failure, &kept) {
            (Some(why), _) => {
                item.status = InboxStatus::Cancelled;
                item.detail = format!("The sign-in did not finish ({why}). Press Add account to try again.");
            }
            (None, Some(a)) => item.answer = format!("signed in as {}", a.email),
            (None, None) => {}
        }
        let item = match self.store.update(move |tx| tx.put_inbox(item)).await {
            Ok(item) => item,
            Err(e) => {
                tracing::error!(account = id, item = item_id, err = %e, "close the sign-in item");
                return;
            }
        };
        self.notify.inbox_updated(&item);
        if let Some(a) = kept {
            self.probe_account(ends, &a.id).await;
        }
    }

    /// Keeps the account of config dir id under the address it signed in as. An address that has an account already takes
    /// the new login in its place: the old account is removed and the cards on it move, so signing in again repairs one
    /// whose login ran out. The default account, the machine's own login, is not replaced: its address signed in is an
    /// account of its own.
    async fn keep_signed_in(&self, dirs: &dyn AccountDirs, id: &str) -> Result<Account> {
        let email = dirs
            .email(id)
            .await
            .map_err(|e| Error::refused(format!("signed in, but whose account is not known: {e}")))?;
        let replaced;
        let a;
        {
            let _accounts = self.account_mu.lock().await;
            let mut kept = Account {
                id: id.to_owned(),
                email: email.clone(),
                created_at: self.now(),
                ..Account::default()
            };
            let mut old_id = None;
            for old in self.account_list() {
                // the machine's own login is not replaced
                if old.id != DEFAULT_ACCOUNT && old.email.to_lowercase() == email.to_lowercase() {
                    kept.created_at = old.created_at;
                    old_id = Some(old.id);
                }
            }
            self.keep_account(kept.clone()).await?;
            (replaced, a) = (old_id, kept);
        }
        if let Some(old) = replaced
            && let Err(e) = self.remove_account(&old).await
        {
            tracing::error!(account = old, err = %e, "remove the account a new sign-in replaced");
        }
        Ok(a)
    }

    /// Gives the sign-in of a login item the code the owner typed, and nothing else: it is stored nowhere, the item stays
    /// open until the sign-in ends, and no event, decision or log carries it. Only the owner gives one.
    pub async fn submit_login_code(&self, item_id: &str, code: &str, by: &Actor) -> Result<()> {
        crate::owner_acts::for_owner(crate::owner_acts::ACT_SIGN_IN_CODE, by)?;
        let wanted = item_id.to_owned();
        let item = match self.store.read(move |r| r.inbox_item(&wanted)).await {
            Ok(item) => item,
            Err(store::Error::NotFound) => return Err(Error::refused(format!("no item {item_id}"))),
            Err(e) => return Err(e.into()),
        };
        let running = self.shared().signing.get(&item.account).cloned();
        let Some(running) = running.filter(|r| r.item == item_id) else {
            return Err(Error::refused(
                "this sign-in is no longer waiting for a code: ask the Assistant to sign the account in again",
            ));
        };
        running
            .login
            .code(code)
            .await
            .map_err(|e| Error::refused(e.to_string()))
    }
}
