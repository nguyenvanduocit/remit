//! How much the agents used each tool, MCP tool and skill.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use model::{Card, Chain, OutcomeUsage, ToolTally};

/// Claude Code's name for an MCP tool: `mcp__<server>__<tool>`.
const MCP_PREFIX: &str = "mcp__";
/// Claude Code's name for the tool that loads a skill.
const SKILL_TOOL: &str = "Skill";
/// A Skill call's title, followed by the skill's name.
const SKILL_TITLE: &str = "Load skill: ";
const UNKNOWN_SKILL: &str = "unknown";

/// Which tab a tool call counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tab {
    Builtin,
    Mcp,
    Skill,
}

/// What a tally counts as: the tab it belongs in, its server (MCP only) and its name. A tally with no tool name counts as
/// none (the store names every call that names a tool, an older one included).
fn classify(t: &ToolTally) -> Option<(Tab, &str, &str)> {
    if t.name == SKILL_TOOL {
        return Some((Tab::Skill, "", skill_of(&t.title)));
    }
    if t.name.is_empty() {
        return None;
    }
    if let Some(rest) = t.name.strip_prefix(MCP_PREFIX) {
        let (server, name) = rest.split_once("__").unwrap_or((rest, ""));
        return Some((Tab::Mcp, server, name));
    }
    Some((Tab::Builtin, "", t.name.as_str()))
}

fn skill_of(title: &str) -> &str {
    match title.strip_prefix(SKILL_TITLE) {
        Some(name) if !name.is_empty() => name,
        _ => UNKNOWN_SKILL,
    }
}

/// One usage being counted, with the agents and chains seen so far.
#[derive(Default)]
struct Seen<'a> {
    calls: i64,
    failed: i64,
    agents: BTreeSet<&'a str>,
    chains: BTreeSet<&'a str>,
}

fn len(set: &BTreeSet<&str>) -> i64 {
    i64::try_from(set.len()).unwrap_or(i64::MAX)
}

/// The usages of one tab, the most called first.
pub(crate) struct Usages {
    pub(crate) tools: Vec<OutcomeUsage>,
    pub(crate) mcp: Vec<OutcomeUsage>,
    pub(crate) skills: Vec<OutcomeUsage>,
}

/// Counts the tallies into one usage per tool, MCP tool and skill, the most called first. A chain is a card's home that
/// is one of `chains`; the Assistant's home is not.
pub(crate) fn usages(tallies: &[ToolTally], cards: &[Card], chains: &[Chain]) -> Usages {
    let is_chain: HashSet<&str> = chains.iter().map(|c| c.id.as_str()).collect();
    let home_of: HashMap<&str, &str> = cards.iter().map(|c| (c.id.as_str(), c.home.as_str())).collect();
    let mut by_key: BTreeMap<(Tab, &str, &str), Seen> = BTreeMap::new();
    for t in tallies {
        let Some(key) = classify(t) else {
            continue;
        };
        let s = by_key.entry(key).or_default();
        s.calls = s.calls.saturating_add(t.calls);
        s.failed = s.failed.saturating_add(t.failed);
        s.agents.insert(t.card_id.as_str());
        if let Some(home) = home_of.get(t.card_id.as_str()).filter(|h| is_chain.contains(*h)) {
            s.chains.insert(home);
        }
    }
    let mut out = Usages {
        tools: Vec::new(),
        mcp: Vec::new(),
        skills: Vec::new(),
    };
    for ((tab, server, name), s) in by_key {
        let usage = OutcomeUsage {
            server: server.to_owned(),
            name: name.to_owned(),
            calls: s.calls,
            failed: s.failed,
            agents: len(&s.agents),
            chains: len(&s.chains),
        };
        match tab {
            Tab::Builtin => out.tools.push(usage),
            Tab::Mcp => out.mcp.push(usage),
            Tab::Skill => out.skills.push(usage),
        }
    }
    for list in [&mut out.tools, &mut out.mcp, &mut out.skills] {
        list.sort_by(|a, b| {
            b.calls
                .cmp(&a.calls)
                .then_with(|| a.server.cmp(&b.server))
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    out
}
