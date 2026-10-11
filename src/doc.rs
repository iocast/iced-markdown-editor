//! Rich document model following the CommonMark structure.
//!
//! The document is a flat list of leaf blocks (paragraphs, headings, code
//! blocks, HTML blocks, thematic breaks, table cells, or source lines in
//! source mode). A table is a run of cells, row by row.
//! Each block carries the path of container blocks it lives in (block quotes
//! and list items), outermost first. Two blocks are in the same container
//! when the container ids are equal. Every character carries its own inline
//! style, like a word processor. Positions are `(block, character offset)`.
//!
//! Special characters inside inline content:
//! * `'\n'`  hard line break
//! * [`SOFT`] soft line break (displayed as a space)
//! * [`OBJ`]  placeholder for an empty link text or an empty image alt

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Soft line break (a line ending inside a paragraph).
pub const SOFT: char = '\u{2028}';
/// Placeholder for an empty link text / empty image description.
pub const OBJ: char = '\u{FFFC}';

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A new unique id for containers and lists.
pub fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Link or image target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Link {
    pub url: String,
    pub title: Option<String>,
}

impl Link {
    pub fn new(url: impl Into<String>, title: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            url: url.into(),
            title: title.filter(|title| !title.is_empty()),
        })
    }
}

/// Inline formatting of a single character.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<Arc<Link>>,
    /// The character is part of an image description (alt text).
    pub image: Option<Arc<Link>>,
    /// Raw inline HTML.
    pub html: bool,
}

/// Toggleable inline formatting flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Bold,
    Italic,
    Strike,
    Code,
}

impl Style {
    pub fn get(&self, flag: Flag) -> bool {
        match flag {
            Flag::Bold => self.bold,
            Flag::Italic => self.italic,
            Flag::Strike => self.strike,
            Flag::Code => self.code,
        }
    }

    pub fn set(&mut self, flag: Flag, value: bool) {
        match flag {
            Flag::Bold => self.bold = value,
            Flag::Italic => self.italic = value,
            Flag::Strike => self.strike = value,
            Flag::Code => self.code = value,
        }
    }
}

/// A list shared by its items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListInfo {
    pub id: u64,
    pub ordered: bool,
    /// Number of the first item (ordered lists).
    pub start: u64,
    /// Tight lists have no blank lines between their items.
    pub tight: bool,
}

impl ListInfo {
    pub fn new(ordered: bool, start: u64) -> Self {
        Self {
            id: next_id(),
            ordered,
            start,
            tight: true,
        }
    }
}

/// A container block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Quote {
        id: u64,
    },
    Item {
        list: ListInfo,
        id: u64,
        /// Task list item (GFM): `Some(checked)`.
        task: Option<bool>,
    },
}

impl Container {
    pub fn quote() -> Self {
        Container::Quote { id: next_id() }
    }

    pub fn item(list: ListInfo) -> Self {
        Self::task_item(list, None)
    }

    pub fn task_item(list: ListInfo, task: Option<bool>) -> Self {
        Container::Item {
            list,
            id: next_id(),
            task,
        }
    }

    /// The same container instance (same quote / same list item).
    pub fn same(&self, other: &Container) -> bool {
        match (self, other) {
            (Container::Quote { id: a }, Container::Quote { id: b }) => a == b,
            (
                Container::Item {
                    list: la, id: a, ..
                },
                Container::Item {
                    list: lb, id: b, ..
                },
            ) => la.id == lb.id && a == b,
            _ => false,
        }
    }

    pub fn list(&self) -> Option<ListInfo> {
        match self {
            Container::Item { list, .. } => Some(*list),
            Container::Quote { .. } => None,
        }
    }

    pub fn task(&self) -> Option<bool> {
        match self {
            Container::Item { task, .. } => *task,
            Container::Quote { .. } => None,
        }
    }

    pub fn is_quote(&self) -> bool {
        matches!(self, Container::Quote { .. })
    }
}

/// Horizontal alignment of a table column (GFM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnAlign {
    /// No alignment given (`---`), displayed left.
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// A table cell (GFM). The cells of a table are consecutive blocks with the
/// same `table` id, row by row; the cells of a row share the `row` id. The
/// first row is the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub table: u64,
    pub row: u64,
    pub header: bool,
    /// Alignment of the column (the same in all cells of a column).
    pub align: ColumnAlign,
}

/// Leaf block kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaf {
    Paragraph,
    Heading(u8),
    Code,
    Html,
    Rule,
    /// A table cell.
    Cell(Cell),
    /// A line of Markdown source (source mode).
    Line,
}

impl Leaf {
    pub fn is_text(self) -> bool {
        matches!(self, Leaf::Paragraph | Leaf::Heading(_) | Leaf::Cell(_))
    }

    pub fn cell(self) -> Option<Cell> {
        match self {
            Leaf::Cell(cell) => Some(cell),
            _ => None,
        }
    }

    /// Code and HTML blocks: line breaks stay inside the block.
    pub fn is_raw(self) -> bool {
        matches!(self, Leaf::Code | Leaf::Html)
    }

    /// No inline formatting (code, HTML, source lines).
    pub fn is_plain(self) -> bool {
        matches!(self, Leaf::Code | Leaf::Html | Leaf::Line)
    }
}

/// Paragraph-level formatting as offered in the toolbar and the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading(u8),
    /// Block quote around the paragraph.
    Quote,
    /// Bullet list item.
    Bullet,
    /// Numbered list item.
    Ordered,
    /// Task list item (`- [ ]`, GFM).
    Task,
    Code,
    Rule,
    /// Raw HTML block.
    Html,
}

impl BlockKind {
    pub fn is_list(self) -> bool {
        matches!(
            self,
            BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task
        )
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub containers: Vec<Container>,
    pub leaf: Leaf,
    pub chars: Vec<char>,
    pub styles: Vec<Style>,
    /// Info string of a fenced code block, e.g. `rust` or `python title="x"`.
    pub info: Option<Arc<str>>,
}

impl Block {
    pub fn new(containers: Vec<Container>, leaf: Leaf) -> Self {
        Self {
            containers,
            leaf,
            chars: Vec::new(),
            styles: Vec::new(),
            info: None,
        }
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn push(&mut self, c: char, style: Style) {
        self.chars.push(c);
        self.styles.push(style);
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// The language: first word of the info string.
    pub fn language(&self) -> Option<&str> {
        self.info
            .as_deref()
            .and_then(|info| info.split_whitespace().next())
            .filter(|language| !language.is_empty())
    }

    pub fn cell(&self) -> Option<Cell> {
        self.leaf.cell()
    }

    /// The innermost container.
    pub fn inner(&self) -> Option<&Container> {
        self.containers.last()
    }

    /// Offset of the first character of the line containing `offset`
    /// (lines are separated by `\n` inside the block).
    pub fn line_start(&self, offset: usize) -> usize {
        let offset = offset.min(self.chars.len());
        self.chars[..offset]
            .iter()
            .rposition(|c| *c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    /// Start offsets of all lines in the block.
    pub fn line_starts(&self) -> Vec<usize> {
        let mut starts = vec![0];
        for (i, c) in self.chars.iter().enumerate() {
            if *c == '\n' {
                starts.push(i + 1);
            }
        }
        starts
    }

    /// The user-facing kind of this block.
    pub fn kind(&self) -> BlockKind {
        match self.leaf {
            Leaf::Heading(level) => BlockKind::Heading(level),
            Leaf::Code => BlockKind::Code,
            Leaf::Rule => BlockKind::Rule,
            Leaf::Html => BlockKind::Html,
            Leaf::Line | Leaf::Cell(_) => BlockKind::Paragraph,
            Leaf::Paragraph => match self.inner() {
                Some(Container::Item { task: Some(_), .. }) => BlockKind::Task,
                Some(Container::Item { list, .. }) if list.ordered => BlockKind::Ordered,
                Some(Container::Item { .. }) => BlockKind::Bullet,
                Some(Container::Quote { .. }) => BlockKind::Quote,
                None => BlockKind::Paragraph,
            },
        }
    }
}

/// A caret position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Pos {
    pub block: usize,
    pub offset: usize,
}

impl Pos {
    pub fn new(block: usize, offset: usize) -> Self {
        Self { block, offset }
    }
}

/// A selection. When `anchor == head` it is a plain caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: Pos,
    pub head: Pos,
}

impl Selection {
    pub fn caret(pos: Pos) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }

    pub fn range(&self) -> (Pos, Pos) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\'' || c == '\u{2019}'
}

fn is_break(c: char) -> bool {
    c == '\n' || c == SOFT
}

#[derive(Debug, Clone)]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            blocks: vec![Block::new(Vec::new(), Leaf::Paragraph)],
        }
    }
}

impl Document {
    pub fn start(&self) -> Pos {
        Pos::new(0, 0)
    }

    pub fn end(&self) -> Pos {
        let last = self.blocks.len() - 1;
        Pos::new(last, self.blocks[last].len())
    }

    pub fn clamp(&self, pos: Pos) -> Pos {
        let block = pos.block.min(self.blocks.len() - 1);
        Pos::new(block, pos.offset.min(self.blocks[block].len()))
    }

    pub fn word_count(&self) -> usize {
        self.blocks
            .iter()
            .map(|block| {
                block
                    .text()
                    .split(|c: char| c.is_whitespace() || c == OBJ)
                    .filter(|word| !word.is_empty())
                    .count()
            })
            .sum()
    }

    /// Per-block character ranges `(block, start, end)` covered by `a..b`.
    pub fn spans(&self, a: Pos, b: Pos) -> Vec<(usize, usize, usize)> {
        (a.block..=b.block)
            .map(|index| {
                let start = if index == a.block { a.offset } else { 0 };
                let end = if index == b.block {
                    b.offset
                } else {
                    self.blocks[index].len()
                };
                (index, start, end.max(start))
            })
            .collect()
    }

    /// Index of the first block of the container `depth` of `block`
    /// (walking back while the previous block shares it).
    pub fn container_start(&self, block: usize, depth: usize) -> usize {
        let container = self.blocks[block].containers[depth];
        let mut index = block;
        while index > 0 {
            let previous = &self.blocks[index - 1];
            if previous
                .containers
                .get(depth)
                .is_some_and(|other| other.same(&container))
            {
                index -= 1;
            } else {
                break;
            }
        }
        index
    }

    /// Index one past the last block of the container `depth` of `block`.
    pub fn container_end(&self, block: usize, depth: usize) -> usize {
        let container = self.blocks[block].containers[depth];
        let mut index = block + 1;
        while index < self.blocks.len()
            && self.blocks[index]
                .containers
                .get(depth)
                .is_some_and(|other| other.same(&container))
        {
            index += 1;
        }
        index
    }

    /// Sets the task state of the list item at `depth` of `block` (on all
    /// blocks of that item). `None` turns it into a normal list item.
    pub fn set_task(&mut self, block: usize, depth: usize, task: Option<bool>) {
        let start = self.container_start(block, depth);
        let end = self.container_end(block, depth);
        for block in &mut self.blocks[start..end] {
            if let Some(Container::Item { task: value, .. }) = block.containers.get_mut(depth) {
                *value = task;
            }
        }
    }

    // ----------------------------------------------------------------
    // Editing
    // ----------------------------------------------------------------

    /// Deletes `a..b` and returns the resulting caret position.
    pub fn delete_range(&mut self, a: Pos, b: Pos) -> Pos {
        if a >= b {
            return a;
        }
        if (a.block..=b.block).any(|index| self.blocks[index].cell().is_some()) {
            return self.delete_range_in_tables(a, b);
        }

        if a.block == b.block {
            let block = &mut self.blocks[a.block];
            block.chars.drain(a.offset..b.offset);
            block.styles.drain(a.offset..b.offset);
        } else {
            let tail_chars: Vec<char> = self.blocks[b.block].chars[b.offset..].to_vec();
            let tail_styles: Vec<Style> = self.blocks[b.block].styles[b.offset..].to_vec();

            {
                let first = &mut self.blocks[a.block];
                first.chars.truncate(a.offset);
                first.styles.truncate(a.offset);
                first.chars.extend(tail_chars);
                first.styles.extend(tail_styles);
                if first.leaf == Leaf::Rule && !first.chars.is_empty() {
                    first.leaf = Leaf::Paragraph;
                }
            }

            self.blocks.drain(a.block + 1..=b.block);
        }

        a
    }

    /// Inserts a single character without any paragraph handling.
    pub fn insert_char(&mut self, at: Pos, c: char, style: &Style) -> Pos {
        let block = &mut self.blocks[at.block];
        if block.leaf == Leaf::Rule {
            block.leaf = Leaf::Paragraph;
        }
        let style = if block.leaf.is_plain() {
            Style::default()
        } else {
            style.clone()
        };
        block.chars.insert(at.offset, c);
        block.styles.insert(at.offset, style);
        Pos::new(at.block, at.offset + 1)
    }

    /// Inserts text. Newlines create new paragraphs (or line breaks
    /// inside code and HTML blocks).
    pub fn insert_text(&mut self, at: Pos, text: &str, style: &Style) -> Pos {
        let mut pos = at;
        for c in text.chars() {
            match c {
                '\r' => {}
                '\n' if self.blocks[pos.block].cell().is_some() => {
                    // A cell holds a single line.
                    pos = self.insert_char(pos, ' ', style);
                }
                '\n' => {
                    if self.blocks[pos.block].leaf.is_raw() {
                        pos = self.insert_char(pos, '\n', style);
                    } else {
                        pos = self.split_block(pos);
                    }
                }
                '\t' => pos = self.insert_char(pos, '\t', style),
                c if c.is_control() => {}
                c => pos = self.insert_char(pos, c, style),
            }
        }
        pos
    }

    /// Inserts `count` spaces at `offset` of `block` (unstyled).
    pub fn insert_spaces(&mut self, block: usize, offset: usize, count: usize) {
        let block = &mut self.blocks[block];
        let offset = offset.min(block.chars.len());
        block
            .chars
            .splice(offset..offset, std::iter::repeat(' ').take(count));
        block.styles.splice(
            offset..offset,
            std::iter::repeat(Style::default()).take(count),
        );
    }

    /// Splits the block at `at` (the Enter key). In a list the new block
    /// becomes a new list item (an unchecked task in a task list); later
    /// blocks of the old item move into it.
    pub fn split_block(&mut self, at: Pos) -> Pos {
        let old = &self.blocks[at.block];
        let len = old.len();
        let leaf = match old.leaf {
            Leaf::Heading(_) if at.offset >= len => Leaf::Paragraph,
            Leaf::Rule => Leaf::Paragraph,
            other => other,
        };
        let info = if leaf == Leaf::Code {
            old.info.clone()
        } else {
            None
        };
        let mut containers = old.containers.clone();
        let item_depth = containers
            .iter()
            .rposition(|c| matches!(c, Container::Item { .. }));
        let mut moved: Option<(usize, Container, Container)> = None;
        if let Some(depth) = item_depth.filter(|depth| *depth + 1 == containers.len()) {
            if let Container::Item { list, task, .. } = containers[depth] {
                let old_item = containers[depth];
                let new_item = Container::task_item(list, task.map(|_| false));
                containers[depth] = new_item;
                moved = Some((depth, old_item, new_item));
            }
        }

        let block = &mut self.blocks[at.block];
        let chars = block.chars.split_off(at.offset);
        let styles = block.styles.split_off(at.offset);

        self.blocks.insert(
            at.block + 1,
            Block {
                containers,
                leaf,
                chars,
                styles,
                info,
            },
        );

        // Blocks that followed in the old item now belong to the new one.
        if let Some((depth, old_item, new_item)) = moved {
            let mut index = at.block + 2;
            while index < self.blocks.len()
                && self.blocks[index]
                    .containers
                    .get(depth)
                    .is_some_and(|c| c.same(&old_item))
            {
                self.blocks[index].containers[depth] = new_item;
                index += 1;
            }
        }

        Pos::new(at.block + 1, 0)
    }

    /// Joins `block` onto the previous block (Backspace at block start).
    pub fn merge_with_previous(&mut self, block: usize) -> Pos {
        if block == 0 || block >= self.blocks.len() {
            return Pos::new(block.min(self.blocks.len() - 1), 0);
        }

        if self.blocks[block - 1].leaf == Leaf::Rule {
            self.blocks.remove(block - 1);
            return Pos::new(block - 1, 0);
        }

        // Tables keep their cells: only empty blocks next to them go.
        let previous_cell = self.blocks[block - 1].cell().is_some();
        let current_cell = self.blocks[block].cell().is_some();
        if previous_cell && !current_cell {
            if self.blocks[block].is_empty() {
                self.blocks.remove(block);
            }
            return Pos::new(block - 1, self.blocks[block - 1].len());
        }
        if current_cell {
            if !previous_cell && self.blocks[block - 1].is_empty() {
                self.blocks.remove(block - 1);
                return Pos::new(block - 1, 0);
            }
            return Pos::new(block, 0);
        }

        let current = self.blocks.remove(block);
        let previous = &mut self.blocks[block - 1];
        let offset = previous.len();
        let plain = previous.leaf.is_plain();
        previous.chars.extend(current.chars);
        if plain {
            previous
                .styles
                .extend(std::iter::repeat(Style::default()).take(current.styles.len()));
        } else {
            previous.styles.extend(current.styles);
        }
        Pos::new(block - 1, offset)
    }

    // ----------------------------------------------------------------
    // Formatting
    // ----------------------------------------------------------------

    pub fn all_have(&self, a: Pos, b: Pos, flag: Flag) -> bool {
        let mut any = false;
        for (index, start, end) in self.spans(a, b) {
            let block = &self.blocks[index];
            if block.leaf.is_plain() {
                continue;
            }
            for (c, style) in block.chars[start..end]
                .iter()
                .zip(&block.styles[start..end])
            {
                if is_break(*c) {
                    continue;
                }
                any = true;
                if !style.get(flag) {
                    return false;
                }
            }
        }
        any
    }

    pub fn set_flag(&mut self, a: Pos, b: Pos, flag: Flag, value: bool) {
        for (index, start, end) in self.spans(a, b) {
            if self.blocks[index].leaf.is_plain() {
                continue;
            }
            for style in &mut self.blocks[index].styles[start..end] {
                if !style.html {
                    style.set(flag, value);
                }
            }
        }
    }

    pub fn set_link(&mut self, a: Pos, b: Pos, link: Option<Arc<Link>>) {
        for (index, start, end) in self.spans(a, b) {
            if self.blocks[index].leaf.is_plain() {
                continue;
            }
            for style in &mut self.blocks[index].styles[start..end] {
                if !style.html {
                    style.link = link.clone();
                }
            }
        }
    }

    pub fn link_at(&self, pos: Pos) -> Option<Arc<Link>> {
        self.blocks[pos.block]
            .styles
            .get(pos.offset)
            .and_then(|style| style.link.clone())
    }

    /// The style that newly typed text receives at `pos`.
    pub fn typing_style(&self, pos: Pos) -> Style {
        let block = &self.blocks[pos.block];
        if block.leaf.is_plain() || block.styles.is_empty() {
            return Style::default();
        }

        let mut style = if pos.offset == 0 {
            let mut style = block.styles[0].clone();
            style.link = None;
            style
        } else {
            let mut style = block.styles[pos.offset - 1].clone();
            let next_link = block
                .styles
                .get(pos.offset)
                .and_then(|next| next.link.clone());
            if style.link != next_link {
                style.link = None;
            }
            style
        };
        // Typing never extends an image description or raw HTML.
        style.image = None;
        style.html = false;
        style
    }

    /// Text of `a..b`; cells of a table row are separated by tabs.
    pub fn plain_text(&self, a: Pos, b: Pos) -> String {
        let mut out = String::new();
        for (index, start, end) in self.spans(a, b) {
            for c in &self.blocks[index].chars[start..end] {
                match *c {
                    SOFT => out.push(' '),
                    OBJ => {}
                    c => out.push(c),
                }
            }
            if index != b.block {
                let same_row = match (self.blocks[index].cell(), self.blocks[index + 1].cell()) {
                    (Some(this), Some(next)) => this.row == next.row,
                    _ => false,
                };
                out.push(if same_row { '\t' } else { '\n' });
            }
        }
        out
    }

    // ----------------------------------------------------------------
    // Tables
    // ----------------------------------------------------------------

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
    fn delete_range_in_tables(&mut self, a: Pos, b: Pos) -> Pos {
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

    // ----------------------------------------------------------------
    // Navigation
    // ----------------------------------------------------------------

    pub fn prev_pos(&self, pos: Pos) -> Pos {
        if pos.offset > 0 {
            Pos::new(pos.block, pos.offset - 1)
        } else if pos.block > 0 {
            Pos::new(pos.block - 1, self.blocks[pos.block - 1].len())
        } else {
            pos
        }
    }

    pub fn next_pos(&self, pos: Pos) -> Pos {
        if pos.offset < self.blocks[pos.block].len() {
            Pos::new(pos.block, pos.offset + 1)
        } else if pos.block + 1 < self.blocks.len() {
            Pos::new(pos.block + 1, 0)
        } else {
            pos
        }
    }

    pub fn word_left(&self, pos: Pos) -> Pos {
        if pos.offset == 0 {
            return self.prev_pos(pos);
        }
        let chars = &self.blocks[pos.block].chars;
        let mut offset = pos.offset;
        while offset > 0 && chars[offset - 1].is_whitespace() {
            offset -= 1;
        }
        if offset > 0 && is_word_char(chars[offset - 1]) {
            while offset > 0 && is_word_char(chars[offset - 1]) {
                offset -= 1;
            }
        } else if offset > 0 {
            offset -= 1;
        }
        Pos::new(pos.block, offset)
    }

    pub fn word_right(&self, pos: Pos) -> Pos {
        let chars = &self.blocks[pos.block].chars;
        let len = chars.len();
        if pos.offset >= len {
            return self.next_pos(pos);
        }
        let mut offset = pos.offset;
        if is_word_char(chars[offset]) {
            while offset < len && is_word_char(chars[offset]) {
                offset += 1;
            }
        } else if !chars[offset].is_whitespace() {
            offset += 1;
        }
        while offset < len && chars[offset].is_whitespace() {
            offset += 1;
        }
        Pos::new(pos.block, offset)
    }

    /// The word around `pos` (used by double click and formatting).
    pub fn word_at(&self, pos: Pos) -> (Pos, Pos) {
        let chars = &self.blocks[pos.block].chars;
        let mut start = pos.offset.min(chars.len());
        let mut end = start;
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }
        while end < chars.len() && is_word_char(chars[end]) {
            end += 1;
        }
        if start == end && end < chars.len() {
            end += 1;
        }
        (Pos::new(pos.block, start), Pos::new(pos.block, end))
    }
}
