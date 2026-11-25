//! git as Remit runs it in a chain's clones and worktrees: each command in its own process group with the environment
//! childenv names, the clones Remit holds, and the check that a worktree is a chain's own before Remit takes it over or
//! removes it. Every crate that touches a repository (submits, archiving, teaching) runs git through here.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use model::RepoWork;
use serde::Serialize;

use crate::chain_core::Core;

/// A repository as its remote names it: the forge's host and its path there (GitLab's group/project, GitHub's
/// owner/name). It is what Remit knows of a repository; every other fact (default branch, authors) is git's to say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoRemote {
    pub host: String,
    pub project: String,
}

impl RepoRemote {
    pub fn named(&self) -> String {
        format!("{}/{}", self.host, self.project)
    }
}

/// Whether s is one path element of a project or host: it names a directory under the repos directory, so it holds
/// nothing that could climb out of it.
fn segment(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Where a repository lives, read from a git remote URL (scp-like git@host:group/project.git, ssh://, https://) or from
/// host/project, whose host is the first element when it holds a dot, and what git clones it from: a URL as given, and
/// for host/project the ssh form of the host, which uses the machine's own key.
pub fn parse_remote(s: &str) -> Option<(RepoRemote, String)> {
    let s = s.trim();
    let (host, path, clone_url) = if let Some((scheme, rest)) = s.split_once("://") {
        if !matches!(scheme, "ssh" | "https" | "http") {
            return None;
        }
        let (authority, path) = match rest.find('/') {
            Some(i) => (rest.get(..i)?, rest.get(i..)?),
            None => (rest, ""),
        };
        let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = if scheme == "ssh" {
            host_port.split_once(':').map_or(host_port, |(h, _)| h)
        } else {
            host_port
        };
        (host.to_owned(), path.to_owned(), s.to_owned())
    } else if s.contains('@') && s.contains(':') {
        // git@host:group/project.git
        let (at, rest) = s.split_once('@')?;
        if at.contains('/') {
            return None;
        }
        let (host, path) = rest.split_once(':').unwrap_or((rest, ""));
        (host.to_owned(), path.to_owned(), s.to_owned())
    } else {
        // host/group/project
        let (host, path) = s.split_once('/').unwrap_or((s, ""));
        if !host.contains('.') {
            return None;
        }
        let project = path.trim_matches('/');
        let project = project.strip_suffix(".git").unwrap_or(project).trim_matches('/');
        let url = format!("git@{host}:{project}.git");
        (host.to_owned(), path.to_owned(), url)
    };
    let trimmed = path.trim_matches('/');
    let project = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    if !segment(&host.replace(':', "-")) || project.is_empty() || !project.split('/').all(segment) {
        return None;
    }
    Some((
        RepoRemote {
            host: host.to_lowercase(),
            project: project.to_owned(),
        },
        clone_url,
    ))
}

/// The repository s names when it is written as the path of a checkout (absolute, or starting with ~/): its origin as
/// written, before any insteadOf rewrite, read as `parse_remote` reads a remote; nothing else of the checkout is used.
/// None when s is no path; the error says why the checkout names no repository.
pub async fn checkout_remote(s: &str) -> Option<Result<(RepoRemote, String), String>> {
    let s = s.trim();
    let dir = match s.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var_os("HOME")?).join(rest),
        None => PathBuf::from(s),
    };
    if !dir.is_absolute() {
        return None;
    }
    let d = path_str(&dir);
    let named = match git(&d, &["config", "--get", "remote.origin.url"]).await {
        Ok(origin) => {
            parse_remote(&origin).ok_or_else(|| format!("the origin of {d} ({origin}) is not on a host Remit can name"))
        }
        Err(err) => Err(format!("{d} has no origin remote to name the repository by: {err}")),
    };
    Some(named)
}

/// Brings Remit's clone at dir up to date with its remote, making it first from clone_url when it does not exist (empty:
/// the clone is there already). A clone is made beside its place and moved in, so a crash or a second chain opening the
/// same repository never sees half of one.
pub async fn ensure_clone(dir: &Path, clone_url: &str) -> Result<(), String> {
    let d = path_str(dir);
    if dir.join(".git").exists() {
        return git(&d, &["fetch", "-q", "origin"]).await.map(|_| ());
    }
    if clone_url.is_empty() {
        return Err(format!("no clone at {d}"));
    }
    let parent = dir.parent().ok_or_else(|| format!("{d} has no parent directory"))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| format!("{}: {e}", parent.display()))?;
    let tmp = parent.join(crate::chain_core::new_id(".clone-"));
    let tmp_s = path_str(&tmp);
    let made = async {
        git(&path_str(parent), &["clone", "-q", clone_url, &tmp_s]).await?;
        match tokio::fs::rename(&tmp, dir).await {
            Ok(()) => Ok(()),
            Err(_) if dir.join(".git").exists() => Ok(()), // another chain made it first
            Err(e) => Err(format!("{d}: {e}")),
        }
    }
    .await;
    // what stays is a hidden temporary directory
    tokio::fs::remove_dir_all(&tmp).await.ok();
    made
}

/// A command that runs in an agent's clone, as its own process group (a fetch or push ends with the ssh it started). The
/// clone is the agent's to write, so what the command runs is the agent's too: a test file, a hook, an alias or diff
/// driver in the clone's config. It starts with the environment childenv names, never the server's whole.
pub fn repo_command(dir: &str, name: &str, args: &[&str]) -> procgroup::Command {
    let parent: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    repo_command_in(&parent, dir, name, args)
}

/// `repo_command` for a server whose environment is parent (`NAME=value` entries).
fn repo_command_in(parent: &[String], dir: &str, name: &str, args: &[&str]) -> procgroup::Command {
    let mut cmd = procgroup::command(name, args);
    cmd.current_dir(dir).env_clear().envs(
        childenv::of::<String, &str>(parent, &[])
            .iter()
            .filter_map(|kv| kv.split_once('=')),
    );
    cmd
}

/// Runs script with sh -c in dir, as its own process group: when the caller's future is dropped, everything the script
/// started (go test and its test binaries, a pipeline's other side) ends with it.
pub fn shell(dir: &str, script: &str) -> procgroup::Command {
    repo_command(dir, "sh", &["-c", script])
}

/// Runs git with args in dir, as the agent's clone's command, and says what it printed, trimmed: its output then its
/// errors. A failure says the command, how it ended and the tail of what it printed.
pub async fn git(dir: &str, args: &[&str]) -> Result<String, String> {
    let out = repo_command(dir, "git", args)
        .output()
        .await
        .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
    let mut printed = String::from_utf8_lossy(&out.stdout).into_owned();
    printed.push_str(&String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        return Err(format!(
            "git {}: {}: {}",
            args.join(" "),
            out.status,
            tail(&printed, 400)
        ));
    }
    Ok(printed.trim().to_owned())
}

/// git's reason for a command that failed, from its errors as `git` appends them: ": <their tail>", or "" when it
/// printed none.
pub fn stderr_of(stderr: &[u8]) -> String {
    if stderr.is_empty() {
        return String::new();
    }
    format!(": {}", tail(&String::from_utf8_lossy(stderr), 400))
}

/// The last n bytes of s, trimmed, after an ellipsis when s was longer (cut on a character's boundary).
pub fn tail(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.len() <= n {
        return s.to_owned();
    }
    let mut from = s.len().saturating_sub(n);
    while !s.is_char_boundary(from) {
        from = from.saturating_add(1);
    }
    format!("…{}", s.get(from..).unwrap_or_default())
}

/// p as git and the other commands are given it.
pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Checks that rw.path is the chain's own worktree: the root of a git worktree checked out on the chain's branch, which
/// no other chain uses. Remit takes over or removes only such a directory.
pub async fn own_worktree(rw: &RepoWork) -> Result<(), String> {
    let top = git(&rw.path, &["rev-parse", "--show-toplevel"])
        .await
        .map_err(|_| format!("{} is not a git worktree", rw.path))?;
    if !same_path(Path::new(&top), Path::new(&rw.path)) {
        return Err(format!("{} is inside {top}, not a worktree of its own", rw.path));
    }
    let head = git(&rw.path, &["symbolic-ref", "-q", "--short", "HEAD"])
        .await
        .map_err(|err| {
            format!(
                "{} is not on any branch, not on the chain's branch {}: {err}",
                rw.path, rw.branch
            )
        })?;
    if head != rw.branch {
        return Err(format!(
            "{} is on {head:?}, not on the chain's branch {}",
            rw.path, rw.branch
        ));
    }
    Ok(())
}

/// Whether a and b are the same directory once their links are followed.
pub fn same_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// A check a submit ran on a change: the command, whether it passed, and the tail of what it printed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Check {
    pub command: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub output: String,
}

/// The checks of a submit, run on the repository at dir, whose change touches the files in changed: it gives the checks
/// that ran, and stops at the first that fails. It is a dependency so that a test runs no go build, vet or test.
pub type CheckRunner = Arc<
    dyn Fn(String, Vec<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Check>> + Send>>
        + Send
        + Sync,
>;

/// Lists the clones under dir, a directory of host's: a directory holding .git is one, and nothing inside it is looked
/// at. A directory that cannot be read holds no clone to list.
fn walk_clones(root: &Path, dir: &Path, host: &str, out: &mut Vec<RepoRemote>) {
    if dir.join(".git").exists() {
        let rel = dir.strip_prefix(root).map(path_str).unwrap_or_default();
        out.push(RepoRemote {
            host: host.to_owned(),
            project: if rel.is_empty() {
                ".".to_owned()
            } else {
                rel.replace(std::path::MAIN_SEPARATOR, "/")
            },
        });
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut subs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    subs.sort();
    for s in subs {
        walk_clones(root, &s, host, out);
    }
}

impl Core {
    /// Where Remit keeps its own clone of r.
    pub fn clone_dir(&self, r: &RepoRemote) -> PathBuf {
        let mut p = self.cfg.repos_dir.join(&r.host);
        p.extend(r.project.split('/'));
        p
    }
    /// The repositories Remit already holds a clone of.
    pub fn clones(&self) -> Vec<RepoRemote> {
        let Ok(hosts) = std::fs::read_dir(&self.cfg.repos_dir) else {
            return Vec::new(); // no repos directory yet: nothing is cloned
        };
        let mut out = Vec::new();
        for h in hosts.flatten() {
            let host = h.file_name().to_string_lossy().into_owned();
            let root = h.path();
            if root.is_dir() {
                walk_clones(&root, &root, &host, &mut out);
            }
        }
        out
    }
}

/// The first eight characters of a commit id.
pub fn short(sha: &str) -> &str {
    sha.get(..8).unwrap_or(sha)
}
