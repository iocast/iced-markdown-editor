//! CommonMark import and export.
//!
//! Import uses `pulldown-cmark`, a CommonMark 0.31.2 parser, with three GFM
//! extensions: `~~strikethrough~~`, task lists (`- [ ]` / `- [x]`) and
//! tables.
//! Export writes Markdown that parses back to the same document; the round
//! trip is tested against all 652 examples of the CommonMark 0.31.2
//! specification and against random documents (see `tests.rs`).
//!
//! Inline formatting is written with Markdown delimiters (`**`, `*`, `~~`)
//! when they survive a round trip. CommonMark's delimiter rules cannot
//! express every combination (e.g. bold text that starts with punctuation
//! directly after a letter), so such paragraphs fall back to
//! `<strong>`, `<em>` and `<del>` tags, which the importer recognises.
//!
//! * `import`: Markdown to a document.
//! * `inline`: inline content to Markdown.
//! * `export`: blocks and containers to Markdown.
//! * `table`: GFM tables to Markdown.

mod export;
mod import;
mod inline;
mod table;
#[cfg(test)]
mod tests;

pub use export::serialize;
pub use import::{parse, parse_with};
pub(crate) use inline::image_markdown;

use crate::doc::{Flag, SOFT};
use crate::features::Features;
use pulldown_cmark::Options;

fn options(features: Features) -> Options {
    let mut options = Options::empty();
    options.set(Options::ENABLE_STRIKETHROUGH, features.strikethrough);
    options.set(Options::ENABLE_TASKLISTS, features.task_lists);
    options.set(Options::ENABLE_TABLES, features.tables);
    options
}

fn is_break(c: char) -> bool {
    c == '\n' || c == SOFT
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

/// Parses and writes Markdown again, in the form the editor saves it.
pub fn normalize(markdown: &str) -> String {
    serialize(&parse(markdown))
}
