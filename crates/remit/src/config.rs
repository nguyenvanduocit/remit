//! From the command line to the process's settings: what the orchestrator is configured with, and what the process
//! needs besides, failing on what must be there before anything starts.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::flags::{Flags, require_efforts, require_models};

/// The agent program this machine starts sessions on, the config dir it runs with, and what is kept from it: handed to the
/// AI runtime on its stdin.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFlags {
    /// Claude Code command line.
    pub claude_cmd: String,
    /// The Claude Code config dir of the agents, prepared by `agent_home::prepare`.
    pub claude_home: String,
}

impl AgentFlags {
    /// The Claude Code command line, as the agents and the start's login check run it.
    pub fn command(&self) -> Vec<String> {
        self.claude_cmd.split_whitespace().map(str::to_owned).collect()
    }

    /// Where the logins agents run on live: the default account in the agents' config dir, the others beside it.
    pub fn accounts(&self) -> agent::ClaudeAccounts {
        let home = Path::new(&self.claude_home);
        agent::ClaudeAccounts {
            command: self.command(),
            default: home.to_path_buf(),
            dir: home.parent().unwrap_or(Path::new("/")).join("claude-accounts"),
        }
    }
}

/// What the process needs besides the orchestrator's settings.
pub struct ServerConfig {
    /// Where HTTP (TCP) listens, and the link (UDP) unless `link` says otherwise.
    pub addr: SocketAddr,
    /// Where the link (UDP) listens: `addr`, or `-link-addr`.
    pub link: SocketAddr,
    pub data_dir: PathBuf,
    /// This binary, which the AI runtime is started from.
    pub executable: PathBuf,
    pub agents: AgentFlags,
    /// Where the owner key is kept.
    pub owner_key_file: PathBuf,
    /// Google sign-in for the owner beside the owner key (ADR-0105); none: off.
    pub google: Option<crate::google_signin::Setup>,
    /// Where this node's key is kept (ADR-0072 decision 12).
    pub node_key_file: PathBuf,
    /// Where the memory daemon Remit once ran kept its database, imported once at start (ADR-0117).
    pub old_memory_dir: PathBuf,
    /// The credentials of the sources: the process environment, then -source-env, then the vault's env files (ADR-0015).
    pub source_env: source::Env,
}

/// The file `remit update` appends its output to when a release of this server's repository starts it, in the releases
/// directory.
const UPDATE_LOG: &str = "update.log";

/// The names of the server's environment `remit update` takes besides those every child takes (`childenv::of`), each
/// because the update fails without it: gh's login (`GH_TOKEN`, docs/deploy.md), the host it signs in to (`GH_HOST`) and
/// the directory its login is kept in when not the default (`GH_CONFIG_DIR`), to download the release; the process
/// manager's home (`PM2_HOME`), for -update-restart to reach the supervisor that runs this server.
const UPDATE_ENV: [&str; 4] = ["GH_TOKEN", "GH_HOST", "GH_CONFIG_DIR", "PM2_HOME"];

/// How this server runs `remit update` on itself, the binary it is with the flags it was started with and every
/// `remit update` flag but `-to`; none without -update-restart (ADR-0087).
fn updater_of(f: &Flags, releases: &Path) -> Option<orch::Updater> {
    if f.update_restart.trim().is_empty() {
        return None;
    }
    let args = [
        ("update", None),
        ("-addr", Some(&f.addr)),
        ("-data", Some(&f.data_dir)),
        ("-gh", Some(&f.gh)),
        ("-repo", Some(&f.update_repo)),
        ("-restart", Some(&f.update_restart)),
    ]
    .into_iter()
    .flat_map(|(name, value)| std::iter::once(name.to_owned()).chain(value.cloned()))
    .chain(["-releases".to_owned(), releases.display().to_string()])
    .collect();
    Some(orch::Updater {
        program: PathBuf::from(&f.executable),
        args,
        lock: releases.join(crate::update::LOCK),
        log: releases.join(UPDATE_LOG),
        inherits: UPDATE_ENV.map(str::to_owned).to_vec(),
    })
}

/// The process's settings from flags, home being the user's home.
pub fn configure(f: Flags, home: &Path) -> Result<(ServerConfig, orch::Config), String> {
    let jira = match orch::Jira::load(Path::new(&f.jira_env), &f.jira_jql) {
        Ok(j) => Some(Arc::new(j)),
        Err(err) => {
            tracing::warn!(%err, "jira not available to list_issues");
            None
        }
    };
    let mut env_files = vec![PathBuf::from(&f.source_env)];
    env_files.extend(source::vault_files(Path::new(&f.vault_secrets())));
    let process_env = |name: &str| std::env::var(name).unwrap_or_default();
    let source_env = source::load_env(process_env, &env_files).unwrap_or_else(|err| {
        tracing::warn!(%err, "source credentials: only the process environment is read");
        source::Env::new(process_env)
    });
    let mut principles = f.principles.clone();
    if std::fs::metadata(&principles).is_err() {
        principles.clear(); // agents are not sent to read a file that is not there
    }
    let all = provider::Catalog::default();
    let models = all.up_to(&f.top_model).ok_or_else(|| {
        format!(
            "-top-model: {:?} is not a model this machine offers: choose one of {}",
            f.top_model,
            all.describe()
        )
    })?;
    require_models(
        &models,
        &[
            ("-chair-model", &f.chair_model),
            ("-assistant-model", &f.assistant_model),
        ],
    )?;
    require_efforts(&[
        ("-chair-effort", &f.chair_effort),
        ("-assistant-effort", &f.assistant_effort),
    ])?;
    // every judge runs on the strongest model offered, so none grades an author stronger than itself (ADR-0102, ADR-0133)
    let judge_model = models.strongest().map(|m| m.id.clone()).unwrap_or_default();
    let addr = listen_addr(&f.addr)?;
    let link = link_addr(&f.link_addr, addr)?;
    let mcp = format!("{}{}", local_url(addr), crate::agentmcp::PATTERN);
    let otlp = format!("{}{}", local_url(addr), crate::otlp::PATTERN);
    let forges = f.forges_by_host()?;
    let count = |name: &str, n: i64| usize::try_from(n).map_err(|_| format!("-{name} is {n}: a count is 0 or more"));
    let claude_config = if f.claude_dir.trim().is_empty() {
        home.join(".claude")
    } else {
        PathBuf::from(&f.claude_dir)
    };
    let data = PathBuf::from(&f.data_dir);
    let openrouter = openrouter::OpenRouter::new(PathBuf::from(&f.openrouter_key_file))
        .map_err(|e| format!("the OpenRouter client: {e}"))?;
    let cfg = orch::Config {
        core: orch::chains::Config {
            work_root: data.join("work"),
            mcp_url: mcp,
            otlp_url: otlp,
            principles,
            claude_config_dir: claude_config,
            chair_model: f.chair_model.clone(),
            chair_effort: f.chair_effort.clone(),
            models: Some(models),
            mode: f.mode.clone(),
            language: f.language.clone(),
            autonomous: f.autonomous,
            assistant_model: f.assistant_model.clone(),
            assistant_effort: f.assistant_effort.clone(),
            assistant_dir: data.join("assistant"),
            repos_dir: data.join("repos"),
            folders_dir: data.join("folders"),
            library_dir: data.join("library"),
            secret_key_file: PathBuf::from(&f.secret_key_file),
            key_backup_dir: PathBuf::from(&f.key_backup_dir),
            database: PathBuf::from(&f.database),
            client_key_files: f.client_keys().iter().map(PathBuf::from).collect(),
            secret_dirs: f.secret_dir_list().iter().map(PathBuf::from).collect(),
            ext_dir: f.ext_dir(),
            event_chains_per_day: count("event-chains-per-day", f.event_chains)?,
            host: Some(Arc::new(crate::host::ThisHost::new())),
            snapshot_every: time::Duration::try_from(f.snapshot_every).map_err(|e| format!("-snapshot-every: {e}"))?,
            turn_idle: f.turn_idle,
            turn_repeats: count("turn-repeats", f.turn_repeats)?,
            spin_turns: count("spin-turns", f.spin_turns)?,
            keep_worktrees: f.keep_worktrees,
            idle_alert: f.idle_alert,
            source_env_file: PathBuf::from(&f.source_env),
            require_approvals: true,
            goal_check: true,
            judge_model,
            forges,
            update_repo: f.update_repo.clone(),
            releases_dir: f.releases_dir(),
            decider: Some(Arc::new(crate::ports::OpenRouterDecider(openrouter.clone()))),
            ..orch::chains::Config::default()
        },
        jira,
        memory_models: Some(Arc::new(crate::ports::OpenRouterModels(openrouter))),
        updater: updater_of(&f, &f.releases_dir()),
        release_steps: orch::ReleaseSteps::Programs(orch::ProgramRunner::new()),
        ..orch::Config::default()
    };
    let server = ServerConfig {
        addr,
        link,
        data_dir: data,
        executable: PathBuf::from(&f.executable),
        agents: AgentFlags {
            claude_cmd: f.agent_cmd.clone(),
            claude_home: f.agent_home().display().to_string(),
        },
        owner_key_file: PathBuf::from(&f.owner_key_file),
        google: crate::google_signin::setup(&f)?,
        node_key_file: PathBuf::from(&f.node_key_file),
        old_memory_dir: f.old_memory_dir(),
        source_env,
    };
    Ok((server, cfg))
}

/// The address -addr names. The owner key travels in plain HTTP, so the server listens on this machine alone (ADR-0026):
/// anything a machine but this one could reach is refused, a half-made setup must not turn into an open door.
pub fn listen_addr(addr: &str) -> Result<SocketAddr, String> {
    let (host, port) = addr
        .rsplit_once(':')
        .ok_or_else(|| format!("owner key: -addr {addr:?}: missing port in address"))?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let port: u16 = port
        .parse()
        .map_err(|_| format!("owner key: -addr {addr:?}: invalid port"))?;
    let ip: IpAddr = if host == "localhost" {
        IpAddr::from([127, 0, 0, 1])
    } else {
        host.parse().map_err(|_| refused_addr(addr))?
    };
    if !ip.is_loopback() {
        return Err(refused_addr(addr));
    }
    Ok(SocketAddr::new(ip, port))
}

/// Where the link listens: `addr` when -link-addr is empty. The link is QUIC, encrypted end to end and opened only with a
/// credential the gate checks, so unlike HTTP it may face other machines; it must still take this machine's own address,
/// where the AI runtime dials it.
pub fn link_addr(flag: &str, addr: SocketAddr) -> Result<SocketAddr, String> {
    if flag.is_empty() {
        return Ok(addr);
    }
    let link: SocketAddr = flag
        .parse()
        .map_err(|_| format!("-link-addr {flag:?}: not an address with a port, as 0.0.0.0:4320"))?;
    if !(link.ip().is_loopback() || link.ip().is_unspecified()) {
        return Err(format!(
            "-link-addr {flag:?}: the AI runtime dials the link on this machine, so it listens on all addresses (0.0.0.0:<port>, [::]:<port>) or on loopback"
        ));
    }
    Ok(link)
}

fn refused_addr(addr: &str) -> String {
    format!(
        "the owner key travels in plain HTTP, so the server listens on this machine alone: set -addr to 127.0.0.1:<port>, not {addr:?}, and reach it from other machines through a proxy you trust"
    )
}

/// Where a process on this machine (an agent, the AI runtime) reaches this server over HTTP.
pub fn local_url(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

/// Where a peer on this machine opens its session on the link.
pub fn link_url(addr: SocketAddr) -> String {
    format!("https://{addr}{}", link_quic::PATH)
}
