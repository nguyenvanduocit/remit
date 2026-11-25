//! Seals the credentials an MCP server needs, so the database and its backups hold none in the clear.
//!
//! A value is sealed with AES-256-GCM under a key kept in a file beside the database. The caller names the slot the value
//! belongs to ("server/slot") as additional data, so a sealed value copied to another slot does not open there. A sealed
//! value carries the id of the key that sealed it (`key_id`), so a key that is missing or another is told apart from a
//! value that is damaged, and the key file is backed up under that id (`back_up_key`).
//!
//! The limit: agents run as the same OS user as the server, so an agent can read the key file as the server does.
//! Sealing protects the database file and its backups. It does not protect the running host from an agent that tries.

mod secrets;

pub use secrets::{
    Error, KEY_SIZE, Sealed, back_up_key, backup_file, key_id, load_key, load_or_create_key, open, seal,
};
