//! # iced-markdown-editor
//!
//! A Word-style WYSIWYG CommonMark editor widget for [Iced](https://iced.rs),
//! with the GFM extensions strikethrough, task lists and tables. Desktop and
//! web.
//!
//! ```ignore
//! use iced_markdown_editor::{markdown_editor, Action, EditorState, Event};
//!
//! struct App { editor: EditorState }
//!
//! #[derive(Debug, Clone)]
//! enum Message { Editor(Action) }
//!
//! impl App {
//!     fn update(&mut self, message: Message) -> iced::Task<Message> {
//!         match message {
//!             Message::Editor(action) => {
//!                 let output = self.editor.update(action);
//!                 if output.events.contains(&Event::Changed) {
//!                     println!("{}", self.editor.markdown());
//!                 }
//!                 output.task.map(Message::Editor)
//!             }
//!         }
//!     }
//!
//!     fn view(&self) -> iced::Element<'_, Message> {
//!         markdown_editor(&self.editor).view().map(Message::Editor)
//!     }
//!
//!     fn subscription(&self) -> iced::Subscription<Message> {
//!         self.editor.subscription().map(Message::Editor)
//!     }
//! }
//! ```
//!
//! ## Features
//!
//! | Feature | Default | Adds |
//! |---|---|---|
//! | `syntect` | off | [`SyntectHighlighter`], a ready-made code highlighter |
//! | `images` | off | Showing images, and pasting images from the clipboard (desktop) |
//!
//! ## Tables
//!
//! GFM tables are edited in place: [`Command::InsertTable`], row and column
//! commands, [`Command::SetColumnAlign`] with [`ColumnAlign`]. `Tab` moves to
//! the next cell, `Enter` to the cell below.
//!
//! ## Switching features off
//!
//! Tables, task lists and strikethrough can be disabled at runtime with
//! [`Features`] ([`EditorState::features`]).
//!
//! ## Extensions
//!
//! A [`BlockRenderer`] (from your own crate) draws fenced code blocks of
//! chosen languages, e.g. formulas or diagrams
//! ([`EditorState::block_renderer`]). A [`CodeHighlighter`] colours code.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub(crate) mod doc;
pub mod extension;
pub mod features;
pub(crate) mod fonts;
pub mod highlight;
pub mod images;
pub(crate) mod layout;
mod markdown;
pub mod page;
mod render;
pub mod style;
pub mod widget;
pub mod zoom;

#[cfg(feature = "syntect")]
#[cfg_attr(docsrs, doc(cfg(feature = "syntect")))]
pub mod syntect;

pub use doc::{BlockKind, ColumnAlign};
pub use extension::{BlockRenderer, RenderRequest};
pub use features::Features;
pub use fonts::FONTS;
pub use highlight::{CodeHighlighter, HighlightRequest, HighlightSpan};
pub use images::{
    ImageId, ImageLoader, ImageMode, ImageRejection, ImageSettings, ImageSource, PastedImage,
};
pub use markdown::normalize;
pub use page::{IsoSeries, Margins, Measure, Orientation, PageSize, Paper, ParseError, Unit};
pub use style::{EditorStyle, Palette, Typography};
pub use widget::{
    Action, Command, DEFAULT_TAB_WIDTH, EditorState, Event, Formatting, MarkdownEditor, Mode,
    Output, markdown_editor,
};
pub use zoom::{KeyBinding, ZoomSettings, ZoomShortcuts};

#[cfg(feature = "syntect")]
#[cfg_attr(docsrs, doc(cfg(feature = "syntect")))]
pub use crate::syntect::SyntectHighlighter;
