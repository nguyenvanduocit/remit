use std::path::Path;

use model::{Agent, Loadout, Source, Time};
use serde_json::Value;
use sha2::{Digest, Sha256};
use skills::Fields;

use crate::read::{check_name, entry_target, read_limited};
use crate::{Error, Kind, Known, Report, SOURCE_KIND};

/// The frontmatter keys an Agent has a place for; every other key is reported.
const MAPPED_FIELDS: [&str; 6] = ["name", "description", "tools", "model", "effort", "skills"];

/// Imports agents/<name>.md. The record keeps only what the library can resolve (a model of the catalog, an effort it
/// takes and skills it holds); the rest is in the Report, the MCP servers the file names included: an agent of Remit holds Remit's own tools.
/// The record's Source is stamped with the time of the import and a hash of the file.
pub fn agent(config_dir: &Path, name: &str, at: Time, known: &Known) -> Result<(Agent, Report), Error> {
    let data = read_agent(config_dir, name).map_err(|source| Error::Entry {
        kind: Kind::Agent,
        name: name.to_owned(),
        source: Box::new(source),
    })?;
    let (mut agent, report) = map_agent(&data, known)?;
    agent.source = Some(Source {
        kind: SOURCE_KIND.to_owned(),
        name: name.to_owned(),
        hash: hex::encode(Sha256::digest(&data)),
        at,
    });
    Ok((agent, report))
}

/// Reads the file of an agent: a regular file, or a link to one.
pub(crate) fn read_agent(config_dir: &Path, name: &str) -> Result<Vec<u8>, Error> {
    check_name(name)?;
    let path = entry_target(config_dir, "agents", &format!("{name}.md"))?;
    let meta = path.metadata().map_err(|e| Error::io(&path, e))?;
    if !meta.is_file() {
        return Err(Error::NotRegular(format!("agents/{name}.md")));
    }
    read_limited(&path, skills::MAX_FILE_BYTES)
}

pub(crate) fn map_agent(data: &[u8], known: &Known) -> Result<(Agent, Report), Error> {
    let text = std::str::from_utf8(data).map_err(|_| Error::Field {
        field: "the file",
        problem: "is not UTF-8 text",
    })?;
    let (fields, body) = skills::parse_frontmatter(text)?;
    let mut report = Report::default();

    let name = match fields.get("name") {
        Some(Value::String(raw)) if !raw.trim().is_empty() => raw.trim().to_owned(),
        _ => return Err(Error::NoName),
    };
    let id = library_id(&name);
    if !library::valid_id(&id) {
        return Err(Error::BadId(name));
    }
    let description = match fields.get("description") {
        Some(Value::String(d)) => d.trim().to_owned(),
        _ => String::new(),
    };

    let tools = tools(fields.get("tools"))?;
    // A deny list cannot be mapped: an agent that holds "every tool but Bash" would come in holding all of them.
    match fields.get("disallowedTools") {
        None | Some(Value::Null) => {}
        Some(Value::String(s)) if s.is_empty() => {}
        Some(_) => return Err(Error::DisallowedTools),
    }

    let mut model = String::new();
    match fields.get("model") {
        None | Some(Value::Null) => {}
        Some(Value::String(m)) if known.models.contains(m) => model.clone_from(m),
        Some(Value::String(m)) if m.is_empty() => {}
        Some(Value::String(m)) => report.unmapped.push(format!("model {m}")),
        Some(_) => {
            return Err(Error::Field {
                field: "model",
                problem: "is not text",
            });
        }
    }

    let mut effort = String::new();
    match fields.get("effort") {
        None | Some(Value::Null) => {}
        Some(Value::String(e)) if matches!(provider::Effort::parse(e), Ok(Some(_))) => effort.clone_from(e),
        Some(Value::String(e)) if e.is_empty() => {}
        Some(Value::String(e)) => report.unmapped.push(format!("effort {e}")),
        Some(_) => {
            return Err(Error::Field {
                field: "effort",
                problem: "is not text",
            });
        }
    }

    let (held, unmapped) = resolve_skills(fields.get("skills"), &known.skills)?;
    report.unmapped.extend(unmapped);
    report.unmapped.extend(unmapped_fields(&fields));

    let agent = Agent {
        id,
        name,
        description,
        model,
        effort,
        loadout: Loadout {
            instructions: body.trim().to_owned(),
            tools,
            skills: held,
        },
        ..Agent::default()
    };
    Ok((agent, report))
}

/// A name as the library writes an agent's id: lower case, spaces and underscores turned into dashes.
fn library_id(name: &str) -> String {
    name.trim().to_lowercase().replace([' ', '_'], "-")
}

/// Every key of the frontmatter an Agent has no place for, in key order.
fn unmapped_fields(fields: &Fields) -> impl Iterator<Item = String> + '_ {
    fields
        .keys()
        .filter(|key| !MAPPED_FIELDS.contains(&key.as_str()))
        .map(|key| format!("field {key}"))
}

/// Reads a frontmatter value written as a comma-separated string or as a YAML list; null is no items.
fn items(value: Option<&Value>, field: &'static str) -> Result<Vec<Value>, Error> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(s)) => Ok(s
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| Value::String(part.to_owned()))
            .collect()),
        Some(Value::Array(list)) => Ok(list.clone()),
        Some(_) => Err(Error::Field {
            field,
            problem: "must be a comma-separated string or a list",
        }),
    }
}

/// Reads the allowlist. Absent is None, which means every tool; present is a list, empty meaning none.
fn tools(value: Option<&Value>) -> Result<Option<Vec<String>>, Error> {
    if matches!(value, None | Some(Value::Null)) {
        return Ok(None);
    }
    let mut out = Vec::new();
    for item in items(value, "tools")? {
        let Value::String(name) = item else {
            return Err(Error::Field {
                field: "tools",
                problem: "every entry must be a tool name",
            });
        };
        let name = name.trim();
        if !name.is_empty() {
            out.push(name.to_owned());
        }
    }
    Ok(Some(out))
}

/// Splits the skills an agent names into what the library holds and what it lacks.
fn resolve_skills(value: Option<&Value>, known: &[String]) -> Result<(Vec<String>, Vec<String>), Error> {
    let mut held = Vec::new();
    let mut unmapped = Vec::new();
    for item in items(value, "skills")? {
        let Value::String(name) = item else {
            return Err(Error::Field {
                field: "skills",
                problem: "every entry must be a name",
            });
        };
        if known.contains(&name) {
            held.push(name);
        } else {
            unmapped.push(format!("skill {name}"));
        }
    }
    Ok((held, unmapped))
}
