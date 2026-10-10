# Getting started

```toml
[dependencies]
iced = "0.14"
iced-markdown-editor = "0.2.5"
```

1. Register the bundled fonts on the application: `for font in FONTS { app = app.font(font); }`.
   Without them the caret does not line up with the text.
2. Keep an `EditorState` in your state, e.g. `EditorState::with_markdown(text).paper(Paper::a4())`.
3. In `update`, forward `Action`s with `self.editor.update(action)`, handle the
   events of the returned `Output`, and return `output.task.map(Message::Editor)`.
   Commands you run with `execute(command)` return an `Output` too; handle it the same way.
4. In `view`: `markdown_editor(&self.editor).view().map(Message::Editor)`.
5. In `subscription`: `self.editor.subscription().map(Message::Editor)`
   (caret blinking, dropped files).

`markdown()` builds the whole text: call it on `Event::Changed` or when
saving, not in `view`. See `examples/minimal.rs`.
