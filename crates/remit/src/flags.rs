//! The server's command line: what each flag means, the paths it leaves to their defaults, and what follows from them
//! (the paths no agent session may read or change, the orchestrator's settings). Reading files is limited to what a
//! path's default depends on.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::dash_flags::{self, Flag, Kind, Parsed};

/// The file the process manager (pm2) starts the server from, with the flags it runs with, kept in the data directory beside
/// the database. A session that rewrote it would decide what the server runs at its next start.
const SUPERVISOR_CONFIG: &str = "ecosystem.config.cjs";

/// The directory the process manager keeps its state in under the user's home unless PM2_HOME says otherwise: it holds the
/// environment of every app it runs, the server's credentials among it.
const SUPERVISOR_STATE_NAME: &str = ".pm2";

/// The node's key (ADR-0072 decision 12), in the data directory.
pub const NODE_KEY: &str = "node.key";

/// The command line, its paths absolute once `with_paths` has run.
#[derive(Debug, Clone, Default)]
pub struct Flags {
    pub addr: String,
    pub link_addr: String,
    pub data_dir: String,
    pub agent_cmd: String,
    pub principles: String,
    pub chair_model: String,
    pub chair_effort: String,
    pub assistant_model: String,
    pub assistant_effort: String,
    pub mode: String,
    pub glab: String,
    pub gh: String,
    pub forges: String,
    pub vault: String,
    pub language: String,
    pub claude_dir: String,
    pub autonomous: bool,
    pub event_chains: i64,
    pub turn_idle: Duration,
    pub turn_repeats: i64,
    pub spin_turns: i64,
    pub snapshot_every: Duration,
    pub keep_worktrees: Duration,
    pub idle_alert: Duration,
    pub secret_key_file: String,
    pub key_backup_dir: String,
    pub client_key_files: String,
    pub secret_dirs: String,
    pub jira_env: String,
    pub jira_jql: String,
    pub source_env: String,
    pub openrouter_key_file: String,
    /// The env every record Remit sends to Datadog is tagged with (ADR-0119).
    pub datadog_env: String,
    /// The GitHub repository this server's releases come from, owner/name; empty: it does not run from releases.
    pub update_repo: String,
    /// The supervisor's restart command `remit update -restart` takes; empty: nothing installs a release on this server.
    pub update_restart: String,
    /// Google sign-in for the owner (ADR-0105): the Firebase project, its web API key and auth domain, and the accounts
    /// that come in; all empty: it is off.
    pub firebase_project: String,
    pub firebase_api_key: String,
    pub firebase_auth_domain: String,
    pub google_emails: String,
    /// No flag: set by `with_paths`, under the data directory.
    pub database: String,
    pub owner_key_file: String,
    pub node_key_file: String,
    /// No flag: set by `with_paths`: the binary being run, the process manager's state directory and the launchd job.
    pub executable: String,
    pub supervisor_state: String,
    pub launch_agent: String,
}

/// The flags, with this machine's defaults: home is the user's home, claude_config_dir the process's CLAUDE_CONFIG_DIR.
pub fn defs(home: &Path, claude_config_dir: &str) -> Vec<Flag> {
    let s = |d: &str| d.to_owned();
    let dur = |secs: u64| dash_flags::duration_text(Duration::from_secs(secs));
    let f = |name, kind, default: String, usage| Flag {
        name,
        kind,
        default,
        usage,
    };
    use Kind::{Bool, Duration as D, Int, Str};
    vec![
        f(
            "addr",
            Str,
            s("127.0.0.1:4320"),
            "listen address (TCP for HTTP, UDP for the link's WebTransport)",
        ),
        f(
            "link-addr",
            Str,
            s(""),
            "where the link's WebTransport (UDP) listens instead of -addr, for browsers on other machines: an address that also takes this machine's own, 0.0.0.0:<port> or [::]:<port>, as the AI runtime dials the link on loopback; HTTP, which carries the owner key, stays on -addr (empty: -addr)",
        ),
        f(
            "data",
            Str,
            home.join(".remit").display().to_string(),
            "the database, its backups and the chains' working directories",
        ),
        f(
            "agent",
            Str,
            s("claude"),
            "Claude Code command every session runs on, signed in with this machine's login (-claude-config-dir), in a config dir of its own (<data>/claude)",
        ),
        f(
            "principles",
            Str,
            s(""),
            "owner's principles file given to every agent (empty or absent: none)",
        ),
        f(
            "chair-model",
            Str,
            s("fable"),
            "model for the Chair; the Chair chooses the model of every agent it hires",
        ),
        f(
            "chair-effort",
            Str,
            s("high"),
            "effort for the Chair (low, medium, high, xhigh, max; empty: Claude Code's default); the Chair chooses the effort of every agent it hires",
        ),
        f(
            "assistant-model",
            Str,
            s("fable"),
            "model the Assistant, the owner's agent for the whole app, runs on",
        ),
        f(
            "assistant-effort",
            Str,
            s("high"),
            "effort the Assistant runs at (low, medium, high, xhigh, max; empty: Claude Code's default)",
        ),
        f("mode", Str, s("auto"), "Claude Code permission mode for every agent"),
        f(
            "autonomous",
            Bool,
            s("true"),
            "chains decide without the owner: the Chair asks him only to clear a block from outside the chain; Remit lands each approved change itself",
        ),
        f(
            "glab",
            Str,
            s("glab"),
            "glab binary, logged in on this machine, that Remit reads GitLab hosts through (where a branch stands, the checks on a commit)",
        ),
        f(
            "gh",
            Str,
            s("gh"),
            "gh binary, logged in on this machine, that Remit reads GitHub hosts through (where a branch stands, the checks on a commit)",
        ),
        f(
            "forges",
            Str,
            s("github.com=github,gitlab.com=gitlab"),
            "the forge of each host a repository may be on, host=kind separated by commas, kind gitlab or github: a repository on a host not named here cannot be opened",
        ),
        f(
            "vault",
            Str,
            s(""),
            "a directory of the owner's files: <vault>/secret holds credential env files (empty: no vault)",
        ),
        f(
            "event-chains-per-day",
            Int,
            s("3"),
            "chains the events of one event signal may start a day; past it its events wait for the next day (0: no limit)",
        ),
        f(
            "turn-idle",
            D,
            dur(30 * 60),
            "how long an agent in a turn may send nothing (no message from Claude Code) before Remit cancels the turn, and closes its session if the turn has not ended a minute later; the turn fails, with a retry in the inbox (0: never)",
        ),
        f(
            "turn-repeats",
            Int,
            s("3"),
            "how many times the same tool call with the same result, or the same error, among a turn's last 8 calls (numbers, times and ids masked) makes Remit steer a note into the turn; the second time it tells the agent to end the turn with its report, and ends the turn if the calls go on (0: never; docs/adr/0092)",
        ),
        f(
            "spin-turns",
            Int,
            s("10"),
            "how many turns a running chain's agents may take with nothing produced (no plan step completed, no work accepted, no verdict) before Remit reminds its Chair, and again every as many turns, finishing the chain after 3 reminders; for the Assistant, turns in a row on agents' replies with no word from the owner before its messages to agents carry a reminder, refused after 3 (0: never; docs/adr/0104)",
        ),
        f(
            "language",
            Str,
            s("English"),
            "language of everything the owner reads (messages, why, labels, summaries)",
        ),
        f(
            "claude-config-dir",
            Str,
            s(claude_config_dir),
            "Claude Code config dir whose login the agents use (on Linux; on macOS the agents sign in once for <data>/claude, as the start says), and whose skills and agents the library imports from; empty uses ~/.claude",
        ),
        f(
            "secret-key-file",
            Str,
            s(""),
            "the key the sources' credentials and the event signals' keys are sealed with, created (mode 0600) when the first one is sealed, and backed up in -key-backup-dir before anything is sealed under it (default <data>/secret.key)",
        ),
        f(
            "key-backup-dir",
            Str,
            s(""),
            "where the secret key is backed up, one owner-only file per key named by its id; a directory named secret keeps it under the agents' secret-read rules, and one on another disk survives losing the data directory (default <data>/backups/secret)",
        ),
        f(
            "client-key-files",
            Str,
            s(""),
            "comma-separated absolute paths of the files where the holders of client keys keep theirs, which no agent session can read, as for -secret-key-file (ADR-0018)",
        ),
        f(
            "secret-dirs",
            Str,
            s(""),
            "comma-separated absolute paths of directories that hold the owner's credentials and that no agent session may read (a vault's secret directory is hidden already)",
        ),
        f(
            "keep-worktrees",
            D,
            dur(7 * 24 * 3600),
            "how long a finished chain with no landing on its way keeps its worktrees before they are removed (its branches stay; reopening the chain brings them back) (0: for ever)",
        ),
        f(
            "idle-alert",
            D,
            dur(2 * 3600),
            "how long nothing may run while work waits (queued chains, changes waiting to land or questions for the owner) before the owner's inbox says so, once (0: never)",
        ),
        f(
            "snapshot-every",
            D,
            dur(24 * 3600),
            "how often the database is copied, whole and consistent, into <data>/backups; the newest five copies are kept (0: never)",
        ),
        f(
            "jira-env",
            Str,
            s(""),
            "env file with ATLASSIAN_HOST, ATLASSIAN_EMAIL, ATLASSIAN_TOKEN the issues capability's list_issues reads Jira with (default <vault>/secret/atlassian/jira.env)",
        ),
        f(
            "jira-jql",
            Str,
            s(""),
            "the JQL of the Jira issues list_issues shows (without it Jira is not read)",
        ),
        f(
            "source-env",
            Str,
            s(""),
            "env file with the credentials of the sources agents read the outside world through (MONGODB_URI, STARROCKS_HOST, BYTEBASE_URL, ...; which variables each source needs: crates/source, ADR-0015). The default connection of each source reads the values the owner entered in his inbox first, then the process environment, this file and the vault's own env files (devops/grafana.env, devops/coroot.env, devops/datadog.env, services/intercom.env, services/amplitude.env); a named connection reads only the values entered for it (ADR-0017). Agents are asked before they read it (default <data>/sources.env)",
        ),
        f(
            "openrouter-key-file",
            Str,
            s(""),
            "file holding the OpenRouter key memory is searched by meaning and judged for relevance with (qwen3-embedding-8b, qwen3-reranker-8b), read at each use and hidden from every session; with no key memory is searched by words and links (default <data>/openrouter.key)",
        ),
        f(
            "datadog-env",
            Str,
            s(""),
            "the env every record sent to Datadog is tagged with: the server's logs and spans and the agents' telemetry, each naming its chain, sent to the site of the Datadog source's default connection once the owner enters its key (ADR-0119) (empty: this machine's host name)",
        ),
        f(
            "update-repo",
            Str,
            s(""),
            "the GitHub repository this server runs the releases of, owner/name: a release of it that its procedure published is installed here as its last step, and taken back here when it breaches (ADR-0116) (empty: this server does not run from releases, as a dev instance)",
        ),
        f(
            "update-restart",
            Str,
            s(""),
            "the command, run by sh -c, with which this server's supervisor restarts it on <data>/releases/current, as remit update -restart takes it: set, a release of -update-repo is installed here (empty: nothing restarts this server; needs -update-repo)",
        ),
        f(
            "firebase-project",
            Str,
            s(""),
            "the Firebase project id the owner signs in with Google through, beside the owner key (ADR-0105); set with -firebase-api-key and -google-emails (empty: Google sign-in is off)",
        ),
        f(
            "firebase-api-key",
            Str,
            s(""),
            "the web API key of -firebase-project, which the way in sets Firebase up with (not a secret: Firebase shows it to every page that signs in)",
        ),
        f(
            "firebase-auth-domain",
            Str,
            s(""),
            "the domain Firebase's Google sign-in window runs on (empty: <firebase-project>.firebaseapp.com)",
        ),
        f(
            "google-emails",
            Str,
            s(""),
            "the Google accounts that come in as the owner, comma-separated; any other account is refused (set with -firebase-project)",
        ),
    ]
}

/// The flags of a subcommand that reaches a server installed with the server's flags: the server's own flags it names
/// (as the server defines them, so a default has one owner), then its own.
pub fn subcommand_defs(home: &Path, server_flags: &[&str], own: Vec<Flag>) -> Vec<Flag> {
    let mut defs: Vec<Flag> = defs(home, "")
        .into_iter()
        .filter(|f| server_flags.contains(&f.name))
        .collect();
    defs.extend(own);
    defs
}

impl Flags {
    /// The flags a parsed command line sets.
    pub fn of(p: &Parsed) -> Flags {
        Flags {
            addr: p.str("addr"),
            link_addr: p.str("link-addr"),
            data_dir: p.str("data"),
            agent_cmd: p.str("agent"),
            principles: p.str("principles"),
            chair_model: p.str("chair-model"),
            chair_effort: p.str("chair-effort"),
            assistant_model: p.str("assistant-model"),
            assistant_effort: p.str("assistant-effort"),
            mode: p.str("mode"),
            autonomous: p.bool("autonomous"),
            glab: p.str("glab"),
            gh: p.str("gh"),
            forges: p.str("forges"),
            vault: p.str("vault"),
            event_chains: p.int("event-chains-per-day"),
            turn_idle: p.duration("turn-idle"),
            turn_repeats: p.int("turn-repeats"),
            spin_turns: p.int("spin-turns"),
            language: p.str("language"),
            claude_dir: p.str("claude-config-dir"),
            secret_key_file: p.str("secret-key-file"),
            key_backup_dir: p.str("key-backup-dir"),
            client_key_files: p.str("client-key-files"),
            secret_dirs: p.str("secret-dirs"),
            keep_worktrees: p.duration("keep-worktrees"),
            idle_alert: p.duration("idle-alert"),
            snapshot_every: p.duration("snapshot-every"),
            jira_env: p.str("jira-env"),
            jira_jql: p.str("jira-jql"),
            source_env: p.str("source-env"),
            openrouter_key_file: p.str("openrouter-key-file"),
            datadog_env: p.str("datadog-env"),
            update_repo: p.str("update-repo"),
            update_restart: p.str("update-restart"),
            firebase_project: p.str("firebase-project"),
            firebase_api_key: p.str("firebase-api-key"),
            firebase_auth_domain: p.str("firebase-auth-domain"),
            google_emails: p.str("google-emails"),
            ..Flags::default()
        }
    }

    /// Fills the paths left to their defaults and makes each absolute, so the rules that keep agents from Remit's own
    /// secrets name the files themselves. `executable` is this binary, `home` the user's home, `pm2_home` PM2_HOME.
    pub fn with_paths(mut self, executable: &Path, home: &Path, pm2_home: &str) -> Result<Flags, String> {
        if self.jira_env.is_empty() && !self.vault.is_empty() {
            self.jira_env = join(&self.vault, &["secret", "atlassian", "jira.env"]);
        }
        if self.secret_key_file.is_empty() {
            self.secret_key_file = join(&self.data_dir, &["secret.key"]);
        }
        if self.source_env.is_empty() {
            self.source_env = join(&self.data_dir, &["sources.env"]);
        }
        if self.openrouter_key_file.is_empty() {
            self.openrouter_key_file = join(&self.data_dir, &["openrouter.key"]);
        }
        self.database = join(&self.data_dir, &["remit.db"]);
        self.owner_key_file = join(&self.data_dir, &["owner.key"]);
        self.node_key_file = join(&self.data_dir, &[NODE_KEY]);
        if self.key_backup_dir.is_empty() {
            self.key_backup_dir = store::snapshot_dir(Path::new(&self.database))
                .join("secret")
                .display()
                .to_string();
        }
        require_update(&self.update_repo, &self.update_restart)?;
        let cwd = std::env::current_dir().map_err(|e| format!("the working directory: {e}"))?;
        for p in [
            &mut self.data_dir,
            &mut self.secret_key_file,
            &mut self.key_backup_dir,
            &mut self.database,
            &mut self.owner_key_file,
            &mut self.node_key_file,
            &mut self.source_env,
            &mut self.openrouter_key_file,
        ] {
            if !p.is_empty() {
                *p = absolute(&cwd, p).display().to_string();
            }
        }
        self.executable = executable.display().to_string();
        self.supervisor_state = if pm2_home.is_empty() {
            home.join(SUPERVISOR_STATE_NAME).display().to_string()
        } else {
            pm2_home.to_owned()
        };
        self.launch_agent = home
            .join("Library")
            .join("LaunchAgents")
            .join("remit.plist")
            .display()
            .to_string();
        self.require_writable_workplaces(&cwd)?;
        Ok(self)
    }

    /// Fails when a directory sessions work in (the data directory, the vault) lies inside the paths the server runs from:
    /// they would read it and could not write it, and no session could work.
    fn require_writable_workplaces(&self, cwd: &Path) -> Result<(), String> {
        for work in [&self.data_dir, &self.vault] {
            if work.is_empty() {
                continue;
            }
            let abs = absolute(cwd, work);
            for server in server_files(Path::new(&self.executable)) {
                if abs.starts_with(&server) {
                    return Err(format!(
                        "{} is inside {}, which sessions can read and not change, so they could not work there: move it, or run the binary from elsewhere",
                        abs.display(),
                        server.display()
                    ));
                }
            }
        }
        Ok(())
    }

    /// The vault's directory of credential env files; empty when there is no vault.
    pub fn vault_secrets(&self) -> String {
        if self.vault.is_empty() {
            return String::new();
        }
        join(&self.vault, &["secret"])
    }

    /// The directories -secret-dirs names, cleaned.
    pub fn secret_dir_list(&self) -> Vec<String> {
        self.secret_dirs
            .split(',')
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(|d| clean(Path::new(d)).display().to_string())
            .collect()
    }

    /// The files where the holders of client keys keep theirs, as -client-key-files names them.
    pub fn client_keys(&self) -> Vec<String> {
        self.client_key_files
            .split(',')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// Where the programs of the extensions the owner applied are kept.
    pub fn ext_dir(&self) -> PathBuf {
        Path::new(&self.data_dir).join("ext")
    }

    /// The Claude Code config dir the agents run with: Remit's own, under its data, which the AI runtime prepares.
    pub fn agent_home(&self) -> PathBuf {
        Path::new(&self.data_dir).join("claude")
    }

    /// Where the memory daemon Remit once ran kept its database, which the start imports once (ADR-0117).
    pub fn old_memory_dir(&self) -> PathBuf {
        Path::new(&self.data_dir).join("memory")
    }

    /// The directory `remit update` installs releases in, keeps its lock and writes its outcome to (ADR-0087).
    pub fn releases_dir(&self) -> PathBuf {
        Path::new(&self.data_dir).join("releases")
    }

    /// The paths a session cannot reach as the machine's user could (ADR-0018). `hide` cannot be read (and a hidden file
    /// cannot be renamed or deleted): the sealing key and its backups, the owner key, the node key, the database with its
    /// write-ahead files, snapshots and hand-made copies, the files of source, Jira and OpenRouter credentials, the vault's secret
    /// directory, the client keys' files, the process manager's config, copies and state, the launchd job, the logs, the
    /// employees' old memory and the releases directory (its binaries, the update's lock, outcome and links, which a
    /// session that wrote them would install or forge). `read_only` reads as it is and cannot be changed: the directory of
    /// the server's binary (or the checkout whose bin/ it is) and the extensions' programs. The data directory itself is in neither: the chains' work lives in it.
    pub fn protected_from_agents(&self) -> (Vec<String>, Vec<String>) {
        let data = Path::new(&self.data_dir);
        let db_name = Path::new(&self.database)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut hide = vec![
            self.secret_key_file.clone(),
            self.key_backup_dir.clone(),
            store::snapshot_dir(Path::new(&self.database)).display().to_string(),
            self.source_env.clone(),
            self.jira_env.clone(),
            self.vault_secrets(),
            self.database.clone(),
            format!("{}-wal", self.database),
            format!("{}-shm", self.database),
            self.owner_key_file.clone(),
            self.node_key_file.clone(),
            self.openrouter_key_file.clone(),
        ];
        hide.extend(with_prefix(data, &format!("{db_name}.")));
        hide.extend(with_prefix(data, SUPERVISOR_CONFIG));
        hide.extend(with_prefix(data, "agents.log"));
        hide.extend(with_prefix(data, "remit.log"));
        hide.extend(
            self.client_keys()
                .iter()
                .map(|p| clean(Path::new(p)).display().to_string()),
        );
        hide.push(self.supervisor_state.clone());
        hide.push(self.launch_agent.clone());
        hide.extend(self.secret_dir_list());
        hide.push(data.join("employees").display().to_string());
        hide.push(self.releases_dir().display().to_string());
        hide.retain(|p| !p.is_empty());
        let mut read_only: Vec<String> = server_files(Path::new(&self.executable))
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        read_only.push(self.ext_dir().display().to_string());
        (hide, read_only)
    }

    /// The forge of each host -forges names, each driven through its program.
    pub fn forges_by_host(&self) -> Result<HashMap<String, Arc<dyn orch::Forge>>, String> {
        let hosts = forge::parse_hosts(&self.forges).map_err(|e| format!("-forges: {e}"))?;
        Ok(hosts
            .into_iter()
            .map(|(host, kind)| {
                let f: Arc<dyn orch::Forge> = match kind {
                    forge::Kind::GitLab => Arc::new(forge::GitLab::new(&self.glab)),
                    forge::Kind::GitHub => Arc::new(forge::GitHub::new(&self.gh)),
                };
                (host, f)
            })
            .collect())
    }
}

/// Refuses at start an -update-repo that is not owner/name, and an -update-restart without the repository whose
/// releases it would install.
fn require_update(repo: &str, restart: &str) -> Result<(), String> {
    if !repo.is_empty()
        && repo
            .split_once('/')
            .is_none_or(|(o, n)| o.is_empty() || n.is_empty() || n.contains('/'))
    {
        return Err(format!(
            "-update-repo {repo:?}: name the GitHub repository as owner/name"
        ));
    }
    if repo.is_empty() && !restart.trim().is_empty() {
        return Err("-update-restart needs -update-repo: the repository whose releases it installs".to_owned());
    }
    Ok(())
}

/// Fails when a flag names a model the catalog does not offer, naming the ones it does: better at start than at the first
/// session.
pub fn require_models(catalog: &provider::Catalog, named: &[(&str, &str)]) -> Result<(), String> {
    let mut named = named.to_vec();
    named.sort_unstable();
    for (flag, model) in named {
        if catalog.lookup(model).is_none() {
            return Err(format!(
                "{flag}: {model:?} is not a model this machine offers: choose one of {}",
                catalog.describe()
            ));
        }
    }
    Ok(())
}

/// Refuses at start an effort flag Claude Code does not take, with the levels it does; empty keeps Claude Code's default.
pub fn require_efforts(named: &[(&str, &str)]) -> Result<(), String> {
    for (flag, effort) in named {
        provider::Effort::parse(effort).map_err(|e| format!("{flag}: {e}"))?;
    }
    Ok(())
}

/// The paths the server is run and built from, given its binary: the binary's directory, or the checkout around it when that
/// directory is the checkout's bin/ (a Cargo.toml or go.mod beside it), which holds the sources the next build compiles.
pub fn server_files(executable: &Path) -> Vec<PathBuf> {
    let bin = executable.parent().unwrap_or(Path::new("/")).to_path_buf();
    if let Some(checkout) = bin.parent()
        && (checkout.join("Cargo.toml").exists() || checkout.join("go.mod").exists())
    {
        return vec![checkout.to_path_buf()];
    }
    vec![bin]
}

/// The entries of dir whose names start with prefix.
fn with_prefix(dir: &Path, prefix: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
        .map(|e| e.path().display().to_string())
        .collect()
}

fn join(base: &str, parts: &[&str]) -> String {
    parts
        .iter()
        .fold(PathBuf::from(base), |p, part| p.join(part))
        .display()
        .to_string()
}

/// p against cwd, cleaned.
pub fn absolute(cwd: &Path, p: &str) -> PathBuf {
    clean(&cwd.join(p))
}

/// p with `.` and `..` resolved lexically.
fn clean(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where README's flag table starts and ends.
    const TABLE_START: &str = "<!-- flags: written by `make gen` from crates/remit/src/flags.rs -->";
    const TABLE_END: &str = "<!-- /flags -->";

    /// The flags as a Markdown table, in the order they are defined, each default as `-h` prints it for a user whose
    /// home is ~ (none when it is empty: the usage says what an empty value comes to).
    fn flag_table(defs: &[Flag]) -> String {
        let mut out = String::from("| Flag | Default | What |\n|---|---|---|\n");
        for d in defs {
            let default = if d.default.is_empty() {
                String::new()
            } else {
                format!("`{}`", d.default)
            };
            out.push_str(&format!(
                "| `-{}` | {default} | {} |\n",
                d.name,
                d.usage.replace('|', "\\|")
            ));
        }
        out
    }

    /// readme with what lies between the table's markers replaced by table; None when it has no markers.
    fn with_table(readme: &str, table: &str) -> Option<String> {
        let (head, rest) = readme.split_once(TABLE_START)?;
        let (_, tail) = rest.split_once(TABLE_END)?;
        Some(format!("{head}{TABLE_START}\n{table}{TABLE_END}{tail}"))
    }

    // README's flag table is written from `defs`, so the two cannot say different things: `make gen` (which sets
    // REMIT_WRITE_README) writes it, and this fails while it differs.
    #[test]
    fn the_readme_flag_table_is_the_one_the_flags_write() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
        let readme = std::fs::read_to_string(&path).unwrap();
        let table = flag_table(&defs(Path::new("~"), "$CLAUDE_CONFIG_DIR"));
        let written = with_table(&readme, &table).expect("README.md holds the flag table's markers");
        if written != readme && std::env::var_os("REMIT_WRITE_README").is_some() {
            std::fs::write(&path, &written).unwrap();
            return;
        }
        assert!(
            written == readme,
            "README.md's flag table is not the one crates/remit/src/flags.rs writes: run `make gen`"
        );
    }
}
