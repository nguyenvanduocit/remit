//! A permission prompt in words (D4 of docs/research-agentes-chat/report.md): the action as a title, one sentence of why
//! it is asked, what about it is risky as typed traits, how careful to be, and the exact call kept apart, all read off
//! the call and the rules when it is asked; and once it no longer waits, one line of how it ended. Pure: no clock, no
//! store, no model. A call's own description is the only words its asker gave: kept apart as the asker's, and never
//! read for the title or a trait, since an asker may word a call as anything.

use model::{
    Actor, ActorKind, PermissionAsk, PermissionOutcome, PermissionRisk, PermissionRiskSaid, PermissionTrait,
    PermissionTraitSaid,
};
use serde_json::{Map, Value};

use crate::policy::{paths_in, secret_path};

/// The longest title: a command used as one is cut here.
const TITLE_CHARS: usize = 100;

/// What a call asked for over `can_use_tool` is, in words.
pub struct Call<'a> {
    /// Claude Code's name for the tool.
    pub tool: &'a str,
    pub input: &'a Map<String, Value>,
    /// The stream's title of the call (`wire::runtime::ToolCall::title`).
    pub title: &'a str,
    /// Why Claude Code asks, in its words; often empty.
    pub reason: &'a str,
    /// The auto-mode classifier's reason when the call is one it refused (ADR-0083).
    pub classifier: &'a str,
    /// The session's working directory.
    pub cwd: &'a str,
}

/// Call c as the one deciding reads it, still open.
pub fn ask_of(c: &Call<'_>) -> PermissionAsk {
    let traits = traits_of(c);
    PermissionAsk {
        action: action_of(c),
        said: said_of(c),
        reason: reason_of(c, &traits),
        risk: said_risk(risk_of(c.tool, &traits)),
        tool: c.tool.to_owned(),
        command: command_of(c.tool, c.input),
        traits: traits.into_iter().map(said_trait).collect(),
        ..PermissionAsk::default()
    }
}

/// The text of an ask the Chair and the Assistant read: why, what is risky, then the call's input as sent.
pub fn detail_of(a: &PermissionAsk, input: &Map<String, Value>) -> String {
    let mut out = a.reason.clone();
    if !a.traits.is_empty() {
        let words: Vec<&str> = a.traits.iter().map(|t| t.says.as_str()).collect();
        out.push_str(&format!("\n{}: {}.", a.risk.says, words.join(", ")));
    }
    if !input.is_empty() {
        out.push('\n');
        out.push_str(&serde_json::to_string_pretty(input).unwrap_or_default());
    }
    out
}

/// Ask a as it ended: allowed or refused by by, the owner reading.
pub fn decided(mut a: PermissionAsk, allow: bool, by: &Actor) -> PermissionAsk {
    let (outcome, verb) = if allow {
        (PermissionOutcome::Allowed, "Allowed")
    } else {
        (PermissionOutcome::Refused, "Refused")
    };
    a.decided = format!("{verb} by {}", decider(by, "you"));
    a.outcome = Some(outcome);
    a
}

/// Ask a as it ended with nobody deciding, for why.
pub fn withdrawn(mut a: PermissionAsk, why: &str) -> PermissionAsk {
    a.decided = format!("Withdrawn: {why}");
    a.outcome = Some(PermissionOutcome::Withdrawn);
    a
}

/// Who decided a prompt, owner being how the reader names the owner ("you" to him, "the owner" to the chain). Of the
/// agents only the Chair above the session decides (`may_answer`); the asker never does.
pub fn decider(by: &Actor, owner: &str) -> String {
    match by.kind {
        ActorKind::Owner => owner.to_owned(),
        ActorKind::Assistant => format!("the Assistant, for {owner}"),
        ActorKind::Remit => "a rule".to_owned(),
        ActorKind::Agent => "the Chair".to_owned(),
        ActorKind::Client => "an outside agent".to_owned(),
        ActorKind::Mentor => "a mentor".to_owned(),
    }
}

/// The action in words, read off the call itself: a command cut to one line, the tool and its server, or what the stream
/// titles it. Never the agent's own description of it: a call titled by its asker could read as anything ("Clean temp
/// files" over `rm -rf ~`), so the asker's words stand apart, as its own (`said_of`).
fn action_of(c: &Call<'_>) -> String {
    if let Some((server, tool)) = c.tool.strip_prefix("mcp__").and_then(|t| t.split_once("__")) {
        return format!("Use {} ({server})", tool.replace('_', " "));
    }
    match c.tool {
        "Bash" | "PowerShell" => match str_of(c.input, "command").lines().next().unwrap_or_default().trim() {
            "" => "Run a command".to_owned(),
            line => format!("Run {}", cut(line)),
        },
        // the stream titles a subagent by its asker's description
        "Agent" | "Task" => "Start a subagent".to_owned(),
        _ if !c.title.trim().is_empty() => cut(c.title.trim()),
        tool => tool.to_owned(),
    }
}

/// What the agent says the call does, in its own words, or nothing; shown beside the action as the agent's, never as it.
fn said_of(c: &Call<'_>) -> String {
    cut(str_of(c.input, "description").trim())
}

/// One sentence of why the call waits: the classifier that stopped it, else what about it is risky, else Claude Code's
/// own reason, else that no rule lets it run.
fn reason_of(c: &Call<'_>, traits: &[PermissionTrait]) -> String {
    let classifier = c.classifier.trim().trim_start_matches('[').trim_end_matches(']');
    if !classifier.is_empty() {
        return format!("Claude Code's safety check stopped it ({classifier}), so it runs only if it is allowed.");
    }
    if let Some((first, rest)) = traits.split_first() {
        let mut why = format!("It {}", trait_clause(first));
        if let Some(second) = rest.first() {
            why.push_str(&format!(" and {}", trait_clause(second)));
        }
        return why + ", so it runs only if it is allowed.";
    }
    match c.reason.trim().trim_end_matches('.') {
        "" => format!(
            "No rule lets {} run unasked here, so it runs only if it is allowed.",
            c.tool
        ),
        said => format!("{said}."),
    }
}

/// A trait as a clause after "It", the most serious first in `traits_of`'s order.
fn trait_clause(t: &PermissionTrait) -> &'static str {
    match t {
        PermissionTrait::Secrets => "names a secret",
        PermissionTrait::Irreversible => "removes what cannot be brought back",
        PermissionTrait::Production => "touches production",
        PermissionTrait::Unsandboxed => "asks to run outside the sandbox",
        PermissionTrait::OutsideWorktree => "writes outside its working directory",
        PermissionTrait::Network => "reaches another machine",
    }
}

/// A trait with the words its badge reads.
fn said_trait(kind: PermissionTrait) -> PermissionTraitSaid {
    let says = match kind {
        PermissionTrait::Secrets => "secrets",
        PermissionTrait::Irreversible => "cannot be undone",
        PermissionTrait::Production => "production",
        PermissionTrait::Unsandboxed => "outside the sandbox",
        PermissionTrait::OutsideWorktree => "writes outside the worktree",
        PermissionTrait::Network => "network",
    };
    PermissionTraitSaid {
        kind,
        says: says.to_owned(),
    }
}

/// How careful to be, with the words its badge reads.
fn said_risk(kind: PermissionRisk) -> PermissionRiskSaid {
    let says = match kind {
        PermissionRisk::High => "High risk",
        PermissionRisk::Medium => "Medium risk",
        PermissionRisk::Low => "Low risk",
    };
    PermissionRiskSaid {
        kind,
        says: says.to_owned(),
    }
}

/// High is what cannot be undone, a secret given away included; low is a read with nothing risky about it.
fn risk_of(tool: &str, traits: &[PermissionTrait]) -> PermissionRisk {
    if traits.contains(&PermissionTrait::Irreversible) || traits.contains(&PermissionTrait::Secrets) {
        PermissionRisk::High
    } else if traits.is_empty() && matches!(tool, "Read" | "Glob" | "Grep" | "LS" | "NotebookRead" | "TodoWrite") {
        PermissionRisk::Low
    } else {
        PermissionRisk::Medium
    }
}

/// The exact call: a command as typed, the file or address it is about, else its whole input.
fn command_of(tool: &str, input: &Map<String, Value>) -> String {
    let key = match tool {
        "Bash" | "PowerShell" => "command",
        "Read" | "Write" | "Edit" | "MultiEdit" => "file_path",
        "NotebookEdit" => "notebook_path",
        "WebFetch" => "url",
        _ => "",
    };
    match str_of(input, key) {
        "" if input.is_empty() => String::new(),
        "" => serde_json::to_string_pretty(input).unwrap_or_default(),
        said => said.to_owned(),
    }
}

/// What about call c is risky, the most serious first.
fn traits_of(c: &Call<'_>) -> Vec<PermissionTrait> {
    let command = str_of(c.input, "command");
    let path = ["file_path", "notebook_path", "path"]
        .into_iter()
        .map(|k| str_of(c.input, k))
        .find(|p| !p.is_empty())
        .unwrap_or_default();
    // what the call does, never what its asker says it does
    let said = [command, path, str_of(c.input, "url")].join(" ");
    let checks = [
        (PermissionTrait::Secrets, names_secret(&said)),
        (PermissionTrait::Irreversible, irreversible(command)),
        (PermissionTrait::Production, names_production(&said)),
        (
            PermissionTrait::Unsandboxed,
            c.input.get("dangerouslyDisableSandbox").and_then(Value::as_bool) == Some(true),
        ),
        (
            PermissionTrait::OutsideWorktree,
            writes_outside(c.tool, command, path, c.cwd),
        ),
        (
            PermissionTrait::Network,
            matches!(c.tool, "WebFetch" | "WebSearch") || reaches_out(command),
        ),
    ];
    checks.into_iter().filter(|(_, on)| *on).map(|(t, _)| t).collect()
}

/// Whether text names a secret: a secret's path (the rail's own test, `policy::secret_path`), a variable holding one, or
/// a keychain read.
fn names_secret(text: &str) -> bool {
    paths_in(text).any(|w| {
        let name = w.trim_start_matches('$').trim_start_matches('{');
        secret_path(w)
            || name
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
                && ["TOKEN", "SECRET", "PASSWORD", "API_KEY"]
                    .iter()
                    .any(|s| name.contains(s))
            || matches!(w, "find-generic-password" | "find-internet-password")
    })
}

/// Whether text names production: a word prod, prd or production, wherever it stands (prod-db, --context=production).
fn names_production(text: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|w| matches!(w.to_ascii_lowercase().as_str(), "prod" | "prd" | "production"))
}

/// The shell commands and statements whose effect does not come back: deleting files, rows, tables or resources,
/// overwriting a history. Matched on the command's words, spaced as one.
const IRREVERSIBLE: [&str; 20] = [
    " rm ",
    " shred ",
    " mkfs",
    " dd ",
    " delete from ",
    " drop table ",
    " drop database ",
    " drop schema ",
    " truncate ",
    " push --force",
    " push -f ",
    " reset --hard",
    " clean -f",
    " kubectl delete ",
    " helm uninstall ",
    " terraform destroy",
    " s3 rm ",
    " flushall",
    " flushdb",
    " repo delete ",
];

/// Whether a shell command removes or overwrites what cannot be brought back.
fn irreversible(command: &str) -> bool {
    if command.is_empty() {
        return false;
    }
    let spaced = format!(" {} ", words(command));
    IRREVERSIBLE.iter().any(|w| spaced.contains(w)) || spaced.contains(" aws ") && spaced.contains(" delete-")
}

/// The programs that reach another machine, as a command's word.
const NETWORK: [&str; 18] = [
    "curl",
    "wget",
    "ssh",
    "scp",
    "sftp",
    "rsync",
    "nc",
    "telnet",
    "gh",
    "glab",
    "aws",
    "gcloud",
    "az",
    "kubectl",
    "helm",
    "terraform",
    "argocd",
    "psql",
];

/// Whether a shell command reaches another machine: a network program, an address, or git or a package manager talking
/// to a remote.
fn reaches_out(command: &str) -> bool {
    if command.is_empty() {
        return false;
    }
    let spaced = format!(" {} ", words(command));
    command.contains("://")
        || NETWORK.iter().any(|p| spaced.contains(&format!(" {p} ")))
        || [
            " git push",
            " git pull",
            " git fetch",
            " git clone",
            " npm publish",
            " docker push",
            " docker pull",
        ]
        .iter()
        .any(|w| spaced.contains(w))
}

/// The shell commands that write where their paths say.
const WRITERS: [&str; 10] = [
    "rm", "mv", "cp", "mkdir", "touch", "chmod", "chown", "ln", "tee", "install",
];

/// Whether a call writes outside cwd: an edit of a file there, or a shell command that writes (a writer, a redirect) and
/// names an absolute path there.
fn writes_outside(tool: &str, command: &str, path: &str, cwd: &str) -> bool {
    if cwd.is_empty() {
        return false;
    }
    let outside = |p: &str| (p.starts_with('/') || p.starts_with('~')) && !p.starts_with("/dev/") && !within(p, cwd);
    match tool {
        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => outside(path),
        "Bash" | "PowerShell" => {
            let writes = command.contains('>') || paths_in(command).any(|w| WRITERS.contains(&w));
            writes && paths_in(command).any(outside)
        }
        _ => false,
    }
}

/// Whether path p is dir or inside it.
fn within(p: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches('/');
    p == dir || p.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'))
}

/// text lowercased, its words split where a shell splits them and joined by one space.
fn words(text: &str) -> String {
    paths_in(&text.to_lowercase()).collect::<Vec<_>>().join(" ")
}

fn str_of<'a>(input: &'a Map<String, Value>, key: &str) -> &'a str {
    input.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// s cut to `TITLE_CHARS` characters.
fn cut(s: &str) -> String {
    if s.chars().count() <= TITLE_CHARS {
        return s.to_owned();
    }
    s.chars().take(TITLE_CHARS).collect::<String>() + "…"
}
