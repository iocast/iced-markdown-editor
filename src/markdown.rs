//! CommonMark import and export.
//!
//! Import uses `pulldown-cmark`, a CommonMark 0.31.2 parser, with three GFM
//! extensions: `~~strikethrough~~`, task lists (`- [ ]` / `- [x]`) and
//! tables.
//! Export writes Markdown that parses back to the same document; the round
//! trip is tested against all 652 examples of the CommonMark 0.31.2
//! specification and against random documents (see the tests at the end of
//! this file).
//!
//! Inline formatting is written with Markdown delimiters (`**`, `*`, `~~`)
//! when they survive a round trip. CommonMark's delimiter rules cannot
//! express every combination (e.g. bold text that starts with punctuation
//! directly after a letter), so such paragraphs fall back to
//! `<strong>`, `<em>` and `<del>` tags, which the importer recognises.

use crate::doc::{
    Block, Cell, ColumnAlign, Container, Document, Flag, Leaf, Link, ListInfo, OBJ, SOFT, Style,
    next_id,
};
use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag,
};
use std::collections::HashMap;
use std::sync::Arc;

fn options() -> Options {
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES
}

fn is_break(c: char) -> bool {
    c == '\n' || c == SOFT
}

// ====================================================================
// Import
// ====================================================================

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

/// Inline HTML tags that map to formatting (the export fallback).
fn html_flag(tag: &str) -> Option<(Flag, bool)> {
    match tag {
        "<strong>" => Some((Flag::Bold, false)),
        "</strong>" => Some((Flag::Bold, true)),
        "<em>" => Some((Flag::Italic, false)),
        "</em>" => Some((Flag::Italic, true)),
        "<del>" => Some((Flag::Strike, false)),
        "</del>" => Some((Flag::Strike, true)),
        _ => None,
    }
}

fn flag_bit(flag: Flag) -> u8 {
    match flag {
        Flag::Bold => 1,
        Flag::Italic => 2,
        Flag::Strike => 4,
        Flag::Code => 8,
    }
}

fn flag_tag(flag: Flag, closing: bool) -> &'static str {
    match (flag, closing) {
        (Flag::Bold, false) => "<strong>",
        (Flag::Bold, true) => "</strong>",
        (Flag::Italic, false) => "<em>",
        (Flag::Italic, true) => "</em>",
        (Flag::Strike, false) => "<del>",
        (Flag::Strike, true) => "</del>",
        (Flag::Code, false) => "<code>",
        (Flag::Code, true) => "</code>",
    }
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
        match html_flag(tag) {
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

/// Parses CommonMark (with `~~strikethrough~~` and task lists).
pub fn parse(source: &str) -> Document {
    let mut builder = Builder::default();

    for event in Parser::new_ext(source, options()) {
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

// ====================================================================
// Export: inline content
// ====================================================================

const MARKS: [Flag; 3] = [Flag::Bold, Flag::Italic, Flag::Strike];

fn mark_token(flag: Flag, use_html: bool, closing: bool) -> &'static str {
    if use_html {
        flag_tag(flag, closing)
    } else {
        match flag {
            Flag::Bold => "**",
            Flag::Italic => "*",
            Flag::Strike => "~~",
            Flag::Code => "`",
        }
    }
}

fn esc_text(c: char, line_start: bool, heading: bool) -> String {
    if heading && c == '#' {
        return "\\#".into();
    }
    if "\\`*_[]<>~&|!".contains(c) {
        return format!("\\{c}");
    }
    if line_start && "#>+-=".contains(c) {
        return format!("\\{c}");
    }
    c.to_string()
}

fn esc_dest(url: &str) -> String {
    // Plain form when nothing needs escaping, `<...>` otherwise.
    let plain = !url.is_empty()
        && !url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "<>()\\&".contains(c));
    if plain {
        return url.to_string();
    }
    let mut out = String::from("<");
    for c in url.chars() {
        match c {
            '<' | '>' | '\\' | '&' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('>');
    out
}

fn esc_title(title: &str) -> String {
    let mut out = String::from("\"");
    for c in title.chars() {
        if matches!(c, '"' | '\\' | '&') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn code_span(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let ticks = "`".repeat(longest + 1);
    let pad = text.starts_with('`')
        || text.ends_with('`')
        || (text.starts_with(' ') && text.ends_with(' ') && !text.trim_matches(' ').is_empty());
    if pad {
        format!("{ticks} {text} {ticks}")
    } else {
        format!("{ticks}{text}{ticks}")
    }
}

/// `scheme:...` usable as an autolink.
fn is_autolink(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let len_ok = (2..=32).contains(&scheme.len());
    let scheme_ok = scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    let rest_ok = rest
        .chars()
        .all(|c| !c.is_whitespace() && !c.is_control() && c != '<' && c != '>');
    first_ok && len_ok && scheme_ok && rest_ok
}

fn is_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    let local_ok = !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c));
    let domain_ok = !domain.is_empty()
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        });
    local_ok && domain_ok
}

fn image_md(alt: &[char], target: &Link) -> String {
    let mut text = String::new();
    for c in alt {
        match *c {
            OBJ => {}
            SOFT | '\n' => text.push(' '),
            c => text.push_str(&esc_text(c, false, false)),
        }
    }
    let title = target
        .title
        .as_deref()
        .map(|title| format!(" {}", esc_title(title)))
        .unwrap_or_default();
    format!("![{text}]({}{title})", esc_dest(&target.url))
}

/// `![alt](url)` with escaping, for inserting images as Markdown text.
pub(crate) fn image_markdown(alt: &str, url: &str) -> String {
    let alt: Vec<char> = alt.chars().collect();
    image_md(
        &alt,
        &Link {
            url: url.to_string(),
            title: None,
        },
    )
}

struct Inline<'a> {
    chars: &'a [char],
    styles: &'a [Style],
    heading: bool,
    use_html: bool,
    para_start: bool,
    para_end: bool,
    out: String,
    open: Vec<Flag>,
    line_start: bool,
}

impl<'a> Inline<'a> {
    fn has(&self, k: usize, flag: Flag) -> bool {
        self.styles[k].get(flag)
    }

    fn wanted(&self, k: usize) -> Vec<Flag> {
        MARKS
            .into_iter()
            .filter(|flag| self.has(k, *flag))
            .collect()
    }

    fn mark_end(&self, flag: Flag, mut k: usize) -> usize {
        while k < self.chars.len() && self.has(k, flag) {
            k += 1;
        }
        k
    }

    /// Closes every open mark not in `keep` (and everything opened after it).
    fn close_to(&mut self, keep: &[Flag]) {
        while self.open.iter().any(|flag| !keep.contains(flag)) {
            if let Some(flag) = self.open.pop() {
                self.out.push_str(mark_token(flag, self.use_html, true));
            }
        }
    }

    /// Opens `marks` at `k`; the longest-running mark becomes outermost.
    fn open_at(&mut self, k: usize, marks: &[Flag]) {
        let mut new: Vec<(Flag, usize)> = marks
            .iter()
            .filter(|flag| !self.open.contains(flag))
            .map(|flag| (*flag, self.mark_end(*flag, k)))
            .collect();
        new.sort_by(|a, b| b.1.cmp(&a.1));
        for (flag, _) in new {
            self.open.push(flag);
            self.out.push_str(mark_token(flag, self.use_html, false));
        }
    }

    fn emit_text(&mut self, text: &[char], at_end: bool) {
        let mut text: Vec<char> = text.to_vec();
        let mut tail = String::new();
        if at_end {
            while let Some(c) = text.last().copied() {
                if c == ' ' || c == '\t' {
                    tail.insert_str(0, if c == ' ' { "&#32;" } else { "&#9;" });
                    text.pop();
                } else {
                    break;
                }
            }
        }

        let mut k = 0;
        while k < text.len() {
            let c = text[k];
            if c == OBJ {
                k += 1;
                continue;
            }
            if self.line_start && (c == ' ' || c == '\t') {
                self.out.push_str(if c == ' ' { "&#32;" } else { "&#9;" });
                k += 1;
                continue;
            }
            if self.line_start && c.is_ascii_digit() {
                let mut m = k;
                while m < text.len() && text[m].is_ascii_digit() {
                    m += 1;
                }
                if m < text.len() && (text[m] == '.' || text[m] == ')') {
                    self.out.extend(&text[k..m]);
                    self.out.push('\\');
                    self.out.push(text[m]);
                    k = m + 1;
                    self.line_start = false;
                    continue;
                }
            }
            if c == '\u{a0}' {
                self.out.push_str("&nbsp;");
            } else {
                let escaped = esc_text(c, self.line_start, self.heading);
                self.out.push_str(&escaped);
            }
            self.line_start = false;
            k += 1;
        }

        if !tail.is_empty() {
            self.out.push_str(&tail);
            self.line_start = false;
        }
    }

    fn run(mut self) -> String {
        let n = self.chars.len();
        let mut i = 0;

        while i < n {
            let c = self.chars[i];
            let style = self.styles[i].clone();

            if is_break(c) {
                if style.html && c == SOFT {
                    // A line ending inside raw HTML.
                    self.out.push('\n');
                    self.line_start = true;
                    i += 1;
                    continue;
                }
                let trailing = self.chars[i + 1..]
                    .iter()
                    .all(|ch| *ch == '\n' || *ch == OBJ);
                if c == '\n' && ((i == 0 && self.para_start) || (self.para_end && trailing)) {
                    // A hard break at the edge of a paragraph cannot be expressed.
                    i += 1;
                    continue;
                }
                let prev = if i > 0 { Some(self.chars[i - 1]) } else { None };
                let next = self.chars.get(i + 1).copied();
                let edge = (prev.is_none() && self.para_start)
                    || (next.is_none() && self.para_end)
                    || prev.is_some_and(is_break)
                    || next.is_some_and(is_break);
                if c == SOFT && (self.heading || edge) {
                    self.out.push_str("&#10;");
                    self.line_start = false;
                } else if self.heading {
                    self.out.push('\n');
                    self.line_start = true;
                } else {
                    self.out.push_str(if c == '\n' { "\\\n" } else { "\n" });
                    self.line_start = true;
                }
                i += 1;
                continue;
            }

            if let Some(link) = style.link.clone() {
                let mut j = i;
                while j < n && self.styles[j].link.as_ref() == Some(&link) {
                    j += 1;
                }
                // Marks covering the whole link and continuing outside it
                // are written around the link, others inside.
                let after = if j < n { self.wanted(j) } else { Vec::new() };
                let outside: Vec<Flag> = self
                    .wanted(i)
                    .into_iter()
                    .filter(|flag| {
                        (self.open.contains(flag) || after.contains(flag))
                            && (i..j).all(|k| self.has(k, *flag))
                    })
                    .collect();
                self.close_to(&outside);
                self.open_at(i, &outside);

                let text: String = self.chars[i..j].iter().collect();
                let plain_style = Style {
                    bold: outside.contains(&Flag::Bold),
                    italic: outside.contains(&Flag::Italic),
                    strike: outside.contains(&Flag::Strike),
                    link: Some(link.clone()),
                    ..Style::default()
                };
                let plain = self.styles[i..j].iter().all(|st| *st == plain_style);
                if plain && link.title.is_none() && link.url == text && is_autolink(&link.url) {
                    self.out.push_str(&format!("<{}>", link.url));
                } else if plain
                    && link.title.is_none()
                    && link.url == format!("mailto:{text}")
                    && is_email(&text)
                {
                    self.out.push_str(&format!("<{text}>"));
                } else {
                    let inner: Vec<Style> = self.styles[i..j]
                        .iter()
                        .map(|st| {
                            let mut st = st.clone();
                            st.link = None;
                            for flag in &outside {
                                st.set(*flag, false);
                            }
                            st
                        })
                        .collect();
                    let body = Inline {
                        chars: &self.chars[i..j],
                        styles: &inner,
                        heading: self.heading,
                        use_html: self.use_html,
                        para_start: false,
                        para_end: false,
                        out: String::new(),
                        open: Vec::new(),
                        line_start: true,
                    }
                    .run();
                    let title = link
                        .title
                        .as_deref()
                        .map(|title| format!(" {}", esc_title(title)))
                        .unwrap_or_default();
                    self.out
                        .push_str(&format!("[{body}]({}{title})", esc_dest(&link.url)));
                }
                self.line_start = false;
                i = j;
                continue;
            }

            let wanted = self.wanted(i);
            self.close_to(&wanted);
            self.open_at(i, &wanted);

            let mut j = i;
            if let Some(image) = style.image.clone() {
                while j < n && self.styles[j] == style {
                    j += 1;
                }
                self.out.push_str(&image_md(&self.chars[i..j], &image));
                self.line_start = false;
            } else if style.html {
                while j < n && self.styles[j] == style && self.chars[j] != SOFT {
                    j += 1;
                }
                self.out.extend(&self.chars[i..j]);
                self.line_start = false;
            } else if style.code {
                while j < n && self.styles[j] == style && !is_break(self.chars[j]) {
                    j += 1;
                }
                let text: String = self.chars[i..j].iter().collect();
                self.out.push_str(&code_span(&text));
                self.line_start = false;
            } else {
                while j < n && self.styles[j] == style && !is_break(self.chars[j]) {
                    j += 1;
                }
                let rest = &self.chars[j..];
                let at_end = rest.iter().all(|ch| *ch == OBJ)
                    || rest.first().is_some_and(|ch| is_break(*ch));
                let chars = self.chars;
                self.emit_text(&chars[i..j], at_end);
            }
            i = j;
        }

        self.close_to(&[]);
        self.out
    }
}

fn inline_md(chars: &[char], styles: &[Style], heading: bool, use_html: bool) -> String {
    Inline {
        chars,
        styles,
        heading,
        use_html,
        para_start: true,
        para_end: true,
        out: String::new(),
        open: Vec::new(),
        line_start: true,
    }
    .run()
}

/// Markdown delimiters when they survive a round trip, otherwise HTML tags.
fn inline_checked(block: &Block, heading: bool, wrap: impl Fn(&str) -> String) -> String {
    let mut text = String::new();
    for use_html in [false, true] {
        text = inline_md(&block.chars, &block.styles, heading, use_html);
        if block.chars.is_empty() {
            return text;
        }
        let back = parse(&wrap(&text));
        if back.blocks.len() == 1
            && back.blocks[0].chars == block.chars
            && back.blocks[0].styles == block.styles
        {
            return text;
        }
    }
    text
}

// ====================================================================
// Export: blocks
// ====================================================================

/// Hard breaks at the start or end of a paragraph cannot be expressed.
fn strip_edges(block: &Block) -> Block {
    let mut block = block.clone();
    while block.chars.first() == Some(&'\n') {
        block.chars.remove(0);
        block.styles.remove(0);
    }
    while block.chars.last() == Some(&'\n') {
        block.chars.pop();
        block.styles.pop();
    }
    block
}

/// Drops empty paragraphs unless they are the only content of their
/// container (an empty list item or block quote).
fn expressible(blocks: &[Block]) -> Vec<Block> {
    let mut out = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let empty = match block.leaf {
            Leaf::Paragraph => strip_edges(block).chars.is_empty(),
            Leaf::Html => block.chars.is_empty(),
            _ => false,
        };
        if empty {
            let Some(inner) = block.containers.last() else {
                continue;
            };
            let shared = blocks
                .iter()
                .enumerate()
                .any(|(other, b)| other != index && b.containers.iter().any(|c| c.same(inner)));
            if shared {
                continue;
            }
            out.push(Block::new(block.containers.clone(), Leaf::Paragraph));
        } else {
            out.push(block.clone());
        }
    }
    if out.is_empty() {
        out.push(Block::new(Vec::new(), Leaf::Paragraph));
    }
    out
}

const HTML_BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// HTML block types 1-6 can interrupt a paragraph, type 7 cannot.
fn html_interrupts(first_line: &str) -> bool {
    let s = first_line.trim_start_matches(' ');
    let lower = s.to_ascii_lowercase();
    for tag in ["<script", "<pre", "<style", "<textarea"] {
        if let Some(rest) = lower.strip_prefix(tag) {
            if rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace() || c == '>') {
                return true;
            }
        }
    }
    if s.starts_with("<!--") || s.starts_with("<?") || s.starts_with("<![CDATA[") {
        return true;
    }
    if s.starts_with("<!") && s[2..].starts_with(|c: char| c.is_ascii_alphabetic()) {
        return true;
    }
    let name_start = if s.starts_with("</") {
        2
    } else if s.starts_with('<') {
        1
    } else {
        return false;
    };
    let name: String = s[name_start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if name.is_empty() {
        return false;
    }
    let rest = &s[name_start + name.len()..];
    let ends_ok = rest.is_empty()
        || rest.starts_with(|c: char| c.is_whitespace())
        || rest.starts_with('>')
        || rest.starts_with("/>");
    ends_ok && HTML_BLOCK_TAGS.contains(&name.to_ascii_lowercase().as_str())
}

/// Written as a setext heading (content with line breaks).
fn is_setext(block: &Block) -> bool {
    matches!(block.leaf, Leaf::Heading(1 | 2))
        && strip_edges(block).chars.iter().any(|c| is_break(*c))
}

/// Can `b` start right after `a` without a blank line?
fn can_interrupt(a: &Block, b: &Block, common: usize) -> bool {
    if b.containers.len() > common
        && b.containers[common].is_quote()
        && a.containers.get(common).is_some_and(Container::is_quote)
    {
        return false; // two different quotes at the same depth would merge
    }
    if a.leaf == Leaf::Html {
        // An HTML block ends when its container closes.
        return common < a.containers.len();
    }
    if a.cell().is_some() {
        // A table continues with every line that is not another block.
        if b.containers.len() > common {
            return true;
        }
        return match b.leaf {
            Leaf::Heading(_) | Leaf::Code | Leaf::Rule => !is_setext(b),
            Leaf::Html => {
                let text = b.text();
                html_interrupts(text.split('\n').next().unwrap_or(""))
            }
            Leaf::Paragraph | Leaf::Line | Leaf::Cell(_) => false,
        };
    }
    if a.leaf != Leaf::Paragraph || strip_edges(a).chars.is_empty() {
        return true;
    }
    if b.containers.len() > common {
        let container = b.containers[common];
        let Container::Item { list, .. } = container else {
            return true;
        };
        let first_of_list = !a.containers.get(common).is_some_and(
            |other| matches!(other, Container::Item { list: l, .. } if l.id == list.id),
        );
        if first_of_list {
            if list.ordered && list.start != 1 {
                return false;
            }
            let empty_item = b.leaf == Leaf::Paragraph
                && strip_edges(b).chars.is_empty()
                && b.containers.len() == common + 1;
            if empty_item {
                return false;
            }
        }
        return true;
    }
    if is_setext(b) {
        return false; // the first line of a setext heading is paragraph text
    }
    match b.leaf {
        Leaf::Heading(_) | Leaf::Code | Leaf::Rule => true,
        // Outside the paragraph's container the header row would be a lazy
        // continuation line of the paragraph.
        Leaf::Cell(_) => common >= a.containers.len(),
        Leaf::Html => {
            let text = b.text();
            html_interrupts(text.split('\n').next().unwrap_or(""))
        }
        Leaf::Paragraph | Leaf::Line => false,
    }
}

fn needs_blank(a: &Block, b: &Block, common: usize) -> bool {
    // Deepest list both blocks are in (same list, possibly different items).
    let mut shared: Option<ListInfo> = None;
    for d in 0..a.containers.len().min(b.containers.len()) {
        match (a.containers[d], b.containers[d]) {
            (
                Container::Item {
                    list: la, id: ia, ..
                },
                Container::Item {
                    list: lb, id: ib, ..
                },
            ) if la.id == lb.id => {
                shared = Some(la);
                if ia != ib {
                    break;
                }
            }
            (ca, cb) if ca.same(&cb) => {}
            _ => break,
        }
    }
    !(shared.is_some_and(|list| list.tight) && can_interrupt(a, b, common))
}

/// Escapes the `|` that would end a table cell (also inside code spans and
/// link targets, where GFM removes the backslash again). Text already has
/// `\|`; with `all`, every `|` gets one more backslash, for a literal `\|`
/// inside a code span.
fn escape_pipes(text: &str, all: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous = None;
    for c in text.chars() {
        if c == '|' && (all || previous != Some('\\')) {
            out.push('\\');
        }
        out.push(c);
        previous = Some(c);
    }
    out
}

/// The content of a table cell on one line.
fn cell_md(block: &Block) -> String {
    let mut block = block.clone();
    for c in &mut block.chars {
        if is_break(*c) {
            *c = ' ';
        }
    }
    let mut text = String::new();
    for (use_html, all) in [(false, false), (false, true), (true, false), (true, true)] {
        text = escape_pipes(
            &inline_md(&block.chars, &block.styles, false, use_html),
            all,
        );
        if block.chars.is_empty() {
            return text;
        }
        let back = parse(&format!("| {text} |\n| --- |\n"));
        if back.blocks.len() == 1
            && back.blocks[0].chars == block.chars
            && back.blocks[0].styles == block.styles
        {
            return text;
        }
    }
    text
}

/// A table as Markdown lines, columns padded to the same width.
fn table_lines(cells: &[Block]) -> String {
    let mut rows: Vec<Vec<&Block>> = Vec::new();
    for cell in cells {
        let row = cell.cell().map(|cell| cell.row);
        match rows.last_mut() {
            Some(last) if last[0].cell().map(|cell| cell.row) == row => last.push(cell),
            _ => rows.push(vec![cell]),
        }
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(1);
    let texts: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let mut texts: Vec<String> = row.iter().map(|cell| cell_md(cell)).collect();
            texts.resize(columns, String::new());
            texts
        })
        .collect();
    let aligns: Vec<ColumnAlign> = (0..columns)
        .map(|column| {
            rows[0]
                .get(column)
                .and_then(|cell| cell.cell())
                .map(|cell| cell.align)
                .unwrap_or_default()
        })
        .collect();
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            texts
                .iter()
                .map(|row| row[column].chars().count())
                .max()
                .unwrap_or(0)
                .max(3)
        })
        .collect();

    let line = |row: &[String]| {
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(text, width)| {
                let pad = width - text.chars().count();
                format!("{text}{}", " ".repeat(pad))
            })
            .collect();
        format!("| {} |", cells.join(" | "))
    };
    let delimiter: Vec<String> = aligns
        .iter()
        .zip(&widths)
        .map(|(align, width)| match align {
            ColumnAlign::None => "-".repeat(*width),
            ColumnAlign::Left => format!(":{}", "-".repeat(width - 1)),
            ColumnAlign::Right => format!("{}:", "-".repeat(width - 1)),
            ColumnAlign::Center => format!(":{}:", "-".repeat(width - 2)),
        })
        .collect();

    let mut lines = vec![line(&texts[0]), format!("| {} |", delimiter.join(" | "))];
    lines.extend(texts[1..].iter().map(|row| line(row)));
    lines.join("\n")
}

fn block_lines(block: &Block) -> Option<String> {
    match block.leaf {
        Leaf::Cell(_) => Some(table_lines(std::slice::from_ref(block))),
        Leaf::Rule => Some("___".into()), // never confused with a list marker
        Leaf::Html | Leaf::Line => Some(block.text()),
        Leaf::Paragraph => {
            let block = strip_edges(block);
            if block.chars.is_empty() {
                None
            } else {
                Some(inline_checked(&block, false, |text| text.to_string()))
            }
        }
        Leaf::Heading(level) => {
            let level = level.clamp(1, 6) as usize;
            let block = strip_edges(block);
            if is_setext(&block) {
                let under = format!("\n{}", if level == 1 { "===" } else { "---" });
                let text = inline_checked(&block, false, |text| format!("{text}{under}"));
                return Some(format!("{text}{under}"));
            }
            let mut atx = block.clone();
            for c in &mut atx.chars {
                if *c == '\n' {
                    *c = ' ';
                }
            }
            let hashes = "#".repeat(level);
            let text = inline_checked(&atx, true, |text| format!("{hashes} {text}"));
            if text.is_empty() {
                Some(hashes)
            } else {
                Some(format!("{hashes} {text}"))
            }
        }
        Leaf::Code => {
            let text = block.text();
            let info = block.info.as_deref().unwrap_or("");
            let fence_char = if info.contains('`') { '~' } else { '`' };
            let mut longest = 0;
            let mut run = 0;
            for c in text.chars() {
                run = if c == fence_char { run + 1 } else { 0 };
                longest = longest.max(run);
            }
            let fence: String = std::iter::repeat(fence_char)
                .take((longest + 1).max(3))
                .collect();
            if text.is_empty() {
                Some(format!("{fence}{info}\n{fence}"))
            } else {
                Some(format!("{fence}{info}\n{text}\n{fence}"))
            }
        }
    }
}

fn first_prefix(containers: &[Container], common: usize, markers: &HashMap<u64, String>) -> String {
    let mut prefix = String::new();
    for (d, container) in containers.iter().enumerate() {
        match container {
            Container::Quote { .. } => prefix.push_str("> "),
            Container::Item { id, .. } => {
                let marker = markers.get(id).map(String::as_str).unwrap_or("- ");
                if d >= common {
                    prefix.push_str(marker);
                } else {
                    prefix.push_str(&" ".repeat(marker.len()));
                }
            }
        }
    }
    prefix
}

fn cont_prefix(containers: &[Container], markers: &HashMap<u64, String>) -> String {
    let mut prefix = String::new();
    for container in containers {
        match container {
            Container::Quote { .. } => prefix.push_str("> "),
            Container::Item { id, .. } => {
                let width = markers.get(id).map(String::len).unwrap_or(2);
                prefix.push_str(&" ".repeat(width));
            }
        }
    }
    prefix
}

/// The task marker written before `block`: only for the first paragraph of
/// a task item (GFM puts the marker at the start of that paragraph).
fn task_marker(block: &Block, common: usize) -> Option<&'static str> {
    let depth = block.containers.len().checked_sub(1)?;
    if depth < common || block.leaf != Leaf::Paragraph || strip_edges(block).chars.is_empty() {
        return None;
    }
    match block.containers[depth].task()? {
        true => Some("[x] "),
        false => Some("[ ] "),
    }
}

/// Writes the document as Markdown.
pub fn serialize(doc: &Document) -> String {
    let blocks = expressible(&doc.blocks);
    let mut lines: Vec<String> = Vec::new();
    let mut markers: HashMap<u64, String> = HashMap::new();
    let mut list_marker: HashMap<u64, char> = HashMap::new();
    let mut list_counter: HashMap<u64, u64> = HashMap::new();
    let mut previous: Option<&Block> = None;
    let mut index = 0;

    while index < blocks.len() {
        let block = &blocks[index];
        // A table is written as a whole: from its first to its last cell.
        let end = match block.cell() {
            Some(cell) => {
                let mut end = index + 1;
                while blocks
                    .get(end)
                    .and_then(Block::cell)
                    .is_some_and(|other| other.table == cell.table)
                {
                    end += 1;
                }
                end
            }
            None => index + 1,
        };
        let start = std::mem::replace(&mut index, end);
        let containers = &block.containers;
        let mut common = 0;
        if let Some(prev) = previous {
            while common < containers.len().min(prev.containers.len())
                && containers[common].same(&prev.containers[common])
            {
                common += 1;
            }
        }

        // Markers for items starting here.
        for d in common..containers.len() {
            let Container::Item { list, id, .. } = containers[d] else {
                continue;
            };
            if markers.contains_key(&id) {
                continue;
            }
            if !list_marker.contains_key(&list.id) {
                // A list right after another list at the same depth needs a
                // different marker, otherwise both would merge.
                let other = previous.and_then(|prev| {
                    let sibling = prev.containers.get(d)?;
                    let Container::Item { list: other, .. } = sibling else {
                        return None;
                    };
                    let same_parent = (0..d).all(|k| prev.containers[k].same(&containers[k]));
                    (other.id != list.id && same_parent)
                        .then(|| list_marker.get(&other.id).copied())
                        .flatten()
                });
                let marker = if list.ordered {
                    if other == Some('.') { ')' } else { '.' }
                } else if other == Some('-') {
                    '*'
                } else {
                    '-'
                };
                list_marker.insert(list.id, marker);
                list_counter.insert(list.id, list.start.min(999_999_999));
            }
            let marker = list_marker[&list.id];
            let text = if list.ordered {
                let number = list_counter[&list.id];
                list_counter.insert(list.id, (number + 1).min(999_999_999));
                format!("{number}{marker} ")
            } else {
                format!("{marker} ")
            };
            markers.insert(id, text);
        }

        if let Some(prev) = previous {
            if needs_blank(prev, block, common) {
                lines.push(
                    cont_prefix(&containers[..common], &markers)
                        .trim_end()
                        .to_string(),
                );
            }
        }

        let mut first = first_prefix(containers, common, &markers);
        if let Some(task) = task_marker(block, common) {
            first.push_str(task);
        }
        let cont = cont_prefix(containers, &markers);
        let body = if block.cell().is_some() {
            Some(table_lines(&blocks[start..end]))
        } else {
            block_lines(block)
        };
        match body {
            None => lines.push(first.trim_end().to_string()),
            Some(body) => {
                for (k, line) in body.split('\n').enumerate() {
                    let prefix = if k == 0 { &first } else { &cont };
                    if line.is_empty() {
                        lines.push(prefix.trim_end().to_string());
                    } else {
                        lines.push(format!("{prefix}{line}"));
                    }
                }
            }
        }

        previous = Some(&blocks[end - 1]);
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Parses and writes Markdown again, in the form the editor saves it.
pub fn normalize(markdown: &str) -> String {
    serialize(&parse(markdown))
}

// ====================================================================
// Tests
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn html(markdown: &str) -> String {
        let mut out = String::new();
        pulldown_cmark::html::push_html(&mut out, Parser::new_ext(markdown, options()));
        out
    }

    #[derive(serde::Deserialize)]
    struct Example {
        markdown: String,
        example: u32,
        section: String,
    }

    /// Examples the editor cannot reproduce exactly because its model has
    /// no "emphasis inside the same emphasis" (e.g. `*(*foo*)*`). The
    /// rendering looks the same; only the redundant nesting is lost.
    const NESTED_EMPHASIS: &[u32] = &[
        368, 372, 388, 406, 407, 408, 416, 417, 418, 424, 425, 426, 431, 460, 462, 463, 464, 465,
        467,
    ];

    /// Different formats on the same text whose nesting order changes,
    /// e.g. `<del>*foo*</del>` is written as `*~~foo~~*`. The model stores
    /// formats per character, not their order; the rendering looks the same.
    const FORMAT_ORDER: &[u32] = &[168];

    fn examples() -> Vec<Example> {
        serde_json::from_str(include_str!("../tests/commonmark-0.31.2.json")).unwrap()
    }

    /// Every CommonMark 0.31.2 example produces the same HTML after the
    /// editor has read and written it.
    #[test]
    fn commonmark_spec_round_trip() {
        let examples = examples();
        assert_eq!(examples.len(), 652);

        let mut unexpected = Vec::new();
        let mut known = 0;
        for example in &examples {
            let written = normalize(&example.markdown);
            let expected = html(&example.markdown);
            let actual = html(&written);
            if expected != actual {
                let is_known = NESTED_EMPHASIS.contains(&example.example)
                    || FORMAT_ORDER.contains(&example.example);
                if is_known {
                    known += 1;
                } else {
                    unexpected.push(format!(
                        "example {} ({})\n--- markdown\n{}--- written\n{}--- expected html\n{}--- actual html\n{}",
                        example.example, example.section, example.markdown, written, expected, actual
                    ));
                }
            }
        }

        println!(
            "round trip: {} of 652 identical, {known} known differences",
            652 - known - unexpected.len()
        );
        if !unexpected.is_empty() {
            panic!(
                "{} unexpected differences:\n\n{}",
                unexpected.len(),
                unexpected.join("\n\n")
            );
        }
    }

    /// Writing is stable: saving an opened file twice gives the same text.
    #[test]
    fn commonmark_spec_stable() {
        for example in &examples() {
            let once = normalize(&example.markdown);
            let twice = normalize(&once);
            assert_eq!(once, twice, "example {} is not stable", example.example);
        }
    }

    #[test]
    fn task_lists() {
        for source in [
            "- [ ] asdf\n",
            "- [ ] open\n- [x] done\n",
            "1. [x] first\n2. [ ] second\n",
            "- [ ] parent\n  - [x] child\n  - plain child\n",
            "- [x] item\n\n  with a second paragraph\n",
            "> - [ ] in a quote\n",
            "- [ ] **bold** and [link](https://x.org)\n",
        ] {
            assert_eq!(normalize(source), source, "task list changed");
        }

        let doc = parse("- [ ] open\n- [X] done\n* [x] other list\n");
        let tasks: Vec<Option<bool>> = doc
            .blocks
            .iter()
            .map(|block| block.containers.last().and_then(Container::task))
            .collect();
        assert_eq!(tasks, vec![Some(false), Some(true), Some(true)]);
        assert_eq!(doc.blocks[0].text(), "open");
        assert_eq!(normalize("- [X] done\n"), "- [x] done\n");

        assert_eq!(
            normalize("- \\[ \\] not a task\n"),
            "- \\[ \\] not a task\n"
        );
        let doc = parse("- \\[ \\] not a task\n");
        assert_eq!(doc.blocks[0].containers[0].task(), None);
        assert_eq!(doc.blocks[0].text(), "[ ] not a task");
    }

    #[test]
    fn tables() {
        for source in [
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n",
            "| left | center | right | none |\n| :--- | :----: | ----: | ---- |\n| 1    | 2      | 3     | 4    |\n",
            "| **bold** | `code` | [link](https://x.org) |\n| -------- | ------ | --------------------- |\n",
            "| a\\|b | `x\\|y` |\n| ---- | ------ |\n|      |        |\n",
            "text\n\n| a   |\n| --- |\n\nmore text\n",
            "- item\n- | a   |\n  | --- |\n  | 1   |\n- next\n",
            "> | a   |\n> | --- |\n> | 1   |\n",
            "| a   |\n| --- |\n\n| b   |\n| --- |\n",
            "| a   |\n| --- |\n\n# heading\n",
            "| &#32;x&#32; |\n| ----------- |\n",
        ] {
            assert_eq!(normalize(source), source, "table changed");
        }

        let doc = parse("| a | b |\n|:-|-:|\n| 1 |\n");
        assert_eq!(doc.blocks.len(), 4);
        let cells: Vec<Cell> = doc.blocks.iter().filter_map(Block::cell).collect();
        assert!(cells[0].header && cells[1].header && !cells[2].header);
        assert_eq!(cells[0].row, cells[1].row);
        assert_ne!(cells[1].row, cells[2].row);
        assert!(cells.iter().all(|cell| cell.table == cells[0].table));
        assert_eq!(cells[0].align, ColumnAlign::Left);
        assert_eq!(cells[3].align, ColumnAlign::Right);
        assert_eq!(doc.blocks[3].text(), "", "missing cells are filled");

        // Not a table: the delimiter row does not match the header.
        assert_eq!(parse("| a | b |\n| - |\n").blocks[0].leaf, Leaf::Paragraph);
        // A paragraph after a table needs a blank line.
        let mut doc = parse("| a |\n| - |\n");
        doc.blocks.push(Block::new(Vec::new(), Leaf::Paragraph));
        doc.blocks[1].push('x', Style::default());
        assert_eq!(serialize(&doc), "| a   |\n| --- |\n\nx\n");
    }

    #[test]
    fn image_markdown_round_trips() {
        let text = image_markdown("a [b]", "assets/my image.png");
        assert_eq!(text, "![a \\[b\\]](<assets/my image.png>)");
        let doc = parse(&text);
        let style = &doc.blocks[0].styles[0];
        assert_eq!(style.image.as_ref().unwrap().url, "assets/my image.png");
        assert_eq!(doc.blocks[0].text(), "a [b]");
    }

    // ---- random documents as the editor can create them ---------------

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn chance(&mut self, p: f64) -> bool {
            (self.next() % 10_000) as f64 / 10_000.0 < p
        }
        fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
            &items[self.below(items.len() as u64) as usize]
        }
    }

    const PIECES: &[&str] = &[
        "a", "b", "c", " ", "x", "y", "z", "1", "2", "3", "*", "_", "[", "]", "#", "<", ">", "!",
        "-", "+", "=", ".", ")", "`", "\\", "&", "~", "|", "  ", "\t", "\u{2028}", "\n", "\u{a0}",
        "[ ] ", "[x] ",
    ];

    fn random_inline(rng: &mut Rng, len: usize) -> (Vec<char>, Vec<Style>) {
        let links = [
            Link::new("https://x.org/a", None),
            Link::new("/u", Some("T \"q\"".into())),
            Link::new("mailto:a@b.co", None),
        ];
        let mut chars = Vec::new();
        let mut styles = Vec::new();
        let mut style = Style::default();
        while chars.len() < len {
            if rng.chance(0.25) {
                style = Style {
                    bold: rng.chance(0.3),
                    italic: rng.chance(0.3),
                    strike: rng.chance(0.2),
                    code: rng.chance(0.15),
                    link: rng.chance(0.15).then(|| rng.pick(&links).clone()),
                    ..Style::default()
                };
            }
            for mut c in rng.pick(PIECES).chars() {
                if style.code && (is_break(c) || c == '\t') {
                    c = 'q';
                }
                chars.push(c);
                styles.push(style.clone());
            }
        }
        (chars, styles)
    }

    fn random_blocks(rng: &mut Rng, path: Vec<Container>, depth: usize, out: &mut Vec<Block>) {
        for _ in 0..1 + rng.below(3) {
            let roll = rng.below(100);
            if depth < 3 && roll < 20 {
                let mut inner = path.clone();
                inner.push(Container::quote());
                random_blocks(rng, inner, depth + 1, out);
            } else if (44..50).contains(&roll) {
                let table = crate::doc::next_id();
                let columns = 1 + rng.below(3);
                let aligns: Vec<ColumnAlign> = (0..columns)
                    .map(|_| {
                        *rng.pick(&[
                            ColumnAlign::None,
                            ColumnAlign::Left,
                            ColumnAlign::Center,
                            ColumnAlign::Right,
                        ])
                    })
                    .collect();
                for row in 0..1 + rng.below(3) {
                    let id = crate::doc::next_id();
                    for align in &aligns {
                        let mut block = Block::new(
                            path.clone(),
                            Leaf::Cell(Cell {
                                table,
                                row: id,
                                header: row == 0,
                                align: *align,
                            }),
                        );
                        let len = rng.below(8) as usize;
                        let (chars, styles) = random_inline(rng, len);
                        block.chars = chars
                            .into_iter()
                            .map(|c| if is_break(c) { 'q' } else { c })
                            .collect();
                        block.styles = styles;
                        out.push(block);
                    }
                }
            } else if depth < 3 && roll < 40 {
                let mut list = ListInfo::new(rng.chance(0.5), *rng.pick(&[1, 1, 3, 10]));
                list.tight = rng.chance(0.6);
                for _ in 0..1 + rng.below(3) {
                    let task = if rng.chance(0.3) {
                        Some(rng.chance(0.5))
                    } else {
                        None
                    };
                    let mut inner = path.clone();
                    inner.push(Container::task_item(list, task));
                    random_blocks(rng, inner, depth + 1, out);
                }
            } else {
                let leaf = *rng.pick(&[
                    Leaf::Paragraph,
                    Leaf::Paragraph,
                    Leaf::Paragraph,
                    Leaf::Heading(1),
                    Leaf::Heading(2),
                    Leaf::Heading(3),
                    Leaf::Code,
                    Leaf::Rule,
                    Leaf::Html,
                ]);
                let mut block = Block::new(path.clone(), leaf);
                match leaf {
                    Leaf::Rule => {}
                    Leaf::Code => {
                        for _ in 0..rng.below(13) {
                            block.push(
                                *rng.pick(&['x', ' ', '\n', '`', '~', '\t']),
                                Style::default(),
                            );
                        }
                        block.info = (*rng.pick(&[None, Some("rust"), Some("a b")])).map(Arc::from);
                    }
                    Leaf::Html => {
                        let text = format!("<div>\n{}\n</div>", rng.pick(&["x", "*y*", "z"]));
                        for c in text.chars() {
                            block.push(c, Style::default());
                        }
                    }
                    _ => {
                        let len = 1 + rng.below(14) as usize;
                        let (chars, styles) = random_inline(rng, len);
                        block.chars = chars;
                        block.styles = styles;
                    }
                }
                out.push(block);
            }
        }
    }

    /// What Markdown cannot express, applied to the expected document.
    fn expected(blocks: &[Block]) -> Vec<Block> {
        let mut out: Vec<Block> = Vec::new();
        for block in expressible(blocks) {
            let mut block = if block.leaf.is_text() {
                strip_edges(&block)
            } else {
                block
            };
            if matches!(block.leaf, Leaf::Heading(3..=6)) {
                for c in &mut block.chars {
                    if *c == '\n' {
                        *c = ' ';
                    }
                }
            }
            out.push(block);
        }
        for index in 0..out.len() {
            for depth in 0..out[index].containers.len() {
                let Container::Item { task: Some(_), .. } = out[index].containers[depth] else {
                    continue;
                };
                let container = out[index].containers[depth];
                let first = (0..out.len())
                    .find(|k| {
                        out[*k]
                            .containers
                            .get(depth)
                            .is_some_and(|c| c.same(&container))
                    })
                    .unwrap();
                let ok = out[first].leaf == Leaf::Paragraph
                    && out[first].containers.len() == depth + 1
                    && !out[first].chars.is_empty();
                if !ok {
                    if let Container::Item { task, .. } = &mut out[index].containers[depth] {
                        *task = None;
                    }
                }
            }
        }
        out
    }

    type Canon = Vec<(
        Vec<(char, usize, usize, bool, u64, Option<bool>)>,
        String,
        String,
        Vec<Style>,
        Option<String>,
    )>;

    fn canon(blocks: &[Block]) -> Canon {
        let mut ids: HashMap<(char, u64), usize> = HashMap::new();
        let mut id = |kind: char, value: u64| {
            let len = ids.len();
            *ids.entry((kind, value)).or_insert(len)
        };
        blocks
            .iter()
            .map(|block| {
                let containers: Vec<(char, usize, usize, bool, u64, Option<bool>)> = block
                    .containers
                    .iter()
                    .map(|c| match c {
                        Container::Quote { id: q } => ('q', id('q', *q), 0, false, 0, None),
                        Container::Item {
                            list,
                            id: item,
                            task,
                        } => (
                            'i',
                            id('l', list.id),
                            id('i', *item),
                            list.ordered,
                            if list.ordered { list.start } else { 1 },
                            *task,
                        ),
                    })
                    .collect();
                let styles: Vec<Style> = block
                    .chars
                    .iter()
                    .zip(&block.styles)
                    .map(|(c, s)| {
                        if is_break(*c) {
                            Style::default()
                        } else {
                            s.clone()
                        }
                    })
                    .collect();
                let leaf = match block.leaf {
                    Leaf::Cell(cell) => format!(
                        "Cell({}, {}, {}, {:?})",
                        id('t', cell.table),
                        id('r', cell.row),
                        cell.header,
                        cell.align
                    ),
                    leaf => format!("{leaf:?}"),
                };
                (
                    containers,
                    leaf,
                    block.text(),
                    styles,
                    block.info.as_deref().map(str::to_string),
                )
            })
            .collect()
    }

    #[test]
    fn random_documents_round_trip() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut failures = Vec::new();
        for _ in 0..3000 {
            let mut blocks = Vec::new();
            random_blocks(&mut rng, Vec::new(), 0, &mut blocks);
            let doc = Document { blocks };
            let written = serialize(&doc);
            let back = parse(&written);
            if canon(&back.blocks) != canon(&expected(&doc.blocks)) {
                let (x, y) = (canon(&back.blocks), canon(&expected(&doc.blocks)));
                let diff = x.iter().zip(&y).find(|(a, b)| a != b);
                failures.push(format!("not preserved:\n{written}\n{diff:#?}"));
                continue;
            }
            let again = serialize(&back);
            if serialize(&parse(&again)) != again {
                failures.push(format!("not stable:\n{again}"));
            }
        }
        if !failures.is_empty() {
            panic!("{} of 3000 failed, first:\n{}", failures.len(), failures[0]);
        }
    }
}
