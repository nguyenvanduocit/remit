//! `remit-client`, the native client of a Remit server: `call <method> [<args json>] [--request <id>]`, `snapshot` and
//! `watch` speak the owner's contract (`remit_client::app`), presenting the owner key read from `--key-file`, never from
//! an argument. An answer, the snapshot or each push is JSON on stdout; a failure is `{"code","message"}` on stderr and
//! exit status 1.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use link::{Failure, Request};
use remit_client::app::{self, Owner};
use serde_json::Value;

/// The server a plain install listens at (`remit -addr`'s default).
const DEFAULT_SERVER: &str = "http://127.0.0.1:4320";

const USAGE: &str = "usage:
  remit-client call <method> [<args json>] [--request <id>] [--server <address>] [--key-file <path>]
  remit-client snapshot [--server <address>] [--key-file <path>]
  remit-client watch [--server <address>] [--key-file <path>]
--server defaults to http://127.0.0.1:4320, --key-file to ~/.remit/owner.key";

/// What the command line asks for.
#[derive(Debug, PartialEq)]
enum Command {
    Call {
        on: Session,
        method: String,
        args: Value,
        request: Option<Request>,
    },
    Snapshot {
        on: Session,
    },
    Watch {
        on: Session,
    },
}

/// Where an owner's command goes and the file its key is read from.
#[derive(Debug, PartialEq)]
struct Session {
    server: String,
    key_file: PathBuf,
}

/// The command `args` (without the program's name) ask for, home being the user's home. Pure.
fn parse(args: &[String], home: &Path) -> Result<Command, String> {
    let (command, rest) = args.split_first().ok_or("no command")?;
    let mut words = Vec::new();
    let mut server = None;
    let mut key_file = None;
    let mut request = None;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or(format!("{name} takes a value"));
        match a.as_str() {
            "--server" => server = Some(value("--server")?),
            "--key-file" => key_file = Some(PathBuf::from(value("--key-file")?)),
            "--request" => {
                request = Some(Request {
                    id: value("--request")?,
                })
            }
            flag if flag.starts_with("--") => return Err(format!("no flag {flag}")),
            word => words.push(word.to_owned()),
        }
    }
    let on = || Session {
        server: server.clone().unwrap_or_else(|| DEFAULT_SERVER.to_owned()),
        key_file: key_file
            .clone()
            .unwrap_or_else(|| home.join(".remit").join("owner.key")),
    };
    let none_but = |allowed: bool, what: &str| {
        if allowed {
            Ok(())
        } else {
            Err(format!("{command} takes no {what}"))
        }
    };
    match command.as_str() {
        "call" => {
            let (method, args) = match words.as_slice() {
                [method] => (method.clone(), Value::Null),
                [method, args] => (
                    method.clone(),
                    serde_json::from_str(args).map_err(|e| format!("the args are not JSON: {e}"))?,
                ),
                _ => return Err("call takes a method and, after it, its args as one JSON object".to_owned()),
            };
            Ok(Command::Call {
                on: on(),
                method,
                args,
                request,
            })
        }
        "snapshot" | "watch" => {
            none_but(
                words.is_empty() && request.is_none(),
                "argument but --server and --key-file",
            )?;
            Ok(if command == "snapshot" {
                Command::Snapshot { on: on() }
            } else {
                Command::Watch { on: on() }
            })
        }
        other => Err(format!("no command {other}")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let command = match parse(&args, &home) {
        Ok(c) => c,
        Err(why) => {
            eprintln!("remit-client: {why}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("remit-client: start the runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(run(command))
}

async fn run(command: Command) -> ExitCode {
    match command {
        Command::Call {
            on,
            method,
            args,
            request,
        } => match owner(&on) {
            Ok(o) => answered(app::call(&o, &method, args, request).await),
            Err(f) => failed(&f),
        },
        Command::Snapshot { on } => match owner(&on) {
            Ok(o) => answered(app::snapshot(&o).await),
            Err(f) => failed(&f),
        },
        Command::Watch { on } => match owner(&on) {
            Ok(o) => failed(&app::watch(&o, tokio::io::stdout()).await),
            Err(f) => failed(&f),
        },
    }
}

/// The owner's session, its key read from its file.
fn owner(on: &Session) -> Result<Owner, Failure> {
    let key = std::fs::read_to_string(&on.key_file).map_err(|e| {
        Failure::new(
            link::ErrorCode::InvalidArgument,
            format!("the owner key {}: {e}", on.key_file.display()),
        )
    })?;
    Ok(Owner {
        server: on.server.clone(),
        key: key.trim().to_owned(),
    })
}

fn answered(answer: Result<Value, Failure>) -> ExitCode {
    match answer {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Err(f) => failed(&f),
    }
}

fn failed(f: &Failure) -> ExitCode {
    eprintln!("{}", serde_json::json!({ "code": f.code, "message": f.message }));
    ExitCode::FAILURE
}
