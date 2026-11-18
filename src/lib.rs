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
//! ## Theme
//!
//! Colours and font sizes come from an [`EditorStyle`]: a preset, one
//! derived from the application's Iced theme, or your own [`Palette`] and
//! [`Typography`] ([`EditorState::style`]).
//!
//! ```
//! use iced_markdown_editor::{EditorState, EditorStyle, FromTheme};
//!
//! let editor = EditorState::new().style(EditorStyle::dark());
//!
//! // Match the rest of the application.
//! let editor = EditorState::new().style(EditorStyle::from_theme(&iced::Theme::Dark));
//! ```
//!
//! [`EditorState::set_style`] changes it at runtime, e.g. when the
//! application theme changes.
//!
//! ## Languages
//!
//! The toolbar, link bar and status bar speak English, German, French,
//! Spanish and Italian ([`Language`], [`EditorState::language`]). The
//! document itself is never translated.
//!
//! ```
//! use iced_markdown_editor::{EditorState, Language};
//!
//! // E.g. from the operating system locale; unknown tags fall back to English.
//! let language = Language::from_tag("de-CH").unwrap_or_default();
//! let editor = EditorState::new().language(language);
//! ```
//!
//! [`EditorState::set_language`] switches it at runtime.
//!
//! ## Extensions
//!
//! A [`BlockRenderer`] (from your own crate) draws fenced code blocks of
//! chosen languages, e.g. formulas or diagrams
//! ([`EditorState::block_renderer`]). A [`CodeHighlighter`] colours code.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub(crate) mod convert;
pub(crate) mod draw;
pub(crate) mod fonts;
pub mod i18n;
pub mod images;
pub mod style;
pub mod widget;

pub use markdown_editor_core::{extension, features, geometry, highlight, keyboard, page, zoom};

pub(crate) use markdown_editor_core::render;

#[cfg(feature = "syntect")]
#[cfg_attr(docsrs, doc(cfg(feature = "syntect")))]
pub use markdown_editor_core::syntect;

pub use convert::color_from_iced;
pub use extension::{BlockRenderer, RenderRequest};
pub use features::Features;
pub use fonts::FONTS;
pub use geometry::{Color, Point, Rectangle, Size, Vector};
pub use highlight::{CodeHighlighter, HighlightRequest, HighlightSpan};
pub use i18n::Language;
pub use images::{
    ImageId, ImageLoader, ImageMode, ImageRejection, ImageRejectionMessage, ImageSettings,
    ImageSource, PastedImage,
};
pub use keyboard::{Key, Modifiers, Named};
pub use markdown_editor_core::{BlockKind, ColumnAlign, normalize};
pub use page::{IsoSeries, Margins, Measure, Orientation, PageSize, Paper, ParseError, Unit};
pub use style::{EditorStyle, FromTheme, Palette, Typography};
pub use widget::{
    Action, Command, DEFAULT_TAB_WIDTH, EditorCore, EditorState, Event, Formatting, MarkdownEditor,
    Mode, Output, markdown_editor,
};
pub use zoom::{KeyBinding, ZoomSettings, ZoomShortcuts};

#[cfg(feature = "syntect")]
#[cfg_attr(docsrs, doc(cfg(feature = "syntect")))]
pub use markdown_editor_core::SyntectHighlighter;
