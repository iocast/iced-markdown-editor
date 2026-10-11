# Changelog

All notable changes to this project are documented in this file.
Format: Keep a Changelog (https://keepachangelog.com), versioning: Semantic Versioning (https://semver.org).

## [Unreleased]

### Added
- GFM tables: import and export with column alignment (also inside lists and block quotes), shown with borders and a shaded header row
- Table editing: `Command::InsertTable`, `InsertRow`, `InsertColumn`, `DeleteRow`, `DeleteColumn`, `DeleteTable` and `SetColumnAlign` (new `ColumnAlign` type), with toolbar buttons
- In a table `Tab` / `Shift+Tab` move between cells (adding a row after the last cell) and `Enter` moves to the cell below; typing `| a | b |` and pressing `Enter` creates a table
- `Formatting::table` and `Formatting::column_align`

## [0.2.6] - 2026-10-10

### Added
- WYSIWYG CommonMark editor widget for iced 0.14 with the GFM extensions strikethrough and task lists; round trip tested against all 652 examples of the CommonMark 0.31.2 spec
- Word-processor page layout (A4, Letter, custom sizes and margins) or full width
- Formatting toolbar, keyboard shortcuts and Word-style autoformat
- Nested lists, task lists with clickable checkboxes, block quotes and code blocks
- Markdown source mode on the same pages as the WYSIWYG view
- Zoom, undo / redo and clipboard support
- The editor scrolls itself (mouse wheel, Shift+wheel, draggable scrollbars, keyboard) and only draws the visible part of the document
- Optional `syntect` feature: `SyntectHighlighter` for syntax highlighting in code blocks
- Optional `images` feature: display images, and paste, drop or insert them; `ImageSettings` rejects, embeds (`data:` URLs) or delegates them to the app
- Web support (WebAssembly), with a web example and a Zola website with a live demo
- Examples: `editor`, `minimal`, `images_folder`, `images_embed`, `images_upload`, `images_insert`
- Minimum Rust version 1.88

[unreleased]: https://github.com/iocast/iced-markdown-editor/compare/v0.2.6...HEAD
[0.2.6]: https://github.com/iocast/iced-markdown-editor/releases/tag/v0.2.6
