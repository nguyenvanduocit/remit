use model::{Loadout, Skill};
use serde::{Deserialize, Serialize};

/// The plugin directory that holds the library's skills. Claude Code prefixes a plugin's skills with it, so it is part of
/// every skill's canonical name.
pub const PLUGIN_NAME: &str = skills::PLUGIN_NAME;

/// The plugin of the skills that ship with Remit. An agent of the library cannot name one; a card holds one because its
/// kind is the Assistant or because its chain lists the capability that brings it (`Input::builtin`).
pub const BUILTIN_PLUGIN_NAME: &str = skills::BUILTIN_PLUGIN_NAME;

/// The Claude Code tool that runs a skill. A `tools` allowlist drops it unless it is listed.
const SKILL_TOOL: &str = "Skill";

/// The name Claude Code gives a library skill, the one `skills` must list. Claude Code turns every dot of a skill's
/// directory name into a dash (vault.help is library:vault-help), and an allowlist that names the dotted form lists
/// nothing.
pub fn canonical_skill(name: &str) -> String {
    canonical(PLUGIN_NAME, name)
}

/// The name Claude Code gives a skill that ships with Remit.
pub fn canonical_builtin(name: &str) -> String {
    canonical(BUILTIN_PLUGIN_NAME, name)
}

fn canonical(plugin: &str, name: &str) -> String {
    format!("{plugin}:{}", name.replace('.', "-"))
}

/// A skill name Claude Code would give the same canonical name as another skill of the library: no allowlist could tell
/// the two apart.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("skill {name:?} would be named {canonical} by Claude Code, as {existing:?} already is: rename one of them")]
pub struct NameClash {
    pub name: String,
    pub canonical: String,
    pub existing: String,
}

/// Refuses a skill name that Claude Code would give the same canonical name as another skill of the library.
pub fn check_skill_name(name: &str, existing: &[Skill]) -> Result<(), NameClash> {
    let canonical = canonical_skill(name);
    match existing
        .iter()
        .find(|s| s.name != name && canonical_skill(&s.name) == canonical)
    {
        Some(s) => Err(NameClash {
            name: name.to_owned(),
            canonical,
            existing: s.name.clone(),
        }),
        None => Ok(()),
    }
}

/// What a card is for. A worker and a Chair hold a loadout of the library, the agent that fills the seat; Remit starts
/// the others for itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CardKind {
    #[default]
    Worker,
    Chair,
    Judge,
    Assistant,
    Probe,
}

/// The inventory an agent can hold things from, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Library {
    pub skills: Vec<Skill>,
}

impl Library {
    pub(crate) fn has_skill(&self, name: &str) -> bool {
        self.skills.iter().any(|s| s.name == name)
    }
}

/// Everything [`access`] decides from. `model` and `effort` are the card's own, chosen when it was hired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    pub kind: CardKind,
    /// What a worker or a Chair holds: the one its card took when it was hired or seated, never its agent's as stored now.
    /// The default loadout is a plain agent's: every tool, nothing from the library.
    pub loadout: Loadout,
    /// The skills that ship with Remit which this card holds: the caller names them, by the kind of card they are for,
    /// and no agent definition can.
    pub builtin: Vec<Skill>,
    pub model: String,
    pub effort: String,
    pub library: Library,
}

/// Something an agent's definition names and the library cannot give, with the reason, so the owner sees it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Missing {
    /// skill
    pub kind: String,
    pub name: String,
    pub why: String,
}

/// What a card is given, and nothing else: the host's own Claude Code configuration adds to none of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    /// The Claude Code setting layers to load. Always the project's: its CLAUDE.md and settings are the repository's
    /// rules, while the user's layer is the server owner's preferences.
    pub setting_sources: Vec<String>,
    /// The allowlist of Claude Code tools; none means every tool.
    pub tools: Option<Vec<String>>,
    /// Canonical names. Claude Code treats an absent list as "every skill" and an empty one as none, so it is always a
    /// list.
    pub skills: Vec<String>,
    /// The same skills by the name the library and the agent's definition give them, in the same order.
    pub held_skills: Vec<String>,
    /// The plugins the skills above come from, by name (library, builtin): each must be loaded for its skills to exist.
    /// Where a plugin lives on disk is not decided here.
    pub plugins: Vec<String>,
    pub missing: Vec<Missing>,
    /// Appended to the system prompt.
    pub instructions: String,
    pub model: String,
    pub effort: String,
}

/// The one place that decides what a session gets. What it returns shares nothing with its input.
pub fn access(input: &Input) -> Session {
    let mut s = Session {
        setting_sources: vec!["project".to_owned()],
        model: input.model.clone(),
        effort: input.effort.clone(),
        ..Session::default()
    };
    if matches!(input.kind, CardKind::Worker | CardKind::Chair) {
        access_worker(&mut s, input);
    }
    if !s.skills.is_empty() {
        s.plugins.push(PLUGIN_NAME.to_owned());
    }
    for b in &input.builtin {
        s.held_skills.push(b.name.clone());
        s.skills.push(canonical_builtin(&b.name));
    }
    if !input.builtin.is_empty() {
        s.plugins.push(BUILTIN_PLUGIN_NAME.to_owned());
        add_skill_tool(&mut s.tools);
    }
    s
}

/// Fills `s` with what a worker's loadout names: its instructions, tools and library skills.
fn access_worker(s: &mut Session, input: &Input) {
    let l = &input.loadout;
    s.instructions = l.instructions.clone();
    s.tools = l.tools.clone();
    // The order the agent lists its skills in is kept.
    for name in &l.skills {
        if input.library.has_skill(name) {
            s.held_skills.push(name.clone());
            s.skills.push(canonical_skill(name));
        } else {
            s.missing.push(Missing {
                kind: "skill".into(),
                name: name.clone(),
                why: "not in the library".into(),
            });
        }
    }
    if !s.skills.is_empty() {
        add_skill_tool(&mut s.tools);
    }
}

/// Lets an allowlist run skills; a session with every tool runs them already.
fn add_skill_tool(tools: &mut Option<Vec<String>>) {
    if let Some(tools) = tools
        && !tools.iter().any(|t| t == SKILL_TOOL)
    {
        tools.push(SKILL_TOOL.to_owned());
    }
}
