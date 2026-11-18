//! The public widget API.
//!
//! * [`EditorState`] owns the editor (an [`EditorCore`], whose reading and
//!   writing methods it offers) and the interface language. Keep it in your
//!   application state.
//! * [`markdown_editor`] builds the widget from a `&EditorState`.
//! * The widget produces [`Action`]s. Pass them to
//!   [`EditorState::update`], which returns an [`Output`] with a [`Task`]
//!   and the [`Event`]s that happened.
//! * [`EditorState::execute`] runs a [`Command`] programmatically.
//!
//! * `view`: the widget (page canvas, link bar, status bar).
//! * `toolbar`: the built-in toolbar.
//! * `menu`: the context menu (right mouse button).
//! * `dialog`: the link dialog opened from the context menu.

mod dialog;
mod menu;
mod toolbar;
mod view;

pub use view::{MarkdownEditor, markdown_editor};

pub use markdown_editor_core::editor::{
    Command, DEFAULT_TAB_WIDTH, EditorCore, Event, Formatting, Mode,
};

pub(crate) use markdown_editor_core::editor::{BLINK, Effect, Input as Inner};

use crate::extension::BlockRenderer;
use crate::features::Features;
use crate::fonts;
use crate::highlight::CodeHighlighter;
use crate::i18n::{self, Language};
use crate::images::{ImageLoader, ImageSettings};
use crate::page::Paper;
use crate::style::EditorStyle;
use crate::zoom::ZoomSettings;
use iced::widget::{Id, canvas, operation};
use iced::{Subscription, Task};
use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

/// A message produced by the widget. Opaque: forward it to
/// [`EditorState::update`]. Create one from a [`Command`] with
/// `Action::from(command)` to drive the editor from your own widgets.
#[derive(Debug, Clone)]
pub struct Action(pub(crate) Inner);

impl From<Command> for Action {
    fn from(command: Command) -> Self {
        Action(Inner::Command(command))
    }
}

/// Result of [`EditorState::update`].
pub struct Output {
    /// Must be returned from your `update` (mapped to your message type).
    pub task: Task<Action>,
    /// Events produced by this update, in order.
    pub events: Vec<Event>,
}

/// What the page canvas drew last: layout generation and scroll offsets.
pub(crate) type Drawn = (u64, u32, u32);

/// The editor state. Keep one per editor in your application.
///
/// It dereferences to [`EditorCore`], which has the methods for reading and
/// changing the document, e.g. [`EditorCore::markdown`].
pub struct EditorState {
    core: EditorCore,
    pub(crate) language: Language,
    pub(crate) cache: RefCell<canvas::Cache>,
    pub(crate) drawn: Cell<Option<Drawn>>,
    /// The text fields of the link dialog.
    pub(crate) link_text_id: Id,
    pub(crate) link_url_id: Id,
}

impl Deref for EditorState {
    type Target = EditorCore;

    fn deref(&self) -> &EditorCore {
        &self.core
    }
}

impl DerefMut for EditorState {
    fn deref_mut(&mut self) -> &mut EditorCore {
        &mut self.core
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Builders that only change the core.
macro_rules! builders {
    ($($(#[$meta:meta])* $name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            $(#[$meta])*
            pub fn $name(mut self, $($arg: $ty),*) -> Self {
                self.core = self.core.$name($($arg),*);
                self
            }
        )*
    };
}

/// Setters of the core. Written out, rather than reached through `DerefMut`,
/// so that `editor.set_x(editor.y())` keeps compiling.
macro_rules! setters {
    ($($(#[$meta:meta])* $name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            $(#[$meta])*
            pub fn $name(&mut self, $($arg: $ty),*) {
                self.core.$name($($arg),*);
            }
        )*
    };
}

impl EditorState {
    /// An empty document.
    pub fn new() -> Self {
        Self::with_markdown("")
    }

    /// A document parsed from CommonMark (with the GFM extensions
    /// strikethrough, task lists and tables; see [`Self::features`]).
    pub fn with_markdown(markdown_text: &str) -> Self {
        let mut state = Self {
            core: EditorCore::with_fonts(markdown_text, fonts::shared().clone()),
            language: Language::default(),
            cache: RefCell::new(canvas::Cache::new()),
            drawn: Cell::new(None),
            link_text_id: Id::unique(),
            link_url_id: Id::unique(),
        };
        state.set_language(Language::default());
        state
    }

    builders! {
        /// Builder: sets the style.
        style(style: EditorStyle);
        /// Builder: sets the paper.
        paper(paper: Paper);
        /// Builder: sets the zoom settings.
        zoom_settings(settings: ZoomSettings);
        /// Builder: sets the initial zoom.
        zoom(zoom: f32);
        /// Builder: number of spaces the Tab key inserts (0 = Tab not handled).
        tab_width(spaces: usize);
        /// Builder: installs a code highlighting plugin.
        highlighter(highlighter: impl CodeHighlighter);
        /// Builder: enables or disables optional Markdown features. The
        /// document is read again with the new features.
        features(features: Features);
        /// Builder: adds a renderer for fenced code blocks of some languages.
        /// Call it once per renderer; the first one listing a language wins.
        block_renderer(renderer: impl BlockRenderer);
        /// Builder: what happens with pasted, dropped and inserted images.
        images(settings: ImageSettings);
        /// Builder: folder for relative image URLs (desktop display).
        image_base(folder: impl Into<PathBuf>);
        /// Builder: loads image bytes for display by URL.
        image_loader(loader: impl Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static);
        /// Builder: sets the mode.
        mode(mode: Mode);
    }

    setters! {
        /// Replaces the whole document. Clears history, pending images and
        /// the dirty flag.
        set_markdown(markdown_text: &str);
        /// Clears the dirty flag (call after saving).
        mark_saved();
        /// Sets the style.
        set_style(style: EditorStyle);
        /// Sets the paper.
        set_paper(paper: Paper);
        /// Sets the zoom settings.
        set_zoom_settings(settings: ZoomSettings);
        /// Sets the zoom factor (1.0 = 100 %), also when zoom is disabled.
        set_zoom(zoom: f32);
        /// Spaces per Tab; `0` leaves Tab to the application.
        set_tab_width(spaces: usize);
        /// Installs or removes the code highlighting plugin.
        set_highlighter(highlighter: Option<Arc<dyn CodeHighlighter>>);
        /// Enables or disables optional Markdown features. In WYSIWYG mode
        /// the document is read again; the undo history starts again.
        set_features(features: Features);
        /// Replaces the block renderers. The first renderer listing a
        /// language draws it.
        set_block_renderers(renderers: Vec<Arc<dyn BlockRenderer>>);
        /// Sets what happens with pasted, dropped and inserted images.
        set_image_settings(settings: ImageSettings);
        /// Sets the folder for relative image URLs (desktop display).
        set_image_base(folder: Option<PathBuf>);
        /// Sets the loader of image bytes for display by URL.
        set_image_loader(loader: Option<ImageLoader>);
        /// Forgets loaded images and loads them again.
        reload_images();
        /// Switches between WYSIWYG and source mode. The undo history starts
        /// again after a switch.
        set_mode(mode: Mode);
    }

    /// Builder: sets the language of the toolbar, link bar and status bar.
    pub fn language(mut self, language: Language) -> Self {
        self.set_language(language);
        self
    }

    pub fn current_language(&self) -> Language {
        self.language
    }

    /// Sets the language of the toolbar, link bar and status bar. The
    /// document is not changed.
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
        self.core.pending_image_alt = i18n::text(language, "image-saving");
    }

    /// Runs a command and reports what happened.
    pub fn execute(&mut self, command: Command) -> Output {
        self.update(Action(Inner::Command(command)))
    }

    /// Handles an [`Action`] produced by the widget.
    pub fn update(&mut self, action: Action) -> Output {
        let update = self.core.update(action.0);
        let tasks: Vec<_> = update
            .effects
            .into_iter()
            .map(|effect| self.perform(effect))
            .collect();
        Output {
            task: Task::batch(tasks),
            events: update.events,
        }
    }

    /// Caret blinking and dropped image files:
    /// `state.subscription().map(Message::Editor)`.
    pub fn subscription(&self) -> Subscription<Action> {
        let blink = if self.focused {
            iced::time::every(BLINK).map(|_| Action(Inner::Blink))
        } else {
            Subscription::none()
        };
        let drops = if self.image_settings.enabled() && self.image_settings.drop {
            iced::event::listen_with(file_dropped)
        } else {
            Subscription::none()
        };
        Subscription::batch([blink, drops])
    }

    /// Carries out work the core asked for.
    fn perform(&self, effect: Effect) -> Task<Action> {
        match effect {
            Effect::WriteClipboard(text) => iced::clipboard::write(text),
            Effect::ReadClipboard => iced::clipboard::read().map(|text| Action(Inner::Paste(text))),
            Effect::FocusLinkDialog => {
                let url = self.link_url_id.clone();
                operation::focus(url.clone()).chain(operation::select_all(url))
            }
            Effect::CycleLinkFocus => self.link_dialog_tab(),
        }
    }

    /// Tab in the link dialog: focus the other field.
    fn link_dialog_tab(&self) -> Task<Action> {
        let Some(dialog) = &self.link_dialog else {
            return Task::none();
        };
        let url_id = self.link_url_id.clone();
        if dialog.text.is_none() {
            return operation::focus(url_id);
        }
        let text_id = self.link_text_id.clone();
        operation::is_focused(text_id.clone()).then(move |on_text| {
            let next = if on_text {
                url_id.clone()
            } else {
                text_id.clone()
            };
            operation::focus(next.clone()).chain(operation::select_all(next))
        })
    }
}

/// Files dropped onto the window (images are filtered in the editor).
fn file_dropped(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Action> {
    match event {
        iced::Event::Window(iced::window::Event::FileDropped(path)) => {
            Some(Action(Inner::FileDropped(path)))
        }
        _ => None,
    }
}
