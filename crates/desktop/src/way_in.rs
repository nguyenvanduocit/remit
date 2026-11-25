//! How the owner, or a mentor, comes in (ADR-0128 decision 3, ADR-0026): the link the server prints at start
//! (`http://host:port/#key=…`) or the one the owner passed a mentor carries the server and the credential. The window
//! keeps both in Remit's directory under the user's home, readable by its user alone, and comes in with them on the
//! next start. Nothing here decides who is let in: the server checks the credential the hello carries.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The server to come in to and the credential that comes in: the owner key or a mentor's code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    /// `http://host:port`: where its `/link.json` is.
    pub server: String,
    pub key: String,
}

/// The server and credential a link carries (`#key=`); none for text that is not such a link.
pub fn read_link(text: &str) -> Option<Credential> {
    let url = link_quic::Url::parse(text.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    // The fragment reads as a query does: `key=…&…`, percent-encoded.
    let pairs = link_quic::Url::parse(&format!("remit:?{}", url.fragment()?)).ok()?;
    let key = pairs
        .query_pairs()
        .find_map(|(name, value)| (name == "key").then(|| value.into_owned()))
        .filter(|key| !key.is_empty())?;
    Some(Credential {
        server: url.origin().ascii_serialization(),
        key,
    })
}

/// Where the window keeps its credential: `~/.remit/desktop.json`.
pub fn kept_at(home: &Path) -> PathBuf {
    home.join(".remit").join("desktop.json")
}

/// The credential kept at `path`; none when there is none, or it does not read.
pub fn kept(path: &Path) -> Option<Credential> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Keeps `credential` at `path`, in place of the one kept.
pub fn keep(path: &Path, credential: &Credential) -> std::io::Result<()> {
    write_private(path, &serde_json::to_vec(credential)?)
}

/// Writes `bytes` to `path`, readable and writable by this user alone: what the window keeps is a credential or the
/// owner's own words.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt as _;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create(true).truncate(true);
    #[cfg(unix)]
    open.mode(0o600);
    let mut file = open.open(path)?;
    // A file kept before with a wider mode is narrowed: `mode` sets it only on creation.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_printed_link_carries_the_server_and_the_key_and_anything_else_does_not() {
        assert_eq!(
            read_link(" http://127.0.0.1:4320/#key=ab%2Bc&tab=x "),
            Some(Credential {
                server: "http://127.0.0.1:4320".to_owned(),
                key: "ab+c".to_owned(),
            })
        );
        assert_eq!(read_link("http://127.0.0.1:4320/"), None);
        assert_eq!(read_link("http://127.0.0.1:4320/#key="), None);
        assert_eq!(read_link("ab+c"), None);
        assert_eq!(read_link("file:///tmp/#key=k"), None);
    }

    #[test]
    fn a_kept_credential_comes_back_and_only_its_user_reads_it() {
        let dir = std::env::temp_dir().join(format!("remit-desktop-{}", std::process::id()));
        let path = kept_at(&dir);
        let credential = Credential {
            server: "http://127.0.0.1:4320".to_owned(),
            key: "k".to_owned(),
        };
        keep(&path, &credential).unwrap();
        assert_eq!(kept(&path), Some(credential));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
