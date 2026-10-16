# iced-markdown-editor

A Word-style WYSIWYG **CommonMark** editor widget for [iced](https://iced.rs) 0.14,
with the GFM extensions strikethrough, task lists and tables. Desktop and web.

- Pages like a word processor (A4, Letter, custom sizes, margins) or full width
- Formatting toolbar, keyboard shortcuts, Word-style autoformat
- Nested lists, task lists with clickable checkboxes, block quotes, code blocks
- Tables with column alignment; Tab and Enter move between cells
- WYSIWYG and Markdown source mode on the same pages
- Zoom, undo / redo, clipboard
- Toolbar and status bar in English, German, French, Spanish and Italian (Fluent)
- Optional syntax highlighting (`syntect`) and images (`images`)
- Round trip tested against all 652 examples of the CommonMark 0.31.2 spec

```toml
[dependencies]
iced = "0.14"
iced-markdown-editor = { version = "0.3", features = ["syntect", "images"] }
```

```rust
use iced_markdown_editor::{markdown_editor, Action, EditorState, Event, FONTS};

struct App {
    editor: EditorState,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
}

impl App {
    fn new() -> Self {
        Self { editor: EditorState::with_markdown("# Hello\n\n- [ ] first task\n") }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Editor(action) => {
                let output = self.editor.update(action);
                if output.events.contains(&Event::Changed) {
                    let _markdown = self.editor.markdown();
                }
                output.task.map(Message::Editor)
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        markdown_editor(&self.editor).view().map(Message::Editor)
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        self.editor.subscription().map(Message::Editor)
    }
}

pub fn main() -> iced::Result {
    let mut app = iced::application(App::new, App::update, App::view)
        .subscription(App::subscription);
    for font in FONTS {
        app = app.font(font);
    }
    app.run()
}
```

## Examples

```bash
cargo run --example editor --features syntect,images   # full demo
cargo run --example minimal
cargo run --example images_folder --features images
cargo run --example images_embed --features images
cargo run --example images_upload --features images
cargo run --example images_insert --features images
cd examples/web && trunk serve --open                  # in the browser
```

## Documentation

See [`docs/`](docs/README.md) and [docs.rs](https://docs.rs/iced-markdown-editor).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE.txt) or [MIT license](LICENSE-MIT.txt) at your option. The bundled DejaVu fonts have their own license (`assets/fonts/LICENSE-DejaVu.txt`).
