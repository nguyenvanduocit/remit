use std::collections::HashSet;
use std::fmt;

use model::{Agent, Loadout, RESERVED_AGENT_IDS, Signs, Title};

use crate::access::Library;

const MAX_IDENTIFIER_LEN: usize = 40;
const MAX_NAME_LEN: usize = 80;
const MAX_DESCRIPTION_LEN: usize = 200;
const MAX_INSTRUCTIONS_LEN: usize = 20000;
const MAX_TOOL_NAME_LEN: usize = 64;

const IDENTIFIER_RULE: &str = "lowercase letters, digits and single dashes, like frontend-dev";
const TOOL_NAME_RULE: &str = "letters, digits and underscores, like Read or Bash";

/// Everything wrong with a record, one line each, so whoever writes it fixes all of it in one round.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Problems(pub Vec<String>);

impl fmt::Display for Problems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("\n"))
    }
}

impl std::error::Error for Problems {}

impl Problems {
    fn add(&mut self, problem: Option<String>) {
        self.0.extend(problem);
    }

    fn merge(&mut self, more: Result<(), Problems>) {
        if let Err(more) = more {
            self.0.extend(more.0);
        }
    }

    fn into_result(self) -> Result<(), Problems> {
        if self.0.is_empty() { Ok(()) } else { Err(self) }
    }
}

/// What to change for the library to store the agent, or `Ok`. It holds the agent to the library it would run against: a
/// skill it names must exist.
pub fn validate_agent(a: &Agent, lib: &Library) -> Result<(), Problems> {
    let mut p = Problems::default();

    p.add(check_name(
        "id",
        &a.id,
        MAX_IDENTIFIER_LEN,
        is_identifier,
        IDENTIFIER_RULE,
    ));
    if RESERVED_AGENT_IDS.contains(&a.id.as_str()) {
        p.add(Some(format!(
            "id {:?} is reserved for an agent Remit starts itself: choose another",
            a.id
        )));
    }

    if a.name.trim().is_empty() {
        p.add(Some(format!(
            "name is empty: say what to call the agent, in 1 to {MAX_NAME_LEN} characters"
        )));
    }
    p.add(check_length("name", &a.name, MAX_NAME_LEN));

    if a.description.trim().is_empty() {
        p.add(Some(format!(
            "description is empty: say in one line, in 1 to {MAX_DESCRIPTION_LEN} characters, what the agent is for"
        )));
    }
    p.add(check_length("description", &a.description, MAX_DESCRIPTION_LEN));
    if a.description.contains(['\r', '\n']) {
        p.add(Some(
            "description must be one line: move the rest into the instructions".to_owned(),
        ));
    }

    p.add(check_signs(a));

    p.merge(validate_loadout(&a.loadout, lib));

    p.into_result()
}

/// The gate an agent signs, held to its title (ADR-0032): the title judge signs the goal and nothing else, and no other
/// title signs the goal. Review and qc belong to workers, so they stay open to an untitled template too.
fn check_signs(a: &Agent) -> Option<String> {
    let judge = a.title == Some(Title::Judge);
    match &a.signs {
        signs if judge && *signs != Signs::Goal => Some(format!(
            "title judge signs the goal: set signs to {:?} (a judge never signs review or qc, and one that signs nothing judges nothing)",
            Signs::Goal.as_str()
        )),
        Signs::Goal if !judge => Some(format!(
            "signs {:?} belongs to the title judge: give the template that title, or use review, qc or leave signs empty",
            Signs::Goal.as_str()
        )),
        _ => None,
    }
}

/// What to change for a loadout to be held: an agent's, or the one a hire gives its card. It holds the loadout to the
/// library it would run against: a skill it names must exist.
pub fn validate_loadout(l: &Loadout, lib: &Library) -> Result<(), Problems> {
    let mut p = Problems::default();

    p.add(check_length("instructions", &l.instructions, MAX_INSTRUCTIONS_LEN));

    let tools = l.tools.as_deref().unwrap_or_default();
    for tool in tools {
        p.add(check_name(
            "tool name",
            tool,
            MAX_TOOL_NAME_LEN,
            is_tool_name,
            TOOL_NAME_RULE,
        ));
    }
    p.merge(repeated("tool", tools));

    for name in l.skills.iter().filter(|name| !lib.has_skill(name)) {
        p.add(Some(format!(
            "skill {name:?} is not in the library: add it to the library first, or remove it from skills"
        )));
    }
    p.merge(repeated("skill", &l.skills));

    p.into_result()
}

/// Whether `id` has the shape of an id the library accepts: lowercase letters, digits and single dashes.
pub fn valid_id(id: &str) -> bool {
    id.len() <= MAX_IDENTIFIER_LEN && is_identifier(id)
}

/// Lowercase letters and digits in words joined by single dashes.
fn is_identifier(v: &str) -> bool {
    v.split('-')
        .all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// Letters, digits and underscores.
fn is_tool_name(v: &str) -> bool {
    !v.is_empty() && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Holds a name to a length and a shape. The length is checked first, so a message never repeats a huge value.
fn check_name(what: &str, v: &str, limit: usize, shape: fn(&str) -> bool, rule: &str) -> Option<String> {
    match v.chars().count() {
        0 => Some(format!("{what} is empty: use {rule}")),
        n if n > limit => Some(format!("{what} is {n} characters: use at most {limit}")),
        _ if !shape(v) => Some(format!("{what} {v:?} is not valid: use {rule}")),
        _ => None,
    }
}

fn check_length(what: &str, v: &str, limit: usize) -> Option<String> {
    let n = v.chars().count();
    (n > limit).then(|| format!("{what} is {n} characters: shorten it to at most {limit}"))
}

/// Each name that appears more than once, reported once.
fn repeated(kind: &str, names: &[String]) -> Result<(), Problems> {
    let mut p = Problems::default();
    let mut seen = HashSet::with_capacity(names.len());
    let mut reported = HashSet::new();
    for n in names {
        if !seen.insert(n) && reported.insert(n) {
            p.add(Some(format!("{kind} {n:?} is listed more than once: list it once")));
        }
    }
    p.into_result()
}
