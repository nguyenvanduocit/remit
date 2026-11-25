//! `remit emit` (ADR-0024) writes one event of an event signal from a script or a shell, as any outside system does: it
//! posts the body to the signal's door on a Remit server with the signal's key. The key is read from a file only its
//! owner can read, or from `REMIT_EVENT_KEY`, never from the command line, whose arguments every user of the machine
//! can list.

use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::time::Duration;

use crate::dash_flags::{self, Flag, Kind};

/// The variable `remit emit` reads the signal's key from when it is given no key file.
const KEY_ENV: &str = "REMIT_EVENT_KEY";
/// How long `remit emit` waits for the server.
const TIMEOUT: Duration = Duration::from_secs(30);
/// How much of the server's answer is read.
const ANSWER_MAX: usize = 64 << 10;

fn defs() -> Vec<Flag> {
    let f = |name, default: &str, usage| Flag {
        name,
        kind: Kind::Str,
        default: default.to_owned(),
        usage,
    };
    vec![
        f(
            "url",
            "http://127.0.0.1:4320",
            "the Remit server, as its owner reaches it: the door is <url>/hooks/<name>",
        ),
        f(
            "name",
            "",
            "the event signal the event is of (event:<name>), one the owner made",
        ),
        f(
            "key-file",
            "",
            "a file of mode 0600 holding the signal's key, as its page shows it; empty reads REMIT_EVENT_KEY",
        ),
        f(
            "sender-id",
            "",
            "your own id for the event: posting it again with the same id is the same event; empty: the same body is",
        ),
    ]
}

/// `remit emit` with args, the words after emit: it posts the event and returns the server's answer. The body is the file
/// args name, or stdin when they name none or name `-`.
pub async fn run(args: &[String]) -> Result<String, String> {
    let defs = defs();
    let p = dash_flags::parse(&defs, args).map_err(|e| match e {
        dash_flags::Error::Help => format!(
            "usage: remit emit -name <event signal> [-url <server>] [-key-file <path>] [-sender-id <id>] [file | -]\n{}",
            dash_flags::usage(&defs)
        ),
        dash_flags::Error::Bad(why) => why,
    })?;
    let name = p.str("name");
    if name.is_empty() {
        return Err("-name names the event signal: list_event_signals shows them".to_owned());
    }
    if p.rest.len() > 1 {
        return Err(format!("one body at a time: {} files named", p.rest.len()));
    }
    let key = key(&p.str("key-file"), std::env::var(KEY_ENV).unwrap_or_default())?;
    let body = body(p.rest.first().map(String::as_str).unwrap_or_default())?;
    let address = format!(
        "{}/hooks/{}",
        p.str("url").trim_end_matches('/'),
        urlescape::path_segment(&name)
    );
    post(&address, &key, &p.str("sender-id"), body).await
}

/// The signal's key: the content of key_file, which only its owner may read, or else of the variable.
fn key(key_file: &str, from_env: String) -> Result<String, String> {
    if key_file.is_empty() {
        let key = from_env.trim().to_owned();
        if key.is_empty() {
            return Err(format!(
                "no key: give -key-file, a file of mode 0600 that holds the signal's key, or set {KEY_ENV}"
            ));
        }
        return Ok(key);
    }
    let mode = std::fs::metadata(key_file)
        .map_err(|e| format!("{key_file}: {e}"))?
        .permissions()
        .mode()
        & 0o777;
    if mode & 0o077 != 0 {
        return Err(format!(
            "{key_file} can be read by others (mode {mode:o}): a key is kept where only its owner reads it; chmod 600 it"
        ));
    }
    let key = std::fs::read_to_string(key_file)
        .map_err(|e| format!("{key_file}: {e}"))?
        .trim()
        .to_owned();
    if key.is_empty() {
        return Err(format!("{key_file} holds no key"));
    }
    Ok(key)
}

/// The event's body: the file at path, or stdin for none and for `-`.
fn body(path: &str) -> Result<Vec<u8>, String> {
    if path.is_empty() || path == "-" {
        let mut b = Vec::new();
        std::io::stdin()
            .read_to_end(&mut b)
            .map_err(|e| format!("stdin: {e}"))?;
        return Ok(b);
    }
    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
}

/// Posts body to the door at address with key, saying it comes from the CLI, and returns what the server answered when it
/// took the event (202).
async fn post(address: &str, key: &str, sender_id: &str, body: Vec<u8>) -> Result<String, String> {
    // An https server is checked against the system's roots.
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let json = serde_json::from_slice::<serde_json::Value>(&body).is_ok();
    let mut req = client
        .post(address)
        .bearer_auth(key)
        .header(door::VIA_HEADER, "cli")
        .header(
            "Content-Type",
            if json {
                "application/json"
            } else {
                "text/plain; charset=utf-8"
            },
        )
        .body(body);
    if !sender_id.is_empty() {
        req = req.header("Idempotency-Key", sender_id);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    let answer = res.bytes().await.map_err(|e| e.to_string())?;
    let answer = String::from_utf8_lossy(answer.get(..ANSWER_MAX).unwrap_or(&answer))
        .trim()
        .to_owned();
    if status != reqwest::StatusCode::ACCEPTED {
        return Err(format!("the server answered {status}: {answer}"));
    }
    Ok(answer)
}
