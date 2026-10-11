//! GFM tables in the document: a table is a run of cell blocks, row by row.

use super::{Block, Cell, ColumnAlign, Document, Leaf, Pos, next_id};

impl Document {
    /// Blocks `start..end` of the table containing `block`.
    pub fn table_range(&self, block: usize) -> Option<(usize, usize)> {
        let table = self.blocks.get(block)?.cell()?.table;
        let in_table = |index: usize| {
            self.blocks
                .get(index)
                .and_then(Block::cell)
                .is_some_and(|cell| cell.table == table)
        };
        let mut start = block;
        while start > 0 && in_table(start - 1) {
            start -= 1;
        }
        let mut end = block + 1;
        while in_table(end) {
            end += 1;
        }
        Some((start, end))
    }

    /// Rows `(start, end)` of the table occupying the blocks `start..end`.
    pub fn table_rows(&self, start: usize, end: usize) -> Vec<(usize, usize)> {
        let mut rows: Vec<(usize, usize)> = Vec::new();
        for index in start..end {
            let row = self.blocks[index].cell().map(|cell| cell.row);
            match rows.last_mut() {
                Some(last) if self.blocks[last.0].cell().map(|cell| cell.row) == row => {
                    last.1 = index + 1;
                }
                _ => rows.push((index, index + 1)),
            }
        }
        rows
    }

    /// `(row, column)` of a cell inside its table.
    pub fn cell_position(&self, block: usize) -> Option<(usize, usize)> {
        let (start, end) = self.table_range(block)?;
        self.table_rows(start, end)
            .iter()
            .enumerate()
            .find(|(_, (first, last))| (*first..*last).contains(&block))
            .map(|(row, (first, _))| (row, block - first))
    }

    /// Alignment of every column, taken from the header row.
    fn column_aligns(&self, start: usize, end: usize) -> Vec<ColumnAlign> {
        let rows = self.table_rows(start, end);
        let columns = rows.iter().map(|(a, b)| b - a).max().unwrap_or(0);
        let (first, last) = rows[0];
        (0..columns)
            .map(|column| {
                (first + column < last)
                    .then(|| self.blocks[first + column].cell())
                    .flatten()
                    .map(|cell| cell.align)
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Inserts an empty table with a header row and `rows - 1` body rows at
    /// `at`. Returns the position in the first header cell.
    pub fn insert_table(&mut self, at: Pos, rows: usize, columns: usize) -> Pos {
        let rows = rows.max(1);
        let columns = columns.max(1);
        if self.blocks[at.block].cell().is_some() {
            return at;
        }
        let containers = self.blocks[at.block].containers.clone();
        let replace = {
            let block = &self.blocks[at.block];
            block.is_empty() && matches!(block.leaf, Leaf::Paragraph | Leaf::Rule)
        };
        let index = if replace {
            self.blocks.remove(at.block);
            at.block
        } else if at.offset == 0 {
            at.block
        } else if at.offset >= self.blocks[at.block].len() {
            at.block + 1
        } else {
            self.split_block(at).block
        };

        let table = next_id();
        let mut cells = Vec::with_capacity(rows * columns);
        for row in 0..rows {
            let row_id = next_id();
            for _ in 0..columns {
                cells.push(Block::new(
                    containers.clone(),
                    Leaf::Cell(Cell {
                        table,
                        row: row_id,
                        header: row == 0,
                        align: ColumnAlign::None,
                    }),
                ));
            }
        }
        let after = index + cells.len();
        self.blocks.splice(index..index, cells);
        if after >= self.blocks.len() {
            self.blocks.push(Block::new(containers, Leaf::Paragraph));
        }
        Pos::new(index, 0)
    }

    /// Inserts an empty row above or below the row of `block`. Returns the
    /// cell of the new row in the same column.
    pub fn insert_row(&mut self, block: usize, below: bool) -> Option<usize> {
        let (start, end) = self.table_range(block)?;
        let (row, column) = self.cell_position(block)?;
        let rows = self.table_rows(start, end);
        let aligns = self.column_aligns(start, end);
        let template = self.blocks[block].cell()?;
        let containers = self.blocks[block].containers.clone();
        let header = row == 0 && !below;
        if header {
            // The new row becomes the header.
            for block in &mut self.blocks[rows[0].0..rows[0].1] {
                if let Leaf::Cell(cell) = &mut block.leaf {
                    cell.header = false;
                }
            }
        }
        let row_id = next_id();
        let cells: Vec<Block> = aligns
            .iter()
            .map(|align| {
                Block::new(
                    containers.clone(),
                    Leaf::Cell(Cell {
                        row: row_id,
                        header,
                        align: *align,
                        ..template
                    }),
                )
            })
            .collect();
        let at = if below { rows[row].1 } else { rows[row].0 };
        let count = cells.len();
        self.blocks.splice(at..at, cells);
        Some(at + column.min(count - 1))
    }

    /// Deletes the row of `block` (the whole table if it is the only row).
    pub fn delete_row(&mut self, block: usize) -> Option<Pos> {
        let (start, end) = self.table_range(block)?;
        let (row, column) = self.cell_position(block)?;
        let rows = self.table_rows(start, end);
        if rows.len() == 1 {
            return self.delete_table(block);
        }
        let (first, last) = rows[row];
        self.blocks.drain(first..last);
        if row == 0 {
            // The next row becomes the header.
            let count = rows[1].1 - rows[1].0;
            for block in &mut self.blocks[first..first + count] {
                if let Leaf::Cell(cell) = &mut block.leaf {
                    cell.header = true;
                }
            }
        }
        let rows = self.table_rows(start, end - (last - first));
        let (a, b) = rows[row.min(rows.len() - 1)];
        Some(Pos::new(a + column.min(b - a - 1), 0))
    }

    /// Inserts an empty column left or right of the column of `block`.
    /// Returns the new cell in the row of `block`.
    pub fn insert_column(&mut self, block: usize, right: bool) -> Option<usize> {
        let (start, end) = self.table_range(block)?;
        let (row, column) = self.cell_position(block)?;
        let at = if right { column + 1 } else { column };
        let rows = self.table_rows(start, end);
        // From the last row, so the indices of earlier rows stay valid.
        let mut target = block;
        for (r, (first, last)) in rows.iter().enumerate().rev() {
            let cell = self.blocks[*first].cell()?;
            let containers = self.blocks[*first].containers.clone();
            let index = first + at.min(last - first);
            self.blocks.insert(
                index,
                Block::new(
                    containers,
                    Leaf::Cell(Cell {
                        align: ColumnAlign::None,
                        ..cell
                    }),
                ),
            );
            if r == row {
                target = index;
            }
        }
        // Rows above `row` gained one cell each.
        Some(target + row)
    }

    /// Deletes the column of `block` (the whole table if it is the only one).
    pub fn delete_column(&mut self, block: usize) -> Option<Pos> {
        let (start, end) = self.table_range(block)?;
        let (row, column) = self.cell_position(block)?;
        let rows = self.table_rows(start, end);
        if rows.iter().all(|(a, b)| b - a <= 1) {
            return self.delete_table(block);
        }
        let mut removed = 0;
        for (first, last) in rows.iter().rev() {
            if column < last - first && last - first > 1 {
                self.blocks.remove(first + column);
                removed += 1;
            }
        }
        let rows = self.table_rows(start, end - removed);
        let (a, b) = rows[row];
        Some(Pos::new(a + column.min(b - a - 1), 0))
    }

    /// Sets the alignment of the column of `block`. `false` if nothing changed.
    pub fn set_column_align(&mut self, block: usize, align: ColumnAlign) -> bool {
        let (Some((start, end)), Some((_, column))) =
            (self.table_range(block), self.cell_position(block))
        else {
            return false;
        };
        let mut changed = false;
        for (first, last) in self.table_rows(start, end) {
            if first + column < last
                && let Leaf::Cell(cell) = &mut self.blocks[first + column].leaf
            {
                changed |= cell.align != align;
                cell.align = align;
            }
        }
        changed
    }

    /// Replaces the table of `block` with an empty paragraph.
    pub fn delete_table(&mut self, block: usize) -> Option<Pos> {
        let (start, end) = self.table_range(block)?;
        let containers = self.blocks[start].containers.clone();
        self.blocks
            .splice(start..end, [Block::new(containers, Leaf::Paragraph)]);
        Some(Pos::new(start, 0))
    }

    /// [`Document::delete_range`] when tables are involved. Tables the range
    /// covers completely and reaches beyond are removed; other cells only
    /// lose their text.
    pub(super) fn delete_range_in_tables(&mut self, a: Pos, b: Pos) -> Pos {
        let mut remove = vec![false; self.blocks.len()];
        let mut index = a.block;
        while index <= b.block {
            if let Some((start, end)) = self.table_range(index) {
                let last = Pos::new(end - 1, self.blocks[end - 1].len());
                let covered = a <= Pos::new(start, 0) && b >= last;
                if covered && (a.block < start || b.block >= end) {
                    remove[start..end].fill(true);
                }
                index = end;
            } else {
                remove[index] = index > a.block && index < b.block;
                index += 1;
            }
        }

        // Text removed from the blocks that stay, from the back.
        for (index, start, end) in self.spans(a, b).into_iter().rev() {
            if !remove[index] {
                let block = &mut self.blocks[index];
                block.chars.drain(start..end);
                block.styles.drain(start..end);
            }
        }

        let new_a = remove[..a.block].iter().filter(|gone| !**gone).count();
        let new_b =
            (!remove[b.block]).then(|| remove[..b.block].iter().filter(|gone| !**gone).count());
        let a_removed = remove[a.block];
        let mut flags = remove.into_iter();
        self.blocks.retain(|_| !flags.next().unwrap_or(false));

        if self.blocks.is_empty() {
            self.blocks.push(Block::new(Vec::new(), Leaf::Paragraph));
            return Pos::new(0, 0);
        }
        if new_a >= self.blocks.len() {
            let containers = self.blocks[self.blocks.len() - 1].containers.clone();
            self.blocks.push(Block::new(containers, Leaf::Paragraph));
            return Pos::new(self.blocks.len() - 1, 0);
        }
        if a_removed {
            return Pos::new(new_a, 0);
        }

        // Two text blocks that now touch are joined, as without tables.
        let joinable = new_b == Some(new_a + 1)
            && a.block != b.block
            && self.blocks[new_a].cell().is_none()
            && self.blocks[new_a + 1].cell().is_none();
        if joinable {
            return self.merge_with_previous(new_a + 1);
        }
        Pos::new(new_a, a.offset.min(self.blocks[new_a].len()))
    }
}
