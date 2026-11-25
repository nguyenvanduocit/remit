//! What an agent is given of its memory while it works (ADR-0117), beside what each turn's message is given
//! (`recall_for_turn`): after a call of a tool that names what it touches (a file read, edited or searched, a command
//! run), the memories about those subjects; as a subagent starts, what bears on the task it was spawned for. Each is
//! judged by the reranker as a turn's recall is, and counted as recalled only once its note has reached the agent
//! (`count_given`). Pure: `subjects_of`.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use model::{Card, Memory};
use serde_json::{Map, Value};
use wire::runtime::Moment;

use crate::Crew;
use crate::memory::passage_of;
use crate::recall::{self, Candidate, Purpose};
use chains::Result;

/// How many memories a tool call is given at most: it adds to every call's result, so only the few that bear on it.
const TOOL_RECALL: usize = 3;

/// How many memories a subagent's start is given at most, and how much of its task its recall reads, in characters.
const START_RECALL: usize = 5;
const START_QUERY_CHARS: usize = 2000;

/// How long the judge of a tool call's candidates is waited for: past it, they are ranked with no judge, before the
/// core's bound on the whole recall (`chains::recall::RECALL_WITHIN`) gives up on them.
const JUDGE_WITHIN: Duration = Duration::from_millis(2000);

/// The commands a shell line runs another through, which are not what it is about.
const RUNNERS: [&str; 5] = ["sudo", "env", "time", "nohup", "exec"];

/// The subjects (`model::subject_of`) a call of tool with input touches, for a card working in cwd: `file:` and `crate:`
/// for a path it reads, edits or searches (repository-relative; `crate:<name>` under `crates/<name>/`), `tool:` for each
/// command a shell line runs. None for a tool that names none. Pure.
pub fn subjects_of(tool: &str, input: &Map<String, Value>, cwd: &str) -> Vec<String> {
    let str_of = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    let mut add = |raw: String| {
        if let Some(s) = model::subject_of(&raw)
            && !out.contains(&s)
        {
            out.push(s);
        }
    };
    match tool {
        "Read" | "Edit" | "Write" | "MultiEdit" | "Grep" | "Glob" => {
            let key = if matches!(tool, "Grep" | "Glob") {
                "path"
            } else {
                "file_path"
            };
            if let Some(path) = in_repo(str_of(key), cwd) {
                if let Some(name) = path.strip_prefix("crates/").and_then(|p| p.split('/').next()) {
                    add(format!("crate:{name}"));
                }
                add(format!("file:{path}"));
            }
        }
        "Bash" => {
            for head in command_heads(str_of("command")) {
                add(format!("tool:{head}"));
            }
        }
        _ => {}
    }
    out
}

/// path relative to the repository at cwd: none when it is empty or outside it.
fn in_repo(path: &str, cwd: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let rel = match path.strip_prefix('/') {
        Some(_) => {
            let root = cwd.trim_end_matches('/');
            path.strip_prefix(root)?.strip_prefix('/')?
        }
        None => path,
    };
    let rel = rel.trim_start_matches("./").trim_end_matches('/');
    (!rel.is_empty() && rel != ".").then(|| rel.to_owned())
}

/// The programs a shell line runs, each once, in order: the first word of each command of it, without its directory,
/// past variables it sets and the runners it goes through; a change of directory runs none.
fn command_heads(line: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in line.split(['\n', ';', '|', '&']) {
        let head = part
            .split_whitespace()
            .find(|w| !w.contains('=') && !RUNNERS.contains(w))
            .filter(|w| *w != "cd")
            .map(|w| w.rsplit('/').next().unwrap_or(w).to_owned());
        if let Some(h) = head.filter(|h| h.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)))
            && !out.contains(&h)
        {
            out.push(h);
        }
    }
    out
}

/// What the note given at moment m says its memories bear on, read before them (`recall_note`). Pure.
pub fn lead_of(m: &Moment) -> &'static str {
    match m {
        Moment::ToolUsed { .. } => "What you remember about what this tool call touched, from earlier work.",
        Moment::Started => "What you remember that bears on the task you were given, from earlier work.",
    }
}

impl Crew {
    /// The memories card c's agent is given at moment m of its work, none of given, best first: none for a card that
    /// remembers nothing, or when nothing bears on it. Not counted yet (`count_given`).
    pub async fn recall_at(&self, c: &Card, m: &Moment, given: &HashSet<String>) -> Result<Vec<Memory>> {
        let Some(reader) = self.reader_of(c).await? else {
            return Ok(Vec::new());
        };
        match m {
            Moment::ToolUsed { tool, input } => {
                let subjects = subjects_of(tool, input, &c.cwd);
                if subjects.is_empty() {
                    return Ok(Vec::new());
                }
                let about = subjects.clone();
                let readable = self.store.read(move |r| r.memories_about(&reader, &about)).await?;
                let targets = subjects.join(", ");
                let query = format!("about to {tool} {targets}: {}", c.name);
                Ok(self.about(readable, &subjects, &query, given).await)
            }
            Moment::Started => {
                let query: String = format!("{}\n{}", c.name, c.brief)
                    .chars()
                    .take(START_QUERY_CHARS)
                    .collect();
                self.search_memory(reader, &query, Purpose::Turn, START_RECALL, given)
                    .await
            }
        }
    }

    /// Of readable, the memories about one of subjects and not of given that the judge finds bear on query, best first,
    /// at most `TOOL_RECALL`; with no judge (none, it failed, or it took longer than `JUDGE_WITHIN`), those about them,
    /// which a direct subject stands behind.
    async fn about(
        &self,
        readable: Vec<Memory>,
        subjects: &[String],
        query: &str,
        given: &HashSet<String>,
    ) -> Vec<Memory> {
        let now = self.now();
        let readable: HashMap<String, Memory> = readable
            .into_iter()
            .filter(|m| !given.contains(&m.id) && m.about.iter().any(|a| subjects.contains(a)))
            .map(|m| (m.id.clone(), m))
            .collect();
        if readable.is_empty() {
            return Vec::new();
        }
        let mut candidates: Vec<Candidate> = readable
            .values()
            .map(|m| Candidate {
                id: m.id.clone(),
                fused: recall::strength(m, now),
                agreed: true,
            })
            .collect();
        candidates.sort_by(|a, b| b.fused.total_cmp(&a.fused).then_with(|| a.id.cmp(&b.id)));
        candidates.truncate(recall::JUDGED);
        let (ids, docs): (Vec<String>, Vec<String>) = candidates
            .iter()
            .filter_map(|c| readable.get(&c.id).map(|m| (c.id.clone(), passage_of(m))))
            .unzip();
        let relevance =
            match tokio::time::timeout(JUDGE_WITHIN, self.models.rerank(recall::RERANK_MODEL, query, &docs)).await {
                Ok(Ok(scores)) => Some(ids.into_iter().zip(scores).collect::<HashMap<String, f32>>()),
                Ok(Err(err)) => {
                    tracing::debug!(%err, "memory: a tool call's recall ranked with no judge");
                    None
                }
                Err(_) => {
                    tracing::debug!("memory: a tool call's recall ranked with no judge, which took too long");
                    None
                }
            };
        recall::rank(
            &readable,
            &candidates,
            relevance.as_ref(),
            Purpose::Turn,
            TOOL_RECALL,
            now,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(v: Value) -> Map<String, Value> {
        match v {
            Value::Object(m) => m,
            _ => Map::new(),
        }
    }

    // What a tool call is about is what its memories are found by: a path in the repository as its file and its crate,
    // a shell line as the programs it runs, and nothing outside the repository.
    #[test]
    fn a_tool_call_is_about_the_files_and_programs_it_names() {
        let cwd = "/w/remit";
        let read = input(serde_json::json!({ "file_path": "/w/remit/crates/chains/src/session.rs" }));
        assert_eq!(
            subjects_of("Read", &read, cwd),
            ["crate:chains", "file:crates/chains/src/session.rs"]
        );
        let grep = input(serde_json::json!({ "pattern": "fn take", "path": "./docs/" }));
        assert_eq!(subjects_of("Grep", &grep, cwd), ["file:docs"]);
        let outside = input(serde_json::json!({ "file_path": "/etc/hosts" }));
        assert!(subjects_of("Read", &outside, cwd).is_empty());
        let bash = input(serde_json::json!({ "command": "cd crates && RUST_LOG=debug cargo nextest run | tail -5" }));
        assert_eq!(subjects_of("Bash", &bash, cwd), ["tool:cargo", "tool:tail"]);
        assert!(subjects_of("WebFetch", &read, cwd).is_empty());
    }
}
