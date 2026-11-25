//! Typed wire format of Claude Code hooks, as of Claude Code 2.1.322.
//!
//! - [`input`]: the JSON a hook reads on stdin (or as the HTTP body / `$ARGUMENTS`).
//! - [`output`]: the JSON a hook prints on stdout to steer Claude Code.
//! - [`config`]: the `hooks` block of a `settings.json`, to write hook registrations.
//!
//! The full reference, with what each event means and how exit codes act, is `docs/claude-code-hooks.md`.

pub mod config;
pub mod input;
pub mod output;

pub use input::{Event, HookInput};
pub use output::{AsyncOutput, HookOutput, SpecificOutput};

/// Parses the JSON Claude Code hands a hook. A newer Claude Code still parses: unknown fields are ignored, an unknown
/// event is [`Event::Unknown`] and a value newer than a set this crate knows is that set's `Other` (`Unknown` for
/// [`input::ApiError`]). A missing required field is an error.
pub fn parse_input(json: &str) -> Result<HookInput, serde_json::Error> {
    serde_json::from_str(json)
}
