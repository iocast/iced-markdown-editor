//! Editing behaviour: keyboard, mouse, commands, lists, task lists, quotes,
//! images, scrolling, zoom, history.
//!
//! * `list`: lists, task lists and block quotes.
//! * `table`: tables.
//! * `image`: images.

mod image;
mod list;
mod table;
#[cfg(test)]
mod tests;

use super::{Action, Command, EditKind, EditorState, Event, Inner, Mode, Snapshot};
use crate::doc::{
    Block, BlockKind, Container, Document, Flag, Leaf, Link, ListInfo, Pos, Selection, Style,
};
use crate::images::{self, ImageSource};
use iced::Task;
use iced::keyboard::{Key, Modifiers, key::Named};
use iced::mouse::ScrollDelta;
use list::{item_depth, nest_item, quote_depth, unnest_item, wrap_list, wrap_quote};
use std::sync::Arc;
use table::table_header;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Motion {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Built-in Ctrl/Cmd shortcuts. Any other Ctrl/Cmd combination is left
/// to the application (e.g. Ctrl+S, Ctrl+N).
pub(crate) fn builtin_command(key: &str, shift: bool) -> Option<Command> {
    let command = match (key, shift) {
        ("a", false) => Command::SelectAll,
        ("b", false) => Command::ToggleBold,
        ("i", false) => Command::ToggleItalic,
        ("d", false) => Command::ToggleStrike,
        ("e", false) => Command::ToggleCode,
        ("k", false) => Command::EditLink,
        ("z", true) => Command::Redo,
        ("z", false) => Command::Undo,
        ("y", false) => Command::Redo,
        ("c", false) => Command::Copy,
        ("x", false) => Command::Cut,
        ("v", false) => Command::Paste,
        ("n", true) => Command::SetBlock(BlockKind::Paragraph),
        ("1", false) => Command::SetBlock(BlockKind::Heading(1)),
        ("2", false) => Command::SetBlock(BlockKind::Heading(2)),
        ("3", false) => Command::SetBlock(BlockKind::Heading(3)),
        ("u", false) => Command::SetBlock(BlockKind::Bullet),
        ("l", false) => Command::SetBlock(BlockKind::Ordered),
        ("l", true) => Command::SetBlock(BlockKind::Task),
        ("q", true) => Command::SetBlock(BlockKind::Quote),
        _ => return None,
    };
    Some(command)
}

impl EditorState {
    pub(crate) fn apply(&mut self, inner: Inner) -> Task<Action> {
        match inner {
            Inner::Key(key, modifiers, text) => return self.on_key(key, modifiers, text),
            Inner::Modifiers(modifiers) => self.modifiers = modifiers,
            Inner::Press { x, y, clicks } => return self.on_press(x, y, clicks),
            Inner::Drag { x, y } => {
                let pos = self.layout.borrow().hit(x, y);
                self.sel.head = pos;
                self.touch();
            }
            Inner::PressOutside => self.focused = false,
            Inner::Blink => {
                if self.last_activity.elapsed() < super::BLINK {
                    self.caret_on = true;
                } else {
                    self.caret_on = !self.caret_on;
                }
            }
            Inner::ScrollBy { dx, dy } => {
                let (x, y) = (self.scroll_x + dx, self.scroll_y + dy);
                self.set_scroll(x, y);
            }
            Inner::ScrollTo { x, y } => {
                let x = x.unwrap_or(self.scroll_x);
                let y = y.unwrap_or(self.scroll_y);
                self.set_scroll(x, y);
            }
            Inner::Wheel(delta) => {
                if self.zoom_settings.enabled && self.zoom_settings.mouse_wheel {
                    return match delta {
                        ScrollDelta::Lines { y, .. } if y > 0.0 => {
                            let next = self.zoom_settings.step(self.zoom, 1);
                            self.zoom_to(next)
                        }
                        ScrollDelta::Lines { y, .. } if y < 0.0 => {
                            let next = self.zoom_settings.step(self.zoom, -1);
                            self.zoom_to(next)
                        }
                        ScrollDelta::Pixels { y, .. } => {
                            self.zoom_to(self.zoom * (y * 0.0025).exp())
                        }
                        _ => Task::none(),
                    };
                }
            }
            Inner::Command(command) => return self.command(command),
            Inner::LinkUrl(url) => self.link_url = Some(url),
            Inner::LinkApply => return self.link_apply(false),
            Inner::LinkRemove => return self.link_apply(true),
            Inner::LinkCancel => {
                self.link_url = None;
                self.link_range = None;
                self.focused = true;
            }
            Inner::Paste(Some(text)) => {
                if self.link_url.is_none() {
                    let text = if self.mode == Mode::Source {
                        self.embedded.shorten(&text)
                    } else {
                        text
                    };
                    return self.insert_text(&text, EditKind::Other);
                }
            }
            Inner::Paste(None) => {}
            Inner::FileDropped(path) => {
                let settings = &self.image_settings;
                if settings.enabled() && settings.drop && images::is_image_path(&path) {
                    return self.read_image_file(&path, ImageSource::Drop);
                }
            }
        }
        Task::none()
    }

    fn command(&mut self, command: Command) -> Task<Action> {
        if !self.features.allows(&command) {
            return Task::none();
        }
        let keeps_focus = !matches!(
            command,
            Command::Blur
                | Command::SetMode(_)
                | Command::ZoomIn
                | Command::ZoomOut
                | Command::ResetZoom
                | Command::SetZoom(_)
                | Command::FitWidth
                | Command::FitPage
                | Command::ResolveImage { .. }
                | Command::CancelImage { .. }
                | Command::RequestImage
        );
        if keeps_focus {
            self.focused = true;
        }

        let zoom = self.zoom_settings.enabled;

        if self.mode == Mode::Source {
            if let Some(task) = self.source_command(&command) {
                return task;
            }
        }

        match command {
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ToggleBold => self.toggle_flag(Flag::Bold),
            Command::ToggleItalic => self.toggle_flag(Flag::Italic),
            Command::ToggleStrike => self.toggle_flag(Flag::Strike),
            Command::ToggleCode => self.toggle_flag(Flag::Code),
            Command::SetBlock(kind) => self.set_block(kind),
            Command::ToggleChecked => self.toggle_checked(),
            Command::SetCodeLanguage(language) => self.set_code_language(language),
            Command::InsertRule => self.insert_rule(),
            Command::InsertTable { rows, columns } => self.insert_table(rows, columns),
            Command::InsertRow { below } => {
                self.table_edit(|doc, block| doc.insert_row(block, below).map(|b| Pos::new(b, 0)))
            }
            Command::InsertColumn { right } => self
                .table_edit(|doc, block| doc.insert_column(block, right).map(|b| Pos::new(b, 0))),
            Command::DeleteRow => self.table_edit(Document::delete_row),
            Command::DeleteColumn => self.table_edit(Document::delete_column),
            Command::DeleteTable => self.table_edit(Document::delete_table),
            Command::SetColumnAlign(align) => self.set_column_align(align),
            Command::EditLink => self.link_open(),
            Command::InsertText(text) => self.insert_text(&text, EditKind::Other),
            Command::Indent => self.tab(false),
            Command::Outdent => self.tab(true),
            Command::SelectAll => {
                self.sel = Selection {
                    anchor: self.doc.start(),
                    head: self.doc.end(),
                };
                self.touch();
                Task::none()
            }
            Command::Copy => {
                let (a, b) = self.sel.range();
                if a == b {
                    Task::none()
                } else {
                    iced::clipboard::write(self.embedded.expand(&self.doc.plain_text(a, b)))
                }
            }
            Command::Cut => {
                let (a, b) = self.sel.range();
                if a == b {
                    return Task::none();
                }
                let copy = iced::clipboard::write(self.embedded.expand(&self.doc.plain_text(a, b)));
                self.begin_edit(EditKind::Other);
                let pos = self.doc.delete_range(a, b);
                self.sel = Selection::caret(pos);
                let edit = self.after_edit();
                Task::batch([copy, edit])
            }
            Command::Paste => {
                if self.image_settings.enabled() {
                    if let Some(bytes) = images::clipboard_image() {
                        return self.receive_image(bytes, ImageSource::Paste, None);
                    }
                }
                iced::clipboard::read().map(|text| Action(Inner::Paste(text)))
            }
            Command::InsertImage { bytes, name } => {
                self.receive_image(bytes, ImageSource::Insert, name)
            }
            Command::InsertImageFile(path) => self.read_image_file(&path, ImageSource::Insert),
            Command::InsertImageLink { url, alt } => self.insert_image(&url, &alt),
            Command::ResolveImage { id, url, alt } => {
                self.resolve_image(id, Some((url.as_str(), alt.as_str())));
                Task::none()
            }
            Command::CancelImage { id } => {
                self.resolve_image(id, None);
                Task::none()
            }
            Command::RequestImage => {
                self.pending_events.push(Event::ImageRequested);
                Task::none()
            }
            Command::SetMode(mode) => {
                self.set_mode(mode);
                Task::none()
            }
            Command::Focus => {
                self.focused = true;
                self.touch();
                Task::none()
            }
            Command::Blur => {
                self.focused = false;
                Task::none()
            }
            Command::ZoomIn if zoom => {
                let next = self.zoom_settings.step(self.zoom, 1);
                self.zoom_to(next)
            }
            Command::ZoomOut if zoom => {
                let next = self.zoom_settings.step(self.zoom, -1);
                self.zoom_to(next)
            }
            Command::ResetZoom if zoom => self.zoom_to(1.0),
            Command::SetZoom(value) if zoom => self.zoom_to(value),
            Command::FitWidth if zoom => self.fit(false),
            Command::FitPage if zoom => self.fit(true),
            Command::ZoomIn
            | Command::ZoomOut
            | Command::ResetZoom
            | Command::SetZoom(_)
            | Command::FitWidth
            | Command::FitPage => Task::none(),
        }
    }

    /// Zoom so the page width (or the whole page) fits the widget.
    fn fit(&mut self, whole_page: bool) -> Task<Action> {
        if self.paper.is_fill() {
            return Task::none();
        }
        let Some((width, height)) = self.paper.size_px() else {
            return Task::none();
        };
        let viewport = self.viewport.get();
        let gap = self.style.typography.page_gap;
        let by_width = (viewport.width - 2.0 * gap - 16.0) / width;
        let zoom = if whole_page {
            by_width.min((viewport.height - 2.0 * gap) / height)
        } else {
            by_width
        };
        self.zoom_to(zoom)
    }

    // ----------------------------------------------------------------
    // Mouse
    // ----------------------------------------------------------------

    /// A press in document coordinates.
    fn on_press(&mut self, x: f32, y: f32, clicks: u8) -> Task<Action> {
        self.focused = true;

        let task = self.layout.borrow().task_at(x, y);
        if let Some(task) = task {
            let current = self.doc.blocks[task.block].containers[task.depth].task();
            if let Some(checked) = current {
                self.begin_edit(EditKind::Other);
                self.doc.set_task(task.block, task.depth, Some(!checked));
                self.sel = Selection::caret(Pos::new(task.block, 0));
                self.last_edit = EditKind::None;
                return self.after_edit();
            }
        }

        let pos = self.layout.borrow().hit(x, y);

        if clicks == 1 && self.modifiers.command() {
            let block = &self.doc.blocks[pos.block];
            let url = block
                .styles
                .get(pos.offset)
                .and_then(|style| style.link.clone())
                .or_else(|| {
                    pos.offset
                        .checked_sub(1)
                        .and_then(|i| block.styles.get(i))
                        .and_then(|style| style.link.clone())
                });
            if let Some(link) = url {
                self.pending_events
                    .push(Event::LinkClicked(link.url.clone()));
            }
        }

        match clicks {
            1 => {
                if self.modifiers.shift() {
                    self.sel.head = pos;
                } else {
                    self.sel = Selection::caret(pos);
                }
            }
            2 => {
                let (a, b) = self.doc.word_at(pos);
                self.sel = Selection { anchor: a, head: b };
            }
            _ => {
                let len = self.block_len(pos.block);
                self.sel = Selection {
                    anchor: Pos::new(pos.block, 0),
                    head: Pos::new(pos.block, len),
                };
            }
        }
        self.pending = None;
        self.goal_x = None;
        self.last_edit = EditKind::None;
        self.touch();
        Task::none()
    }

    // ----------------------------------------------------------------
    // Keyboard
    // ----------------------------------------------------------------

    fn on_key(&mut self, key: Key, modifiers: Modifiers, text: Option<String>) -> Task<Action> {
        if self.link_url.is_some() || !self.focused {
            return Task::none();
        }

        let command = modifiers.command() && !modifiers.alt();
        let shift = modifiers.shift();

        match key.as_ref() {
            Key::Named(Named::ArrowLeft) => self.move_caret(Motion::Left, shift, command),
            Key::Named(Named::ArrowRight) => self.move_caret(Motion::Right, shift, command),
            Key::Named(Named::ArrowUp) => self.move_caret(Motion::Up, shift, command),
            Key::Named(Named::ArrowDown) => self.move_caret(Motion::Down, shift, command),
            Key::Named(Named::Home) => self.move_caret(Motion::Home, shift, command),
            Key::Named(Named::End) => self.move_caret(Motion::End, shift, command),
            Key::Named(Named::PageUp) => self.move_caret(Motion::PageUp, shift, command),
            Key::Named(Named::PageDown) => self.move_caret(Motion::PageDown, shift, command),
            Key::Named(Named::Backspace) => return self.backspace(command),
            Key::Named(Named::Delete) => return self.delete_forward(command),
            Key::Named(Named::Enter) if command => {
                return self.command(Command::ToggleChecked);
            }
            Key::Named(Named::Enter) if self.mode == Mode::Source => {
                return self.source_enter(shift);
            }
            Key::Named(Named::Enter) => return self.enter(shift),
            Key::Named(Named::Tab) if !command => return self.tab(shift),
            Key::Named(Named::Escape) => {
                let head = self.sel.head;
                self.sel = Selection::caret(head);
            }
            Key::Character(c) if command => {
                let c = c.to_lowercase();
                return match builtin_command(&c, shift) {
                    Some(command) => self.command(command),
                    None => Task::none(),
                };
            }
            _ => {
                if let Some(text) = text {
                    if !command && !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                        return self.insert_text(&text, EditKind::Typing);
                    }
                }
                return Task::none();
            }
        }

        self.scroll_to_caret()
    }

    fn move_caret(&mut self, motion: Motion, extend: bool, by_word: bool) {
        let head = self.sel.head;
        let (a, b) = self.sel.range();
        let collapsed = a == b;
        let mut vertical = false;

        let target = match motion {
            Motion::Left => {
                if !extend && !collapsed {
                    a
                } else if by_word {
                    self.doc.word_left(head)
                } else {
                    self.doc.prev_pos(head)
                }
            }
            Motion::Right => {
                if !extend && !collapsed {
                    b
                } else if by_word {
                    self.doc.word_right(head)
                } else {
                    self.doc.next_pos(head)
                }
            }
            Motion::Up | Motion::Down | Motion::PageUp | Motion::PageDown => {
                vertical = true;
                let caret_x = self.layout.borrow().caret(head).map(|caret| caret.0);
                let goal = self.goal_x.or(caret_x).unwrap_or(0.0);
                self.goal_x = Some(goal);
                let line_h = (self.style.typography.body_line_height * self.zoom).max(1.0);
                let page = ((self.viewport.get().height / line_h) as i32 - 2).max(1);
                let delta = match motion {
                    Motion::Up => -1,
                    Motion::Down => 1,
                    Motion::PageUp => -page,
                    _ => page,
                };
                let current = self.layout.borrow().line_for(head).unwrap_or(0) as i32;
                let last = self.layout.borrow().lines.len() as i32 - 1;
                let line = (current + delta).clamp(0, last.max(0));
                if line == current {
                    if delta < 0 {
                        self.doc.start()
                    } else {
                        self.doc.end()
                    }
                } else {
                    let moved = self.layout.borrow().vertical(head, goal, line - current);
                    moved.unwrap_or(head)
                }
            }
            Motion::Home => {
                if by_word {
                    self.doc.start()
                } else {
                    let start = self.layout.borrow().line_start(head);
                    start
                }
            }
            Motion::End => {
                if by_word {
                    self.doc.end()
                } else {
                    let end = self.layout.borrow().line_end(head);
                    end
                }
            }
        };

        if !vertical {
            self.goal_x = None;
        }
        if extend {
            self.sel.head = target;
        } else {
            self.sel = Selection::caret(target);
        }
        self.pending = None;
        self.last_edit = EditKind::None;
        self.touch();
    }

    // ----------------------------------------------------------------
    // Text editing
    // ----------------------------------------------------------------

    pub(crate) fn insert_text(&mut self, text: &str, kind: EditKind) -> Task<Action> {
        let expanded;
        let text = if text.contains('\t') {
            expanded = text.replace('\t', &" ".repeat(self.expand_width()));
            expanded.as_str()
        } else {
            text
        };

        self.begin_edit(kind);
        let style = self.typing_style();
        let (a, b) = self.sel.range();
        let pos = self.doc.delete_range(a, b);
        let pos = self.doc.insert_text(pos, text, &style);
        self.sel = Selection::caret(pos);
        self.pending = None;

        if text == " " {
            if self.mode == Mode::Wysiwyg {
                self.autoformat();
            }
            self.last_edit = EditKind::None;
        }

        self.after_edit()
    }

    /// Word-style autoformat at the start of a paragraph.
    fn autoformat(&mut self) {
        let pos = self.sel.head;
        let block = &self.doc.blocks[pos.block];
        if block.leaf != Leaf::Paragraph {
            return;
        }
        let prefix: String = block.chars[..pos.offset].iter().collect();

        enum Format {
            Heading(u8),
            List(bool, u64),
            Quote,
            Task(bool),
        }

        let format = match prefix.as_str() {
            "# " => Format::Heading(1),
            "## " => Format::Heading(2),
            "### " => Format::Heading(3),
            "- " | "* " | "+ " => Format::List(false, 1),
            "> " => Format::Quote,
            "[ ] " | "[] " if self.features.task_lists => Format::Task(false),
            "[x] " | "[X] " if self.features.task_lists => Format::Task(true),
            other => {
                let number = other
                    .strip_suffix(". ")
                    .or_else(|| other.strip_suffix(") "))
                    .filter(|digits| !digits.is_empty() && digits.len() <= 9)
                    .and_then(|digits| digits.parse::<u64>().ok());
                match number {
                    Some(start) => Format::List(true, start),
                    None => return,
                }
            }
        };

        if let Format::Task(_) = format {
            let Some(depth) = item_depth(block) else {
                return;
            };
            if self.doc.container_start(pos.block, depth) != pos.block
                || block.containers[depth].task().is_some()
            {
                return;
            }
        }

        let block = &mut self.doc.blocks[pos.block];
        block.chars.drain(..pos.offset);
        block.styles.drain(..pos.offset);
        match format {
            Format::Heading(level) => block.leaf = Leaf::Heading(level),
            Format::List(ordered, start) => {
                wrap_list(&mut self.doc, pos.block, pos.block, ordered, start, None)
            }
            Format::Quote => wrap_quote(&mut self.doc, pos.block, pos.block),
            Format::Task(checked) => {
                let depth = self.doc.blocks[pos.block].containers.len() - 1;
                self.doc.set_task(pos.block, depth, Some(checked));
            }
        }
        self.sel = Selection::caret(Pos::new(pos.block, 0));
    }

    /// Tab (indent) and Shift+Tab (outdent).
    fn tab(&mut self, outdent: bool) -> Task<Action> {
        let width = self.tab_width;
        if width == 0 {
            return Task::none();
        }
        if self.mode == Mode::Source {
            return self.source_tab(outdent);
        }
        self.focused = true;
        if self.doc.blocks[self.sel.head.block].cell().is_some() {
            return self.table_tab(outdent);
        }

        let (a, b) = self.sel.range();
        let all_items = (a.block..=b.block).all(|i| item_depth(&self.doc.blocks[i]).is_some());
        let at_item_start = a == b && a.offset == 0;
        if all_items && (outdent || at_item_start || a.block != b.block) {
            let before = Snapshot {
                doc: self.doc.clone(),
                sel: self.sel,
            };
            let mut changed = false;
            let mut index = a.block;
            while index <= b.block {
                let Some(depth) = item_depth(&self.doc.blocks[index]) else {
                    index += 1;
                    continue;
                };
                let start = self.doc.container_start(index, depth);
                let end = self.doc.container_end(start, depth);
                changed |= if outdent {
                    unnest_item(&mut self.doc, index)
                } else {
                    nest_item(&mut self.doc, index)
                };
                index = end.max(index + 1);
            }
            if !changed {
                return Task::none();
            }
            self.undo.push(before);
            self.redo.clear();
            self.last_edit = EditKind::None;
            return self.after_edit();
        }

        if a == b && !outdent {
            let block = &self.doc.blocks[a.block];
            let column = a.offset - block.line_start(a.offset);
            let count = width - column % width;
            let spaces = " ".repeat(count);
            let task = self.insert_text(&spaces, EditKind::Typing);
            self.last_edit = EditKind::None;
            return task;
        }

        let last_block = if b.block > a.block && b.offset == 0 {
            b.block - 1
        } else {
            b.block
        };

        let before = Snapshot {
            doc: self.doc.clone(),
            sel: self.sel,
        };
        let mut anchor = self.sel.anchor;
        let mut head = self.sel.head;
        let mut changed = false;

        for index in a.block..=last_block {
            if self.doc.blocks[index].leaf == Leaf::Rule {
                continue;
            }
            let len = self.doc.blocks[index].len();
            let start = if index == a.block { a.offset } else { 0 };
            let end = if index == b.block { b.offset } else { len };

            let starts = self.doc.blocks[index].line_starts();
            let touched: Vec<usize> = starts
                .iter()
                .enumerate()
                .filter(|(k, line)| {
                    let line_end = starts.get(k + 1).map(|next| next - 1).unwrap_or(len);
                    **line <= end && line_end >= start
                })
                .map(|(_, line)| *line)
                .collect();

            for &line in touched.iter().rev() {
                if outdent {
                    let chars = &self.doc.blocks[index].chars;
                    let mut count = 0;
                    while count < width && line + count < chars.len() && chars[line + count] == ' '
                    {
                        count += 1;
                    }
                    if count == 0 {
                        continue;
                    }
                    let block = &mut self.doc.blocks[index];
                    block.chars.drain(line..line + count);
                    block.styles.drain(line..line + count);
                    for pos in [&mut anchor, &mut head] {
                        if pos.block == index && pos.offset > line {
                            pos.offset -= count.min(pos.offset - line);
                        }
                    }
                } else {
                    self.doc.insert_spaces(index, line, width);
                    for pos in [&mut anchor, &mut head] {
                        if pos.block == index && pos.offset >= line {
                            pos.offset += width;
                        }
                    }
                }
                changed = true;
            }
        }

        if !changed {
            return Task::none();
        }

        self.undo.push(before);
        self.redo.clear();
        self.last_edit = EditKind::None;
        self.sel = Selection { anchor, head };
        self.after_edit()
    }

    fn backspace(&mut self, by_word: bool) -> Task<Action> {
        let (a, b) = self.sel.range();
        if a != b {
            self.begin_edit(EditKind::Delete);
            let pos = self.doc.delete_range(a, b);
            self.sel = Selection::caret(pos);
            return self.after_edit();
        }

        let pos = self.sel.head;

        if pos.offset == 0 {
            let block = &self.doc.blocks[pos.block];

            if block.cell().is_some() {
                return Task::none();
            }
            if block.leaf.is_raw() {
                self.begin_edit(EditKind::Other);
                let block = &mut self.doc.blocks[pos.block];
                block.leaf = Leaf::Paragraph;
                block.info = None;
                return self.after_edit();
            }
            if let Some(depth) = item_depth(block) {
                let is_task = block.containers[depth].task().is_some();
                if self.doc.container_start(pos.block, depth) == pos.block {
                    self.begin_edit(EditKind::Other);
                    if is_task {
                        self.doc.set_task(pos.block, depth, None);
                    } else {
                        unnest_item(&mut self.doc, pos.block);
                    }
                    return self.after_edit();
                }
            }
            if let Some(depth) = quote_depth(block) {
                if self.doc.container_start(pos.block, depth) == pos.block {
                    self.begin_edit(EditKind::Other);
                    self.doc.blocks[pos.block].containers.pop();
                    return self.after_edit();
                }
            }
            if block.leaf == Leaf::Rule && pos.block == 0 {
                self.begin_edit(EditKind::Other);
                self.doc.blocks[0].leaf = Leaf::Paragraph;
                return self.after_edit();
            }
            if pos.block == 0 {
                return Task::none();
            }
            self.begin_edit(EditKind::Other);
            let pos = self.doc.merge_with_previous(pos.block);
            self.sel = Selection::caret(pos);
            return self.after_edit();
        }

        self.begin_edit(EditKind::Delete);
        let from = if by_word {
            self.doc.word_left(pos)
        } else {
            Pos::new(pos.block, pos.offset - 1)
        };
        let from = if from.block == pos.block {
            from
        } else {
            Pos::new(pos.block, 0)
        };
        let pos = self.doc.delete_range(from, pos);
        self.sel = Selection::caret(pos);
        self.after_edit()
    }

    fn delete_forward(&mut self, by_word: bool) -> Task<Action> {
        let (a, b) = self.sel.range();
        if a != b {
            self.begin_edit(EditKind::Delete);
            let pos = self.doc.delete_range(a, b);
            self.sel = Selection::caret(pos);
            return self.after_edit();
        }

        let pos = self.sel.head;
        let len = self.block_len(pos.block);

        if pos.offset >= len {
            if pos.block + 1 >= self.doc.blocks.len() || self.doc.blocks[pos.block].cell().is_some()
            {
                return Task::none();
            }
            self.begin_edit(EditKind::Other);
            let merged = self.doc.merge_with_previous(pos.block + 1);
            self.sel = Selection::caret(merged);
            return self.after_edit();
        }

        self.begin_edit(EditKind::Delete);
        let to = if by_word {
            self.doc.word_right(pos)
        } else {
            Pos::new(pos.block, pos.offset + 1)
        };
        let to = if to.block == pos.block {
            to
        } else {
            Pos::new(pos.block, len)
        };
        let pos = self.doc.delete_range(pos, to);
        self.sel = Selection::caret(pos);
        self.after_edit()
    }

    fn enter(&mut self, line_break: bool) -> Task<Action> {
        let (a, b) = self.sel.range();
        if self.doc.blocks[a.block].cell().is_some() || self.doc.blocks[b.block].cell().is_some() {
            return self.table_enter();
        }
        self.begin_edit(EditKind::Other);
        let style = self.typing_style();
        let (a, b) = self.sel.range();
        let pos = self.doc.delete_range(a, b);

        let (leaf, len, empty, ends_with_newline, text) = {
            let block = &self.doc.blocks[pos.block];
            (
                block.leaf,
                block.len(),
                block.is_empty(),
                block.chars.last() == Some(&'\n'),
                block.text(),
            )
        };

        let fence_info = text
            .strip_prefix("```")
            .filter(|rest| !rest.contains('`'))
            .map(|rest| rest.trim().to_string());

        let next = if leaf == Leaf::Code {
            if pos.offset == len && ends_with_newline && !line_break {
                let block = &mut self.doc.blocks[pos.block];
                block.chars.pop();
                block.styles.pop();
                let containers = block.containers.clone();
                self.doc
                    .blocks
                    .insert(pos.block + 1, Block::new(containers, Leaf::Paragraph));
                Pos::new(pos.block + 1, 0)
            } else {
                let block = &self.doc.blocks[pos.block];
                let line = block.line_start(pos.offset);
                let indent = block.chars[line..pos.offset]
                    .iter()
                    .take_while(|c| **c == ' ')
                    .count();
                let after = self.doc.insert_char(pos, '\n', &Style::default());
                self.doc.insert_spaces(after.block, after.offset, indent);
                Pos::new(after.block, after.offset + indent)
            }
        } else if leaf == Leaf::Html || line_break {
            self.doc.insert_char(pos, '\n', &style)
        } else if leaf == Leaf::Paragraph && pos.offset == len && fence_info.is_some() {
            let block = &mut self.doc.blocks[pos.block];
            block.chars.clear();
            block.styles.clear();
            block.leaf = Leaf::Code;
            block.info = fence_info
                .filter(|info| !info.is_empty())
                .map(|info| Arc::from(info.as_str()));
            Pos::new(pos.block, 0)
        } else if leaf == Leaf::Paragraph
            && pos.offset == len
            && self.features.tables
            && table_header(&text).is_some()
        {
            let headers = table_header(&text).unwrap_or_default();
            let block = &mut self.doc.blocks[pos.block];
            block.chars.clear();
            block.styles.clear();
            let first = self
                .doc
                .insert_table(Pos::new(pos.block, 0), 2, headers.len());
            for (column, header) in headers.iter().enumerate() {
                let cell = Pos::new(first.block + column, 0);
                self.doc.insert_text(cell, header, &Style::default());
            }
            Pos::new(first.block + headers.len(), 0)
        } else if leaf == Leaf::Paragraph && pos.offset == len && text == "---" {
            let block = &mut self.doc.blocks[pos.block];
            block.chars.clear();
            block.styles.clear();
            block.leaf = Leaf::Rule;
            let containers = block.containers.clone();
            self.doc
                .blocks
                .insert(pos.block + 1, Block::new(containers, Leaf::Paragraph));
            Pos::new(pos.block + 1, 0)
        } else if empty
            && leaf == Leaf::Paragraph
            && !self.doc.blocks[pos.block].containers.is_empty()
        {
            let block = &self.doc.blocks[pos.block];
            if item_depth(block).is_some() {
                unnest_item(&mut self.doc, pos.block);
            } else {
                self.doc.blocks[pos.block].containers.pop();
            }
            pos
        } else {
            self.doc.split_block(pos)
        };

        self.sel = Selection::caret(next);
        self.pending = None;
        self.after_edit()
    }

    // ----------------------------------------------------------------
    // Formatting
    // ----------------------------------------------------------------

    fn toggle_flag(&mut self, flag: Flag) -> Task<Action> {
        let (mut a, mut b) = self.sel.range();

        if a == b {
            let (start, end) = self.doc.word_at(a);
            if start < a && a < end {
                a = start;
                b = end;
            } else {
                let mut style = self.typing_style();
                let value = !style.get(flag);
                style.set(flag, value);
                self.pending = Some(style);
                self.touch();
                return Task::none();
            }
        }

        let value = !self.doc.all_have(a, b, flag);
        self.begin_edit(EditKind::Other);
        self.doc.set_flag(a, b, flag, value);
        self.after_edit()
    }

    fn set_block(&mut self, kind: BlockKind) -> Task<Action> {
        let (a, b) = self.sel.range();
        let (first, last) = (a.block, b.block);
        let blocks = &self.doc.blocks[first..=last];
        // Table cells keep their kind; lists and quotes would split a table.
        if blocks.iter().any(|block| block.cell().is_some()) {
            let leaf_kind = matches!(
                kind,
                BlockKind::Paragraph | BlockKind::Heading(_) | BlockKind::Code | BlockKind::Html
            );
            if !leaf_kind || blocks.iter().all(|block| block.cell().is_some()) {
                return Task::none();
            }
        }

        match kind {
            BlockKind::Task => {
                let items: Vec<Option<(usize, Option<bool>)>> = blocks
                    .iter()
                    .map(|block| item_depth(block).map(|d| (d, block.containers[d].task())))
                    .collect();
                self.begin_edit(EditKind::Other);
                if items
                    .iter()
                    .all(|item| item.is_some_and(|(_, task)| task.is_some()))
                {
                    for block in &mut self.doc.blocks[first..=last] {
                        block.containers.pop();
                    }
                } else if items.iter().all(Option::is_some) {
                    for (offset, item) in items.into_iter().enumerate() {
                        if let Some((depth, None)) = item {
                            self.doc.set_task(first + offset, depth, Some(false));
                        }
                    }
                } else {
                    wrap_list(&mut self.doc, first, last, false, 1, Some(false));
                }
            }
            BlockKind::Bullet | BlockKind::Ordered => {
                let ordered = kind == BlockKind::Ordered;
                let innermost: Vec<Option<(ListInfo, Option<bool>)>> = blocks
                    .iter()
                    .map(|block| {
                        item_depth(block).and_then(|d| {
                            let container = block.containers[d];
                            container.list().map(|list| (list, container.task()))
                        })
                    })
                    .collect();
                self.begin_edit(EditKind::Other);
                let same_type = innermost.iter().all(|item| {
                    item.is_some_and(|(list, task)| list.ordered == ordered && task.is_none())
                });
                if same_type {
                    for block in &mut self.doc.blocks[first..=last] {
                        block.containers.pop();
                    }
                } else if innermost.iter().all(Option::is_some) {
                    let ids: Vec<u64> = innermost.iter().flatten().map(|(l, _)| l.id).collect();
                    for block in &mut self.doc.blocks {
                        for container in &mut block.containers {
                            if let Container::Item { list, .. } = container {
                                if ids.contains(&list.id) {
                                    list.ordered = ordered;
                                }
                            }
                        }
                    }
                    for index in first..=last {
                        if let Some(depth) = item_depth(&self.doc.blocks[index]) {
                            self.doc.set_task(index, depth, None);
                        }
                    }
                } else {
                    wrap_list(&mut self.doc, first, last, ordered, 1, None);
                }
            }
            BlockKind::Quote => {
                let all_quoted = blocks.iter().all(|block| quote_depth(block).is_some());
                self.begin_edit(EditKind::Other);
                if all_quoted {
                    for block in &mut self.doc.blocks[first..=last] {
                        block.containers.pop();
                    }
                } else {
                    wrap_quote(&mut self.doc, first, last);
                }
            }
            BlockKind::Rule => return self.insert_rule(),
            BlockKind::Paragraph | BlockKind::Heading(_) | BlockKind::Code | BlockKind::Html => {
                let leaf = match kind {
                    BlockKind::Heading(level) => Leaf::Heading(level.clamp(1, 6)),
                    BlockKind::Code => Leaf::Code,
                    BlockKind::Html => Leaf::Html,
                    _ => Leaf::Paragraph,
                };
                let all = blocks
                    .iter()
                    .filter(|block| block.leaf != Leaf::Rule && block.cell().is_none())
                    .all(|block| block.leaf == leaf);
                let target = if all && leaf != Leaf::Paragraph {
                    Leaf::Paragraph
                } else {
                    leaf
                };
                self.begin_edit(EditKind::Other);
                for block in &mut self.doc.blocks[first..=last] {
                    if block.leaf == Leaf::Rule || block.cell().is_some() {
                        continue;
                    }
                    block.leaf = target;
                    if target.is_raw() {
                        for style in &mut block.styles {
                            *style = Style::default();
                        }
                    } else {
                        block.info = None;
                    }
                }
            }
        }
        self.after_edit()
    }

    fn set_code_language(&mut self, language: Option<String>) -> Task<Action> {
        let info: Option<Arc<str>> = language
            .map(|language| language.trim().to_string())
            .filter(|language| !language.is_empty() && !language.contains('`'))
            .map(|language| Arc::from(language.as_str()));

        let (a, b) = self.sel.range();
        let targets: Vec<usize> = (a.block..=b.block)
            .filter(|index| {
                let block = &self.doc.blocks[*index];
                block.leaf == Leaf::Code && block.info != info
            })
            .collect();
        if targets.is_empty() {
            return Task::none();
        }

        self.begin_edit(EditKind::Other);
        for index in targets {
            self.doc.blocks[index].info = info.clone();
        }
        self.after_edit()
    }

    fn insert_rule(&mut self) -> Task<Action> {
        let (a, b) = self.sel.range();
        if self.doc.blocks[a.block].cell().is_some() || self.doc.blocks[b.block].cell().is_some() {
            return Task::none();
        }
        self.begin_edit(EditKind::Other);
        let pos = self.doc.delete_range(a, b);
        let containers = self.doc.blocks[pos.block].containers.clone();
        let empty_paragraph = {
            let block = &self.doc.blocks[pos.block];
            block.is_empty() && block.leaf == Leaf::Paragraph
        };

        let rule_index = if empty_paragraph {
            self.doc.blocks[pos.block].leaf = Leaf::Rule;
            pos.block
        } else if pos.offset == 0 {
            self.doc
                .blocks
                .insert(pos.block, Block::new(containers.clone(), Leaf::Rule));
            pos.block
        } else {
            let after = self.doc.split_block(pos);
            self.doc
                .blocks
                .insert(after.block, Block::new(containers.clone(), Leaf::Rule));
            after.block
        };

        if rule_index + 1 >= self.doc.blocks.len() {
            self.doc
                .blocks
                .push(Block::new(containers, Leaf::Paragraph));
        }

        self.sel = Selection::caret(Pos::new(rule_index + 1, 0));
        self.after_edit()
    }

    fn link_open(&mut self) -> Task<Action> {
        let (mut a, mut b) = self.sel.range();
        if a == b {
            let (start, end) = self.doc.word_at(a);
            a = start;
            b = end;
        }
        if a == b {
            return Task::none();
        }

        let current = self
            .doc
            .link_at(a)
            .map(|link| link.url.clone())
            .unwrap_or_else(|| "https://".to_string());

        self.sel = Selection { anchor: a, head: b };
        self.link_range = Some((a, b));
        self.link_url = Some(current);
        Task::none()
    }

    fn link_apply(&mut self, remove: bool) -> Task<Action> {
        let url = self.link_url.take().unwrap_or_default();
        let Some((a, b)) = self.link_range.take() else {
            return Task::none();
        };
        self.focused = true;
        let a = self.doc.clamp(a);
        let b = self.doc.clamp(b);
        let url = url.trim().to_string();
        let title = self.doc.link_at(a).and_then(|link| link.title.clone());
        let link: Option<Arc<Link>> = if remove || url.is_empty() || url == "https://" {
            None
        } else {
            Some(Link::new(url, title))
        };

        self.begin_edit(EditKind::Other);
        self.doc.set_link(a, b, link);
        self.sel = Selection::caret(b);
        self.after_edit()
    }

    // ----------------------------------------------------------------
    // History
    // ----------------------------------------------------------------

    fn undo(&mut self) -> Task<Action> {
        if let Some(snapshot) = self.undo.pop() {
            self.redo.push(Snapshot {
                doc: self.doc.clone(),
                sel: self.sel,
            });
            self.restore(snapshot);
        }
        self.scroll_to_caret()
    }

    fn redo(&mut self) -> Task<Action> {
        if let Some(snapshot) = self.redo.pop() {
            self.undo.push(Snapshot {
                doc: self.doc.clone(),
                sel: self.sel,
            });
            self.restore(snapshot);
        }
        self.scroll_to_caret()
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.doc = snapshot.doc;
        self.sel = Selection {
            anchor: self.doc.clamp(snapshot.sel.anchor),
            head: self.doc.clamp(snapshot.sel.head),
        };
        self.last_edit = EditKind::None;
        self.pending = None;
        self.dirty = true;
        self.revision += 1;
        self.relayout();
        self.touch();
    }
}
