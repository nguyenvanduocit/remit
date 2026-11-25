//! The skills that ship with Remit. They are text in the binary, not records of the library: the owner neither edits nor
//! deletes them, and an agent of the library cannot hold one. Each is for one kind of card, and its knowledge is read when
//! the work calls for it, not carried in that card's brief from the first message.

use std::collections::BTreeMap;

use model::Skill;
use rust_embed::RustEmbed;
use skills::FrontmatterError;

const SKILL_FILE: &str = "SKILL.md";

/// Every shipped skill's files, one directory per skill.
#[derive(RustEmbed)]
#[folder = "skills"]
struct Shipped;

/// Why a built-in skill cannot be read. Each is a defect of the build, never of the owner's data.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("built-in skill {name}: {path} is not UTF-8 text")]
    NotText { name: String, path: String },
    #[error("built-in skill {name}: {source}")]
    Header { name: String, source: FrontmatterError },
}

/// The skills of the Assistant, in name order: its manual of the app, area by area.
pub fn for_assistant() -> Result<Vec<Skill>, Error> {
    let mut by_skill: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for path in Shipped::iter() {
        // A file directly under skills/ belongs to no skill.
        let Some((name, rel)) = path.split_once('/') else {
            continue;
        };
        // What starts with a dot or an underscore (a .DS_Store, an editor's swap file) is no part of a skill.
        if path.split('/').any(|segment| segment.starts_with(['.', '_'])) {
            continue;
        }
        let Some(file) = Shipped::get(&path) else { continue };
        let text = String::from_utf8(file.data.into_owned()).map_err(|_| Error::NotText {
            name: name.to_owned(),
            path: rel.to_owned(),
        })?;
        by_skill
            .entry(name.to_owned())
            .or_default()
            .insert(rel.to_owned(), text);
    }
    by_skill.into_iter().map(|(name, files)| read(name, files)).collect()
}

/// One skill: every file of its directory, and the description its SKILL.md gives.
fn read(name: String, files: BTreeMap<String, String>) -> Result<Skill, Error> {
    let skill_md = files.get(SKILL_FILE).map_or("", String::as_str);
    let (_, description) = match skills::header(skill_md) {
        Ok(header) => header,
        Err(source) => return Err(Error::Header { name, source }),
    };
    Ok(Skill {
        name,
        description,
        files,
        ..Skill::default()
    })
}
