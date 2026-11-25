//! The keys of Remit's entrances (an event signal's door, a client key, a worker key) are made, sealed and opened the same
//! way, here: 256 random bits behind a prefix that secret scanners know, sealed under the key file beside the database with
//! the record they belong to as the additional data, so a key moved to another record does not open there. The key file
//! itself is credentials.rs's (`sealing_key`, `unseal`).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrets::Sealed;

use crate::chain_core::Core;
use crate::{Error, Result};

/// A key nobody can guess, behind prefix.
pub fn new_key(prefix: &str) -> Result<String> {
    let mut b = [0_u8; 32];
    getrandom::fill(&mut b).map_err(|e| Error::refused(format!("no random bits for a new key: {e}")))?;
    Ok(format!("{prefix}{}", URL_SAFE_NO_PAD.encode(b)))
}

impl Core {
    /// A new key behind prefix, sealed for the record aad names.
    pub async fn seal_new_key(&self, prefix: &str, aad: &str) -> Result<Sealed> {
        let file_key = self.sealing_key().await?;
        let key = new_key(prefix)?;
        secrets::seal(&file_key, aad, key.as_bytes()).map_err(|e| Error::refused(e.to_string()))
    }

    /// The key a sealed value holds, for the record aad names.
    pub fn opened_key(&self, aad: &str, sealed: &Sealed) -> Result<String> {
        let key = self.unseal(aad, sealed)?;
        String::from_utf8(key).map_err(|_| Error::refused("cannot be opened: the value is not a key"))
    }
}
