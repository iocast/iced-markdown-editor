//! Use case: documents that reference existing image files or web images.
//! Nothing is copied, uploaded or embedded: the app inserts links.
//!
//! cargo run --example images_insert --features images

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Subscription, Task};
use iced_markdown_editor::{
    Action, Command, EditorState, FONTS, ImageSettings, Paper, markdown_editor,
};
use std::path::{Path, PathBuf};

pub fn main() -> iced::Result {
    let mut app = iced::application(App::new, App::update, App::view)
        .title("Insert image links")
        .subscription(App::subscription)
        .window_size((1100.0, 860.0));
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}

struct App {
    editor: EditorState,
    folder: PathBuf,
    url: String,
    alt: String,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    PickFile,
    Picked(Option<PathBuf>),
    Url(String),
    Alt(String),
    InsertUrl,
}

/// `path` relative to `base` when it is inside it, with `/` separators.
fn relative(path: &Path, base: &Path) -> String {
    let path = path.strip_prefix(base).unwrap_or(path);
    path.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

impl App {
    fn new() -> Self {
        let folder = std::env::current_dir().unwrap_or_default();
        Self {
            editor: EditorState::with_markdown(
                "# Image links\n\nPut the caret where the image should go, then pick a \
                 file or enter a URL.\n",
            )
            .images(ImageSettings::disabled())
            .image_base(&folder)
            .paper(Paper::a4()),
            folder,
            url: String::new(),
            alt: String::new(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => self.editor.update(action).task.map(Message::Editor),
            Message::PickFile => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                Message::Picked,
            ),
            Message::Picked(Some(path)) => {
                let alt = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let url = relative(&path, &self.folder);
                self.editor
                    .execute(Command::InsertImageLink { url, alt })
                    .task
                    .map(Message::Editor)
            }
            Message::Picked(None) => Task::none(),
            Message::Url(url) => {
                self.url = url;
                Task::none()
            }
            Message::Alt(alt) => {
                self.alt = alt;
                Task::none()
            }
            Message::InsertUrl => {
                let url = std::mem::take(&mut self.url);
                let alt = std::mem::take(&mut self.alt);
                if url.trim().is_empty() {
                    return Task::none();
                }
                self.editor
                    .execute(Command::InsertImageLink {
                        url: url.trim().to_string(),
                        alt,
                    })
                    .task
                    .map(Message::Editor)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let bar = row![
            button(text("Insert file…").size(13)).on_press(Message::PickFile),
            text_input("https://example.com/picture.png", &self.url)
                .on_input(Message::Url)
                .on_submit(Message::InsertUrl)
                .size(13)
                .width(Length::Fixed(320.0)),
            text_input("Description", &self.alt)
                .on_input(Message::Alt)
                .on_submit(Message::InsertUrl)
                .size(13)
                .width(Length::Fixed(180.0)),
            button(text("Insert URL").size(13)).on_press(Message::InsertUrl),
        ]
        .spacing(8)
        .padding(8)
        .align_y(Alignment::Center);

        column![
            container(bar).width(Length::Fill),
            markdown_editor(&self.editor).view().map(Message::Editor),
            container(text(format!("Links relative to {}", self.folder.display())).size(12))
                .padding(6),
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.editor.subscription().map(Message::Editor)
    }
}
