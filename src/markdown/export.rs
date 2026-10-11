//! Export of blocks: containers, list markers, blank lines between blocks.

use super::inline::inline_checked;
use super::is_break;
use super::table::table_lines;
use crate::doc::{Block, Container, Document, Leaf, ListInfo};
use std::collections::HashMap;

/// Hard breaks at the start or end of a paragraph cannot be expressed.
pub(super) fn strip_edges(block: &Block) -> Block {
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
pub(super) fn expressible(blocks: &[Block]) -> Vec<Block> {
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
