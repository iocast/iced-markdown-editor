//! The public widget API.
//!
//! * [`EditorState`] owns the document, selection, history, paper, zoom,
//!   tab width, highlighter, image settings and style. Keep it in your
//!   application state.
//! * [`markdown_editor`] builds the widget from a `&EditorState`.
//! * The widget produces [`Action`]s. Pass them to
//!   [`EditorState::update`], which returns an [`Output`] with a [`Task`]
//!   and the [`Event`]s that happened.
//! * [`EditorState::execute`] runs a [`Command`] programmatically.

mod edit;
mod source;
mod view;

pub use view::{markdown_editor, MarkdownEditor};

use crate::doc::{Block, BlockKind, Container, Document, Leaf, Pos, Selection, Style};
use crate::fonts::Fonts;
use crate::highlight::{self, CodeHighlighter, HighlightRequest, COMMON_LANGUAGES};
use crate::images::{
    self, ImageCache, ImageId, ImageLoader, ImageRejection, ImageSettings, ImageSource,
    PastedImage,
};
use crate::layout::{compute as compute_layout, Layout};
use crate::markdown;
use crate::page::Paper;
use crate::style::EditorStyle;
use crate::zoom::ZoomSettings;
use iced::keyboard::{Key, Modifiers};
use iced::mouse::ScrollDelta;
use iced::widget::canvas;
use iced::{Color, Size, Subscription, Task};
use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
// `std::time::Instant` panics in the browser; `web_time` is std on desktop.
use web_time::Instant;

pub(crate) const BLINK: Duration = Duration::from_millis(530);

/// Default number of spaces inserted by the Tab key.
pub const DEFAULT_TAB_WIDTH: usize = 4;

/// Editing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Formatted, Word-like editing.
    Wysiwyg,
    /// Raw Markdown text.
    Source,
}

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

#[derive(Debug, Clone)]
pub(crate) enum Inner {
    Key(Key, Modifiers, Option<String>),
    Modifiers(Modifiers),
    /// Press in document coordinates.
    Press { x: f32, y: f32, clicks: u8 },
    /// Drag in document coordinates.
    Drag { x: f32, y: f32 },
    PressOutside,
    Blink,
    /// Scroll by a distance in pixels.
    ScrollBy { dx: f32, dy: f32 },
    /// Scroll to an absolute offset (scrollbar drag).
    ScrollTo { x: Option<f32>, y: Option<f32> },
    /// Ctrl/Cmd + wheel: zoom.
    Wheel(ScrollDelta),
    Command(Command),
    LinkUrl(String),
    LinkApply,
    LinkRemove,
    LinkCancel,
    Paste(Option<String>),
    /// A file dropped onto the window.
    FileDropped(PathBuf),
}

/// Operations that can be triggered programmatically.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Undo,
    Redo,
    ToggleBold,
    ToggleItalic,
    ToggleStrike,
    ToggleCode,
    /// Changes the paragraph(s) under the selection: block type, list,
    /// task list or block quote. Applying the current kind again reverts.
    SetBlock(BlockKind),
    /// Checks or unchecks the task list item(s) under the selection.
    ToggleChecked,
    /// Sets the language of the code block(s) under the selection.
    SetCodeLanguage(Option<String>),
    InsertRule,
    /// Opens the link bar for the selection (or word under the caret).
    EditLink,
    /// Inserts text at the caret, replacing the selection.
    InsertText(String),
    /// Same as the Tab key.
    Indent,
    /// Same as Shift+Tab.
    Outdent,
    SelectAll,
    Copy,
    Cut,
    /// Pastes an image (with the `images` feature, desktop) or text.
    Paste,
    /// Inserts an image from its encoded bytes, following the [`ImageSettings`].
    InsertImage {
        bytes: Vec<u8>,
        /// File name, used as description and in [`PastedImage::name`].
        name: Option<String>,
    },
    /// Like [`Command::InsertImage`] with the content of a file (desktop).
    InsertImageFile(PathBuf),
    /// Inserts `![alt](url)` for an image that already has a URL.
    InsertImageLink { url: String, alt: String },
    /// Answer to [`Event::ImagePasted`]: the placeholder becomes `![alt](url)`.
    ResolveImage {
        id: ImageId,
        url: String,
        alt: String,
    },
    /// Answer to [`Event::ImagePasted`]: the placeholder is removed.
    CancelImage { id: ImageId },
    /// Emits [`Event::ImageRequested`] (the toolbar's image button).
    RequestImage,
    SetMode(Mode),
    Focus,
    Blur,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    /// Sets a zoom factor (1.0 = 100 %).
    SetZoom(f32),
    /// Zooms so the page width fits the widget.
    FitWidth,
    /// Zooms so a whole page fits the widget.
    FitPage,
}

/// Something that happened inside the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The document content changed. Call [`EditorState::markdown`] here,
    /// not in `view`.
    Changed,
    /// The caret or selection moved.
    SelectionChanged,
    ModeChanged(Mode),
    /// The zoom factor changed (1.0 = 100 %).
    ZoomChanged(f32),
    Focused,
    Blurred,
    /// A link was Ctrl/Cmd-clicked.
    LinkClicked(String),
    /// An image to store ([`ImageMode::Delegate`](crate::ImageMode::Delegate)).
    /// Answer with [`Command::ResolveImage`] or [`Command::CancelImage`].
    ImagePasted(PastedImage),
    /// An image was not accepted.
    ImageRejected {
        source: ImageSource,
        reason: ImageRejection,
    },
    /// The toolbar's image button was pressed. Show a file dialog.
    ImageRequested,
}

/// Result of [`EditorState::update`].
pub struct Output {
    /// Must be returned from your `update` (mapped to your message type).
    pub task: Task<Action>,
    /// Events produced by this update, in order.
    pub events: Vec<Event>,
}

/// Formatting at the caret, for building custom toolbars.
#[derive(Debug, Clone, PartialEq)]
pub struct Formatting {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<String>,
    /// Block type: `Paragraph`, `Heading`, `Code`, `Rule` or `Html`.
    pub block: BlockKind,
    /// Innermost list: `Some(true)` numbered, `Some(false)` bullets.
    pub list: Option<bool>,
    /// Task state of the innermost list item.
    pub task: Option<bool>,
    /// Number of nested lists at the caret.
    pub list_depth: usize,
    /// The caret is inside a block quote.
    pub quote: bool,
    /// Language of the code block at the caret, if any.
    pub code_language: Option<String>,
}

pub(crate) struct Snapshot {
    pub doc: Document,
    pub sel: Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditKind {
    None,
    Typing,
    Delete,
    Other,
}

/// The editor state. Keep one per editor in your application.
pub struct EditorState {
    pub(crate) doc: Document,
    pub(crate) sel: Selection,
    pub(crate) layout: RefCell<Layout>,
    pub(crate) cache: RefCell<canvas::Cache>,
    pub(crate) viewport: Cell<Size>,
    pub(crate) style: EditorStyle,
    pub(crate) paper: Paper,
    pub(crate) zoom: f32,
    pub(crate) zoom_settings: ZoomSettings,
    pub(crate) tab_width: usize,
    pub(crate) highlighter: Option<Arc<dyn CodeHighlighter>>,
    pub(crate) highlight_cache: RefCell<HashMap<u64, Arc<Vec<Color>>>>,
    pub(crate) image_settings: ImageSettings,
    pub(crate) image_base: Option<PathBuf>,
    pub(crate) image_loader: Option<ImageLoader>,
    pub(crate) image_cache: RefCell<ImageCache>,
    pub(crate) pending_images: HashMap<ImageId, Arc<[u8]>>,
    pub(crate) next_image_id: u64,
    /// Long `data:` URLs shortened in source mode.
    pub(crate) embedded: source::Embedded,
    pub(crate) mode: Mode,
    pub(crate) focused: bool,
    pub(crate) caret_on: bool,
    pub(crate) link_url: Option<String>,
    pub(crate) link_range: Option<(Pos, Pos)>,
    pub(crate) fonts: Fonts,
    pub(crate) pending: Option<Style>,
    pub(crate) goal_x: Option<f32>,
    pub(crate) undo: Vec<Snapshot>,
    pub(crate) redo: Vec<Snapshot>,
    pub(crate) last_edit: EditKind,
    pub(crate) modifiers: Modifiers,
    pub(crate) last_activity: Instant,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
    pub(crate) revision: u64,
    pub(crate) dirty: bool,
    pub(crate) pending_events: Vec<Event>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorState {
    /// An empty document.
    pub fn new() -> Self {
        Self::with_markdown("")
    }

    /// A document parsed from CommonMark (with GFM strikethrough and task lists).
    pub fn with_markdown(markdown_text: &str) -> Self {
        let state = Self {
            doc: markdown::parse(markdown_text),
            sel: Selection::caret(Pos::new(0, 0)),
            layout: RefCell::new(Layout::default()),
            cache: RefCell::new(canvas::Cache::new()),
            viewport: Cell::new(Size::new(1000.0, 700.0)),
            style: EditorStyle::default(),
            paper: Paper::default(),
            zoom: 1.0,
            zoom_settings: ZoomSettings::default(),
            tab_width: DEFAULT_TAB_WIDTH,
            highlighter: None,
            highlight_cache: RefCell::new(HashMap::new()),
            image_settings: ImageSettings::default(),
            image_base: None,
            image_loader: None,
            image_cache: RefCell::new(ImageCache::default()),
            pending_images: HashMap::new(),
            next_image_id: 1,
            embedded: source::Embedded::default(),
            mode: Mode::Wysiwyg,
            focused: false,
            caret_on: true,
            link_url: None,
            link_range: None,
            fonts: Fonts::load(),
            pending: None,
            goal_x: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: EditKind::None,
            modifiers: Modifiers::empty(),
            last_activity: Instant::now(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            revision: 0,
            dirty: false,
            pending_events: Vec::new(),
        };
        state.relayout();
        state
    }

    /// Builder: sets the style.
    pub fn style(mut self, style: EditorStyle) -> Self {
        self.set_style(style);
        self
    }

    /// Builder: sets the paper.
    pub fn paper(mut self, paper: Paper) -> Self {
        self.set_paper(paper);
        self
    }

    /// Builder: sets the zoom settings.
    pub fn zoom_settings(mut self, settings: ZoomSettings) -> Self {
        self.set_zoom_settings(settings);
        self
    }

    /// Builder: sets the initial zoom.
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.set_zoom(zoom);
        self
    }

    /// Builder: number of spaces the Tab key inserts (0 = Tab not handled).
    pub fn tab_width(mut self, spaces: usize) -> Self {
        self.set_tab_width(spaces);
        self
    }

    /// Builder: installs a code highlighting plugin.
    pub fn highlighter(mut self, highlighter: impl CodeHighlighter) -> Self {
        self.set_highlighter(Some(Arc::new(highlighter)));
        self
    }

    /// Builder: what happens with pasted, dropped and inserted images.
    pub fn images(mut self, settings: ImageSettings) -> Self {
        self.set_image_settings(settings);
        self
    }

    /// Builder: folder for relative image URLs (desktop display).
    pub fn image_base(mut self, folder: impl Into<PathBuf>) -> Self {
        self.set_image_base(Some(folder.into()));
        self
    }

    /// Builder: loads image bytes for display by URL.
    pub fn image_loader(
        mut self,
        loader: impl Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static,
    ) -> Self {
        self.set_image_loader(Some(Arc::new(loader)));
        self
    }

    /// Builder: sets the mode.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.set_mode(mode);
        self
    }

    // ----------------------------------------------------------------
    // Reading
    // ----------------------------------------------------------------

    /// The document as Markdown, without image placeholders and with full
    /// `data:` URLs. Builds the whole text: call it on changes, not in `view`.
    pub fn markdown(&self) -> String {
        match self.mode {
            Mode::Wysiwyg => markdown::serialize(&images::without_pending(&self.doc)),
            Mode::Source => self
                .embedded
                .expand(&source::text(&images::without_pending(&self.doc))),
        }
    }

    /// The document as plain text.
    pub fn plain_text(&self) -> String {
        self.doc.plain_text(self.doc.start(), self.doc.end())
    }

    /// The selected text, if any.
    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.sel.range();
        (a != b).then(|| self.embedded.expand(&self.doc.plain_text(a, b)))
    }

    pub fn word_count(&self) -> usize {
        self.doc.word_count()
    }

    pub fn current_mode(&self) -> Mode {
        self.mode
    }

    pub fn current_style(&self) -> &EditorStyle {
        &self.style
    }

    pub fn current_paper(&self) -> &Paper {
        &self.paper
    }

    /// Current zoom factor (1.0 = 100 %).
    pub fn current_zoom(&self) -> f32 {
        self.zoom
    }

    pub fn current_zoom_settings(&self) -> &ZoomSettings {
        &self.zoom_settings
    }

    /// Spaces per Tab (0 = Tab not handled).
    pub fn current_tab_width(&self) -> usize {
        self.tab_width
    }

    pub fn current_image_settings(&self) -> &ImageSettings {
        &self.image_settings
    }

    /// Images handed to the application and not answered yet.
    pub fn pending_images(&self) -> usize {
        self.pending_images.len()
    }

    pub fn has_highlighter(&self) -> bool {
        self.highlighter.is_some()
    }

    /// Languages offered for code blocks.
    pub fn code_languages(&self) -> Vec<String> {
        let languages = self
            .highlighter
            .as_ref()
            .map(|highlighter| highlighter.languages())
            .unwrap_or_default();
        if languages.is_empty() {
            COMMON_LANGUAGES.iter().map(|name| name.to_string()).collect()
        } else {
            languages
        }
    }

    /// `(checked, total)` task list items.
    pub fn task_progress(&self) -> (usize, usize) {
        let mut seen = Vec::new();
        let mut checked = 0;
        for block in &self.doc.blocks {
            for container in &block.containers {
                if let Container::Item {
                    id, task: Some(done), ..
                } = container
                {
                    if !seen.contains(id) {
                        seen.push(*id);
                        if *done {
                            checked += 1;
                        }
                    }
                }
            }
        }
        (checked, seen.len())
    }

    /// Number of pages (1 in full-width mode).
    pub fn page_count(&self) -> usize {
        self.layout.borrow().pages.len().max(1)
    }

    /// Page (1-based) that contains the caret.
    pub fn current_page(&self) -> usize {
        self.layout.borrow().page_of(self.sel.head) + 1
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// `true` when the content changed since the last load or save.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Formatting at the caret (or at the start of the selection).
    pub fn formatting(&self) -> Formatting {
        let (a, b) = self.sel.range();
        let style = if a == b {
            self.typing_style()
        } else {
            self.doc.blocks[a.block]
                .styles
                .get(a.offset)
                .cloned()
                .unwrap_or_default()
        };
        let block = &self.doc.blocks[self.sel.head.block];
        let leaf_kind = match block.leaf {
            Leaf::Paragraph | Leaf::Line => BlockKind::Paragraph,
            Leaf::Heading(level) => BlockKind::Heading(level),
            Leaf::Code => BlockKind::Code,
            Leaf::Html => BlockKind::Html,
            Leaf::Rule => BlockKind::Rule,
        };
        let item = block
            .containers
            .iter()
            .rev()
            .find(|c| matches!(c, Container::Item { .. }));
        Formatting {
            bold: style.bold,
            italic: style.italic,
            strike: style.strike,
            code: style.code,
            link: style.link.map(|link| link.url.clone()),
            block: leaf_kind,
            list: item.and_then(Container::list).map(|list| list.ordered),
            task: item.and_then(Container::task),
            list_depth: block
                .containers
                .iter()
                .filter(|c| c.list().is_some())
                .count(),
            quote: block.containers.iter().any(Container::is_quote),
            code_language: if block.leaf == Leaf::Code {
                block.language().map(str::to_string)
            } else {
                None
            },
        }
    }

    // ----------------------------------------------------------------
    // Writing
    // ----------------------------------------------------------------

    /// Replaces the whole document. Clears history, pending images and the
    /// dirty flag.
    pub fn set_markdown(&mut self, markdown_text: &str) {
        self.doc = match self.mode {
            Mode::Wysiwyg => markdown::parse(markdown_text),
            Mode::Source => source::document(&self.embedded.shorten(markdown_text)),
        };
        self.sel = Selection::caret(Pos::new(0, 0));
        self.undo.clear();
        self.redo.clear();
        self.pending_images.clear();
        self.reset_transient();
        self.dirty = false;
        self.revision += 1;
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        self.relayout();
    }

    /// Clears the dirty flag (call after saving).
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn set_style(&mut self, style: EditorStyle) {
        self.style = style;
        self.relayout();
    }

    pub fn set_paper(&mut self, paper: Paper) {
        self.paper = paper;
        self.relayout();
        self.clamp_scroll();
    }

    pub fn set_zoom_settings(&mut self, settings: ZoomSettings) {
        self.zoom_settings = settings;
        self.zoom = self.zoom_settings.clamp(self.zoom);
        self.relayout();
        self.clamp_scroll();
    }

    /// Sets the zoom factor (1.0 = 100 %), also when zoom is disabled.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = self.zoom_settings.clamp(zoom);
        self.relayout();
        self.clamp_scroll();
    }

    /// Spaces per Tab; `0` leaves Tab to the application.
    pub fn set_tab_width(&mut self, spaces: usize) {
        self.tab_width = spaces.min(16);
    }

    pub fn set_highlighter(&mut self, highlighter: Option<Arc<dyn CodeHighlighter>>) {
        self.highlighter = highlighter;
        self.highlight_cache.borrow_mut().clear();
        self.relayout();
    }

    pub fn set_image_settings(&mut self, settings: ImageSettings) {
        self.image_settings = settings;
    }

    pub fn set_image_base(&mut self, folder: Option<PathBuf>) {
        self.image_base = folder;
        self.reload_images();
    }

    pub fn set_image_loader(&mut self, loader: Option<ImageLoader>) {
        self.image_loader = loader;
        self.reload_images();
    }

    /// Forgets loaded images and loads them again.
    pub fn reload_images(&mut self) {
        self.image_cache.borrow_mut().clear();
        self.relayout();
    }

    /// Switches between WYSIWYG and source mode. The undo history starts
    /// again after a switch.
    pub fn set_mode(&mut self, mode: Mode) {
        if mode == self.mode {
            return;
        }
        self.doc = match mode {
            Mode::Source => {
                let text = markdown::serialize(&self.doc);
                let text = self.embedded.shorten(&text);
                source::document(&text)
            }
            Mode::Wysiwyg => {
                let text = self.embedded.expand(&source::text(&self.doc));
                markdown::parse(&text)
            }
        };
        self.mode = mode;
        self.sel = Selection::caret(Pos::new(0, 0));
        self.undo.clear();
        self.redo.clear();
        self.reset_transient();
        self.highlight_cache.borrow_mut().clear();
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        self.relayout();
    }

    /// Runs a command and reports what happened.
    pub fn execute(&mut self, command: Command) -> Output {
        self.update(Action(Inner::Command(command)))
    }

    /// Handles an [`Action`] produced by the widget.
    pub fn update(&mut self, action: Action) -> Output {
        let revision = self.revision;
        let sel = self.sel;
        let mode = self.mode;
        let focused = self.focused;
        let zoom = self.zoom;

        let task = self.apply(action.0);

        let mut events = std::mem::take(&mut self.pending_events);
        if self.focused != focused {
            events.push(if self.focused {
                Event::Focused
            } else {
                Event::Blurred
            });
        }
        if self.mode != mode {
            events.push(Event::ModeChanged(self.mode));
        }
        if (self.zoom - zoom).abs() > f32::EPSILON {
            events.push(Event::ZoomChanged(self.zoom));
        }
        if self.revision != revision {
            events.push(Event::Changed);
        }
        if self.sel != sel {
            events.push(Event::SelectionChanged);
        }

        Output { task, events }
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

    // ----------------------------------------------------------------
    // Internal helpers
    // ----------------------------------------------------------------

    pub(crate) fn expand_width(&self) -> usize {
        if self.tab_width == 0 {
            DEFAULT_TAB_WIDTH
        } else {
            self.tab_width
        }
    }

    pub(crate) fn new_image_id(&mut self) -> ImageId {
        let id = ImageId(self.next_image_id);
        self.next_image_id += 1;
        id
    }

    /// Highlight colours for a code block, from the plugin (cached).
    pub(crate) fn code_colors(&self, index: usize) -> Option<Arc<Vec<Color>>> {
        let highlighter = self.highlighter.as_ref()?;
        let block = self.doc.blocks.get(index)?;
        if block.leaf != Leaf::Code || block.chars.is_empty() {
            return None;
        }
        let language = block.language()?;
        let palette = &self.style.palette;

        let mut hasher = DefaultHasher::new();
        language.hash(&mut hasher);
        block.chars.hash(&mut hasher);
        for channel in [palette.code_background, palette.code] {
            channel.r.to_bits().hash(&mut hasher);
            channel.g.to_bits().hash(&mut hasher);
            channel.b.to_bits().hash(&mut hasher);
        }
        let key = hasher.finish();

        if let Some(colors) = self.highlight_cache.borrow().get(&key) {
            return Some(colors.clone());
        }

        let code: String = block.chars.iter().collect();
        let spans = highlighter.highlight(&HighlightRequest {
            language,
            code: &code,
            background: palette.code_background,
            dark: highlight::is_dark(palette.code_background),
        });

        let len = block.chars.len();
        let mut colors = vec![palette.code; len];
        for span in spans {
            let start = span.range.start.min(len);
            let end = span.range.end.min(len);
            for color in &mut colors[start..end.max(start)] {
                *color = span.color;
            }
        }

        let colors = Arc::new(colors);
        let mut cache = self.highlight_cache.borrow_mut();
        if cache.len() > 512 {
            cache.clear();
        }
        cache.insert(key, colors.clone());
        Some(colors)
    }

    pub(crate) fn relayout(&self) {
        let cache = &self.image_cache;
        let base = self.image_base.as_deref();
        let loader = self.image_loader.as_ref();
        let lookup = |url: &str| cache.borrow_mut().get(url, base, loader);
        let layout = compute_layout(
            &self.doc,
            &self.fonts,
            &self.style,
            &self.paper,
            self.viewport.get(),
            self.zoom,
            &lookup,
        );
        *self.layout.borrow_mut() = layout;
        self.cache.borrow_mut().clear();
    }

    /// Called while drawing with the widget size.
    pub(crate) fn sync_viewport(&self, size: Size) {
        let current = self.viewport.get();
        if (current.width - size.width).abs() > 0.5 || (current.height - size.height).abs() > 0.5 {
            self.viewport.set(size);
            self.relayout();
        }
    }

    /// Largest scroll offsets for the current layout and widget size.
    pub(crate) fn max_scroll(&self) -> (f32, f32) {
        let layout = self.layout.borrow();
        let viewport = self.viewport.get();
        (
            (layout.width - viewport.width).max(0.0),
            (layout.height - viewport.height).max(0.0),
        )
    }

    /// Scroll offsets clamped to the content (used while drawing).
    pub(crate) fn clamped_scroll(&self) -> (f32, f32) {
        let (max_x, max_y) = self.max_scroll();
        (self.scroll_x.clamp(0.0, max_x), self.scroll_y.clamp(0.0, max_y))
    }

    pub(crate) fn set_scroll(&mut self, x: f32, y: f32) {
        let (max_x, max_y) = self.max_scroll();
        let x = x.clamp(0.0, max_x);
        let y = y.clamp(0.0, max_y);
        if (x - self.scroll_x).abs() > 0.01 || (y - self.scroll_y).abs() > 0.01 {
            self.scroll_x = x;
            self.scroll_y = y;
            self.cache.borrow_mut().clear();
        }
    }

    pub(crate) fn clamp_scroll(&mut self) {
        let (x, y) = (self.scroll_x, self.scroll_y);
        self.set_scroll(x, y);
    }

    pub(crate) fn reset_transient(&mut self) {
        self.pending = None;
        self.goal_x = None;
        self.last_edit = EditKind::None;
        self.link_url = None;
        self.link_range = None;
    }

    pub(crate) fn touch(&mut self) {
        self.last_activity = Instant::now();
        self.caret_on = true;
    }

    pub(crate) fn typing_style(&self) -> Style {
        match &self.pending {
            Some(style) => style.clone(),
            None => self.doc.typing_style(self.sel.range().0),
        }
    }

    pub(crate) fn begin_edit(&mut self, kind: EditKind) {
        let coalesce =
            kind == self.last_edit && matches!(kind, EditKind::Typing | EditKind::Delete);
        if !coalesce {
            self.undo.push(Snapshot {
                doc: self.doc.clone(),
                sel: self.sel,
            });
            if self.undo.len() > 300 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = kind;
    }

    pub(crate) fn after_edit(&mut self) -> Task<Action> {
        self.dirty = true;
        self.revision += 1;
        self.goal_x = None;
        self.relayout();
        self.touch();
        self.scroll_to_caret()
    }

    /// Scrolls so that the caret is visible.
    pub(crate) fn scroll_to_caret(&mut self) -> Task<Action> {
        let caret = self.layout.borrow().caret(self.sel.head);
        let Some((x, y, height)) = caret else {
            return Task::none();
        };
        let viewport = self.viewport.get();
        let margin = 24.0;
        let (mut sx, mut sy) = (self.scroll_x, self.scroll_y);

        if y < sy + margin {
            sy = y - 2.0 * margin;
        } else if y + height > sy + viewport.height - margin {
            sy = y + height - viewport.height + 2.0 * margin;
        }
        if !self.paper.is_fill() {
            if x < sx + margin {
                sx = x - 4.0 * margin;
            } else if x > sx + viewport.width - margin {
                sx = x - viewport.width + 4.0 * margin;
            }
        }
        self.set_scroll(sx, sy);
        Task::none()
    }

    /// Changes the zoom and keeps the visible area roughly in place.
    pub(crate) fn zoom_to(&mut self, zoom: f32) -> Task<Action> {
        let zoom = self.zoom_settings.clamp(zoom);
        if (zoom - self.zoom).abs() < 1.0e-4 {
            return Task::none();
        }
        let ratio = zoom / self.zoom;
        let viewport = self.viewport.get();
        let center_x = self.scroll_x + viewport.width / 2.0;
        let center_y = self.scroll_y + viewport.height / 2.0;

        self.zoom = zoom;
        self.relayout();
        self.set_scroll(
            center_x * ratio - viewport.width / 2.0,
            center_y * ratio - viewport.height / 2.0,
        );
        Task::none()
    }

    pub(crate) fn block_len(&self, index: usize) -> usize {
        self.doc.blocks.get(index).map(Block::len).unwrap_or(0)
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
