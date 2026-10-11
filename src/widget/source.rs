//! Markdown source mode.
//!
//! In source mode the document is the Markdown text, one [`Leaf::Line`]
//! block per line, shown on the same pages as WYSIWYG mode.
//!
//! * `Enter` in a list item continues the list, in a block quote continues
//!   the quote, and on an empty item leaves the list one level at a time.
//! * `Tab` / `Shift+Tab` in a list item nest / un-nest the item; elsewhere
//!   they insert / remove spaces. With a selection every line moves.
//! * Toolbar buttons and shortcuts work on the Markdown text of the
//!   selected lines.
//! * Long `data:` URLs are shown as `data:<mime>;embedded,<n>`.

use super::{Action, Command, EditKind, EditorState, Snapshot};
use crate::doc::{Block, BlockKind, Document, Leaf, Pos, Selection, Style};
use crate::markdown;
use iced::Task;
use std::collections::HashMap;

// ------------------------------------------------------------------------
// Conversion
// ------------------------------------------------------------------------

/// The Markdown text as a source document (one block per line).
pub(crate) fn document(text: &str) -> Document {
    let blocks = text
        .split('\n')
        .map(|line| {
            let mut block = Block::new(Vec::new(), Leaf::Line);
            for c in line.chars().filter(|c| *c != '\r') {
                block.push(c, Style::default());
            }
            block
        })
        .collect();
    Document { blocks }
}

/// The Markdown text of a source document.
pub(crate) fn text(doc: &Document) -> String {
    doc.blocks
        .iter()
        .map(Block::text)
        .collect::<Vec<_>>()
        .join("\n")
}

// ------------------------------------------------------------------------
// Long data: URLs
// ------------------------------------------------------------------------

/// `data:` URLs longer than this are shortened in source mode.
const LONG_DATA_URL: usize = 200;

/// Long `data:` URLs shown as `data:<mime>;embedded,<n>` in source mode.
/// The same URL always gets the same number.
#[derive(Debug, Default)]
pub(crate) struct Embedded {
    urls: HashMap<u64, String>,
    ids: HashMap<String, u64>,
    next: u64,
}

/// End of a URL starting at `start`.
fn url_end(text: &str, start: usize) -> usize {
    text[start..]
        .find(|c: char| c.is_whitespace() || matches!(c, ')' | '>' | '"' | '\'' | ']'))
        .map_or(text.len(), |i| start + i)
}

impl Embedded {
    /// Replaces long `data:` URLs with short tokens.
    pub(crate) fn shorten(&mut self, text: &str) -> String {
        let mut out = String::new();
        let mut copied = 0;
        let mut from = 0;
        while let Some(found) = text[from..].find("data:") {
            let start = from + found;
            let end = url_end(text, start);
            let url = &text[start..end];
            if url.len() > LONG_DATA_URL && !url.contains(";embedded,") {
                let mime = url[5..].split([';', ',']).next().unwrap_or("").to_string();
                let id = match self.ids.get(url) {
                    Some(id) => *id,
                    None => {
                        self.next += 1;
                        self.ids.insert(url.to_string(), self.next);
                        self.urls.insert(self.next, url.to_string());
                        self.next
                    }
                };
                out.push_str(&text[copied..start]);
                out.push_str(&format!("data:{mime};embedded,{id}"));
                copied = end;
            }
            from = end.max(start + 5);
        }
        out.push_str(&text[copied..]);
        out
    }

    /// Puts the full URLs back.
    pub(crate) fn expand(&self, text: &str) -> String {
        let mut out = String::new();
        let mut copied = 0;
        let mut from = 0;
        while let Some(found) = text[from..].find("data:") {
            let start = from + found;
            let end = url_end(text, start);
            let full = text[start..end]
                .split_once(";embedded,")
                .and_then(|(_, n)| n.parse::<u64>().ok())
                .and_then(|n| self.urls.get(&n));
            if let Some(full) = full {
                out.push_str(&text[copied..start]);
                out.push_str(full);
                copied = end;
            }
            from = end.max(start + 5);
        }
        out.push_str(&text[copied..]);
        out
    }
}

// ------------------------------------------------------------------------
// Line analysis
// ------------------------------------------------------------------------

/// A list marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Marker {
    Bullet(char),
    Ordered { number: u64, delim: char },
}

impl Marker {
    fn text(&self) -> String {
        match *self {
            Marker::Bullet(c) => format!("{c} "),
            Marker::Ordered { number, delim } => format!("{number}{delim} "),
        }
    }

    fn width(&self) -> usize {
        self.text().chars().count()
    }

    fn next(&self) -> Self {
        match *self {
            Marker::Ordered { number, delim } => Marker::Ordered {
                number: number + 1,
                delim,
            },
            bullet => bullet,
        }
    }

    fn first(&self) -> Self {
        match *self {
            Marker::Ordered { delim, .. } => Marker::Ordered { number: 1, delim },
            bullet => bullet,
        }
    }
}

/// The Markdown prefix of a source line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct LinePrefix {
    pub quote: String,
    pub indent: String,
    pub marker: Option<Marker>,
    pub task: Option<bool>,
    pub heading: Option<u8>,
    /// Characters taken by the prefix in the line.
    pub len: usize,
    /// Text after the prefix.
    pub rest: String,
}

impl LinePrefix {
    pub fn compose(&self) -> String {
        let mut out = self.quote.clone();
        out.push_str(&self.indent);
        if let Some(marker) = self.marker {
            out.push_str(&marker.text());
            if let Some(checked) = self.task {
                out.push_str(if checked { "[x] " } else { "[ ] " });
            }
        }
        if let Some(level) = self.heading {
            out.push_str(&"#".repeat(level.clamp(1, 6) as usize));
            out.push(' ');
        }
        out
    }

    fn is_blank(&self) -> bool {
        self.marker.is_none() && self.heading.is_none() && self.rest.trim().is_empty()
    }
}

fn indent_width(indent: &str) -> usize {
    indent.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

fn marker_at(chars: &[char], i: usize) -> Option<(Marker, usize)> {
    let len = chars.len();
    let ends = |k: usize| k == len || chars[k] == ' ' || chars[k] == '\t';
    let after = |k: usize| if k < len { k + 1 } else { k };

    let c = *chars.get(i)?;
    if matches!(c, '-' | '*' | '+') && ends(i + 1) {
        return Some((Marker::Bullet(c), after(i + 1)));
    }

    let mut k = i;
    while k < len && chars[k].is_ascii_digit() && k - i < 9 {
        k += 1;
    }
    if k > i && k < len && matches!(chars[k], '.' | ')') && ends(k + 1) {
        let number: String = chars[i..k].iter().collect();
        let number = number.parse().ok()?;
        return Some((
            Marker::Ordered {
                number,
                delim: chars[k],
            },
            after(k + 1),
        ));
    }
    None
}

/// Splits a source line into its Markdown prefix and the rest.
pub(crate) fn parse_line(line: &str) -> LinePrefix {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut prefix = LinePrefix::default();
    let mut i = 0;

    loop {
        let mut j = i;
        while j < len && j - i < 3 && chars[j] == ' ' {
            j += 1;
        }
        if j < len && chars[j] == '>' {
            j += 1;
            if j < len && chars[j] == ' ' {
                j += 1;
            }
            prefix.quote.extend(&chars[i..j]);
            i = j;
        } else {
            break;
        }
    }

    let start = i;
    while i < len && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    prefix.indent = chars[start..i].iter().collect();

    if let Some((marker, end)) = marker_at(&chars, i) {
        prefix.marker = Some(marker);
        i = end;
        if i + 2 < len
            && chars[i] == '['
            && matches!(chars[i + 1], ' ' | 'x' | 'X')
            && chars[i + 2] == ']'
            && (i + 3 == len || chars[i + 3] == ' ')
        {
            prefix.task = Some(chars[i + 1] != ' ');
            i = (i + 4).min(len);
        }
    }

    let mut k = i;
    while k < len && chars[k] == '#' && k - i < 7 {
        k += 1;
    }
    if (1..=6).contains(&(k - i)) && (k == len || chars[k] == ' ') {
        prefix.heading = Some((k - i) as u8);
        i = (k + 1).min(len);
    }

    prefix.len = i;
    prefix.rest = chars[i..].iter().collect();
    prefix
}

fn strip_last_quote(quote: &str) -> String {
    let trimmed = quote.trim_end();
    match trimmed.rfind('>') {
        Some(position) => trimmed[..position].to_string(),
        None => String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TabPlan {
    NotList,
    Nothing,
    Replace(String),
}

/// Nests (`outdent == false`) or un-nests the list item on line `index`.
pub(crate) fn plan_tab(lines: &[&str], index: usize, outdent: bool) -> TabPlan {
    let current = parse_line(lines[index]);
    let Some(marker) = current.marker else {
        return TabPlan::NotList;
    };
    let width = indent_width(&current.indent);

    if !outdent {
        let mut k = index;
        while k > 0 {
            k -= 1;
            let line = parse_line(lines[k]);
            if line.is_blank() {
                continue;
            }
            if line.quote != current.quote {
                break;
            }
            if let Some(sibling) = line.marker {
                if line.indent == current.indent {
                    let mut next = current.clone();
                    next.indent = format!("{}{}", current.indent, " ".repeat(sibling.width()));
                    next.marker = Some(marker.first());
                    return TabPlan::Replace(next.compose());
                }
            }
            if indent_width(&line.indent) < width {
                break;
            }
        }
        return TabPlan::Nothing;
    }

    let mut next = current.clone();
    if current.indent.is_empty() {
        next.marker = None;
        next.task = None;
        return TabPlan::Replace(next.compose());
    }

    let mut k = index;
    while k > 0 {
        k -= 1;
        let line = parse_line(lines[k]);
        if line.is_blank() {
            continue;
        }
        if line.quote != current.quote {
            break;
        }
        if let Some(parent) = line.marker {
            if indent_width(&line.indent) < width {
                next.indent = line.indent.clone();
                if let (Marker::Ordered { .. }, Marker::Ordered { .. }) = (parent, marker) {
                    next.marker = Some(parent.next());
                }
                return TabPlan::Replace(next.compose());
            }
        }
    }
    next.indent = String::new();
    TabPlan::Replace(next.compose())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EnterPlan {
    Plain,
    Continue(String),
    Replace { old: usize, new: String },
}

/// What Enter does on line `index` with the cursor at character `col`.
pub(crate) fn plan_enter(lines: &[&str], index: usize, col: usize) -> EnterPlan {
    let current = parse_line(lines[index]);
    if col < current.len {
        return EnterPlan::Plain;
    }
    let empty = current.heading.is_none() && current.rest.trim().is_empty();

    if let Some(marker) = current.marker {
        if empty {
            return match plan_tab(lines, index, true) {
                TabPlan::Replace(new) => EnterPlan::Replace {
                    old: current.len,
                    new,
                },
                _ => EnterPlan::Plain,
            };
        }
        let mut next = current.clone();
        next.marker = Some(marker.next());
        next.task = current.task.map(|_| false);
        next.heading = None;
        return EnterPlan::Continue(next.compose());
    }

    if !current.quote.is_empty() {
        if empty && current.indent.is_empty() {
            return EnterPlan::Replace {
                old: current.len,
                new: strip_last_quote(&current.quote),
            };
        }
        if current.heading.is_some() {
            return EnterPlan::Continue(current.quote.clone());
        }
        return EnterPlan::Continue(format!("{}{}", current.quote, current.indent));
    }

    if !current.indent.is_empty() && !current.rest.trim().is_empty() {
        return EnterPlan::Continue(current.indent.clone());
    }

    EnterPlan::Plain
}

// ------------------------------------------------------------------------
// Editing
// ------------------------------------------------------------------------

impl EditorState {
    fn source_lines(&self) -> Vec<String> {
        self.doc.blocks.iter().map(Block::text).collect()
    }

    fn source_line_range(&self) -> (usize, usize) {
        let (a, b) = self.sel.range();
        let last = if b.block > a.block && b.offset == 0 {
            b.block - 1
        } else {
            b.block
        };
        (a.block, last)
    }

    fn replace_line_prefix(&mut self, line: usize, old: usize, new: &str) {
        let block = &mut self.doc.blocks[line];
        let old = old.min(block.chars.len());
        let new_len = new.chars().count();
        block.chars.splice(0..old, new.chars());
        block
            .styles
            .splice(0..old, std::iter::repeat(Style::default()).take(new_len));
        for pos in [&mut self.sel.anchor, &mut self.sel.head] {
            if pos.block == line {
                pos.offset = if pos.offset >= old {
                    pos.offset - old + new_len
                } else {
                    new_len
                };
            }
        }
    }

    /// Enter (`plain`: Shift+Enter, no list continuation).
    pub(crate) fn source_enter(&mut self, plain: bool) -> Task<Action> {
        self.begin_edit(EditKind::Other);
        let (a, b) = self.sel.range();
        let pos = self.doc.delete_range(a, b);
        self.sel = Selection::caret(pos);

        let plan = if plain {
            EnterPlan::Plain
        } else {
            let lines = self.source_lines();
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            plan_enter(&refs, pos.block, pos.offset)
        };

        match plan {
            EnterPlan::Plain => {
                let next = self.doc.split_block(pos);
                self.sel = Selection::caret(next);
            }
            EnterPlan::Continue(prefix) => {
                let next = self.doc.split_block(pos);
                let end = self.doc.insert_text(next, &prefix, &Style::default());
                self.sel = Selection::caret(end);
            }
            EnterPlan::Replace { old, new } => self.replace_line_prefix(pos.block, old, &new),
        }
        self.after_edit()
    }

    /// Tab / Shift+Tab.
    pub(crate) fn source_tab(&mut self, outdent: bool) -> Task<Action> {
        let width = self.tab_width;
        if width == 0 {
            return Task::none();
        }
        self.focused = true;

        let (a, b) = self.sel.range();
        let before = Snapshot {
            doc: self.doc.clone(),
            sel: self.sel,
        };
        let mut changed = false;

        if a == b && !outdent {
            let lines = self.source_lines();
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            match plan_tab(&refs, a.block, false) {
                TabPlan::Replace(prefix) => {
                    let old = parse_line(&lines[a.block]).len;
                    self.replace_line_prefix(a.block, old, &prefix);
                    changed = true;
                }
                TabPlan::Nothing => {}
                TabPlan::NotList => {
                    let count = width - a.offset % width;
                    let task = self.insert_text(&" ".repeat(count), EditKind::Typing);
                    self.last_edit = EditKind::None;
                    return task;
                }
            }
        } else {
            let (first, last) = self.source_line_range();
            for index in first..=last {
                let lines = self.source_lines();
                let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
                let current = parse_line(&lines[index]);
                let quote = current.quote.chars().count();
                match plan_tab(&refs, index, outdent) {
                    TabPlan::Replace(prefix) => {
                        self.replace_line_prefix(index, current.len, &prefix);
                        changed = true;
                    }
                    TabPlan::Nothing => {}
                    TabPlan::NotList if outdent => {
                        let spaces = current
                            .indent
                            .chars()
                            .take(width)
                            .take_while(|c| *c == ' ')
                            .count();
                        if spaces > 0 {
                            self.replace_line_prefix(index, quote + spaces, &current.quote);
                            changed = true;
                        }
                    }
                    TabPlan::NotList => {
                        let new = format!("{}{}", current.quote, " ".repeat(width));
                        self.replace_line_prefix(index, quote, &new);
                        changed = true;
                    }
                }
            }
        }

        if !changed {
            return Task::none();
        }
        self.undo.push(before);
        self.redo.clear();
        self.last_edit = EditKind::None;
        self.after_edit()
    }

    fn source_wrap(&mut self, delimiter: &str) -> Task<Action> {
        let (a, b) = self.sel.range();
        self.begin_edit(EditKind::Other);
        let style = Style::default();
        if a == b {
            let end = self
                .doc
                .insert_text(a, &format!("{delimiter}{delimiter}"), &style);
            self.sel = Selection::caret(Pos::new(end.block, end.offset - delimiter.len()));
        } else {
            let selected = self.doc.plain_text(a, b);
            let d = delimiter.len();
            let wrapped = selected.len() >= 2 * d
                && selected.starts_with(delimiter)
                && selected.ends_with(delimiter);
            let text = if wrapped {
                selected[d..selected.len() - d].to_string()
            } else {
                format!("{delimiter}{selected}{delimiter}")
            };
            let start = self.doc.delete_range(a, b);
            let end = self.doc.insert_text(start, &text, &style);
            self.sel = Selection {
                anchor: start,
                head: end,
            };
        }
        self.after_edit()
    }

    fn source_link(&mut self) -> Task<Action> {
        let (a, b) = self.sel.range();
        self.begin_edit(EditKind::Other);
        let style = Style::default();
        if a == b {
            self.doc.insert_text(a, "[](https://)", &style);
            self.sel = Selection::caret(Pos::new(a.block, a.offset + 1));
        } else {
            let selected = self.doc.plain_text(a, b);
            let start = self.doc.delete_range(a, b);
            let end = self
                .doc
                .insert_text(start, &format!("[{selected}](https://)"), &style);
            self.sel = Selection::caret(end);
        }
        self.after_edit()
    }

    fn source_prefix(&mut self, change: impl Fn(&mut LinePrefix)) -> Task<Action> {
        let (first, last) = self.source_line_range();
        let lines = self.source_lines();
        let edits: Vec<(usize, usize, String)> = (first..=last)
            .filter_map(|index| {
                let current = parse_line(&lines[index]);
                let mut next = current.clone();
                change(&mut next);
                let new = next.compose();
                (new != current.compose()).then_some((index, current.len, new))
            })
            .collect();
        self.apply_prefix_edits(edits)
    }

    fn apply_prefix_edits(&mut self, edits: Vec<(usize, usize, String)>) -> Task<Action> {
        if edits.is_empty() {
            return Task::none();
        }
        self.begin_edit(EditKind::Other);
        for (index, old, new) in edits {
            self.replace_line_prefix(index, old, &new);
        }
        self.after_edit()
    }

    fn source_all(&self, test: impl Fn(&LinePrefix) -> bool) -> bool {
        let (first, last) = self.source_line_range();
        let lines = self.source_lines();
        (first..=last).all(|index| test(&parse_line(&lines[index])))
    }

    fn source_block(&mut self, kind: BlockKind) -> Task<Action> {
        match kind {
            BlockKind::Heading(level) => {
                let all = self.source_all(|p| p.heading == Some(level));
                self.source_prefix(|p| p.heading = (!all).then_some(level.clamp(1, 6)))
            }
            BlockKind::Paragraph => self.source_prefix(|p| p.heading = None),
            BlockKind::Bullet => {
                let all = self.source_all(|p| {
                    matches!(p.marker, Some(Marker::Bullet(_))) && p.task.is_none()
                });
                self.source_prefix(|p| {
                    p.marker = if all {
                        None
                    } else {
                        Some(match p.marker {
                            Some(Marker::Bullet(c)) => Marker::Bullet(c),
                            _ => Marker::Bullet('-'),
                        })
                    };
                    p.task = None;
                })
            }
            BlockKind::Ordered => {
                let all = self.source_all(|p| {
                    matches!(p.marker, Some(Marker::Ordered { .. })) && p.task.is_none()
                });
                let (first, last) = self.source_line_range();
                let lines = self.source_lines();
                let mut number = 1;
                let mut edits = Vec::new();
                for index in first..=last {
                    let current = parse_line(&lines[index]);
                    let mut next = current.clone();
                    if all {
                        next.marker = None;
                    } else {
                        let delim = match current.marker {
                            Some(Marker::Ordered { delim, .. }) => delim,
                            _ => '.',
                        };
                        next.marker = Some(Marker::Ordered { number, delim });
                        number += 1;
                    }
                    next.task = None;
                    let new = next.compose();
                    if new != current.compose() {
                        edits.push((index, current.len, new));
                    }
                }
                self.apply_prefix_edits(edits)
            }
            BlockKind::Task => {
                let all = self.source_all(|p| p.task.is_some());
                self.source_prefix(|p| {
                    if all {
                        p.marker = None;
                        p.task = None;
                    } else {
                        p.marker = Some(p.marker.unwrap_or(Marker::Bullet('-')));
                        p.task = Some(p.task.unwrap_or(false));
                    }
                })
            }
            BlockKind::Quote => {
                let all = self.source_all(|p| !p.quote.is_empty());
                self.source_prefix(|p| {
                    p.quote = if all {
                        strip_last_quote(&p.quote)
                    } else {
                        format!("{}> ", p.quote)
                    };
                })
            }
            BlockKind::Code => self.source_fence(),
            BlockKind::Rule => self.source_rule(),
            BlockKind::Html => Task::none(),
        }
    }

    fn source_fence(&mut self) -> Task<Action> {
        let (first, last) = self.source_line_range();
        self.begin_edit(EditKind::Other);
        let style = Style::default();
        let end = Pos::new(last, self.block_len(last));
        self.doc.insert_text(end, "\n```", &style);
        self.doc.insert_text(Pos::new(first, 0), "```\n", &style);
        let offset = self.sel.head.offset;
        self.sel = Selection::caret(Pos::new(self.sel.head.block + 1, offset));
        self.after_edit()
    }

    fn source_rule(&mut self) -> Task<Action> {
        let (_, last) = self.source_line_range();
        self.begin_edit(EditKind::Other);
        let end = Pos::new(last, self.block_len(last));
        let caret = self.doc.insert_text(end, "\n\n___\n", &Style::default());
        self.sel = Selection::caret(caret);
        self.after_edit()
    }

    /// Inserts an empty table after the current line(s).
    fn source_table(&mut self, rows: usize, columns: usize) -> Task<Action> {
        let mut table = Document::default();
        table.insert_table(Pos::new(0, 0), rows, columns);
        let text = markdown::serialize(&table);
        let text = text.trim_end();
        let (_, last) = self.source_line_range();
        self.begin_edit(EditKind::Other);
        let end = Pos::new(last, self.block_len(last));
        let start = self.doc.insert_text(end, "\n\n", &Style::default());
        self.doc.insert_text(start, text, &Style::default());
        // Caret in the first header cell.
        self.sel = Selection::caret(Pos::new(start.block, 2));
        self.after_edit()
    }

    /// Runs a command in source mode. `None` when the command works the
    /// same in both modes.
    pub(crate) fn source_command(&mut self, command: &Command) -> Option<Task<Action>> {
        let task = match command {
            Command::ToggleBold => self.source_wrap("**"),
            Command::ToggleItalic => self.source_wrap("*"),
            Command::ToggleStrike => self.source_wrap("~~"),
            Command::ToggleCode => self.source_wrap("`"),
            Command::EditLink => self.source_link(),
            Command::SetBlock(kind) => self.source_block(*kind),
            Command::ToggleChecked => {
                let all = self.source_all(|p| p.task == Some(true));
                self.source_prefix(|p| {
                    if p.task.is_some() {
                        p.task = Some(!all);
                    }
                })
            }
            Command::InsertRule => self.source_rule(),
            Command::InsertTable { rows, columns } => self.source_table(*rows, *columns),
            Command::InsertRow { .. }
            | Command::InsertColumn { .. }
            | Command::DeleteRow
            | Command::DeleteColumn
            | Command::DeleteTable
            | Command::SetColumnAlign(_) => Task::none(),
            Command::Indent => self.source_tab(false),
            Command::Outdent => self.source_tab(true),
            Command::SetCodeLanguage(_) => Task::none(),
            _ => return None,
        };
        Some(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_round_trip() {
        for text in ["", "a", "# T\n\n- [ ] x\n", "a\r\nb"] {
            let back = super::text(&document(text));
            assert_eq!(back, text.replace('\r', ""));
        }
    }

    #[test]
    fn parses_prefixes() {
        let p = parse_line("- [ ] asdf");
        assert_eq!(p.marker, Some(Marker::Bullet('-')));
        assert_eq!(p.task, Some(false));
        assert_eq!(p.len, 6);
        assert_eq!(p.rest, "asdf");
        assert_eq!(p.compose(), "- [ ] ");

        let p = parse_line("> > 12) [X] text");
        assert_eq!(p.quote, "> > ");
        assert_eq!(
            p.marker,
            Some(Marker::Ordered {
                number: 12,
                delim: ')'
            })
        );
        assert_eq!(p.task, Some(true));
        assert_eq!(p.rest, "text");

        let p = parse_line("## Title");
        assert_eq!(p.heading, Some(2));
        assert_eq!(p.rest, "Title");

        assert_eq!(parse_line("---").marker, None);
        assert_eq!(parse_line("plain").len, 0);
    }

    #[test]
    fn enter_continues_lists_and_quotes() {
        assert_eq!(
            plan_enter(&["- [x] a"], 0, 7),
            EnterPlan::Continue("- [ ] ".into())
        );
        assert_eq!(
            plan_enter(&["9. a"], 0, 4),
            EnterPlan::Continue("10. ".into())
        );
        assert_eq!(
            plan_enter(&["  * b"], 0, 5),
            EnterPlan::Continue("  * ".into())
        );
        assert_eq!(
            plan_enter(&["> - a"], 0, 5),
            EnterPlan::Continue("> - ".into())
        );
        assert_eq!(plan_enter(&["> a"], 0, 3), EnterPlan::Continue("> ".into()));
        assert_eq!(plan_enter(&["text"], 0, 4), EnterPlan::Plain);
        assert_eq!(plan_enter(&["# Title"], 0, 7), EnterPlan::Plain);
        assert_eq!(plan_enter(&["- a"], 0, 0), EnterPlan::Plain);
    }

    #[test]
    fn enter_on_empty_item_leaves_one_level() {
        assert_eq!(
            plan_enter(&["- "], 0, 2),
            EnterPlan::Replace {
                old: 2,
                new: String::new()
            }
        );
        assert_eq!(
            plan_enter(&["- a", "  - "], 1, 4),
            EnterPlan::Replace {
                old: 4,
                new: "- ".into()
            }
        );
        assert_eq!(
            plan_enter(&["> "], 0, 2),
            EnterPlan::Replace {
                old: 2,
                new: String::new()
            }
        );
    }

    #[test]
    fn tab_nests_and_shift_tab_unnests() {
        assert_eq!(
            plan_tab(&["- a", "- b"], 1, false),
            TabPlan::Replace("  - ".into())
        );
        assert_eq!(
            plan_tab(&["1. a", "2. b"], 1, false),
            TabPlan::Replace("   1. ".into())
        );
        assert_eq!(
            plan_tab(&["- [ ] a", "- [x] b"], 1, false),
            TabPlan::Replace("  - [x] ".into())
        );
        assert_eq!(plan_tab(&["- a"], 0, false), TabPlan::Nothing);
        assert_eq!(
            plan_tab(&["- a", "  - b"], 1, true),
            TabPlan::Replace("- ".into())
        );
        assert_eq!(
            plan_tab(&["1. a", "   1. b"], 1, true),
            TabPlan::Replace("2. ".into())
        );
        assert_eq!(plan_tab(&["- a"], 0, true), TabPlan::Replace(String::new()));
        assert_eq!(plan_tab(&["text"], 0, false), TabPlan::NotList);
    }

    #[test]
    fn long_data_urls_are_shortened() {
        let url = format!("data:image/png;base64,{}", "A".repeat(500));
        let text = format!("x {url} y {url}");
        let mut embedded = Embedded::default();
        let short = embedded.shorten(&text);
        assert_eq!(
            short,
            "x data:image/png;embedded,1 y data:image/png;embedded,1"
        );
        assert_eq!(embedded.expand(&short), text);
        // Short URLs are left alone.
        assert_eq!(
            embedded.shorten("data:image/png;base64,AA"),
            "data:image/png;base64,AA"
        );
    }
}
