//! Tables: inserting, row and column commands, Tab and Enter in cells.

use super::super::{Action, EditKind, EditorState, Snapshot};
use crate::doc::{Block, ColumnAlign, Document, Leaf, Pos, Selection};
use iced::Task;

/// The header cells of a paragraph typed as `| a | b |` (autoformat).
pub(super) fn table_header(text: &str) -> Option<Vec<String>> {
    let inner = text.trim().strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<String> = inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect();
    (!inner.trim().is_empty() && cells.len() <= 50).then_some(cells)
}

impl EditorState {
    pub(super) fn insert_table(&mut self, rows: usize, columns: usize) -> Task<Action> {
        let (a, b) = self.sel.range();
        if self.doc.blocks[a.block].cell().is_some() || self.doc.blocks[b.block].cell().is_some() {
            return Task::none();
        }
        self.begin_edit(EditKind::Other);
        let pos = self.doc.delete_range(a, b);
        let pos = self
            .doc
            .insert_table(pos, rows.clamp(1, 100), columns.clamp(1, 50));
        self.sel = Selection::caret(pos);
        self.pending = None;
        self.after_edit()
    }

    /// Runs a table operation on the cell at the caret.
    pub(super) fn table_edit(
        &mut self,
        edit: impl FnOnce(&mut Document, usize) -> Option<Pos>,
    ) -> Task<Action> {
        let block = self.sel.head.block;
        if self.doc.blocks[block].cell().is_none() {
            return Task::none();
        }
        let before = Snapshot {
            doc: self.doc.clone(),
            sel: self.sel,
        };
        let Some(pos) = edit(&mut self.doc, block) else {
            return Task::none();
        };
        self.undo.push(before);
        self.redo.clear();
        self.last_edit = EditKind::None;
        self.pending = None;
        self.sel = Selection::caret(self.doc.clamp(pos));
        self.after_edit()
    }

    pub(super) fn set_column_align(&mut self, align: ColumnAlign) -> Task<Action> {
        let block = self.sel.head.block;
        if self.doc.blocks[block].cell().is_none() {
            return Task::none();
        }
        let before = Snapshot {
            doc: self.doc.clone(),
            sel: self.sel,
        };
        if !self.doc.set_column_align(block, align) {
            return Task::none();
        }
        self.undo.push(before);
        self.redo.clear();
        self.last_edit = EditKind::None;
        self.after_edit()
    }

    /// Selects the content of a cell.
    fn select_cell(&mut self, block: usize) -> Task<Action> {
        self.sel = Selection {
            anchor: Pos::new(block, 0),
            head: Pos::new(block, self.block_len(block)),
        };
        self.pending = None;
        self.goal_x = None;
        self.last_edit = EditKind::None;
        self.touch();
        self.scroll_to_caret()
    }

    /// Tab: next cell, a new row after the last one. Shift+Tab: previous cell.
    pub(super) fn table_tab(&mut self, back: bool) -> Task<Action> {
        let block = self.sel.head.block;
        let Some((start, end)) = self.doc.table_range(block) else {
            return Task::none();
        };
        if back {
            return if block > start {
                self.select_cell(block - 1)
            } else {
                Task::none()
            };
        }
        if block + 1 < end {
            return self.select_cell(block + 1);
        }
        self.begin_edit(EditKind::Other);
        let Some(cell) = self.doc.insert_row(block, true) else {
            return Task::none();
        };
        let first = self
            .doc
            .cell_position(cell)
            .map(|(_, column)| cell - column)
            .unwrap_or(cell);
        self.sel = Selection::caret(Pos::new(first, 0));
        self.pending = None;
        self.after_edit()
    }

    /// Enter in a table: the cell below; after the last row a new row, or,
    /// in an empty last row, a paragraph after the table.
    pub(super) fn table_enter(&mut self) -> Task<Action> {
        let (a, b) = self.sel.range();
        let edited = a != b;
        if edited {
            self.begin_edit(EditKind::Other);
            let pos = self.doc.delete_range(a, b);
            self.sel = Selection::caret(pos);
        }
        let block = self.sel.head.block;
        let (Some((start, end)), Some((row, column))) =
            (self.doc.table_range(block), self.doc.cell_position(block))
        else {
            return if edited {
                self.after_edit()
            } else {
                Task::none()
            };
        };
        let rows = self.doc.table_rows(start, end);

        if row + 1 < rows.len() {
            let (first, last) = rows[row + 1];
            let below = first + column.min(last - first - 1);
            self.sel = Selection::caret(Pos::new(below, self.block_len(below)));
            self.pending = None;
            self.goal_x = None;
            if edited {
                return self.after_edit();
            }
            self.last_edit = EditKind::None;
            self.touch();
            return self.scroll_to_caret();
        }

        if !edited {
            self.begin_edit(EditKind::Other);
        }
        let (first, last) = rows[row];
        let empty = row > 0 && (first..last).all(|index| self.doc.blocks[index].is_empty());
        let next = if empty {
            let containers = self.doc.blocks[block].containers.clone();
            self.doc.blocks.drain(first..last);
            self.doc
                .blocks
                .insert(first, Block::new(containers, Leaf::Paragraph));
            Pos::new(first, 0)
        } else {
            let cell = self.doc.insert_row(block, true).unwrap_or(block);
            Pos::new(cell, 0)
        };
        self.sel = Selection::caret(next);
        self.pending = None;
        self.after_edit()
    }
}
