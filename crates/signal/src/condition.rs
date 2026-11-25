//! A subscription's condition is an expression in CEL, a small language made to be run on untrusted input (it has no loop it
//! cannot bound, no files, no network, and a cost bound stops one that would run long). It reads one reading as it arrived:
//!
//! - `value`: the reading's JSON (dyn); a value that is no JSON has no fields
//! - `header`: the string members of `value.header`, or of `value.event.header` for a reading of an event signal; empty when
//!   it has none
//! - `body`: for a reading of an event signal, the JSON of `value.event.body`; empty when the body is no JSON or was cut
//! - `now`: the time of the evaluation (timestamp, in UTC)
//! - `chain`: the chain that owns the subscription: id, marks, status; empty for a subscription of no chain
//! - `cause`: who made the reading: kind, who, chainId
//!
//! A field a reading lacks makes the condition false, never an error that stops the other subscriptions: a condition that
//! fails to run holds for nothing. The shell calls `check` when a subscription is made, so a condition that cannot compile
//! never gets stored; `evaluate` treats one that does (the store was edited by hand) as false rather than act on a part of
//! it. The owner never writes an expression: he tells the Assistant, who looks at a real reading and writes it.
//!
//! The `cel` crate parses and runs; `check.rs` is the type check cel-go runs at compile, which the crate lacks.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, LazyLock};

use cel::common::ast::{CallExpr, EntryExpr, Expr, IdedExpr, LiteralValue, operators};
use cel::common::types::{CelBool, CelString};
use cel::objects::{Key, Map};
use cel::{Context, Env, Value};
use model::{Reading, Time};
use serde_json::Value as Json;

use crate::ChainFacts;
use crate::check::{self, Kind};

/// The longest a condition may be.
const EXPRESSION_MAX: usize = 1000;

/// The cost one evaluation may reach, as cel-go's `CostLimit`: a comprehension's iterations, multiplied through its nesting,
/// count at least two each (the step and the loop condition).
const EVAL_COST: u64 = 100_000;

/// The environment every condition is parsed and run in: the standard library and the strings extension, with no optional
/// syntax (cel-go's environment declares none). It is costly to build, read-only once built and the same for every
/// evaluation, which is why it is the one static of the crate.
static CONDITION_ENV: LazyLock<Result<Arc<Env>, String>> = LazyLock::new(|| {
    let mut env = Env::stdlib().with_optional_support(false);
    env.add_extension(cel::extensions::strings)
        .map_err(|e| format!("the condition environment does not build: {e}"))?;
    Ok(Arc::new(env))
});

fn condition_env() -> Result<&'static Arc<Env>, ExprError> {
    CONDITION_ENV.as_ref().map_err(|e| ExprError::Run(e.clone()))
}

/// Why an expression cannot be a condition, or could not be read on a reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprError {
    TooLong {
        what: &'static str,
        chars: usize,
    },
    DoesNotCompile {
        what: &'static str,
        message: String,
    },
    WrongKind {
        what: &'static str,
        gives: String,
        takes: &'static str,
    },
    /// The expression ran past its cost bound.
    TooCostly,
    /// The expression failed as it ran.
    Run(String),
    /// A condition gave something other than true or false.
    NotBool(String),
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprError::TooLong { what, chars } => {
                write!(f, "{what} is too long ({chars} characters, at most {EXPRESSION_MAX})")
            }
            ExprError::DoesNotCompile { what, message } => write!(f, "{what} does not compile: {message}"),
            ExprError::WrongKind { what, gives, takes } => {
                write!(f, "{what} gives {gives}, which is not what it takes ({takes})")
            }
            ExprError::TooCostly => f.write_str("operation cancelled: actual cost limit exceeded"),
            ExprError::Run(message) => f.write_str(message),
            ExprError::NotBool(v) => write!(f, "the condition gave {v}, and a condition gives true or false"),
        }
    }
}

impl std::error::Error for ExprError {}

/// `check` is `Ok` when `expr` can be a subscription's condition: empty, or short enough, compiling, and giving a boolean.
/// The shell calls it when a subscription is made, and tells the Assistant what it says.
pub fn check(expr: &str) -> Result<(), ExprError> {
    if empty(expr) {
        return Ok(());
    }
    compile(expr).map(|_| ())
}

/// `check_text` is `Ok` when `expr` can be what an action reads as text: empty, or short enough, compiling, and giving text.
pub fn check_text(expr: &str) -> Result<(), ExprError> {
    if empty(expr) {
        return Ok(());
    }
    compile_as(expr, "expression", "text", &[Kind::String]).map(|_| ())
}

/// `check_texts` is `Ok` when `expr` can be what an action reads as text or a list of text, such as the marks of a chain.
pub fn check_texts(expr: &str) -> Result<(), ExprError> {
    if empty(expr) {
        return Ok(());
    }
    compile_as(
        expr,
        "expression",
        "text or a list of text",
        &[Kind::String, Kind::List],
    )
    .map(|_| ())
}

/// What `expr` gives on `r` at `now`, as a list of text (one text is a list of one; empty ones are dropped), for an
/// expression that compiled and ran; `None` for one that did not, so a caller can say it gave nothing.
pub fn eval_texts(expr: &str, r: &Reading, now: Time) -> Option<Vec<String>> {
    eval_texts_on(expr, &facts_of(r).activation("", &ChainFacts::default(), now))
}

pub(crate) fn eval_texts_on(expr: &str, act: &Activation) -> Option<Vec<String>> {
    let prg = compile_as(
        expr,
        "expression",
        "text or a list of text",
        &[Kind::String, Kind::List],
    )
    .ok()?;
    let out = match prg.run(act).ok()? {
        Value::String(s) => vec![s.as_str().to_owned()],
        Value::List(items) => items
            .iter()
            .map(|v| match v {
                Value::String(s) => Some(s.as_str().to_owned()),
                _ => None,
            })
            .collect::<Option<Vec<String>>>()?,
        _ => return None,
    };
    Some(out.into_iter().filter(|s| !s.trim().is_empty()).collect())
}

/// What `expr` gives on `r` at `now`, as text; `None` when it did not compile or run.
pub fn eval_text(expr: &str, r: &Reading, now: Time) -> Option<String> {
    eval_text_on(expr, &facts_of(r).activation("", &ChainFacts::default(), now))
}

pub(crate) fn eval_text_on(expr: &str, act: &Activation) -> Option<String> {
    let prg = compile_as(expr, "expression", "text", &[Kind::String]).ok()?;
    match prg.run(act).ok()? {
        Value::String(s) => Some(s.as_str().to_owned()),
        _ => None,
    }
}

pub(crate) fn empty(expr: &str) -> bool {
    expr.trim().is_empty()
}

/// An expression compiled: the tree it runs (lowered, see `lower`), whether it reads the signal, and what bounds its cost.
#[derive(Debug, Clone)]
pub(crate) struct Program {
    run: IdedExpr,
    pub(crate) reads_signal: bool,
    pub(crate) gives: check::Ty,
    depth: u32,
    widest_literal: u64,
}

/// The program of `expr`, a condition, bounded however it is written.
pub(crate) fn compile(expr: &str) -> Result<Program, ExprError> {
    compile_as(expr, "condition", "true or false", &[Kind::Bool])
}

/// The program of `expr`, parsed and checked, without the check of what it gives; `what` names it in the error.
pub(crate) fn parse(expr: &str, what: &'static str) -> Result<Program, ExprError> {
    let chars = expr.chars().count();
    if chars > EXPRESSION_MAX {
        return Err(ExprError::TooLong { what, chars });
    }
    let parsed = condition_env()?.compile(expr).map_err(|e| ExprError::DoesNotCompile {
        what,
        message: e.to_string(),
    })?;
    let checked = check::check(parsed.expression()).map_err(|e| ExprError::DoesNotCompile {
        what,
        message: e.to_string(),
    })?;
    let refs = parsed.references();
    let reads_signal = refs.has_variable("value") || refs.has_variable("body");
    let mut widest_literal = 0;
    let run = lower(parsed.expression(), &mut widest_literal);
    Ok(Program {
        run,
        reads_signal,
        gives: checked.ty,
        depth: checked.depth,
        widest_literal,
    })
}

/// The program of `expr`, bounded however it is written, which must give one of `kinds` (or dyn, which only a run can
/// tell). `what` and `takes` name it and what it takes in the error.
fn compile_as(expr: &str, what: &'static str, takes: &'static str, kinds: &[Kind]) -> Result<Program, ExprError> {
    let prg = parse(expr, what)?;
    let kind = prg.gives.kind();
    if kind != Kind::Dyn && !kinds.contains(&kind) {
        return Err(ExprError::WrongKind {
            what,
            gives: prg.gives.to_string(),
            takes,
        });
    }
    Ok(prg)
}

impl Program {
    /// What the program gives on `act`; an error when it fails or would run past its cost bound. The bound is counted
    /// before the run, from how deep its comprehensions nest and the longest list or map they could range over.
    pub(crate) fn run(&self, act: &Activation) -> Result<Value, ExprError> {
        if self.depth > 0 {
            let width = act.widest.max(self.widest_literal).max(1);
            let cost = width.saturating_pow(self.depth).saturating_mul(2);
            if cost > EVAL_COST {
                return Err(ExprError::TooCostly);
            }
        }
        let mut ctx = Context::with_env(Arc::clone(condition_env()?));
        for (name, value) in [
            ("value", &act.value),
            ("header", &act.header),
            ("body", &act.body),
            ("now", &act.now),
            ("chain", &act.chain),
            ("cause", &act.cause),
        ] {
            ctx.add_variable_from_value(name, value.clone());
        }
        ctx.add_function(PRESENT, present)
            .map_err(|e| ExprError::Run(e.to_string()))?;
        ctx.add_function(NAN, nan).map_err(|e| ExprError::Run(e.to_string()))?;
        ctx.resolve(&self.run).map_err(|e| ExprError::Run(e.to_string()))
    }
}

/// The function `has(x.f)` is lowered to: false on a scalar, as cel-go answers, where the crate errs.
const PRESENT: &str = "@present";
/// The function an ordering is guarded by: a NaN orders as false in cel-go, where the crate errs.
const NAN: &str = "@nan";

fn present(of: Value, field: Arc<String>) -> Result<bool, cel::ExecutionError> {
    match of {
        Value::Map(m) => Ok(m.map.contains_key(&Key::String(field))),
        // A list takes an index, not a field: cel-go refuses to qualify it with one.
        Value::List(_) => Err(cel::ExecutionError::function_error(
            "has",
            format!("unsupported index type for field {field}"),
        )),
        _ => Ok(false),
    }
}

fn nan(a: Value, b: Value) -> bool {
    matches!(a, Value::Float(x) if x.is_nan()) || matches!(b, Value::Float(x) if x.is_nan())
}

/// `e` as it runs: `has(x.f)` is `@present(x, "f")`, and `a < b` (each ordering) is `@nan(a, b) ? false : a < b`, so the
/// crate answers what cel-go answers. `widest` is raised to the longest list literal in `e`, which a comprehension may range
/// over.
fn lower(e: &IdedExpr, widest: &mut u64) -> IdedExpr {
    let id = e.id;
    let node = |expr: Expr| IdedExpr { id, expr };
    let call = |name: &str, args: Vec<IdedExpr>| {
        node(Expr::Call(CallExpr {
            func_name: name.to_owned(),
            target: None,
            args,
        }))
    };
    let expr = match &e.expr {
        Expr::Select(s) if s.test => {
            let field = node(Expr::Literal(LiteralValue::String(CelString::from(s.field.clone()))));
            return call(PRESENT, vec![lower(&s.operand, widest), field]);
        }
        Expr::Select(s) => {
            let mut s = s.clone();
            s.operand = Box::new(lower(&s.operand, widest));
            Expr::Select(s)
        }
        Expr::Call(c) => {
            let mut c = c.clone();
            c.target = c.target.map(|t| Box::new(lower(&t, widest)));
            c.args = c.args.iter().map(|a| lower(a, widest)).collect();
            let ordering = [
                operators::LESS,
                operators::LESS_EQUALS,
                operators::GREATER,
                operators::GREATER_EQUALS,
            ];
            if c.target.is_none() && c.args.len() == 2 && ordering.contains(&c.func_name.as_str()) {
                let guard = call(NAN, c.args.clone());
                let no = node(Expr::Literal(LiteralValue::Boolean(CelBool::from(false))));
                return call(operators::CONDITIONAL, vec![guard, no, node(Expr::Call(c))]);
            }
            Expr::Call(c)
        }
        Expr::List(l) => {
            *widest = (*widest).max(u64::try_from(l.elements.len()).unwrap_or(u64::MAX));
            let mut l = l.clone();
            l.elements = l.elements.iter().map(|x| lower(x, widest)).collect();
            Expr::List(l)
        }
        Expr::Map(m) => {
            let mut m = m.clone();
            for entry in &mut m.entries {
                if let EntryExpr::MapEntry(me) = &mut entry.expr {
                    me.key = lower(&me.key, widest);
                    me.value = lower(&me.value, widest);
                }
            }
            Expr::Map(m)
        }
        Expr::Comprehension(c) => {
            let mut c = c.clone();
            c.iter_range = lower(&c.iter_range, widest);
            c.accu_init = lower(&c.accu_init, widest);
            c.loop_cond = lower(&c.loop_cond, widest);
            c.loop_step = lower(&c.loop_step, widest);
            c.result = lower(&c.result, widest);
            Expr::Comprehension(c)
        }
        other => other.clone(),
    };
    node(expr)
}

/// The programs of one evaluation, each compiled once however many readings it judges.
#[derive(Default)]
pub(crate) struct Conditions(HashMap<String, Result<Program, ExprError>>);

impl Conditions {
    /// Whether `expr` holds on `act`: an empty one does. One that does not compile, fails to run, runs past its bounds or
    /// gives anything but true or false does not, and the error says why (`evaluate` records it as condition_error).
    pub(crate) fn holds(&mut self, expr: &str, act: &Activation) -> Result<bool, ExprError> {
        if empty(expr) {
            return Ok(true);
        }
        let prg = self
            .0
            .entry(expr.to_owned())
            .or_insert_with(|| compile(expr))
            .as_ref()
            .map_err(Clone::clone)?;
        match prg.run(act)? {
            Value::Bool(held) => Ok(held),
            other => Err(ExprError::NotBool(shown(&other))),
        }
    }
}

/// A value as an error shows it.
pub(crate) fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.as_str().to_owned(),
        Value::Int(i) => i.to_string(),
        Value::UInt(u) => u.to_string(),
        Value::Float(f) => f.to_string(),
        other => format!("{other:?}"),
    }
}

/// What the conditions see of one reading, decoded once for all the subscriptions that judge it.
pub(crate) struct ReadingFacts {
    value: Json,
    header: BTreeMap<String, String>,
    body: Json,
    cause: BTreeMap<String, String>,
}

pub(crate) fn facts_of(r: &Reading) -> ReadingFacts {
    let value = r
        .value
        .as_ref()
        .and_then(|raw| serde_json::from_value::<Json>(raw.0.clone()).ok())
        .filter(|v| !v.is_null())
        .unwrap_or_else(empty_object);
    let mut header = BTreeMap::new();
    let mut body = empty_object();
    if let Json::Object(obj) = &value {
        add_strings(&mut header, obj.get("header"));
        if let Some(Json::Object(event)) = obj.get("event") {
            add_strings(&mut header, event.get("header"));
            body = event_body(event);
        }
    }
    let cause = BTreeMap::from([
        ("kind".to_owned(), r.cause.actor.kind.as_str().to_owned()),
        ("who".to_owned(), r.cause.actor.who.clone()),
        ("chainId".to_owned(), r.cause.chain_id.clone()),
    ]);
    ReadingFacts {
        value,
        header,
        body,
        cause,
    }
}

fn empty_object() -> Json {
    Json::Object(serde_json::Map::new())
}

/// Adds the string members of `members`, a JSON object, to `into`.
fn add_strings(into: &mut BTreeMap<String, String>, members: Option<&Json>) {
    if let Some(Json::Object(obj)) = members {
        for (k, v) in obj {
            if let Json::String(s) = v {
                into.insert(k.clone(), s.clone());
            }
        }
    }
}

/// The JSON an event carries as its body: nothing when the body was cut or is no JSON, so no field of it is there.
fn event_body(event: &serde_json::Map<String, Json>) -> Json {
    let cut = matches!(event.get("bodyCut"), Some(Json::Bool(true)));
    match event.get("body") {
        Some(Json::String(text)) if !cut && !text.is_empty() => serde_json::from_str::<Json>(text)
            .ok()
            .filter(|v| !v.is_null())
            .unwrap_or_else(empty_object),
        _ => empty_object(),
    }
}

impl ReadingFacts {
    /// What a condition of a subscription of chain `chain_id` (its facts; the default when unknown) sees at `now`.
    pub(crate) fn activation(&self, chain_id: &str, chain: &ChainFacts, now: Time) -> Activation {
        let chain = Json::Object(serde_json::Map::from_iter([
            ("id".to_owned(), Json::String(chain_id.to_owned())),
            (
                "marks".to_owned(),
                Json::Array(chain.marks.iter().cloned().map(Json::String).collect()),
            ),
            ("status".to_owned(), Json::String(chain.status.as_str().to_owned())),
        ]));
        Activation::new(&self.value, &self.header, &self.body, now, &chain, &self.cause)
    }
}

/// The variables a condition runs on, as CEL values, and the longest list or map among them (what bounds a loop).
pub(crate) struct Activation {
    value: Value,
    header: Value,
    body: Value,
    now: Value,
    chain: Value,
    cause: Value,
    widest: u64,
}

impl Activation {
    pub(crate) fn new(
        value: &Json,
        header: &BTreeMap<String, String>,
        body: &Json,
        now: Time,
        chain: &Json,
        cause: &BTreeMap<String, String>,
    ) -> Activation {
        let mut widest = 0;
        let value = from_json(value, &mut widest);
        let body = from_json(body, &mut widest);
        let chain = from_json(chain, &mut widest);
        Activation {
            value,
            header: text_map(header),
            body,
            now: timestamp(now),
            chain,
            cause: text_map(cause),
            widest,
        }
    }
}

/// JSON as a CEL value: every number is a double.
fn from_json(j: &Json, widest: &mut u64) -> Value {
    let mut note = |n: usize| *widest = (*widest).max(u64::try_from(n).unwrap_or(u64::MAX));
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Float(n.as_f64().unwrap_or(f64::NAN)),
        Json::String(s) => Value::String(Arc::new(s.clone())),
        Json::Array(a) => {
            note(a.len());
            Value::List(Arc::new(a.iter().map(|x| from_json(x, widest)).collect()))
        }
        Json::Object(o) => {
            note(o.len());
            let map = o
                .iter()
                .map(|(k, v)| (Key::String(Arc::new(k.clone())), from_json(v, widest)))
                .collect();
            Value::Map(Map { map: Arc::new(map) })
        }
    }
}

fn text_map(m: &BTreeMap<String, String>) -> Value {
    let map = m
        .iter()
        .map(|(k, v)| (Key::String(Arc::new(k.clone())), Value::String(Arc::new(v.clone()))))
        .collect();
    Value::Map(Map { map: Arc::new(map) })
}

/// `now` in UTC, so `now.getHours()` reads the hour in UTC as cel-go does, whatever offset the caller's time carries.
fn timestamp(t: Time) -> Value {
    let at = t.0;
    let utc = chrono::DateTime::from_timestamp(at.unix_timestamp(), at.nanosecond()).unwrap_or_default();
    Value::Timestamp(utc.fixed_offset())
}
