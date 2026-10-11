//! The editor in the browser. Also runs on the desktop with `cargo run`.
//!
//! * full width editor with toolbar and status bar
//! * syntax highlighting for code blocks (`syntect` feature)
//! * light / dark theme, paper / full width
//! * the live Markdown in a side panel (updated on `Event::Changed`)
//! * images embedded as `data:` URLs; the **Image** button opens the
//!   browser's file picker and inserts the file with `Command::InsertImage`

use iced::widget::{button, column, container, row, scrollable, text, Text};
use iced::{Alignment, Element, Font, Length, Subscription, Task, Theme};
use iced_markdown_editor::{
    markdown_editor, Action, Command, EditorState, EditorStyle, Event, ImageSettings, Output,
    Paper, SyntectHighlighter, FONTS,
};

/// The browser has no system fonts: everything uses the bundled DejaVu fonts.
const UI_FONT: Font = Font::with_name("DejaVu Sans");

/// Banner image (`banner.png`), embedded so the demo needs no server.
const BANNER_BASE64: &str = include_str!("../banner.b64");

fn sample() -> String {
    let banner = format!("data:image/png;base64,{}", BANNER_BASE64.trim());
    let lines = [
        "# iced-markdown-editor in the browser",
        "",
        "A Word-style **WYSIWYG** editor for *CommonMark*, written in Rust with",
        "[iced](https://iced.rs). This page runs it as WebAssembly.",
        "",
        "![iced-markdown-editor](BANNER)",
        "",
        "## Try it",
        "",
        "- [x] Type, select, use **Ctrl+B**, *Ctrl+I*, `Ctrl+E`",
        "- [ ] Press **Tab** at the start of an item to nest it",
        "  - [ ] Click a checkbox",
        "- [ ] Switch to **Source** in the toolbar",
        "- [ ] Insert an image with the **Image** button",
        "",
        "> Block quotes, numbered lists and code blocks work too.",
        "",
        "1. First",
        "2. Second",
        "",
        "```rust",
        "use iced_markdown_editor::{Action, EditorState, markdown_editor};",
        "",
        "struct App {",
        "    editor: EditorState,",
        "}",
        "",
        "impl App {",
        "    fn update(&mut self, action: Action) -> Task<Action> {",
        "        self.editor.update(action).task",
        "    }",
        "",
        "    fn view(&self) -> Element<'_, Action> {",
        "        markdown_editor(&self.editor).view()",
        "    }",
        "}",
        "```",
    ];
    lines.join("\n").replace("BANNER", &banner) + "\n"
}

pub fn main() -> iced::Result {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let mut app = iced::application(App::new, App::update, App::view)
        .title("iced-markdown-editor · web demo")
        .subscription(App::subscription)
        .theme(App::theme)
        .default_font(UI_FONT);
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}

struct App {
    editor: EditorState,
    dark: bool,
    paper: bool,
    preview: bool,
    /// Markdown for the side panel, rebuilt on changes only.
    markdown: String,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    ToggleDark,
    TogglePaper,
    TogglePreview,
    Picked(Option<(String, Vec<u8>)>),
}

/// A label in the bundled font.
fn label<'a>(content: impl Into<String>, size: u32) -> Text<'a> {
    let content: String = content.into();
    text(content).size(size).font(UI_FONT)
}

/// Shortens long lines (embedded images) for the side panel.
fn shorten(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let count = line.chars().count();
            if count > 100 {
                let short: String = line.chars().take(100).collect();
                format!("{short}… ({count} characters)")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl App {
    fn new() -> Self {
        let editor = EditorState::with_markdown(&sample())
            .images(ImageSettings::embed().max_bytes(2 * 1024 * 1024))
            .highlighter(SyntectHighlighter::new())
            .paper(Paper::fill())
            .tab_width(2);
        let markdown = shorten(&editor.markdown());
        Self {
            editor,
            dark: false,
            paper: false,
            preview: true,
            markdown,
            status: String::new(),
        }
    }

    fn theme(&self) -> Theme {
        if self.dark {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    fn handle(&mut self, output: Output) -> Task<Message> {
        let mut tasks = vec![output.task.map(Message::Editor)];
        for event in output.events {
            match event {
                Event::Changed => {
                    self.markdown = shorten(&self.editor.markdown());
                    self.status.clear();
                }
                Event::ImageRequested => tasks.push(Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                            .pick_file()
                            .await?;
                        let bytes = file.read().await;
                        Some((file.file_name(), bytes))
                    },
                    Message::Picked,
                )),
                Event::ImageRejected { reason, .. } => {
                    self.status = format!("Image not inserted: {reason}");
                }
                _ => {}
            }
        }
        Task::batch(tasks)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                let output = self.editor.update(action);
                self.handle(output)
            }
            Message::Picked(Some((name, bytes))) => {
                let output = self.editor.execute(Command::InsertImage {
                    bytes,
                    name: Some(name),
                });
                self.handle(output)
            }
            Message::Picked(None) => Task::none(),
            Message::ToggleDark => {
                self.dark = !self.dark;
                self.editor.set_style(if self.dark {
                    EditorStyle::dark()
                } else {
                    EditorStyle::light()
                });
                Task::none()
            }
            Message::TogglePaper => {
                self.paper = !self.paper;
                self.editor.set_paper(if self.paper {
                    Paper::a4()
                } else {
                    Paper::fill()
                });
                Task::none()
            }
            Message::TogglePreview => {
                self.preview = !self.preview;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let toggle = |name: &str, on: bool, message: Message| {
            button(label(name, 13))
                .padding([4, 10])
                .style(if on {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(message)
        };

        let bar = row![
            label("iced-markdown-editor", 15),
            label(self.status.clone(), 12).width(Length::Fill),
            toggle("Dark", self.dark, Message::ToggleDark),
            toggle("A4 paper", self.paper, Message::TogglePaper),
            toggle("Markdown", self.preview, Message::TogglePreview),
        ]
        .spacing(8)
        .padding([6, 10])
        .align_y(Alignment::Center);

        let editor = markdown_editor(&self.editor).view().map(Message::Editor);

        let body: Element<'_, Message> = if self.preview {
            let side = container(scrollable(label(self.markdown.clone(), 12)))
                .padding(12)
                .width(Length::FillPortion(2))
                .height(Length::Fill);
            row![container(editor).width(Length::FillPortion(3)), side].into()
        } else {
            editor
        };

        column![bar, body].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.editor.subscription().map(Message::Editor)
    }
}
