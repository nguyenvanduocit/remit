//! Programs, the form T2 of ADR-0018: an agent runs a command-line program with a connection's credential and never holds
//! it. The credential is in the environment of the program, which runs in a jail (the `Runner`, implemented by the
//! orchestrator over the jail crate); the agent writes the arguments, and the source fixes everything else: the binary, the
//! environment, the subcommands that may run and the addresses the program may reach. That the program only reads is the
//! credential's own rights (the owner supplies a read-only credential): this form enforces it no further than that, and says
//! so wherever it is shown.
//!
//! This file is the form's whole contract. `ProgramSpec::check` is pure; the run is the shell around it (`program_exec`); a
//! source is reached this way by answering `Provider::program`, as CloudWatch does with `aws`.

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use grant::Effect;
use regex::Regex;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::connection::{Detail, Resolved, ServedTool, Set, asked_connection, key};
use crate::error::{BoxError, BoxFuture, Error, Result, err, fail};
use crate::gate::{Caller, Form};
use crate::redact::{redact_text, scrub};
use crate::script::InjectJob;
use crate::source::{FROM_OUTSIDE, Kind, Provider, schema_of, with_connection};
use crate::text::{OUTPUT_MAX, limit, tail_from};
use crate::when::duration_text;

/// The one tool through which an agent runs a program: it takes the source, so the grammar is one for every source.
pub const RUN_TOOL: &str = "run";

/// Under the call's deadline, so the jail ends the program and says so before the gate's deadline does.
pub(crate) const PROGRAM_TIMEOUT: Duration = Duration::from_secs(25);
pub(crate) const MAX_ARGS: usize = 64;
pub(crate) const MAX_ARG_BYTES: usize = 4096;

/// The PATH a source's program runs with, a command line or a script's interpreter: a short fixed one, the system's (and on
/// macOS Homebrew's, where it gets its command lines and interpreters). A host offers a program only if it is on it.
#[cfg(target_os = "macos")]
pub const PROGRAM_PATH: &str = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin";
#[cfg(not(target_os = "macos"))]
pub const PROGRAM_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Checks, before every run, that the program on disk is the one the owner applied.
pub type Verify = Arc<dyn Fn() -> std::result::Result<(), String> + Send + Sync>;

/// The whole environment of a program, `NAME=value` entries, for the scratch directory it is given.
pub type ProgramEnv = Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>;

/// Everything about a program except the arguments, which are the agent's. A source builds it from its own connection; an
/// agent never sees `env`, `secrets` or `egress`.
#[derive(Clone)]
pub struct ProgramSpec {
    /// The program, a name the jail looks up along the PATH of `env`.
    pub binary: String,
    /// The leading words of every command that may run: the arguments must start with one of them.
    pub allow: Vec<Vec<String>>,
    /// The flags no argument may be: the ones that move where the program connects, whose identity it takes or what it
    /// reads. A program's parser may take an abbreviation of a flag, so an argument that is a prefix of a denied flag is
    /// refused too.
    pub deny: Vec<String>,
    /// The prefixes no argument's value may start with: the ones that make the program read a file or fetch an address of
    /// the agent's choosing. The value of `--flag=value` is what follows the first `=`.
    pub deny_values: Vec<String>,
    /// The whole environment of the program, with the credential, for a scratch directory it may write.
    pub env: ProgramEnv,
    /// The values of the credential: each is removed from everything the program wrote, before the answer is made.
    pub secrets: Vec<String>,
    /// The host:port addresses the program may reach, and nothing else.
    pub egress: Vec<String>,
    /// The paths of the host the program may read besides the system's, its own files.
    pub read_only: Vec<String>,
    /// When set, checked before every run.
    pub verify: Option<Verify>,
}

impl ProgramSpec {
    /// Whether `args` may run as this program: the arguments of an agent are the one hostile input of this form. It is pure
    /// and needs no shell: the arguments are an argv and none is ever interpreted.
    pub fn check(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return fail!("args is empty: start with one of {}", self.commands());
        }
        if args.len() > MAX_ARGS {
            return fail!("{} arguments: at most {MAX_ARGS}", args.len());
        }
        for a in args {
            if a.len() > MAX_ARG_BYTES {
                return fail!("an argument of {} bytes: at most {MAX_ARG_BYTES}", a.len());
            }
            if a.contains('\0') {
                return fail!("an argument holds a NUL byte");
            }
        }
        if !self.allowed(args) {
            let head = args.iter().take(2).cloned().collect::<Vec<_>>().join(" ");
            return fail!(
                "{:?} does not run here: the commands that do are {}",
                limit(&head, 80),
                self.commands()
            );
        }
        args.iter().try_for_each(|a| self.refuses(a))
    }

    fn allowed(&self, args: &[String]) -> bool {
        self.allow
            .iter()
            .any(|words| args.len() >= words.len() && args.iter().zip(words).all(|(a, w)| a == w))
    }

    /// The allowed commands, for an error and a description.
    pub(crate) fn commands(&self) -> String {
        self.allow
            .iter()
            .map(|words| format!("{} {}", self.binary, words.join(" ")))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// The reason argument `a` may not be passed, Ok when it may.
    fn refuses(&self, a: &str) -> Result<()> {
        let mut value = a;
        if a.starts_with("--") {
            let (name, rest) = match a.split_once('=') {
                Some((n, r)) => (n, Some(r)),
                None => (a, None),
            };
            if self.deny.iter().any(|d| d.starts_with(name)) {
                return fail!(
                    "{} is not passed to {}: it changes where the program connects, who it acts as or what it reads, which the connection fixes",
                    limit(name, 40),
                    self.binary
                );
            }
            if let Some(rest) = rest {
                value = rest;
            }
        }
        let lower = value.to_lowercase();
        if let Some(prefix) = self.deny_values.iter().find(|p| lower.starts_with(p.as_str())) {
            return fail!(
                "a value starting {prefix} is not passed to {}: it would read a file or fetch an address of your choosing",
                self.binary
            );
        }
        Ok(())
    }
}

/// One run of a program, as the Runner receives it: all of it is the source's but the arguments of `argv`.
#[derive(Clone)]
pub struct Job {
    /// The binary and the checked arguments.
    pub argv: Vec<String>,
    /// The whole environment, with the credential.
    pub env: Vec<String>,
    /// A scratch directory of this run alone, removed when it ends.
    pub dir: PathBuf,
    /// The host:port addresses the program may reach.
    pub egress: Vec<String>,
    /// The paths of the host the program may read besides the system's.
    pub read_only: Vec<String>,
    /// When set, the form T1: the program reaches only that connection's address, through a proxy that adds the credential;
    /// `egress` is then empty and `env` holds no credential.
    pub inject: Option<InjectJob>,
    /// Ends the program.
    pub timeout: Duration,
    /// How many bytes of each of stdout and stderr are kept.
    pub max_output: usize,
}

/// What a program left behind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
}

/// Runs a Job in a jail. The orchestrator implements it over the jail crate; this crate depends on neither (ADR-0015). An
/// error is for a run that could not be made; a program that fails is an Output.
pub trait Runner: Send + Sync {
    fn run<'a>(&'a self, j: &'a Job) -> BoxFuture<'a, std::result::Result<Output, BoxError>>;

    /// Starts `j` in a jail and leaves it running, its stdin and stdout piped to the caller: an MCP server over stdio
    /// (ADR-0077). `j.timeout` bounds its processor time; `j.max_output` is not used. A host that starts none says so.
    fn spawn<'a>(&'a self, j: &'a Job) -> BoxFuture<'a, std::result::Result<Spawned, BoxError>> {
        let _ = j;
        Box::pin(async { Err("this host starts no long-lived program".into()) })
    }
}

/// A program the jail keeps running: what it reads and writes, and what holds it. Dropping `hold` ends it and everything it
/// started, and closes its way out.
pub struct Spawned {
    pub stdin: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
    pub stdout: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
    pub hold: Box<dyn Send>,
}

/// What an agent gives the run tool; the kernel adds `connection`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct RunInput {
    /// the source whose program to run, as list_sources names it
    pub source: String,
    /// the arguments of the program, one element each, without the program's name; no shell reads them
    pub args: Vec<String>,
}

impl Set {
    /// The run tool on one connection: the act of its program (form env), a read as far as the tool's code goes.
    pub(crate) fn program_resolved(&self, p: &Arc<dyn Provider>, k: &Kind, connection: &str) -> Result<Resolved> {
        let (Some(spec), Some(runner)) = (p.program(), self.inner.runner.clone()) else {
            return fail!("{} runs no program on this server", p.name());
        };
        let mut res = Resolved::new(key(&k.id, connection), connection, Effect::Read, Form::Env);
        res.detail = Some(program_detail(spec.clone()));
        res.exec = Some(Arc::new(move |args: Value| {
            let spec = spec.clone();
            let runner = Arc::clone(&runner);
            Box::pin(async move { program_exec(&spec, runner.as_ref(), args).await })
        }));
        Ok(res)
    }

    /// The run tool, once, for the programmed sources among `held`. Nothing is served without a Runner or a source to run.
    pub(crate) fn run_tool(&self, held: &[Kind]) -> Option<ServedTool> {
        let ks: Vec<Kind> = held
            .iter()
            .filter(|k| self.template(k).program().is_some())
            .cloned()
            .collect();
        if self.inner.runner.is_none() || ks.is_empty() {
            return None;
        }
        let mut schema = schema_of::<RunInput>();
        if let Some(prop) = schema.pointer_mut("/properties/source").and_then(Value::as_object_mut) {
            prop.insert(
                "enum".to_owned(),
                ks.iter().map(|k| Value::String(k.id.clone())).collect(),
            );
        }
        let set = self.clone();
        let description = run_description(self, &ks);
        Some(ServedTool::new(
            RUN_TOOL.to_owned(),
            description,
            with_connection(schema),
            false, // a program's or script's effect is its form's, declared per call: it may write
            Arc::new(move |caller: Caller, args: Value| {
                let set = set.clone();
                let ks = ks.clone();
                Box::pin(async move {
                    let source = args
                        .get("source")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    if !ks.iter().any(|k| k.id == source) {
                        let ids: Vec<&str> = ks.iter().map(|k| k.id.as_str()).collect();
                        return fail!(
                            "run: no source {:?} runs programs here: the ones that do are {}",
                            limit(&source, 40),
                            ids.join(", ")
                        );
                    }
                    // The source as it is now: an extension applied again, or removed, after this session began.
                    let Some(cur) = set.kind(&source) else {
                        return fail!("run: no source {:?} runs programs here any more", limit(&source, 40));
                    };
                    let connection = asked_connection(&args);
                    set.gate(&cur).run(&caller, RUN_TOOL, &connection, args).await
                })
            }),
        ))
    }
}

/// Carries out run for the connection whose program is `spec`: it checks the arguments, makes the scratch directory, has
/// the Runner run the job and shapes what comes back (`answer_of`).
async fn program_exec(spec: &ProgramSpec, runner: &dyn Runner, args: Value) -> Result<String> {
    let typed: RunInput = serde_json::from_value(args).map_err(|e| err!("run: the input does not fit: {e}"))?;
    spec.check(&typed.args).map_err(|e| e.context("run"))?;
    if let Some(verify) = &spec.verify {
        verify().map_err(|e| err!("run: {e}"))?;
    }
    let dir = tempfile::Builder::new()
        .prefix("remit-run-")
        .tempdir()
        .map_err(|e| err!("run: scratch directory: {e}"))?;
    let mut argv = vec![spec.binary.clone()];
    argv.extend(typed.args);
    let job = Job {
        argv: argv.clone(),
        env: (spec.env)(&dir.path().to_string_lossy()),
        dir: dir.path().to_path_buf(),
        egress: spec.egress.clone(),
        read_only: spec.read_only.clone(),
        inject: None,
        timeout: PROGRAM_TIMEOUT,
        max_output: OUTPUT_MAX,
    };
    let out = runner
        .run(&job)
        .await
        .map_err(|e| Error::msg(format!("run: {}", scrub(&e.to_string(), &spec.secrets))))?;
    answer_of(spec, &argv, &out, PROGRAM_TIMEOUT)
}

/// Which program with which arguments a run was, as one line, for the record of the call: the argv, with the connection's
/// values removed from it.
fn program_detail(spec: ProgramSpec) -> Detail {
    Arc::new(move |input: &Value| {
        let Ok(typed) = serde_json::from_value::<RunInput>(input.clone()) else {
            return String::new();
        };
        let mut argv = vec![spec.binary.clone()];
        argv.extend(typed.args);
        redact_text(&scrub(&shell_line(&argv), &spec.secrets))
    })
}

/// The line that heads a program's answer: the honest effect of this form (ADR-0018, invariant 4).
pub(crate) const PROGRAM_FORM: &str = "form: program (env in a jail; read-only is the credential's rights)";

/// What an agent reads of a finished program: its output under the form and the command, or an error that says how it
/// failed. The program's secrets are removed first. Pure.
pub(crate) fn answer_of(spec: &ProgramSpec, argv: &[String], out: &Output, timeout: Duration) -> Result<String> {
    finished(
        RUN_TOOL,
        &spec.binary,
        &format!("{PROGRAM_FORM}\n$ {}\n", shell_line(argv)),
        &spec.secrets,
        out,
        timeout,
    )
}

/// What a program or a script left behind, shaped for the tool that ran it: `head` and the output, or an error that says how
/// it failed (a timeout, a status other than 0). The secrets are removed from everything first.
pub(crate) fn finished(
    tool: &str,
    name: &str,
    head: &str,
    secrets: &[String],
    out: &Output,
    timeout: Duration,
) -> Result<String> {
    let stdout = scrub(&out.stdout, secrets);
    let stderr = scrub(&out.stderr, secrets);
    if out.timed_out {
        return fail!(
            "{tool}: {name} did not finish within {} and was ended{}",
            duration_text(timeout),
            tail("; it wrote", &stdout, &stderr)
        );
    }
    if out.exit_code != 0 {
        return fail!(
            "{tool}: {name} exited with status {}{}",
            out.exit_code,
            tail(":", &stderr, &stdout)
        );
    }
    let mut b = format!("{head}{stdout}");
    if !stderr.is_empty() {
        b.push_str(&format!("\n[stderr]\n{stderr}"));
    }
    Ok(b)
}

/// The end of what the program wrote, to explain a failure: `first` (stderr) unless it is empty, then `second`.
fn tail(lead: &str, first: &str, second: &str) -> String {
    let text = if first.trim().is_empty() { second } else { first };
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    const KEEP: usize = 2000;
    if text.len() > KEEP {
        return format!("{lead} ...{}", tail_from(text, text.len().saturating_sub(KEEP)));
    }
    format!("{lead} {text}")
}

#[expect(clippy::expect_used, reason = "the pattern is a constant a test compiles")]
static PLAIN_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_@%+=:,./-]+$").expect("the plain word shape compiles"));

/// `argv` as one line to read, quoting what needs it. It is for the eye: nothing runs it.
pub(crate) fn shell_line(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if PLAIN_WORD.is_match(a) {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What the run tool does for one source, for the catalog and the tool's description: generated from the source's own
/// spec, so what is allowed is said once.
pub(crate) fn run_what(name: &str, spec: &ProgramSpec) -> String {
    format!("{name} runs: {}", spec.commands())
}

/// The run tool's description over the programmed sources `ks`.
fn run_description(set: &Set, ks: &[Kind]) -> String {
    let mut b = String::from(
        "Run a command-line program with the credential of a connection, which you never see: the program runs in a jail with the credential in its environment, reaches only that connection's address, and finds no file but its own scratch directory. \
You give the source and the arguments (an argv, no shell; the program's name is fixed), and read its output. Use it when no typed tool of the source answers; a typed tool comes first. Sources and what each runs:",
    );
    for k in ks {
        let p = set.template(k);
        if let Some(spec) = p.program() {
            b.push_str(&format!("\n- {}: {}", k.id, run_what(&p.name(), &spec)));
        }
    }
    b.push_str("\nRefused before anything runs: any other command; flags that move where the program connects, who it acts as or what it reads (--endpoint-url, --profile, --region, --debug...); a value starting file://, fileb://, http:// or https://. \
Enforced: credential. That the program only reads is the credential's own rights (the owner supplies a read-only credential), not this tool's code. \
Needs a read grant the owner gave for programs on the connection (the Chair asks with request_grant, form env), and is never offered on a protected connection. \
An exit status other than 0, a timeout and a cut output are reported as such. ");
    b.push_str(FROM_OUTSIDE);
    b
}
