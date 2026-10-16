# Changelog

All notable changes to this project are documented in this file.
Format: Keep a Changelog (https://keepachangelog.com), versioning: Semantic Versioning (https://semver.org).

## [Unreleased]

### Added
- The toolbar, link bar and status bar are available in English, German, French, Spanish and Italian: `Language` (`EditorState::language`, `set_language`, `current_language`); `Language::from_tag` maps a locale such as `de-CH` to a language
- `ImageRejection::message` gives the rejection reason as a translated sentence for the user

### Changed
- The status bar uses the singular for one item, e.g. "1 word" instead of "1 words"

## [0.3.0] - 2026-10-11

### Added
- GFM tables: import and export with column alignment (also inside lists and block quotes), shown with borders and a shaded header row
- Table editing: `Command::InsertTable`, `InsertRow`, `InsertColumn`, `DeleteRow`, `DeleteColumn`, `DeleteTable` and `SetColumnAlign` (new `ColumnAlign` type), with toolbar buttons
- In a table `Tab` / `Shift+Tab` move between cells (adding a row after the last cell) and `Enter` moves to the cell below; typing `| a | b |` and pressing `Enter` creates a table
- `Formatting::table` and `Formatting::column_align`
- `Features` (`EditorState::features`, `set_features`) switches tables, task lists and strikethrough on or off at runtime; `Features::COMMONMARK` gives plain CommonMark, and `Features::allows` helps hide commands in a custom toolbar
- `BlockRenderer` trait (`EditorState::block_renderer`, `extension` module) draws fenced code blocks of chosen languages, e.g. formulas or diagrams, while the caret is elsewhere; the Markdown is unchanged

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

[unreleased]: https://github.com/iocast/iced-markdown-editor/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/iocast/iced-markdown-editor/compare/v0.2.6...v0.3.0
[0.2.6]: https://github.com/iocast/iced-markdown-editor/releases/tag/v0.2.6
