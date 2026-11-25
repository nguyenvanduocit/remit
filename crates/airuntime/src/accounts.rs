//! The accounts' config dirs and logins, which live with the runtime that runs sessions on them: what the core asks of
//! them, and the sign-ins in progress.

use agent::BoxFuture;

/// The accounts' config dirs and logins (`agent::ClaudeAccounts` in production).
pub trait Accounts: Send + Sync {
    fn prepare<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), agent::Error>>;
    fn sign_in<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Box<dyn Login>, agent::Error>>;
    fn email<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<String, agent::Error>>;
    fn discard<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), agent::Error>>;
}

/// A sign-in in progress (`claudecode::Login` in production).
pub trait Login: Send + Sync {
    fn url(&self) -> &str;
    fn code<'a>(&'a self, code: &'a str) -> BoxFuture<'a, Result<(), agent::Error>>;
    /// Resolves once the sign-in has ended.
    fn done(&self) -> BoxFuture<'static, ()>;
    /// Why it ended without signing in; none when it signed in.
    fn err(&self) -> Option<String>;
    fn cancel(&self);
}

impl Accounts for agent::ClaudeAccounts {
    fn prepare<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), agent::Error>> {
        Box::pin(async move { agent::ClaudeAccounts::prepare(self, id) })
    }

    fn sign_in<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Box<dyn Login>, agent::Error>> {
        Box::pin(async move {
            let login: Box<dyn Login> = Box::new(agent::ClaudeAccounts::sign_in(self, id).await?);
            Ok(login)
        })
    }

    fn email<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<String, agent::Error>> {
        Box::pin(agent::ClaudeAccounts::email(self, id))
    }

    fn discard<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), agent::Error>> {
        Box::pin(agent::ClaudeAccounts::discard(self, id))
    }
}

impl Login for claudecode::Login {
    fn url(&self) -> &str {
        claudecode::Login::url(self)
    }

    fn code<'a>(&'a self, code: &'a str) -> BoxFuture<'a, Result<(), agent::Error>> {
        Box::pin(async move { Ok(claudecode::Login::code(self, code).await?) })
    }

    fn done(&self) -> BoxFuture<'static, ()> {
        Box::pin(claudecode::Login::done(self))
    }

    fn err(&self) -> Option<String> {
        claudecode::Login::err(self)
    }

    fn cancel(&self) {
        claudecode::Login::cancel(self);
    }
}
