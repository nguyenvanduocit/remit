//! Reads a script written for Claude Code's Workflow tool before anything runs (ADR-0075, decision 4): oxc parses it as a
//! module whose top level may await and return; its first statement must be `export const meta = <literal>`, read from
//! the tree without running code. The body QuickJS runs is the script with that `export` blanked to spaces, so positions
//! in errors stay true, wrapped in an async function so it may await and return at the top level.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrayExpressionElement, BindingPattern, Declaration, Expression, ObjectPropertyKind, PropertyKey, PropertyKind,
    Statement, UnaryOperator, VariableDeclarationKind,
};
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;
use serde_json::{Map, Number, Value};

use crate::Meta;

const EXPORT: &str = "export";

/// The script, read: the function QuickJS evaluates and the meta it announced.
pub(crate) struct Compiled {
    pub body: String,
    pub meta: Meta,
}

/// Reads the script's meta and makes the body QuickJS evaluates.
pub(crate) fn compile(script: &str) -> Result<Compiled, String> {
    let missing = || "the script must begin with `export const meta = {...}`".to_owned();
    let alloc = Allocator::default();
    let options = ParseOptions {
        allow_return_outside_function: true,
        ..ParseOptions::default()
    };
    let parsed = Parser::new(&alloc, script, SourceType::mjs())
        .with_options(options)
        .parse();
    if let Some(e) = parsed.diagnostics.errors().next() {
        return Err(format!("read the script: {e}"));
    }
    let program = &parsed.program;
    if program.hashbang.is_some() || !program.directives.is_empty() {
        return Err(missing());
    }
    let Some(Statement::ExportDeclaration(export)) = program.body.first() else {
        return Err(missing());
    };
    let Declaration::VariableDeclaration(decl) = &export.declaration else {
        return Err(missing());
    };
    let Some(first) = decl.declarations.first() else {
        return Err(missing());
    };
    let named_meta = matches!(&first.id, BindingPattern::BindingIdentifier(id) if id.name == "meta");
    if decl.kind != VariableDeclarationKind::Const || !named_meta {
        return Err(missing());
    }
    if decl.declarations.len() != 1 {
        return Err("the script's meta must be the script's first statement: `export const meta = {...}`".to_owned());
    }
    let Some(init) = &first.init else {
        return Err("the script's meta must be the script's first statement: `export const meta = {...}`".to_owned());
    };
    let meta = literal(init)
        .map_err(|why| format!("the script's meta must be a pure literal: {why}"))
        .and_then(meta_of)?;

    let start = usize::try_from(export.span.start).map_err(|e| e.to_string())?;
    let after = start.checked_add(EXPORT.len()).ok_or_else(missing)?;
    let (Some(before), Some(EXPORT), Some(rest)) = (script.get(..start), script.get(start..after), script.get(after..))
    else {
        return Err(missing());
    };
    let body = format!("(async function () {{{before}{}{rest}\n}})", " ".repeat(EXPORT.len()));
    Ok(Compiled { body, meta })
}

/// The meta from its literal, read as JSON into `Meta`.
fn meta_of(v: Value) -> Result<Meta, String> {
    let meta: Meta =
        serde_json::from_value(v).map_err(|e| format!("the script's meta has a field of the wrong type: {e}"))?;
    if meta.name.is_empty() || meta.description.is_empty() {
        return Err("the script's meta needs a name and a description".to_owned());
    }
    Ok(meta)
}

/// A literal expression as a JSON value, refusing anything that would run code.
fn literal(x: &Expression<'_>) -> Result<Value, String> {
    match x {
        Expression::StringLiteral(s) => text(s.value.as_str()).map(Value::String),
        Expression::NumericLiteral(n) => number(n.value),
        Expression::BooleanLiteral(b) => Ok(Value::Bool(b.value)),
        Expression::NullLiteral(_) => Ok(Value::Null),
        Expression::TemplateLiteral(t) => match (t.expressions.is_empty(), t.quasis.as_slice()) {
            (true, [q]) => q
                .value
                .cooked
                .as_ref()
                .ok_or_else(|| "a template's escape is not valid".to_owned())
                .and_then(|c| text(c.as_str()))
                .map(Value::String),
            _ => Err("a template may not interpolate".to_owned()),
        },
        Expression::UnaryExpression(u) => match (&u.operator, &u.argument) {
            (UnaryOperator::UnaryNegation, Expression::NumericLiteral(n)) => number(-n.value),
            _ => Err("an operator is not a literal".to_owned()),
        },
        Expression::ArrayExpression(a) => a
            .elements
            .iter()
            .map(|el| match el {
                ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => {
                    Err("an array may not spread or leave a hole".to_owned())
                }
                el => el
                    .as_expression()
                    .ok_or_else(|| "an array item is not a literal".to_owned())
                    .and_then(literal),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Expression::ObjectExpression(o) => {
            let mut out = Map::new();
            for p in &o.properties {
                let ObjectPropertyKind::ObjectProperty(p) = p else {
                    return Err("each property must be `key: value`".to_owned());
                };
                if p.computed || p.method || p.shorthand || p.kind != PropertyKind::Init {
                    return Err("each property must be `key: value`".to_owned());
                }
                let key = match &p.key {
                    PropertyKey::StaticIdentifier(i) => i.name.to_string(),
                    PropertyKey::StringLiteral(s) => text(s.value.as_str())?,
                    _ => return Err("a key must be a name or a string".to_owned()),
                };
                out.insert(key, literal(&p.value)?);
            }
            Ok(Value::Object(out))
        }
        Expression::ParenthesizedExpression(p) => literal(&p.expression),
        _ => Err("it runs code".to_owned()),
    }
}

fn number(n: f64) -> Result<Value, String> {
    Number::from_f64(n)
        .map(Value::Number)
        .ok_or_else(|| "a number JSON cannot hold".to_owned())
}

/// A string's value; none when it holds a lone surrogate, which no JSON text can.
fn text(s: Option<&str>) -> Result<String, String> {
    s.map(str::to_owned)
        .ok_or_else(|| "a string holds a lone surrogate".to_owned())
}
