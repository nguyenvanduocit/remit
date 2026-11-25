use std::collections::BTreeMap;

use model::Skill;

use crate::frontmatter::{FrontmatterError, header};

// The limits of a skill. Claude Code loads a skill's files into a session on demand, and the library keeps every one of
// them in a store document, so each is bounded.
pub const MAX_NAME_LEN: usize = 64;
/// In characters.
pub const MAX_DESCRIPTION_LEN: usize = 1024;
pub const MAX_FILE_BYTES: usize = 64 << 10;
pub const MAX_TOTAL_BYTES: usize = 1 << 20;
pub const MAX_FILES: usize = 200;

const SKILL_FILE: &str = "SKILL.md";

/// Why the library may not store a skill.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error(
        "skill name {0:?} is not lower-case words joined by single dashes or dots, at most {MAX_NAME_LEN} characters"
    )]
    Name(String),
    #[error("skill {0}: the description must be 1 to {MAX_DESCRIPTION_LEN} characters")]
    Description(String),
    #[error("skill {0} has no {SKILL_FILE}")]
    NoSkillFile(String),
    #[error("skill {skill} has {count} files, at most {MAX_FILES}")]
    TooManyFiles { skill: String, count: usize },
    #[error("skill {skill}: {source}")]
    Path { skill: String, source: PathError },
    #[error("skill {skill}: {path} is {size} bytes, at most {MAX_FILE_BYTES}")]
    FileTooLarge { skill: String, path: String, size: usize },
    #[error("skill {skill}: {path} is not text")]
    NotText { skill: String, path: String },
    #[error("skill {skill}: {dir} is both a file and a directory")]
    FileAndDirectory { skill: String, dir: String },
    #[error("skill {skill} holds {total} bytes, at most {MAX_TOTAL_BYTES}")]
    TooLarge { skill: String, total: usize },
    #[error("skill {skill}: {SKILL_FILE}: {source}")]
    Header { skill: String, source: FrontmatterError },
    #[error("skill {skill}: {SKILL_FILE} names the skill {named:?}")]
    NamedOtherwise { skill: String, named: String },
    #[error("skill {0}: the description is not the one {SKILL_FILE} carries: there is one description, the file's")]
    DescriptionDiffers(String),
}

/// Why a path cannot name a file inside a skill's directory.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("a file path is empty")]
    Empty,
    #[error("path {0:?} has a backslash")]
    Backslash(String),
    #[error("path {0:?} is absolute")]
    Absolute(String),
    #[error("path {0:?} has a control character")]
    Control(String),
    #[error("path {0:?} has an empty segment")]
    EmptySegment(String),
    #[error("path {path:?} has a {segment:?} segment")]
    DotSegment { path: String, segment: String },
    #[error("path {0:?} is hidden: a skill holds no dot file or dot directory")]
    Hidden(String),
}

/// Whether the library may store the skill: the one gate for a skill an agent wrote and for one the importer read.
pub fn validate(s: &Skill) -> Result<(), SkillError> {
    if s.name.len() > MAX_NAME_LEN || !is_skill_name(&s.name) {
        return Err(SkillError::Name(s.name.clone()));
    }
    if s.description.trim().is_empty() || s.description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(SkillError::Description(s.name.clone()));
    }
    let Some(md) = s.files.get(SKILL_FILE) else {
        return Err(SkillError::NoSkillFile(s.name.clone()));
    };
    // Sizes first: parsing a frontmatter takes longer than its text grows, and an agent may send a skill of any size.
    validate_files(&s.name, &s.files)?;
    let (name, description) = header(md).map_err(|source| SkillError::Header {
        skill: s.name.clone(),
        source,
    })?;
    if name != s.name {
        return Err(SkillError::NamedOtherwise {
            skill: s.name.clone(),
            named: name,
        });
    }
    if description.trim() != s.description.trim() {
        return Err(SkillError::DescriptionDiffers(s.name.clone()));
    }
    Ok(())
}

/// A name is lower-case words joined by a single dash or dot: Claude Code skills in use are named so (vault.help).
fn is_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .split(['-', '.'])
            .all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

fn validate_files(skill: &str, files: &BTreeMap<String, String>) -> Result<(), SkillError> {
    if files.len() > MAX_FILES {
        return Err(SkillError::TooManyFiles {
            skill: skill.to_owned(),
            count: files.len(),
        });
    }
    let mut total: usize = 0;
    // A BTreeMap walks its paths in order, so the first problem reported is the same every time.
    for (p, content) in files {
        valid_skill_path(p).map_err(|source| SkillError::Path {
            skill: skill.to_owned(),
            source,
        })?;
        if content.len() > MAX_FILE_BYTES {
            return Err(SkillError::FileTooLarge {
                skill: skill.to_owned(),
                path: p.clone(),
                size: content.len(),
            });
        }
        if !is_text(content) {
            return Err(SkillError::NotText {
                skill: skill.to_owned(),
                path: p.clone(),
            });
        }
        total = total.saturating_add(content.len());
        if let Some(dir) = parents(p).find(|dir| files.contains_key(*dir)) {
            return Err(SkillError::FileAndDirectory {
                skill: skill.to_owned(),
                dir: dir.to_owned(),
            });
        }
    }
    if total > MAX_TOTAL_BYTES {
        return Err(SkillError::TooLarge {
            skill: skill.to_owned(),
            total,
        });
    }
    Ok(())
}

/// The directories `p` is in, innermost first: `a/b/c.md` is in `a/b` and `a`.
pub(crate) fn parents(p: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(p.rsplit_once('/').map(|(dir, _)| dir), |dir| {
        dir.rsplit_once('/').map(|(up, _)| up)
    })
}

/// Whether `p` can name a file inside a skill's directory: relative, slash-separated, already clean, so nothing written
/// under a root can land outside it.
pub fn valid_path(p: &str) -> Result<(), PathError> {
    if p.is_empty() {
        return Err(PathError::Empty);
    }
    if p.contains('\\') {
        return Err(PathError::Backslash(p.to_owned()));
    }
    if p.starts_with('/') {
        return Err(PathError::Absolute(p.to_owned()));
    }
    // A Rust string is UTF-8 already, so only the control characters are left to refuse.
    if p.chars().any(|c| c < '\u{20}' || c == '\u{7f}') {
        return Err(PathError::Control(p.to_owned()));
    }
    for segment in p.split('/') {
        match segment {
            "" => return Err(PathError::EmptySegment(p.to_owned())),
            "." | ".." => {
                return Err(PathError::DotSegment {
                    path: p.to_owned(),
                    segment: segment.to_owned(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Whether `p` can name a file a skill holds: a valid path ([`valid_path`]) with no hidden segment. A skill holds no dot
/// file or dot directory: .env and .git hold credentials and repositories. (The rendered library, which has a
/// .claude-plugin directory of its own, is checked with [`valid_path`] alone.)
pub fn valid_skill_path(p: &str) -> Result<(), PathError> {
    valid_path(p)?;
    if p.split('/').any(|segment| segment.starts_with('.')) {
        return Err(PathError::Hidden(p.to_owned()));
    }
    Ok(())
}

/// Whether `content` is text a skill may hold: no NUL (a Rust string is valid UTF-8 already).
pub fn is_text(content: &str) -> bool {
    !content.contains('\0')
}
