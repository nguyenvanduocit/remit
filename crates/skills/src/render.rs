use std::collections::BTreeMap;

use model::Skill;
use serde::Serialize;

// Claude Code names a plugin's skill "<plugin>:<skill>", so the plugin's name is the prefix of every skill in it. The
// library's skills (the owner's) are in one plugin, and the skills that ship with Remit are in another: an agent of the
// library can never name one of the built-in skills, and an owner's skill can never take one's name.
pub const PLUGIN_NAME: &str = "library";
pub const BUILTIN_PLUGIN_NAME: &str = "builtin";

const MANIFEST_PATH: &str = ".claude-plugin/plugin.json";
/// Where a plugin's skills are, one directory each.
const SKILLS_DIR: &str = "skills";

#[derive(Serialize)]
struct Manifest<'a> {
    name: &'a str,
    version: &'a str,
    description: String,
}

/// The skills laid out as the plugin directory Claude Code loads, named `plugin`: path to content. The same skills give
/// the same map in any order, so a rendered directory can be compared with the one on disk.
pub fn render(plugin: &str, list: &[Skill]) -> BTreeMap<String, String> {
    let manifest = Manifest {
        name: plugin,
        version: "0.0.0",
        description: format!("Remit {plugin} skills"),
    };
    #[expect(clippy::expect_used, reason = "a struct of strings always serialises")]
    let manifest = serde_json::to_string(&manifest).expect("the manifest serialises");
    let mut out = BTreeMap::from([(MANIFEST_PATH.to_owned(), manifest)]);
    for s in list {
        for (p, content) in &s.files {
            out.insert(format!("{SKILLS_DIR}/{}/{p}", s.name), content.clone());
        }
    }
    out
}
