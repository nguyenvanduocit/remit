//! What a Remit agent session is given and whether a definition may be stored. It is pure: the store, the sealed secrets
//! and the launcher stay outside, and hand it plain values.

mod access;
mod diff;
mod validate;

pub use access::{
    BUILTIN_PLUGIN_NAME, CardKind, Input, Library, Missing, NameClash, PLUGIN_NAME, Session, access, canonical_builtin,
    canonical_skill, check_skill_name,
};
pub use diff::{diff_fields, json_text};
pub use validate::{Problems, valid_id, validate_agent, validate_loadout};
