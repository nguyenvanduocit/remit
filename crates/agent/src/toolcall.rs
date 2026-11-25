use std::path::Path;

use model::ToolStatus;
use serde_json::{Map, Value};
use wire::runtime::ToolKind;

/// The kind and title of a use of tool name with input, in a session whose working directory is cwd: what the stream
/// shows of it. A tool this does not know (an MCP tool among them) is titled by its name.
pub(crate) fn describe(name: &str, input: &Map<String, Value>, cwd: &str) -> (ToolKind, String) {
    let str_of = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or_default();
    let path = |key: &str| {
        let p = str_of(key);
        if p.is_empty() {
            String::new()
        } else {
            display_path(p, cwd)
        }
    };
    match name {
        "Bash" | "PowerShell" => (ToolKind::Execute, or(str_of("command"), "Terminal")),
        "Read" => {
            let p = path("file_path");
            (ToolKind::Read, format!("Read {}{}", or(&p, "File"), read_range(input)))
        }
        "Write" => (
            ToolKind::Edit,
            prefixed("Write ", &path("file_path"), "Preparing file…"),
        ),
        "Edit" | "MultiEdit" => (ToolKind::Edit, prefixed("Edit ", &path("file_path"), "Edit")),
        "NotebookEdit" => (
            ToolKind::Edit,
            prefixed("Edit notebook ", &path("notebook_path"), "Edit notebook"),
        ),
        "Glob" => {
            let mut title = "Find".to_string();
            for key in ["path", "pattern"] {
                if !str_of(key).is_empty() {
                    title += &format!(" `{}`", str_of(key));
                }
            }
            (ToolKind::Search, title)
        }
        "Grep" => {
            let mut title = "grep".to_string();
            if !str_of("pattern").is_empty() {
                title += &format!(" \"{}\"", str_of("pattern"));
            }
            if !str_of("path").is_empty() {
                title += &format!(" {}", str_of("path"));
            }
            (ToolKind::Search, title)
        }
        "WebFetch" => (ToolKind::Fetch, prefixed("Fetch ", str_of("url"), "Fetch")),
        "WebSearch" => match str_of("query") {
            "" => (ToolKind::Fetch, "Web search".into()),
            q => (ToolKind::Fetch, format!("Search \"{q}\"")),
        },
        "Agent" | "Task" => (ToolKind::Think, or(str_of("description"), "Task")),
        "TodoWrite" => (ToolKind::Think, "Update TODOs".into()),
        "ExitPlanMode" => (ToolKind::SwitchMode, "Approve Plan".into()),
        "Skill" => (ToolKind::Other, prefixed("Load skill: ", str_of("skill"), "Load skill")),
        _ => (ToolKind::Other, or(name, "Unknown Tool")),
    }
}

/// The lines a Read asks for, as its title says them.
fn read_range(input: &Map<String, Value>) -> String {
    let num = |key: &str| input.get(key).and_then(Value::as_i64).unwrap_or_default();
    let (offset, limit) = (num("offset"), num("limit"));
    if limit > 0 {
        let from = offset.max(1);
        return format!(" ({} - {})", from, from.saturating_add(limit).saturating_sub(1));
    }
    if offset > 0 {
        return format!(" (from line {})", offset);
    }
    String::new()
}

/// p relative to cwd when it is inside it, else p.
pub(crate) fn display_path(p: &str, cwd: &str) -> String {
    if cwd.is_empty() {
        return p.to_string();
    }
    match Path::new(p).strip_prefix(cwd) {
        Ok(rel) if rel.as_os_str().is_empty() => ".".into(),
        Ok(rel) => rel.display().to_string(),
        Err(_) => p.to_string(),
    }
}

fn prefixed(prefix: &str, s: &str, fallback: &str) -> String {
    if s.is_empty() {
        fallback.to_string()
    } else {
        format!("{prefix}{s}")
    }
}

fn or(s: &str, fallback: &str) -> String {
    if s.is_empty() { fallback } else { s }.to_string()
}

/// The plan a TodoWrite call sets: each todo, the one in progress by what it is doing.
pub(crate) fn plan_of(input: &Map<String, Value>) -> Vec<model::PlanEntry> {
    let todos = input
        .get("todos")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    todos
        .iter()
        .map(|todo| {
            let field = |key: &str| todo.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
            let (mut content, status, active) = (field("content"), field("status"), field("activeForm"));
            if status == ToolStatus::InProgress.as_str() && !active.is_empty() {
                content = active;
            }
            model::PlanEntry {
                content,
                status,
                priority: "medium".into(),
            }
        })
        .collect()
}
