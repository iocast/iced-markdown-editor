+++
title = "Getting started"
template = "page.html"
+++

```toml
[dependencies]
iced = "0.14"
iced-markdown-editor = { version = "0.3", features = ["syntect", "images"] }
```

1. Register the bundled fonts: `for font in FONTS { app = app.font(font); }`
2. Keep an `EditorState` in your application state.
3. Forward `Action`s in `update` and return `output.task`.
4. Show it with `markdown_editor(&self.editor).view()`.
5. Add `self.editor.subscription()` to your subscription.

See the [examples](https://github.com/iocast/iced-markdown-editor/tree/main/examples)
and the [API documentation](https://docs.rs/iced-markdown-editor).
