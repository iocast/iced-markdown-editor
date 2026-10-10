//! Code highlighting plugin interface.
//!
//! The editor does not highlight code itself. A highlighter is a plugin:
//! any type implementing [`CodeHighlighter`]. With the `syntect` feature the
//! crate provides a ready-made implementation,
//! [`SyntectHighlighter`](crate::SyntectHighlighter).
//!
//! ```
//! use iced_markdown_editor::{CodeHighlighter, HighlightRequest, HighlightSpan};
//! use iced_markdown_editor::highlight::Color;
//!
//! /// Colours every line starting with `#` as a comment.
//! struct Comments;
//!
//! impl CodeHighlighter for Comments {
//!     fn highlight(&self, request: &HighlightRequest<'_>) -> Vec<HighlightSpan> {
//!         let mut spans = Vec::new();
//!         let mut offset = 0;
//!         for line in request.code.split('\n') {
//!             let len = line.chars().count();
//!             if line.trim_start().starts_with('#') {
//!                 spans.push(HighlightSpan {
//!                     range: offset..offset + len,
//!                     color: Color::from_rgb(0.4, 0.55, 0.4),
//!                 });
//!             }
//!             offset += len + 1;
//!         }
//!         spans
//!     }
//! }
//! ```

use std::ops::Range;

/// Colour type used by highlighters (re-export of `iced::Color`).
pub use iced::Color;

/// One code block to highlight.
#[derive(Debug, Clone, Copy)]
pub struct HighlightRequest<'a> {
    /// The code block language: the first word of the fence info string.
    pub language: &'a str,
    /// The code. Lines are separated by `\n`.
    pub code: &'a str,
    /// Background colour of the code block.
    pub background: Color,
    /// `true` when the background is dark; pick a matching colour theme.
    pub dark: bool,
}

/// A coloured range of the code.
#[derive(Debug, Clone, PartialEq)]
pub struct HighlightSpan {
    /// **Character** offsets (not bytes) into [`HighlightRequest::code`].
    pub range: Range<usize>,
    pub color: Color,
}

/// A code highlighting plugin. Results are cached per code block content,
/// language and background.
pub trait CodeHighlighter: Send + Sync + 'static {
    /// Colours for one code block. Return an empty list for unknown languages.
    fn highlight(&self, request: &HighlightRequest<'_>) -> Vec<HighlightSpan>;

    /// Language names offered in the toolbar's language list. When empty,
    /// the editor offers [`COMMON_LANGUAGES`].
    fn languages(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Language list used when no highlighter is set or it lists none.
pub const COMMON_LANGUAGES: &[&str] = &[
    "bash",
    "c",
    "cpp",
    "csharp",
    "css",
    "go",
    "html",
    "java",
    "javascript",
    "json",
    "kotlin",
    "markdown",
    "python",
    "ruby",
    "rust",
    "sql",
    "swift",
    "toml",
    "typescript",
    "xml",
    "yaml",
];

/// Relative luminance used to decide whether a background is dark.
pub(crate) fn is_dark(color: Color) -> bool {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b < 0.5
}
