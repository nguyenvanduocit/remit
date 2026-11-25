use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use subtle::ConstantTimeEq;

/// Who a browser that came in with the owner key is: the one person who can read the key file on this machine.
pub const LOCAL_OWNER: &str = "local";

/// Why the owner key could not be opened, or a credential is not it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The credential is not the owner key.
    #[error("invalid token: not the owner key")]
    Invalid,
    #[error("owner key: {0}")]
    Io(#[from] io::Error),
    #[error("owner key {} is not a regular file", .0.display())]
    NotRegular(PathBuf),
    #[error("owner key {} may be read by others (mode {}): chmod 600 it", .path.display(), perm_string(*.mode))]
    ReadableByOthers { path: PathBuf, mode: u32 },
    #[error("owner key {} holds no key: delete it and a new one is made at the next start", .0.display())]
    NoKey(PathBuf),
}

/// Lets in a browser that holds the owner key (ADR-0026). The key lives in a file only the server's OS user may read,
/// which every agent session has hidden from it, so holding it is being the owner at this machine.
#[derive(Debug)]
pub struct LocalKey {
    key: String,
}

impl LocalKey {
    /// Reads the owner key at path, or makes one there when the file does not exist. A file it cannot trust (one others
    /// may read, a link, one that holds no key) is an error and is left as it is: replacing it would hand the way in to
    /// whoever placed it.
    pub fn open(path: &Path) -> Result<LocalKey, Error> {
        let info = match fs::symlink_metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return make_local_key(path),
            other => other?,
        };
        if !info.file_type().is_file() {
            return Err(Error::NotRegular(path.to_path_buf()));
        }
        let perm = info.permissions().mode() & 0o777;
        if perm & 0o077 != 0 {
            return Err(Error::ReadableByOthers {
                path: path.to_path_buf(),
                mode: perm,
            });
        }
        let text = fs::read_to_string(path)?;
        let key = text.trim();
        if !key_shape(key) {
            return Err(Error::NoKey(path.to_path_buf()));
        }
        Ok(LocalKey { key: key.to_string() })
    }

    /// The owner key, for the link the server hands its owner.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Says the credential is the owner key, or `Error::Invalid`. The comparison takes the same time however much of it
    /// matches (`subtle`); a credential of another length is refused at once, which tells its length and nothing of the
    /// key's bytes.
    pub fn verify(&self, credential: &str) -> Result<&'static str, Error> {
        if bool::from(credential.as_bytes().ct_eq(self.key.as_bytes())) {
            Ok(LOCAL_OWNER)
        } else {
            Err(Error::Invalid)
        }
    }
}

/// What an owner key file holds: 32 random bytes in lower-case hex, with a newline or not (trimmed before).
fn key_shape(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Writes a new key to path, failing rather than replacing a file made there meanwhile.
fn make_local_key(path: &Path) -> Result<LocalKey, Error> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(io::Error::from)?;
    let key = hex::encode(b);
    let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    f.write_all(format!("{key}\n").as_bytes())?;
    Ok(LocalKey { key })
}

/// A permission mode written as `ls -l` writes one (`-rw-r--r--`).
fn perm_string(mode: u32) -> String {
    let mut s = String::from("-");
    // Bit 8 (owner read) down to bit 0 (others execute), in the order the letters are written.
    for (c, bit) in "rwxrwxrwx".chars().zip((0..9u32).rev()) {
        s.push(if mode & (1 << bit) != 0 { c } else { '-' });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_owner_key_lets_a_browser_in() {
        let dir = tempfile::tempdir().unwrap();
        let k = LocalKey::open(&dir.path().join("owner.key")).unwrap();
        assert_eq!(k.verify(k.key()).unwrap(), LOCAL_OWNER);
        let capital = k.key().to_ascii_uppercase();
        let longer = format!("{}0", k.key());
        for (name, credential) in [
            ("nothing", ""),
            (
                "another key",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            ("the key's start", &k.key()[..32]),
            ("the key and more", longer.as_str()),
            ("a JWT", "eyJhbGciOiJSUzI1NiJ9.e30.sig"),
            ("the key in capital", capital.as_str()),
        ] {
            assert!(
                matches!(k.verify(credential), Err(Error::Invalid)),
                "{name}: Verify let it in"
            );
        }
    }
}
