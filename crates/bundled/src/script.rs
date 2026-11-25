//! The script source (ADR-0044): what no source gives is read by code the owner applies. A sensor of this source holds the code
//! in its query; each poll runs it on the host as the server's user and the lines it prints are the events. The code is the
//! reader and nothing else: the cursor, the dedupe by id, "the first poll answers nothing older" and a failure as one error item
//! a day are the same for every reader and live here, once.

use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::LazyLock;
use std::time::Duration;

use jiff::Timestamp;
use plugin_sdk::{Definition, Error, Poll, PollAnswer, PollItem, Var};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use source::{Clock, limit, rfc3339};
use tokio::io::{AsyncRead, AsyncReadExt};

/// The plugin's id, which the server and the signals a script fills name it by.
pub(crate) const ID: &str = "script";

/// How many answered ids the cursor keeps: more than a reader prints in a day, few enough for a cursor.
const SEEN_MAX: usize = 1000;
/// The most items a poll answers when the host does not say.
const POLL_ITEMS: usize = 100;
/// The time a script has. The gate ends a poll after its call timeout (30 s) by dropping it, which ends the script's process
/// group too; this budget ends the script first, so the poll still answers what it printed and says why it stopped.
const BUDGET: Duration = Duration::from_secs(25);
/// How long the output is still read once the script exited, for what a process it left behind still holds.
const PIPE_WAIT: Duration = Duration::from_secs(1);
/// The bytes of output kept of each stream: a reader that prints more is cut, and the cut is an error.
const OUTPUT_MAX: usize = 8 << 20;

/// What a script can be written in, each run with the flag that takes its code.
const RUNTIMES: [&str; 4] = ["sh", "node", "python3", "ego-browser"];

/// What an error's id leaves out, so the same failure with other numbers is one event.
#[expect(clippy::expect_used, reason = "the pattern is a constant a test compiles")]
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new("[0-9]+").expect("the digits shape compiles"));

/// The sensor's query: the runtime and the code the owner reads when he applies it.
#[derive(Debug, PartialEq)]
struct Query {
    runtime: String,
    code: String,
}

impl Query {
    /// A query that is the code itself under a first line naming its runtime, `#!node` or `#!ego-browser`, so the owner reads
    /// the script as written, with nothing escaped.
    fn parse(q: &str) -> Result<Query, String> {
        let (first, code) = q.split_once('\n').unwrap_or((q, ""));
        let Some(runtime) = first.trim().strip_prefix("#!") else {
            return Err(format!(
                "the query begins with its runtime on a line of its own, #!{}, and the code follows",
                RUNTIMES.join(" | #!")
            ));
        };
        let runtime = runtime.trim();
        if !RUNTIMES.contains(&runtime) {
            return Err(format!(
                "runtime {:?} is not one of {}",
                limit(runtime, 40),
                RUNTIMES.join(", ")
            ));
        }
        if code.trim().is_empty() {
            return Err("the query has no code to run".to_owned());
        }
        Ok(Query {
            runtime: runtime.to_owned(),
            code: code.to_owned(),
        })
    }

    /// How the runtime runs the code: the program and its arguments. `ego` is the connection's EGO_BROWSER, empty for the
    /// default place.
    fn argv(&self, home: &Path, ego: &str) -> (PathBuf, Vec<String>) {
        let code = self.code.clone();
        match self.runtime.as_str() {
            "node" => ("node".into(), vec!["-e".into(), code]),
            "python3" => ("python3".into(), vec!["-c".into(), code]),
            "ego-browser" => {
                let program = if ego.is_empty() {
                    home.join(".local/bin/ego-browser")
                } else {
                    ego.into()
                };
                (program, vec!["nodejs".into(), "-e".into(), code])
            }
            _ => ("/bin/sh".into(), vec!["-c".into(), code]),
        }
    }
}

/// Where a script finds programs: it has none of the server's environment (ADR-0034), so the places a person's tools live are
/// named here.
fn script_path(home: &Path) -> String {
    [
        home.join(".local/bin").display().to_string(),
        "/opt/homebrew/bin".to_owned(),
        "/usr/local/bin".to_owned(),
        "/usr/bin".to_owned(),
        "/bin".to_owned(),
    ]
    .join(":")
}

/// Where `program` is on the script's PATH. A bare name is looked up on the script's PATH, not the server's, done here.
fn on_script_path(home: &Path, program: &Path) -> Result<PathBuf, String> {
    if program.is_absolute() {
        return Ok(program.to_owned());
    }
    let path = script_path(home);
    path.split(':')
        .map(|dir| Path::new(dir).join(program))
        .find(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
        .ok_or_else(|| {
            format!(
                "{} is not installed on this machine: the script's PATH is {path}",
                program.display()
            )
        })
}

/// Where the polls of a query are: when the first one was (Unix seconds) and the ids already answered.
#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    at: i64,
    seen: Option<Vec<String>>,
}

/// What a script prints that Remit reads: an item has an id, an error has an error. Anything else is the script's own log. A
/// member that is there, null included, is `Some`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Line {
    #[serde(deserialize_with = "present")]
    id: Option<Value>,
    version: String,
    #[serde(deserialize_with = "present")]
    body: Option<Value>,
    error: String,
    target: String,
}

fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// A failure a script reported, or its run's own.
#[derive(Debug, PartialEq)]
struct Failure {
    target: String,
    message: String,
}

/// What one run printed: its items in order and its error lines.
#[derive(Debug, Default)]
struct Read {
    items: Vec<PollItem>,
    errors: Vec<Failure>,
}

fn read_lines(out: &str) -> Read {
    let mut r = Read::default();
    for line in out.lines().map(str::trim).filter(|l| l.starts_with('{')) {
        let Ok(l) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        if !l.error.is_empty() {
            r.errors.push(Failure {
                target: l.target,
                message: l.error,
            });
            continue;
        }
        match l.id {
            None => {}
            Some(Value::String(id)) if !id.is_empty() => r.items.push(PollItem {
                id,
                version: l.version,
                body: l.body.unwrap_or_else(|| json!({})).into(),
            }),
            Some(other) => r.errors.push(Failure {
                target: String::new(),
                message: format!(
                    "an item line has id {}: an id is a non-empty string ({})",
                    limit(&other.to_string(), 40),
                    limit(line, 120)
                ),
            }),
        }
    }
    r
}

/// A stream's bytes, at most `OUTPUT_MAX` of them, and whether more were dropped.
#[derive(Debug, Default)]
struct Capped {
    bytes: Vec<u8>,
    cut: bool,
}

impl Capped {
    /// Reads `pipe` to its end, keeping what fits; what does not is still read, so the script never blocks on a full pipe.
    async fn fill(&mut self, pipe: Option<impl AsyncRead + Unpin>) {
        let Some(mut pipe) = pipe else { return };
        let mut chunk = [0u8; 64 << 10];
        while let Ok(n) = pipe.read(&mut chunk).await {
            let Some(got) = chunk.get(..n).filter(|g| !g.is_empty()) else {
                return;
            };
            let room = OUTPUT_MAX.saturating_sub(self.bytes.len());
            self.cut = self.cut || got.len() > room;
            self.bytes
                .extend_from_slice(got.get(..room.min(got.len())).unwrap_or_default());
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

/// How a process that failed ended, in words: `exit status 1`, `signal: 9`.
fn ended(status: ExitStatus) -> String {
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("exit status {code}"),
        (None, Some(signal)) => format!("signal: {signal}"),
        (None, None) => status.to_string(),
    }
}

/// Runs the code of `q` on the host and answers what it printed on either stream (ego-browser prints a script's console on
/// stderr), with the error of a run that failed or ran out of time. Its process group ends with the run, or with the future
/// when the caller drops it.
async fn run(home: &Path, ego: &str, q: &Query) -> (String, Option<String>) {
    let (program, args) = q.argv(home, ego);
    let program = match on_script_path(home, &program) {
        Ok(p) => p,
        Err(e) => return (String::new(), Some(e)),
    };
    let mut cmd = procgroup::command(program, args);
    cmd.env_clear()
        .env("HOME", home)
        .env("PATH", script_path(home))
        .env("LANG", "en_US.UTF-8")
        .env("SCRIPT_BUDGET_MS", BUDGET.as_millis().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (String::new(), Some(e.to_string())),
    };
    let (out_pipe, err_pipe) = (child.stdout.take(), child.stderr.take());
    let (mut stdout, mut stderr) = (Capped::default(), Capped::default());
    let ran = tokio::time::timeout(BUDGET, async {
        let reads = async { tokio::join!(stdout.fill(out_pipe), stderr.fill(err_pipe)) };
        tokio::pin!(reads);
        tokio::select! {
            status = child.wait() => {
                // What the script left running may still hold the pipes: what it printed by now is all that is read.
                tokio::time::timeout(PIPE_WAIT, &mut reads).await.ok();
                status
            }
            _ = &mut reads => child.wait().await,
        }
    })
    .await;
    // A script still running past its budget is ended here, with everything it started.
    drop(child);
    let output = format!("{}\n{}", stdout.text(), stderr.text());
    let err = match ran {
        Err(_) => Some(format!("the script ran out of its {}s", BUDGET.as_secs())),
        Ok(Err(e)) => Some(e.to_string()),
        Ok(Ok(status)) if !status.success() => Some(format!(
            "{}: {}",
            ended(status),
            limit(&last_line(&[&stderr.text(), &stdout.text()]), 300)
        )),
        Ok(Ok(_)) if stdout.cut || stderr.cut => Some(format!(
            "the script printed more than {OUTPUT_MAX} bytes, which is all that is read"
        )),
        Ok(Ok(_)) => None,
    };
    (output, err)
}

/// The last non-empty line of the first of `streams` that has one: what a failed script said last.
fn last_line(streams: &[&str]) -> String {
    streams
        .iter()
        .find_map(|s| s.trim().lines().last().map(str::trim).filter(|l| !l.is_empty()))
        .unwrap_or_default()
        .to_owned()
}

/// The event of a failure: its id holds the target, the UTC day and the message without its numbers, so one failure alerts
/// once a day and shows again the day after (Remit drops a repeated event id for good).
fn error_item(e: &Failure, now: Timestamp) -> PollItem {
    let sum = hex::encode(Sha256::digest(DIGITS.replace_all(&e.message, "").as_bytes()));
    PollItem {
        id: format!(
            "error:{}:{}:{}",
            e.target,
            now.strftime("%Y-%m-%d"),
            sum.get(..24).unwrap_or_default()
        ),
        version: String::new(),
        body: json!({"kind": "error", "target": e.target, "error": limit(&e.message, 400), "seen_at": rfc3339(now)})
            .into(),
    }
}

/// The plugin's poll. The first poll, with no cursor, runs the script and answers no item: what it printed then is history. A
/// later poll answers the items whose id is not in the cursor, in the order printed, at most `limit`; an item left over for the
/// limit is not marked answered, so it comes on the next poll.
async fn poll(
    now: Timestamp,
    home: &Path,
    ego: &str,
    query: &str,
    cursor: &str,
    limit_items: i64,
) -> Result<PollAnswer, Error> {
    let q = Query::parse(query)?;
    let first = cursor.is_empty();
    let (at, mut seen) = if first {
        (now.as_second(), Vec::new())
    } else {
        serde_json::from_str::<Cursor>(cursor)
            .ok()
            .and_then(|c| Some((c.at, c.seen?)))
            .ok_or_else(|| format!("poll cursor {:?} is not one this plugin gave", limit(cursor, 40)))?
    };
    let (out, run_err) = run(home, ego, &q).await;
    let mut read = read_lines(&out);
    if let Some(e) = run_err {
        if read.items.is_empty() && read.errors.is_empty() {
            return Err(e.into());
        }
        read.errors.push(Failure {
            target: String::new(),
            message: e,
        });
    }
    let most = usize::try_from(limit_items)
        .ok()
        .filter(|n| *n > 0)
        .unwrap_or(POLL_ITEMS);
    let mut items = Vec::new();
    for it in read.items {
        if seen.contains(&it.id) {
            continue;
        }
        if first {
            seen.push(it.id);
        } else if items.len() < most {
            seen.push(it.id.clone());
            items.push(it);
        }
    }
    if !first {
        items.extend(read.errors.iter().map(|e| error_item(e, now)));
    }
    let keep = seen.len().saturating_sub(SEEN_MAX);
    let cursor = serde_json::to_string(&Cursor {
        at,
        seen: Some(seen.split_off(keep)),
    })?;
    Ok(PollAnswer { items, cursor })
}

/// The bundled plugin that runs a sensor's code (ADR-0044). It declares no tool and ships no signal: the owner, through the
/// Assistant, makes the signals and their sensors, and reads the code when he applies one.
pub(crate) fn plugin(clock: Clock) -> Definition {
    Definition {
        label: "Script".into(),
        capability: "data".into(),
        vars: vec![Var {
            name: "EGO_BROWSER".into(),
            what: "the absolute path of ego-browser, when it is not ~/.local/bin/ego-browser".into(),
            optional: true,
            ..Default::default()
        }],
        poll: Some(Poll::new(move |call, q| {
            let (now, ego) = (clock.now(), call.var("EGO_BROWSER").to_owned());
            async move {
                let home = std::env::home_dir().ok_or("the home directory of the server's user is not known")?;
                poll(now, &home, &ego, &q.query, &q.cursor, q.limit).await
            }
        })),
        ..Default::default()
    }
}
