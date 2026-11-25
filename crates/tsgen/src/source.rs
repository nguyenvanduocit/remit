//! A crate's source as a tree of modules: `lib.rs`, the files of its `mod` items, and the modules written inline.

use std::collections::BTreeMap;

use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::{Attribute, Item};

use crate::error::Error;

pub struct Module {
    /// From the root of the crate written: `[]` is its `lib.rs`, `["runtime", "to_core"]` is `mod to_core` in
    /// `runtime.rs`. A crate it reaches is below `::` and its name, as Rust writes a path into it: `["::", "link",
    /// "envelope"]`.
    pub path: Vec<String>,
    /// The file it is in, as an error names it: a crate it reaches puts its name before (`link/envelope.rs`).
    pub file: String,
    /// Every item but the `mod` items, which are modules of their own.
    pub items: Vec<Item>,
    /// Why the module cannot be read, raised only when something it holds is needed.
    pub unread: Option<Error>,
}

/// A crate's source: its name as a Rust path writes it (`link`, `client_web`) and its `src` files, each a file name
/// (`lib.rs`, `app.rs`) and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub name: String,
    pub files: BTreeMap<String, String>,
}

/// Every module of `krate`, parents before children: below `[]` when it is the crate written, below `["::", name]` when
/// it is one that crate reaches.
pub fn modules(krate: &Source, written: bool) -> Result<Vec<Module>, Error> {
    let walker = Walker {
        files: &krate.files,
        label: if written {
            String::new()
        } else {
            format!("{}/", krate.name)
        },
    };
    let root = if written {
        Vec::new()
    } else {
        vec!["::".to_owned(), krate.name.clone()]
    };
    let lib = krate
        .files
        .get("lib.rs")
        .ok_or_else(|| Error::new(&walker.label("lib.rs"), 0, "the crate has no lib.rs"))?;
    let items = walker.parse("lib.rs", lib)?.items;
    let mut out = Vec::new();
    walker.walk("lib.rs", &root, root.clone(), items, None, &mut out)?;
    Ok(out)
}

/// Walks one crate's files.
struct Walker<'a> {
    files: &'a BTreeMap<String, String>,
    /// What an error puts before a file's name.
    label: String,
}

impl Walker<'_> {
    fn label(&self, file: &str) -> String {
        format!("{}{file}", self.label)
    }

    fn parse(&self, file: &str, text: &str) -> Result<syn::File, Error> {
        syn::parse_file(text).map_err(|e| Error::syn(&self.label(file), &e))
    }

    /// `file` is the name in `files` of the file `items` are in; `root` is the crate's root module, whose `mod` items
    /// alone are files of their own.
    fn walk(
        &self,
        file: &str,
        root: &[String],
        path: Vec<String>,
        items: Vec<Item>,
        unread: Option<Error>,
        out: &mut Vec<Module>,
    ) -> Result<(), Error> {
        let label = self.label(file);
        let (mods, items): (Vec<_>, Vec<_>) = items.into_iter().partition(|i| matches!(i, Item::Mod(_)));
        out.push(Module {
            path: path.clone(),
            file: label.clone(),
            items,
            unread: unread.clone(),
        });
        for item in mods {
            let Item::Mod(m) = item else { continue };
            let mut child = path.clone();
            child.push(m.ident.unraw().to_string());
            let unread = unread.clone().or_else(|| attributes(&label, &m.attrs));
            match m.content {
                Some((_, inner)) => self.walk(file, root, child, inner, unread, out)?,
                None if path == root => {
                    let name = format!("{}.rs", m.ident.unraw());
                    let text = self.files.get(&name).ok_or_else(|| {
                        Error::at(&label, m.ident.span(), format!("no file {name} for `mod {}`", m.ident))
                    })?;
                    self.walk(&name, root, child, self.parse(&name, text)?.items, unread, out)?;
                }
                None => {
                    let why = Error::at(
                        &label,
                        m.ident.span(),
                        "a module in a file of its own below the crate root is not read",
                    );
                    self.walk(file, root, child, Vec::new(), Some(why), out)?;
                }
            }
        }
        Ok(())
    }
}

/// A module under `cfg` or any other attribute might not be what is compiled: only its docs are taken.
fn attributes(file: &str, attrs: &[Attribute]) -> Option<Error> {
    attrs
        .iter()
        .find(|a| !a.path().is_ident("doc"))
        .map(|a| Error::at(file, a.span(), "a module's only attribute can be its docs"))
}
