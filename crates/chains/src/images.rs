//! The images the owner or a mentor puts in a message to an agent. A channel puts an image's bytes first, by an id it makes,
//! in the home of the box it is typed in; the draft names it (`Draft::images`), the message it becomes keeps the names in
//! its data (`EventData::images`), and the agent reads the bytes with the words (`prompt_of`).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use model::{Draft, Event, ForeignJson, IMAGE_MAX_BYTES, Image, ImageRef, Prompt};
use serde::Deserialize;
use wire::app::EventData;

use crate::chain_core::Core;
use crate::outbox::read_of;
use crate::{Error, Result};

impl Core {
    /// Keeps image as id in home. An id put again keeps what it was first put with, so a replayed call changes nothing.
    pub async fn put_image(&self, id: String, home: String, image: Image) -> Result<()> {
        if id.is_empty() || home.is_empty() {
            return Err(Error::refused("an image needs an id and the home it is put in"));
        }
        let bytes = STANDARD
            .decode(image.data.as_bytes())
            .map_err(|err| Error::refused(format!("image {id} is not base64: {err}")))?;
        if bytes.is_empty() {
            return Err(Error::refused(format!("image {id} is empty")));
        }
        if bytes.len() > IMAGE_MAX_BYTES {
            return Err(Error::refused(format!(
                "image {id} is {} bytes; a model reads one of at most {IMAGE_MAX_BYTES}",
                bytes.len()
            )));
        }
        let now = self.now();
        self.store
            .update(move |tx| tx.put_image(&id, &home, image.media_type, &bytes, now))
            .await?;
        Ok(())
    }

    /// Image id's kind and bytes.
    pub async fn image(&self, id: String) -> Result<Image> {
        let (media_type, bytes) = self.store.read(move |r| r.image(&id)).await?;
        Ok(Image {
            media_type,
            data: STANDARD.encode(bytes),
        })
    }

    /// Refuses a draft naming an image that was not put in its home.
    pub(crate) async fn check_draft_images(&self, d: &Draft) -> Result<()> {
        for i in &d.images {
            let id = i.id.clone();
            let home = match self.store.read(move |r| r.image_home(&id)).await {
                Err(store::Error::NotFound) => return Err(Error::refused(format!("image {} was not put", i.id))),
                other => other?,
            };
            if home != d.home {
                return Err(Error::refused(format!("image {} is not of this draft's home", i.id)));
            }
        }
        Ok(())
    }

    /// What the agent reads of message e: its note and words (`read_of`), then the bytes of the images it holds.
    pub async fn prompt_of(&self, e: &Event) -> Result<Prompt> {
        let mut images = Vec::new();
        for i in images_in(e.data.as_ref()) {
            images.push(self.image(i.id).await?);
        }
        Ok(Prompt {
            text: read_of(e),
            images,
        })
    }
}

/// The images a message's data names.
pub fn images_in(data: Option<&ForeignJson>) -> Vec<ImageRef> {
    data.and_then(|d| EventData::deserialize(&d.0).ok())
        .and_then(|d| d.images)
        .unwrap_or_default()
}

/// A message's data with images named in it; data as it is when there are none.
pub fn with_images(data: Option<ForeignJson>, images: &[ImageRef]) -> Option<ForeignJson> {
    if images.is_empty() {
        return data;
    }
    Some(ForeignJson::with_field(
        data.as_ref(),
        "images",
        serde_json::json!(images),
    ))
}
