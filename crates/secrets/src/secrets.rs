use std::fs::{self, DirBuilder, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit};
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// The size of the key in bytes: AES-256.
pub const KEY_SIZE: usize = 32;

/// The size of the random nonce at the start of a sealed box: GCM's standard 12 bytes.
const NONCE_SIZE: usize = 12;

/// Why a key could not be read, made or backed up, or a value not sealed or opened. No variant carries a value, a key or
/// sealed bytes: errors are logged and shown, and they would leak it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("secrets: open key file: {0}")]
    OpenKeyFile(#[source] io::Error),
    #[error("secrets: stat key file: {0}")]
    StatKeyFile(#[source] io::Error),
    #[error("secrets: key file {} is not a regular file", .0.display())]
    NotRegular(PathBuf),
    #[error("secrets: key file {path} is accessible to group or other (mode {mode:04o}): run chmod 600 {path}", path = .path.display())]
    ReadableByOthers { path: PathBuf, mode: u32 },
    #[error("secrets: read key file {}: {source}", .path.display())]
    ReadKeyFile { path: PathBuf, source: io::Error },
    #[error(
        "secrets: key file {} holds {size} bytes, want {KEY_SIZE}: restore it from a backup, or remove it to start with a new key and lose every sealed value",
        .path.display()
    )]
    WrongSize { path: PathBuf, size: u64 },
    #[error("secrets: create key file (its directory must exist): {0}")]
    CreateTemp(#[source] io::Error),
    #[error("secrets: write key file: {0}")]
    WriteKeyFile(#[source] io::Error),
    #[error("secrets: create key file: {0}")]
    LinkKeyFile(#[source] io::Error),
    #[error("secrets: the system's random source failed: {0}")]
    Random(#[source] io::Error),
    #[error("secrets: no directory to back the key up in")]
    NoBackupDir,
    #[error(
        "secrets: the backup {} does not hold the key its name says: it was changed, and is left as it is for you to look at",
        .0.display()
    )]
    BackupChanged(PathBuf),
    #[error("secrets: the backup of the key: {0}")]
    Backup(#[source] Box<Error>),
    #[error("secrets: make the key backup directory: {0}")]
    BackupDir(#[source] io::Error),
    /// The backup written just now does not read back as the key; `read` is why it could not be read, when it could not.
    #[error("secrets: the backup {} does not hold the key{}", .path.display(), .read.as_ref().map(|e| format!("\n{e}")).unwrap_or_default())]
    BackupNotHeld { path: PathBuf, read: Option<Box<Error>> },
    #[error("secrets: key has {0} bytes, want {KEY_SIZE}")]
    KeyLength(usize),
    /// A value does not open under the key that sealed it: its bytes changed, or it is not the slot's own.
    #[error("secrets: the value does not open under its own key: it was damaged, or it is another slot's")]
    Damaged,
    /// A value was sealed under another key than the one given: the value may be whole, and opens again once the key that
    /// sealed it is back. `sealed` is that key's id.
    #[error("secrets: the value was sealed under key {sealed}, and the key given is {given}")]
    WrongKey { sealed: String, given: String },
}

impl Error {
    /// Says the key file is not there (what `load_key` returns when there is none).
    pub fn is_not_found(&self) -> bool {
        self.io_kind() == Some(io::ErrorKind::NotFound)
    }

    /// Says a key file was already at the path a new one was to be written to.
    fn is_already_exists(&self) -> bool {
        self.io_kind() == Some(io::ErrorKind::AlreadyExists)
    }

    fn io_kind(&self) -> Option<io::ErrorKind> {
        match self {
            Error::OpenKeyFile(e)
            | Error::StatKeyFile(e)
            | Error::CreateTemp(e)
            | Error::WriteKeyFile(e)
            | Error::LinkKeyFile(e)
            | Error::BackupDir(e)
            | Error::ReadKeyFile { source: e, .. } => Some(e.kind()),
            Error::Backup(inner) => inner.io_kind(),
            _ => None,
        }
    }
}

/// Reads the key file at path, creating it at the first start. The directory of path must exist: it is the operator's
/// data directory, and a missing one is a mistake to report, not to hide.
///
/// A key file that others can read, or that is not `KEY_SIZE` bytes, is refused and left as it is: replacing it would
/// orphan every value sealed under it.
pub fn load_or_create_key(path: &Path) -> Result<Vec<u8>, Error> {
    match read_key(path) {
        Err(e) if e.is_not_found() => {}
        other => return other,
    }
    // Whoever loses the race for the file reads the winner's key.
    match create_key(path) {
        Err(e) if !e.is_already_exists() => return Err(e),
        _ => {}
    }
    read_key(path)
}

/// Reads the key file at path and never creates it: with no file, the error says so (`Error::is_not_found`). The same
/// refusals as `load_or_create_key` apply to a file that others can read or that has the wrong size.
pub fn load_key(path: &Path) -> Result<Vec<u8>, Error> {
    read_key(path)
}

fn read_key(path: &Path) -> Result<Vec<u8>, Error> {
    let f = File::open(path).map_err(Error::OpenKeyFile)?;
    let info = f.metadata().map_err(Error::StatKeyFile)?;
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
    let mut key = Vec::with_capacity(KEY_SIZE + 1);
    // One byte past the key size tells a key file that is too long from one that is exactly right.
    f.take(u64::try_from(KEY_SIZE + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut key)
        .map_err(|source| Error::ReadKeyFile {
            path: path.to_path_buf(),
            source,
        })?;
    if key.len() != KEY_SIZE {
        return Err(Error::WrongSize {
            path: path.to_path_buf(),
            size: info.len(),
        });
    }
    Ok(key)
}

/// Makes a new key and writes it at path. The link fails when path exists, which is what lets one caller win.
fn create_key(path: &Path) -> Result<(), Error> {
    let mut key = [0u8; KEY_SIZE];
    getrandom::fill(&mut key).map_err(|e| Error::Random(e.into()))?;
    write_key(path, &key)
}

/// Writes key under a temporary name and links it to path, so path never shows a file that is empty or half written,
/// and a caller that finds it there reads a whole key. The link fails when path exists: a key file is never replaced.
fn write_key(path: &Path, key: &[u8]) -> Result<(), Error> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let base = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    // tempfile makes the file 0600 and removes the temporary name when it is dropped: the key
    // lives on under path.
    let mut tmp = tempfile::Builder::new()
        .prefix(&format!("{base}.tmp-"))
        .tempfile_in(dir)
        .map_err(Error::CreateTemp)?;
    tmp.write_all(key)
        .and_then(|()| tmp.as_file().sync_all())
        .map_err(Error::WriteKeyFile)?;
    fs::hard_link(tmp.path(), path).map_err(Error::LinkKeyFile)
}

/// Names key without giving it away: the first 8 bytes of an HMAC of a constant under the key, in hex. A sealed value
/// records it, so the key it needs can be named, and its backup found, when the key file holds another or none.
pub fn key_id(key: &[u8]) -> String {
    // HMAC takes a key of any length, so new_from_slice cannot fail; the fallback keeps the function total.
    let Ok(mut mac) = <Hmac<Sha256>>::new_from_slice(key) else {
        return String::new();
    };
    mac.update(b"remit secret key id");
    let digest = mac.finalize().into_bytes();
    // A SHA-256 digest has 32 bytes, so the first 8 are always there; the fallback keeps the function total.
    let Some(id) = digest.first_chunk::<8>() else {
        return String::new();
    };
    hex::encode(id)
}

/// Where `back_up_key` keeps the key of that id in dir.
pub fn backup_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("secret-{id}.key"))
}

/// Keeps a copy of key in dir (made owner-only when missing), named by its id, and returns its path. A key is copied
/// once: a copy already there is read back and must hold this very key. One that holds anything else was changed after
/// it was written, and is refused and left as it is, because what it holds now is the evidence. Another key gets a copy
/// of its own beside it, so restoring a database finds the key its values name.
pub fn back_up_key(key: &[u8], dir: &Path) -> Result<PathBuf, Error> {
    if dir.as_os_str().is_empty() {
        return Err(Error::NoBackupDir);
    }
    let file = backup_file(dir, &key_id(key));
    match read_key(&file) {
        Ok(held) if held == key => return Ok(file),
        Ok(_) => return Err(Error::BackupChanged(file)),
        Err(e) if !e.is_not_found() => return Err(Error::Backup(Box::new(e))),
        Err(_) => {}
    }
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(Error::BackupDir)?;
    match write_key(&file, key) {
        Err(e) if !e.is_already_exists() => return Err(e),
        _ => {}
    }
    // Whoever lost the race to write it checks what the winner wrote.
    match read_key(&file) {
        Ok(held) if held == key => Ok(file),
        Ok(_) => Err(Error::BackupNotHeld { path: file, read: None }),
        Err(e) => Err(Error::BackupNotHeld {
            path: file,
            read: Some(Box::new(e)),
        }),
    }
}

/// A value `seal` returned: the box (the nonce followed by the ciphertext and its tag) and the id of the key that sealed
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sealed {
    pub key_id: String,
    pub r#box: Vec<u8>,
}

/// Returns plaintext sealed under key: the nonce followed by the ciphertext, and the key's id. aad names the slot the
/// value belongs to and must be the same at `open`. The nonce is random: a key seals a handful of values, far below where
/// random nonces collide.
pub fn seal(key: &[u8], aad: &str, plaintext: &[u8]) -> Result<Sealed, Error> {
    let gcm = new_gcm(key)?;
    let mut nonce = [0u8; NONCE_SIZE];
    getrandom::fill(&mut nonce).map_err(|e| Error::Random(e.into()))?;
    let ciphertext = gcm
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        // aes-gcm fails only on a message past GCM's limit of about 64 GiB, which no credential is.
        .map_err(|_| Error::Damaged)?;
    let mut sealed_box = Vec::with_capacity(NONCE_SIZE.saturating_add(ciphertext.len()));
    sealed_box.extend_from_slice(&nonce);
    sealed_box.extend_from_slice(&ciphertext);
    Ok(Sealed {
        key_id: key_id(key),
        r#box: sealed_box,
    })
}

/// Returns the plaintext of a value sealed by `seal` under the same key and aad. A value sealed under another key is
/// `Error::WrongKey`; one that its own key does not open is `Error::Damaged`. Neither says more: GCM cannot tell a
/// changed byte from another slot, and the caller knows which slot it asked for.
pub fn open(key: &[u8], aad: &str, s: &Sealed) -> Result<Vec<u8>, Error> {
    let gcm = new_gcm(key)?;
    let id = key_id(key);
    if s.key_id != id {
        return Err(Error::WrongKey {
            sealed: s.key_id.clone(),
            given: id,
        });
    }
    let Some((nonce, ciphertext)) = s.r#box.split_first_chunk::<NONCE_SIZE>() else {
        return Err(Error::Damaged);
    };
    gcm.decrypt(
        nonce.into(),
        Payload {
            msg: ciphertext,
            aad: aad.as_bytes(),
        },
    )
    .map_err(|_| Error::Damaged)
}

fn new_gcm(key: &[u8]) -> Result<Aes256Gcm, Error> {
    if key.len() != KEY_SIZE {
        return Err(Error::KeyLength(key.len()));
    }
    Aes256Gcm::new_from_slice(key).map_err(|_| Error::KeyLength(key.len()))
}
