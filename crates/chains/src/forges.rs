//! The forges a repository is hosted on (GitLab, GitHub), as a port crates/forge fills, and who Remit works as on each
//! host, which a firing asks without waiting for the forge. Remit lands a change itself (ADR-0114): of a forge it asks only
//! who it pushes as, where a base stands and the checks its pipeline ran on a commit.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError};

use async_trait::async_trait;
use forge::{Account, Repo};
use model::{Checks, Time, Worktree};

use crate::chain_core::Core;
use crate::{Error, Result};

/// A service a repository is hosted on: what Remit asks of it to land a chain's change and follow the pipeline on it.
/// crates/forge speaks to each.
#[async_trait]
pub trait Forge: Send + Sync {
    /// As the owner reads it: GitLab, GitHub.
    fn name(&self) -> &'static str;
    async fn account(&self, host: &str) -> Result<Account, forge::Error>;
    /// The commit branch stands at on the forge: the forge's word, whatever a clone says.
    async fn branch_head(&self, r: &Repo, branch: &str) -> Result<String, forge::Error>;
    /// The checks the forge's pipeline ran on commit sha; none while it has run none.
    async fn checks(&self, r: &Repo, sha: &str) -> Result<Option<Checks>, forge::Error>;
    /// Closes merge request iid with comment once: whether it was open. Written for migration 0119's carried draft MRs
    /// (`shipping::carried`) and deleted with them.
    async fn close_mr(&self, r: &Repo, iid: i64, comment: &str) -> Result<bool, forge::Error>;
}

/// Forge for each of crates/forge's clients, which hold the same methods.
macro_rules! forge_of {
    ($t:ty) => {
        #[async_trait]
        impl Forge for $t {
            fn name(&self) -> &'static str {
                <$t>::name(self)
            }
            async fn account(&self, host: &str) -> Result<Account, forge::Error> {
                <$t>::account(self, host).await
            }
            async fn branch_head(&self, r: &Repo, branch: &str) -> Result<String, forge::Error> {
                <$t>::branch_head(self, r, branch).await
            }
            async fn checks(&self, r: &Repo, sha: &str) -> Result<Option<Checks>, forge::Error> {
                <$t>::checks(self, r, sha).await
            }
            async fn close_mr(&self, r: &Repo, iid: i64, comment: &str) -> Result<bool, forge::Error> {
                <$t>::close_mr(self, r, iid, comment).await
            }
        }
    };
}

forge_of!(forge::GitHub);
forge_of!(forge::GitLab);

/// How long Remit waits for a forge to answer one call made for an event.
pub const FORGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
/// How long what a forge said of the account Remit works as on a host is taken as it is before it is asked again; the
/// earlier answer is served while the new one comes.
const ACCOUNT_FRESH: time::Duration = time::Duration::minutes(10);
/// The least time between two asks of a forge that has not answered.
const ACCOUNT_RETRY: time::Duration = time::Duration::seconds(30);

/// What Remit remembers of who it works as on one host, in memory (a restart asks again), and whether it is being asked.
#[derive(Debug, Clone, Default)]
pub struct ForgeAccount {
    /// The account the forge last said; empty until it has said.
    login: String,
    /// Why the last ask failed, until one succeeds.
    err: String,
    /// When the forge was last asked; none before it is.
    asked: Option<Time>,
    asking: bool,
}

/// Who Remit works as on each host, by host.
pub type ForgeAccounts = HashMap<String, ForgeAccount>;

/// Where the worktree w is hosted.
pub fn remote_of(w: &Worktree) -> Repo {
    Repo {
        host: w.host.clone(),
        project: w.project.clone(),
    }
}

impl Core {
    /// The forge the owner configured for host.
    pub fn forge_on(&self, host: &str) -> Result<Arc<dyn Forge>> {
        if host.is_empty() {
            return Err(Error::refused("the repository has no host recorded"));
        }
        self.cfg.forges.get(host).cloned().ok_or_else(|| {
            Error::refused(format!(
                "no forge is configured for {host}: start Remit with -forges {host}=gitlab or {host}=github"
            ))
        })
    }

    /// The login of the account Remit works as on host, as its forge last said. It never asks the forge itself: a
    /// firing runs with the Tick's lock held, and a forge that is slow would stop the scheduler with it. A forge never
    /// asked, or one that has not answered yet, is asked in the background, and the refusal is `NotAdmitted`: the firing
    /// waits, and is judged again when the answer is in (`ask_forge`).
    pub fn account_on(self: &Arc<Self>, f: &Arc<dyn Forge>, host: &str) -> Result<String> {
        let now = self.now();
        let (ask, login, failed) = {
            let mut logins = self.forge_logins.lock().unwrap_or_else(PoisonError::into_inner);
            let a = logins.entry(host.to_owned()).or_default();
            let every = if a.login.is_empty() {
                ACCOUNT_RETRY
            } else {
                ACCOUNT_FRESH
            };
            let ask = !a.asking
                && a.asked
                    .is_none_or(|asked| asked.0.checked_add(every).is_none_or(|due| now.0 >= due));
            if ask {
                a.asking = true;
            }
            (ask, a.login.clone(), a.err.clone())
        };
        if ask {
            let (o, f, h) = (Arc::clone(self), Arc::clone(f), host.to_owned());
            if !self.background(move |_| async move { o.ask_forge(&f, &h).await }) {
                self.forge_account(host, |a| a.asking = false);
            }
        }
        if !login.is_empty() {
            return Ok(login);
        }
        let name = f.name();
        if !failed.is_empty() {
            return Err(Error::NotAdmitted(format!(
                "{name} on {host} has not said which account Remit works as ({failed}): it asks again"
            )));
        }
        Err(Error::NotAdmitted(format!(
            "asking {name} on {host} which account Remit works as"
        )))
    }

    /// Asks f who Remit works as on host, keeps the answer, and has what waited on it judged again. An answer that fails
    /// leaves the one before it standing.
    async fn ask_forge(&self, f: &Arc<dyn Forge>, host: &str) {
        let answer = match tokio::time::timeout(FORGE_TIMEOUT, f.account(host)).await {
            Ok(Ok(u)) if u.login.is_empty() => Err(format!("the {} account on {host} has no login", f.name())),
            Ok(Ok(u)) => Ok(u.login),
            Ok(Err(err)) => Err(err.to_string()),
            Err(_) => Err(format!(
                "{} on {host} did not answer in {}s",
                f.name(),
                FORGE_TIMEOUT.as_secs()
            )),
        };
        let now = self.now();
        self.forge_account(host, |a| {
            a.asked = Some(now);
            match answer {
                Ok(login) => {
                    a.login = login;
                    a.err.clear();
                }
                Err(err) => a.err = err,
            }
        });
        self.kick(); // what waited on the forge's answer is judged on the Tick
        self.forge_account(host, |a| a.asking = false);
    }

    fn forge_account(&self, host: &str, edit: impl FnOnce(&mut ForgeAccount)) {
        let mut logins = self.forge_logins.lock().unwrap_or_else(PoisonError::into_inner);
        edit(logins.entry(host.to_owned()).or_default());
    }
}

/// A forge that keeps its branches, checks and merge requests in memory and writes down every call it is asked, for the
/// tests of what Remit asks a forge: crates/forge's clients run `glab` and `gh`, which a test cannot stand in for.
#[cfg(any(test, feature = "testing"))]
pub mod fake {
    use std::collections::HashSet;
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    pub struct FakeForge {
        /// Each call, as "method detail".
        pub calls: Mutex<Vec<String>>,
        /// What branch_head answers, by branch; a branch not here is refused.
        pub branches: Mutex<HashMap<String, String>>,
        /// What checks answers, by commit; a commit not here has none yet.
        pub checks: Mutex<HashMap<String, Checks>>,
        /// The merge requests open on it, by iid.
        pub open_mrs: Mutex<HashSet<i64>>,
        /// A bare repository standing for the forge's: where a branch it holds stands is read there, so a push to it
        /// is seen as the forge sees one. None: branches answers alone.
        pub origin: Mutex<Option<String>>,
    }

    fn held<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        m.lock().unwrap_or_else(PoisonError::into_inner)
    }

    impl FakeForge {
        fn note(&self, call: String) {
            held(&self.calls).push(call);
        }

        pub fn calls(&self) -> Vec<String> {
            held(&self.calls).clone()
        }

        /// Puts branch at sha, as a push that reached the forge does.
        pub fn set_branch(&self, branch: &str, sha: &str) {
            held(&self.branches).insert(branch.to_owned(), sha.to_owned());
        }

        /// Has the pipeline report c on sha.
        pub fn set_checks(&self, sha: &str, c: Checks) {
            held(&self.checks).insert(sha.to_owned(), c);
        }

        /// Opens merge request iid.
        pub fn open_mr(&self, iid: i64) {
            held(&self.open_mrs).insert(iid);
        }

        /// Reads where its branches stand in the bare repository at dir, as a forge hosting it would.
        pub fn backed_by(&self, dir: &str) {
            *held(&self.origin) = Some(dir.to_owned());
        }
    }

    #[async_trait]
    impl Forge for FakeForge {
        fn name(&self) -> &'static str {
            "GitLab"
        }
        async fn account(&self, host: &str) -> Result<Account, forge::Error> {
            self.note(format!("account {host}"));
            Ok(Account {
                login: "remit".to_owned(),
                name: "Remit".to_owned(),
                email: "remit@example.com".to_owned(),
            })
        }
        async fn branch_head(&self, _r: &Repo, branch: &str) -> Result<String, forge::Error> {
            let origin = held(&self.origin).clone();
            if let Some(dir) = origin
                && let Ok(at) =
                    crate::git::git(&dir, &["rev-parse", "-q", "--verify", &format!("refs/heads/{branch}")]).await
            {
                return Ok(at);
            }
            held(&self.branches)
                .get(branch)
                .cloned()
                .ok_or_else(|| forge::Error::Unexpected {
                    forge: "gitlab",
                    what: format!("branch {branch}"),
                    answer: "404 Branch Not Found".to_owned(),
                })
        }
        async fn checks(&self, _r: &Repo, sha: &str) -> Result<Option<Checks>, forge::Error> {
            self.note(format!("checks {sha}"));
            Ok(held(&self.checks).get(sha).cloned())
        }
        async fn close_mr(&self, _r: &Repo, iid: i64, comment: &str) -> Result<bool, forge::Error> {
            self.note(format!("close_mr {iid} {comment}"));
            Ok(held(&self.open_mrs).remove(&iid))
        }
    }
}
