//! GFM tables: column widths, rows across page breaks, cell lines.

use super::images::{block_images, place_images, row_height};
use super::{CellBox, Decor, IMAGE_GAP, ImageLookup, Line, Pass, Row, metrics, wrap};
use crate::doc::ColumnAlign;

const CELL_PAD_X: f32 = 8.0;
const CELL_PAD_Y: f32 = 5.0;

impl Pass<'_> {
    /// The table occupying the blocks `start..end`, `left`..`left + width`.
    pub(super) fn table(
        &mut self,
        (start, end): (usize, usize),
        indent: &[f32],
        left: f32,
        width: f32,
        images: ImageLookup<'_>,
    ) {
        let doc = self.doc;
        let p = self.style.palette;
        let z = self.z;
        let m = metrics(doc.blocks[start].leaf, self.style, z);
        let pad_x = CELL_PAD_X * z;
        let pad_y = CELL_PAD_Y * z;
        let border = z.max(1.0);
        let max_image_height = self.flow.usable_height() - m.line_height - 2.0 * pad_y;
        let rows = doc.table_rows(start, end);
        let columns = rows.iter().map(|(a, b)| b - a).max().unwrap_or(1);

        // Natural column widths, scaled down when the table is too wide.
        let mut natural = vec![2.0 * pad_x + m.size; columns];
        for &(first, last) in &rows {
            for (column, index) in (first..last).enumerate() {
                let block = &doc.blocks[index];
                let (sizes, _) = block_images(block, images, z, width, max_image_height);
                let text = wrap(block, self.fonts, &m, f32::INFINITY, &sizes)
                    .iter()
                    .map(|(_, _, xs)| xs.last().copied().unwrap_or(0.0))
                    .fold(0.0, f32::max);
                natural[column] = natural[column].max(text + 2.0 * pad_x + border);
            }
        }
        let total: f32 = natural.iter().sum();
        let scale = if total > width { width / total } else { 1.0 };
        let widths: Vec<f32> = natural.iter().map(|w| w * scale).collect();
        let mut column_x = vec![left];
        for w in &widths {
            column_x.push(column_x[column_x.len() - 1] + w);
        }
        let table_width = column_x[columns] - left;
        let mut previous_page = None;

        for (r, &(first, last)) in rows.iter().enumerate() {
            let mut cells = Vec::new();
            let mut content = m.line_height;
            for (column, index) in (first..last).enumerate() {
                let block = &doc.blocks[index];
                let inner = (widths[column] - 2.0 * pad_x).max(1.0);
                let (sizes, runs) = block_images(block, images, z, inner, max_image_height);
                let lines: Vec<(Row, f32)> = wrap(block, self.fonts, &m, inner, &sizes)
                    .into_iter()
                    .map(|row| {
                        let height = row_height(&sizes, row.0, row.1, &m, z).0;
                        (row, height)
                    })
                    .collect();
                content = content.max(lines.iter().map(|(_, h)| h).sum());
                cells.push((index, column, lines, runs));
            }

            let row_height = content + 2.0 * pad_y;
            self.flow.fit(row_height);
            let top = self.flow.y;
            let bottom = top + row_height;

            if doc.blocks[first].cell().is_some_and(|cell| cell.header) {
                self.layout.decor.push(Decor::Rect {
                    x: left,
                    y: top,
                    w: table_width,
                    h: row_height,
                    color: p.code_background,
                });
            }
            // The top line once per page; rows below share the line above.
            let lines = if r == 0 || previous_page != Some(self.flow.page) {
                &[top, bottom - border][..]
            } else {
                &[bottom - border][..]
            };
            previous_page = Some(self.flow.page);
            for y in lines {
                self.layout.decor.push(Decor::Rect {
                    x: left,
                    y: *y,
                    w: table_width,
                    h: border,
                    color: p.rule,
                });
            }
            for (column, x) in column_x.iter().enumerate() {
                self.layout.decor.push(Decor::Rect {
                    x: if column == columns { x - border } else { *x },
                    y: top,
                    w: border,
                    h: row_height,
                    color: p.rule,
                });
            }
            self.quote_bars(&doc.blocks[first], indent, r == 0, top, bottom);
            if r == 0 {
                self.markers(start, indent, &m, top + pad_y);
            }

            for (index, column, lines, runs) in cells {
                let align = doc.blocks[index]
                    .cell()
                    .map(|cell| cell.align)
                    .unwrap_or_default();
                let inner = widths[column] - 2.0 * pad_x;
                let count = lines.len();
                let mut y = top + pad_y;
                for (k, ((line_start, line_end, xs), height)) in lines.into_iter().enumerate() {
                    let used = xs.last().copied().unwrap_or(0.0);
                    let shift = match align {
                        ColumnAlign::Center => ((inner - used) / 2.0).max(0.0),
                        ColumnAlign::Right => (inner - used).max(0.0),
                        ColumnAlign::None | ColumnAlign::Left => 0.0,
                    };
                    let x0 = column_x[column] + pad_x + shift;
                    let line_images = place_images(
                        &mut self.layout,
                        &runs,
                        (line_start, line_end),
                        &xs,
                        x0,
                        y + height - IMAGE_GAP * z,
                    );
                    self.layout.lines.push(Line {
                        block: index,
                        start: line_start,
                        end: line_end,
                        xs: xs.into_iter().map(|x| x + x0).collect(),
                        y,
                        height,
                        size: m.size,
                        color: m.color,
                        last: k + 1 == count,
                        page: self.flow.page,
                        images: line_images,
                        cell: Some(CellBox {
                            row: self.rows,
                            x: column_x[column],
                            width: widths[column],
                            top,
                            bottom,
                        }),
                    });
                    y += height;
                }
            }
            self.rows += 1;
            self.flow.y = bottom;
        }
    }
}
