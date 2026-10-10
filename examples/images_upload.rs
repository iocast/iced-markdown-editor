//! Use case: a web-backed or team app. Images are uploaded to a server and
//! the Markdown links the returned URL. The server is simulated: an upload
//! waits 1.5 s, every third upload fails.
//!
//! cargo run --example images_upload --features images

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Subscription, Task};
use iced_markdown_editor::{
    markdown_editor, Action, Command, EditorState, Event, ImageId, ImageSettings, Output,
    PastedImage, FONTS,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SERVER: &str = "https://uploads.example.com/";

pub fn main() -> iced::Result {
    let mut app = iced::application(App::new, App::update, App::view)
        .title("Upload images")
        .subscription(App::subscription)
        .window_size((1100.0, 860.0));
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}

/// Simulated image server: URL → bytes.
type Server = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct App {
    editor: EditorState,
    server: Server,
    uploads: u32,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    Uploaded(ImageId, Result<String, String>),
    Picked(Option<PathBuf>),
    Publish,
}

async fn upload(server: Server, image: PastedImage, attempt: u32) -> Result<String, String> {
    tokio::time::sleep(Duration::from_millis(1500)).await;
    if attempt % 3 == 0 {
        return Err("server busy (simulated)".into());
    }
    let url = format!("{SERVER}{}", image.file_name());
    server
        .lock()
        .map_err(|_| "server error".to_string())?
        .insert(url.clone(), image.bytes.to_vec());
    Ok(url)
}

impl App {
    fn new() -> Self {
        let server: Server = Arc::default();
        let store = server.clone();
        let editor = EditorState::with_markdown(
            "# Uploads\n\nPaste, drop or insert images. They are uploaded (1.5 s, every \
             third upload fails) and linked by URL.\n",
        )
        .images(ImageSettings::delegate().max_bytes(5 * 1024 * 1024))
        .image_loader(move |url| store.lock().ok()?.get(url).cloned());

        Self {
            editor,
            server,
            uploads: 0,
            status: String::new(),
        }
    }

    fn handle(&mut self, output: Output) -> Task<Message> {
        let mut tasks = vec![output.task.map(Message::Editor)];
        for event in output.events {
            match event {
                Event::ImagePasted(image) => {
                    self.uploads += 1;
                    let id = image.id;
                    tasks.push(Task::perform(
                        upload(self.server.clone(), image, self.uploads),
                        move |result| Message::Uploaded(id, result),
                    ));
                }
                Event::ImageRejected { reason, .. } => {
                    self.status = format!("Not uploaded: {reason}");
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
            Message::Uploaded(id, Ok(url)) => {
                self.status = format!("Uploaded {url}");
                let output = self.editor.execute(Command::ResolveImage {
                    id,
                    url,
                    alt: String::new(),
                });
                self.handle(output)
            }
            Message::Uploaded(id, Err(error)) => {
                self.status = format!("Upload failed: {error}. The placeholder was removed.");
                let output = self.editor.execute(Command::CancelImage { id });
                self.handle(output)
            }
            Message::Picked(Some(path)) => {
                let output = self.editor.execute(Command::InsertImageFile(path));
                self.handle(output)
            }
            Message::Picked(None) => Task::none(),
            Message::Publish => {
                self.status = format!(
                    "Published {} bytes of Markdown",
                    self.editor.markdown().len()
                );
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let pending = self.editor.pending_images();
        let publish = button(text("Publish").size(13))
            .on_press_maybe((pending == 0).then_some(Message::Publish));

        let bar = row![
            publish,
            text(if pending > 0 {
                format!("Uploading {pending} image(s)…")
            } else {
                String::new()
            })
            .size(12),
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
