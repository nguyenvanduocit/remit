# claudehooks

Typed wire format of Claude Code hooks (Claude Code 2.1.322): what a hook reads, what it prints, and the `hooks`
block of `settings.json` that registers it. The reference for every event, its meaning and its exit codes is
[`docs/claude-code-hooks.md`](../../docs/claude-code-hooks.md).

- `input` (input.rs): `HookInput` = `Common` fields + one `Event`, tagged by `hook_event_name`; `parse_input`.
- `output` (output.rs): `HookOutput` (sync answer), `AsyncOutput` (`{"async": true}`), `SpecificOutput` tagged by
  `hookEventName`.
- `config` (config.rs): `Hooks`, `MatcherGroup`, `Handler` (`command`, `prompt`, `mcp_tool`, `http`, `agent`).

Sets in the upstream schema are enums, each with an `Other` variant (`Unknown` for `ApiError`, `Event::Unknown` for
the event) that takes a value newer than this crate, so a Claude Code update degrades instead of failing every hook;
open sets (`notification_type`, `mcp_server.source`, `permission_mode`) are strings. Unknown fields are ignored. A JSON number the schema does not mark
as an integer is an `f64`, as Claude Code (JavaScript) reads it.

`examples/guard.rs` is a `PreToolUse` hook that denies `rm -rf`.
