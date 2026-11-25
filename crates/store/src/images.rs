//! Images: the bytes of the images the owner or a mentor puts in a message, stored once by the id their channel makes and
//! named by the drafts and messages that hold them (`model::ImageRef`). An image goes with its home (`delete_home`).

use model::{ImageType, Time};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, found, ms};

impl Tx<'_> {
    /// Keeps bytes of kind media_type as image id of home. An id put again keeps what it was first put with: the call is
    /// replayed, never an image changed.
    pub fn put_image(&self, id: &str, home: &str, media_type: ImageType, bytes: &[u8], at: Time) -> Result<()> {
        self.exec(
            "INSERT INTO images(id, home, media_type, bytes, at) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",
            params![id, home, media_type.as_str(), bytes, ms(at)],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    /// Image id's kind and bytes; `Error::NotFound` when none was put by that id.
    pub fn image(&self, id: &str) -> Result<(ImageType, Vec<u8>)> {
        let (kind, bytes): (String, Vec<u8>) = found(
            self.c
                .prepare_cached("SELECT media_type, bytes FROM images WHERE id=?")?
                .query_row([id], |r| Ok((r.get(0)?, r.get(1)?))),
        )?;
        let kind = kind.parse().map_err(|why| self.left_out("Image", id, &why))?;
        Ok((kind, bytes))
    }

    /// Image id's home; `Error::NotFound` when none was put by that id.
    pub fn image_home(&self, id: &str) -> Result<String> {
        self.value("SELECT home FROM images WHERE id=?", [id])
    }
}
