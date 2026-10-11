//! Export of GFM tables.

use super::inline::inline_md;
use super::{is_break, parse};
use crate::doc::{Block, ColumnAlign};

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
pub(super) fn table_lines(cells: &[Block]) -> String {
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
