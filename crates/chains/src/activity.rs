//! What an event of an agent's work is, typed for a channel to render (`wire::app::Activity`): a tool call with its group
//! and the words it is shown by, a message the agent sent another through a tool, where a turn ended, a permission prompt
//! asked or decided, a block shown, a reply people marked, the owner starting over. Computed from the event as recorded,
//! here and nowhere else, so every channel shows the same and none reads raw tool input or titles.

use model::{Event, EventStatus, SystemNote};
use serde::Deserialize;
use wire::app::{Activity, Asker, EventData, EventRead, ReplyMark, ToolGroup, ToolState};

const REMIT_TOOL: &str = "mcp__remit__";

/// An event as a channel reads it: the event, and what it is typed as. Every event a channel is answered or pushed is
/// built here, so none reaches one untyped.
pub fn event_read(event: Event) -> EventRead {
    EventRead {
        activity: activity_of(&event),
        event,
    }
}

/// The event typed; none for one that is no step of this kind.
pub fn activity_of(e: &Event) -> Option<Activity> {
    let data = data_of(e);
    match e.status {
        EventStatus::Tool(_) => Some(sent_of(e, &data).unwrap_or_else(|| tool_of(e, &data))),
        EventStatus::Stop(_) => Some(Activity::TurnEnd {
            ms: data.duration_ms,
            turn: e
                .data
                .as_ref()
                .and_then(|d| d.0.get("turn"))
                .and_then(|t| serde_json::from_value(t.clone()).ok()),
        }),
        EventStatus::System(SystemNote::PermissionAsked) => Some(Activity::Asked {
            id: request_id(&data).unwrap_or_default(),
            request: request_of(&e.title),
            of: if data.step.as_deref() == Some("owner") {
                Asker::Owner
            } else {
                Asker::Chair
            },
        }),
        EventStatus::System(SystemNote::PermissionAnswered) => Some(Activity::Decided {
            id: request_id(&data),
            request: request_of(&e.title),
            allowed: data.allow == Some(true),
        }),
        EventStatus::System(SystemNote::Block) => crate::blocks::block_of(e).map(|block| Activity::Block { block }),
        EventStatus::System(SystemNote::NewSession) if crate::assistant::started_over(e) => Some(Activity::StartedOver),
        EventStatus::Message => {
            let marks = marks_of(e);
            (!marks.is_empty()).then_some(Activity::Marked { marks })
        }
        _ => None,
    }
}

/// The id of the permission prompt an event is about: none for a refusal by rule, which no prompt was asked for.
fn request_id(data: &EventData) -> Option<String> {
    data.request.clone().filter(|id| !id.is_empty())
}

/// What e's data holds that a channel reads; empty when it holds none of it.
fn data_of(e: &Event) -> EventData {
    e.data
        .as_ref()
        .and_then(|d| EventData::deserialize(&d.0).ok())
        .unwrap_or_default()
}

/// A field of the call's input, as text.
fn input_of<'a>(data: &'a EventData, field: &str) -> Option<&'a str> {
    data.input.as_ref()?.0.get(field)?.as_str()
}

/// The tool's own name: as recorded, else (a call recorded before names were) its title.
fn tool_name(e: &Event) -> &str {
    if e.tool_name.is_empty() { &e.title } else { &e.tool_name }
}

/// Where a tool call stands as a channel shows it: running until it has ended.
fn status_of(e: &Event) -> ToolState {
    match e.status {
        EventStatus::Tool(model::ToolStatus::Completed) => ToolState::Completed,
        EventStatus::Tool(model::ToolStatus::Failed) => ToolState::Failed,
        _ => ToolState::Running,
    }
}

fn group_of(e: &Event) -> ToolGroup {
    if tool_name(e).starts_with(REMIT_TOOL) {
        return ToolGroup::Remit;
    }
    match e.tool_kind.as_str() {
        "read" => ToolGroup::Read,
        "edit" | "delete" | "move" => ToolGroup::Edit,
        "execute" => ToolGroup::Execute,
        "search" => ToolGroup::Search,
        "fetch" => ToolGroup::Fetch,
        _ => ToolGroup::Other,
    }
}

/// A message the call sent, once the tool took it: a call that failed or has not finished sent nothing.
fn sent_of(e: &Event, data: &EventData) -> Option<Activity> {
    if status_of(e) != ToolState::Completed {
        return None;
    }
    let field = |k: &str| input_of(data, k).map_or_else(String::new, |s| s.trim().to_owned());
    match tool_name(e).strip_prefix(REMIT_TOOL)? {
        "route" => {
            let to = field("to_card");
            if to.is_empty() {
                return None;
            }
            let kind = field("kind");
            let label = field("label");
            Some(Activity::Sent {
                to: Some(to),
                label: if label.is_empty() { kind.clone() } else { label },
                kind,
                text: field("message"),
            })
        }
        "ask_chair" => {
            let question = field("question");
            Some(Activity::Sent {
                to: None,
                kind: "question".to_owned(),
                label: question.clone(),
                text: question,
            })
        }
        _ => None,
    }
}

/// Claude Code's verbs that lead a read's or an edit's title ("Read /a/b.go"): the object is what is shown.
const VERBS: [&str; 8] = ["Read", "Edit", "Write", "MultiEdit", "Grep", "Glob", "Search", "Fetch"];

fn tool_of(e: &Event, data: &EventData) -> Activity {
    let group = group_of(e);
    let status = status_of(e);
    let tool = |name: &str, summary: &str| Activity::Tool {
        group,
        name: name.to_owned(),
        summary: summary.to_owned(),
        status,
    };
    let title = e.title.as_str();
    match group {
        ToolGroup::Remit => tool(tool_name(e).trim_start_matches(REMIT_TOOL), "Remit"),
        ToolGroup::Execute => {
            let input = |k: &str| input_of(data, k);
            let (name, args) = command(input("command").unwrap_or_else(|| unquoted(title)));
            // The agent's own words for what the command does read better than its arguments; the command stays in
            // the call's input, one click away.
            let summary = input("description").map(str::trim).filter(|d| !d.is_empty());
            tool(&name, summary.unwrap_or(&args))
        }
        ToolGroup::Other => {
            if let Some((server, name)) = mcp_tool(tool_name(e)) {
                return tool(name, &format!("MCP {server}"));
            }
            let mut words = title.split_whitespace();
            let name = words.next().unwrap_or("tool");
            tool(name, &words.collect::<Vec<_>>().join(" "))
        }
        ToolGroup::Read | ToolGroup::Edit | ToolGroup::Search | ToolGroup::Fetch => {
            let name = match group {
                ToolGroup::Read => "read",
                ToolGroup::Edit => "edit",
                ToolGroup::Search => "search",
                _ => "fetch",
            };
            tool(name, without_verb(title))
        }
    }
}

/// The server and the tool of an MCP tool's name as Claude Code gives it (`mcp__<server>__<tool>`).
fn mcp_tool(name: &str) -> Option<(&str, &str)> {
    name.strip_prefix("mcp__")?
        .split_once("__")
        .filter(|(server, tool)| !server.is_empty() && !tool.is_empty())
}

/// A title as Claude Code wraps a command in it (`` `go test` ``), without the backticks.
fn unquoted(title: &str) -> &str {
    title
        .strip_prefix('`')
        .and_then(|t| t.strip_suffix('`'))
        .unwrap_or(title)
}

fn without_verb(title: &str) -> &str {
    VERBS
        .iter()
        .find_map(|v| {
            let rest = title.strip_prefix(v)?;
            let object = rest.trim_start();
            (object.len() < rest.len()).then_some(object)
        })
        .unwrap_or(title)
}

/// What a permission prompt asked, as the command that matters in it: a bare program is its own summary.
fn request_of(title: &str) -> String {
    let (name, summary) = command(title);
    if summary == name {
        name
    } else {
        format!("{name} {summary}")
    }
}

/// Commands that only set the stage for the one that does the work ("cd repo && go test").
const SETUP: [&str; 10] = [
    "cd", "pushd", "popd", "export", "set", "unset", "source", ".", "true", ":",
];
/// Prefixes that run another program; the program is what matters.
const WRAPPERS: [&str; 7] = ["sudo", "env", "time", "nohup", "exec", "command", "nice"];

/// A shell command named by the program doing the work (glab, go, make) rather than "bash", and summed up as that
/// program's arguments, without the cd that usually leads in: a best-effort reading, for the owner's eye only.
pub fn command(cmd: &str) -> (String, String) {
    let mut runs: Vec<(String, String)> = Vec::new();
    for (piece, start) in pieces(cmd) {
        let ws = words(piece, start);
        let Some(at) = program_at(&ws) else { continue };
        let Some(prog) = ws.get(at) else { continue };
        let name = prog.text.rsplit('/').next().unwrap_or(&prog.text).to_owned();
        let rest = ws.get(at.saturating_add(1));
        // bash -c "go test ./..." runs what is in the string
        if is_shell(&name)
            && rest.is_some_and(|r| is_dash_c(&r.text))
            && let Some(inner) = ws.get(at.saturating_add(2))
        {
            let inner = command(&inner.text);
            if inner.0 != "bash" {
                return inner;
            }
        }
        let from = rest.map_or(prog.start, |r| r.start);
        let summary = cmd.get(from..).unwrap_or_default().trim().to_owned();
        runs.push((name, summary));
    }
    let main = runs
        .iter()
        .position(|(name, _)| !SETUP.contains(&name.as_str()))
        .unwrap_or(0);
    if main < runs.len() {
        return runs.swap_remove(main);
    }
    ("bash".to_owned(), cmd.trim().to_owned())
}

fn is_shell(name: &str) -> bool {
    matches!(name, "sh" | "bash" | "zsh")
}

/// `-c`, or flags that end in it (`-lc`).
fn is_dash_c(word: &str) -> bool {
    word.strip_prefix('-')
        .and_then(|w| w.strip_suffix('c'))
        .is_some_and(|flags| flags.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

struct Word {
    text: String,
    /// Where it starts in the whole command, in bytes.
    start: usize,
}

/// One simple command's words, quoted parts kept together and the quotes dropped.
fn words(s: &str, base: usize) -> Vec<Word> {
    let mut out = Vec::new();
    let mut cur: Option<Word> = None;
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        if quote.is_none() && c.is_whitespace() {
            out.extend(cur.take());
            continue;
        }
        let w = cur.get_or_insert_with(|| Word {
            text: String::new(),
            start: base.saturating_add(i),
        });
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => w.text.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None => w.text.push(c),
        }
    }
    out.extend(cur);
    out
}

/// A command line's simple commands, cut at unquoted `&&`, `||`, `;`, `|` and newlines, each with where it starts.
fn pieces(cmd: &str) -> Vec<(&str, usize)> {
    let mut out = Vec::new();
    let mut quote: Option<char> = None;
    let mut start = 0;
    let mut chars = cmd.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == '\\' && q == '"' {
                chars.next();
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '\\' => {
                chars.next();
            }
            ';' | '\n' | '|' | '&' => {
                let doubled = chars.peek().is_some_and(|&(_, n)| n == c);
                if c == '&' && !doubled {
                    continue;
                }
                out.push((cmd.get(start..i).unwrap_or_default(), start));
                start = i.saturating_add(1);
                if doubled && (c == '|' || c == '&') {
                    chars.next();
                    start = start.saturating_add(1);
                }
            }
            _ => {}
        }
    }
    out.push((cmd.get(start..).unwrap_or_default(), start));
    out.retain(|(p, _)| !p.trim().is_empty());
    out
}

/// Where the program a simple command runs is among its words: past variable assignments, flags and wrappers like sudo
/// or timeout (and its duration).
fn program_at(ws: &[Word]) -> Option<usize> {
    let mut i = 0;
    while let Some(w) = ws.get(i) {
        let w = w.text.as_str();
        if is_assignment(w) || WRAPPERS.contains(&w) || w.starts_with('-') {
            i = i.saturating_add(1);
        } else if w == "timeout" {
            i = i.saturating_add(2);
        } else {
            return Some(i);
        }
    }
    None
}

fn is_assignment(w: &str) -> bool {
    let Some((name, _)) = w.split_once('=') else {
        return false;
    };
    let mut cs = name.chars();
    cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The marks on reply e as they stand, one per person; none on any other event. Pure.
pub fn marks_of(e: &Event) -> Vec<ReplyMark> {
    e.data
        .as_ref()
        .and_then(|d| d.0.get("marks"))
        .and_then(|marks| serde_json::from_value(marks.clone()).ok())
        .unwrap_or_default()
}

/// What an agent calling one of Remit's tools is doing, by tool; a test holds it to every tool the server gives.
pub fn tool_doing(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "hire_agent" => "Hiring an agent…",
        "route" => "Routing work between agents…",
        "ask_owner" => "Drafting a question for you…",
        "withdraw_question" => "Taking back its question…",
        "ask_chair" => "Asking the Chair…",
        "finish_chain" => "Wrapping up the chain…",
        "state_check" => "Stating how the result will be checked…",
        "set_project" => "Naming the product the chain is about…",
        "set_title" => "Titling the chain…",
        "list_projects" => "Reading the projects…",
        "read_project" => "Reading a project…",
        "create_project" => "Making a project…",
        "update_project" => "Rewriting a project…",
        "archive_project" => "Archiving a project…",
        "restore_project" => "Bringing a project back…",
        "open_verification" => "Opening a verification chain…",
        "list_agents" => "Checking on the agents…",
        "read_output" => "Reading a report…",
        "stop_agent" => "Stopping an agent…",
        "accept_work" => "Accepting a report…",
        "run_workflow" => "Starting a workflow…",
        "list_workflows" => "Checking on its workflows…",
        "stop_workflow" => "Stopping a workflow…",
        "answer_permission" => "Answering a permission request…",
        "update_plan" => "Updating the plan…",
        "send_feedback" => "Writing feedback for the owner…",
        "record_verdict" => "Recording a review verdict…",
        "land" => "Landing the change…",
        "open_repo" => "Opening a repo…",
        "subscribe" => "Planning to check back later…",
        "create_sensor" => "Making a sensor to read what it waits on…",
        "resume_subscription" => "Retrying a held subscription…",
        "skip_firing" => "Giving up a failed firing…",
        "list_chains" => "Reading past chains…",
        "read_chain" => "Reading a past chain…",
        "list_issues" => "Reading customer-reported bugs…",
        "propose_chain" => "Proposing a chain…",
        "refine_key_result" => "Refining a key result…",
        "lift_key_result_stall" => "Lifting a stalled key result…",
        "list_changes" => "Reading what was changed…",
        "read_change" => "Reading a change…",
        "list_playbooks" => "Reading the playbooks…",
        "read_playbook" => "Reading a playbook…",
        "create_playbook" => "Writing a playbook…",
        "update_playbook" => "Rewriting a playbook…",
        "delete_playbook" => "Deleting a playbook…",
        "reset_playbook" => "Resetting a playbook…",

        "list_agent_definitions" => "Reading the agents…",
        "read_agent_definition" => "Reading an agent…",
        "create_agent_definition" => "Writing an agent…",
        "update_agent_definition" => "Rewriting an agent…",
        "delete_agent_definition" => "Deleting an agent…",
        "reset_agent_definition" => "Resetting an agent…",

        "list_library" => "Reading the library…",
        "list_sources" => "Reading the sources…",
        "request_source_credentials" => "Asking you for a credential…",
        "request_grant" => "Asking you for a grant…",
        "request_capability" => "Asking you for a capability…",
        "grant_capability" => "Giving the chain a capability…",
        "set_connection_protected" => "Marking a connection…",
        "read_skill" => "Reading a skill…",
        "list_importable" => "Looking at what can be imported…",
        "create_skill" => "Writing a skill…",
        "update_skill" => "Rewriting a skill…",
        "edit_skill_file" => "Editing a skill…",
        "delete_skill" => "Deleting a skill…",
        "import_skill" => "Importing a skill…",
        "import_agent_definition" => "Importing an agent…",
        "my_work" => "Looking back at its earlier work…",
        "read_my_card" => "Reading one of its earlier cards…",
        "search_memory" => "Recalling what it learned…",
        "rate_memory" => "Rating a memory…",
        "next_teachings" | "read_teaching" | "recall_for" => "Reading what was taught…",
        "remember" | "mark_retaught" | "link_memories" | "retract_memory" | "settle_teaching" => "Keeping memory…",
        "name_yourself" => "Choosing a name…",
        "state_strengths" => "Saying what it is good at…",
        "hand_over" => "Handing work over…",
        "list_mentors" => "Looking through the mentors…",
        "bring_mentor" => "Bringing a mentor onto a card…",
        "escalate_permission" => "Passing a permission request to you…",
        _ => return None,
    })
}
