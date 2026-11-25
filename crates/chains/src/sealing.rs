//! The key file values are sealed under (-secret-key-file): every crate that seals or opens a value (credentials, the
//! keys of the entrances) goes through it, so it is the core's.

use crate::chain_core::Core;
use crate::{Error, Result};

const NO_KEY_FILE: &str = "no secret key file is configured: start Remit with -secret-key-file";

/// Why a sealed value did not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unseal {
    /// It does not open under the key that sealed it: the value is lost, the one failure that is the value's own.
    Damaged(String),
    /// The key file is missing or holds another key: the value is kept, and opens again once its key is back.
    Refused(String),
}

impl std::fmt::Display for Unseal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Damaged(s) | Self::Refused(s) => f.write_str(s),
        }
    }
}

impl From<Unseal> for Error {
    fn from(u: Unseal) -> Self {
        Error::refused(u.to_string())
    }
}

impl Core {
    /// The key a value is sealed with now. The file is made at the first value sealed (a Remit that holds no credential
    /// never makes one), and whenever it is missing and no stored value names a key that has a backup: while one does, the
    /// key is to be restored, not replaced, because a new key would leave the stored values unopened and take the place
    /// the restored key goes back to.
    pub async fn sealing_key(&self) -> Result<Vec<u8>> {
        let file = &self.cfg.secret_key_file;
        if file.as_os_str().is_empty() {
            return Err(Error::refused(NO_KEY_FILE));
        }
        let key = match secrets::load_key(file) {
            Err(e) if e.is_not_found() => {
                if let Some(id) = self.restorable_key().await? {
                    return Err(Error::refused(format!(
                        "the key file {} is missing, and values sealed under key {id} are stored: {}",
                        file.display(),
                        self.restore_key(&id)
                    )));
                }
                secrets::load_or_create_key(file)
            }
            other => other,
        }
        .map_err(|e| Error::refused(e.to_string()))?;
        secrets::back_up_key(&key, &self.cfg.key_backup_dir)
            .map_err(|e| Error::refused(format!("nothing is sealed under a key that is not backed up: {e}")))?;
        Ok(key)
    }
    /// Opens a sealed value with the key file's key. A key file that is missing or holds another key is refused with what
    /// to do, and the value is kept: it opens again once its key is back.
    pub fn unseal(&self, aad: &str, sealed: &secrets::Sealed) -> Result<Vec<u8>, Unseal> {
        let file = &self.cfg.secret_key_file;
        if file.as_os_str().is_empty() {
            return Err(Unseal::Refused(NO_KEY_FILE.to_owned()));
        }
        let key = match secrets::load_key(file) {
            Ok(key) => key,
            Err(e) if e.is_not_found() => {
                return Err(Unseal::Refused(format!(
                    "cannot be opened: the key file {} is missing; {}",
                    file.display(),
                    self.restore_key(&sealed.key_id)
                )));
            }
            Err(e) => return Err(Unseal::Refused(format!("cannot be opened: {e}"))),
        };
        secrets::open(&key, aad, sealed).map_err(|e| match &e {
            secrets::Error::WrongKey { sealed, .. } => {
                Unseal::Refused(format!("cannot be opened: {e}; {}", self.restore_key(sealed)))
            }
            secrets::Error::Damaged => Unseal::Damaged(format!("cannot be opened: {e}")),
            _ => Unseal::Refused(format!("cannot be opened: {e}")),
        })
    }
    /// The id of a key that stored values were sealed under and that has a backup, if any has.
    pub async fn restorable_key(&self) -> Result<Option<String>> {
        let ids = self.store.read(|r| r.sealed_key_ids()).await?;
        Ok(ids
            .into_iter()
            .find(|id| secrets::backup_file(&self.cfg.key_backup_dir, id).exists()))
    }
    /// How the owner brings back the key of that id, and that nothing was cleared.
    pub fn restore_key(&self, id: &str) -> String {
        let backup = secrets::backup_file(&self.cfg.key_backup_dir, id);
        if !backup.exists() {
            return format!(
                "no backup of key {id} is in {}: put that key back at {} from where you keep it; nothing was cleared",
                self.cfg.key_backup_dir.display(),
                self.cfg.secret_key_file.display()
            );
        }
        format!(
            "copy its backup {} to {} (mode 0600) and try again; nothing was cleared",
            backup.display(),
            self.cfg.secret_key_file.display()
        )
    }
}
