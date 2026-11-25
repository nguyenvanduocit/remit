//! Attachment bytes are stored once and go with their home.

use model::{AttachmentRef, Time};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, found, ms};

impl Tx<'_> {
    /// Keeps the first upload under this id, including its metadata.
    pub fn put_attachment(&self, meta: &AttachmentRef, home: &str, bytes: &[u8], at: Time) -> Result<()> {
        self.exec(
            "INSERT INTO attachments(id, home, media_type, name, bytes, at) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING",
            params![meta.id, home, meta.media_type, meta.name, bytes, ms(at)],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn attachment(&self, id: &str) -> Result<(String, String, Vec<u8>)> {
        found(
            self.c
                .prepare_cached("SELECT name, media_type, bytes FROM attachments WHERE id=?")?
                .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))),
        )
    }

    pub fn attachment_metadata(&self, id: &str) -> Result<(String, AttachmentRef)> {
        found(
            self.c
                .prepare_cached("SELECT home, name, media_type FROM attachments WHERE id=?")?
                .query_row([id], |r| {
                    Ok((
                        r.get(0)?,
                        AttachmentRef {
                            id: id.to_owned(),
                            name: r.get(1)?,
                            media_type: r.get(2)?,
                        },
                    ))
                }),
        )
    }
}
