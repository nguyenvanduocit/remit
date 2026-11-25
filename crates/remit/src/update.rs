//! `remit update -repo <owner/name> -restart <command>`: puts a GitHub release live on the machine that runs the server,
//! in one command that can be run again at any point (docs/deploy.md, *Install a release*). Under a lock in the releases
//! directory, so two never race, it:
//!  1. finds the release (`-to`, else the newest) and what `releases/current` and the running server are on: when both are
//!     on it already there is nothing to do but check it;
//!  2. downloads the server's asset with `gh` and checks it against the release's SHA256SUMS;
//!  3. copies the database (`store::snapshot_before_update`, beside the server that writes it);
//!  4. installs the binary as `<releases>/<tag>/remit` and points `current` at it and `previous` at what `current` was,
//!     each by a rename, so a supervisor that restarts in the middle finds one whole binary;
//!  5. restarts the server through its supervisor (`-restart`, run by `sh -c`: restarting is the supervisor's);
//!  6. waits until `/healthz` is healthy and names the release's commit and a hello with the owner key is welcomed on
//!     the link;
//!  7. when it is not by `-wait`, or the restart command fails, points `current` back at the previous release and
//!     restarts again. A release that migrated the database (its schema version is past the copy's) is one-way: the
//!     release before refuses a database migrated past what it knows, so the way back also puts the copy back
//!     (`store::restore`, once no server holds the database), losing only what the failed release wrote. A release
//!     that came up healthy keeps what it migrated: its way back is forward.
//!
//! It writes one JSON object, `{id, from, to, changed, backup, healthy, rolledBack, oneWay, error}`, on stdout and, under
//! the lock, to `<releases>/update.json`, where the server reads the last update's outcome by its id (ADR-0087), and
//! exits 1 unless the server ends healthy on the release asked for. The decisions are pure (`plan`, `runs`, `asset`,
//! `listed_sum`); `run` acts.

use std::fs::{File, OpenOptions, TryLockError};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::dash_flags::{self, Flag, Kind};
use crate::probe;

/// The file whose lock says an update is under way, in the releases directory: the server reads it too, so it starts
/// none while one runs (orch `install_release`).
pub const LOCK: &str = "update.lock";

/// The file the last update's outcome is written to, in the releases directory.
const OUTCOME: &str = "update.json";

/// How long between two looks at a restarted server: a look is a loopback request, so a short gap costs nothing and
/// the update reports as soon as the server is up.
const LOOK_EVERY: Duration = Duration::from_millis(200);

/// The commit a release's tag names: `<yyyymmdd>-<short commit>`, as the release workflow tags what it publishes
/// (.github/workflows/release.yml, scripts/release_tag.sh).
pub fn commit_of(tag: &str) -> Option<&str> {
    tag.split_once('-').map(|(_, c)| c).filter(|c| !c.is_empty())
}

/// Whether a server that reports revision on `/healthz` runs the release tag.
pub fn runs(tag: &str, revision: &str) -> bool {
    commit_of(tag).is_some_and(|c| revision.starts_with(c))
}

/// The release asset that is the server for this OS and architecture (`std::env::consts`), as the release names it.
pub fn asset(os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "linux" => "linux",
        "macos" => "darwin",
        _ => return None,
    };
    let arch = match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return None,
    };
    Some(format!("remit-{os}-{arch}"))
}

/// The SHA-256 that SHA256SUMS (as `sha256sum` writes it) lists for name.
pub fn listed_sum<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (sum, file) = line.split_once(char::is_whitespace)?;
        (file.trim_start().trim_start_matches('*') == name).then_some(sum)
    })
}

/// What an update has to do, from the release `current` points at, the one asked for and the commit the server runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Download and install the release, then restart.
    Install,
    /// The release is installed but the server runs another: restart.
    Restart,
    /// Installed and running: only check it.
    Check,
}

pub fn plan(current: Option<&str>, target: &str, running: Option<&str>) -> Plan {
    if current != Some(target) {
        Plan::Install
    } else if running.is_some_and(|r| runs(target, r)) {
        Plan::Check
    } else {
        Plan::Restart
    }
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq)]
struct Settings {
    addr: String,
    data: PathBuf,
    releases: PathBuf,
    repo: String,
    gh: String,
    to: Option<String>,
    restart: String,
    wait: Duration,
}

fn own_flags() -> Vec<Flag> {
    let f = |name, kind, default: &str, usage| Flag {
        name,
        kind,
        default: default.to_owned(),
        usage,
    };
    vec![
        f(
            "repo",
            Kind::Str,
            "",
            "the GitHub repository the releases come from, owner/name (required)",
        ),
        f(
            "gh",
            Kind::Str,
            "gh",
            "gh binary, signed in with read access to -repo, that finds and downloads the release",
        ),
        f(
            "releases",
            Kind::Str,
            "",
            "the directory of the installed releases, each <tag>/remit, with the links current and previous (default <data>/releases)",
        ),
        f("to", Kind::Str, "", "the release's tag (default the newest)"),
        f(
            "restart",
            Kind::Str,
            "",
            "the command, run by sh -c, with which the server's supervisor restarts it on releases/current: pm2 restart remit, systemctl restart remit, launchctl kickstart -k gui/<uid>/remit (required)",
        ),
        f(
            "wait",
            Kind::Duration,
            "2m0s",
            "how long the restarted server has to be healthy on the release's commit and welcome the owner on the link before the update goes back to the previous release",
        ),
    ]
}

impl Settings {
    /// The settings a parsed command line holds, or what is missing. Pure.
    fn of(p: &dash_flags::Parsed) -> Result<Self, String> {
        let repo = p.str("repo");
        if repo.split_once('/').is_none_or(|(o, n)| o.is_empty() || n.is_empty()) {
            return Err("-repo names the GitHub repository as owner/name".to_owned());
        }
        let restart = p.str("restart");
        if restart.trim().is_empty() {
            return Err("-restart names the command that restarts the server through its supervisor".to_owned());
        }
        let data = PathBuf::from(p.str("data"));
        let releases = match p.str("releases") {
            r if r.is_empty() => data.join("releases"),
            r => PathBuf::from(r),
        };
        Ok(Self {
            addr: p.str("addr"),
            data,
            releases,
            repo,
            gh: p.str("gh"),
            to: Some(p.str("to")).filter(|t| !t.is_empty()),
            restart,
            wait: p.duration("wait"),
        })
    }
}

/// What the update did, written as one JSON object.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    /// This update's own id: the server records each update's outcome once.
    id: String,
    /// The release `current` pointed at before.
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    to: String,
    /// A release was installed or the server restarted.
    changed: bool,
    /// The copy of the database taken before the restart.
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<String>,
    /// The server ended healthy: on `to`, or after a rollback on `from`.
    healthy: bool,
    rolled_back: bool,
    /// The release migrated the database: going back to the release before needs the copy back.
    one_way: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    error: String,
}

/// Updates as `args` say and prints the outcome; an error unless the server ends healthy on the release asked for.
pub async fn run(args: &[String], home: &Path) -> Result<(), String> {
    let defs = crate::flags::subcommand_defs(home, &["addr", "data"], own_flags());
    let parsed = match dash_flags::parse(&defs, args) {
        Ok(p) => p,
        Err(dash_flags::Error::Help) => {
            eprintln!("Usage of remit update:\n{}", dash_flags::usage(&defs));
            return Ok(());
        }
        Err(dash_flags::Error::Bad(why)) => return Err(why),
    };
    let s = Settings::of(&parsed)?;
    let mut out = Outcome {
        id: update_id(),
        ..Outcome::default()
    };
    let (done, written) = match lock(&s.releases) {
        Ok(_lock) => {
            let done = update(&s, &mut out).await;
            if let Err(why) = &done {
                out.error.clone_from(why);
            }
            // Under the lock, so the file holds one update's outcome whole.
            (done, write_outcome(&s.releases, &out))
        }
        // Another update is under way: its outcome is the one the file is for.
        Err(why) => {
            out.error.clone_from(&why);
            (Err(why), Ok(()))
        }
    };
    println!("{}", serde_json::to_string(&out).map_err(|e| e.to_string())?);
    written?;
    match done {
        Ok(()) if out.healthy => Ok(()),
        Ok(()) => Err(format!("{} is installed but the server is not healthy on it", out.to)),
        Err(why) => Err(why),
    }
}

/// A new update id: `up_` and twelve random hex digits.
fn update_id() -> String {
    let mut b = [0_u8; 6];
    // The OS's random source does not fail on the systems Remit runs on; were it to, the id still differs from the
    // last update's by its zeroes, and the server records an outcome once per id.
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut b).ok();
    format!("up_{}", hex::encode(b))
}

/// Writes the outcome to `<releases>/update.json`, by a rename over it, so a reader finds one whole outcome.
fn write_outcome(releases: &Path, out: &Outcome) -> Result<(), String> {
    let path = releases.join(OUTCOME);
    let fresh = releases.join(format!("{OUTCOME}.new"));
    let at = |e: std::io::Error| format!("{}: {e}", path.display());
    let json = serde_json::to_vec(out).map_err(|e| e.to_string())?;
    std::fs::write(&fresh, json).map_err(at)?;
    std::fs::rename(&fresh, &path).map_err(at)
}

async fn update(s: &Settings, out: &mut Outcome) -> Result<(), String> {
    let target = match &s.to {
        Some(t) => t.clone(),
        None => newest(s).await?,
    };
    out.to.clone_from(&target);
    let current = linked(&s.releases, "current");
    out.from.clone_from(&current);
    let running = probe::healthz(&s.addr).await.ok().map(|h| h.revision().to_owned());
    // Checked before anything changes, and the database copied before `current` moves: a supervisor that restarts on
    // its own from here on starts a release whose database has its copy.
    let binary = match plan(current.as_deref(), &target, running.as_deref()) {
        Plan::Check => {
            out.healthy = ready(s, &target).await.is_ok();
            return Ok(());
        }
        Plan::Install => Some(download(s, &target).await?),
        Plan::Restart => None,
    };
    let copy = backup(&s.data).await?;
    out.backup = copy.as_ref().map(|c| c.path.display().to_string());
    out.changed = true;
    if let Some(binary) = binary {
        install(s, &target, &binary, current.as_deref())?;
    }
    let came_up = match restart(s).await {
        Ok(()) => ready(s, &target).await,
        Err(why) => Err(why),
    };
    let Err(why) = came_up else {
        out.healthy = true;
        out.one_way = migrated(&s.data, copy.as_ref()).await?;
        return Ok(());
    };
    let previous = linked(&s.releases, "previous").filter(|p| *p != target);
    let Some(previous) = previous else {
        return Err(format!(
            "{target} is not healthy ({why}), and there is no previous release to go back to"
        ));
    };
    point(&s.releases, "current", &previous)?;
    point(&s.releases, "previous", &target)?;
    out.rolled_back = true;
    // Read once the release that failed is stopped, so it cannot migrate after the look.
    restart(s).await?;
    out.one_way = migrated(&s.data, copy.as_ref()).await?;
    if let (true, Some(copy)) = (out.one_way, &copy) {
        put_back(s, copy).await?;
        restart(s).await?;
    }
    out.healthy = ready(s, &previous).await.is_ok();
    let put = match (out.one_way, &out.backup) {
        (true, Some(copy)) => format!(", with the database put back from {copy}"),
        _ => String::new(),
    };
    Err(format!("{target} is not healthy ({why}): back on {previous}{put}"))
}

/// Holds the update lock until dropped, or says another update holds it.
fn lock(releases: &Path) -> Result<File, String> {
    std::fs::create_dir_all(releases).map_err(|e| format!("{}: {e}", releases.display()))?;
    let path = releases.join(LOCK);
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    match f.try_lock() {
        Ok(()) => Ok(f),
        Err(TryLockError::WouldBlock) => Err("another remit update is running".to_owned()),
        Err(TryLockError::Error(e)) => Err(format!("lock {}: {e}", path.display())),
    }
}

/// Runs gh with args; what it wrote on stdout.
async fn gh(s: &Settings, args: &[&str]) -> Result<String, String> {
    let out = procgroup::command(&s.gh, args)
        .output()
        .await
        .map_err(|e| format!("run {}: {e}", s.gh))?;
    if !out.status.success() {
        return Err(format!(
            "{} {}: {}",
            s.gh,
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// The newest release's tag.
async fn newest(s: &Settings) -> Result<String, String> {
    let tag = gh(
        s,
        &[
            "release", "view", "-R", &s.repo, "--json", "tagName", "--jq", ".tagName",
        ],
    )
    .await?;
    if tag.is_empty() {
        return Err(format!("{} has no release", s.repo));
    }
    Ok(tag)
}

/// The release the link `name` in releases points at.
fn linked(releases: &Path, name: &str) -> Option<String> {
    let to = std::fs::read_link(releases.join(name)).ok()?;
    Some(to.file_name()?.to_string_lossy().into_owned())
}

/// Points the link `name` in releases at `<releases>/<tag>`, by a rename over it.
fn point(releases: &Path, name: &str, tag: &str) -> Result<(), String> {
    let link = releases.join(name);
    let fresh = releases.join(format!("{name}.new"));
    let at = |e: std::io::Error| format!("{}: {e}", link.display());
    match std::fs::remove_file(&fresh) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(at(e)),
        _ => {}
    }
    std::os::unix::fs::symlink(releases.join(tag), &fresh).map_err(at)?;
    std::fs::rename(&fresh, &link).map_err(at)
}

/// Release tag's server, downloaded and checked against the release's SHA256SUMS.
async fn download(s: &Settings, tag: &str) -> Result<Vec<u8>, String> {
    let asset = asset(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
        format!(
            "no release is built for {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let download = s.releases.join(format!(".download-{tag}"));
    std::fs::remove_dir_all(&download).ok();
    std::fs::create_dir_all(&download).map_err(|e| format!("{}: {e}", download.display()))?;
    let fetched = fetch(s, tag, &asset, &download).await;
    std::fs::remove_dir_all(&download).ok();
    fetched
}

/// Installs binary as release tag's server and points `current` at it, with what `current` was as `previous`.
fn install(s: &Settings, tag: &str, binary: &[u8], current: Option<&str>) -> Result<(), String> {
    let dir = s.releases.join(tag);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // A running binary is replaced by a new file, never written over.
    let fresh = dir.join("remit.new");
    let at = |e: std::io::Error| format!("{}: {e}", fresh.display());
    std::fs::write(&fresh, binary).map_err(at)?;
    std::fs::set_permissions(&fresh, std::fs::Permissions::from_mode(0o755)).map_err(at)?;
    std::fs::rename(&fresh, dir.join("remit")).map_err(at)?;
    if let Some(prev) = current.filter(|c| *c != tag) {
        point(&s.releases, "previous", prev)?;
    }
    point(&s.releases, "current", tag)
}

/// The server's binary of release tag, downloaded into dir and checked against the release's SHA256SUMS.
async fn fetch(s: &Settings, tag: &str, asset: &str, dir: &Path) -> Result<Vec<u8>, String> {
    let into = dir.display().to_string();
    gh(
        s,
        &[
            "release",
            "download",
            tag,
            "-R",
            &s.repo,
            "-p",
            asset,
            "-p",
            "SHA256SUMS",
            "-D",
            &into,
            "--clobber",
        ],
    )
    .await?;
    let binary = std::fs::read(dir.join(asset)).map_err(|e| format!("{tag} {asset}: {e}"))?;
    let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).map_err(|e| format!("{tag} SHA256SUMS: {e}"))?;
    let want = listed_sum(&sums, asset).ok_or_else(|| format!("{tag}'s SHA256SUMS lists no {asset}"))?;
    let got = hex::encode(ring::digest::digest(&ring::digest::SHA256, &binary));
    if !got.eq_ignore_ascii_case(want) {
        return Err(format!(
            "{tag} {asset} does not match its SHA256SUMS: nothing was installed"
        ));
    }
    Ok(binary)
}

/// The copy of the database taken before the restart, and the schema version it holds.
struct Copy {
    path: PathBuf,
    version: i64,
}

/// Copies the database before the restart; None when there is none yet. The server on it does not migrate, so the
/// version read beside the copy is the copy's.
async fn backup(data: &Path) -> Result<Option<Copy>, String> {
    let db = data.join("remit.db");
    if !db.exists() {
        return Ok(None);
    }
    let now = model::Time(time::OffsetDateTime::now_utc());
    let copied = tokio::task::spawn_blocking(move || {
        let version = store::schema_version(&db)?;
        Ok::<_, store::Error>(Copy {
            path: store::snapshot_before_update(&db, now)?,
            version,
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("copy the database: {e}"))?;
    Ok(Some(copied))
}

/// Whether the database is past the schema version of the copy taken before the update: the release migrated it.
/// Without a copy there was no database for a release before to read, so none is past.
async fn migrated(data: &Path, copy: Option<&Copy>) -> Result<bool, String> {
    let Some(copy) = copy else {
        return Ok(false);
    };
    let db = data.join("remit.db");
    let now = tokio::task::spawn_blocking(move || store::schema_version(&db))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("read the database's schema version: {e}"))?;
    Ok(now > copy.version)
}

/// Puts the copy back as the database, once no server holds it: the supervisor may be starting the release before,
/// which takes the database and lets it go as it refuses a schema past its own, so a held database is asked again
/// until `-wait` runs out.
async fn put_back(s: &Settings, copy: &Copy) -> Result<(), String> {
    let deadline = tokio::time::Instant::now()
        .checked_add(s.wait)
        .unwrap_or_else(tokio::time::Instant::now);
    loop {
        let (db, from) = (s.data.join("remit.db"), copy.path.clone());
        let now = model::Time(time::OffsetDateTime::now_utc());
        let put = tokio::task::spawn_blocking(move || store::restore(&db, &from, now))
            .await
            .map_err(|e| e.to_string())?;
        match put {
            Ok(_) => return Ok(()),
            Err(store::Error::InUse(_)) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(LOOK_EVERY).await;
            }
            Err(e) => return Err(format!("put {} back as the database: {e}", copy.path.display())),
        }
    }
}

/// Restarts the server through its supervisor; what the command says goes to stderr, so stdout holds the outcome alone.
async fn restart(s: &Settings) -> Result<(), String> {
    let out = procgroup::command("sh", ["-c", s.restart.as_str()])
        .output()
        .await
        .map_err(|e| format!("run the restart command: {e}"))?;
    eprint!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("the restart command failed: {}", out.status))
    }
}

/// Waits up to `-wait` until the server is healthy on tag's commit and welcomes the owner's hello; why not, when it is
/// not by then.
async fn ready(s: &Settings, tag: &str) -> Result<(), String> {
    let deadline = tokio::time::Instant::now()
        .checked_add(s.wait)
        .unwrap_or_else(tokio::time::Instant::now);
    loop {
        let why = match probe::healthz(&s.addr).await {
            Ok(h) if h.ok() && runs(tag, h.revision()) => match probe::link_json(&s.addr).await {
                // Read on each look: a server's first start makes the key, so before it there is none to read.
                Ok(link) => match probe::hello(&link, &probe::owner_key(&s.data).unwrap_or_default()).await {
                    Ok(()) => return Ok(()),
                    Err(down) => format!("the link: {down}"),
                },
                Err(why) => why,
            },
            Ok(h) if h.ok() => format!("it runs {}, not {tag}", h.revision()),
            Ok(h) => format!("/healthz {}: {}", h.status, h.reason()),
            Err(why) => why,
        };
        if tokio::time::Instant::now() >= deadline {
            return Err(why);
        }
        tokio::time::sleep(LOOK_EVERY).await;
    }
}
