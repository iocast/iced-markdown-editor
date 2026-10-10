//! Use case: single-file documents. Images are written into the Markdown as
//! `data:` URLs, so the `.md` file contains everything. Limited to 512 KiB.
//!
//! * `ImageSettings::embed()` – no application code needed for storage.
//! * `Event::ImageRequested` – the toolbar "Image" button: show a file dialog
//!   and send `Command::InsertImageFile`.
//! * `Event::ImageRejected` – tell the user why an image was not inserted.
//! * The preview is rebuilt on `Event::Changed`, not in `view`.
//!
//! cargo run --example images_embed --features images

use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length, Subscription, Task};
use iced_markdown_editor::{
    markdown_editor, Action, Command, EditorState, Event, ImageSettings, Output, Paper, FONTS,
};
use std::path::PathBuf;

pub fn main() -> iced::Result {
    let mut app = iced::application(App::new, App::update, App::view)
        .title("Embedded images")
        .subscription(App::subscription)
        .window_size((1300.0, 820.0));
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}

struct App {
    editor: EditorState,
    status: String,
    /// Shortened Markdown for the side panel, rebuilt only on changes.
    preview: String,
    size: usize,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    Picked(Option<PathBuf>),
}

impl App {
    fn new() -> Self {
        let editor = EditorState::with_markdown(
            "# Embedded images\n\nPaste, drop or insert an image (up to 512 KiB). It is \
             stored inside the Markdown as a `data:` URL.\n",
        )
        .images(
            ImageSettings::embed()
                .max_bytes(512 * 1024)
                .accept(["image/png", "image/jpeg", "image/gif"]),
        )
        .paper(Paper::fill());
        let mut app = Self {
            editor,
            status: String::new(),
            preview: String::new(),
            size: 0,
        };
        app.refresh();
        app
    }

    /// Builds the preview once per change.
    fn refresh(&mut self) {
        let markdown = self.editor.markdown();
        self.size = markdown.len();
        self.preview = markdown
            .lines()
            .map(|line| {
                let count = line.chars().count();
                if count > 120 {
                    let short: String = line.chars().take(120).collect();
                    format!("{short}… ({count} characters)")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    /// Reacts to the events of every editor output.
    fn handle(&mut self, output: Output) -> Task<Message> {
        let mut tasks = vec![output.task.map(Message::Editor)];
        for event in output.events {
            match event {
                Event::ImageRejected { source, reason } => {
                    self.status = format!("{source:?}: {reason}");
                }
                Event::ImageRequested => tasks.push(Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif"])
                            .pick_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    Message::Picked,
                )),
                Event::Changed => {
                    self.status.clear();
                    self.refresh();
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
            Message::Picked(Some(path)) => {
                let output = self.editor.execute(Command::InsertImageFile(path));
                self.handle(output)
            }
            Message::Picked(None) => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let side = container(scrollable(
            column![
                text(format!("Markdown: {} bytes", self.size)).size(12),
                text(&self.status).size(12),
                text(&self.preview).size(12),
            ]
            .spacing(8),
        ))
        .padding(12)
        .width(Length::Fixed(420.0))
        .height(Length::Fill);

        row![markdown_editor(&self.editor).view().map(Message::Editor), side].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.editor.subscription().map(Message::Editor)
    }
}
