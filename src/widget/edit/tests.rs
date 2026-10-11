use super::*;
use crate::doc::ColumnAlign;
use crate::extension::{BlockRenderer, Frame, Rectangle, RenderRequest, Size};
use crate::features::Features;
use crate::images::{ImageRejection, ImageSettings, PastedImage};
use crate::layout::Decor;
use crate::markdown::{parse, serialize};

fn round(doc: &Document) -> String {
    serialize(doc)
}

/// A 1×1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0,
    0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[test]
fn tab_nests_and_shift_tab_unnests_list_items() {
    let mut doc = parse("- a\n- b\n- c\n");
    assert!(nest_item(&mut doc, 1));
    assert_eq!(round(&doc), "- a\n  - b\n- c\n");
    assert!(nest_item(&mut doc, 2));
    assert_eq!(round(&doc), "- a\n  - b\n  - c\n");
    assert!(unnest_item(&mut doc, 1));
    assert_eq!(round(&doc), "- a\n- b\n  - c\n");
    assert!(unnest_item(&mut doc, 0));
    assert_eq!(round(&doc), "a\n\n- b\n  - c\n");
}

#[test]
fn first_item_cannot_be_nested() {
    let mut doc = parse("- a\n- b\n");
    assert!(!nest_item(&mut doc, 0));
}

#[test]
fn enter_in_list_creates_new_item() {
    let mut doc = parse("- ab\n");
    let pos = doc.split_block(Pos::new(0, 1));
    assert_eq!(pos, Pos::new(1, 0));
    assert_eq!(round(&doc), "- a\n- b\n");
}

#[test]
fn enter_in_task_creates_unchecked_task() {
    let mut doc = parse("- [x] ab\n");
    doc.split_block(Pos::new(0, 1));
    assert_eq!(round(&doc), "- [x] a\n- [ ] b\n");
}

#[test]
fn nesting_keeps_task_state() {
    let mut doc = parse("- [ ] a\n- [x] b\n");
    assert!(nest_item(&mut doc, 1));
    assert_eq!(round(&doc), "- [ ] a\n  - [x] b\n");
    assert!(unnest_item(&mut doc, 1));
    assert_eq!(round(&doc), "- [ ] a\n- [x] b\n");
}

#[test]
fn wrap_and_join_lists_and_quotes() {
    let mut doc = parse("- a\n\nb\n");
    wrap_list(&mut doc, 1, 1, false, 1, None);
    assert_eq!(round(&doc), "- a\n- b\n");
    let mut doc = parse("x\n\ny\n");
    wrap_quote(&mut doc, 0, 1);
    assert_eq!(round(&doc), "> x\n>\n> y\n");
    let mut doc = parse("x\n");
    wrap_list(&mut doc, 0, 0, false, 1, Some(false));
    assert_eq!(round(&doc), "- [ ] x\n");
}

#[test]
fn toggle_task_state() {
    let mut doc = parse("- [ ] a\n");
    doc.set_task(0, 0, Some(true));
    assert_eq!(round(&doc), "- [x] a\n");
    doc.set_task(0, 0, None);
    assert_eq!(round(&doc), "- a\n");
}

fn insert(editor: &mut EditorState, bytes: &[u8]) -> Vec<Event> {
    editor
        .execute(Command::InsertImage {
            bytes: bytes.to_vec(),
            name: Some("photo.png".into()),
        })
        .events
}

fn pasted(events: &[Event]) -> PastedImage {
    events
        .iter()
        .find_map(|event| match event {
            Event::ImagePasted(image) => Some(image.clone()),
            _ => None,
        })
        .expect("no ImagePasted event")
}

#[test]
fn disabled_images_are_rejected() {
    let mut editor = EditorState::with_markdown("text");
    let events = insert(&mut editor, PNG);
    assert!(events.contains(&Event::ImageRejected {
        source: ImageSource::Insert,
        reason: ImageRejection::Disabled,
    }));
    assert_eq!(editor.markdown(), "text\n");
}

#[test]
fn embedded_images_become_data_urls() {
    let mut editor = EditorState::with_markdown("").images(ImageSettings::embed());
    insert(&mut editor, PNG);
    let markdown = editor.markdown();
    assert!(markdown.starts_with("![photo](data:image/png;base64,iVBORw0KGgo"));
}

#[test]
fn embedded_images_are_short_in_source_mode() {
    let url = format!("data:image/png;base64,{}", "A".repeat(500));
    let text = format!("x ![a]({url}) y\n");
    let mut editor = EditorState::with_markdown(&text);
    editor.set_mode(Mode::Source);
    let line = editor.doc.blocks[0].text();
    assert_eq!(line, "x ![a](data:image/png;embedded,1) y");
    assert_eq!(editor.markdown(), text);
    editor.set_mode(Mode::Wysiwyg);
    assert_eq!(editor.markdown(), text);
    // Switching again keeps the same number.
    editor.set_mode(Mode::Source);
    assert_eq!(editor.doc.blocks[0].text(), line);
}

#[test]
fn delegated_images_are_resolved_or_cancelled() {
    let mut editor = EditorState::with_markdown("").images(ImageSettings::delegate());
    let image = pasted(&insert(&mut editor, PNG));
    assert_eq!(image.mime, "image/png");
    assert_eq!(image.extension(), "png");
    assert_eq!(image.file_name(), "photo.png");
    assert_eq!(editor.markdown(), "\n");
    assert_eq!(editor.pending_images(), 1);

    editor.execute(Command::ResolveImage {
        id: image.id,
        url: "assets/photo.png".into(),
        alt: "A photo".into(),
    });
    assert_eq!(editor.markdown(), "![A photo](assets/photo.png)\n");
    assert_eq!(editor.pending_images(), 0);

    editor.execute(Command::Undo);
    editor.execute(Command::Redo);
    assert_eq!(editor.markdown(), "![A photo](assets/photo.png)\n");

    let second = pasted(&insert(&mut editor, PNG));
    editor.execute(Command::CancelImage { id: second.id });
    assert_eq!(editor.markdown(), "![A photo](assets/photo.png)\n");
}

#[test]
fn delegated_images_in_source_mode() {
    let mut editor = EditorState::with_markdown("a")
        .images(ImageSettings::delegate())
        .mode(Mode::Source);
    editor.execute(Command::SelectAll);
    editor.execute(Command::InsertText("x ".into()));
    let image = pasted(&insert(&mut editor, PNG));
    assert_eq!(editor.markdown(), "x ");
    editor.execute(Command::ResolveImage {
        id: image.id,
        url: "a.png".into(),
        alt: String::new(),
    });
    assert_eq!(editor.markdown(), "x ![](a.png)");
}

#[test]
fn rejects_unknown_types_and_large_images() {
    let mut editor = EditorState::new().images(ImageSettings::embed().max_bytes(10));
    let events = insert(&mut editor, b"%PDF-1.7");
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ImageRejected {
            reason: ImageRejection::UnsupportedType(_),
            ..
        }
    )));
    let events = insert(&mut editor, PNG);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ImageRejected {
            reason: ImageRejection::TooLarge { .. },
            ..
        }
    )));
}

#[test]
fn image_button_requests_an_image() {
    let mut editor = EditorState::new().images(ImageSettings::embed());
    let events = editor.execute(Command::RequestImage).events;
    assert!(events.contains(&Event::ImageRequested));
}

#[test]
fn insert_table_and_type_in_cells() {
    let mut editor = EditorState::with_markdown("intro\n");
    editor.sel = Selection::caret(Pos::new(0, 5));
    editor.execute(Command::InsertTable {
        rows: 2,
        columns: 2,
    });
    assert_eq!(editor.sel.head, Pos::new(1, 0));
    editor.execute(Command::InsertText("a".into()));
    editor.execute(Command::Indent);
    editor.execute(Command::InsertText("b".into()));
    editor.execute(Command::Indent);
    editor.execute(Command::InsertText("1\n2".into()));
    assert_eq!(
        editor.markdown(),
        "intro\n\n| a   | b   |\n| --- | --- |\n| 1 2 |     |\n"
    );
    assert_eq!(
        editor.doc.plain_text(Pos::new(1, 0), Pos::new(4, 0)),
        "a\tb\n1 2\t"
    );
}

#[test]
fn tab_moves_between_cells_and_adds_rows() {
    let mut editor = EditorState::with_markdown("| a |\n| - |\n| 1 |\n");
    editor.sel = Selection::caret(Pos::new(1, 1));
    editor.execute(Command::Indent);
    assert_eq!(editor.doc.blocks.len(), 3);
    assert_eq!(editor.sel, Selection::caret(Pos::new(2, 0)));
    editor.execute(Command::Outdent);
    assert_eq!(
        editor.sel,
        Selection {
            anchor: Pos::new(1, 0),
            head: Pos::new(1, 1),
        }
    );
}

#[test]
fn enter_moves_down_adds_rows_and_leaves_the_table() {
    let mut editor = EditorState::with_markdown("| a | b |\n| - | - |\n| 1 | 2 |\n");
    editor.sel = Selection::caret(Pos::new(1, 1));
    let _ = editor.enter(false);
    assert_eq!(editor.sel.head, Pos::new(3, 1));
    let _ = editor.enter(false);
    assert_eq!(editor.doc.blocks.len(), 6);
    assert_eq!(editor.sel.head, Pos::new(5, 0));
    let _ = editor.enter(false);
    assert_eq!(editor.doc.blocks.len(), 5);
    assert_eq!(editor.doc.blocks[4].leaf, Leaf::Paragraph);
    assert_eq!(editor.sel.head, Pos::new(4, 0));
    editor.execute(Command::InsertText("after".into()));
    assert_eq!(
        editor.markdown(),
        "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n\nafter\n"
    );
}

#[test]
fn typing_a_header_row_creates_a_table() {
    let mut editor = EditorState::new();
    editor.execute(Command::InsertText("| Name | Age |".into()));
    let _ = editor.enter(false);
    assert_eq!(editor.sel.head, Pos::new(2, 0));
    editor.execute(Command::InsertText("Ann".into()));
    assert_eq!(
        editor.markdown(),
        "| Name | Age |\n| ---- | --- |\n| Ann  |     |\n"
    );
}

#[test]
fn backspace_and_delete_keep_cells() {
    let mut editor = EditorState::with_markdown("| a | b |\n| - | - |\n\nx\n");
    editor.sel = Selection::caret(Pos::new(1, 0));
    let _ = editor.backspace(false);
    assert_eq!(editor.doc.blocks.len(), 3);
    editor.sel = Selection::caret(Pos::new(0, 1));
    let _ = editor.delete_forward(false);
    assert_eq!(editor.doc.blocks[0].text(), "a");
    // At the start of the paragraph after the table: into the last cell.
    editor.sel = Selection::caret(Pos::new(2, 0));
    let _ = editor.backspace(false);
    assert_eq!(editor.sel.head, Pos::new(1, 1));
    assert_eq!(editor.markdown(), "| a   | b   |\n| --- | --- |\n\nx\n");
    // Block kinds that would split the table do nothing.
    editor.sel = Selection::caret(Pos::new(0, 0));
    editor.execute(Command::SetBlock(BlockKind::Bullet));
    editor.execute(Command::SetBlock(BlockKind::Heading(1)));
    editor.execute(Command::InsertRule);
    assert_eq!(editor.markdown(), "| a   | b   |\n| --- | --- |\n\nx\n");
}

#[test]
fn deleting_across_a_table() {
    let mut doc = parse("before\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\nafter\n");
    let pos = doc.delete_range(Pos::new(2, 0), Pos::new(5, 2));
    assert_eq!(pos, Pos::new(2, 0));
    assert_eq!(
        round(&doc),
        "before\n\n| a   |     |\n| --- | --- |\n|     |     |\n\nter\n"
    );

    let mut doc = parse("before\n\n| a | b |\n| - | - |\n\nafter\n");
    let pos = doc.delete_range(Pos::new(0, 3), Pos::new(3, 2));
    assert_eq!(pos, Pos::new(0, 3));
    assert_eq!(round(&doc), "befter\n");
}

#[test]
fn row_and_column_commands() {
    let mut editor = EditorState::with_markdown("| a | b |\n| - | - |\n| 1 | 2 |\n");
    editor.sel = Selection::caret(Pos::new(0, 0));
    editor.execute(Command::SetColumnAlign(ColumnAlign::Center));
    assert_eq!(editor.formatting().column_align, Some(ColumnAlign::Center));
    editor.execute(Command::InsertColumn { right: true });
    editor.execute(Command::InsertRow { below: false });
    assert_eq!(
        editor.markdown(),
        "|     |     |     |\n| :-: | --- | --- |\n| a   |     | b   |\n| 1   |     | 2   |\n"
    );
    editor.execute(Command::DeleteRow);
    editor.execute(Command::DeleteColumn);
    assert_eq!(
        editor.markdown(),
        "| a   | b   |\n| :-: | --- |\n| 1   | 2   |\n"
    );
    editor.execute(Command::Undo);
    editor.execute(Command::DeleteTable);
    assert_eq!(editor.markdown(), "\n");
    assert!(!editor.formatting().table);
}

#[test]
fn table_layout_and_navigation() {
    let editor = EditorState::with_markdown("| a | b |\n| - | - |\n| 1 | 2 |\n\nend\n");
    let layout = editor.layout.borrow();
    let (ax, ay, _) = layout.caret(Pos::new(0, 0)).unwrap();
    let (bx, by, _) = layout.caret(Pos::new(1, 0)).unwrap();
    let (_, y1, _) = layout.caret(Pos::new(2, 0)).unwrap();
    assert_eq!(ay, by);
    assert!(bx > ax);
    assert!(y1 > ay);
    assert_eq!(layout.hit(bx + 1.0, by + 1.0).block, 1);
    assert_eq!(layout.vertical(Pos::new(1, 0), bx, 1), Some(Pos::new(3, 0)));
    assert_eq!(
        layout.vertical(Pos::new(3, 0), bx, 1).map(|p| p.block),
        Some(4)
    );
    assert_eq!(
        layout.vertical(Pos::new(4, 0), ax, -1).map(|p| p.block),
        Some(2)
    );
}

#[test]
fn insert_table_in_source_mode() {
    let mut editor = EditorState::with_markdown("x").mode(Mode::Source);
    editor.execute(Command::InsertTable {
        rows: 2,
        columns: 2,
    });
    assert_eq!(
        editor.markdown(),
        "x\n\n|     |     |\n| --- | --- |\n|     |     |\n"
    );
}

#[test]
fn scrolling_is_clamped() {
    let text = "line\n\n".repeat(400);
    let mut editor = EditorState::with_markdown(&text);
    editor.update(Action(Inner::ScrollBy { dx: 0.0, dy: 500.0 }));
    assert_eq!(editor.scroll_y, 500.0);
    editor.update(Action(Inner::ScrollBy {
        dx: 0.0,
        dy: -9999.0,
    }));
    assert_eq!(editor.scroll_y, 0.0);
    editor.update(Action(Inner::ScrollTo {
        x: None,
        y: Some(1.0e9),
    }));
    assert_eq!(editor.scroll_y, editor.max_scroll().1);
}

fn type_text(editor: &mut EditorState, text: &str) {
    for c in text.chars() {
        editor.execute(Command::InsertText(c.to_string()));
    }
}

#[test]
fn disabled_features_are_not_created() {
    let mut editor = EditorState::new().features(Features::COMMONMARK);
    type_text(&mut editor, "| a | b |");
    let _ = editor.enter(false);
    type_text(&mut editor, "- [ ] x");
    editor.execute(Command::InsertTable {
        rows: 2,
        columns: 2,
    });
    editor.execute(Command::SetBlock(BlockKind::Task));
    editor.execute(Command::SelectAll);
    editor.execute(Command::ToggleStrike);
    assert_eq!(editor.markdown(), "\\| a \\| b \\|\n\n- \\[ \\] x\n");
    assert!(!Features::COMMONMARK.allows(&Command::ToggleChecked));
    assert!(Features::COMMONMARK.allows(&Command::ToggleBold));
}

#[test]
fn switching_features_reads_the_document_again() {
    let mut editor = EditorState::with_markdown("- [x] done ~~old~~\n\n| a |\n| - |\n");
    editor.set_features(Features {
        task_lists: false,
        tables: false,
        ..Features::ALL
    });
    // The table is written (padded) first, then read as text.
    assert_eq!(
        editor.markdown(),
        "- \\[x\\] done ~~old~~\n\n\\| a   \\|\n\\| --- \\|\n"
    );
    assert!(!editor.formatting().table);
}

/// Draws ```` ```box ```` blocks as a 100 × 50 box.
struct Boxes;

impl BlockRenderer for Boxes {
    fn languages(&self) -> Vec<String> {
        vec!["box".into()]
    }

    fn measure(&self, request: &RenderRequest<'_>) -> Option<Size> {
        (request.code != "invalid").then(|| Size::new(100.0, 50.0))
    }

    fn draw(&self, _: &mut Frame, _: Rectangle, _: &RenderRequest<'_>) {}
}

fn rendered(editor: &EditorState, block: usize) -> bool {
    let layout = editor.layout.borrow();
    let custom = layout
        .decor
        .iter()
        .any(|decor| matches!(decor, Decor::Custom { block: b, .. } if *b == block));
    let lines: Vec<_> = layout
        .lines
        .iter()
        .filter(|line| line.block == block)
        .collect();
    if custom {
        assert_eq!(lines.len(), 1);
        assert_eq!((lines[0].end, lines[0].height), (0, 50.0));
    }
    custom
}

#[test]
fn rendered_blocks_show_code_while_editing() {
    let text = "intro\n\n```box\nx\n```\n\n```box\ninvalid\n```\n\nend\n";
    let mut editor = EditorState::with_markdown(text).block_renderer(Boxes);
    assert!(editor.code_languages().contains(&"box".to_string()));
    assert!(rendered(&editor, 1));
    assert!(!rendered(&editor, 2), "the renderer declined");

    // A click on the block puts the caret in it and shows the code.
    let (x, y) = {
        let layout = editor.layout.borrow();
        let line = layout.lines.iter().find(|line| line.block == 1).unwrap();
        (line.xs[0] + 10.0, line.y + 10.0)
    };
    editor.update(Action(Inner::Press { x, y, clicks: 1 }));
    assert_eq!(editor.sel.head.block, 1);
    assert!(!rendered(&editor, 1));

    editor.execute(Command::InsertText("y".into()));
    editor.update(Action(Inner::Key(
        Key::Named(Named::ArrowDown),
        Modifiers::empty(),
        None,
    )));
    assert_ne!(editor.sel.head.block, 1);
    assert!(rendered(&editor, 1));
    assert_eq!(
        editor.markdown(),
        "intro\n\n```box\nyx\n```\n\n```box\ninvalid\n```\n\nend\n"
    );

    // Source mode shows the Markdown.
    editor.set_mode(Mode::Source);
    assert!(!rendered(&editor, 2));
}
