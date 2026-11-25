//! Attachments the owner or a mentor puts in a message to an agent. Their bytes are stored once, by an id their channel makes,
//! apart from the records that name them: a draft and the message it becomes carry only `AttachmentRef`s, so a draft saved as
//! it is typed and a message pushed to every page stay small. The agent receives attachments with the message's words
//! (`Prompt`).

use serde::{Deserialize, Serialize};

use crate::text_enum;

text_enum! {
    /// The kinds of image a model reads.
    #[derive(Default)]
    pub enum ImageType {
        #[default]
        Png = "image/png",
        Jpeg = "image/jpeg",
        Gif = "image/gif",
        Webp = "image/webp",
    }
}

/// The most bytes one uploaded attachment may hold.
pub const ATTACHMENT_MAX_BYTES: usize = 5 * 1024 * 1024;

/// An attachment a message holds: the id of its stored bytes and its display metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AttachmentRef {
    pub id: String,
    pub media_type: String,
    pub name: String,
}

/// The bytes of an attachment, base64, with its filename and media type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Attachment {
    pub name: String,
    pub media_type: String,
    pub data: String,
}

/// An image's bytes, base64, the form a model and a page both take them in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Image {
    pub media_type: ImageType,
    pub data: String,
}

/// What an agent reads in one message: its words, inline images and files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prompt {
    pub text: String,
    pub images: Vec<Image>,
    pub files: Vec<Attachment>,
}

impl From<String> for Prompt {
    fn from(text: String) -> Self {
        Self {
            text,
            ..Self::default()
        }
    }
}

impl From<&str> for Prompt {
    fn from(text: &str) -> Self {
        Self::from(text.to_owned())
    }
}
