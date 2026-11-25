//! A card's deny rules (`deny_for`) are Claude Code's: it refuses what they name before anything is asked
//! (disallowedTools). `denied_by` reads the same rules against a permission request that reaches Remit all the same, so
//! nobody, the Chair included, can allow what they forbid. Each rule is a
//! matcher of its own, so none is compiled at a call and none can fail.

use agent::EDIT_TOOLS;
use wire::runtime::{ToolCall, ToolKind};

/// How deep a shell's script inside a shell's script is read.
const MAX_SCRIPT_DEPTH: usize = 3;

/// The first of rules that forbids the tool call call, none when none does. It reads the two kinds of rule a card's
/// deny rules hold: a command (`Bash(git push:*)`, a prefix of a command, or the exact command) and an edit tool by
/// name. A request that no rule it reads fits is not forbidden: the policy decides it.
pub fn denied_by<'r>(rules: &'r [String], call: &ToolCall) -> Option<&'r str> {
    let commands = if call.kind == ToolKind::Execute {
        let line = call
            .input
            .get("command")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        commands_of(line, 0)
    } else {
        Vec::new()
    };
    let editing = matches!(call.kind, ToolKind::Edit | ToolKind::Delete | ToolKind::Move);
    rules
        .iter()
        .find(|rule| {
            let (tool, spec) = match rule.split_once('(') {
                Some((tool, spec)) => (tool, Some(spec.strip_suffix(')').unwrap_or(spec))),
                None => (rule.as_str(), None),
            };
            match spec {
                None => editing && EDIT_TOOLS.contains(&tool),
                Some(spec) => tool == "Bash" && commands.iter().any(|c| matches_command_rule(spec, c)),
            }
        })
        .map(String::as_str)
}

/// Whether command is what the spec of a Bash rule names, as Claude Code reads it: * stands for any run of characters
/// and the rest is literal, and a spec that ends in " *" (or its older spelling ":*") also names the command without
/// what follows, so `git push:*` is `git push` and every `git push ...`.
pub fn matches_command_rule(spec: &str, command: &str) -> bool {
    let spec = match spec.strip_suffix(":*") {
        Some(prefix) => format!("{prefix} *"),
        None => spec.to_owned(),
    };
    if let Some(prefix) = spec.strip_suffix(" *")
        && wildcard(prefix, command)
    {
        return true;
    }
    wildcard(&spec, command)
}

/// Whether the whole of command is what spec says, each * any run of characters (newlines too).
fn wildcard(spec: &str, command: &str) -> bool {
    let mut parts = spec.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = command.strip_prefix(first) else {
        return false;
    };
    let middle: Vec<&str> = parts.collect();
    let Some((last, between)) = middle.split_last() else {
        return rest.is_empty();
    };
    for part in between {
        match rest.find(part) {
            Some(at) => rest = rest.get(at.saturating_add(part.len())..).unwrap_or_default(),
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// The simple commands a shell command line runs, so a rule sees each: the line split at its unquoted separators, each
/// part without the assignments and wrappers before it, and a script given to a shell read the same way. A command
/// inside quotes is an argument, not a command.
fn commands_of(line: &str, depth: usize) -> Vec<String> {
    let mut out = Vec::new();
    for part in split_unquoted(line) {
        let part = bare(&part);
        match shell_script(&part) {
            Some(script) if depth < MAX_SCRIPT_DEPTH => out.extend(commands_of(&script, depth.saturating_add(1))),
            _ if !part.is_empty() => out.push(part),
            _ => {}
        }
    }
    out
}

/// line cut at each ; | & and newline that is not in quotes (or escaped), so `&&` and `||` cut too.
fn split_unquoted(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for r in line.chars() {
        if escaped {
            escaped = false;
        } else if r == '\\' && quote != Some('\'') {
            escaped = true;
        } else if let Some(q) = quote {
            if r == q {
                quote = None;
            }
        } else if r == '\'' || r == '"' {
            quote = Some(r);
        } else if matches!(r, ';' | '|' | '&' | '\n') {
            parts.push(std::mem::take(&mut cur));
            continue;
        }
        cur.push(r);
    }
    parts.push(cur);
    parts
}

/// A command without what a shell reads around it: a group's brackets, a negation, assignments and wrappers.
fn bare(part: &str) -> String {
    let mut part = part.to_owned();
    loop {
        let before = part.clone();
        let trimmed = part.trim().trim_start_matches(['(', '{', '!']).trim();
        let trimmed = trimmed.trim_end_matches([')', '}']).trim();
        part = strip_prefix_word(trimmed).to_owned();
        if part == before {
            return part;
        }
    }
}

/// The ASCII spaces.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0c' | '\r')
}

/// The first word of s and what follows the spaces after it; none when no space follows it.
fn first_word(s: &str) -> Option<(&str, &str)> {
    let end = s.find(is_space)?;
    let (word, rest) = s.split_at(end);
    Some((word, rest.trim_start_matches(is_space)))
}

/// part without what stands before the command in it: an assignment (FOO=1) or a word that runs what follows (sudo),
/// with the spaces after it.
fn strip_prefix_word(part: &str) -> &str {
    const WRAPPERS: [&str; 8] = ["sudo", "env", "time", "nohup", "nice", "command", "exec", "builtin"];
    let Some((word, rest)) = first_word(part) else {
        return part;
    };
    if WRAPPERS.contains(&word) || is_assignment(word) {
        rest
    } else {
        part
    }
}

/// Whether word sets a variable: a name, then =.
fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The script of a shell run with one: `bash -c 'git push'`, `/bin/sh -lc "make && git push"`; none for any other
/// command.
fn shell_script(part: &str) -> Option<String> {
    let (shell, mut rest) = first_word(part)?;
    let name = shell.rsplit('/').next().unwrap_or(shell);
    if !matches!(name, "sh" | "bash" | "zsh" | "dash") {
        return None;
    }
    loop {
        let (flag, after) = first_word(rest)?;
        let letters = flag.strip_prefix('-')?;
        if !letters.chars().all(|c| c.is_ascii_lowercase()) {
            return None;
        }
        if letters.ends_with('c')
            && let Some(script) = quoted(after)
        {
            return Some(script);
        }
        if letters.is_empty() {
            return None;
        }
        rest = after;
    }
}

/// What the quotes that are the whole of s hold: single quotes hold no quote, double quotes hold escapes as written.
fn quoted(s: &str) -> Option<String> {
    if let Some(inner) = s.strip_prefix('\'') {
        let inner = inner.strip_suffix('\'')?;
        return (!inner.contains('\'')).then(|| inner.to_owned());
    }
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return None,
            '\\' => match chars.next() {
                Some('\n') | None => return None,
                Some(_) => {}
            },
            _ => {}
        }
    }
    Some(inner.to_owned())
}
