//! Optional Markdown features, switched on or off at runtime.
//!
//! The editor supports the GFM extensions strikethrough, task lists and
//! tables. Each can be turned off with [`Features`]. A disabled feature:
//!
//! * is not recognised when Markdown is loaded (the text stays a normal
//!   paragraph, e.g. `| a | b |` or `~~text~~`),
//! * has no toolbar button,
//! * ignores its [`Command`]s and shortcuts ([`Features::allows`]),
//! * is not created by autoformat (typing `[ ] ` or `| a | b |` + Enter).
//!
//! Set the features before loading the document. Switching a feature off
//! later turns its content into text; switching it on again does not
//! restore it.
//!
//! These are not Cargo features; Cargo features (`syntect`, `images`) add
//! code, these switch editor behaviour.
//!
//! ```
//! use iced_markdown_editor::{EditorState, Features};
//!
//! let mut editor = EditorState::new().features(Features {
//!     tables: false,
//!     ..Features::default()
//! });
//! editor.set_markdown("| a |\n| - |\n");
//! assert!(!editor.formatting().table);
//! assert_eq!(editor.markdown(), "\\| a \\|\n\\| - \\|\n");
//! ```

use crate::doc::BlockKind;
use crate::widget::Command;

/// Optional Markdown features. All are enabled by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features {
    /// GFM tables.
    pub tables: bool,
    /// GFM task lists (`- [ ]`, `- [x]`).
    pub task_lists: bool,
    /// GFM strikethrough (`~~text~~`).
    pub strikethrough: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self::ALL
    }
}

impl Features {
    /// Every feature enabled (the default).
    pub const ALL: Self = Self {
        tables: true,
        task_lists: true,
        strikethrough: true,
    };

    /// Plain CommonMark: every optional feature disabled.
    pub const COMMONMARK: Self = Self {
        tables: false,
        task_lists: false,
        strikethrough: false,
    };

    /// Whether `command` belongs to an enabled feature. Commands of disabled
    /// features do nothing; use this to hide them in a custom toolbar.
    pub fn allows(&self, command: &Command) -> bool {
        match command {
            Command::ToggleStrike => self.strikethrough,
            Command::SetBlock(BlockKind::Task) | Command::ToggleChecked => self.task_lists,
            Command::InsertTable { .. }
            | Command::InsertRow { .. }
            | Command::InsertColumn { .. }
            | Command::DeleteRow
            | Command::DeleteColumn
            | Command::DeleteTable
            | Command::SetColumnAlign(_) => self.tables,
            _ => true,
        }
    }
}
