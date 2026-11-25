use crate::Invalid;

/// The group of an agent, a playbook or a skill that belongs to no kind of work the others name.
pub const GROUP_GENERAL: &str = "general";

const MAX_GROUP_LEN: usize = 30;

/// The group an agent, a playbook or a skill is stored under: the kind of work it belongs to, in lowercase words
/// ("engineering", "customer support"). Case and spacing are normalised, so one group is never two columns; empty is
/// GROUP_GENERAL. Whoever writes a record parses the group once, and everything after reads it as it is.
pub fn parse_group(s: &str) -> Result<String, Invalid> {
    // Each character's own lowercase: the first of what Unicode gives (only İ has more), so the length never grows.
    let lower: String = s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect();
    let group = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = group.chars().count();
    if n == 0 {
        return Ok(GROUP_GENERAL.to_owned());
    }
    if n > MAX_GROUP_LEN {
        return Err(Invalid::new(format!(
            "group is {n} characters: use at most {MAX_GROUP_LEN}"
        )));
    }
    // The words are already joined by one space each, so the group is sound when nothing else stands between them.
    if !group
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == ' ')
    {
        return Err(Invalid::new(format!(
            "group {s:?} is not valid: use lowercase letters and digits in one or a few words, like engineering or customer support"
        )));
    }
    Ok(group)
}
