//! The type check cel-go runs when it compiles a condition, which the `cel` crate does not: it gives an expression's type
//! from the six variables a condition reads, and refuses what cel-go refuses (a name no condition declares, a function it
//! does not have, an overload no argument fits, a comparison of an int with a double). `dyn`, the type of a reading's
//! JSON, fits everything, as in cel-go: a field of the value is checked only when the condition runs.

use std::fmt;

use cel::common::ast::{EntryExpr, Expr, IdedExpr, LiteralValue};

/// A CEL type as the checker reasons about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ty {
    Dyn,
    Bool,
    Int,
    Uint,
    Double,
    String,
    Bytes,
    Null,
    Timestamp,
    Duration,
    Type,
    List(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
}

impl Ty {
    fn list(of: Ty) -> Ty {
        Ty::List(Box::new(of))
    }

    fn map(key: Ty, value: Ty) -> Ty {
        Ty::Map(Box::new(key), Box::new(value))
    }

    /// The kind `compile_as` asks for: a list or a map of any element is one kind.
    pub(crate) fn kind(&self) -> Kind {
        match self {
            Ty::Dyn => Kind::Dyn,
            Ty::Bool => Kind::Bool,
            Ty::String => Kind::String,
            Ty::List(_) => Kind::List,
            _ => Kind::Other,
        }
    }
}

/// The kinds a caller of `compile_as` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Dyn,
    Bool,
    String,
    List,
    Other,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Dyn => f.write_str("dyn"),
            Ty::Bool => f.write_str("bool"),
            Ty::Int => f.write_str("int"),
            Ty::Uint => f.write_str("uint"),
            Ty::Double => f.write_str("double"),
            Ty::String => f.write_str("string"),
            Ty::Bytes => f.write_str("bytes"),
            Ty::Null => f.write_str("null_type"),
            Ty::Timestamp => f.write_str("google.protobuf.Timestamp"),
            Ty::Duration => f.write_str("google.protobuf.Duration"),
            Ty::Type => f.write_str("type"),
            Ty::List(e) => write!(f, "list({e})"),
            Ty::Map(k, v) => write!(f, "map({k}, {v})"),
        }
    }
}

/// Whether a value of type `have` may stand where `want` is asked: equal types, or dyn on either side, element by element.
fn fits(have: &Ty, want: &Ty) -> bool {
    match (have, want) {
        (Ty::Dyn, _) | (_, Ty::Dyn) => true,
        (Ty::List(a), Ty::List(b)) => fits(a, b),
        (Ty::Map(ak, av), Ty::Map(bk, bv)) => fits(ak, bk) && fits(av, bv),
        _ => have == want,
    }
}

/// The type two types that fit each other join to: dyn when either is, as cel-go's most general type.
fn join(a: &Ty, b: &Ty) -> Option<Ty> {
    match (a, b) {
        (Ty::Dyn, _) | (_, Ty::Dyn) => Some(Ty::Dyn),
        (Ty::List(x), Ty::List(y)) => join(x, y).map(Ty::list),
        (Ty::Map(xk, xv), Ty::Map(yk, yv)) => Some(Ty::map(join(xk, yk)?, join(xv, yv)?)),
        _ if a == b => Some(a.clone()),
        _ => None,
    }
}

/// The element types of a literal join to dyn when they differ: cel-go takes a heterogeneous literal as a list of dyn.
fn join_loose(a: &Ty, b: &Ty) -> Ty {
    join(a, b).unwrap_or(Ty::Dyn)
}

/// The variables every condition reads (`condition.rs`), with their types.
fn declared(name: &str) -> Option<Ty> {
    Some(match name {
        "value" | "body" => Ty::Dyn,
        "header" | "cause" => Ty::map(Ty::String, Ty::String),
        "now" => Ty::Timestamp,
        "chain" => Ty::map(Ty::String, Ty::Dyn),
        // The type names, which `type(x) == string` compares with.
        "bool" | "int" | "uint" | "double" | "string" | "bytes" | "list" | "map" | "null_type" | "type" => Ty::Type,
        _ => return None,
    })
}

/// What the checker refuses, in cel-go's words where the owner's Assistant reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckError {
    Undeclared(String),
    NoOverload { function: String, args: Vec<Ty> },
    NoFieldSelection { ty: Ty, field: String },
    NotARange(Ty),
    Unsupported(&'static str),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::Undeclared(name) => write!(f, "undeclared reference to '{name}'"),
            CheckError::NoOverload { function, args } => {
                let args: Vec<String> = args.iter().map(ToString::to_string).collect();
                write!(
                    f,
                    "found no matching overload for '{function}' applied to '({})'",
                    args.join(", ")
                )
            }
            CheckError::NoFieldSelection { ty, field } => {
                write!(f, "type '{ty}' does not support field selection (field '{field}')")
            }
            CheckError::NotARange(ty) => write!(f, "expression of type '{ty}' cannot be range of a comprehension"),
            CheckError::Unsupported(what) => write!(f, "{what} is not supported in a condition"),
        }
    }
}

/// The checked shape of an expression: its type, and how deep its comprehensions nest (what bounds its cost).
pub(crate) struct Checked {
    pub(crate) ty: Ty,
    pub(crate) depth: u32,
}

/// The type of `expr`, or why cel-go would not compile it.
pub(crate) fn check(expr: &IdedExpr) -> Result<Checked, CheckError> {
    let mut checker = Checker {
        scope: Vec::new(),
        depth: 0,
        deepest: 0,
    };
    let ty = checker.ty(expr)?;
    Ok(Checked {
        ty,
        depth: checker.deepest,
    })
}

struct Checker {
    /// The comprehension variables in scope, innermost last.
    scope: Vec<(String, Ty)>,
    depth: u32,
    deepest: u32,
}

impl Checker {
    fn ty(&mut self, e: &IdedExpr) -> Result<Ty, CheckError> {
        match &e.expr {
            Expr::Unspecified => Err(CheckError::Unsupported("an empty expression")),
            Expr::Literal(lit) => Ok(match lit {
                LiteralValue::Boolean(_) => Ty::Bool,
                LiteralValue::Bytes(_) => Ty::Bytes,
                LiteralValue::Double(_) => Ty::Double,
                LiteralValue::Int(_) => Ty::Int,
                LiteralValue::Null => Ty::Null,
                LiteralValue::String(_) => Ty::String,
                LiteralValue::UInt(_) => Ty::Uint,
            }),
            Expr::Ident(name) => self.ident(name),
            Expr::List(list) => {
                let mut elem: Option<Ty> = None;
                for el in &list.elements {
                    let t = self.ty(el)?;
                    elem = Some(elem.map_or(t.clone(), |prev| join_loose(&prev, &t)));
                }
                Ok(Ty::list(elem.unwrap_or(Ty::Dyn)))
            }
            Expr::Map(map) => {
                let mut kv: Option<(Ty, Ty)> = None;
                for entry in &map.entries {
                    let EntryExpr::MapEntry(entry) = &entry.expr else {
                        return Err(CheckError::Unsupported("a message literal"));
                    };
                    let (k, v) = (self.ty(&entry.key)?, self.ty(&entry.value)?);
                    kv = Some(match kv {
                        None => (k, v),
                        Some((pk, pv)) => (join_loose(&pk, &k), join_loose(&pv, &v)),
                    });
                }
                let (k, v) = kv.unwrap_or((Ty::Dyn, Ty::Dyn));
                Ok(Ty::map(k, v))
            }
            Expr::Struct(_) => Err(CheckError::Unsupported("a message literal")),
            Expr::Select(select) => {
                let operand = self.ty(&select.operand)?;
                let field = || CheckError::NoFieldSelection {
                    ty: operand.clone(),
                    field: select.field.clone(),
                };
                let value = match &operand {
                    Ty::Dyn => Ty::Dyn,
                    Ty::Map(k, v) if fits(k, &Ty::String) => (**v).clone(),
                    _ => return Err(field()),
                };
                Ok(if select.test { Ty::Bool } else { value })
            }
            Expr::Call(call) => {
                // `strings.quote(x)` parses as a call on the name `strings`, which is the namespace, not a variable.
                if let Some(target) = &call.target
                    && let Expr::Ident(ns) = &target.expr
                    && ns == "strings"
                    && call.func_name == "quote"
                    && self.local(ns).is_none()
                {
                    let args = self.tys(&call.args)?;
                    return resolve("strings.quote", None, &args);
                }
                let target = match &call.target {
                    Some(t) => Some(self.ty(t)?),
                    None => None,
                };
                let args = self.tys(&call.args)?;
                resolve(&call.func_name, target.as_ref(), &args)
            }
            Expr::Comprehension(c) => {
                if c.iter_var2.is_some() {
                    return Err(CheckError::Unsupported("a comprehension over two variables"));
                }
                let range = self.ty(&c.iter_range)?;
                let iter = match &range {
                    Ty::Dyn => Ty::Dyn,
                    Ty::List(e) => (**e).clone(),
                    Ty::Map(k, _) => (**k).clone(),
                    other => return Err(CheckError::NotARange(other.clone())),
                };
                let mut accu = self.ty(&c.accu_init)?;
                self.depth = self.depth.saturating_add(1);
                self.deepest = self.deepest.max(self.depth);
                self.scope.push((c.accu_var.clone(), accu.clone()));
                self.scope.push((c.iter_var.clone(), iter));
                let step = self.ty(&c.loop_step);
                // The accumulator of map and filter starts as an empty list: the step says what it holds.
                if let Ok(step) = &step
                    && accu == Ty::list(Ty::Dyn)
                    && matches!(step, Ty::List(_))
                {
                    accu = step.clone();
                    if let Some(slot) = self.scope.iter_mut().rev().find(|(n, _)| *n == c.accu_var) {
                        slot.1 = accu;
                    }
                }
                let cond = step.and_then(|_| self.ty(&c.loop_cond));
                self.scope.pop();
                let result = cond.and_then(|_| self.ty(&c.result));
                self.scope.pop();
                self.depth = self.depth.saturating_sub(1);
                result
            }
        }
    }

    fn tys(&mut self, es: &[IdedExpr]) -> Result<Vec<Ty>, CheckError> {
        es.iter().map(|e| self.ty(e)).collect()
    }

    fn local(&self, name: &str) -> Option<Ty> {
        self.scope.iter().rev().find(|(n, _)| n == name).map(|(_, t)| t.clone())
    }

    fn ident(&self, name: &str) -> Result<Ty, CheckError> {
        self.local(name)
            .or_else(|| declared(name))
            .ok_or_else(|| CheckError::Undeclared(name.to_owned()))
    }
}

/// One overload: the receiver it is called on (none for a global function), its arguments, and its result.
struct Overload {
    target: Option<Ty>,
    args: Vec<Ty>,
    result: Ty,
}

fn global(args: &[Ty], result: Ty) -> Overload {
    Overload {
        target: None,
        args: args.to_vec(),
        result,
    }
}

fn member(target: Ty, args: &[Ty], result: Ty) -> Overload {
    Overload {
        target: Some(target),
        args: args.to_vec(),
        result,
    }
}

/// The type `function` gives on `target` and `args`: the result of every overload they fit, joined, or an error when none fits.
fn resolve(function: &str, target: Option<&Ty>, args: &[Ty]) -> Result<Ty, CheckError> {
    let no_overload = || {
        let mut all: Vec<Ty> = target.cloned().into_iter().collect();
        all.extend_from_slice(args);
        CheckError::NoOverload {
            function: function.to_owned(),
            args: all,
        }
    };
    if let Some(t) = generic(function, target, args) {
        return t.ok_or_else(no_overload);
    }
    let overloads = overloads(function);
    if overloads.is_empty() {
        return Err(CheckError::Undeclared(function.to_owned()));
    }
    let mut result: Option<Ty> = None;
    for o in overloads.iter().filter(|o| {
        o.args.len() == args.len()
            && match (&o.target, target) {
                (None, None) => true,
                (Some(want), Some(have)) => fits(have, want),
                _ => false,
            }
            && o.args.iter().zip(args).all(|(want, have)| fits(have, want))
    }) {
        result = Some(match result {
            None => o.result.clone(),
            Some(prev) => join_loose(&prev, &o.result),
        });
    }
    result.ok_or_else(no_overload)
}

/// The functions whose signature has a type parameter (`A`), checked by unifying their arguments; `None` for any other
/// function, and `Some(None)` when the arguments do not unify.
fn generic(function: &str, target: Option<&Ty>, args: &[Ty]) -> Option<Option<Ty>> {
    let t = match (function, target, args) {
        ("_==_" | "_!=_", None, [a, b]) => join(a, b).map(|_| Ty::Bool),
        ("_?_:_", None, [c, a, b]) => fits(c, &Ty::Bool).then(|| join(a, b)).flatten(),
        ("@in", None, [a, b]) => match b {
            Ty::Dyn => Some(Ty::Bool),
            Ty::List(e) => join(a, e).map(|_| Ty::Bool),
            Ty::Map(k, _) => join(a, k).map(|_| Ty::Bool),
            _ => None,
        },
        ("_[_]", None, [c, i]) => match c {
            Ty::Dyn => Some(Ty::Dyn),
            Ty::List(e) => fits(i, &Ty::Int).then(|| (**e).clone()),
            Ty::Map(k, v) => fits(i, k).then(|| (**v).clone()),
            _ => None,
        },
        ("_+_", None, [Ty::List(a), Ty::List(b)]) => join(a, b).map(Ty::list),
        ("dyn", None, [_]) => Some(Ty::Dyn),
        ("type", None, [_]) => Some(Ty::Type),
        ("size", None, [c]) | ("size", Some(c), []) => {
            matches!(c, Ty::Dyn | Ty::String | Ty::Bytes | Ty::List(_) | Ty::Map(..)).then_some(Ty::Int)
        }
        _ => return None,
    };
    Some(t)
}

/// The overloads of cel-go's standard library and of its strings extension (`ext.Strings`), as the environment of
/// `condition.rs` declares them; none for a function it does not have.
fn overloads(function: &str) -> Vec<Overload> {
    use Ty::{Bool, Bytes, Double, Duration, Int, String as Str, Timestamp, Uint};
    let numbers = [Int, Uint, Double];
    let ordered = [Bool, Int, Uint, Double, Str, Bytes, Timestamp, Duration];
    let list_of_text = Ty::list(Str);
    match function {
        "!_" => vec![global(&[Bool], Bool)],
        "-_" => vec![global(&[Int], Int), global(&[Double], Double)],
        "_&&_" | "_||_" => vec![global(&[Bool, Bool], Bool)],
        "@not_strictly_false" => vec![global(&[Bool], Bool)],
        "_<_" | "_<=_" | "_>_" | "_>=_" => ordered.iter().map(|t| global(&[t.clone(), t.clone()], Bool)).collect(),
        "_+_" => {
            let mut o: Vec<Overload> = [Int, Uint, Double, Str, Bytes, Duration]
                .iter()
                .map(|t| global(&[t.clone(), t.clone()], t.clone()))
                .collect();
            o.push(global(&[Timestamp, Duration], Timestamp));
            o.push(global(&[Duration, Timestamp], Timestamp));
            o
        }
        "_-_" => {
            let mut o: Vec<Overload> = numbers
                .iter()
                .map(|t| global(&[t.clone(), t.clone()], t.clone()))
                .collect();
            o.push(global(&[Timestamp, Timestamp], Duration));
            o.push(global(&[Timestamp, Duration], Timestamp));
            o.push(global(&[Duration, Duration], Duration));
            o
        }
        "_*_" | "_/_" => numbers
            .iter()
            .map(|t| global(&[t.clone(), t.clone()], t.clone()))
            .collect(),
        "_%_" => vec![global(&[Int, Int], Int), global(&[Uint, Uint], Uint)],
        "int" => [Int, Uint, Double, Str, Timestamp]
            .iter()
            .map(|t| global(std::slice::from_ref(t), Int))
            .collect(),
        "uint" => [Int, Uint, Double, Str]
            .iter()
            .map(|t| global(std::slice::from_ref(t), Uint))
            .collect(),
        "double" => [Int, Uint, Double, Str]
            .iter()
            .map(|t| global(std::slice::from_ref(t), Double))
            .collect(),
        "string" => [Int, Uint, Double, Bool, Str, Bytes, Timestamp, Duration]
            .iter()
            .map(|t| global(std::slice::from_ref(t), Str))
            .collect(),
        "bytes" => vec![global(&[Str], Bytes), global(&[Bytes], Bytes)],
        "bool" => vec![global(&[Bool], Bool), global(&[Str], Bool)],
        "timestamp" => [Str, Int, Timestamp]
            .iter()
            .map(|t| global(std::slice::from_ref(t), Timestamp))
            .collect(),
        "duration" => vec![global(&[Str], Duration), global(&[Duration], Duration)],
        "matches" => vec![global(&[Str, Str], Bool), member(Str, &[Str], Bool)],
        "contains" | "startsWith" | "endsWith" => vec![member(Str, &[Str], Bool)],
        "getFullYear" | "getMonth" | "getDayOfYear" | "getDayOfMonth" | "getDate" | "getDayOfWeek" => {
            vec![member(Timestamp, &[], Int), member(Timestamp, &[Str], Int)]
        }
        "getHours" | "getMinutes" | "getSeconds" | "getMilliseconds" => vec![
            member(Timestamp, &[], Int),
            member(Timestamp, &[Str], Int),
            member(Duration, &[], Int),
        ],
        "charAt" => vec![member(Str, &[Int], Str)],
        "indexOf" | "lastIndexOf" => vec![member(Str, &[Str], Int), member(Str, &[Str, Int], Int)],
        "lowerAscii" | "upperAscii" | "trim" | "reverse" => vec![member(Str, &[], Str)],
        "replace" => vec![member(Str, &[Str, Str], Str), member(Str, &[Str, Str, Int], Str)],
        "split" => vec![
            member(Str, &[Str], list_of_text.clone()),
            member(Str, &[Str, Int], list_of_text),
        ],
        "substring" => vec![member(Str, &[Int], Str), member(Str, &[Int, Int], Str)],
        "join" => vec![member(Ty::list(Str), &[], Str), member(Ty::list(Str), &[Str], Str)],
        "format" => vec![member(Str, &[Ty::list(Ty::Dyn)], Str)],
        "strings.quote" => vec![global(&[Str], Str)],
        _ => Vec::new(),
    }
}
