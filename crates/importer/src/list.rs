use std::fs::DirEntry;
use std::path::Path;

use model::Time;

use crate::agent::{map_agent, read_agent};
use crate::read::confine;
use crate::skill::{map_skill, read_skill};
use crate::{Candidate, Error, Kind, Known};

/// Describes every agent and skill of the configuration, each as an import of it would see it. What is missing (no
/// directory, no file) is an empty list for that kind, not an error. An entry that cannot be imported is listed with the
/// reason in its problem, so the owner sees it and why it is not taken.
pub fn list(config_dir: &Path) -> Result<Vec<Candidate>, Error> {
    if config_dir.as_os_str().is_empty() {
        return Err(Error::NoConfigDir);
    }
    let mut out = Vec::new();
    for (kind, found) in [
        (Kind::Agent, list_agents(config_dir)),
        (Kind::Skill, list_skills(config_dir)),
    ] {
        match found {
            Ok(found) => out.extend(found),
            // One kind that cannot be read (a directory that leaves the configuration directory, a file too large) is
            // listed as a problem of its own, and does not hide what the others hold.
            Err(e) => out.push(candidate(kind, String::new()).with_problem(&e)),
        }
    }
    Ok(out)
}

fn candidate(kind: Kind, name: String) -> Candidate {
    Candidate {
        kind,
        name,
        description: String::new(),
        size: 0,
        problem: String::new(),
    }
}

impl Candidate {
    fn with_problem(mut self, e: &Error) -> Self {
        self.problem = e.to_string();
        self
    }
}

/// The directory of one kind under `config_dir`, in name order; a missing one has no
/// entries.
fn entries(config_dir: &Path, kind: &str) -> Result<Vec<DirEntry>, Error> {
    let dir = match confine(config_dir, &[kind]) {
        Err(e) if e.is_not_found() => return Ok(Vec::new()),
        other => other?,
    };
    let mut out = std::fs::read_dir(&dir)
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(|e| Error::io(&dir, e))?;
    out.sort_by_key(DirEntry::file_name);
    Ok(out)
}

/// An entry's name as an import call takes it. A name that is not UTF-8 cannot be named, so it is listed as a problem.
fn entry_name(kind: Kind, e: &DirEntry) -> Result<String, Candidate> {
    e.file_name().into_string().map_err(|raw| Candidate {
        problem: "the name is not UTF-8 text".to_owned(),
        ..candidate(kind, raw.to_string_lossy().into_owned())
    })
}

fn list_agents(config_dir: &Path) -> Result<Vec<Candidate>, Error> {
    let mut out = Vec::new();
    for e in entries(config_dir, "agents")? {
        if e.file_type().is_ok_and(|t| t.is_dir()) || !e.file_name().to_string_lossy().ends_with(".md") {
            continue;
        }
        let file_name = match entry_name(Kind::Agent, &e) {
            Ok(name) => name,
            Err(c) => {
                out.push(c);
                continue;
            }
        };
        let Some(name) = file_name.strip_suffix(".md") else {
            continue;
        };
        let c = candidate(Kind::Agent, name.to_owned());
        let described = read_agent(config_dir, name)
            .and_then(|data| map_agent(&data, &Known::default()).map(|(agent, _)| (agent, data.len())));
        out.push(match described {
            Ok((agent, len)) => Candidate {
                description: first_line(&agent.description),
                size: size(len),
                ..c
            },
            Err(e) => c.with_problem(&e),
        });
    }
    Ok(out)
}

fn list_skills(config_dir: &Path) -> Result<Vec<Candidate>, Error> {
    let mut out = Vec::new();
    for e in entries(config_dir, "skills")? {
        let name = match entry_name(Kind::Skill, &e) {
            Ok(name) => name,
            Err(c) => {
                out.push(c);
                continue;
            }
        };
        let read = read_skill(config_dir, &name);
        // A file, a directory without SKILL.md, a link to nothing: not a skill.
        if let Err(e) = &read
            && (matches!(e, Error::NotASkill(_)) || e.is_not_found())
        {
            continue;
        }
        let c = candidate(Kind::Skill, name.clone());
        let described = read.and_then(|(files, skipped)| {
            let total = files.values().map(String::len).fold(0, usize::saturating_add);
            // A listing shows the skill's description; when it was read is no part of it.
            map_skill(&name, files, skipped, Time::default()).map(|(skill, _)| (skill, total))
        });
        out.push(match described {
            Ok((skill, total)) => Candidate {
                description: first_line(&skill.description),
                size: size(total),
                ..c
            },
            Err(e) => c.with_problem(&e),
        });
    }
    Ok(out)
}

fn first_line(text: &str) -> String {
    let text = text.trim();
    text.split_once('\n').map_or(text, |(line, _)| line).trim().to_owned()
}

fn size(len: usize) -> i64 {
    i64::try_from(len).unwrap_or(i64::MAX)
}
