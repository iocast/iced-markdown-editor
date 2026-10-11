//! Custom rendering of fenced code blocks.
//!
//! A [`BlockRenderer`] draws fenced code blocks of certain languages in
//! WYSIWYG mode instead of showing their code, e.g. formulas for `math`,
//! diagrams for `mermaid` or a chart for `csv`. The document keeps the code
//! block, so the Markdown does not change:
//!
//! * the block is drawn by the renderer while the caret is elsewhere,
//! * clicking it or moving the caret into it shows the code for editing,
//! * without the renderer (or when it returns `None`), it is a normal code
//!   block.
//!
//! Renderers draw with Iced's canvas [`Frame`], so they work on the desktop
//! and in the browser.
//!
//! ```
//! use iced_markdown_editor::extension::{
//!     BlockRenderer, Color, Frame, Point, Rectangle, RenderRequest, Size,
//! };
//!
//! /// Draws ```` ```swatch ```` blocks (one `r g b` per line, 0-255) as colour swatches.
//! struct Swatches;
//!
//! fn colors(code: &str) -> Vec<Color> {
//!     code.lines()
//!         .filter_map(|line| {
//!             let rgb: Vec<u8> = line.split_whitespace().filter_map(|n| n.parse().ok()).collect();
//!             (rgb.len() == 3).then(|| Color::from_rgb8(rgb[0], rgb[1], rgb[2]))
//!         })
//!         .collect()
//! }
//!
//! impl BlockRenderer for Swatches {
//!     fn languages(&self) -> Vec<String> {
//!         vec!["swatch".into()]
//!     }
//!
//!     fn measure(&self, request: &RenderRequest<'_>) -> Option<Size> {
//!         let count = colors(request.code).len();
//!         (count > 0).then(|| Size::new(count as f32 * 40.0 * request.zoom, 32.0 * request.zoom))
//!     }
//!
//!     fn draw(&self, frame: &mut Frame, bounds: Rectangle, request: &RenderRequest<'_>) {
//!         let size = 40.0 * request.zoom;
//!         for (i, color) in colors(request.code).into_iter().enumerate() {
//!             let at = Point::new(bounds.x + i as f32 * size, bounds.y);
//!             frame.fill_rectangle(at, Size::new(size - 4.0, bounds.height), color);
//!         }
//!     }
//! }
//! ```

use crate::style::EditorStyle;

/// Types used by renderers (re-exports from `iced`).
pub use iced::widget::canvas::Frame;
pub use iced::{Color, Point, Rectangle, Size};

/// One code block to measure or draw.
#[derive(Debug, Clone, Copy)]
pub struct RenderRequest<'a> {
    /// The language: first word of the info string, as listed in
    /// [`BlockRenderer::languages`].
    pub language: &'a str,
    /// The whole info string, e.g. `math display`.
    pub info: &'a str,
    /// The code. Lines are separated by `\n`.
    pub code: &'a str,
    /// Available width in pixels (already zoomed).
    pub width: f32,
    /// Height of a page's text area in pixels (already zoomed); infinite in
    /// full-width mode. Taller blocks overflow the page.
    pub max_height: f32,
    /// Zoom factor (1.0 = 100 %). Scale fixed sizes with it.
    pub zoom: f32,
    /// The editor style: colours and font sizes.
    pub style: &'a EditorStyle,
}

/// Draws fenced code blocks of some languages instead of their code.
///
/// [`measure`](Self::measure) is called on layout and cached per code,
/// size and zoom; [`draw`](Self::draw) is called when the page is redrawn.
pub trait BlockRenderer: Send + Sync + 'static {
    /// Languages this renderer draws. They are also offered in the
    /// toolbar's language list.
    fn languages(&self) -> Vec<String>;

    /// Size of the rendered block, at most [`RenderRequest::width`] wide.
    /// `None` shows the code instead (e.g. for invalid input).
    fn measure(&self, request: &RenderRequest<'_>) -> Option<Size>;

    /// Draws the block into `bounds` (the size from [`measure`](Self::measure),
    /// in document coordinates).
    fn draw(&self, frame: &mut Frame, bounds: Rectangle, request: &RenderRequest<'_>);
}
