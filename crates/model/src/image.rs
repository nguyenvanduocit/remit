//! Images the owner or a mentor puts in a message to an agent. Their bytes are stored once, by an id their channel makes,
//! apart from the records that name them: a draft and the message it becomes carry only `ImageRef`s, so a draft saved as
//! it is typed and a message pushed to every page stay small. The agent reads an image's bytes with the message's words
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

/// The most bytes one image may hold: the model refuses a larger one.
pub const IMAGE_MAX_BYTES: usize = 5 * 1024 * 1024;

/// An image a message holds: the id of its stored bytes, and their kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImageRef {
    pub id: String,
    pub media_type: ImageType,
}

/// An image's bytes, base64, the form a model and a page both take them in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Image {
    pub media_type: ImageType,
    pub data: String,
}

/// What an agent reads in one message: its words, then the images it holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prompt {
    pub text: String,
    pub images: Vec<Image>,
}

impl From<String> for Prompt {
    fn from(text: String) -> Self {
        Self {
            text,
            images: Vec::new(),
        }
    }
}

impl From<&str> for Prompt {
    fn from(text: &str) -> Self {
        Self::from(text.to_owned())
    }
}
