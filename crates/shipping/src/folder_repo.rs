//! The folder's own Git repository, written once when a version of the folder worked in place lands (ADR-0139): a commit
//! on the branch the folder is on, of the version's paths as its approved checkpoint holds them. It is built in a
//! temporary index with `commit-tree`, so no hook runs, the checkout never moves and the user's working tree is not
//! written; the branch moves only if it still points where it was read. The pure part decides the index edits and the
//! message; the rest runs the git.

use std::collections::BTreeMap;

use model::Landing;

use crate::shadow::Shadow;
use chains::git::{path_str, repo_command, stderr_of};

/// How many index entries one `git update-index` or `git reset` is given: the command line has a limit.
const PATHS_PER_CALL: usize = 200;

/// How often a commit is built again when the branch moved while it was built.
const TRIES: usize = 3;

/// A gitlink's mode: a nested repository's commit, whose object lives in that repository, not this one.
const GITLINK: &str = "160000";

/// One change to the index the commit is built from, its path relative to the repository's top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// The checkpoint holds the path: it gets this mode and object.
    Put { path: String, mode: String, object: String },
    /// The checkpoint has no such path: it leaves the tree.
    Remove { path: String },
}

/// The entries `git ls-tree -z` prints (`<mode> <type> <object>\t<path>\0`), by path: mode and object. Pure.
pub fn parse_ls_tree(out: &str) -> BTreeMap<String, (String, String)> {
    out.split('\0')
        .filter_map(|rec| {
            let (meta, path) = rec.split_once('\t')?;
            let mut f = meta.split(' ');
            let (mode, _kind, object) = (f.next()?, f.next()?, f.next()?);
            Some((path.to_owned(), (mode.to_owned(), object.to_owned())))
        })
        .collect()
}

/// The index edits that make HEAD's tree hold paths as the checkpoint does (`at`, from `parse_ls_tree`). Pure.
pub fn edits(paths: &[String], at: &BTreeMap<String, (String, String)>) -> Vec<Edit> {
    paths
        .iter()
        .map(|p| match at.get(p) {
            Some((mode, object)) => Edit::Put {
                path: p.clone(),
                mode: mode.clone(),
                object: object.clone(),
            },
            None => Edit::Remove { path: p.clone() },
        })
        .collect()
}

/// The commit message of landing l: the Chair's title, then its description. Pure.
pub fn message(l: &Landing) -> String {
    let title = l.version.title.trim();
    let body = l.version.description.trim();
    if body.is_empty() {
        title.to_owned()
    } else {
        format!("{title}\n\n{body}")
    }
}

/// A folder that is the top of a Git working tree, as the user keeps it.
#[derive(Debug, Clone)]
pub struct FolderRepo {
    dir: String,
}

impl FolderRepo {
    /// The repository the folder at dir is the top of; none when it is not Git, or is a folder inside a larger
    /// repository (whose history is not this folder's to write).
    pub async fn at(dir: &str) -> Option<Self> {
        let me = Self { dir: dir.to_owned() };
        let top = me.git(&["rev-parse", "--show-toplevel"], None).await.ok()?;
        let same = match (tokio::fs::canonicalize(&top).await, tokio::fs::canonicalize(dir).await) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        same.then_some(me)
    }

    /// git with args in the folder, with index as its index file when given, and what it printed on its output.
    async fn git(&self, args: &[&str], index: Option<&str>) -> Result<String, String> {
        let mut all = vec!["--literal-pathspecs"];
        all.extend_from_slice(args);
        let mut cmd = repo_command(&self.dir, "git", &all);
        if let Some(index) = index {
            cmd.env("GIT_INDEX_FILE", index);
        }
        let out = cmd
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

    /// Commits landing l on the branch the folder is on (or its detached HEAD): HEAD's tree with l's paths as checkpoint
    /// `l.version.from` of shadow holds them, and l's message. Gives the commit, or none when HEAD already holds them so.
    /// The user's index then shows those paths as the new HEAD has them.
    pub async fn commit(&self, l: &Landing, shadow: &Shadow) -> Result<Option<String>, String> {
        let at = parse_ls_tree(&shadow.entries(&l.version.from, &l.version.paths).await?);
        let edits = edits(&l.version.paths, &at);
        for e in &edits {
            if let Edit::Put { mode, object, .. } = e
                && mode != GITLINK
                && self.git(&["cat-file", "-e", object], None).await.is_err()
            {
                shadow.copy_object(object, &self.dir).await?;
            }
        }
        let index = path_str(std::path::Path::new(
            &self
                .git(
                    &["rev-parse", "--path-format=absolute", "--git-path", "remit-land-index"],
                    None,
                )
                .await?,
        ));
        let made = self.build(&edits, &message(l), &index).await;
        tokio::fs::remove_file(&index).await.ok();
        let made = made?;
        if made.is_some() {
            let paths: Vec<&str> = l.version.paths.iter().map(String::as_str).collect();
            for chunk in paths.chunks(PATHS_PER_CALL) {
                let mut args = vec!["reset", "-q", "--"];
                args.extend_from_slice(chunk);
                self.git(&args, None).await?;
            }
        }
        Ok(made)
    }

    /// Builds the commit in the index file index and moves the branch to it, building again when the branch moved
    /// meanwhile.
    async fn build(&self, edits: &[Edit], message: &str, index: &str) -> Result<Option<String>, String> {
        for _ in 0..TRIES {
            let head = self
                .git(&["rev-parse", "-q", "--verify", "HEAD^{commit}"], None)
                .await
                .ok();
            let branch = self
                .git(&["symbolic-ref", "-q", "HEAD"], None)
                .await
                .unwrap_or_else(|_| "HEAD".to_owned());
            match &head {
                Some(h) => self.git(&["read-tree", h], Some(index)).await?,
                None => self.git(&["read-tree", "--empty"], Some(index)).await?,
            };
            let mut puts: Vec<String> = Vec::new();
            let mut removes: Vec<&str> = Vec::new();
            for e in edits {
                match e {
                    Edit::Put { path, mode, object } => puts.push(format!("{mode},{object},{path}")),
                    Edit::Remove { path } => removes.push(path),
                }
            }
            for chunk in puts.chunks(PATHS_PER_CALL) {
                let mut args = vec!["update-index", "--add"];
                for p in chunk {
                    args.extend(["--cacheinfo", p.as_str()]);
                }
                self.git(&args, Some(index)).await?;
            }
            for chunk in removes.chunks(PATHS_PER_CALL) {
                let mut args = vec!["update-index", "--force-remove", "--"];
                args.extend_from_slice(chunk);
                self.git(&args, Some(index)).await?;
            }
            let tree = self.git(&["write-tree"], Some(index)).await?;
            if let Some(h) = &head
                && self.git(&["rev-parse", &format!("{h}^{{tree}}")], None).await? == tree
            {
                return Ok(None);
            }
            let mut args = vec!["commit-tree", tree.as_str(), "-m", message];
            if let Some(h) = &head {
                args.extend(["-p", h.as_str()]);
            }
            let commit = self.git(&args, None).await?;
            let old = head.as_deref().unwrap_or("");
            match self
                .git(&["update-ref", "-m", "remit: land", &branch, &commit, old], None)
                .await
            {
                Ok(_) => return Ok(Some(commit)),
                Err(err) => {
                    let now = self
                        .git(&["rev-parse", "-q", "--verify", "HEAD^{commit}"], None)
                        .await
                        .ok();
                    if now == head {
                        return Err(err);
                    }
                }
            }
        }
        Err(format!(
            "the folder's branch kept moving while Remit committed to it ({TRIES} tries)"
        ))
    }
}

