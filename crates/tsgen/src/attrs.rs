//! The attributes of an item, a field or a variant, within the closed set the generator writes (ADR-0072): serde's
//! `rename`, `rename_all`, `rename_all_fields`, `tag`, `content`, `untagged`, `into` with `from` or `try_from`,
//! `skip_serializing_if = "Option::is_none"`, `skip`, `default` and `flatten`, and `remote = "Self"` on an enum
//! `json_tagged!` tags or a struct `json_paired!` pairs (ADR-0095), besides docs, `derive`, a variant's `#[default]`,
//! `expect` and `schemars` (which shapes only an agent's JSON Schema). Anything else fails, so an unwritten shape cannot
//! pass.

use syn::spanned::Spanned;
use syn::{Attribute, Expr, Lit, LitStr, Meta};

use crate::error::Error;
use crate::rename::Rule;

#[derive(Default)]
pub struct Container {
    pub rename: Option<String>,
    pub rename_all: Option<Rule>,
    pub rename_all_fields: Option<Rule>,
    pub tag: Option<String>,
    pub content: Option<String>,
    pub untagged: bool,
    /// `remote = "Self"`: the type's `Serialize` and `Deserialize` are a macro's, `json_tagged!`'s or `json_paired!`'s.
    pub remote: bool,
    /// `into = "T"`: the type is written as `T`.
    pub into: Option<LitStr>,
    /// `from = "T"` or `try_from = "T"`: the type is read as `T`.
    pub from: Option<LitStr>,
}

#[derive(Debug, Default)]
pub struct Field {
    pub rename: Option<String>,
    pub skip_if_none: bool,
    pub flatten: bool,
    /// `skip`: never written, nor read.
    pub skip: bool,
}

#[derive(Debug, Default)]
pub struct Variant {
    pub rename: Option<String>,
}

/// The `///` lines, each without the one space after the slashes.
pub fn docs(attrs: &[Attribute]) -> Vec<String> {
    let mut lines = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        if let Meta::NameValue(nv) = &attr.meta
            && let Expr::Lit(lit) = &nv.value
            && let Lit::Str(s) = &lit.lit
        {
            let text = s.value();
            for line in text.lines() {
                lines.push(line.strip_prefix(' ').unwrap_or(line).trim_end().to_owned());
            }
        }
    }
    lines
}

pub fn container(file: &str, attrs: &[Attribute]) -> Result<Container, Error> {
    let mut c = Container::default();
    serde(file, attrs, |meta| {
        if meta.path.is_ident("rename") {
            c.rename = Some(string(&meta)?);
        } else if meta.path.is_ident("rename_all") {
            c.rename_all = Some(rule(&meta)?);
        } else if meta.path.is_ident("rename_all_fields") {
            c.rename_all_fields = Some(rule(&meta)?);
        } else if meta.path.is_ident("tag") {
            c.tag = Some(string(&meta)?);
        } else if meta.path.is_ident("content") {
            c.content = Some(string(&meta)?);
        } else if meta.path.is_ident("untagged") {
            c.untagged = true;
        } else if meta.path.is_ident("remote") {
            let remote = meta.value()?.parse::<LitStr>()?;
            if remote.value() != "Self" {
                return Err(syn::Error::new(remote.span(), "the one `remote` written is \"Self\""));
            }
            c.remote = true;
        } else if meta.path.is_ident("default") {
            // As on a field: only what is read changes, and what the server writes keeps every field.
            if meta.input.peek(syn::Token![=]) {
                meta.value()?.parse::<LitStr>()?;
            }
        } else if meta.path.is_ident("into") {
            c.into = Some(meta.value()?.parse::<LitStr>()?);
        } else if meta.path.is_ident("from") || meta.path.is_ident("try_from") {
            if c.from.is_some() {
                return Err(meta.error("`from` and `try_from` both say how the type is read"));
            }
            c.from = Some(meta.value()?.parse::<LitStr>()?);
        } else {
            return Err(unsupported(&meta));
        }
        Ok(())
    })?;
    Ok(c)
}

pub fn field(file: &str, attrs: &[Attribute]) -> Result<Field, Error> {
    let mut f = Field::default();
    serde(file, attrs, |meta| {
        if meta.path.is_ident("rename") {
            f.rename = Some(string(&meta)?);
        } else if meta.path.is_ident("skip_serializing_if") {
            let path = meta.value()?.parse::<LitStr>()?;
            if path.value() != "Option::is_none" {
                return Err(syn::Error::new(
                    path.span(),
                    "the one `skip_serializing_if` written is \"Option::is_none\"",
                ));
            }
            f.skip_if_none = true;
        } else if meta.path.is_ident("default") {
            // Only what is read changes; what the server writes keeps the field.
            if meta.input.peek(syn::Token![=]) {
                meta.value()?.parse::<LitStr>()?;
            }
        } else if meta.path.is_ident("flatten") {
            f.flatten = true;
        } else if meta.path.is_ident("skip") {
            f.skip = true;
        } else {
            return Err(unsupported(&meta));
        }
        Ok(())
    })?;
    Ok(f)
}

pub fn variant(file: &str, attrs: &[Attribute]) -> Result<Variant, Error> {
    let mut v = Variant::default();
    serde(file, attrs, |meta| {
        if meta.path.is_ident("rename") {
            v.rename = Some(string(&meta)?);
        } else {
            return Err(unsupported(&meta));
        }
        Ok(())
    })?;
    Ok(v)
}

fn serde(
    file: &str,
    attrs: &[Attribute],
    mut each: impl FnMut(syn::meta::ParseNestedMeta<'_>) -> syn::Result<()>,
) -> Result<(), Error> {
    for attr in attrs {
        let path = attr.path();
        if path.is_ident("serde") {
            attr.parse_nested_meta(&mut each).map_err(|e| Error::syn(file, &e))?;
        } else if !(path.is_ident("doc")
            || path.is_ident("default")
            || path.is_ident("derive")
            || path.is_ident("expect")
            || path.is_ident("schemars"))
        {
            let name = quote::ToTokens::to_token_stream(path).to_string();
            return Err(Error::at(
                file,
                attr.span(),
                format!("tsgen does not write the attribute `{name}`"),
            ));
        }
    }
    Ok(())
}

fn string(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<String> {
    Ok(meta.value()?.parse::<LitStr>()?.value())
}

fn rule(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Rule> {
    let lit = meta.value()?.parse::<LitStr>()?;
    Rule::parse(&lit.value()).ok_or_else(|| syn::Error::new(lit.span(), "not a serde rename rule"))
}

fn unsupported(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Error {
    let name = quote::ToTokens::to_token_stream(&meta.path).to_string();
    meta.error(format!("tsgen does not write `serde({name})`"))
}
