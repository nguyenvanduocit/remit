//! An owner message carries references to files uploaded in its home.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use model::{ATTACHMENT_MAX_BYTES, Attachment, AttachmentRef, Draft, Event, ForeignJson, Image, Prompt};
use serde::Deserialize;
use wire::app::EventData;

use crate::chain_core::Core;
use crate::outbox::read_of;
use crate::{Error, Result};

impl Core {
    /// An id put again keeps what it was first put with.
    pub async fn put_attachment(&self, id: String, home: String, attachment: Attachment) -> Result<()> {
        if id.is_empty() || home.is_empty() || attachment.name.is_empty() {
            return Err(Error::refused("an attachment needs an id, a home, and a filename"));
        }
        let bytes = STANDARD
            .decode(attachment.data.as_bytes())
            .map_err(|err| Error::refused(format!("attachment {id} is not base64: {err}")))?;
        if bytes.len() > ATTACHMENT_MAX_BYTES {
            return Err(Error::refused(format!(
                "{} exceeds the {ATTACHMENT_MAX_BYTES}-byte attachment limit",
                attachment.name
            )));
        }
        let meta = AttachmentRef {
            id,
            name: attachment.name,
            media_type: attachment.media_type,
        };
        let now = self.now();
        self.store
            .update(move |tx| tx.put_attachment(&meta, &home, &bytes, now))
            .await?;
        Ok(())
    }

    pub async fn attachment(&self, id: String) -> Result<Attachment> {
        let (name, media_type, bytes) = self.store.read(move |r| r.attachment(&id)).await?;
        Ok(Attachment {
            name,
            media_type,
            data: STANDARD.encode(bytes),
        })
    }

    pub(crate) async fn check_draft_attachments(&self, d: &Draft) -> Result<()> {
        for attachment in &d.attachments {
            let id = attachment.id.clone();
            let (home, stored) = match self.store.read(move |r| r.attachment_metadata(&id)).await {
                Err(store::Error::NotFound) => {
                    return Err(Error::refused(format!("attachment {} was not uploaded", attachment.id)));
                }
                other => other?,
            };
            if home != d.home || &stored != attachment {
                return Err(Error::refused(format!(
                    "attachment {} does not match this draft",
                    attachment.id
                )));
            }
        }
        Ok(())
    }

    /// What the agent reads of the message.
    pub async fn prompt_of(&self, e: &Event) -> Result<Prompt> {
        let mut prompt = Prompt::from(read_of(e));
        for reference in attachments_in(e.data.as_ref()) {
            let attachment = self.attachment(reference.id).await?;
            if let Ok(media_type) = attachment.media_type.parse() {
                prompt.images.push(Image {
                    media_type,
                    data: attachment.data,
                });
            } else {
                prompt.files.push(attachment);
            }
        }
        Ok(prompt)
    }
}

pub fn attachments_in(data: Option<&ForeignJson>) -> Vec<AttachmentRef> {
    data.and_then(|d| EventData::deserialize(&d.0).ok())
        .and_then(|d| d.attachments)
        .unwrap_or_default()
}

pub fn with_attachments(data: Option<ForeignJson>, attachments: &[AttachmentRef]) -> Option<ForeignJson> {
    if attachments.is_empty() {
        return data;
    }
    Some(ForeignJson::with_field(
        data.as_ref(),
        "attachments",
        serde_json::json!(attachments),
    ))
}
