use std::collections::BTreeMap;
use std::fs::FileType;
use std::io;
use std::path::Path;

use model::{Skill, Source, Time};
use sha2::{Digest, Sha256};

use crate::read::{check_name, entry_target, read_limited};
use crate::{Error, Kind, Report, SOURCE_KIND, Skip};

const SKILL_FILE: &str = "SKILL.md";

/// How many left-out files a report lists one by one: a skill can hold thousands.
const MAX_SKIPPED: usize = 20;

/// Imports skills/<name>/. A skill is known by the name its SKILL.md gives, which is not always its directory's name;
/// `Source::name` keeps the directory, which is what a later import asks for.
pub fn skill(config_dir: &Path, name: &str, at: Time) -> Result<(Skill, Report), Error> {
    let (files, skipped) = read_skill(config_dir, name).map_err(|source| Error::Entry {
        kind: Kind::Skill,
        name: name.to_owned(),
        source: Box::new(source),
    })?;
    map_skill(name, files, skipped, at)
}

/// Turns the files read into a record, and refuses what the library would refuse.
pub(crate) fn map_skill(
    name: &str,
    files: BTreeMap<String, String>,
    skipped: Vec<Skip>,
    at: Time,
) -> Result<(Skill, Report), Error> {
    let skill_md = files.get(SKILL_FILE).map_or("", String::as_str);
    let (skill_name, description) = skills::header(skill_md).map_err(|source| Error::Header {
        name: name.to_owned(),
        source,
    })?;
    let hash = hash_files(&files);
    let skill = Skill {
        name: skill_name,
        description,
        files,
        source: Some(Source {
            kind: SOURCE_KIND.to_owned(),
            name: name.to_owned(),
            hash,
            at,
        }),
        ..Skill::default()
    };
    skills::validate(&skill)?;
    Ok((
        skill,
        Report {
            skipped,
            unmapped: Vec::new(),
        },
    ))
}

/// Hashes path and content of every file in path order. Each part ends in a NUL, which a text file cannot hold, so no two
/// sets of files give the same bytes.
fn hash_files(files: &BTreeMap<String, String>) -> String {
    let mut sum = Sha256::new();
    for (path, content) in files {
        for part in [path, content] {
            sum.update(part.as_bytes());
            sum.update(b"\0");
        }
    }
    hex::encode(sum.finalize())
}

/// Reads the files of skills/<name>/ that a skill may hold. Links are not followed, so nothing outside the skill's
/// directory can enter it; whatever is left out is in the Skips.
pub(crate) fn read_skill(config_dir: &Path, name: &str) -> Result<(BTreeMap<String, String>, Vec<Skip>), Error> {
    check_name(name)?;
    let dir = entry_target(config_dir, "skills", name)?;
    if !dir.metadata().map_err(|e| Error::io(&dir, e))?.is_dir() {
        return Err(Error::NotASkill(format!("skills/{name} is not a directory")));
    }
    let skill_md = dir.join(SKILL_FILE);
    let skill_md_type = match skill_md.symlink_metadata() {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(Error::NotASkill(format!("skills/{name} has no {SKILL_FILE}")));
        }
        Err(e) => return Err(Error::io(&skill_md, e)),
        Ok(meta) => meta.file_type(),
    };

    let mut r = SkillReader {
        dir: &dir,
        files: BTreeMap::new(),
        skipped: Vec::new(),
        dropped: 0,
        total: 0,
    };
    // SKILL.md goes first so that a limit can never be the reason a skill has none.
    r.take(SKILL_FILE, skill_md_type);
    if !r.files.contains_key(SKILL_FILE) {
        let why = r.skipped.first().map(|s| s.why.clone()).unwrap_or_default();
        return Err(Error::SkillFileNotTaken(why));
    }
    let root = std::fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))?;
    r.walk(root, "");
    if r.dropped > 0 {
        r.skipped.push(Skip {
            what: "…".to_owned(),
            why: format!("{} more left out for the same kinds of reason", r.dropped),
        });
    }
    Ok((r.files, r.skipped))
}

/// Gathers the files of one skill under the limits of a skill.
struct SkillReader<'a> {
    dir: &'a Path,
    files: BTreeMap<String, String>,
    skipped: Vec<Skip>,
    /// Left out without a line of their own: the list is capped.
    dropped: usize,
    total: usize,
}

/// Whether the walk goes on, or a limit of the skill has ended it.
#[derive(PartialEq, Eq)]
enum Walk {
    On,
    Stop,
}

impl SkillReader<'_> {
    /// Visits one directory of the skill, `prefix` its slash-separated path inside the skill ("" for the skill's own),
    /// in name order, without following a link.
    fn walk(&mut self, dir: std::fs::ReadDir, prefix: &str) -> Walk {
        let mut found = Vec::new();
        for entry in dir {
            match entry {
                Ok(entry) => found.push(entry),
                Err(e) => self.skip(prefix.trim_end_matches('/'), &e.to_string()),
            }
        }
        found.sort_by_key(std::fs::DirEntry::file_name);
        for entry in found {
            let raw = entry.file_name();
            let Some(base) = raw.to_str() else {
                self.skip(
                    &format!("{prefix}{}", raw.to_string_lossy()),
                    "the name is not UTF-8 text",
                );
                continue;
            };
            let rel = format!("{prefix}{base}");
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    self.skip(&rel, &e.to_string());
                    continue;
                }
            };
            if file_type.is_dir() {
                if base.starts_with('.') {
                    self.skip(&rel, "a hidden directory is not taken");
                    continue;
                }
                match std::fs::read_dir(entry.path()) {
                    Ok(sub) => {
                        if self.walk(sub, &format!("{rel}/")) == Walk::Stop {
                            return Walk::Stop;
                        }
                    }
                    Err(e) => self.skip(&rel, &e.to_string()),
                }
                continue;
            }
            if rel == SKILL_FILE {
                continue;
            }
            if self.take(&rel, file_type) == Walk::Stop {
                return Walk::Stop;
            }
        }
        Walk::On
    }

    fn skip(&mut self, what: &str, why: &str) {
        if self.skipped.len() >= MAX_SKIPPED {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.skipped.push(Skip {
            what: what.to_owned(),
            why: why.to_owned(),
        });
    }

    /// Adds one file unless it must not be, and says whether the limits end the skill: no later file is tried then.
    fn take(&mut self, rel: &str, file_type: FileType) -> Walk {
        if file_type.is_symlink() {
            self.skip(rel, "a symlink is not followed");
            return Walk::On;
        }
        if !file_type.is_file() {
            self.skip(rel, "not a regular file");
            return Walk::On;
        }
        if let Err(e) = skills::valid_skill_path(rel) {
            self.skip(rel, &e.to_string());
            return Walk::On;
        }
        let data = match read_limited(&self.dir.join(rel), skills::MAX_FILE_BYTES) {
            Ok(data) => data,
            Err(Error::TooBig(limit)) => {
                self.skip(rel, &format!("larger than {limit} bytes"));
                return Walk::On;
            }
            Err(e) => {
                self.skip(rel, &e.to_string());
                return Walk::On;
            }
        };
        let text = match String::from_utf8(data) {
            Ok(text) if skills::is_text(&text) => text,
            _ => {
                self.skip(rel, "not text");
                return Walk::On;
            }
        };
        if self.files.len() >= skills::MAX_FILES {
            self.skip(
                rel,
                &format!(
                    "the skill already holds the limit of {} files: this one and every later one are left out",
                    skills::MAX_FILES
                ),
            );
            return Walk::Stop;
        }
        let total = self.total.saturating_add(text.len());
        if total > skills::MAX_TOTAL_BYTES {
            self.skip(
                rel,
                &format!(
                    "adding it passes the limit of 1 MiB ({} bytes) for a skill: this one and every later one are left out",
                    skills::MAX_TOTAL_BYTES
                ),
            );
            return Walk::Stop;
        }
        self.files.insert(rel.to_owned(), text);
        self.total = total;
        Walk::On
    }
}
