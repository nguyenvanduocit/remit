//! tsgen writes the TypeScript of one module of a crate (ADR-0072 decision 2) from its source: the types the module
//! declares and every type they reach, in that crate or in the crates it is given (`types.generated.ts`), and the
//! module's contracts' calls and pushes (`api.generated.ts`). It reads serde's attributes within a closed set; any other
//! attribute or shape fails with the `file:line` it is at, so nothing crosses the wire in a shape the TypeScript does
//! not say.

use std::collections::{BTreeMap, BTreeSet};

mod attrs;
mod contract;
mod emit;
mod error;
mod lower;
mod rename;
mod source;

pub use error::Error;
pub use source::Source;

use lower::{Crate, Decl, Key, Ty};

/// The files written for a module: its types, and its contracts' calls when it has a contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub types: String,
    pub api: Option<String>,
}

/// Writes the TypeScript of the module `module` (`app`, `runtime` of `wire`) of `krate`; a type it reaches in
/// another crate is read from that crate among `others`.
pub fn generate(krate: &Source, others: &[Source], module: &str) -> Result<Generated, Error> {
    let mut modules = source::modules(krate, true)?;
    for other in others {
        modules.extend(source::modules(other, false)?);
    }
    let krate_name = krate.name.as_str();
    let krate = Crate::index(&modules);
    let root = vec![module.to_owned()];
    if !krate.has_module(&root) {
        return Err(Error::new("lib.rs", 0, format!("the crate has no module `{module}`")));
    }
    let file = modules
        .iter()
        .find(|m| m.path == root)
        .map_or_else(|| format!("{module}.rs"), |m| m.file.clone());

    let contracts = krate.contracts(&root)?;
    let mut decls = BTreeMap::new();
    let mut wanted: Vec<Key> = krate.keys_in(&root);
    for c in &contracts {
        for call in c.calls.iter().chain(&c.pushes) {
            for ty in call.args.iter().map(|a| &a.ty).chain(&call.answer) {
                reached(ty, &mut wanted);
            }
        }
    }
    while let Some(key) = wanted.pop() {
        if decls.contains_key(&key) {
            continue;
        }
        let d = krate.decl(&key)?;
        reached_by(&d, &mut wanted);
        decls.insert(key, d);
    }
    let mut decls: Vec<Decl> = decls.into_values().collect();
    decls.sort_by_key(|d| krate.order(&d.key));

    let mut names = BTreeMap::new();
    for d in &decls {
        if let Some(other) = names.insert(d.name.clone(), &d.key) {
            return Err(Error::new(
                &file,
                0,
                format!(
                    "`{}` names both {}::{} and {}::{}",
                    d.name,
                    other.module.join("::"),
                    other.name,
                    d.key.module.join("::"),
                    d.key.name
                ),
            ));
        }
    }

    let mut prefixes = BTreeSet::new();
    let mut named = Vec::new();
    for c in &contracts {
        let below = c.module.get(root.len()..).unwrap_or_default();
        let prefix: String = below.iter().map(|s| rename::Rule::Pascal.field(s)).collect();
        if !prefixes.insert(prefix.clone()) {
            return Err(Error::new(&file, 0, "two contracts in one module"));
        }
        named.push((prefix, c));
    }

    Ok(Generated {
        types: emit::types(krate_name, &file, &decls),
        api: (!named.is_empty()).then(|| emit::api(krate_name, &file, &named)),
    })
}

fn reached(ty: &Ty, out: &mut Vec<Key>) {
    match ty {
        Ty::Named(_, key) => out.push(key.clone()),
        Ty::Array(inner) | Ty::Nullable(inner) | Ty::Record(inner) => reached(inner, out),
        Ty::String | Ty::Number | Ty::Boolean | Ty::Unknown => {}
    }
}

fn reached_by(d: &Decl, out: &mut Vec<Key>) {
    let object = |o: &lower::Object, out: &mut Vec<Key>| {
        for f in &o.fields {
            reached(&f.ty, out);
        }
        for t in &o.flatten {
            reached(t, out);
        }
    };
    match &d.body {
        lower::Body::Object(o) => object(o, out),
        lower::Body::Alias(t) => reached(t, out),
        lower::Body::Union(_, cases) => {
            for c in cases {
                match &c.data {
                    lower::Data::Unit => {}
                    lower::Data::Newtype(t) => reached(t, out),
                    lower::Data::Struct(o) => object(o, out),
                }
            }
        }
        lower::Body::Strings(_) | lower::Body::Const(_) => {}
    }
}
