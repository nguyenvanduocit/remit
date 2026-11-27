//! The Git directory Remit keeps for a folder worked in place (ADR-0137): `git --git-dir=<data>/folders/<name>.git
//! --work-tree=<folder>`, so the folder's own `.git`, if it has one, is never read or written. Every checkpoint is a commit
//! of what the folder holds then, its `.gitignore` files honoured. What to checkpoint and what a restore refuses is
//! `checkpoint.rs`; this file runs the git.

use std::path::Path;

use crate::checkpoint::{Checkpoint, Refusal, Restore, differing, parse_log, plan_restore, shadow_name};
use chains::git::{path_str, repo_command, stderr_of};

/// How many paths one `git restore` is given: the command line has a limit.
const PATHS_PER_RESTORE: usize = 200;

/// What a restore came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restored {
    Done(Restore),
    Refused(Refusal),
}

/// A folder and the Git directory that holds its checkpoints.
#[derive(Debug, Clone)]
pub struct Shadow {
    dir: String,
    git_dir: String,
}

/// s as one word of a POSIX shell.
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl Shadow {
    /// The shadow of the folder at dir, kept under folders_dir.
    pub fn new(folders_dir: &Path, dir: &str) -> Self {
        Self {
            dir: dir.to_owned(),
            git_dir: path_str(&folders_dir.join(shadow_name(dir))),
        }
    }

    /// git with args over the folder, and what it printed on its output. Commits are Remit's, made with no hook and no
    /// signature; a path is a path, never a pathspec pattern.
    async fn git(&self, args: &[&str]) -> Result<String, String> {
        let mut all = vec![
            "--git-dir",
            &self.git_dir,
            "--work-tree",
            &self.dir,
            "--literal-pathspecs",
            "-c",
            "user.name=Remit",
            "-c",
            "user.email=remit@localhost",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.quotepath=false",
        ];
        all.extend_from_slice(args);
        let out = repo_command(&self.dir, "git", &all)
            .output()
            .await
            .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
        if !out.status.success() {
            return Err(format!(
                "git {}: {}{}",
                args.join(" "),
                out.status,
                stderr_of(&out.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    /// git with args over the checkpoints, as a read: what it printed.
    pub async fn read(&self, args: &[&str]) -> Result<String, String> {
        self.git(args).await
    }

    /// Makes the Git directory when it is not there yet.
    async fn ensure(&self) -> Result<(), String> {
        if Path::new(&self.git_dir).join("HEAD").exists() {
            return Ok(());
        }
        if let Some(parent) = Path::new(&self.git_dir).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        self.git(&["init", "-q", "--template="]).await.map(|_| ())
    }

    /// Takes a checkpoint of the folder as it is now, saying why, and gives its commit: the newest one when nothing in the
    /// folder changed since it. The first checkpoint is taken even of an empty folder.
    pub async fn checkpoint(&self, why: &str) -> Result<String, String> {
        self.ensure().await?;
        self.git(&["add", "-A"]).await?;
        let first = self.git(&["rev-parse", "-q", "--verify", "HEAD"]).await.is_err();
        if first || self.git(&["diff", "--cached", "--quiet"]).await.is_err() {
            self.git(&["commit", "-q", "--allow-empty", "-m", why]).await?;
        }
        self.git(&["rev-parse", "HEAD"]).await
    }

    /// The newest checkpoint.
    pub async fn head(&self) -> Result<String, String> {
        self.git(&["rev-parse", "HEAD"]).await
    }

    /// The files whose content in the folder differs from the newest checkpoint: what its user (or an agent) wrote since.
    pub async fn unsaved(&self) -> Result<Vec<String>, String> {
        self.git(&["add", "-A"]).await?;
        let names = self
            .git(&["diff", "--cached", "--name-only", "-z", "--no-renames", "HEAD"])
            .await?;
        Ok(names.split('\0').filter(|p| !p.is_empty()).map(str::to_owned).collect())
    }

    /// The paths that differ between two checkpoints.
    pub async fn changed_paths(&self, from: &str, to: &str) -> Result<Vec<String>, String> {
        let names = self
            .git(&["diff", "--name-only", "-z", "--no-renames", from, to])
            .await?;
        Ok(names.split('\0').filter(|p| !p.is_empty()).map(str::to_owned).collect())
    }

    /// The stable patch id of the change from one checkpoint to another, "" when it changes nothing.
    pub async fn patch_id(&self, from: &str, to: &str) -> Result<String, String> {
        let script = format!(
            "git --git-dir={} diff --no-ext-diff --no-textconv {} {} | git patch-id --stable",
            quoted(&self.git_dir),
            quoted(from),
            quoted(to)
        );
        let out = chains::git::shell(&self.dir, &script)
            .output()
            .await
            .map_err(|err| format!("patch id: {err}"))?;
        if !out.status.success() {
            return Err(format!("patch id: {}{}", out.status, stderr_of(&out.stderr)));
        }
        Ok(String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned())
    }

    /// The text of the repository's rails file as the checkpoint at holds it; empty when it has none.
    pub async fn rails_at(&self, at: &str, file: &str) -> String {
        self.git(&["show", &format!("{at}:{file}")]).await.unwrap_or_default()
    }

    /// The checkpoints, newest first.
    pub async fn checkpoints(&self) -> Result<Vec<Checkpoint>, String> {
        if !Path::new(&self.git_dir).join("HEAD").exists() {
            return Ok(Vec::new());
        }
        let log = self.git(&["log", "--format=%H%x1f%ct%x1f%s%x1e"]).await?;
        Ok(parse_log(&log))
    }

    /// The command that prints the change between two checkpoints, as a reviewer runs it.
    pub fn diff_command(&self, from: &str, to: &str) -> String {
        format!("git --git-dir={} diff {from} {to}", quoted(&self.git_dir))
    }

    /// Writes the folder back to the checkpoint to, and takes a checkpoint of the result. It writes nothing and refuses
    /// when a file it would overwrite or remove holds an edit no checkpoint holds.
    pub async fn restore(&self, to: &str) -> Result<Restored, String> {
        let unsaved = self.unsaved().await?;
        let status = self
            .git(&["diff", "--cached", "--name-status", "-z", "--no-renames", to])
            .await?;
        let plan = match plan_restore(&differing(&status), &unsaved) {
            Ok(plan) => plan,
            Err(refusal) => return Ok(Restored::Refused(refusal)),
        };
        for chunk in plan.write.chunks(PATHS_PER_RESTORE) {
            let mut args = vec!["restore", "--source", to, "--staged", "--worktree", "--"];
            args.extend(chunk.iter().map(String::as_str));
            self.git(&args).await?;
        }
        for path in &plan.remove {
            let file = Path::new(&self.dir).join(path);
            if let Err(e) = tokio::fs::remove_file(&file).await
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(format!("{}: {e}", file.display()));
            }
        }
        let short = to.get(..8).unwrap_or(to);
        self.checkpoint(&format!("restore {short}")).await?;
        Ok(Restored::Done(plan))
    }
}
