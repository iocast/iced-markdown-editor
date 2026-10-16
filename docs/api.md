# API

| Item | Purpose |
|---|---|
| `EditorState` | Document, selection, history, settings |
| `markdown_editor(&state)` | The widget (`.toolbar(bool)`, `.status_bar(bool)`, `.view()`) |
| `Action`, `Output`, `Event`, `Command` | Message flow |
| `Paper`, `PageSize`, `Margins`, `Measure` | Page setup |
| `ZoomSettings`, `KeyBinding` | Zoom |
| `EditorStyle`, `Palette`, `Typography` | Theming |
| `ImageSettings`, `PastedImage`, `ImageRejection` | Images |
| `CodeHighlighter`, `SyntectHighlighter` | Code highlighting |
| `Language` | Interface language: English, German, French, Spanish, Italian (`.language(…)`) |
| `Features` | Switch tables, task lists, strikethrough on or off (`.features(…)`) |
| `BlockRenderer`, `RenderRequest` | Draw fenced code blocks of chosen languages, e.g. math or diagrams (`.block_renderer(…)`) |
| `normalize(&str)` | Markdown as the editor writes it |

Full reference on docs.rs.
