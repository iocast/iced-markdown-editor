//! Full demo of the `markdown_editor` widget (desktop).
//!
//! Bar 1: file handling, theme, Tab width, syntax highlighting,
//!        fullscreen (F11), event log.
//! Bar 2: full width (no paper), paper size, orientation, margins, zoom.
//! Images are saved in `assets/` next to the saved document.
//!
//! cargo run --example editor --features syntect,images

use iced::event::{self, Status};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{button, column, container, pick_list, row, text};
use iced::{Alignment, Element, Length, Subscription, Task, Theme, window};
use iced_markdown_editor::{
    Action, CodeHighlighter, Command, EditorState, EditorStyle, Event, FONTS, ImageId,
    ImageSettings, KeyBinding, Margins, Measure, Orientation, Output, PageSize, Paper, PastedImage,
    SyntectHighlighter, ZoomSettings, ZoomShortcuts, markdown_editor,
};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SAMPLE: &str = r#"# iced-markdown-editor

A Word-style **WYSIWYG** editor for *CommonMark*, written in Rust with
[iced](https://iced.rs "iced website").

## Task list

- [x] Write the request
- [ ] Post it
  - [ ] wait for the mods
  - [ ] answer questions
- [ ] Share the first release

Click a box, or put the caret in an item and press **Ctrl+Enter**.

## Images

Save this document, then paste a screenshot (Ctrl+V), drop an image file onto
the window, or use the **Image** button. Images are stored in `assets/`.

## Structure

1. Nested lists
   - press **Tab** at the start of an item to nest it
   - **Shift+Tab** moves it out again
2. Items with several paragraphs

   This paragraph belongs to the second item.

> Block quotes can contain lists:
>
> - one
> - two

## Tables

| Key       | In a table                    |  Mode |
| :-------- | :---------------------------: | ----: |
| **Tab**   | next cell, a new row at the end | both |
| **Enter** | cell below, leave on an empty row | WYSIWYG |
| `\|`      | a pipe inside a cell          |   any |

Type `| a | b |` and press **Enter** to start a table.

## Code

```rust
fn parse(line: &str) -> Option<Task> {
    let done = line.starts_with("x ");
    Some(Task { done, text: line.to_string() })
}
```
"#;

pub fn main() -> iced::Result {
    let mut app = iced::application(Demo::new, Demo::update, Demo::view)
        .title(Demo::title)
        .subscription(Demo::subscription)
        .theme(Demo::theme)
        .window_size((1360.0, 940.0));
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}

/// An empty element that takes all remaining width.
fn fill<'a>() -> Element<'a, Message> {
    text("").width(Length::Fill).into()
}

/// A toggle button.
fn toggle<'a>(label: &str, on: bool, message: Message) -> Element<'a, Message> {
    let label = if on {
        format!("\u{2611} {label}")
    } else {
        format!("\u{2610} {label}")
    };
    button(text(label).size(13))
        .padding([4, 10])
        .style(if on {
            button::primary
        } else {
            button::secondary
        })
        .on_press(message)
        .into()
}

// ------------------------------------------------------------------------
// Choices
// ------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum ThemeChoice {
    WordLight,
    WordDark,
    Iced(Theme),
}

impl ThemeChoice {
    fn options() -> Vec<ThemeChoice> {
        let mut options = vec![ThemeChoice::WordLight, ThemeChoice::WordDark];
        options.extend(Theme::ALL.iter().cloned().map(ThemeChoice::Iced));
        options
    }

    fn app_theme(&self) -> Theme {
        match self {
            ThemeChoice::WordLight => Theme::Light,
            ThemeChoice::WordDark => Theme::Dark,
            ThemeChoice::Iced(theme) => theme.clone(),
        }
    }

    fn editor_style(&self) -> EditorStyle {
        match self {
            ThemeChoice::WordLight => EditorStyle::light(),
            ThemeChoice::WordDark => EditorStyle::dark(),
            ThemeChoice::Iced(theme) => EditorStyle::from_theme(theme),
        }
    }
}

impl fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeChoice::WordLight => f.write_str("Word light"),
            ThemeChoice::WordDark => f.write_str("Word dark"),
            ThemeChoice::Iced(theme) => write!(f, "Iced: {theme}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TabWidth(usize);

const TAB_WIDTHS: [TabWidth; 4] = [TabWidth(0), TabWidth(2), TabWidth(4), TabWidth(8)];

impl fmt::Display for TabWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            0 => f.write_str("Tab off"),
            n => write!(f, "Tab = {n} spaces"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PaperChoice {
    A3,
    A4,
    A5,
    B5,
    Letter,
    Legal,
    Custom,
}

impl PaperChoice {
    const ALL: [PaperChoice; 7] = [
        PaperChoice::A3,
        PaperChoice::A4,
        PaperChoice::A5,
        PaperChoice::B5,
        PaperChoice::Letter,
        PaperChoice::Legal,
        PaperChoice::Custom,
    ];

    fn size(self) -> PageSize {
        match self {
            PaperChoice::A3 => PageSize::A3,
            PaperChoice::A4 => PageSize::A4,
            PaperChoice::A5 => PageSize::A5,
            PaperChoice::B5 => PageSize::B5,
            PaperChoice::Letter => PageSize::Letter,
            PaperChoice::Legal => PageSize::Legal,
            PaperChoice::Custom => PageSize::custom(Measure::cm(15.0), Measure::cm(21.0)),
        }
    }
}

impl fmt::Display for PaperChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PaperChoice::A3 => "A3 (297 x 420 mm)",
            PaperChoice::A4 => "A4 (210 x 297 mm)",
            PaperChoice::A5 => "A5 (148 x 210 mm)",
            PaperChoice::B5 => "B5 (176 x 250 mm)",
            PaperChoice::Letter => "Letter (8.5 x 11 in)",
            PaperChoice::Legal => "Legal (8.5 x 14 in)",
            PaperChoice::Custom => "Custom (15 x 21 cm)",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MarginChoice {
    Normal,
    Narrow,
    Moderate,
    Wide,
    Minimal,
}

impl MarginChoice {
    const ALL: [MarginChoice; 5] = [
        MarginChoice::Normal,
        MarginChoice::Narrow,
        MarginChoice::Moderate,
        MarginChoice::Wide,
        MarginChoice::Minimal,
    ];

    fn margins(self) -> Margins {
        match self {
            MarginChoice::Normal => Margins::all(Measure::cm(2.5)),
            MarginChoice::Narrow => Margins::all(Measure::cm(1.27)),
            MarginChoice::Moderate => Margins::symmetric(Measure::cm(2.54), Measure::cm(1.91)),
            MarginChoice::Wide => Margins::symmetric(Measure::cm(2.54), Measure::cm(5.08)),
            MarginChoice::Minimal => Margins::all(Measure::mm(5.0)),
        }
    }
}

impl fmt::Display for MarginChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MarginChoice::Normal => "Normal margins",
            MarginChoice::Narrow => "Narrow margins",
            MarginChoice::Moderate => "Moderate margins",
            MarginChoice::Wide => "Wide margins",
            MarginChoice::Minimal => "Minimal margins",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZoomLevel(u32);

const ZOOM_LEVELS: [ZoomLevel; 11] = [
    ZoomLevel(25),
    ZoomLevel(50),
    ZoomLevel(75),
    ZoomLevel(90),
    ZoomLevel(100),
    ZoomLevel(110),
    ZoomLevel(125),
    ZoomLevel(150),
    ZoomLevel(200),
    ZoomLevel(300),
    ZoomLevel(400),
];

impl fmt::Display for ZoomLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} %", self.0)
    }
}

fn zoom_settings(enabled: bool) -> ZoomSettings {
    ZoomSettings {
        enabled,
        shortcuts: ZoomShortcuts {
            zoom_in: vec![KeyBinding::command("+"), KeyBinding::command("=")],
            zoom_out: vec![KeyBinding::command("-")],
            reset: vec![KeyBinding::command("0")],
        },
        ..ZoomSettings::default()
    }
}

// ------------------------------------------------------------------------
// Application
// ------------------------------------------------------------------------

struct Demo {
    editor: EditorState,
    highlighter: Arc<dyn CodeHighlighter>,
    highlighting: bool,
    tab_width: TabWidth,
    path: Option<PathBuf>,
    theme: ThemeChoice,
    paper: PaperChoice,
    orientation: Orientation,
    margins: MarginChoice,
    full_width: bool,
    fullscreen: bool,
    zoom_enabled: bool,
    log: Vec<String>,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    New,
    Open,
    Opened(Option<PathBuf>),
    Loaded(Result<(PathBuf, String), String>),
    Save,
    SaveAs,
    Saved(Result<PathBuf, String>),
    PickImage,
    ImagePicked(Option<PathBuf>),
    ImageStored(ImageId, Result<String, String>),
    Theme(ThemeChoice),
    TabWidth(TabWidth),
    Highlighting(bool),
    Paper(PaperChoice),
    ToggleOrientation,
    Margins(MarginChoice),
    FullWidth(bool),
    ToggleFullscreen,
    ZoomEnabled(bool),
}

fn run(command: Command) -> Message {
    Message::Editor(Action::from(command))
}

/// Writes an image into `<folder>/assets/` and returns the relative URL.
async fn store_image(folder: PathBuf, image: PastedImage) -> Result<String, String> {
    let assets = folder.join("assets");
    std::fs::create_dir_all(&assets).map_err(|error| error.to_string())?;
    let name = image.file_name();
    std::fs::write(assets.join(&name), &*image.bytes).map_err(|error| error.to_string())?;
    Ok(format!("assets/{name}"))
}

/// Application shortcuts; keys the editor captured are skipped.
fn shortcut(event: iced::Event, status: Status, _window: window::Id) -> Option<Message> {
    if status == Status::Captured {
        return None;
    }
    let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    if let Key::Named(Named::F11) = key.as_ref() {
        return Some(Message::ToggleFullscreen);
    }
    if !modifiers.command() {
        return None;
    }
    let Key::Character(c) = key.as_ref() else {
        return None;
    };
    match (c.to_lowercase().as_str(), modifiers.shift()) {
        ("s", true) => Some(Message::SaveAs),
        ("s", false) => Some(Message::Save),
        ("o", _) => Some(Message::Open),
        ("n", false) => Some(Message::New),
        _ => None,
    }
}

impl Demo {
    fn new() -> Self {
        let highlighter: Arc<dyn CodeHighlighter> = Arc::new(SyntectHighlighter::new());

        let mut demo = Self {
            editor: EditorState::with_markdown(SAMPLE)
                .zoom_settings(zoom_settings(true))
                .images(ImageSettings::delegate())
                .tab_width(4),
            highlighter,
            highlighting: true,
            tab_width: TabWidth(4),
            path: None,
            theme: ThemeChoice::WordLight,
            paper: PaperChoice::A4,
            orientation: Orientation::Portrait,
            margins: MarginChoice::Normal,
            full_width: false,
            fullscreen: false,
            zoom_enabled: true,
            log: Vec::new(),
        };
        demo.editor.set_highlighter(Some(demo.highlighter.clone()));
        demo.editor.set_style(demo.theme.editor_style());
        demo.apply_paper();
        demo
    }

    fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled.md");
        let dirty = if self.editor.is_dirty() {
            "\u{25CF} "
        } else {
            ""
        };
        format!(
            "{dirty}{name} - iced-markdown-editor {}",
            env!("CARGO_PKG_VERSION")
        )
    }

    fn theme(&self) -> Theme {
        self.theme.app_theme()
    }

    fn log(&mut self, entry: String) {
        self.log.push(entry);
        if self.log.len() > 3 {
            self.log.remove(0);
        }
    }

    fn folder(&self) -> Option<PathBuf> {
        self.path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    }

    fn apply_paper(&mut self) {
        let paper = if self.full_width {
            Paper::fill()
        } else {
            Paper::new(self.paper.size())
                .with_orientation(self.orientation)
                .with_margins(self.margins.margins())
        };
        self.editor.set_paper(paper);
    }

    /// Reacts to editor events, also for commands run by the demo itself.
    fn handle(&mut self, output: Output) -> Task<Message> {
        let mut tasks = vec![output.task.map(Message::Editor)];
        for event in output.events {
            match event {
                Event::SelectionChanged | Event::Changed => {}
                Event::ZoomChanged(zoom) => self.log(format!("Zoom {:.0} %", zoom * 100.0)),
                Event::LinkClicked(url) => self.log(format!("Link clicked: {url}")),
                Event::ImagePasted(image) => match self.folder() {
                    Some(folder) => {
                        let id = image.id;
                        tasks.push(Task::perform(store_image(folder, image), move |result| {
                            Message::ImageStored(id, result)
                        }));
                    }
                    None => {
                        self.log("Save the document first, then paste images".into());
                        let cancel = self.editor.execute(Command::CancelImage { id: image.id });
                        tasks.push(cancel.task.map(Message::Editor));
                    }
                },
                Event::ImageRejected { reason, .. } => {
                    self.log(format!("Image not inserted: {reason}"))
                }
                Event::ImageRequested => tasks.push(Task::done(Message::PickImage)),
                other => self.log(format!("{other:?}")),
            }
        }
        Task::batch(tasks)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                let output = self.editor.update(action);
                return self.handle(output);
            }
            Message::PickImage => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                            .pick_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    Message::ImagePicked,
                );
            }
            Message::ImagePicked(Some(path)) => {
                let output = self.editor.execute(Command::InsertImageFile(path));
                return self.handle(output);
            }
            Message::ImagePicked(None) => {}
            Message::ImageStored(id, Ok(url)) => {
                self.log(format!("Image saved: {url}"));
                let output = self.editor.execute(Command::ResolveImage {
                    id,
                    url,
                    alt: String::new(),
                });
                return self.handle(output);
            }
            Message::ImageStored(id, Err(error)) => {
                self.log(format!("Image not saved: {error}"));
                let output = self.editor.execute(Command::CancelImage { id });
                return self.handle(output);
            }
            Message::New => {
                self.editor.set_markdown("");
                self.editor.set_image_base(None);
                self.path = None;
                return self
                    .editor
                    .execute(Command::Focus)
                    .task
                    .map(Message::Editor);
            }
            Message::Open => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Markdown", &["md", "markdown", "txt"])
                            .pick_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    Message::Opened,
                );
            }
            Message::Opened(Some(path)) => {
                return Task::perform(
                    async move {
                        std::fs::read_to_string(&path)
                            .map(|text| (path, text))
                            .map_err(|error| error.to_string())
                    },
                    Message::Loaded,
                );
            }
            Message::Opened(None) => {}
            Message::Loaded(Ok((path, markdown))) => {
                self.editor.set_markdown(&markdown);
                self.log(format!("Opened {}", path.display()));
                self.path = Some(path);
                self.editor.set_image_base(self.folder());
            }
            Message::Loaded(Err(error)) => self.log(format!("Open failed: {error}")),
            Message::Save => return self.save(false),
            Message::SaveAs => return self.save(true),
            Message::Saved(Ok(path)) => {
                self.editor.mark_saved();
                self.log(format!("Saved {}", path.display()));
                self.path = Some(path);
                self.editor.set_image_base(self.folder());
            }
            Message::Saved(Err(error)) => self.log(error),
            Message::Theme(choice) => {
                self.editor.set_style(choice.editor_style());
                self.log(format!("Theme: {choice}"));
                self.theme = choice;
            }
            Message::TabWidth(width) => {
                self.tab_width = width;
                self.editor.set_tab_width(width.0);
                self.log(width.to_string());
            }
            Message::Highlighting(on) => {
                self.highlighting = on;
                self.editor
                    .set_highlighter(on.then(|| self.highlighter.clone()));
            }
            Message::Paper(choice) => {
                self.paper = choice;
                self.full_width = false;
                self.apply_paper();
            }
            Message::ToggleOrientation => {
                self.orientation = match self.orientation {
                    Orientation::Portrait => Orientation::Landscape,
                    Orientation::Landscape => Orientation::Portrait,
                };
                self.full_width = false;
                self.apply_paper();
            }
            Message::Margins(choice) => {
                self.margins = choice;
                self.apply_paper();
            }
            Message::FullWidth(on) => {
                self.full_width = on;
                self.apply_paper();
            }
            Message::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                let mode = if self.fullscreen {
                    window::Mode::Fullscreen
                } else {
                    window::Mode::Windowed
                };
                return window::latest().and_then(move |id| window::set_mode(id, mode));
            }
            Message::ZoomEnabled(enabled) => {
                self.zoom_enabled = enabled;
                self.editor.set_zoom_settings(zoom_settings(enabled));
            }
        }
        Task::none()
    }

    fn save(&mut self, save_as: bool) -> Task<Message> {
        let markdown = self.editor.markdown();
        let current = if save_as { None } else { self.path.clone() };

        Task::perform(
            async move {
                let path = match current {
                    Some(path) => Some(path),
                    None => rfd::AsyncFileDialog::new()
                        .add_filter("Markdown", &["md"])
                        .set_file_name("document.md")
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf()),
                };
                match path {
                    Some(path) => std::fs::write(&path, markdown)
                        .map(|_| path)
                        .map_err(|error| error.to_string()),
                    None => Err("Save cancelled".to_string()),
                }
            },
            Message::Saved,
        )
    }

    fn view(&self) -> Element<'_, Message> {
        let action = |label: &str, message: Option<Message>| {
            button(text(label.to_string()).size(13))
                .padding([4, 10])
                .style(button::secondary)
                .on_press_maybe(message)
        };
        let separator = || text("|").size(16);

        let file_bar = row![
            action("New", Some(Message::New)),
            action("Open", Some(Message::Open)),
            action("Save", Some(Message::Save)),
            action("Save as", Some(Message::SaveAs)),
            separator(),
            pick_list(
                ThemeChoice::options(),
                Some(self.theme.clone()),
                Message::Theme
            )
            .text_size(13)
            .padding([4, 8])
            .width(Length::Fixed(200.0)),
            pick_list(&TAB_WIDTHS[..], Some(self.tab_width), Message::TabWidth)
                .text_size(13)
                .padding([4, 8]),
            toggle(
                "Syntax highlighting",
                self.highlighting,
                Message::Highlighting(!self.highlighting)
            ),
            separator(),
            toggle(
                "Fullscreen (F11)",
                self.fullscreen,
                Message::ToggleFullscreen
            ),
            fill(),
            text(self.log.join("   \u{00B7}   ")).size(12),
        ]
        .spacing(8)
        .padding([6, 10])
        .align_y(Alignment::Center);

        let zoom_on = self.zoom_enabled;
        let zoom_cmd = |command: Command| zoom_on.then(|| run(command));
        let percent = ZoomLevel((self.editor.current_zoom() * 100.0).round() as u32);
        let paged = !self.full_width;
        let orientation = match self.orientation {
            Orientation::Portrait => "Portrait",
            Orientation::Landscape => "Landscape",
        };

        let mut view_bar = row![toggle(
            "Full width (no paper)",
            self.full_width,
            Message::FullWidth(!self.full_width)
        )]
        .spacing(8)
        .padding([6, 10])
        .align_y(Alignment::Center);

        if paged {
            view_bar = view_bar
                .push(
                    pick_list(&PaperChoice::ALL[..], Some(self.paper), Message::Paper)
                        .text_size(13)
                        .padding([4, 8]),
                )
                .push(action(orientation, Some(Message::ToggleOrientation)))
                .push(
                    pick_list(&MarginChoice::ALL[..], Some(self.margins), Message::Margins)
                        .text_size(13)
                        .padding([4, 8]),
                );
        }

        view_bar = view_bar
            .push(separator())
            .push(toggle(
                "Zoom",
                self.zoom_enabled,
                Message::ZoomEnabled(!self.zoom_enabled),
            ))
            .push(action("\u{2212}", zoom_cmd(Command::ZoomOut)))
            .push(
                pick_list(&ZOOM_LEVELS[..], Some(percent), move |level: ZoomLevel| {
                    run(Command::SetZoom(level.0 as f32 / 100.0))
                })
                .text_size(13)
                .padding([4, 8]),
            )
            .push(action("+", zoom_cmd(Command::ZoomIn)))
            .push(action("100 %", zoom_cmd(Command::ResetZoom)));

        if paged {
            view_bar = view_bar
                .push(action("Fit width", zoom_cmd(Command::FitWidth)))
                .push(action("Fit page", zoom_cmd(Command::FitPage)));
        }

        view_bar = view_bar.push(fill()).push(text(self.summary()).size(12));

        column![
            container(file_bar).width(Length::Fill),
            container(view_bar).width(Length::Fill),
            markdown_editor(&self.editor).view().map(Message::Editor),
        ]
        .into()
    }

    fn summary(&self) -> String {
        let paper = self.editor.current_paper();
        let zoom = self.editor.current_zoom() * 100.0;
        if paper.is_fill() {
            return format!("Full width (no paper) \u{00B7} {zoom:.0} %");
        }
        format!(
            "{} {:?} \u{00B7} page {} of {} \u{00B7} {zoom:.0} %",
            paper.size,
            paper.orientation,
            self.editor.current_page(),
            self.editor.page_count(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.editor.subscription().map(Message::Editor),
            event::listen_with(shortcut),
        ])
    }
}
