//! A crate's items, and those of the crates it reaches, as the shapes their JSON has, with every type name resolved. An
//! item is lowered only when the module asked for, or something it reaches, needs it: a module no TypeScript is written
//! for may hold any shape.

use std::collections::BTreeMap;

use proc_macro2::Span;
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::{Expr, Fields, GenericArgument, Item, Lit, PathArguments, Type, UnOp, UseTree, Visibility};

use crate::attrs;
use crate::contract;
use crate::error::Error;
use crate::rename::{self, Rule};
use crate::source::Module;

/// An item: the module it is declared in (below `["::", name]` in a crate reached) and its Rust name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub module: Vec<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    String,
    Number,
    Boolean,
    Unknown,
    /// A declared type, by its TypeScript name.
    Named(String, Key),
    Array(Box<Ty>),
    Nullable(Box<Ty>),
    /// A map with string keys.
    Record(Box<Ty>),
}

pub struct Decl {
    pub key: Key,
    pub name: String,
    pub docs: Vec<String>,
    pub body: Body,
}

pub enum Body {
    Object(Object),
    Alias(Ty),
    /// An enum of unit variants, each written as its name.
    Strings(Vec<Case>),
    Union(Repr, Vec<Case>),
    /// A constant's TypeScript literal.
    Const(String),
}

pub struct Object {
    pub fields: Vec<Field>,
    /// The `flatten`ed fields' types, whose fields the object takes as its own.
    pub flatten: Vec<Ty>,
}

pub struct Field {
    pub name: String,
    pub docs: Vec<String>,
    pub ty: Ty,
    /// Left out when absent (`skip_serializing_if = "Option::is_none"`), rather than written as null.
    pub optional: bool,
}

pub enum Repr {
    Internal {
        tag: String,
    },
    Adjacent {
        tag: String,
        content: String,
    },
    Untagged,
    /// Two keys of the struct that holds it (`json_paired!`): the variant in `tag`, the text it holds in `content`, empty
    /// for a variant that holds none.
    Paired {
        tag: String,
        content: String,
    },
}

pub struct Case {
    pub name: String,
    pub docs: Vec<String>,
    pub data: Data,
}

pub enum Data {
    Unit,
    Newtype(Ty),
    Struct(Object),
}

pub struct Contract {
    pub module: Vec<String>,
    pub calls: Vec<Call>,
    pub pushes: Vec<Call>,
}

/// A call, or a push when `answer` is none.
pub struct Call {
    /// The name on the wire.
    pub method: String,
    pub docs: Vec<String>,
    pub args: Vec<Arg>,
    pub answer: Option<Ty>,
}

pub struct Arg {
    /// The name on the wire and in TypeScript: the macro writes the arguments `camelCase`.
    pub name: String,
    pub ty: Ty,
}

enum Target {
    Item(Key),
    Builtin(Builtin),
}

#[derive(Clone, Copy)]
enum Builtin {
    String,
    Number,
    Boolean,
    Value,
    Vec,
    Option,
    Box,
    Map,
}

struct Declared<'a> {
    module: &'a Module,
    shape: Shape<'a>,
    order: usize,
}

/// What declares an item.
enum Shape<'a> {
    /// An item as Rust writes it.
    Item(&'a Item),
    /// A `text_enum!`, read as the enum its body is: each value `Name = "text"`, written as its text.
    TextEnum(Box<syn::ItemEnum>),
}

/// Every module and declared item of the crate written and the crates it reaches, and what each module's `use` items
/// name.
pub struct Crate<'a> {
    modules: BTreeMap<Vec<String>, &'a Module>,
    items: BTreeMap<Key, Declared<'a>>,
    uses: BTreeMap<Key, Vec<String>>,
    /// The paths each module's glob `use` items bring every item of in from (`pub use account::*` is `["account"]`).
    globs: BTreeMap<Vec<String>, Vec<Vec<String>>>,
    /// The field `json_tagged!(Enum, "field")` puts each enum's variant in.
    tags: BTreeMap<Key, String>,
    /// The struct field `json_paired!(Struct, field, "tag", "content")` writes as two keys, by struct.
    pairs: BTreeMap<Key, Pair>,
    /// Where a `Serialize` or `Deserialize` is written by hand, by the type it is for.
    hand_serde: BTreeMap<Key, Span>,
}

/// A struct's enum field written as two of the struct's keys (`json_paired!`).
#[derive(Clone)]
struct Pair {
    field: String,
    tag: String,
    content: String,
}

/// The words a TypeScript parameter cannot be named, as `createApi` names each after its argument.
const RESERVED: &[&str] = &[
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// How many `use` items a name may pass through before it is taken for a cycle.
const USE_DEPTH: usize = 16;

impl<'a> Crate<'a> {
    pub fn index(modules: &'a [Module]) -> Self {
        let mut krate = Crate {
            modules: BTreeMap::new(),
            items: BTreeMap::new(),
            uses: BTreeMap::new(),
            globs: BTreeMap::new(),
            tags: BTreeMap::new(),
            pairs: BTreeMap::new(),
            hand_serde: BTreeMap::new(),
        };
        for module in modules {
            krate.modules.insert(module.path.clone(), module);
            for item in &module.items {
                let (name, shape) = match item {
                    Item::Struct(i) if public(&i.vis) => (i.ident.unraw(), Shape::Item(item)),
                    Item::Enum(i) if public(&i.vis) => (i.ident.unraw(), Shape::Item(item)),
                    Item::Type(i) if public(&i.vis) => (i.ident.unraw(), Shape::Item(item)),
                    Item::Const(i) if public(&i.vis) => (i.ident.unraw(), Shape::Item(item)),
                    // A body that does not parse declares nothing here: `contracts` says why, and a path to it fails.
                    Item::Macro(m) if text_enum(m) => match m.mac.parse_body::<syn::ItemEnum>() {
                        Ok(e) if public(&e.vis) => (e.ident.unraw(), Shape::TextEnum(Box::new(e))),
                        _ => continue,
                    },
                    Item::Macro(m) if json_tagged(m) => {
                        if let Ok((name, tag)) = m.mac.parse_body_with(tagged) {
                            krate.tags.insert(
                                Key {
                                    module: module.path.clone(),
                                    name,
                                },
                                tag,
                            );
                        }
                        continue;
                    }
                    Item::Macro(m) if json_paired(m) => {
                        if let Ok((name, pair)) = m.mac.parse_body_with(paired) {
                            krate.pairs.insert(
                                Key {
                                    module: module.path.clone(),
                                    name,
                                },
                                pair,
                            );
                        }
                        continue;
                    }
                    Item::Impl(i) if serde_impl(i) => {
                        if let Type::Path(p) = &*i.self_ty
                            && let Some(last) = p.path.segments.last()
                        {
                            krate.hand_serde.insert(
                                Key {
                                    module: module.path.clone(),
                                    name: last.ident.unraw().to_string(),
                                },
                                i.span(),
                            );
                        }
                        continue;
                    }
                    Item::Use(u) => {
                        let mut prefix = Vec::new();
                        if u.leading_colon.is_some() {
                            prefix.push("::".to_owned());
                        }
                        uses(&module.path, &u.tree, prefix, &mut krate.uses, &mut krate.globs);
                        continue;
                    }
                    _ => continue,
                };
                let order = krate.items.len();
                krate.items.insert(
                    Key {
                        module: module.path.clone(),
                        name: name.to_string(),
                    },
                    Declared { module, shape, order },
                );
            }
        }
        krate
    }

    pub fn has_module(&self, path: &[String]) -> bool {
        self.modules.contains_key(path)
    }

    /// The items declared in `prefix` and the modules below it, in the order written.
    pub fn keys_in(&self, prefix: &[String]) -> Vec<Key> {
        let mut keys: Vec<(&Key, usize)> = self
            .items
            .iter()
            .filter(|(k, _)| k.module.starts_with(prefix))
            .map(|(k, d)| (k, d.order))
            .collect();
        keys.sort_by_key(|(_, order)| *order);
        keys.into_iter().map(|(k, _)| k.clone()).collect()
    }

    pub fn order(&self, key: &Key) -> usize {
        self.items.get(key).map_or(usize::MAX, |d| d.order)
    }

    /// The contracts of `prefix` and the modules below it; any item there whose shape tsgen cannot see fails.
    pub fn contracts(&self, prefix: &[String]) -> Result<Vec<Contract>, Error> {
        let mut out = Vec::new();
        for (path, module) in self.modules.range(prefix.to_vec()..) {
            if !path.starts_with(prefix) {
                break;
            }
            if let Some(why) = &module.unread {
                return Err(why.clone());
            }
            for item in &module.items {
                match item {
                    Item::Macro(m) if m.mac.path.segments.last().is_some_and(|s| s.ident == "contract") => {
                        out.push(self.contract(module, m)?);
                    }
                    Item::Macro(m) if json_tagged(m) => {
                        m.mac
                            .parse_body_with(tagged)
                            .map_err(|e| Error::syn(&module.file, &e))?;
                    }
                    Item::Macro(m) if json_paired(m) => {
                        m.mac
                            .parse_body_with(paired)
                            .map_err(|e| Error::syn(&module.file, &e))?;
                    }
                    Item::Macro(m) if text_enum(m) => {
                        m.mac
                            .parse_body::<syn::ItemEnum>()
                            .map_err(|e| Error::syn(&module.file, &e))?;
                    }
                    Item::Macro(m) => {
                        return Err(Error::at(
                            &module.file,
                            m.span(),
                            "a macro other than `contract!`, `json_tagged!`, `json_paired!` and `text_enum!` may declare \
                             what tsgen cannot see",
                        ));
                    }
                    _ => {}
                }
            }
        }
        Ok(out)
    }

    fn contract(&self, module: &Module, m: &syn::ItemMacro) -> Result<Contract, Error> {
        let body = contract::parse(m.mac.tokens.clone()).map_err(|e| Error::syn(&module.file, &e))?;
        let lower = |entries: Vec<contract::Entry>| -> Result<Vec<Call>, Error> {
            entries
                .into_iter()
                .map(|e| {
                    let args = e
                        .args
                        .iter()
                        .map(|(name, ty)| {
                            let wire = Rule::Camel.field(&name.unraw().to_string());
                            if RESERVED.contains(&wire.as_str()) {
                                return Err(Error::at(
                                    &module.file,
                                    name.span(),
                                    format!("`{wire}` is a reserved word in TypeScript, so no parameter can take it"),
                                ));
                            }
                            Ok(Arg {
                                name: wire,
                                ty: self.ty(module, ty)?,
                            })
                        })
                        .collect::<Result<_, Error>>()?;
                    Ok(Call {
                        method: e.name.unraw().to_string(),
                        docs: attrs::docs(&e.docs),
                        args,
                        answer: e.answer.as_ref().map(|t| self.ty(module, t)).transpose()?,
                    })
                })
                .collect()
        };
        Ok(Contract {
            module: module.path.clone(),
            calls: lower(body.calls)?,
            pushes: lower(body.pushes)?,
        })
    }

    /// One declared item, lowered.
    pub fn decl(&self, key: &Key) -> Result<Decl, Error> {
        let d = self
            .items
            .get(key)
            .ok_or_else(|| Error::new("lib.rs", 0, format!("no item {}", key.name)))?;
        let module = d.module;
        if let Some(why) = &module.unread {
            return Err(why.clone());
        }
        let file = module.file.as_str();
        let item = match &d.shape {
            Shape::Item(item) => *item,
            Shape::TextEnum(e) => return text_enum_decl(key, file, e),
        };
        if let Some(span) = self.hand_serde.get(key) {
            return self.hand_written(key, module, item, *span);
        }
        let (attrs, generics, ident) = match item {
            Item::Struct(i) => (&i.attrs, Some(&i.generics), &i.ident),
            Item::Enum(i) => (&i.attrs, Some(&i.generics), &i.ident),
            Item::Type(i) => (&i.attrs, Some(&i.generics), &i.ident),
            Item::Const(i) => (&i.attrs, None, &i.ident),
            _ => return Err(Error::at(file, item.span(), "not a type")),
        };
        if let Some(g) = generics
            && (!g.params.is_empty() || g.where_clause.is_some())
        {
            return Err(Error::at(file, g.span(), "a generic type is not written"));
        }
        let mut c = attrs::container(file, attrs)?;
        let pair = self.pairs.get(key);
        if c.remote {
            if c.tag.is_some() || c.content.is_some() || c.untagged {
                return Err(Error::at(
                    file,
                    ident.span(),
                    "a type `json_tagged!` tags or `json_paired!` pairs takes no `tag`, `content` or `untagged`",
                ));
            }
            match (self.tags.get(key), pair, item) {
                (Some(tag), None, Item::Enum(_)) => c.tag = Some(tag.clone()),
                (None, Some(_), Item::Struct(_)) => {}
                _ => {
                    return Err(Error::at(
                        file,
                        ident.span(),
                        "`remote = \"Self\"` goes with `json_tagged!(Enum, \"field\")` on an enum or \
                         `json_paired!(Struct, field, \"tag\", \"content\")` on a struct",
                    ));
                }
            }
        } else if pair.is_some() {
            return Err(Error::at(
                file,
                ident.span(),
                "a struct `json_paired!` pairs derives its serde with `remote = \"Self\"`",
            ));
        }
        let only_enum = c.rename_all_fields.is_some() || c.tag.is_some() || c.content.is_some() || c.untagged;
        let body = match (&c.into, &c.from, item) {
            (Some(into), Some(from), Item::Struct(_) | Item::Enum(_)) => {
                if into.value() != from.value() {
                    return Err(Error::at(
                        file,
                        from.span(),
                        "`into` and `from` (or `try_from`) name one type, so the TypeScript reads what it writes",
                    ));
                }
                if only_enum || c.rename_all.is_some() || c.remote {
                    return Err(Error::at(
                        file,
                        ident.span(),
                        "a type written as another takes no attribute that shapes its own fields or variants",
                    ));
                }
                let ty: Type = into.parse().map_err(|e| Error::syn(file, &e))?;
                Body::Alias(self.ty(module, &ty)?)
            }
            (Some(_), None, _) | (None, Some(_), _) => {
                return Err(Error::at(
                    file,
                    ident.span(),
                    "`into` goes with `from` or `try_from`: the one type the value is written and read as",
                ));
            }
            (None, None, Item::Struct(i)) => {
                if only_enum {
                    return Err(Error::at(
                        file,
                        i.ident.span(),
                        "`rename_all_fields`, `tag`, `content` and `untagged` belong on an enum",
                    ));
                }
                match &i.fields {
                    Fields::Named(named) => Body::Object(self.object(
                        module,
                        named,
                        c.rename_all.unwrap_or(Rule::None),
                        pair.map(|p| p.field.as_str()),
                    )?),
                    Fields::Unnamed(u) => Body::Alias(self.newtype(module, u)?),
                    other => return Err(Error::at(file, other.span(), "a unit struct is not written")),
                }
            }
            (None, None, Item::Enum(i)) => self.enumeration(module, key, i, &c)?,
            (None, None, Item::Type(i)) => Body::Alias(self.ty(module, &i.ty)?),
            (None, None, Item::Const(i)) => Body::Const(constant(file, &i.expr)?),
            _ => return Err(Error::at(file, item.span(), "not a type")),
        };
        let name = match item {
            Item::Const(_) => rename::constant(&ident.unraw().to_string()),
            _ => c.rename.unwrap_or_else(|| ident.unraw().to_string()),
        };
        Ok(Decl {
            key: key.clone(),
            name,
            docs: attrs::docs(attrs),
            body,
        })
    }

    fn enumeration(&self, module: &Module, key: &Key, e: &syn::ItemEnum, c: &attrs::Container) -> Result<Body, Error> {
        let file = module.file.as_str();
        let paired = self.paired_by(key)?;
        let rule = c.rename_all.unwrap_or(Rule::None);
        let field_rule = c.rename_all_fields.unwrap_or(Rule::None);
        let mut cases = Vec::new();
        for v in &e.variants {
            if let Some((_, d)) = &v.discriminant {
                return Err(Error::at(
                    file,
                    d.span(),
                    "a variant's discriminant does not cross the wire",
                ));
            }
            let va = attrs::variant(file, &v.attrs)?;
            let data = match &v.fields {
                Fields::Unit => Data::Unit,
                Fields::Unnamed(u) => Data::Newtype(self.newtype(module, u)?),
                Fields::Named(_) if paired.is_some() => {
                    return Err(Error::at(
                        file,
                        v.span(),
                        "a variant of an enum `json_paired!` pairs holds one text or nothing",
                    ));
                }
                Fields::Named(named) => Data::Struct(self.object(module, named, field_rule, None)?),
            };
            cases.push(Case {
                name: va.rename.unwrap_or_else(|| rule.variant(&v.ident.unraw().to_string())),
                docs: attrs::docs(&v.attrs),
                data,
            });
        }
        if let Some(pair) = paired {
            if c.tag.is_some() || c.content.is_some() || c.untagged || c.remote {
                return Err(Error::at(
                    file,
                    e.ident.span(),
                    "an enum `json_paired!` pairs is externally tagged, as postcard reads it",
                ));
            }
            return Ok(Body::Union(
                Repr::Paired {
                    tag: pair.tag,
                    content: pair.content,
                },
                cases,
            ));
        }
        let repr = match (&c.tag, &c.content, c.untagged) {
            (None, None, false) => {
                if cases.iter().all(|k| matches!(k.data, Data::Unit)) {
                    return Ok(Body::Strings(cases));
                }
                return Err(Error::at(
                    file,
                    e.ident.span(),
                    "an enum whose variants carry data takes `tag` (or `tag` and `content`, or `untagged`)",
                ));
            }
            (Some(tag), None, false) => Repr::Internal { tag: tag.clone() },
            (Some(tag), Some(content), false) => Repr::Adjacent {
                tag: tag.clone(),
                content: content.clone(),
            },
            (None, None, true) => Repr::Untagged,
            _ => {
                return Err(Error::at(
                    file,
                    e.ident.span(),
                    "`content` goes with `tag`, and `untagged` with neither",
                ));
            }
        };
        Ok(Body::Union(repr, cases))
    }

    /// A type whose `Serialize` or `Deserialize` is written by hand: its JSON is not its Rust shape, so it is refused,
    /// unless it is a newtype over JSON another system defines, which is any JSON whatever form it takes on the link.
    fn hand_written(&self, key: &Key, module: &Module, item: &Item, span: Span) -> Result<Decl, Error> {
        if let Item::Struct(i) = item
            && let Fields::Unnamed(u) = &i.fields
            && let Ok(Ty::Unknown) = self.newtype(module, u)
        {
            return Ok(Decl {
                key: key.clone(),
                name: key.name.clone(),
                docs: attrs::docs(&i.attrs),
                body: Body::Alias(Ty::Unknown),
            });
        }
        Err(Error::at(
            &module.file,
            span,
            format!(
                "a hand-written Serialize or Deserialize of `{}` has a shape tsgen cannot see",
                key.name
            ),
        ))
    }

    /// The keys an enum is written in when it is the field `json_paired!` pairs on a struct; none for any other enum.
    fn paired_by(&self, key: &Key) -> Result<Option<Pair>, Error> {
        for (holder, pair) in &self.pairs {
            let Some(Declared {
                module,
                shape: Shape::Item(Item::Struct(s)),
                ..
            }) = self.items.get(holder)
            else {
                continue;
            };
            let Fields::Named(named) = &s.fields else { continue };
            let field = named
                .named
                .iter()
                .find(|f| f.ident.as_ref().is_some_and(|i| i.unraw() == pair.field.as_str()));
            let Some(field) = field else {
                return Err(Error::at(
                    &module.file,
                    s.ident.span(),
                    format!("`json_paired!` names no field `{}` of `{}`", pair.field, holder.name),
                ));
            };
            if let Ty::Named(_, k) = self.ty(module, &field.ty)?
                && &k == key
            {
                return Ok(Some(pair.clone()));
            }
        }
        Ok(None)
    }

    /// A newtype is written as the one type it holds.
    fn newtype(&self, module: &Module, u: &syn::FieldsUnnamed) -> Result<Ty, Error> {
        let mut fields = u.unnamed.iter();
        match (fields.next(), fields.next()) {
            (Some(f), None) => {
                let fa = attrs::field(&module.file, &f.attrs)?;
                if fa.rename.is_some() || fa.skip_if_none || fa.flatten || fa.skip {
                    return Err(Error::at(
                        &module.file,
                        f.span(),
                        "a newtype's field takes no serde attribute",
                    ));
                }
                self.ty(module, &f.ty)
            }
            _ => Err(Error::at(
                &module.file,
                u.span(),
                "a tuple of more than one field is not written",
            )),
        }
    }

    /// A struct's or a variant's named fields; `paired` is the field `json_paired!` writes as two keys, which the object
    /// takes as its enum's union.
    fn object(
        &self,
        module: &Module,
        named: &syn::FieldsNamed,
        rule: Rule,
        paired: Option<&str>,
    ) -> Result<Object, Error> {
        let file = module.file.as_str();
        let mut object = Object {
            fields: Vec::new(),
            flatten: Vec::new(),
        };
        for f in &named.named {
            let fa = attrs::field(file, &f.attrs)?;
            if fa.skip {
                if fa.rename.is_some() || fa.skip_if_none || fa.flatten {
                    return Err(Error::at(file, f.span(), "a field serde skips takes nothing else"));
                }
                continue;
            }
            let ty = self.ty(module, &f.ty)?;
            let ident = f.ident.as_ref().map(|i| i.unraw().to_string()).unwrap_or_default();
            if paired == Some(ident.as_str()) {
                if fa.rename.is_some() || fa.skip_if_none || fa.flatten || !matches!(ty, Ty::Named(..)) {
                    return Err(Error::at(
                        file,
                        f.span(),
                        "the field `json_paired!` pairs is an enum, as it is",
                    ));
                }
                object.flatten.push(ty);
                continue;
            }
            if fa.flatten {
                if fa.rename.is_some() || fa.skip_if_none || !matches!(ty, Ty::Named(..) | Ty::Record(_)) {
                    return Err(Error::at(
                        file,
                        f.span(),
                        "a flattened field is a struct, an enum or a map, as it is",
                    ));
                }
                object.flatten.push(ty);
                continue;
            }
            let (ty, optional) = match (fa.skip_if_none, ty) {
                (true, Ty::Nullable(inner)) => (*inner, true),
                (true, _) => {
                    return Err(Error::at(
                        file,
                        f.span(),
                        "`skip_serializing_if = \"Option::is_none\"` on a field that is not an Option",
                    ));
                }
                (false, ty) => (ty, false),
            };
            object.fields.push(Field {
                name: fa.rename.unwrap_or_else(|| rule.field(&ident)),
                docs: attrs::docs(&f.attrs),
                ty,
                optional,
            });
        }
        Ok(object)
    }

    fn ty(&self, module: &Module, ty: &Type) -> Result<Ty, Error> {
        let file = module.file.as_str();
        let path = match ty {
            Type::Paren(p) => return self.ty(module, &p.elem),
            Type::Group(g) => return self.ty(module, &g.elem),
            Type::Path(p) if p.qself.is_none() => &p.path,
            other => {
                let text = quote::ToTokens::to_token_stream(other).to_string();
                return Err(Error::at(
                    file,
                    other.span(),
                    format!("the type `{text}` is not written"),
                ));
            }
        };
        let text = quote::ToTokens::to_token_stream(path).to_string();
        let mut segs: Vec<String> = path.segments.iter().map(|s| s.ident.unraw().to_string()).collect();
        if path.leading_colon.is_some() {
            segs.insert(0, "::".to_owned());
        }
        let mut args = Vec::new();
        for (i, seg) in path.segments.iter().enumerate() {
            match &seg.arguments {
                PathArguments::None => {}
                PathArguments::AngleBracketed(a) if i.checked_add(1) == Some(path.segments.len()) => {
                    for arg in &a.args {
                        let GenericArgument::Type(t) = arg else {
                            return Err(Error::at(
                                file,
                                arg.span(),
                                "only types go between a type's angle brackets",
                            ));
                        };
                        args.push(t);
                    }
                }
                other => {
                    return Err(Error::at(
                        file,
                        other.span(),
                        format!("the type `{text}` is not written"),
                    ));
                }
            }
        }
        let target = self
            .resolve(&module.path, &segs, 0)
            .ok_or_else(|| Error::at(file, path.span(), format!("tsgen cannot find `{text}`")))?;
        Ok(match (target, args.as_slice()) {
            (Target::Item(key), []) => Ty::Named(self.name(&key)?, key),
            (Target::Builtin(Builtin::String), []) => Ty::String,
            (Target::Builtin(Builtin::Number), []) => Ty::Number,
            (Target::Builtin(Builtin::Boolean), []) => Ty::Boolean,
            (Target::Builtin(Builtin::Value), []) => Ty::Unknown,
            (Target::Builtin(Builtin::Box), [t]) => self.ty(module, t)?,
            (Target::Builtin(Builtin::Vec), [t]) => Ty::Array(Box::new(self.ty(module, t)?)),
            (Target::Builtin(Builtin::Option), [t]) => match self.ty(module, t)? {
                Ty::Nullable(_) => {
                    return Err(Error::at(
                        file,
                        path.span(),
                        "an Option of an Option cannot be told apart in JSON",
                    ));
                }
                inner => Ty::Nullable(Box::new(inner)),
            },
            (Target::Builtin(Builtin::Map), [k, v]) => {
                if self.ty(module, k)? != Ty::String {
                    return Err(Error::at(file, k.span(), "a map's keys are Strings"));
                }
                Ty::Record(Box::new(self.ty(module, v)?))
            }
            _ => {
                return Err(Error::at(
                    file,
                    path.span(),
                    format!("`{text}` does not take these type arguments"),
                ));
            }
        })
    }

    /// A declared type's TypeScript name: its serde `rename`, else its own.
    fn name(&self, key: &Key) -> Result<String, Error> {
        let d = self
            .items
            .get(key)
            .ok_or_else(|| Error::new("lib.rs", 0, format!("no item {}", key.name)))?;
        let attrs = match &d.shape {
            Shape::TextEnum(_) => return Ok(key.name.clone()),
            Shape::Item(Item::Struct(i)) => &i.attrs,
            Shape::Item(Item::Enum(i)) => &i.attrs,
            Shape::Item(Item::Type(i)) => &i.attrs,
            Shape::Item(other) => {
                return Err(Error::at(
                    &d.module.file,
                    other.span(),
                    format!("`{}` is not a type", key.name),
                ));
            }
        };
        Ok(attrs::container(&d.module.file, attrs)?
            .rename
            .unwrap_or_else(|| key.name.clone()))
    }

    /// What a path names from `module`: an item of the crate, or a type tsgen knows from outside it.
    fn resolve(&self, module: &[String], segs: &[String], depth: usize) -> Option<Target> {
        if depth > USE_DEPTH {
            return None;
        }
        let next = depth.saturating_add(1);
        match segs {
            [] => None,
            [name] => {
                let key = Key {
                    module: module.to_vec(),
                    name: name.clone(),
                };
                if self.items.contains_key(&key) {
                    return Some(Target::Item(key));
                }
                if let Some(full) = self.uses.get(&key) {
                    return self.resolve(module, full, next);
                }
                if let Some(item) = self.glob(module, name, next) {
                    return Some(item);
                }
                prelude(name).map(Target::Builtin)
            }
            [first, rest @ ..] => match first.as_str() {
                "::" => self.external(rest, next),
                "crate" => self.resolve(root_of(module), rest, next),
                "self" => self.resolve(module, rest, next),
                "super" => self.resolve(module.split_last().map_or(&[], |(_, p)| p), rest, next),
                _ => {
                    let mut child = module.to_vec();
                    child.push(first.clone());
                    if self.modules.contains_key(&child) {
                        return self.resolve(&child, rest, next);
                    }
                    let key = Key {
                        module: module.to_vec(),
                        name: first.clone(),
                    };
                    if let Some(full) = self.uses.get(&key) {
                        let mut expanded = full.clone();
                        expanded.extend(rest.iter().cloned());
                        return self.resolve(module, &expanded, next);
                    }
                    let root = root_of(module);
                    if module == root {
                        return self.external(segs, next);
                    }
                    // A path from outside the crate, as a `use` in a child module writes it.
                    self.resolve(root, segs, next)
                }
            },
        }
    }

    /// The item a glob `use` of `module` brings in as `name`. A glob brings in items only, so a name none of them has
    /// falls to the prelude, as in Rust.
    fn glob(&self, module: &[String], name: &str, depth: usize) -> Option<Target> {
        self.globs.get(module)?.iter().find_map(|from| {
            let mut path = from.clone();
            path.push(name.to_owned());
            match self.resolve(module, &path, depth)? {
                Target::Item(key) => Some(Target::Item(key)),
                Target::Builtin(_) => None,
            }
        })
    }

    /// What a path from outside the crate names: an item of a crate it reaches, or a type tsgen knows.
    fn external(&self, segs: &[String], depth: usize) -> Option<Target> {
        if let [name, rest @ ..] = segs {
            let root = vec!["::".to_owned(), name.clone()];
            if self.modules.contains_key(&root) {
                return self.resolve(&root, rest, depth);
            }
        }
        known(segs)
    }
}

/// The root module of the crate `module` is in: `[]`, or `["::", name]` in a crate reached.
fn root_of(module: &[String]) -> &[String] {
    match module {
        [colons, ..] if colons == "::" => module.get(..2).unwrap_or(module),
        _ => &[],
    }
}

fn public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn uses(
    module: &[String],
    tree: &UseTree,
    prefix: Vec<String>,
    out: &mut BTreeMap<Key, Vec<String>>,
    globs: &mut BTreeMap<Vec<String>, Vec<Vec<String>>>,
) {
    let mut bind = |local: String, mut full: Vec<String>, last: String| {
        full.push(last);
        out.insert(
            Key {
                module: module.to_vec(),
                name: local,
            },
            full,
        );
    };
    match tree {
        UseTree::Path(p) => {
            let mut next = prefix;
            next.push(p.ident.unraw().to_string());
            uses(module, &p.tree, next, out, globs);
        }
        UseTree::Name(n) if n.ident == "self" => {
            if let Some((last, rest)) = prefix.split_last() {
                bind(last.clone(), rest.to_vec(), last.clone());
            }
        }
        UseTree::Name(n) => {
            let name = n.ident.unraw().to_string();
            bind(name.clone(), prefix, name);
        }
        UseTree::Rename(r) => bind(r.rename.unraw().to_string(), prefix, r.ident.unraw().to_string()),
        UseTree::Group(g) => {
            for t in &g.items {
                uses(module, t, prefix.clone(), out, globs);
            }
        }
        UseTree::Glob(_) => globs.entry(module.to_vec()).or_default().push(prefix),
    }
}

fn prelude(name: &str) -> Option<Builtin> {
    Some(match name {
        "String" => Builtin::String,
        "bool" => Builtin::Boolean,
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" | "f32" | "f64" => {
            Builtin::Number
        }
        "Vec" => Builtin::Vec,
        "Option" => Builtin::Option,
        "Box" => Builtin::Box,
        _ => return None,
    })
}

/// A type from a crate tsgen does not read, which it knows the JSON of.
fn known(segs: &[String]) -> Option<Target> {
    let path = segs.join("::");
    let b = match path.as_str() {
        "serde_json::Value" | "serde_json::value::Value" => Builtin::Value,
        "std::collections::BTreeMap" | "std::collections::HashMap" | "alloc::collections::BTreeMap" => Builtin::Map,
        "std::string::String" | "alloc::string::String" => Builtin::String,
        "std::vec::Vec" | "alloc::vec::Vec" => Builtin::Vec,
        "std::option::Option" | "core::option::Option" => Builtin::Option,
        "std::boxed::Box" | "alloc::boxed::Box" => Builtin::Box,
        _ => return None,
    };
    Some(Target::Builtin(b))
}

fn constant(file: &str, expr: &Expr) -> Result<String, Error> {
    match expr {
        Expr::Lit(l) => match &l.lit {
            Lit::Int(i) => Ok(i.base10_digits().to_owned()),
            Lit::Bool(b) => Ok(b.value.to_string()),
            Lit::Str(s) => Ok(crate::emit::quote(&s.value())),
            other => Err(Error::at(
                file,
                other.span(),
                "a constant is a number, a bool or a string",
            )),
        },
        Expr::Unary(u) if matches!(u.op, UnOp::Neg(_)) => Ok(format!("-{}", constant(file, &u.expr)?)),
        other => Err(Error::at(
            file,
            other.span(),
            "a constant is a number, a bool or a string",
        )),
    }
}

/// A `text_enum!`'s declaration: a union of its texts, in the order the body names them. The `Default` it derives and
/// the value that default is say nothing of its JSON.
fn text_enum_decl(key: &Key, file: &str, e: &syn::ItemEnum) -> Result<Decl, Error> {
    let enum_attrs = e.attrs.iter().filter(|a| !a.path().is_ident("derive"));
    let value_attrs = e
        .variants
        .iter()
        .flat_map(|v| &v.attrs)
        .filter(|a| !a.path().is_ident("default"));
    if let Some(a) = enum_attrs.chain(value_attrs).find(|a| !a.path().is_ident("doc")) {
        return Err(Error::at(
            file,
            a.span(),
            "a `text_enum!` takes only docs and a derive, and its values only docs and `#[default]`",
        ));
    }
    let mut cases = Vec::new();
    for v in &e.variants {
        let text = match &v.discriminant {
            Some((_, Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }))) => s.value(),
            _ => {
                return Err(Error::at(
                    file,
                    v.span(),
                    "each value of a `text_enum!` is `Name = \"text\"`",
                ));
            }
        };
        cases.push(Case {
            name: text,
            docs: attrs::docs(&v.attrs),
            data: Data::Unit,
        });
    }
    Ok(Decl {
        key: key.clone(),
        name: key.name.clone(),
        docs: attrs::docs(&e.attrs),
        body: Body::Strings(cases),
    })
}

/// Whether `m` is `text_enum!`.
fn text_enum(m: &syn::ItemMacro) -> bool {
    m.mac.path.segments.last().is_some_and(|s| s.ident == "text_enum")
}

/// Whether `m` is `json_tagged!`.
fn json_tagged(m: &syn::ItemMacro) -> bool {
    m.mac.path.segments.last().is_some_and(|s| s.ident == "json_tagged")
}

/// Whether `m` is `json_paired!`.
fn json_paired(m: &syn::ItemMacro) -> bool {
    m.mac.path.segments.last().is_some_and(|s| s.ident == "json_paired")
}

/// Whether `i` is a `Serialize` or `Deserialize` written by hand.
fn serde_impl(i: &syn::ItemImpl) -> bool {
    i.trait_
        .as_ref()
        .and_then(|(_, p, _)| p.segments.last())
        .is_some_and(|s| s.ident == "Serialize" || s.ident == "Deserialize")
}

/// `Struct, field, "tag", "content"`, the arguments of `json_paired!`.
fn paired(input: syn::parse::ParseStream<'_>) -> syn::Result<(String, Pair)> {
    let name: syn::Ident = input.parse()?;
    input.parse::<syn::Token![,]>()?;
    let field: syn::Ident = input.parse()?;
    input.parse::<syn::Token![,]>()?;
    let tag: syn::LitStr = input.parse()?;
    input.parse::<syn::Token![,]>()?;
    let content: syn::LitStr = input.parse()?;
    Ok((
        name.unraw().to_string(),
        Pair {
            field: field.unraw().to_string(),
            tag: tag.value(),
            content: content.value(),
        },
    ))
}

/// `Enum, "field"`, the arguments of `json_tagged!`.
fn tagged(input: syn::parse::ParseStream<'_>) -> syn::Result<(String, String)> {
    let name: syn::Ident = input.parse()?;
    input.parse::<syn::Token![,]>()?;
    let tag: syn::LitStr = input.parse()?;
    Ok((name.unraw().to_string(), tag.value()))
}
