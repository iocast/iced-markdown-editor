use super::export::{expressible, strip_edges};
use super::*;
use crate::doc::{Block, Cell, ColumnAlign, Container, Document, Leaf, Link, ListInfo, Style};
use crate::features::Features;
use pulldown_cmark::Parser;
use std::collections::HashMap;
use std::sync::Arc;

fn html(markdown: &str) -> String {
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, Parser::new_ext(markdown, options(Features::ALL)));
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
    368, 372, 388, 406, 407, 408, 416, 417, 418, 424, 425, 426, 431, 460, 462, 463, 464, 465, 467,
];

/// Different formats on the same text whose nesting order changes,
/// e.g. `<del>*foo*</del>` is written as `*~~foo~~*`. The model stores
/// formats per character, not their order; the rendering looks the same.
const FORMAT_ORDER: &[u32] = &[168];

fn examples() -> Vec<Example> {
    serde_json::from_str(include_str!("../../tests/commonmark-0.31.2.json")).unwrap()
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

#[test]
fn disabled_features_stay_text() {
    let source = "~~a~~ <del>b</del>\n\n- [ ] t\n\n| a |\n| - |\n";
    let doc = parse_with(source, Features::COMMONMARK);
    assert!(doc.blocks.iter().all(|block| block.cell().is_none()));
    assert!(
        doc.blocks
            .iter()
            .all(|block| block.containers.iter().all(|c| c.task().is_none()))
    );
    assert!(doc.blocks[0].styles.iter().all(|style| !style.strike));
    assert_eq!(doc.blocks[0].text(), "~~a~~ <del>b</del>");
    assert_eq!(doc.blocks[1].text(), "[ ] t");

    let doc = parse_with(source, Features::ALL);
    assert!(doc.blocks[0].styles.iter().any(|style| style.strike));
    assert_eq!(doc.blocks[1].containers[0].task(), Some(false));
    assert!(doc.blocks[2].cell().is_some());
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
    "a", "b", "c", " ", "x", "y", "z", "1", "2", "3", "*", "_", "[", "]", "#", "<", ">", "!", "-",
    "+", "=", ".", ")", "`", "\\", "&", "~", "|", "  ", "\t", "\u{2028}", "\n", "\u{a0}", "[ ] ",
    "[x] ",
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
