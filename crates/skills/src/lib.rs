//! What a Remit skill is: the SKILL.md format Claude Code reads, the rules a skill must meet before the library stores it,
//! and the plugin directory the library's skills are rendered into and written to disk.

mod frontmatter;
mod render;
mod validate;
mod write;

pub use frontmatter::{Fields, FrontmatterError, header, parse_frontmatter};
pub use render::{BUILTIN_PLUGIN_NAME, PLUGIN_NAME, render};
pub use validate::{
    MAX_DESCRIPTION_LEN, MAX_FILE_BYTES, MAX_FILES, MAX_NAME_LEN, MAX_TOTAL_BYTES, PathError, SkillError, is_text,
    valid_path, valid_skill_path, validate,
};
pub use write::{WriteError, write_tree};
