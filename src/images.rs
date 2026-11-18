//! Images: paste, drop and insert; embed in the Markdown or hand them to
//! the application; display.
//!
//! The editor never decides where image files live. Depending on
//! [`ImageSettings::mode`]:
//!
//! | Mode | What happens with a pasted, dropped or inserted image |
//! |---|---|
//! | [`ImageMode::Disabled`] (default) | Nothing; [`Event::ImageRejected`](crate::Event::ImageRejected) |
//! | [`ImageMode::Embed`] | Written into the Markdown as a `data:` URL |
//! | [`ImageMode::Delegate`] | A placeholder is inserted and [`Event::ImagePasted`](crate::Event::ImagePasted) hands the image to the application, which answers with [`Command::ResolveImage`](crate::Command::ResolveImage) or [`Command::CancelImage`](crate::Command::CancelImage) |
//!
//! On the web (`wasm32`) there is no file system and no desktop clipboard:
//! images are shown from `data:` URLs and the loader.

use crate::i18n::{self, Language};

pub use markdown_editor_core::images::{
    ImageId, ImageLoader, ImageMode, ImageRejection, ImageSettings, ImageSource, PastedImage,
};

/// Translated reasons for rejected images.
pub trait ImageRejectionMessage {
    /// The reason as a sentence for the user, in `language`.
    fn message(&self, language: Language) -> String;
}

impl ImageRejectionMessage for ImageRejection {
    fn message(&self, language: Language) -> String {
        match self {
            ImageRejection::Disabled => i18n::text(language, "image-rejected-disabled"),
            ImageRejection::UnsupportedType(mime) => i18n::text_with(
                language,
                "image-rejected-type",
                &[("mime", mime.as_str().into())],
            ),
            ImageRejection::TooLarge { size, max } => i18n::text_with(
                language,
                "image-rejected-too-large",
                &[("size", (size / 1024).into()), ("max", (max / 1024).into())],
            ),
            ImageRejection::Unreadable(error) => i18n::text_with(
                language,
                "image-rejected-unreadable",
                &[("error", error.as_str().into())],
            ),
        }
    }
}
