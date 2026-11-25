# builtin

The skills that ship with Remit (`for_assistant`: the Assistant's manual of the app and its OKR skill), embedded in the
binary with `rust-embed` from the directory `src/lib.rs` names in `#[folder]`, where they are written. Each subdirectory is one skill; its
SKILL.md gives the description. `build.rs` watches the directory so a new or removed file rebuilds the crate.
