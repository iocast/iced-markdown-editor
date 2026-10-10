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

use crate::doc::{Block, Document, Link, Style, OBJ};
use crate::markdown;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

/// URL scheme of placeholders for images the application has not stored yet.
pub(crate) const PENDING_PREFIX: &str = "pending-image:";
/// Text shown for a placeholder.
pub(crate) const PENDING_ALT: &str = "Saving image\u{2026}";

/// What the editor does with pasted, dropped or inserted images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageMode {
    /// Images are not accepted.
    #[default]
    Disabled,
    /// Images are written into the Markdown as `data:` URLs.
    Embed,
    /// Images are handed to the application, which stores them and returns
    /// the URL to write into the Markdown.
    Delegate,
}

/// Image handling of an editor.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageSettings {
    pub mode: ImageMode,
    /// Larger images are rejected.
    pub max_bytes: usize,
    /// Accepted MIME types, detected from the file content.
    pub accept: Vec<String>,
    /// Accept image files dropped onto the window.
    pub drop: bool,
}

impl Default for ImageSettings {
    /// Disabled; 10 MiB; PNG, JPEG, GIF and WebP; drops accepted when enabled.
    fn default() -> Self {
        Self {
            mode: ImageMode::Disabled,
            max_bytes: 10 * 1024 * 1024,
            accept: ["image/png", "image/jpeg", "image/gif", "image/webp"]
                .into_iter()
                .map(String::from)
                .collect(),
            drop: true,
        }
    }
}

impl ImageSettings {
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Images become `data:` URLs in the Markdown.
    pub fn embed() -> Self {
        Self {
            mode: ImageMode::Embed,
            ..Self::default()
        }
    }

    /// Images are handed to the application.
    pub fn delegate() -> Self {
        Self {
            mode: ImageMode::Delegate,
            ..Self::default()
        }
    }

    pub fn max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    pub fn accept(mut self, mime_types: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.accept = mime_types.into_iter().map(Into::into).collect();
        self
    }

    pub fn accept_drops(mut self, accept_drops: bool) -> Self {
        self.drop = accept_drops;
        self
    }

    pub(crate) fn accepts(&self, mime: &str) -> bool {
        self.accept.is_empty() || self.accept.iter().any(|accepted| accepted == mime)
    }

    pub(crate) fn enabled(&self) -> bool {
        self.mode != ImageMode::Disabled
    }
}

/// Where an image came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageSource {
    /// Pasted from the clipboard.
    Paste,
    /// A file dropped onto the window.
    Drop,
    /// [`Command::InsertImage`](crate::Command::InsertImage) or
    /// [`Command::InsertImageFile`](crate::Command::InsertImageFile).
    Insert,
}

/// Identifies a pasted image until the application resolves or cancels it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageId(pub(crate) u64);

impl ImageId {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ImageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An image handed to the application ([`ImageMode::Delegate`]).
#[derive(Clone, PartialEq)]
pub struct PastedImage {
    pub id: ImageId,
    /// The encoded image (clipboard pixels are encoded as PNG).
    pub bytes: Arc<[u8]>,
    /// Detected from the content, e.g. `image/png`.
    pub mime: &'static str,
    pub source: ImageSource,
    /// Original file name for dropped and inserted files.
    pub name: Option<String>,
}

impl PastedImage {
    /// File extension for the image type, e.g. `png`.
    pub fn extension(&self) -> &'static str {
        extension_for(self.mime)
    }

    /// The original file name, or `image-<time>-<id>.<ext>` for clipboard images.
    pub fn file_name(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
        let seconds = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        format!("image-{seconds}-{}.{}", self.id, self.extension())
    }
}

impl fmt::Debug for PastedImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PastedImage")
            .field("id", &self.id)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .field("mime", &self.mime)
            .field("source", &self.source)
            .field("name", &self.name)
            .finish()
    }
}

/// Why an image was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRejection {
    /// [`ImageMode::Disabled`].
    Disabled,
    /// Not an accepted image type (the detected MIME type, or `unknown`).
    UnsupportedType(String),
    TooLarge { size: usize, max: usize },
    /// The file could not be read.
    Unreadable(String),
}

impl fmt::Display for ImageRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageRejection::Disabled => f.write_str("images are disabled"),
            ImageRejection::UnsupportedType(mime) => write!(f, "unsupported image type {mime}"),
            ImageRejection::TooLarge { size, max } => {
                write!(f, "image too large ({} KiB, maximum {} KiB)", size / 1024, max / 1024)
            }
            ImageRejection::Unreadable(error) => write!(f, "cannot read the image: {error}"),
        }
    }
}

/// Loads image bytes for display from a URL as written in the Markdown.
/// Return `None` to fall back to `data:` URLs and files.
pub type ImageLoader = Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

// ------------------------------------------------------------------------
// Formats
// ------------------------------------------------------------------------

/// MIME type from the first bytes of the file.
pub(crate) fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    }
}

pub(crate) fn extension_for(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        _ => "bin",
    }
}

/// Whether a dropped file looks like an image (by extension).
pub(crate) fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| {
            matches!(
                extension.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            )
        })
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(BASE64[(n >> 18) as usize & 63] as char);
        out.push(BASE64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            BASE64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| BASE64.iter().position(|b| *b == c).map(|v| v as u32);
    let digits: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        if chunk.len() == 1 {
            return None;
        }
        let mut n = 0u32;
        for (k, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * k);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

pub(crate) fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

pub(crate) fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    let rest = url.strip_prefix("data:")?;
    let (header, data) = rest.split_once(',')?;
    if header.ends_with(";base64") {
        base64_decode(data)
    } else {
        None
    }
}

// ------------------------------------------------------------------------
// Placeholders
// ------------------------------------------------------------------------

pub(crate) fn pending_url(id: ImageId) -> String {
    format!("{PENDING_PREFIX}{}", id.0)
}

pub(crate) fn pending_id(url: &str) -> Option<ImageId> {
    url.strip_prefix(PENDING_PREFIX)?.parse().ok().map(ImageId)
}

/// One replaced placeholder: block, start, old length, new length.
pub(crate) type PendingEdit = (usize, usize, usize, usize);

/// Replaces the placeholder of `id` (`None`: every placeholder) with
/// `![alt](url)`, or removes it when `replacement` is `None`.
pub(crate) fn replace_pending(
    doc: &mut Document,
    id: Option<ImageId>,
    replacement: Option<(&str, &str)>,
) -> Vec<PendingEdit> {
    let matches = |url: &str| match pending_id(url) {
        Some(found) => id.is_none_or(|id| id == found),
        None => false,
    };
    let mut edits = Vec::new();

    for (index, block) in doc.blocks.iter_mut().enumerate() {
        let mut i = 0;
        while i < block.chars.len() {
            let range = if block.leaf.is_plain() {
                plain_placeholder(block, i, &matches)
            } else {
                styled_placeholder(block, i, &matches)
            };
            let Some((start, end)) = range else {
                break;
            };

            let (chars, styles): (Vec<char>, Vec<Style>) = match replacement {
                None => (Vec::new(), Vec::new()),
                Some((url, alt)) if block.leaf.is_plain() => {
                    let text = markdown::image_markdown(alt, url);
                    let count = text.chars().count();
                    (text.chars().collect(), vec![Style::default(); count])
                }
                Some((url, alt)) => {
                    let style = Style {
                        image: Some(Link::new(url, None)),
                        ..block.styles[start].clone()
                    };
                    let chars: Vec<char> = if alt.is_empty() {
                        vec![OBJ]
                    } else {
                        alt.chars().filter(|c| !c.is_control()).collect()
                    };
                    let count = chars.len();
                    (chars, vec![style; count])
                }
            };
            let new_len = chars.len();
            block.chars.splice(start..end, chars);
            block.styles.splice(start..end, styles);
            edits.push((index, start, end - start, new_len));
            i = start + new_len;
        }
    }
    edits
}

/// The next formatted placeholder at or after `from`.
fn styled_placeholder(
    block: &Block,
    from: usize,
    matches: &impl Fn(&str) -> bool,
) -> Option<(usize, usize)> {
    let start = (from..block.chars.len()).find(|k| {
        block.styles[*k]
            .image
            .as_ref()
            .is_some_and(|link| matches(&link.url))
    })?;
    let link = block.styles[start].image.clone();
    let mut end = start + 1;
    while end < block.chars.len() && block.styles[end].image == link {
        end += 1;
    }
    Some((start, end))
}

/// The next `![…](pending-image:N)` text at or after `from`.
fn plain_placeholder(
    block: &Block,
    from: usize,
    matches: &impl Fn(&str) -> bool,
) -> Option<(usize, usize)> {
    let chars = &block.chars;
    let prefix: Vec<char> = format!("]({PENDING_PREFIX}").chars().collect();
    let mut k = from;
    while k + prefix.len() <= chars.len() {
        if chars[k..k + prefix.len()] == prefix[..] {
            let url_start = k + 2;
            let mut url_end = url_start;
            while url_end < chars.len() && chars[url_end] != ')' {
                url_end += 1;
            }
            let url: String = chars[url_start..url_end].iter().collect();
            let open = (from..k)
                .rev()
                .find(|s| chars[*s] == '!' && chars.get(s + 1) == Some(&'['));
            if url_end < chars.len() && matches(&url) {
                if let Some(start) = open {
                    return Some((start, url_end + 1));
                }
            }
        }
        k += 1;
    }
    None
}

/// The document without placeholders (what is saved).
pub(crate) fn without_pending(doc: &Document) -> Document {
    let mut doc = doc.clone();
    replace_pending(&mut doc, None, None);
    doc
}

// ------------------------------------------------------------------------
// Display
// ------------------------------------------------------------------------

/// A decoded image ready to draw.
#[derive(Debug, Clone)]
pub(crate) struct LoadedImage {
    pub width: f32,
    pub height: f32,
    #[cfg(feature = "images")]
    pub handle: iced::widget::image::Handle,
}

/// Images by URL, loaded once.
#[derive(Default)]
pub(crate) struct ImageCache {
    entries: HashMap<String, Option<LoadedImage>>,
}

impl ImageCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Registers bytes for a URL (pasted images, so they show at once).
    pub fn insert_bytes(&mut self, url: &str, bytes: &[u8]) {
        let loaded = decode(bytes);
        self.entries.insert(url.to_string(), loaded);
    }

    pub fn get(
        &mut self,
        url: &str,
        base: Option<&Path>,
        loader: Option<&ImageLoader>,
    ) -> Option<LoadedImage> {
        if !cfg!(feature = "images") || url.starts_with(PENDING_PREFIX) {
            return None;
        }
        if let Some(entry) = self.entries.get(url) {
            return entry.clone();
        }
        let loaded = load_bytes(url, base, loader).and_then(|bytes| decode(&bytes));
        self.entries.insert(url.to_string(), loaded.clone());
        loaded
    }
}

fn load_bytes(url: &str, base: Option<&Path>, loader: Option<&ImageLoader>) -> Option<Vec<u8>> {
    if let Some(bytes) = loader.and_then(|loader| loader(url)) {
        return Some(bytes);
    }
    if url.starts_with("data:") {
        return decode_data_url(url);
    }
    read_file(url, base)
}

/// Local files (desktop only; the browser has no file system).
#[cfg(not(target_arch = "wasm32"))]
fn read_file(url: &str, base: Option<&Path>) -> Option<Vec<u8>> {
    let path = url.strip_prefix("file://").unwrap_or(url);
    if path.contains("://") {
        return None; // remote URLs need a loader
    }
    let path = std::path::PathBuf::from(path.replace("%20", " "));
    let path = if path.is_absolute() {
        path
    } else {
        base?.join(path)
    };
    std::fs::read(path).ok()
}

#[cfg(target_arch = "wasm32")]
fn read_file(_url: &str, _base: Option<&Path>) -> Option<Vec<u8>> {
    None
}

#[cfg(feature = "images")]
fn decode(bytes: &[u8]) -> Option<LoadedImage> {
    let size = imagesize::blob_size(bytes).ok()?;
    Some(LoadedImage {
        width: size.width as f32,
        height: size.height as f32,
        handle: iced::widget::image::Handle::from_bytes(bytes.to_vec()),
    })
}

#[cfg(not(feature = "images"))]
fn decode(_bytes: &[u8]) -> Option<LoadedImage> {
    None
}

// ------------------------------------------------------------------------
// Clipboard (desktop only; Iced's clipboard is text only)
// ------------------------------------------------------------------------

/// An image on the clipboard, encoded as PNG.
#[cfg(all(feature = "images", not(target_arch = "wasm32")))]
pub(crate) fn clipboard_image() -> Option<Vec<u8>> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let image = clipboard.get_image().ok()?;
    encode_png(image.width as u32, image.height as u32, &image.bytes)
}

#[cfg(not(all(feature = "images", not(target_arch = "wasm32"))))]
pub(crate) fn clipboard_image() -> Option<Vec<u8>> {
    None
}

#[cfg(all(feature = "images", not(target_arch = "wasm32")))]
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Leaf;
    use crate::markdown::{parse, serialize};

    #[test]
    fn base64_round_trip() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            let encoded = base64_encode(bytes);
            assert_eq!(base64_decode(&encoded).unwrap(), bytes);
        }
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
    }

    #[test]
    fn detects_types_and_data_urls() {
        assert_eq!(sniff_mime(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_mime(b"RIFF1234WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_mime(b"%PDF"), None);
        let url = data_url("image/png", b"\x89PNG");
        assert_eq!(decode_data_url(&url).unwrap(), b"\x89PNG");
        assert!(is_image_path(Path::new("a/B.JPG")));
        assert!(!is_image_path(Path::new("notes.md")));
    }

    #[test]
    fn resolves_and_cancels_placeholders() {
        let id = ImageId(7);
        let text = format!("before ![x]({}) after", pending_url(id));
        let mut doc = parse(&text);
        replace_pending(&mut doc, Some(id), Some(("assets/a.png", "photo")));
        assert_eq!(serialize(&doc), "before ![photo](assets/a.png) after\n");

        let mut doc = parse(&text);
        replace_pending(&mut doc, Some(ImageId(8)), None);
        assert!(serialize(&doc).contains("pending-image:7"));
        replace_pending(&mut doc, Some(id), None);
        assert_eq!(serialize(&doc), "before  after\n");
    }

    #[test]
    fn placeholders_in_source_lines() {
        let id = ImageId(3);
        let line = format!("x ![{PENDING_ALT}]({}) y", pending_url(id));
        let mut block = Block::new(Vec::new(), Leaf::Line);
        for c in line.chars() {
            block.push(c, Style::default());
        }
        let mut doc = Document {
            blocks: vec![block],
        };
        replace_pending(&mut doc, Some(id), Some(("a b.png", "")));
        assert_eq!(doc.blocks[0].text(), "x ![](<a b.png>) y");
    }

    #[test]
    fn saved_documents_have_no_placeholders() {
        let doc = parse(&format!("![x]({})\n\ntext", pending_url(ImageId(1))));
        assert_eq!(serialize(&without_pending(&doc)), "text\n");
    }
}
