use std::io;
use std::path::{Path, PathBuf};

use crate::Error;

/// A Claude Code sign-in in progress.
pub use claudecode::Login;

/// Where the logins agents run on live: each account has a Claude Code config dir of its own, which is where Claude Code
/// keeps its login (a file in it, or on macOS a Keychain item named for its path). The default account's is `default`;
/// the others are `dir/<id>`. Their transcripts are one: every other config dir links its projects/ to the default's, so
/// a session started on one account is found by the next, which is what lets a card move between accounts and go on where
/// it left off.
#[derive(Debug, Clone, Default)]
pub struct ClaudeAccounts {
    /// The claude command line.
    pub command: Vec<String>,
    /// The default account's config dir.
    pub default: PathBuf,
    /// Where the other accounts' config dirs are.
    pub dir: PathBuf,
}

impl ClaudeAccounts {
    /// The config dir of account id; the default account's for an empty id.
    pub fn home(&self, id: &str) -> PathBuf {
        if id.is_empty() || id == model::DEFAULT_ACCOUNT {
            return self.default.clone();
        }
        self.dir.join(id)
    }

    /// Makes the config dir of account id and links its transcripts to the default account's. It changes nothing in a dir
    /// that is ready, and refuses one whose projects/ is a directory of its own: its sessions would not be found on
    /// another account.
    pub fn prepare(&self, id: &str) -> Result<(), Error> {
        let home = self.home(id);
        if home == self.default {
            return Ok(());
        }
        let transcripts = self.default.join("projects");
        make_dir(&transcripts).map_err(|e| Error::Prepare(format!("the default account's transcripts: {e}")))?;
        make_dir(&home).map_err(|e| Error::Prepare(format!("the config dir of account {id}: {e}")))?;
        let link = home.join("projects");
        match std::fs::symlink_metadata(&link) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return std::os::unix::fs::symlink(&transcripts, &link).map_err(|e| Error::Prepare(e.to_string()));
            }
            Err(e) => return Err(Error::Prepare(e.to_string())),
            Ok(info) if !info.file_type().is_symlink() => {
                return Err(Error::Prepare(format!(
                    "{} is a directory of its own: move it away so the account's sessions are the default account's",
                    link.display()
                )));
            }
            Ok(_) => {}
        }
        match std::fs::read_link(&link) {
            Ok(to) if to == transcripts => Ok(()),
            _ => Err(Error::Prepare(format!(
                "{} does not lead to {}",
                link.display(),
                transcripts.display()
            ))),
        }
    }

    /// The claude command on the config dir of account id.
    fn auth(&self, id: &str) -> claudecode::Auth {
        claudecode::Auth {
            command: self.command.clone(),
            config_dir: self.home(id).display().to_string(),
            env: Vec::new(),
        }
    }

    /// Starts signing account id in, in its own config dir.
    pub async fn sign_in(&self, id: &str) -> Result<Login, Error> {
        let o = claudecode::LoginOptions {
            auth: self.auth(id),
            ..claudecode::LoginOptions::default()
        };
        Ok(claudecode::start_login(o).await?)
    }

    /// The address account id is signed in as. An account with no claude.ai login to name (not signed in, or signed in
    /// with a key) has none, and the error says so.
    pub async fn email(&self, id: &str) -> Result<String, Error> {
        let status = self.auth(id).status().await?;
        if !status.logged_in || status.email.is_empty() {
            return Err(Error::NotSignedIn);
        }
        Ok(status.email)
    }

    /// Signs account id out and removes its config dir, so nothing of it is left on the machine. It never touches the
    /// default account, whose login is the machine's own.
    pub async fn discard(&self, id: &str) -> Result<(), Error> {
        let home = self.home(id);
        if home == self.default {
            return Err(Error::DiscardDefault);
        }
        let logout = self.auth(id).logout().await;
        match std::fs::remove_dir_all(&home) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::Prepare(format!("remove the config dir of account {id}: {e}"))),
        }
        Ok(logout?)
    }
}

/// Makes dir and its parents, readable by this user alone.
fn make_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
}
