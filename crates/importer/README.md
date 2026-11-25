# importer

Reads the Claude Code configuration on this machine (`agents/*.md`, `skills/*/`) and turns an entry into a record of
the library: `list` the candidates, `agent` and `skill` one import with a `Report` of what was left out. It only reads,
and only inside the configuration directory: a name is a file name, the kind's directory must resolve inside it, and
nothing inside a skill is followed through a link. Not a pure crate (it reads files), so it takes the workspace's lints
without the pure crates' `clippy.toml`.
