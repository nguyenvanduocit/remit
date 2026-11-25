//! An account's OAuth access token, read where Claude Code keeps the login of its config dir, and renewed by Claude Code
//! itself: a session that moves onto another account takes this token (`Conn::set_oauth_token`) while the account's
//! refresh token never leaves the store that Claude Code alone refreshes (docs/research-claude-account-hot-swap/).

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::{Auth, Conn, Error, Handler, Message, Options, Permission, PermissionRequest};

/// How long a renewal waits for the store to hold a fresh token.
const RENEW_TIMEOUT: Duration = Duration::from_secs(30);
/// How often a renewal reads the store again.
const RENEW_POLL: Duration = Duration::from_millis(250);

/// An OAuth access token and when it expires. It prints as its expiry, never its value.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken {
    token: String,
    /// When it expires, in milliseconds since the Unix epoch, as Claude Code stores it.
    expires_at_ms: u64,
}

impl AccessToken {
    /// The token itself, for the one caller that hands it on: `Conn::set_oauth_token`.
    pub fn secret(&self) -> &str {
        &self.token
    }

    /// How long it is still good for: zero once it has expired.
    pub fn left(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let left = u128::from(self.expires_at_ms).saturating_sub(now);
        Duration::from_millis(u64::try_from(left).unwrap_or(u64::MAX))
    }

    /// Whether it is still good for d from now.
    pub fn fresh_for(&self, d: Duration) -> bool {
        self.left() > d
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccessToken(expires at {} ms)", self.expires_at_ms)
    }
}

/// Where Claude Code keeps the login of a config dir: on macOS a Keychain item named for the dir's path, elsewhere a
/// file in the dir. Claude Code names the item `Claude Code-credentials-` and the first 8 hex digits of the SHA-256 of
/// the dir's path, under the account `$USER` (`claude-code-user` when that has other characters than letters, digits and
/// `._-`), in the build this was read from (2.1.322).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Store {
    Keychain { service: String, account: String },
    File(String),
}

impl Store {
    /// Where this machine's Claude Code keeps the login of config_dir.
    pub fn of(config_dir: &str) -> Store {
        if cfg!(target_os = "macos") {
            Store::Keychain {
                service: keychain_service(config_dir),
                account: keychain_account(std::env::var("USER").ok()),
            }
        } else {
            Store::File(format!("{config_dir}/.credentials.json"))
        }
    }

    async fn read(&self) -> Result<Vec<u8>, Error> {
        match self {
            Store::File(path) => tokio::fs::read(path)
                .await
                .map_err(|e| Error::Login(format!("read {path}: {e}"))),
            Store::Keychain { service, account } => {
                let mut cmd = procgroup::command(
                    "security",
                    ["find-generic-password", "-a", account, "-s", service, "-w"],
                );
                crate::set_env(&mut cmd, []);
                let out = cmd
                    .output()
                    .await
                    .map_err(|e| Error::Login(format!("run security: {e}")))?;
                if !out.status.success() {
                    return Err(Error::Login(format!(
                        "the Keychain has no item {service}: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    )));
                }
                Ok(out.stdout)
            }
        }
    }
}

fn keychain_service(config_dir: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(config_dir.as_bytes());
    let hex: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("Claude Code-credentials-{hex}")
}

fn keychain_account(user: Option<String>) -> String {
    match user {
        Some(u) if !u.is_empty() && u.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) => u,
        _ => "claude-code-user".to_string(),
    }
}

/// The part of a stored login this crate reads.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    claude_ai_oauth: Option<StoredOAuth>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredOAuth {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    expires_at: u64,
}

/// The access token of a stored login, or why it has none: not a claude.ai login, or one Claude Code marked dead by
/// clearing it.
fn parse(stored: &[u8]) -> Result<AccessToken, Error> {
    let s: Stored = serde_json::from_slice(stored).map_err(|e| Error::Login(format!("read the stored login: {e}")))?;
    match s.claude_ai_oauth {
        Some(o) if !o.access_token.is_empty() => Ok(AccessToken {
            token: o.access_token,
            expires_at_ms: o.expires_at,
        }),
        _ => Err(Error::Login("the account holds no claude.ai login: sign it in".into())),
    }
}

impl Auth {
    /// The access token the config dir's login holds now, expired or not.
    pub async fn stored_token(&self, store: &Store) -> Result<AccessToken, Error> {
        parse(&store.read().await?)
    }

    /// An access token of the config dir's login good for at least `fresh_for`. A token that is not has Claude Code
    /// renew it: Claude Code renews its login when it starts, before any request to the model, so a process is started on
    /// the config dir and ended once the store holds a fresh token. Claude Code takes its own lock to renew, so this runs
    /// beside sessions on the same account.
    pub async fn fresh_token(&self, store: &Store, fresh_for: Duration) -> Result<AccessToken, Error> {
        if let Ok(t) = self.stored_token(store).await
            && t.fresh_for(fresh_for)
        {
            return Ok(t);
        }
        let o = Options {
            command: self.command.clone(),
            cwd: std::env::temp_dir().display().to_string(),
            env: [format!("CLAUDE_CONFIG_DIR={}", self.config_dir)]
                .into_iter()
                .chain(self.env.iter().cloned())
                .collect(),
            ..Options::default()
        };
        let conn = Conn::start(&o, Arc::new(Quiet), None).await?;
        let renewed = tokio::time::timeout(RENEW_TIMEOUT, async {
            loop {
                if let Ok(t) = self.stored_token(store).await
                    && t.fresh_for(fresh_for)
                {
                    return t;
                }
                tokio::time::sleep(RENEW_POLL).await;
            }
        })
        .await;
        conn.close();
        renewed.map_err(|_| {
            Error::Login(format!(
                "Claude Code did not renew the login of {} within {}s: sign the account in again",
                self.config_dir,
                RENEW_TIMEOUT.as_secs()
            ))
        })
    }
}

/// The handler of a process started only to renew a login: it sends nothing to the model, so nothing comes to answer.
struct Quiet;

impl Handler for Quiet {
    fn message(&self, _m: Message) {}

    async fn can_use_tool(&self, _req: &PermissionRequest) -> Permission {
        Permission::Deny {
            message: "this process only renews a login".into(),
        }
    }

    async fn hook(&self, _input: claudehooks::HookInput) -> claudehooks::HookOutput {
        claudehooks::HookOutput::default()
    }
}
