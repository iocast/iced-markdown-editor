//! The context menu opened with the right mouse button.
//!
//! With a selection it offers clipboard and inline formatting; with the
//! caret in a table (no selection) it offers row, column and alignment
//! commands. The menu is drawn in a layer above the page canvas.

use super::toolbar::button_style;
use super::{Action, Command, EditorState, Inner, Mode};
use crate::doc::ColumnAlign;
use crate::fonts;
use crate::i18n;
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, opaque, pin, row, text};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Shadow, Size, Theme, Vector,
};

/// Width of the menu.
const WIDTH: f32 = 280.0;
/// Height of one entry.
const ITEM: f32 = 26.0;
/// Height of a separator (line and space around it).
const SEPARATOR: f32 = 9.0;
/// Inner padding of the menu.
const PADDING: f32 = 4.0;

enum Entry {
    Item {
        label: String,
        hint: Option<String>,
        checked: bool,
        command: Command,
    },
    Separator,
}

/// Shortcut text for Ctrl/Cmd + `key`.
fn shortcut(key: char) -> Option<String> {
    Some(if cfg!(target_os = "macos") {
        format!("\u{2318}{key}")
    } else {
        format!("Ctrl+{key}")
    })
}

/// The entries for the current selection.
fn entries(state: &EditorState) -> Vec<Entry> {
    let tr = |id| i18n::text(state.language, id);
    let item = |id, hint: Option<String>, command| Entry::Item {
        label: tr(id),
        hint,
        checked: false,
        command,
    };
    let (a, b) = state.sel.range();
    let selected = a != b;
    let format = state.formatting();
    let features = state.features;
    let mut entries = Vec::new();

    if selected {
        entries.push(item("menu-cut", shortcut('X'), Command::Cut));
        entries.push(item("menu-copy", shortcut('C'), Command::Copy));
    }
    entries.push(item("menu-paste", shortcut('V'), Command::Paste));
    entries.push(item("menu-select-all", shortcut('A'), Command::SelectAll));

    if selected {
        entries.push(Entry::Separator);
        entries.push(item("menu-bold", shortcut('B'), Command::ToggleBold));
        entries.push(item("menu-italic", shortcut('I'), Command::ToggleItalic));
        if features.strikethrough {
            entries.push(item("menu-strike", shortcut('D'), Command::ToggleStrike));
        }
        entries.push(item("menu-code", shortcut('E'), Command::ToggleCode));
        entries.push(item("menu-link", shortcut('K'), Command::EditLink));
    } else if format.table && features.tables && state.mode == Mode::Wysiwyg {
        entries.push(Entry::Separator);
        entries.push(item(
            "menu-row-above",
            None,
            Command::InsertRow { below: false },
        ));
        entries.push(item(
            "menu-row-below",
            None,
            Command::InsertRow { below: true },
        ));
        entries.push(item(
            "menu-column-left",
            None,
            Command::InsertColumn { right: false },
        ));
        entries.push(item(
            "menu-column-right",
            None,
            Command::InsertColumn { right: true },
        ));
        entries.push(Entry::Separator);
        entries.push(item("menu-delete-row", None, Command::DeleteRow));
        entries.push(item("menu-delete-column", None, Command::DeleteColumn));
        entries.push(item("menu-delete-table", None, Command::DeleteTable));
        entries.push(Entry::Separator);
        let current = format.column_align.unwrap_or_default();
        for (id, align) in [
            ("menu-align-left", ColumnAlign::Left),
            ("menu-align-center", ColumnAlign::Center),
            ("menu-align-right", ColumnAlign::Right),
        ] {
            entries.push(Entry::Item {
                label: tr(id),
                hint: None,
                checked: current == align,
                command: Command::SetColumnAlign(align),
            });
        }
    }
    entries
}

/// Top-left corner of a menu of `size` opened at `at`, kept inside `area`.
/// It opens below and to the right of the pointer, and flips up or left
/// when there is no room.
fn place(at: Point, size: Size, area: Size) -> Point {
    let x = if at.x + size.width <= area.width {
        at.x
    } else {
        (area.width - size.width).max(0.0)
    };
    let y = if at.y + size.height <= area.height {
        at.y
    } else if at.y >= size.height {
        at.y - size.height
    } else {
        (area.height - size.height).max(0.0)
    };
    Point::new(x, y)
}

/// The menu layer: covers the page, the menu is pinned at `at` (widget
/// coordinates of the page).
pub(crate) fn view(state: &EditorState, at: Point) -> Element<'static, Action> {
    let palette = state.style.palette;
    let entries = entries(state);
    let visual = state.mode == Mode::Wysiwyg;

    let height = 2.0 * PADDING
        + entries
            .iter()
            .map(|entry| match entry {
                Entry::Item { .. } => ITEM,
                Entry::Separator => SEPARATOR,
            })
            .sum::<f32>();
    let position = place(at, Size::new(WIDTH, height), state.viewport.get());

    let mut list = column![].width(Length::Fill);
    for entry in entries {
        list = list.push(match entry {
            Entry::Item {
                label,
                hint,
                checked,
                command,
            } => {
                let check = text(if checked { "\u{2713}" } else { "" })
                    .size(13)
                    .font(fonts::SANS)
                    .width(Length::Fixed(18.0));
                let name = text(label)
                    .size(13)
                    .font(fonts::SANS)
                    .wrapping(Wrapping::None)
                    .width(Length::Fill);
                let mut content = row![check, name]
                    .height(Length::Fill)
                    .align_y(Alignment::Center);
                if let Some(hint) = hint {
                    content = content.push(
                        text(hint)
                            .size(12)
                            .font(fonts::SANS)
                            .color(palette.muted)
                            .wrapping(Wrapping::None),
                    );
                }
                Element::from(
                    button(content)
                        .on_press(match command {
                            // A dialog, as in word processors (not the link bar).
                            Command::EditLink if visual => Action(Inner::LinkDialog),
                            command => Action::from(command),
                        })
                        .padding([0, 8])
                        .width(Length::Fill)
                        .height(Length::Fixed(ITEM))
                        .style(button_style(palette, false)),
                )
            }
            Entry::Separator => container(
                container(column![])
                    .width(Length::Fill)
                    .height(Length::Fixed(1.0))
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(palette.toolbar_border)),
                        ..container::Style::default()
                    }),
            )
            .padding([(SEPARATOR - 1.0) / 2.0, 6.0])
            .into(),
        });
    }

    let menu = container(list)
        .padding(PADDING)
        .width(Length::Fixed(WIDTH))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(palette.toolbar_background)),
            text_color: Some(palette.toolbar_text),
            border: Border {
                color: palette.toolbar_border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.25,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 3.0),
                blur_radius: 10.0,
            },
            ..container::Style::default()
        });

    pin(opaque(menu)).x(position.x).y(position.y).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_stays_inside_the_page() {
        let area = Size::new(800.0, 600.0);
        let size = Size::new(280.0, 200.0);
        assert_eq!(
            place(Point::new(10.0, 20.0), size, area),
            Point::new(10.0, 20.0)
        );
        // No room below: opens above the pointer.
        assert_eq!(
            place(Point::new(10.0, 500.0), size, area),
            Point::new(10.0, 300.0)
        );
        // No room on the right.
        assert_eq!(
            place(Point::new(700.0, 20.0), size, area),
            Point::new(520.0, 20.0)
        );
        // No room above or below.
        let small = Size::new(800.0, 250.0);
        assert_eq!(
            place(Point::new(0.0, 100.0), size, small),
            Point::new(0.0, 50.0)
        );
    }

    fn commands(state: &EditorState) -> Vec<Command> {
        entries(state)
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Item { command, .. } => Some(command),
                Entry::Separator => None,
            })
            .collect()
    }

    #[test]
    fn selection_offers_formatting() {
        let mut state = EditorState::with_markdown("hello world\n");
        state.execute(Command::SelectAll);
        let commands = commands(&state);
        for command in [
            Command::Cut,
            Command::Copy,
            Command::ToggleBold,
            Command::ToggleItalic,
            Command::ToggleStrike,
            Command::ToggleCode,
            Command::EditLink,
        ] {
            assert!(commands.contains(&command), "{command:?}");
        }
        assert!(!commands.contains(&Command::DeleteRow));
    }

    #[test]
    fn caret_in_a_table_offers_rows_and_columns() {
        let state = EditorState::with_markdown("| a | b |\n| - | - |\n");
        let commands = commands(&state);
        for command in [
            Command::InsertRow { below: false },
            Command::InsertRow { below: true },
            Command::InsertColumn { right: false },
            Command::InsertColumn { right: true },
            Command::DeleteRow,
            Command::DeleteColumn,
            Command::DeleteTable,
            Command::SetColumnAlign(ColumnAlign::Center),
        ] {
            assert!(commands.contains(&command), "{command:?}");
        }
        assert!(!commands.contains(&Command::Cut));
        assert!(!commands.contains(&Command::ToggleBold));
    }

    #[test]
    fn disabled_features_are_hidden() {
        let mut state = EditorState::with_markdown("a\n").features(crate::Features {
            strikethrough: false,
            ..crate::Features::default()
        });
        state.execute(Command::SelectAll);
        assert!(!commands(&state).contains(&Command::ToggleStrike));
    }
}
