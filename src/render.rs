//! Drawing pages, text, images, selection and caret.
//!
//! All functions draw in document coordinates; the view translates the
//! frame by the scroll offset first.

use crate::doc::{Document, Leaf, OBJ, SOFT, Selection};
use crate::fonts;
use crate::layout::{Decor, Layout, face_for};
use crate::style::Palette;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::widget::text::{LineHeight, Shaping};
use iced::{Color, Pixels, Point, Size};
use std::sync::Arc;

/// Per-character colours of a highlighted code block, by block index.
pub type CodeColors<'a> = &'a dyn Fn(usize) -> Option<Arc<Vec<Color>>>;

/// Approximate baseline of a line (glyphs are centred in the line box).
fn baseline(y: f32, line_height: f32, size: f32) -> f32 {
    y + line_height / 2.0 + size * 0.32
}

fn outline(frame: &mut Frame, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let path = Path::rectangle(Point::new(x, y), Size::new(w, h));
    frame.stroke(&path, Stroke::default().with_width(1.0).with_color(color));
}

/// Task list checkbox: empty frame, or filled with a check mark.
fn checkbox(frame: &mut Frame, x: f32, y: f32, size: f32, checked: bool, palette: &Palette) {
    let stroke = (size / 12.0).max(1.0);
    let rect = Path::rectangle(Point::new(x, y), Size::new(size, size));
    if checked {
        frame.fill(&rect, palette.accent);
        let check = Path::new(|path| {
            path.move_to(Point::new(x + size * 0.22, y + size * 0.52));
            path.line_to(Point::new(x + size * 0.42, y + size * 0.72));
            path.line_to(Point::new(x + size * 0.78, y + size * 0.30));
        });
        frame.stroke(
            &check,
            Stroke::default()
                .with_width(stroke * 1.6)
                .with_color(palette.page),
        );
    } else {
        frame.fill(&rect, palette.page);
        frame.stroke(
            &rect,
            Stroke::default()
                .with_width(stroke)
                .with_color(palette.marker),
        );
    }
}

/// Draws a loaded image (needs the `images` feature).
#[cfg(feature = "images")]
fn image(frame: &mut Frame, x: f32, y: f32, w: f32, h: f32, image: &crate::images::LoadedImage) {
    frame.draw_image(
        iced::Rectangle::new(Point::new(x, y), Size::new(w, h)),
        iced::widget::canvas::Image::new(image.handle.clone()),
    );
}

#[cfg(not(feature = "images"))]
fn image(_: &mut Frame, _: f32, _: f32, _: f32, _: f32, _: &crate::images::LoadedImage) {}

/// The area behind the pages (in widget coordinates, not scrolled).
pub fn draw_desk(frame: &mut Frame, layout: &Layout, palette: &Palette, size: Size) {
    let color = if layout.fill {
        palette.page
    } else {
        palette.background
    };
    frame.fill_rectangle(Point::ORIGIN, size, color);
}

/// The sheets of paper.
pub fn draw_pages(frame: &mut Frame, layout: &Layout, palette: &Palette) {
    if layout.fill {
        return;
    }
    for page in &layout.pages {
        frame.fill_rectangle(
            Point::new(page.x + 2.0, page.y + 3.0),
            Size::new(page.width, page.height),
            palette.page_shadow,
        );
        frame.fill_rectangle(
            Point::new(page.x - 1.0, page.y - 1.0),
            Size::new(page.width + 2.0, page.height + 2.0),
            palette.page_border,
        );
        frame.fill_rectangle(
            Point::new(page.x, page.y),
            Size::new(page.width, page.height),
            palette.page,
        );
    }
}

/// Text, decorations and images.
pub fn draw_document(
    frame: &mut Frame,
    doc: &Document,
    layout: &Layout,
    palette: &Palette,
    code_colors: CodeColors<'_>,
    visible: (f32, f32),
) {
    let (top, bottom) = visible;

    for decor in &layout.decor {
        if let Decor::Rect { x, y, w, h, color } = decor {
            if *y + *h >= top && *y <= bottom {
                frame.fill_rectangle(Point::new(*x, *y), Size::new(*w, *h), *color);
            }
        }
    }

    for line in &layout.lines {
        if line.y + line.height < top || line.y > bottom {
            continue;
        }
        let block = &doc.blocks[line.block];

        // Inline code backgrounds.
        if !block.leaf.is_raw() {
            let mut j = line.start;
            while j < line.end {
                if !block.styles[j].code || block.chars[j] == '\n' {
                    j += 1;
                    continue;
                }
                let mut k = j + 1;
                while k < line.end && block.styles[k].code && block.chars[k] != '\n' {
                    k += 1;
                }
                let x0 = line.xs[j - line.start];
                let x1 = line.xs[k - line.start];
                let pad = line.height * 0.09;
                frame.fill_rectangle(
                    Point::new(x0 - 2.0, line.y + pad),
                    Size::new(x1 - x0 + 4.0, line.height - 2.0 * pad),
                    palette.inline_code_background,
                );
                j = k;
            }
        }

        let colors = if block.leaf == Leaf::Code {
            code_colors(line.block)
        } else {
            None
        };
        let highlight = |i: usize| colors.as_ref().and_then(|colors| colors.get(i).copied());
        let mut j = line.start;

        while j < line.end {
            if let Some(&(_, end)) = line.images.iter().find(|(s, e)| j >= *s && j < *e) {
                j = end;
                continue;
            }

            let c = block.chars[j];
            if c == '\n' {
                j += 1;
                continue;
            }
            let style = &block.styles[j];

            if c == OBJ {
                let x0 = line.xs[j - line.start];
                let x1 = line.xs[j + 1 - line.start];
                let inset = line.height * 0.22;
                let color = if style.link.is_some() {
                    palette.link
                } else {
                    palette.muted
                };
                outline(
                    frame,
                    x0 + 1.0,
                    line.y + inset,
                    x1 - x0 - 2.0,
                    line.height - 2.0 * inset,
                    color,
                );
                j += 1;
                continue;
            }

            let run_color = highlight(j);
            let mut k = j + 1;
            while k < line.end
                && !matches!(block.chars[k], '\n' | OBJ)
                && block.styles[k] == *style
                && highlight(k) == run_color
                && !line.images.iter().any(|(s, _)| *s == k)
            {
                k += 1;
            }

            let content: String = block.chars[j..k]
                .iter()
                .map(|c| match *c {
                    SOFT => " ".to_string(),
                    '\t' => "    ".to_string(),
                    c => c.to_string(),
                })
                .collect();
            let x0 = line.xs[j - line.start];
            let x1 = line.xs[k - line.start];
            let color = if style.link.is_some() {
                palette.link
            } else if style.image.is_some() || style.html {
                palette.muted
            } else {
                run_color.unwrap_or(line.color)
            };

            frame.fill_text(Text {
                content,
                position: Point::new(x0, line.y),
                color,
                size: Pixels(line.size),
                line_height: LineHeight::Absolute(Pixels(line.height)),
                font: face_for(block.leaf, style).font(),
                shaping: Shaping::Basic,
                ..Text::default()
            });

            let base = baseline(line.y, line.height, line.size);
            let thickness = (line.size / 14.0).max(1.0);
            if style.link.is_some() {
                frame.fill_rectangle(
                    Point::new(x0, base + line.size * 0.12),
                    Size::new(x1 - x0, thickness),
                    palette.link,
                );
            }
            if style.strike {
                frame.fill_rectangle(
                    Point::new(x0, base - line.size * 0.3),
                    Size::new(x1 - x0, thickness),
                    color,
                );
            }
            if style.image.is_some() {
                let inset = line.height * 0.08;
                outline(
                    frame,
                    x0 - 2.0,
                    line.y + inset,
                    x1 - x0 + 4.0,
                    line.height - 2.0 * inset,
                    palette.muted,
                );
            }

            j = k;
        }
    }

    for decor in &layout.decor {
        match decor {
            Decor::Marker {
                text,
                x,
                y,
                size,
                height,
                color,
            } => {
                frame.fill_text(Text {
                    content: text.clone(),
                    position: Point::new(*x, *y),
                    color: *color,
                    size: Pixels(*size),
                    line_height: LineHeight::Absolute(Pixels(*height)),
                    font: fonts::SANS,
                    shaping: Shaping::Basic,
                    ..Text::default()
                });
            }
            Decor::Checkbox {
                x,
                y,
                size,
                checked,
            } => checkbox(frame, *x, *y, *size, *checked, palette),
            Decor::Image {
                x,
                y,
                w,
                h,
                image: loaded,
            } => image(frame, *x, *y, *w, *h, loaded),
            Decor::Rect { .. } => {}
        }
    }
}

/// Selection highlight.
pub fn draw_selection(frame: &mut Frame, layout: &Layout, selection: Selection, palette: &Palette) {
    if selection.is_collapsed() {
        return;
    }
    let (a, b) = selection.range();

    for line in &layout.lines {
        if line.block < a.block || line.block > b.block {
            continue;
        }
        let start = if line.block == a.block {
            a.offset.max(line.start)
        } else {
            line.start
        };
        let end = if line.block == b.block {
            b.offset.min(line.end)
        } else {
            line.end
        };
        if start > end {
            continue;
        }
        if line.block == a.block && a.offset >= line.end && !line.last {
            continue;
        }

        let continues =
            line.block < b.block || b.offset > line.end || (b.offset == line.end && !line.last);
        let x0 = line.xs[start - line.start];
        let x1 = line.xs[end - line.start];
        let extra = if continues { line.size * 0.4 } else { 0.0 };
        let width = x1 - x0 + extra;

        if width > 0.5 {
            frame.fill_rectangle(
                Point::new(x0, line.y),
                Size::new(width, line.height),
                palette.selection,
            );
        }
    }
}

/// The caret.
pub fn draw_caret(frame: &mut Frame, layout: &Layout, selection: Selection, palette: &Palette) {
    if !selection.is_collapsed() {
        return;
    }
    if let Some((x, y, height)) = layout.caret(selection.head) {
        let inset = height * 0.1;
        frame.fill_rectangle(
            Point::new(x - 0.5, y + inset),
            Size::new(1.5, height - 2.0 * inset),
            palette.caret,
        );
    }
}
