use serde::Deserialize;

use crate::Error;

/// Names the config dir a `claude auth` command works on and how to run it: the login of one account.
#[derive(Debug, Clone, Default)]
pub struct Auth {
    /// The claude command line, without its subcommand.
    pub command: Vec<String>,
    /// The config dir whose login it is, whatever the server's own environment says.
    pub config_dir: String,
    /// What else the process is given, `NAME=value`.
    pub env: Vec<String>,
}

/// What `claude auth status` says of a config dir's login.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Status {
    pub logged_in: bool,
    /// The account's address, for a claude.ai login; empty for an API key.
    pub email: String,
}

impl Auth {
    /// A `claude auth <subcommand> <args>` on the config dir. Its environment is the one every Claude Code child gets with
    /// the config dir named, so a login is read and made where it is asked for, never where the server's own environment
    /// points Claude Code's secure storage.
    pub(crate) fn command(&self, subcommand: &str, args: &[&str]) -> Result<procgroup::Command, Error> {
        let (program, own_args) = self.command.split_first().ok_or(Error::NoCommand)?;
        if self.config_dir.is_empty() {
            return Err(Error::NoConfigDir);
        }
        let args = own_args
            .iter()
            .map(String::as_str)
            .chain(["auth", subcommand])
            .chain(args.iter().copied());
        let mut cmd = procgroup::command(program, args);
        let dir = format!("CLAUDE_CONFIG_DIR={}", self.config_dir);
        crate::set_env(&mut cmd, std::iter::once(&dir).chain(&self.env));
        Ok(cmd)
    }

    /// Runs `claude auth <subcommand> <args>` and returns what it printed.
    async fn run(&self, subcommand: &str, args: &[&str]) -> Result<Vec<u8>, Error> {
        let failed = |why: String| Error::Auth {
            subcommand: subcommand.to_string(),
            why,
        };
        let out = self
            .command(subcommand, args)?
            .output()
            .await
            .map_err(|e| failed(e.to_string()))?;
        if !out.status.success() {
            return Err(failed(format!(
                "{}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(out.stdout)
    }

    /// Reads who the config dir is signed in as.
    pub async fn status(&self) -> Result<Status, Error> {
        let out = self.run("status", &["--json"]).await?;
        serde_json::from_slice(&out).map_err(Error::Status)
    }

    /// Signs the config dir out, which also removes its login from the secure storage that holds it (on macOS the
    /// Keychain).
    pub async fn logout(&self) -> Result<(), Error> {
        self.run("logout", &[]).await.map(drop)
    }
}
