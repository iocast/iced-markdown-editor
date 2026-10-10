//! Use case: a desktop notes app. Images are saved as files in an `assets/`
//! folder next to the document; the Markdown links them relatively.
//!
//! cargo run --example images_folder --features images

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Subscription, Task};
use iced_markdown_editor::{
    Action, Command, EditorState, Event, FONTS, ImageId, ImageSettings, Output, Paper, PastedImage,
    markdown_editor,
};
use std::path::PathBuf;

pub fn main() -> iced::Result {
    let mut app = iced::application(App::new, App::update, App::view)
        .title("Notes with image folder")
        .subscription(App::subscription)
        .window_size((1100.0, 860.0));
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}

struct App {
    editor: EditorState,
    /// Folder of the note; images go to `<folder>/assets`.
    folder: PathBuf,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    Stored(ImageId, Result<String, String>),
    Picked(Option<PathBuf>),
    Save,
}

/// Saves the image and returns the URL relative to the note.
async fn store(folder: PathBuf, image: PastedImage) -> Result<String, String> {
    let original = image.file_name();
    let (stem, extension) = original
        .rsplit_once('.')
        .unwrap_or((original.as_str(), image.extension()));
    let mut name = original.clone();
    let mut counter = 1;
    while folder.join("assets").join(&name).exists() {
        name = format!("{stem}-{counter}.{extension}");
        counter += 1;
    }
    std::fs::write(folder.join("assets").join(&name), &*image.bytes)
        .map_err(|error| error.to_string())?;
    Ok(format!("assets/{name}"))
}

impl App {
    fn new() -> Self {
        let folder = std::env::current_dir()
            .unwrap_or_default()
            .join("example-notes");
        let _ = std::fs::create_dir_all(folder.join("assets"));
        let note = std::fs::read_to_string(folder.join("note.md")).unwrap_or_else(|_| {
            "# My note\n\nPaste a screenshot (Ctrl+V), drop an image file onto the window, \
             or use the **Image** button. Images are saved in `example-notes/assets/`.\n"
                .to_string()
        });

        let editor = EditorState::with_markdown(&note)
            .images(ImageSettings::delegate().max_bytes(20 * 1024 * 1024))
            .image_base(&folder)
            .paper(Paper::a4());

        Self {
            editor,
            status: format!("Folder: {}", folder.display()),
            folder,
        }
    }

    fn handle(&mut self, output: Output) -> Task<Message> {
        let mut tasks = vec![output.task.map(Message::Editor)];
        for event in output.events {
            match event {
                Event::ImagePasted(image) => {
                    self.status = format!("Saving {}…", image.file_name());
                    let id = image.id;
                    tasks.push(Task::perform(
                        store(self.folder.clone(), image),
                        move |result| Message::Stored(id, result),
                    ));
                }
                Event::ImageRejected { reason, .. } => {
                    self.status = format!("Not inserted: {reason}");
                }
                Event::ImageRequested => tasks.push(Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                            .pick_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    Message::Picked,
                )),
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
            Message::Stored(id, Ok(url)) => {
                self.status = format!("Saved {url}");
                let output = self.editor.execute(Command::ResolveImage {
                    id,
                    url,
                    alt: String::new(),
                });
                self.handle(output)
            }
            Message::Stored(id, Err(error)) => {
                self.status = format!("Could not save the image: {error}");
                let output = self.editor.execute(Command::CancelImage { id });
                self.handle(output)
            }
            Message::Picked(Some(path)) => {
                let output = self.editor.execute(Command::InsertImageFile(path));
                self.handle(output)
            }
            Message::Picked(None) => Task::none(),
            Message::Save => {
                let path = self.folder.join("note.md");
                match std::fs::write(&path, self.editor.markdown()) {
                    Ok(()) => {
                        self.editor.mark_saved();
                        self.status = format!("Saved {}", path.display());
                    }
                    Err(error) => self.status = format!("Save failed: {error}"),
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let bar = row![
            button(text("Save note.md").size(13)).on_press(Message::Save),
            text(&self.status).size(12),
        ]
        .spacing(10)
        .padding(8)
        .align_y(Alignment::Center);

        column![
            container(bar).width(Length::Fill),
            markdown_editor(&self.editor).view().map(Message::Editor),
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.editor.subscription().map(Message::Editor)
    }
}
