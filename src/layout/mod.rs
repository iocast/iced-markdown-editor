//! Page layout: pagination, line wrapping, nested containers, tables,
//! images, caret geometry and hit-testing.
//!
//! Everything is computed in document coordinates at the current zoom. The
//! view translates them by the scroll offset.
//!
//! * `table`: GFM tables.
//! * `custom`: code blocks drawn by a [`BlockRenderer`](crate::BlockRenderer).
//! * `images`: images inside text.
//! * `hit`: caret geometry and hit-testing.

mod custom;
mod hit;
mod images;
mod table;

use crate::doc::{Block, Container, Document, Leaf, OBJ, SOFT, Style};
use crate::fonts::{Face, Fonts};
use crate::images::LoadedImage;
use crate::page::Paper;
use crate::style::EditorStyle;
use iced::{Color, Size};
use std::collections::HashMap;

use images::{block_images, place_images, row_height};

const CODE_PAD: f32 = 10.0;
const RULE_HEIGHT: f32 = 26.0;
const MIN_TEXT_WIDTH: f32 = 80.0;
const IMAGE_GAP: f32 = 4.0;
const BULLETS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];

/// Looks up a displayable image by URL.
pub type ImageLookup<'a> = &'a dyn Fn(&str) -> Option<LoadedImage>;

/// Size of a block drawn by an extension: `(block, width, max height)`;
/// `None` lays the block out as text.
pub type CustomLookup<'a> = &'a dyn Fn(usize, f32, f32) -> Option<Size>;

/// Content the layout asks the editor for.
#[derive(Clone, Copy)]
pub struct Lookups<'a> {
    pub images: ImageLookup<'a>,
    pub custom: CustomLookup<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct BlockMetrics {
    pub size: f32,
    pub line_height: f32,
    pub space_before: f32,
    pub space_after: f32,
    pub color: Color,
}

pub fn metrics(leaf: Leaf, style: &EditorStyle, zoom: f32) -> BlockMetrics {
    let t = &style.typography;
    let p = &style.palette;
    let z = zoom;
    let base = BlockMetrics {
        size: t.body_size * z,
        line_height: t.body_line_height * z,
        space_before: 0.0,
        space_after: t.paragraph_spacing * z,
        color: p.ink,
    };

    match leaf {
        Leaf::Paragraph | Leaf::Rule | Leaf::Cell(_) => base,
        Leaf::Heading(level) => {
            let index = (level.clamp(1, 3) - 1) as usize;
            let size = t.heading_sizes[index] * z;
            BlockMetrics {
                size,
                line_height: (size * 1.3).ceil(),
                space_before: size * 0.5,
                space_after: size * 0.35,
                color: p.heading,
            }
        }
        // Markdown source: monospace lines without paragraph spacing.
        Leaf::Line => BlockMetrics {
            size: t.code_size * z,
            line_height: t.code_line_height * z,
            space_after: 0.0,
            ..base
        },
        Leaf::Code | Leaf::Html => BlockMetrics {
            size: t.code_size * z,
            line_height: t.code_line_height * z,
            space_after: (t.paragraph_spacing + 2.0) * z,
            color: if leaf == Leaf::Html { p.muted } else { p.code },
            ..base
        },
    }
}

/// The font face used for a character.
pub fn face_for(leaf: Leaf, style: &Style) -> Face {
    let header = leaf.cell().is_some_and(|cell| cell.header);
    let bold = matches!(leaf, Leaf::Heading(_)) || header || style.bold;
    let italic = style.italic || style.image.is_some();
    let mono = leaf.is_plain() || style.code || style.html;
    Face::select(bold, italic, mono)
}

/// Width of a character in the layout.
pub fn advance(fonts: &Fonts, leaf: Leaf, style: &Style, c: char, size: f32) -> f32 {
    let face = face_for(leaf, style);
    match c {
        '\n' => 0.0,
        SOFT => fonts.advance(face, ' ', size),
        '\t' => 4.0 * fonts.advance(face, ' ', size),
        OBJ => size * 0.9,
        c => fonts.advance(face, c, size),
    }
}

/// One visual line of text.
#[derive(Debug, Clone)]
pub struct Line {
    pub block: usize,
    /// First character (inclusive).
    pub start: usize,
    /// Last character (exclusive).
    pub end: usize,
    /// x of every caret position `start..=end`.
    pub xs: Vec<f32>,
    pub y: f32,
    pub height: f32,
    pub size: f32,
    pub color: Color,
    /// Whether this is the last line of its block.
    pub last: bool,
    /// Page index (0-based).
    pub page: usize,
    /// Character ranges drawn as images instead of text.
    pub images: Vec<(usize, usize)>,
    /// The table cell showing this line.
    pub cell: Option<CellBox>,
}

/// Area of a table cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellBox {
    /// Table row, counted through the whole document.
    pub row: usize,
    pub x: f32,
    pub width: f32,
    pub top: f32,
    pub bottom: f32,
}

impl CellBox {
    /// Horizontal distance from `x` to the cell (0 inside).
    fn distance(&self, x: f32) -> f32 {
        if x < self.x {
            self.x - x
        } else if x > self.x + self.width {
            x - self.x - self.width
        } else {
            0.0
        }
    }
}

/// Non-text decorations.
#[derive(Debug, Clone)]
pub enum Decor {
    Marker {
        text: String,
        x: f32,
        y: f32,
        size: f32,
        height: f32,
        color: Color,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Color,
    },
    Checkbox {
        x: f32,
        y: f32,
        size: f32,
        checked: bool,
    },
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        image: LoadedImage,
    },
    /// A block drawn by an extension, with the room it was measured for.
    Custom {
        block: usize,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        available: Size,
    },
}

/// Clickable checkbox of a task list item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaskBox {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    /// First block of the item and the container depth of the item.
    pub block: usize,
    pub depth: usize,
}

impl TaskBox {
    /// Hit test with a little tolerance around the box.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let pad = self.size * 0.3;
        x >= self.x - pad
            && x <= self.x + self.size + pad
            && y >= self.y - pad
            && y <= self.y + self.size + pad
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub lines: Vec<Line>,
    pub decor: Vec<Decor>,
    pub tasks: Vec<TaskBox>,
    pub pages: Vec<PageRect>,
    /// Size of the whole document area (can exceed the viewport).
    pub width: f32,
    pub height: f32,
    /// Full-width mode: no sheet, no page breaks.
    pub fill: bool,
}

/// Vertical flow with page breaks.
struct Flow {
    y: f32,
    page: usize,
    gap: f32,
    page_height: f32,
    top: f32,
    bottom: f32,
    paged: bool,
}

impl Flow {
    fn page_top(&self, page: usize) -> f32 {
        if page == 0 {
            self.gap
        } else {
            self.gap + page as f32 * (self.page_height + self.gap)
        }
    }

    fn content_top(&self) -> f32 {
        self.page_top(self.page) + self.top
    }

    fn at_page_top(&self) -> bool {
        (self.y - self.content_top()).abs() < 0.5
    }

    fn space(&mut self, amount: f32) {
        if !self.at_page_top() {
            self.y += amount;
        }
    }

    fn fit(&mut self, height: f32) {
        if !self.paged || self.at_page_top() {
            return;
        }
        let limit = self.page_top(self.page) + self.page_height - self.bottom;
        if self.y + height > limit {
            self.page += 1;
            self.y = self.content_top();
        }
    }

    fn usable_height(&self) -> f32 {
        if self.paged {
            (self.page_height - self.top - self.bottom).max(1.0)
        } else {
            f32::INFINITY
        }
    }
}

/// Indentation of each container level.
fn indents(containers: &[Container], style: &EditorStyle, zoom: f32) -> Vec<f32> {
    let t = &style.typography;
    let mut x = 0.0;
    let mut out = vec![0.0];
    for container in containers {
        x += match container {
            Container::Quote { .. } => t.quote_indent * zoom,
            Container::Item { .. } => t.list_indent * zoom,
        };
        out.push(x);
    }
    out
}

/// Whether `block` is the first block of its container at `depth`.
fn starts_container(doc: &Document, index: usize, depth: usize) -> bool {
    let container = doc.blocks[index].containers[depth];
    index == 0
        || !doc.blocks[index - 1]
            .containers
            .get(depth)
            .is_some_and(|other| other.same(&container))
}

/// Shared state while laying out the blocks.
struct Pass<'a> {
    doc: &'a Document,
    fonts: &'a Fonts,
    style: &'a EditorStyle,
    z: f32,
    text_x: f32,
    layout: Layout,
    flow: Flow,
    numbers: HashMap<u64, u64>,
    bars: HashMap<u64, (usize, f32)>,
    rows: usize,
}

impl Pass<'_> {
    /// Bars of the block quotes around one visual row of `block`.
    fn quote_bars(&mut self, block: &Block, indent: &[f32], first: bool, top: f32, bottom: f32) {
        let z = self.z;
        for (depth, container) in block.containers.iter().enumerate() {
            let Container::Quote { id } = container else {
                continue;
            };
            let x = self.text_x + indent[depth] + 2.0 * z;
            let from = match self.bars.get(id) {
                Some((page, end)) if *page == self.flow.page && first => *end,
                _ => top,
            };
            self.layout.decor.push(Decor::Rect {
                x,
                y: from,
                w: 3.0 * z,
                h: bottom - from,
                color: self.style.palette.quote_bar,
            });
            self.bars.insert(*id, (self.flow.page, bottom));
        }
    }

    /// Bullets, numbers and checkboxes of the list items starting at `index`.
    fn markers(&mut self, index: usize, indent: &[f32], m: &BlockMetrics, marker_y: f32) {
        let t = &self.style.typography;
        let p = &self.style.palette;
        let z = self.z;
        let doc = self.doc;
        let mut level = 0;
        for (depth, container) in doc.blocks[index].containers.iter().enumerate() {
            let Container::Item { list, task, .. } = container else {
                continue;
            };
            if starts_container(doc, index, depth) {
                let right = self.text_x + indent[depth + 1];
                let marker_size = m.size.min(t.body_size * z);
                let marker_height = m.line_height;
                if let Some(checked) = task {
                    let size = (marker_size * 0.95).round();
                    let x = right - 10.0 * z - size;
                    let y = marker_y + ((marker_height - size) / 2.0).max(0.0);
                    self.layout.decor.push(Decor::Checkbox {
                        x,
                        y,
                        size,
                        checked: *checked,
                    });
                    self.layout.tasks.push(TaskBox {
                        x,
                        y,
                        size,
                        block: index,
                        depth,
                    });
                    if list.ordered {
                        let number = *self.numbers.get(&list.id).unwrap_or(&list.start);
                        self.numbers.insert(list.id, number + 1);
                    }
                } else {
                    let text = if list.ordered {
                        let number = *self.numbers.get(&list.id).unwrap_or(&list.start);
                        self.numbers.insert(list.id, number + 1);
                        format!("{number}.")
                    } else {
                        BULLETS[level % BULLETS.len()].to_string()
                    };
                    let text_width = self.fonts.width(Face::Sans, &text, marker_size);
                    let offset = if list.ordered { 8.0 } else { 12.0 } * z;
                    self.layout.decor.push(Decor::Marker {
                        text,
                        x: right - offset - text_width,
                        y: marker_y,
                        size: marker_size,
                        height: marker_height,
                        color: p.marker,
                    });
                }
            }
            level += 1;
        }
    }
}

pub fn compute(
    doc: &Document,
    fonts: &Fonts,
    style: &EditorStyle,
    paper: &Paper,
    viewport: Size,
    zoom: f32,
    lookups: Lookups<'_>,
) -> Layout {
    let images = lookups.images;
    let t = &style.typography;
    let p = &style.palette;
    let z = zoom.max(0.05);
    let fill = paper.is_fill() || paper.size_px().is_none();
    let gap = if fill { 0.0 } else { t.page_gap };

    let (page_w, page_h) = match paper.size_px() {
        Some((w, h)) if !fill => (w * z, h * z),
        _ => (viewport.width.max(1.0), f32::INFINITY),
    };
    let width = if fill {
        page_w
    } else {
        (page_w + 2.0 * gap).max(viewport.width)
    };
    let page_x = if fill {
        0.0
    } else {
        ((width - page_w) / 2.0).max(gap)
    };

    let (mut margin_left, mut margin_right, margin_top, margin_bottom) = if fill {
        let pad = t.fill_padding;
        (pad, pad, pad, pad)
    } else {
        let margins = &paper.margins;
        (
            margins.left.to_px() * z,
            margins.right.to_px() * z,
            margins.top.to_px() * z,
            margins.bottom.to_px() * z,
        )
    };
    let min_text = MIN_TEXT_WIDTH.min(page_w * 0.5);
    if page_w - margin_left - margin_right < min_text {
        let room = (page_w - min_text).max(0.0);
        let scale = room / (margin_left + margin_right).max(1.0);
        margin_left *= scale;
        margin_right *= scale;
    }
    let text_x = page_x + margin_left;
    let text_w = (page_w - margin_left - margin_right).max(1.0);

    let mut pass = Pass {
        doc,
        fonts,
        style,
        z,
        text_x,
        layout: Layout {
            fill,
            ..Layout::default()
        },
        flow: Flow {
            y: gap + margin_top,
            page: 0,
            gap,
            page_height: page_h,
            top: margin_top,
            bottom: margin_bottom,
            paged: !fill,
        },
        numbers: HashMap::new(),
        bars: HashMap::new(),
        rows: 0,
    };

    let mut index = 0;
    while index < doc.blocks.len() {
        let block = &doc.blocks[index];
        let table = doc.table_range(index);
        let end = table.map(|(_, end)| end).unwrap_or(index + 1);
        let m = metrics(block.leaf, style, z);
        let indent = indents(&block.containers, style, z);
        let left = text_x + indent[block.containers.len()];
        let width = (text_w - indent[block.containers.len()]).max(MIN_TEXT_WIDTH.min(text_w));

        if index > 0 {
            pass.flow.space(m.space_before);
        }

        let next = doc.blocks.get(end);
        let tight = match block.inner() {
            Some(Container::Item { list, .. }) => {
                list.tight
                    && next.is_some_and(|next| {
                        next.containers
                            .iter()
                            .any(|c| c.list().is_some_and(|other| other.id == list.id))
                    })
            }
            _ => false,
        };
        let space_after = if tight {
            (t.paragraph_spacing * 0.3).max(1.0) * z
        } else {
            m.space_after
        };

        if let Some(table) = table {
            pass.table(table, &indent, left, width, images);
            pass.flow.y += space_after;
            index = end;
            continue;
        }

        if block.leaf == Leaf::Code
            && let Some(size) = (lookups.custom)(index, width, pass.flow.usable_height())
        {
            let available = Size::new(width, pass.flow.usable_height());
            pass.custom(index, &indent, left, available, size);
            pass.flow.y += space_after;
            index += 1;
            continue;
        }

        let raw = block.leaf.is_raw();
        let pad = if raw { CODE_PAD * z } else { 0.0 };
        let (image_sizes, image_runs) = block_images(
            block,
            images,
            z,
            width,
            pass.flow.usable_height() - m.line_height,
        );
        let rows = if block.leaf == Leaf::Rule {
            vec![(0, 0, vec![0.0])]
        } else {
            let inner = if raw { width - 2.0 * pad } else { width };
            wrap(block, fonts, &m, inner.max(1.0), &image_sizes)
        };
        let count = rows.len();

        for (row, (start, end, xs)) in rows.into_iter().enumerate() {
            let first = row == 0;
            let last = row + 1 == count;
            let top_pad = if first { pad } else { 0.0 };
            let bottom_pad = if last { pad } else { 0.0 };

            let (row_height, image_height) = if block.leaf == Leaf::Rule {
                (RULE_HEIGHT * z, 0.0)
            } else {
                row_height(&image_sizes, start, end, &m, z)
            };

            pass.flow.fit(top_pad + row_height + bottom_pad);
            let y = pass.flow.y + top_pad;
            let row_top = pass.flow.y;
            let row_bottom = y + row_height + bottom_pad;

            pass.quote_bars(block, &indent, first, row_top, row_bottom);

            if raw {
                pass.layout.decor.push(Decor::Rect {
                    x: left,
                    y: row_top,
                    w: width,
                    h: top_pad + row_height + bottom_pad,
                    color: p.code_background,
                });
            }

            if block.leaf == Leaf::Rule {
                pass.layout.decor.push(Decor::Rect {
                    x: left,
                    y: y + row_height / 2.0,
                    w: width,
                    h: z.max(1.0),
                    color: p.rule,
                });
            }

            let marker_y = y + row_height
                - m.line_height
                - if image_height > 0.0 {
                    IMAGE_GAP * z
                } else {
                    0.0
                };
            if first {
                pass.markers(index, &indent, &m, marker_y);
            }

            let x0 = left + if raw { pad } else { 0.0 };
            let line_images = place_images(
                &mut pass.layout,
                &image_runs,
                (start, end),
                &xs,
                x0,
                y + row_height - IMAGE_GAP * z,
            );

            pass.layout.lines.push(Line {
                block: index,
                start,
                end,
                xs: xs.into_iter().map(|x| x + x0).collect(),
                y,
                height: row_height,
                size: m.size,
                color: m.color,
                last,
                page: pass.flow.page,
                images: line_images,
                cell: None,
            });

            pass.flow.y = y + row_height + bottom_pad;
        }

        pass.flow.y += space_after;
        index += 1;
    }

    let Pass {
        mut layout, flow, ..
    } = pass;

    if fill {
        let height = (flow.y + margin_bottom).max(viewport.height);
        layout.pages.push(PageRect {
            x: 0.0,
            y: 0.0,
            width: page_w,
            height,
        });
        layout.width = page_w;
        layout.height = height;
    } else {
        let count = flow.page + 1;
        for page in 0..count {
            layout.pages.push(PageRect {
                x: page_x,
                y: flow.page_top(page),
                width: page_w,
                height: page_h,
            });
        }
        layout.width = width;
        layout.height = flow.page_top(count);
    }

    layout
}

/// A wrapped line: `(start, end, xs)`.
type Row = (usize, usize, Vec<f32>);

/// Breaks a block into lines with x positions relative to the line start.
fn wrap(
    block: &Block,
    fonts: &Fonts,
    m: &BlockMetrics,
    width: f32,
    images: &[Option<(f32, f32)>],
) -> Vec<Row> {
    let chars = &block.chars;
    let len = chars.len();
    let advance_at = |i: usize| -> f32 {
        match images.get(i).copied().flatten() {
            Some((w, _)) => w,
            None => advance(fonts, block.leaf, &block.styles[i], chars[i], m.size),
        }
    };
    let breakable = |i: usize| {
        (chars[i] == ' ' || chars[i] == SOFT) && images.get(i).copied().flatten().is_none()
    };

    let mut rows = Vec::new();
    let mut start = 0usize;
    let mut x = 0.0f32;
    let mut xs = vec![0.0f32];
    let mut last_space: Option<usize> = None;
    let mut i = 0usize;

    while i < len {
        let c = chars[i];

        if c == '\n' {
            xs.push(x);
            rows.push((start, i + 1, std::mem::replace(&mut xs, vec![0.0])));
            start = i + 1;
            x = 0.0;
            last_space = None;
            i += 1;
            continue;
        }

        let w = advance_at(i);

        if !breakable(i) && i > start && w > 0.0 && x + w > width {
            let brk = match last_space {
                Some(space) => space + 1,
                None => i,
            };
            xs.truncate(brk - start + 1);
            rows.push((start, brk, std::mem::replace(&mut xs, vec![0.0])));
            start = brk;
            x = 0.0;
            last_space = None;
            for j in start..i {
                x += advance_at(j);
                xs.push(x);
            }
            continue;
        }

        x += w;
        xs.push(x);
        if breakable(i) {
            last_space = Some(i);
        }
        i += 1;
    }

    rows.push((start, len, xs));
    rows
}
