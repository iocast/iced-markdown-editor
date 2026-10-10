//! Minimal integration: the editor on the left, its live Markdown on the
//! right. Full width (no paper), zoom disabled, Tab = 2 spaces.
//!
//! cargo run --example minimal

use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length, Subscription, Task};
use iced_markdown_editor::{
    Action, EditorState, Event, FONTS, Paper, ZoomSettings, markdown_editor,
};

pub fn main() -> iced::Result {
    let mut app = iced::application(App::default, App::update, App::view)
        .title("Minimal editor")
        .subscription(App::subscription)
        .window_size((1300.0, 820.0));
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}

struct App {
    editor: EditorState,
    /// Updated on `Event::Changed`, not in `view`.
    markdown: String,
}

impl Default for App {
    fn default() -> Self {
        let editor = EditorState::with_markdown(
            "# Hello\n\nType here. Try **bold** with Ctrl+B.\n\n- [ ] click the box\n- [x] done\n  - nested\n",
        )
        .paper(Paper::fill())
        .zoom_settings(ZoomSettings::disabled())
        .tab_width(2);
        let markdown = editor.markdown();
        Self { editor, markdown }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                let output = self.editor.update(action);
                if output.events.contains(&Event::Changed) {
                    self.markdown = self.editor.markdown();
                }
                output.task.map(Message::Editor)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let preview = container(scrollable(
            column![text("Markdown").size(12), text(&self.markdown).size(13)].spacing(8),
        ))
        .padding(16)
        .width(Length::Fixed(420.0))
        .height(Length::Fill);

        row![
            markdown_editor(&self.editor)
                .status_bar(false)
                .view()
                .map(Message::Editor),
            preview,
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.editor.subscription().map(Message::Editor)
    }
}
