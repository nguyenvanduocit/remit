//! The pure engine of Remit's workflows (ADR-0073, decision 3; ADR-0075): a script, its arguments and the journal of
//! results its calls have had go in; the calls it made that have no result yet, or its result, or its error come out. A
//! durable run is the script re-evaluated from its start against a longer journal each time, as Temporal and Claude
//! Code's own resume do. The engine reads no store, file or clock but its evaluation's deadline; the shell in orch records
//! the calls, issues them, and calls [`advance`] again when one has its result.
//!
//! The script is plain JavaScript in one file, written for Claude Code's Workflow tool: `export const meta = {...}` first,
//! then a body that may await and return at the top level. It runs unchanged where it uses the API both engines share;
//! Remit's own verbs (hire, wait, askOwner, accept, sendBack) run only here. QuickJS (through rquickjs) runs it, a fresh
//! runtime per evaluation; oxc reads its meta (`script.rs`); a call's input hashes in one canonical spelling (`hash.rs`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::time::{Duration, Instant};

use model::{ForeignJson, WorkflowVerb};
use rquickjs::context::{EvalOptions, intrinsic};
use rquickjs::convert::Coerced;
use rquickjs::promise::PromiseState;
use rquickjs::{Context, Ctx, FromJs, Function, Promise, Runtime};
use serde::Deserialize;

mod hash;
mod script;

/// How long one evaluation may run JavaScript. A script spends no time waiting inside an evaluation (a call with no
/// result suspends it), so a script over the limit is one that loops without calling a verb.
const EVALUATION_LIMIT: Duration = Duration::from_secs(2);

/// How much memory one evaluation's VM may hold. A script keeps its state in its journal, not its heap, so a run that
/// needs more is one that allocates without bound; the limit keeps it from taking the server's memory.
const HEAP_LIMIT: usize = 64 << 20;

/// How deep the VM's own stack may grow: a script that recurses without bound fails there, inside the thread's stack.
const STACK_LIMIT: usize = 1 << 20;

/// The script's environment, installed before it runs (ADR-0075, decision 3).
const PRELUDE: &str = include_str!("prelude.js");
const MEMORY: &str = include_str!("memory.js");
const API: &str = include_str!("api.js");

/// The name the script's frames carry in an error's stack.
const SCRIPT_FILE: &str = "workflow.js";

/// The context's intrinsics: no Performance (a clock), WeakRef (it shows the garbage collector) or DOMException. Eval
/// stays because QuickJS evaluates through it; the prelude takes `eval` and `Function` from scripts.
type Intrinsics = (
    intrinsic::Date,
    intrinsic::Eval,
    intrinsic::RegExpCompiler,
    intrinsic::RegExp,
    intrinsic::Json,
    intrinsic::Proxy,
    intrinsic::MapSet,
    intrinsic::TypedArrays,
    intrinsic::Promise,
);

/// The recorded result of one call: the call at `seq`, which made `verb` with an input hashing to `hash`, ended with
/// `result` (none is null) or failed with `err` (empty when it did not).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub seq: i64,
    pub verb: WorkflowVerb,
    pub hash: String,
    pub result: Option<ForeignJson>,
    pub err: String,
}

/// One call a script made: its order among the script's calls in an evaluation, its input as canonical JSON and the hash
/// of (verb, input), which its entry must carry back; label and phase are how it is shown: the label the script gave it,
/// and the phase it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub seq: i64,
    pub verb: WorkflowVerb,
    pub input: ForeignJson,
    pub hash: String,
    pub label: String,
    pub phase: String,
}

/// The script's `export const meta` literal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub when_to_use: String,
    #[serde(default, deserialize_with = "nullable")]
    pub phases: Vec<MetaPhase>,
}

/// One phase the script announces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MetaPhase {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub model: String,
}

/// Reads a `null` list as empty.
fn nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<MetaPhase>, D::Error> {
    Option::<Vec<MetaPhase>>::deserialize(d).map(Option::unwrap_or_default)
}

/// Where a script stands after an evaluation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScriptState {
    /// It awaits calls the journal has no result for.
    #[default]
    Suspended,
    /// It returned; [`Outcome::result`] holds its value.
    Done,
    /// It threw, diverged from its journal, ran over the limit or could not be read; [`Outcome::err`] says.
    Failed,
}

/// Why an evaluation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// Replayed against its journal, the script made a different call at `step` than the one recorded there: the script
    /// or its arguments are not the ones the journal was written for.
    Diverged { step: i64, why: String },
    /// It ran over the evaluation limit without waiting on a call.
    TooLong,
    /// It ran out of the memory an evaluation may hold.
    OutOfMemory,
    /// It threw, could not be read, or awaits what no call settles.
    Failed(String),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diverged { step, why } => write!(f, "the script diverged at step {step}: {why}"),
            Self::TooLong => write!(
                f,
                "the script ran {}s without waiting on a call",
                EVALUATION_LIMIT.as_secs()
            ),
            Self::OutOfMemory => write!(f, "the script ran out of memory: it may hold {} MiB", HEAP_LIMIT >> 20),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for ScriptError {}

/// What one evaluation came to.
///
/// `calls` are the calls the script made that the journal has no entry for, in seq order: the shell records each it has
/// not recorded yet, marks the log and phase calls done at once ([`waits`]), and issues the others. A script that returned
/// or threw may still list some (a log before its end, a call it never awaited). A run that diverged, ran over the limit
/// or could not be read lists none: nothing it asked for can be trusted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub state: ScriptState,
    pub meta: Meta,
    pub calls: Vec<Call>,
    pub result: Option<ForeignJson>,
    pub err: Option<ScriptError>,
}

impl Outcome {
    /// Why the evaluation failed, for the run's error.
    pub fn why(&self) -> String {
        self.err
            .as_ref()
            .map_or_else(|| "the script failed".to_owned(), ToString::to_string)
    }

    fn failed(meta: Meta, err: ScriptError) -> Self {
        Self {
            state: ScriptState::Failed,
            meta,
            err: Some(err),
            ..Self::default()
        }
    }
}

/// Whether a call of this verb has a result the script awaits. log and phase do not: they are done when the script
/// makes them.
pub fn waits(verb: &WorkflowVerb) -> bool {
    !matches!(verb, WorkflowVerb::Log | WorkflowVerb::Phase)
}

/// Evaluates script in a fresh VM against journal and says where it stands. It runs JavaScript for up to two seconds:
/// call it off an async runtime's workers.
///
/// The journal lists the results in the order they arrived, which is the order `advance` hands them to the script.
/// Concurrent calls (parallel, pipeline) are numbered by the order the script makes them, and that order depends on the
/// order earlier results settled; replaying them in arrival order makes every evaluation number the calls as the first
/// one did.
pub fn advance(script: &str, args: Option<&ForeignJson>, journal: &[Entry]) -> Outcome {
    advance_within(script, args, journal, EVALUATION_LIMIT)
}

fn advance_within(script: &str, args: Option<&ForeignJson>, journal: &[Entry], limit: Duration) -> Outcome {
    let compiled = match script::compile(script) {
        Ok(c) => c,
        Err(why) => return Outcome::failed(Meta::default(), ScriptError::Failed(why)),
    };
    let meta = compiled.meta;
    let mut recorded = BTreeMap::new();
    for en in journal {
        if recorded.insert(en.seq, (en.verb, en.hash.clone())).is_some() {
            return Outcome::failed(
                meta,
                ScriptError::Failed(format!("the journal records step {} twice", en.seq)),
            );
        }
    }
    let seen = Rc::new(RefCell::new(Seen::default()));
    let ended = evaluate(&compiled.body, args, journal, recorded.clone(), &seen, limit);
    let Seen { calls, failure } = seen.take();
    let ended = match (failure, ended) {
        (Some(err), _) | (None, Err(err)) => return Outcome::failed(meta, err),
        (None, Ok(ended)) => ended,
    };
    let calls: Vec<Call> = calls.into_iter().filter(|c| !recorded.contains_key(&c.seq)).collect();
    let mut out = Outcome {
        meta,
        ..Outcome::default()
    };
    match ended {
        Ended::Returned(result) => {
            out.state = ScriptState::Done;
            // a script that returns null or nothing has no result, as a stored run reads one
            out.result = Some(result).filter(|r| !r.0.is_null());
        }
        Ended::Threw(why) => {
            out.state = ScriptState::Failed;
            out.err = Some(ScriptError::Failed(format!("the script threw: {why}")));
        }
        Ended::Pending if !calls.iter().any(|c| waits(&c.verb)) => {
            out.state = ScriptState::Failed;
            out.err = Some(ScriptError::Failed(
                "the script awaits something that is not a call, so it can never go on".to_owned(),
            ));
        }
        Ended::Pending => out.state = ScriptState::Suspended,
    }
    out.calls = calls;
    out
}

/// What one evaluation has seen, shared by the `call` host function, the memory watch and the interrupt handler: the calls
/// the script made, and the failure that stops it (a divergence found inside a call, the VM out of memory, or the
/// deadline).
#[derive(Default)]
struct Seen {
    calls: Vec<Call>,
    failure: Option<ScriptError>,
}

impl Seen {
    fn stop(&mut self, err: ScriptError) {
        self.failure.get_or_insert(err);
    }
}

/// Where the script's promise stood once the journal was handed to it.
enum Ended {
    Pending,
    Returned(ForeignJson),
    Threw(String),
}

/// Runs the script in a fresh runtime: installs the prelude and the API, starts the script, then hands it the journal's
/// results in arrival order, running its promise jobs after each.
fn evaluate(
    body: &str,
    args: Option<&ForeignJson>,
    journal: &[Entry],
    recorded: BTreeMap<i64, (WorkflowVerb, String)>,
    seen: &Rc<RefCell<Seen>>,
    limit: Duration,
) -> Result<Ended, ScriptError> {
    let vm_failed = |e: rquickjs::Error| ScriptError::Failed(format!("start the script's VM: {e}"));
    let rt = Runtime::new().map_err(vm_failed)?;
    rt.set_memory_limit(HEAP_LIMIT);
    rt.set_max_stack_size(STACK_LIMIT);
    let started = Instant::now();
    let watched = Rc::clone(seen);
    // QuickJS calls this every few thousand instructions; true raises an exception no try/catch in the script can catch.
    rt.set_interrupt_handler(Some(Box::new(move || {
        let Ok(mut s) = watched.try_borrow_mut() else {
            return false;
        };
        if s.failure.is_none() && started.elapsed() >= limit {
            s.stop(ScriptError::TooLong);
        }
        s.failure.is_some()
    })));
    let ctx = Context::custom::<Intrinsics>(&rt).map_err(vm_failed)?;
    ctx.with(|ctx| {
        let thrown = |e: rquickjs::Error| {
            let failure = seen.borrow().failure.clone();
            failure.unwrap_or_else(|| ScriptError::Failed(format!("the script threw: {}", caught(&ctx, e))))
        };
        let watch: Function = ctx
            .eval(MEMORY)
            .map_err(|e| ScriptError::Failed(format!("install the memory watch: {}", caught(&ctx, e))))?;
        let heard = Rc::clone(seen);
        let out_of_memory = Function::new(ctx.clone(), move || {
            if let Ok(mut s) = heard.try_borrow_mut() {
                s.stop(ScriptError::OutOfMemory);
            }
        })
        .map_err(|e| ScriptError::Failed(format!("install the memory watch: {}", caught(&ctx, e))))?;
        watch
            .call::<_, ()>((out_of_memory,))
            .map_err(|e| ScriptError::Failed(format!("install the memory watch: {}", caught(&ctx, e))))?;
        ctx.eval::<(), _>(PRELUDE)
            .map_err(|e| ScriptError::Failed(format!("install the prelude: {}", caught(&ctx, e))))?;
        let install: Function = ctx
            .eval(API)
            .map_err(|e| ScriptError::Failed(format!("install the API: {}", caught(&ctx, e))))?;
        let host = Rc::clone(seen);
        let call = Function::new(
            ctx.clone(),
            move |verb: Coerced<String>, input: Coerced<String>, label: Coerced<String>, phase: Coerced<String>| {
                host.borrow_mut().call(&recorded, &verb, &input, label.0, phase.0)
            },
        )
        .map_err(|e| ScriptError::Failed(format!("install the API: {}", caught(&ctx, e))))?;
        let settle: Function = install
            .call((call,))
            .map_err(|e| ScriptError::Failed(format!("install the API: {}", caught(&ctx, e))))?;
        let args = match args {
            Some(a) => ctx.json_parse(a.text()),
            None => Ok(rquickjs::Value::new_undefined(ctx.clone())),
        };
        let args = args.map_err(|e| ScriptError::Failed(format!("set args: {}", caught(&ctx, e))))?;
        ctx.globals()
            .set("args", args)
            .map_err(|e| ScriptError::Failed(format!("set args: {}", caught(&ctx, e))))?;

        let mut options = EvalOptions::default();
        options.filename = Some(SCRIPT_FILE.to_owned());
        let main: Function = ctx.eval_with_options(body, options).map_err(thrown)?;
        let result: Promise = main.call(()).map_err(thrown)?;
        drain(&ctx, seen)?;

        for en in journal.iter().filter(|en| waits(&en.verb)) {
            made(seen, en, "has a result for")?;
            let (ok, payload) = if en.err.is_empty() {
                (true, en.result.as_ref().map(ForeignJson::text).unwrap_or_default())
            } else {
                (false, en.err.clone())
            };
            settle.call::<_, ()>((en.seq, ok, payload)).map_err(thrown)?;
            drain(&ctx, seen)?;
        }
        for en in journal {
            made(seen, en, "records")?;
        }
        match result.state() {
            PromiseState::Pending => Ok(Ended::Pending),
            PromiseState::Rejected => Ok(Ended::Threw(match result.result::<rquickjs::Value>() {
                Some(Err(e)) => caught(&ctx, e),
                _ => "a value that is not an error".to_owned(),
            })),
            PromiseState::Resolved => {
                let value = match result.result::<rquickjs::Value>() {
                    Some(Ok(v)) => v,
                    Some(Err(e)) => return Err(thrown(e)),
                    None => rquickjs::Value::new_undefined(ctx.clone()),
                };
                let json = ctx
                    .json_stringify(value)
                    .and_then(|s| s.map(|s| s.to_string()).transpose())
                    .map_err(|e| {
                        ScriptError::Failed(format!("the script's result is not JSON: {}", caught(&ctx, e)))
                    })?;
                let json = json.unwrap_or_else(|| "null".to_owned());
                ForeignJson::parse(&json)
                    .map(Ended::Returned)
                    .map_err(|e| ScriptError::Failed(format!("the script's result is not JSON: {e}")))
            }
        }
    })
}

impl Seen {
    /// The Rust side of every verb: numbers the call, checks it against the journal's entry at that step, and records it.
    /// A divergence stops the VM through the interrupt handler rather than throwing, so no try/catch in the script can
    /// swallow it.
    fn call(
        &mut self,
        recorded: &BTreeMap<i64, (WorkflowVerb, String)>,
        verb: &str,
        input: &str,
        label: String,
        phase: String,
    ) -> i64 {
        let seq = i64::try_from(self.calls.len()).unwrap_or(i64::MAX);
        let verb: WorkflowVerb = match verb.parse() {
            Ok(verb) => verb,
            Err(e) => {
                self.stop(ScriptError::Failed(format!("step {seq}: {e}")));
                return seq;
            }
        };
        let canonical = match hash::canonical(input) {
            Ok(canonical) => canonical,
            Err(e) => {
                self.stop(ScriptError::Failed(format!("step {seq}: {e}")));
                return seq;
            }
        };
        // the hash is of the canonical text, as the journal recorded it
        let hash = hash::input_hash(&verb, &canonical);
        let input = match ForeignJson::parse(&canonical) {
            Ok(input) => input,
            Err(e) => {
                self.stop(ScriptError::Failed(format!("step {seq}: write call input: {e}")));
                return seq;
            }
        };
        if let Some((was, was_hash)) = recorded.get(&seq) {
            if *was != verb {
                let why = format!("the journal recorded {was}, the script called {verb}");
                self.stop(ScriptError::Diverged { step: seq, why });
            } else if *was_hash != hash {
                let why = format!("the {verb} call's input is not the one recorded");
                self.stop(ScriptError::Diverged { step: seq, why });
            }
        }
        self.calls.push(Call {
            seq,
            verb,
            input,
            hash,
            label,
            phase,
        });
        seq
    }
}

/// Runs the script's promise jobs until none is left, or until the evaluation is stopped: a job that throws is a promise
/// rejected, which the script sees, not a failure of the run.
fn drain(ctx: &Ctx<'_>, seen: &Rc<RefCell<Seen>>) -> Result<(), ScriptError> {
    loop {
        if let Some(f) = seen.borrow().failure.clone() {
            return Err(f);
        }
        if !ctx.execute_pending_job() {
            return Ok(());
        }
    }
}

/// Fails the evaluation when the journal has an entry for a call the script has not made.
fn made(seen: &Rc<RefCell<Seen>>, en: &Entry, what: &str) -> Result<(), ScriptError> {
    let made = i64::try_from(seen.borrow().calls.len()).unwrap_or(i64::MAX);
    if en.seq < made {
        return Ok(());
    }
    let why = format!("the journal {what} {}, a call the script did not make", en.verb);
    Err(ScriptError::Diverged { step: en.seq, why })
}

/// The exception an error out of QuickJS carries, as the script would print it: `TypeError: message`, then the line of
/// the script it was thrown from (not the prelude's or the API's).
fn caught(ctx: &Ctx<'_>, e: rquickjs::Error) -> String {
    if !matches!(e, rquickjs::Error::Exception) {
        return e.to_string();
    }
    let value = ctx.catch();
    let text = Coerced::<String>::from_js(ctx, value.clone())
        .map(|c| c.0)
        .unwrap_or_else(|_| "a value that is not text".to_owned());
    let at = value.as_exception().and_then(rquickjs::Exception::stack).and_then(|s| {
        s.lines()
            .map(str::trim)
            .find(|l| l.contains(SCRIPT_FILE))
            .map(str::to_owned)
    });
    match at {
        Some(at) => format!("{text} ({at})"),
        None => text,
    }
}
