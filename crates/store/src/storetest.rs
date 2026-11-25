//! A store that starts as a new database does, without migrating it: a test binary migrates once and each store starts
//! from a copy of that file. The store's own tests open with `Store::open`: migrating is what they test.

use std::path::Path;
use std::sync::OnceLock;

use crate::store::{Error, Result, Store};

/// The file of a newly migrated database, made once per test binary.
static MIGRATED: OnceLock<std::result::Result<Vec<u8>, String>> = OnceLock::new();

fn migrate() -> std::result::Result<Vec<u8>, String> {
    let dir = std::env::temp_dir().join(format!("storetest-{}", std::process::id()));
    let made = (|| -> Result<Vec<u8>> {
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("m.db");
        // Closing checkpoints the write-ahead log into the file, so the file alone holds the schema.
        Store::open(&path)?.close()?;
        Ok(std::fs::read(&path)?)
    })();
    std::fs::remove_dir_all(&dir).ok();
    made.map_err(|e| e.to_string())
}

/// Opens a new store at path, a copy of the database migrated once. path is the caller's, for a test that reads the
/// file.
pub fn open(path: &Path) -> Result<Store> {
    let bytes = MIGRATED
        .get_or_init(migrate)
        .as_ref()
        .map_err(|e| Error::Refused(e.clone()))?;
    std::fs::write(path, bytes)?;
    Store::open(path)
}
