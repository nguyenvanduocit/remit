//! The body of a `link::contract!` invocation, in the grammar of the macro (`crates/link/src/contract.rs`): the capability,
//! then `reads`, `changes`, `idempotent` and `pushes`, each a list of `name(arg: Type, ...) -> Answer, key = value;`.

use proc_macro2::{Ident, TokenStream};
use syn::ext::IdentExt;
use syn::parse::{ParseStream, Parser};
use syn::{Attribute, LitStr, Token, Type};

/// One call or push as written; a push has no answer.
pub struct Entry {
    pub docs: Vec<Attribute>,
    pub name: Ident,
    pub args: Vec<(Ident, Type)>,
    pub answer: Option<Type>,
}

pub struct Body {
    /// `reads`, `changes` and `idempotent`, in the order written: on the wire they differ only by the `Request` a change
    /// carries, which the core adds.
    pub calls: Vec<Entry>,
    pub pushes: Vec<Entry>,
}

pub fn parse(tokens: TokenStream) -> syn::Result<Body> {
    body.parse2(tokens)
}

fn body(input: ParseStream<'_>) -> syn::Result<Body> {
    let key: Ident = input.parse()?;
    if key != "capability" {
        return Err(syn::Error::new(key.span(), "a contract opens with `capability:`"));
    }
    input.parse::<Token![:]>()?;
    input.parse::<LitStr>()?;
    input.parse::<Token![;]>()?;
    let mut calls = Vec::new();
    for section in ["reads", "changes", "idempotent"] {
        calls.extend(entries(input, section, true)?);
    }
    let pushes = entries(input, "pushes", false)?;
    Ok(Body { calls, pushes })
}

fn entries(input: ParseStream<'_>, section: &str, answers: bool) -> syn::Result<Vec<Entry>> {
    let key: Ident = input.parse()?;
    if key != section {
        return Err(syn::Error::new(key.span(), format!("expected `{section}`")));
    }
    let content;
    syn::braced!(content in input);
    let mut out = Vec::new();
    while !content.is_empty() {
        out.push(entry(&content, answers)?);
    }
    Ok(out)
}

fn entry(input: ParseStream<'_>, answers: bool) -> syn::Result<Entry> {
    let docs = input.call(Attribute::parse_outer)?;
    let name = input.call(Ident::parse_any)?;
    let list;
    syn::parenthesized!(list in input);
    let mut args = Vec::new();
    while !list.is_empty() {
        let arg = list.call(Ident::parse_any)?;
        list.parse::<Token![:]>()?;
        args.push((arg, list.parse::<Type>()?));
        if !list.is_empty() {
            list.parse::<Token![,]>()?;
        }
    }
    let answer = if answers {
        input.parse::<Token![->]>()?;
        Some(input.parse::<Type>()?)
    } else {
        None
    };
    // `why = "..."`, `order = <arg>` and `timeout_ms = N` say how the call is run, not what crosses the wire.
    while input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        if key == "order" {
            input.parse::<Ident>()?;
        } else if key == "why" || key == "timeout_ms" {
            input.parse::<syn::Lit>()?;
        } else {
            return Err(syn::Error::new(
                key.span(),
                "a call's options are `why`, `order` and `timeout_ms`",
            ));
        }
    }
    input.parse::<Token![;]>()?;
    Ok(Entry {
        docs,
        name,
        args,
        answer,
    })
}
