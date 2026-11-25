use std::collections::BTreeMap;

use serde_json::Value;

const FENCE: &str = "---";
/// U+FEFF, which some editors put first in a file.
const BYTE_ORDER_MARK: char = '\u{FEFF}';

/// The YAML mapping a frontmatter holds, by key. A value is any YAML value.
pub type Fields = BTreeMap<String, Value>;

/// Why a text has no frontmatter, or not the one a skill needs.
#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("frontmatter: the text does not start with a --- line")]
    NoOpening,
    #[error("frontmatter: the --- block is never closed")]
    NotClosed,
    #[error("frontmatter: invalid yaml: {0}")]
    Yaml(Box<serde_saphyr::Error>),
    #[error("frontmatter has no {0}")]
    Missing(&'static str),
    #[error("frontmatter {0} is not text")]
    NotText(&'static str),
    #[error("frontmatter {0} is blank")]
    Blank(&'static str),
}

/// Splits a Markdown file into the YAML mapping between its two `---` lines and the text after them. The fence is a line
/// that is exactly `---`: an indented one inside a block scalar belongs to the YAML.
pub fn parse_frontmatter(text: &str) -> Result<(Fields, &str), FrontmatterError> {
    let text = text.strip_prefix(BYTE_ORDER_MARK).unwrap_or(text);
    let (first, mut rest) = text.split_once('\n').unwrap_or((text, ""));
    if first.trim_end_matches([' ', '\t', '\r']) != FENCE {
        return Err(FrontmatterError::NoOpening);
    }
    // Every line keeps its newline: a block scalar's final newline is part of its value.
    let mut block = String::new();
    while !rest.is_empty() {
        let line;
        (line, rest) = rest.split_once('\n').unwrap_or((rest, ""));
        if line.trim_end_matches([' ', '\t', '\r']) == FENCE {
            // An empty block, or one of comments only, is no mapping at all: no fields.
            // YAML 1.2: only true and false are booleans, yes and on are text.
            let options = serde_saphyr::options! { strict_booleans: true };
            let fields: Option<Fields> = serde_saphyr::from_str_with_options(&block, options)
                .map_err(|e| FrontmatterError::Yaml(Box::new(e)))?;
            return Ok((fields.unwrap_or_default(), rest));
        }
        block.push_str(line.strip_suffix('\r').unwrap_or(line));
        block.push('\n');
    }
    Err(FrontmatterError::NotClosed)
}

/// What a SKILL.md says about its skill: the name and the description Claude Code shows the model. Both are required, so
/// a file without them is not a skill.
pub fn header(skill_md: &str) -> Result<(String, String), FrontmatterError> {
    let (fields, _) = parse_frontmatter(skill_md)?;
    Ok((text(&fields, "name")?, text(&fields, "description")?))
}

/// A required non-blank string field, trimmed.
fn text(fields: &Fields, key: &'static str) -> Result<String, FrontmatterError> {
    match fields.get(key) {
        None | Some(Value::Null) => Err(FrontmatterError::Missing(key)),
        Some(Value::String(s)) if s.trim().is_empty() => Err(FrontmatterError::Blank(key)),
        Some(Value::String(s)) => Ok(s.trim().to_owned()),
        Some(_) => Err(FrontmatterError::NotText(key)),
    }
}
