# skills

What a Remit skill is: the SKILL.md frontmatter Claude Code reads (`parse_frontmatter`, `header`), the rules a skill
meets before the library stores it (`validate`, `valid_path`, `valid_skill_path`), the plugin directory its skills are
laid out as (`render`) and the one write of that directory to disk (`write_tree`).

Not a pure crate: `write_tree` writes the rendered plugin directory under a root and removes it again when the write
fails halfway, so the crate takes the workspace's lints without the pure crates'
`clippy.toml` (which bans `std::fs`). Everything else in it is pure, and the `library` crate, which is pure, calls only
that part.
