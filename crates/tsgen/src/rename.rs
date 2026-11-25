//! serde's `rename_all` rules, as serde_derive applies them (`serde_derive/src/internals/case.rs`): a variant is written in
//! PascalCase and a field in snake_case, so each rule has its own way from each.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    None,
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl Rule {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "lowercase" => Self::Lower,
            "UPPERCASE" => Self::Upper,
            "PascalCase" => Self::Pascal,
            "camelCase" => Self::Camel,
            "snake_case" => Self::Snake,
            "SCREAMING_SNAKE_CASE" => Self::ScreamingSnake,
            "kebab-case" => Self::Kebab,
            "SCREAMING-KEBAB-CASE" => Self::ScreamingKebab,
            _ => return None,
        })
    }

    pub fn variant(self, variant: &str) -> String {
        match self {
            Self::None | Self::Pascal => variant.to_owned(),
            Self::Lower => variant.to_ascii_lowercase(),
            Self::Upper => variant.to_ascii_uppercase(),
            Self::Camel => lower_first(variant),
            Self::Snake => snake(variant),
            Self::ScreamingSnake => snake(variant).to_ascii_uppercase(),
            Self::Kebab => snake(variant).replace('_', "-"),
            Self::ScreamingKebab => snake(variant).to_ascii_uppercase().replace('_', "-"),
        }
    }

    pub fn field(self, field: &str) -> String {
        match self {
            Self::None | Self::Lower | Self::Snake => field.to_owned(),
            Self::Upper | Self::ScreamingSnake => field.to_ascii_uppercase(),
            Self::Pascal => pascal(field),
            Self::Camel => lower_first(&pascal(field)),
            Self::Kebab => field.replace('_', "-"),
            Self::ScreamingKebab => field.to_ascii_uppercase().replace('_', "-"),
        }
    }
}

/// A `SCREAMING_SNAKE` constant as a TypeScript `camelCase` one: `ENTRY_BYTES` is `entryBytes`.
pub fn constant(name: &str) -> String {
    Rule::Camel.field(&name.to_ascii_lowercase())
}

fn snake(variant: &str) -> String {
    let mut out = String::new();
    for (i, ch) in variant.char_indices() {
        if i > 0 && ch.is_uppercase() {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn pascal(field: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for ch in field.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            out.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn lower_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}
