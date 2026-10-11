//! Import: CommonMark events from `pulldown-cmark` into a document.

use super::{flag_bit, flag_tag, html_flag, is_break, options};
use crate::doc::{
    Block, Cell, ColumnAlign, Container, Document, Flag, Leaf, Link, ListInfo, OBJ, SOFT, Style,
    next_id,
};
use crate::features::Features;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Parser, Tag};
use std::sync::Arc;

enum Ctx {
    Paragraph,
    Heading,
    Quote,
    List,
    Item,
    Code,
    Html,
    Emphasis,
    Strong,
    Strike,
    Link,
    Image,
    Table,
    TableCell,
    Other,
}

/// The table being read.
struct TableState {
    id: u64,
    aligns: Vec<ColumnAlign>,
    row: u64,
    header: bool,
    column: usize,
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    current: Option<Block>,
    stack: Vec<Ctx>,
    containers: Vec<Container>,
    lists: Vec<(ListInfo, usize)>,
    starts: Vec<usize>,
    bold: u32,
    italic: u32,
    strike: u32,
    /// Formatting from inline HTML tags: (flag, start index in the block).
    html_open: Vec<(Flag, usize)>,
    /// Per character of the current block: flags provided by HTML tags.
    html_bits: Vec<u8>,
    /// Open links: (target, start index).
    links: Vec<(Arc<Link>, usize)>,
    /// Open image: (target, start index).
    image: Option<(Arc<Link>, usize)>,
    table: Option<TableState>,
    features: Features,
}

impl Builder {
    fn html_has(&self, flag: Flag) -> bool {
        self.html_open.iter().any(|(open, _)| *open == flag)
    }

    fn style(&self, code: bool) -> Style {
        Style {
            bold: self.bold > 0 || self.html_has(Flag::Bold),
            italic: self.italic > 0 || self.html_has(Flag::Italic),
            strike: self.strike > 0 || self.html_has(Flag::Strike),
            code,
            link: self.links.last().map(|(link, _)| link.clone()),
            image: self.image.as_ref().map(|(link, _)| link.clone()),
            html: false,
        }
    }

    fn len(&self) -> usize {
        self.current.as_ref().map(Block::len).unwrap_or(0)
    }

    /// The leaf block receiving inline content. Tight list items have no
    /// paragraph events, so a paragraph is created implicitly.
    fn ensure(&mut self) -> &mut Block {
        if self.current.is_none() {
            self.current = Some(Block::new(self.containers.clone(), Leaf::Paragraph));
            self.html_bits.clear();
        }
        self.current.as_mut().expect("current block")
    }

    fn push_styled(&mut self, text: &str, style: Style) {
        let bits = self
            .html_open
            .iter()
            .fold(0u8, |bits, (flag, _)| bits | flag_bit(*flag));
        let block = self.ensure();
        let mut count = 0;
        for c in text.chars() {
            block.push(c, style.clone());
            count += 1;
        }
        self.html_bits.extend(std::iter::repeat(bits).take(count));
    }

    fn push_text(&mut self, text: &str, code: bool) {
        if let Some(block) = self.current.as_mut() {
            if block.leaf.is_raw() {
                for c in text.chars() {
                    if c != '\r' {
                        block.push(c, Style::default());
                    }
                }
                return;
            }
        }
        let style = self.style(code);
        if self.image.is_some() {
            let text: String = text
                .chars()
                .map(|c| if is_break(c) { ' ' } else { c })
                .collect();
            self.push_styled(&text, style);
        } else {
            let text: String = text
                .chars()
                .map(|c| if c == '\n' { SOFT } else { c })
                .collect();
            self.push_styled(&text, style);
        }
    }

    fn push_break(&mut self, c: char) {
        if self.image.is_some() {
            self.push_styled(" ", self.style(false));
        } else {
            let style = self.style(false);
            self.push_styled(&c.to_string(), style);
        }
    }

    /// `[ ]` / `[x]` at the start of a list item.
    fn task(&mut self, checked: bool) {
        let Some(depth) = self
            .containers
            .iter()
            .rposition(|c| matches!(c, Container::Item { .. }))
        else {
            return;
        };
        if let Container::Item { task, .. } = &mut self.containers[depth] {
            *task = Some(checked);
        }
        if let Some(block) = self.current.as_mut() {
            if let Some(Container::Item { task, .. }) = block.containers.get_mut(depth) {
                *task = Some(checked);
            }
        }
    }

    fn inline_html(&mut self, tag: &str) {
        if self.image.is_some() {
            return;
        }
        // Without strikethrough, `<del>` stays raw HTML.
        let flag =
            html_flag(tag).filter(|(flag, _)| *flag != Flag::Strike || self.features.strikethrough);
        match flag {
            Some((flag, false)) => {
                let start = self.len();
                self.ensure();
                self.html_open.push((flag, start));
            }
            Some((flag, true)) if self.html_has(flag) => {
                if let Some(index) = self.html_open.iter().rposition(|(open, _)| *open == flag) {
                    self.html_open.remove(index);
                }
            }
            _ => {
                let raw: String = tag
                    .chars()
                    .map(|c| if c == '\n' { SOFT } else { c })
                    .collect();
                let style = Style {
                    html: true,
                    link: self.links.last().map(|(link, _)| link.clone()),
                    ..Style::default()
                };
                self.push_styled(&raw, style);
            }
        }
    }

    fn flush(&mut self) {
        let Some(mut block) = self.current.take() else {
            return;
        };

        // Unmatched formatting tags become raw HTML again.
        for (flag, start) in std::mem::take(&mut self.html_open).into_iter().rev() {
            let bit = flag_bit(flag);
            for j in start..block.len() {
                if self.html_bits.get(j).is_some_and(|bits| bits & bit != 0) {
                    self.html_bits[j] &= !bit;
                    block.styles[j].set(flag, false);
                }
            }
            let tag = flag_tag(flag, false);
            let count = tag.chars().count();
            let raw = Style {
                html: true,
                ..Style::default()
            };
            block.chars.splice(start..start, tag.chars());
            block
                .styles
                .splice(start..start, std::iter::repeat(raw).take(count));
            self.html_bits
                .splice(start..start, std::iter::repeat(0).take(count));
        }
        self.html_bits.clear();

        match block.leaf {
            Leaf::Code => {
                if block.chars.last() == Some(&'\n') {
                    block.chars.pop();
                    block.styles.pop();
                }
            }
            Leaf::Html => {
                while block.chars.last() == Some(&'\n') {
                    block.chars.pop();
                    block.styles.pop();
                }
            }
            _ => {}
        }
        self.blocks.push(block);
    }

    fn begin(&mut self, leaf: Leaf) {
        self.flush();
        self.current = Some(Block::new(self.containers.clone(), leaf));
        self.html_bits.clear();
    }

    fn start(&mut self, tag: Tag<'_>) {
        if self.image.is_some() {
            // Inside an image description only the text matters.
            self.stack.push(Ctx::Other);
            return;
        }
        match tag {
            Tag::Paragraph => {
                if matches!(self.containers.last(), Some(Container::Item { .. })) {
                    if let Some((list, _)) = self.lists.last_mut() {
                        list.tight = false;
                    }
                }
                self.begin(Leaf::Paragraph);
                self.stack.push(Ctx::Paragraph);
            }
            Tag::Heading { level, .. } => {
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                self.begin(Leaf::Heading(level));
                self.stack.push(Ctx::Heading);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.containers.push(Container::quote());
                self.starts.push(self.blocks.len());
                self.stack.push(Ctx::Quote);
            }
            Tag::CodeBlock(kind) => {
                self.begin(Leaf::Code);
                if let CodeBlockKind::Fenced(info) = kind {
                    let info = info.trim();
                    if !info.is_empty() {
                        if let Some(block) = self.current.as_mut() {
                            block.info = Some(Arc::from(info));
                        }
                    }
                }
                self.stack.push(Ctx::Code);
            }
            Tag::HtmlBlock => {
                self.begin(Leaf::Html);
                self.stack.push(Ctx::Html);
            }
            Tag::List(start) => {
                self.flush();
                let list = ListInfo::new(start.is_some(), start.unwrap_or(1));
                self.lists.push((list, self.blocks.len()));
                self.stack.push(Ctx::List);
            }
            Tag::Item => {
                self.flush();
                let list = self
                    .lists
                    .last()
                    .map(|(list, _)| *list)
                    .unwrap_or(ListInfo::new(false, 1));
                self.containers.push(Container::item(list));
                self.starts.push(self.blocks.len());
                self.stack.push(Ctx::Item);
            }
            Tag::Emphasis => {
                self.italic += 1;
                self.stack.push(Ctx::Emphasis);
            }
            Tag::Strong => {
                self.bold += 1;
                self.stack.push(Ctx::Strong);
            }
            Tag::Strikethrough => {
                self.strike += 1;
                self.stack.push(Ctx::Strike);
            }
            Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            } => {
                let url = if link_type == LinkType::Email {
                    format!("mailto:{dest_url}")
                } else {
                    dest_url.to_string()
                };
                let start = self.len();
                self.ensure();
                self.links
                    .push((Link::new(url, Some(title.to_string())), start));
                self.stack.push(Ctx::Link);
            }
            Tag::Table(aligns) => {
                self.flush();
                self.table = Some(TableState {
                    id: next_id(),
                    aligns: aligns
                        .iter()
                        .map(|align| match align {
                            Alignment::None => ColumnAlign::None,
                            Alignment::Left => ColumnAlign::Left,
                            Alignment::Center => ColumnAlign::Center,
                            Alignment::Right => ColumnAlign::Right,
                        })
                        .collect(),
                    row: 0,
                    header: false,
                    column: 0,
                });
                self.stack.push(Ctx::Table);
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    table.row = next_id();
                    table.header = matches!(tag, Tag::TableHead);
                    table.column = 0;
                }
                self.stack.push(Ctx::Other);
            }
            Tag::TableCell => {
                let cell = self.table.as_mut().map(|table| {
                    let align = table.aligns.get(table.column).copied().unwrap_or_default();
                    table.column += 1;
                    Cell {
                        table: table.id,
                        row: table.row,
                        header: table.header,
                        align,
                    }
                });
                self.begin(cell.map(Leaf::Cell).unwrap_or(Leaf::Paragraph));
                self.stack.push(Ctx::TableCell);
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                let start = self.len();
                self.ensure();
                self.image = Some((
                    Link::new(dest_url.to_string(), Some(title.to_string())),
                    start,
                ));
                self.stack.push(Ctx::Image);
            }
            _ => self.stack.push(Ctx::Other),
        }
    }

    fn end(&mut self) {
        match self.stack.pop() {
            Some(Ctx::Paragraph) | Some(Ctx::Heading) | Some(Ctx::Code) | Some(Ctx::Html)
            | Some(Ctx::TableCell) => self.flush(),
            Some(Ctx::Table) => {
                self.flush();
                self.table = None;
            }
            Some(Ctx::Quote) | Some(Ctx::Item) => {
                self.flush();
                let start = self.starts.pop().unwrap_or(0);
                if self.blocks.len() == start {
                    // Empty block quote / empty list item.
                    self.blocks
                        .push(Block::new(self.containers.clone(), Leaf::Paragraph));
                }
                self.containers.pop();
            }
            Some(Ctx::List) => {
                self.flush();
                if let Some((list, first)) = self.lists.pop() {
                    if !list.tight {
                        for block in &mut self.blocks[first..] {
                            for container in &mut block.containers {
                                if let Container::Item { list: info, .. } = container {
                                    if info.id == list.id {
                                        info.tight = false;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Ctx::Emphasis) => self.italic = self.italic.saturating_sub(1),
            Some(Ctx::Strong) => self.bold = self.bold.saturating_sub(1),
            Some(Ctx::Strike) => self.strike = self.strike.saturating_sub(1),
            Some(Ctx::Link) => {
                if let Some((link, start)) = self.links.last().cloned() {
                    if self.len() == start {
                        // Empty link text keeps a placeholder.
                        let style = self.style(false);
                        self.push_styled(
                            &OBJ.to_string(),
                            Style {
                                link: Some(link),
                                ..style
                            },
                        );
                    }
                }
                self.links.pop();
            }
            Some(Ctx::Image) => {
                if let Some((_, start)) = self.image.clone() {
                    if self.len() == start {
                        let style = self.style(false);
                        self.push_styled(&OBJ.to_string(), style);
                    }
                }
                self.image = None;
            }
            Some(Ctx::Other) | None => {}
        }
    }
}

/// Parses CommonMark with all GFM extensions (strikethrough, task lists,
/// tables).
pub fn parse(source: &str) -> Document {
    parse_with(source, Features::ALL)
}

/// Parses CommonMark with the GFM extensions enabled in `features`.
pub fn parse_with(source: &str, features: Features) -> Document {
    let mut builder = Builder {
        features,
        ..Builder::default()
    };

    for event in Parser::new_ext(source, options(features)) {
        match event {
            Event::Start(tag) => builder.start(tag),
            Event::End(_) => builder.end(),
            Event::Text(text) => builder.push_text(&text, false),
            Event::Code(text) => {
                if builder.image.is_some() {
                    builder.push_text(&text, false);
                } else {
                    builder.push_text(&text, true);
                }
            }
            Event::Html(text) => {
                if !matches!(builder.current.as_ref().map(|b| b.leaf), Some(Leaf::Html)) {
                    builder.begin(Leaf::Html);
                }
                builder.push_text(&text, false);
            }
            Event::InlineHtml(text) => builder.inline_html(&text),
            Event::SoftBreak => builder.push_break(SOFT),
            Event::HardBreak => builder.push_break('\n'),
            Event::TaskListMarker(checked) => builder.task(checked),
            Event::Rule => {
                builder.flush();
                builder
                    .blocks
                    .push(Block::new(builder.containers.clone(), Leaf::Rule));
            }
            _ => {}
        }
    }

    builder.flush();

    if builder.blocks.is_empty() {
        builder.blocks.push(Block::new(Vec::new(), Leaf::Paragraph));
    }

    Document {
        blocks: builder.blocks,
    }
}
