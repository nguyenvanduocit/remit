//! A chain's code work happens in its own git worktree of each repo it touches, on a branch only it uses and never
//! pushes: agents commit there, and landing the change is the backend's (land.rs, queue.rs): it checks each version, has
//! its gates judge it and combines it with the base in the repository's queue, which pushes the base forward itself.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use forge::Account;
use model::{Card, Chain, DecisionKind, RepoWork, by_agent};

use crate::Shipping;
use crate::land::{find_work, last_element};
use chains::apply::Records;
use chains::git::Check;
use chains::git::RepoRemote;
use chains::git::checkout_remote;
use chains::git::ensure_clone;
use chains::git::git;
use chains::git::own_worktree;
use chains::git::parse_remote;
use chains::git::path_str;
use chains::git::repo_command;
use chains::git::tail;
use chains::step::{ChainCmd, Decided, WorkCmd, replace_with};
use chains::{Error, Result};

/// The one clone q names: by host/project, by its project path or by its last element.
pub(crate) fn named_clone(have: &[RepoRemote], q: &str) -> Result<RepoRemote, String> {
    let q = q.trim_matches('/');
    let hit: Vec<&RepoRemote> = have
        .iter()
        .filter(|r| q == r.named() || q == r.project || q == last_element(&r.project))
        .collect();
    match hit.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!(
            "repository {q:?} is not one Remit holds: name it by its remote (github.com/owner/name), a remote URL or the \
             path of a checkout{}",
            held_text(have)
        )),
        many => {
            let names: Vec<String> = many.iter().map(|r| r.named()).collect();
            Err(format!("{q:?} matches several repositories: {}", names.join(", ")))
        }
    }
}

/// The one repository of a chain's project (its remotes, `host/group/project`) that q names by its path or its last
/// element, with what to clone it from; none when no repository does, so q resolves as any other name. Several is a
/// refusal naming them: the project says which product q means, so another clone of that name is not it. Pure.
pub(crate) fn project_repo(repos: &[String], q: &str) -> std::result::Result<Option<(RepoRemote, String)>, String> {
    let q = q.trim_matches('/');
    let mut hits: Vec<(RepoRemote, String)> = repos
        .iter()
        .filter_map(|s| parse_remote(s))
        .filter(|(r, _)| q == r.project || q == last_element(&r.project))
        .collect();
    if hits.len() > 1 {
        let names: Vec<String> = hits.iter().map(|(r, _)| r.named()).collect();
        return Err(format!(
            "{q:?} matches several of the project's repositories: {}",
            names.join(", ")
        ));
    }
    Ok(hits.pop())
}

/// The clones Remit holds, so a name that matched none is retried with one that does.
fn held_text(have: &[RepoRemote]) -> String {
    if have.is_empty() {
        return "; Remit holds no clone yet".to_owned();
    }
    let names: Vec<String> = have.iter().map(RepoRemote::named).collect();
    format!("; it holds: {}", names.join(", "))
}

/// s lower-cased, every run of other characters than a-z and 0-9 one dash, cut to limit bytes; "work" when nothing is
/// left.
pub(crate) fn slug(s: &str, limit: usize) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    let mut s = out.as_str();
    if s.len() > limit {
        s = s.get(..limit).unwrap_or(s).trim_matches('-');
    }
    if s.is_empty() { "work".to_owned() } else { s.to_owned() }
}

/// Where a chain's change lands: develop wherever it exists, as Git Flow (the team's flow) sends features and fixes there even
/// when the repository's default branch is master; else the default branch.
pub(crate) async fn base_branch(clone: &str) -> String {
    if git(clone, &["rev-parse", "--verify", "-q", "refs/remotes/origin/develop"])
        .await
        .is_ok()
    {
        return "develop".to_owned();
    }
    if let Ok(r) = git(clone, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).await {
        return r.strip_prefix("origin/").unwrap_or(&r).to_owned();
    }
    for b in ["main", "master"] {
        if git(
            clone,
            &["rev-parse", "--verify", "-q", &format!("refs/remotes/origin/{b}")],
        )
        .await
        .is_ok()
        {
            return b.to_owned();
        }
    }
    "main".to_owned()
}

/// How long opening a repository may take: a first clone of a large one.
const OPEN_TIMEOUT: Duration = Duration::from_secs(180);

impl Shipping {
    /// Which repository q names and what to clone it from ("" for one Remit holds): a checkout's path (its origin is
    /// read, nothing else of it is used), a remote as `parse_remote` reads it, one of the chain's project's repositories
    /// (`project_repos`, ADR-0108) whether or not Remit holds it, or a repository Remit already holds.
    pub(crate) async fn resolve_repo(&self, q: &str, project_repos: &[String]) -> Result<(RepoRemote, String)> {
        let q = q.trim();
        if let Some(found) = checkout_remote(q).await {
            return found.map_err(Error::Refused);
        }
        if let Some(found) = parse_remote(q) {
            return Ok(found);
        }
        if let Some(found) = project_repo(project_repos, q).map_err(Error::Refused)? {
            return Ok(found);
        }
        let r = named_clone(&self.clones(), q).map_err(Error::Refused)?;
        Ok((r, String::new()))
    }

    /// Gives the chain a worktree of a repository on its own branch, from the branch its changes land on. The repository is
    /// named by its remote, or by a short name its project's repositories resolve first (see `resolve_repo`); Remit
    /// clones it into its own repos directory the first time.
    pub async fn open_repo(self: &Arc<Self>, chair: &Card, query: &str) -> Result<RepoWork> {
        let chain = self.chain(&chair.home).await?;
        if let Some(rw) = find_work(&chain.repos, query).and_then(|i| chain.repos.get(i)) {
            return Ok(rw.clone()); // already open
        }
        let ch = chain.clone();
        let project_repos = self
            .store
            .read(move |r| match chains::project::project_of(r, &ch)? {
                Some(id) => r.project(&id).map(|p| p.repos),
                None => Ok(Vec::new()),
            })
            .await?;
        let (remote, clone_url) = self.resolve_repo(query, &project_repos).await?;
        if let Err(err) = self.forge_on(&remote.host) {
            return Err(Error::refused(format!("{} cannot be published: {err}", remote.named())));
        }
        if let Some(other) = chain
            .repos
            .iter()
            .find(|rw| rw.project == remote.project && rw.repo == remote.project)
        {
            return Err(Error::refused(format!(
                "the chain already has {} open on {}; a chain holds one repository of that name",
                remote.project, other.host
            )));
        }
        let clone = self.clone_dir(&remote);
        let mut rw = RepoWork {
            repo: remote.project.clone(),
            project: remote.project.clone(),
            host: remote.host.clone(),
            branch: branch_for(&chain),
            path: path_str(&Path::new(&chain.workdir).join(slug(last_element(&remote.project), 40))),
            ..RepoWork::default()
        };
        let opened = tokio::time::timeout(OPEN_TIMEOUT, async {
            ensure_clone(&clone, &clone_url).await?;
            let c = path_str(&clone);
            rw.base = base_branch(&c).await;
            if Path::new(&rw.path).exists() {
                // Only the chain's own worktree, made before a crash kept it from being recorded, is taken over: anything
                // else there is another chain's or the owner's.
                own_worktree(&rw).await.map_err(|err| {
                    format!(
                        "{} is there already and is not this chain's worktree, so it is left alone: {err}",
                        rw.path
                    )
                })?;
            } else {
                let from = format!("origin/{}", rw.base);
                git(&c, &["worktree", "add", "-b", &rw.branch, &rw.path, &from]).await?;
            }
            Ok::<(), String>(())
        })
        .await;
        match opened {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return Err(Error::Refused(err)),
            Err(_) => {
                return Err(Error::refused(format!(
                    "opening {} took longer than {OPEN_TIMEOUT:?}",
                    remote.named()
                )));
            }
        }
        let d = Decided {
            kind: DecisionKind::Repo,
            text: format!("Open {} on branch {} (from {}).", rw.repo, rw.branch, rw.base),
            why: String::new(),
            card_id: chair.id.clone(),
            by: by_agent(&chair.id),
        };
        self.record_work(&chain.id, &rw.repo, replace_with(rw.clone()), vec![d])
            .await?;
        Ok(rw)
    }

    /// The account rw's forge works as: the chain's commits there are made under its name and email, as a forge's push
    /// rules check every commit's author and committer against the account that pushes.
    pub(crate) async fn pusher(&self, rw: &RepoWork) -> Result<Account> {
        self.pusher_on(&rw.host).await
    }

    /// The account the forge on host works as, with the name and email its commits are made under.
    pub(crate) async fn pusher_on(&self, host: &str) -> Result<Account> {
        let f = self.forge_on(host)?;
        let a = f.account(host).await.map_err(|err| Error::refused(err.to_string()))?;
        if a.name.is_empty() || a.email.is_empty() {
            return Err(Error::refused(format!(
                "the {} account on {host} has no name and email",
                f.name()
            )));
        }
        Ok(a)
    }

    /// Stores the change edit makes to the chain's work on repo, with the decisions that go with it.
    pub async fn record_work(
        &self,
        chain_id: &str,
        repo: &str,
        edit: chains::step::RepoEdit,
        decisions: Vec<Decided>,
    ) -> Result<chains::apply::Applied> {
        let cmd = ChainCmd::Work(WorkCmd {
            repo: repo.to_owned(),
            edit,
            tell: String::new(),
        });
        self.apply(
            chain_id,
            cmd,
            Records {
                decisions,
                ..Records::default()
            },
        )
        .await
    }
}

/// The branch's commits (beyond base) that already exist on another remote branch, the repository's default one, by
/// patch or by subject: a landing must carry only the chain's own work, and a rebase onto develop easily drags along hotfixes
/// that live on master.
pub(crate) async fn foreign_commits(dir: &str, base: &str) -> Result<Vec<String>, String> {
    let def = match git(dir, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).await {
        Ok(d) if d != format!("origin/{base}") => d,
        _ => return Ok(Vec::new()), // no other line to compare with
    };
    let marked = git(
        dir,
        &[
            "log",
            "--format=%m %h %s",
            "--cherry-mark",
            "--right-only",
            "--no-merges",
            &format!("{def}...HEAD"),
        ],
    )
    .await?;
    let own_range = git(dir, &["log", "--format=%h", &format!("origin/{base}..HEAD")]).await?;
    let subjects = git(dir, &["log", "--format=%s", &format!("origin/{base}..{def}")]).await?;
    let on_default: HashSet<&str> = subjects.split('\n').collect();
    let mut foreign = pick_foreign(&marked, &own_range, &on_default);
    // The default branch's own commits (merges included) on top of base: the branch was built on it.
    let mb = git(dir, &["merge-base", &def, "HEAD"]).await?;
    let shared = git(dir, &["log", "--format=%h %s", &mb, &format!("^origin/{base}")]).await?;
    foreign.extend(shared.split('\n').filter(|l| !l.is_empty()).map(str::to_owned));
    Ok(foreign)
}

/// `foreign_commits`' pure part: of the lines "<mark> <sha> <subject>" (git log --cherry-mark), the ones within the
/// branch's own range that are patch-equal ("=") to, or share a subject with, a commit on the other line.
pub(crate) fn pick_foreign(marked: &str, own_range: &str, on_default: &HashSet<&str>) -> Vec<String> {
    let own: HashSet<&str> = own_range.split_whitespace().collect();
    let mut out = Vec::new();
    for line in marked.split('\n') {
        let mut parts = line.splitn(3, ' ');
        let (Some(mark), Some(sha), Some(subject)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if own.contains(sha) && (mark == "=" || on_default.contains(subject)) {
            out.push(format!("{sha} {subject}"));
        }
    }
    out
}

/// Names a chain's worktree branch: "mc-" and the chain id mark it as Remit's own. It stays in Remit's clone; what lands
/// is the queue's commit on the base.
pub(crate) fn branch_for(chain: &Chain) -> String {
    let id = chain.id.strip_prefix("ch_").unwrap_or(&chain.id);
    format!("feature/mc-{id}-{}", slug(&chain.title, 32))
}

/// The checks the repository's language implies, and only Go is known. A Go module is built whole, so a change cannot
/// break what it did not touch; vet and test run on the packages the change touched, and a vet finding counts only when
/// it is in a changed file: findings the base already had are not this chain's to fix. The packages come from paths an
/// agent named, so go is given each as one argument, through no shell.
pub(crate) async fn go_checks(dir: &str, changed: &[String]) -> Vec<Check> {
    if !Path::new(dir).join("go.mod").exists() {
        return Vec::new();
    }
    let pkgs: Vec<String> = changed_packages(changed)
        .into_iter()
        .filter(|p| Path::new(dir).join(p).exists())
        .collect();
    let mut runs: Vec<Vec<String>> = vec![vec!["build".to_owned(), "./...".to_owned()]];
    if !pkgs.is_empty() {
        for verb in ["vet", "test"] {
            runs.push(std::iter::once(verb.to_owned()).chain(pkgs.iter().cloned()).collect());
        }
    }
    let mut out = Vec::new();
    for args in runs {
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let (ok, printed) = match repo_command(dir, "go", &argv).output().await {
            Ok(o) => {
                let mut p = String::from_utf8_lossy(&o.stdout).into_owned();
                p.push_str(&String::from_utf8_lossy(&o.stderr));
                (o.status.success(), p)
            }
            Err(err) => (false, err.to_string()),
        };
        let mut ck = Check {
            command: format!("go {}", args.join(" ")),
            ok,
            output: tail(&printed, 1500),
        };
        if !ck.ok && args.first().is_some_and(|a| a == "vet") {
            let mine = vet_findings_in(&printed, changed);
            if mine.is_empty() {
                ck.ok = true;
                "only findings outside the change (already on the base)".clone_into(&mut ck.output);
            } else {
                ck.output = tail(&mine.join("\n"), 1500);
            }
        }
        let failed = !ck.ok;
        out.push(ck);
        if failed {
            break;
        }
    }
    out
}

/// The Go package directories that hold the changed file paths, as "./dir" patterns, sorted.
pub(crate) fn changed_packages(changed: &[String]) -> Vec<String> {
    let pkgs: BTreeSet<String> = changed
        .iter()
        .filter(|f| f.ends_with(".go"))
        .map(|f| match f.rsplit_once('/') {
            Some((dir, _)) if !dir.is_empty() => format!("./{dir}"),
            _ => ".".to_owned(),
        })
        .collect();
    pkgs.into_iter().collect()
}

/// The lines of go vet's output that point into one of the changed files.
pub(crate) fn vet_findings_in(output: &str, changed: &[String]) -> Vec<String> {
    output
        .split('\n')
        .filter(|line| {
            let first = line.trim().split(':').next().unwrap_or_default();
            let path = first.strip_prefix("./").unwrap_or(first);
            !path.is_empty() && changed.iter().any(|f| f == path)
        })
        .map(|l| l.trim().to_owned())
        .collect()
}
