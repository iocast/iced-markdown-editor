//! The built-in formatting toolbar.
//!
//! All text uses the bundled fonts explicitly, so labels render even where
//! no system fonts exist (the browser).

use super::{Action, Command, EditorState, Formatting, Mode};
use crate::doc::{BlockKind, ColumnAlign};
use crate::fonts;
use crate::i18n::{self, Language};
use crate::style::Palette;
use iced::widget::{button, pick_list, row, text};
use iced::{Alignment, Background, Border, Color, Element, Font, Length};

/// An empty element that takes all remaining width.
pub(crate) fn fill<'a>() -> Element<'a, Action> {
    text("").width(Length::Fill).into()
}

/// Button style. `on` marks an active format or mode.
pub(crate) fn button_style(
    palette: Palette,
    on: bool,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            button::Status::Disabled => None,
            button::Status::Hovered | button::Status::Pressed if !on => {
                Some(Background::Color(palette.toolbar_hover))
            }
            _ if on => Some(Background::Color(palette.toolbar_active)),
            _ => None,
        };
        let text_color = match status {
            button::Status::Disabled => Color {
                a: 0.4,
                ..palette.toolbar_text
            },
            _ if on => palette.accent,
            _ => palette.toolbar_text,
        };
        button::Style {
            background,
            text_color,
            border: Border {
                color: if on {
                    palette.accent
                } else {
                    Color::TRANSPARENT
                },
                width: if on { 1.0 } else { 0.0 },
                radius: 4.0.into(),
            },
            ..button::Style::default()
        }
    }
}

pub(crate) fn tool(
    palette: Palette,
    label: impl Into<String>,
    font: Font,
    on: bool,
    command: Option<Command>,
) -> Element<'static, Action> {
    let label: String = label.into();
    button(text(label).size(13).font(font))
        .padding([5, 9])
        .style(button_style(palette, on))
        .on_press_maybe(command.map(Action::from))
        .into()
}

fn separator(palette: Palette) -> Element<'static, Action> {
    text("|")
        .size(16)
        .font(fonts::SANS)
        .color(palette.toolbar_border)
        .into()
}

/// Language selector shown while the caret is in a code block.
fn language_list(
    format: &Formatting,
    languages: Vec<String>,
    ui: Language,
) -> Element<'static, Action> {
    // Entry that means "no language".
    let plain = i18n::text(ui, "code-plain-text");
    let mut options = vec![plain.clone()];
    options.extend(languages);
    let current = format
        .code_language
        .clone()
        .unwrap_or_else(|| plain.clone());
    if !options.contains(&current) {
        options.push(current.clone());
    }

    pick_list(options, Some(current), move |language: String| {
        let language = (language != plain).then_some(language);
        Action::from(Command::SetCodeLanguage(language))
    })
    .font(fonts::SANS)
    .text_size(12)
    .padding([3, 6])
    .width(Length::Fixed(130.0))
    .into()
}

pub(crate) fn view(state: &EditorState) -> Element<'static, Action> {
    let format = &state.formatting();
    let features = state.features;
    let (can_undo, can_redo) = (state.can_undo(), state.can_redo());
    let visual = state.mode == Mode::Wysiwyg;
    let block = format.block;
    let ui = fonts::SANS;
    let p = state.style.palette;
    let task = format.task.is_some();
    let set = |kind| Some(Command::SetBlock(kind));
    let tr = |id| i18n::text(state.language, id);

    let mut bar = row![
        tool(p, "\u{21B6}", ui, false, can_undo.then_some(Command::Undo)),
        tool(p, "\u{21B7}", ui, false, can_redo.then_some(Command::Redo)),
        separator(p),
        tool(
            p,
            tr("toolbar-normal"),
            ui,
            visual && block == BlockKind::Paragraph,
            set(BlockKind::Paragraph)
        ),
        tool(
            p,
            "H1",
            fonts::SANS_BOLD,
            visual && block == BlockKind::Heading(1),
            set(BlockKind::Heading(1))
        ),
        tool(
            p,
            "H2",
            fonts::SANS_BOLD,
            visual && block == BlockKind::Heading(2),
            set(BlockKind::Heading(2))
        ),
        tool(
            p,
            "H3",
            fonts::SANS_BOLD,
            visual && block == BlockKind::Heading(3),
            set(BlockKind::Heading(3))
        ),
        separator(p),
        tool(
            p,
            tr("toolbar-bold"),
            fonts::SANS_BOLD,
            visual && format.bold,
            Some(Command::ToggleBold)
        ),
        tool(
            p,
            tr("toolbar-italic"),
            fonts::SANS_ITALIC,
            visual && format.italic,
            Some(Command::ToggleItalic)
        ),
        tool(
            p,
            "</>",
            fonts::MONO,
            visual && format.code,
            Some(Command::ToggleCode)
        ),
        tool(
            p,
            tr("toolbar-link"),
            ui,
            visual && format.link.is_some(),
            Some(Command::EditLink)
        ),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    if features.strikethrough {
        bar = bar.push(tool(
            p,
            tr("toolbar-strike"),
            ui,
            visual && format.strike,
            Some(Command::ToggleStrike),
        ));
    }

    if state.image_settings.enabled() {
        bar = bar.push(tool(
            p,
            tr("toolbar-image"),
            ui,
            false,
            Some(Command::RequestImage),
        ));
    }

    bar = bar
        .push(separator(p))
        .push(tool(
            p,
            "\u{2022} \u{2014}",
            ui,
            visual && !task && format.list == Some(false),
            set(BlockKind::Bullet),
        ))
        .push(tool(
            p,
            "1. \u{2014}",
            ui,
            visual && !task && format.list == Some(true),
            set(BlockKind::Ordered),
        ));

    if features.task_lists {
        bar = bar.push(tool(
            p,
            "\u{2611} \u{2014}",
            ui,
            visual && task,
            set(BlockKind::Task),
        ));
    }

    bar = bar
        .push(tool(
            p,
            "\u{201C} \u{201D}",
            ui,
            visual && format.quote,
            set(BlockKind::Quote),
        ))
        .push(tool(
            p,
            "{ }",
            ui,
            visual && block == BlockKind::Code,
            set(BlockKind::Code),
        ));

    if visual && block == BlockKind::Code {
        bar = bar.push(language_list(
            format,
            state.code_languages(),
            state.language,
        ));
    }

    bar = bar.push(tool(p, "\u{2015}", ui, false, Some(Command::InsertRule)));

    if features.tables {
        bar = bar.push(tool(
            p,
            tr("toolbar-table"),
            ui,
            visual && format.table,
            (!format.table).then_some(Command::InsertTable {
                rows: 3,
                columns: 3,
            }),
        ));
    }

    if visual && format.table && features.tables {
        let align = format.column_align.unwrap_or_default();
        bar = bar
            .push(tool(
                p,
                tr("toolbar-add-row"),
                ui,
                false,
                Some(Command::InsertRow { below: true }),
            ))
            .push(tool(
                p,
                tr("toolbar-add-column"),
                ui,
                false,
                Some(Command::InsertColumn { right: true }),
            ))
            .push(tool(
                p,
                tr("toolbar-delete-row"),
                ui,
                false,
                Some(Command::DeleteRow),
            ))
            .push(tool(
                p,
                tr("toolbar-delete-column"),
                ui,
                false,
                Some(Command::DeleteColumn),
            ))
            .push(tool(
                p,
                "\u{21E4}",
                ui,
                align == ColumnAlign::Left,
                Some(Command::SetColumnAlign(ColumnAlign::Left)),
            ))
            .push(tool(
                p,
                "\u{2194}",
                ui,
                align == ColumnAlign::Center,
                Some(Command::SetColumnAlign(ColumnAlign::Center)),
            ))
            .push(tool(
                p,
                "\u{21E5}",
                ui,
                align == ColumnAlign::Right,
                Some(Command::SetColumnAlign(ColumnAlign::Right)),
            ));
    }

    bar = bar
        .push(separator(p))
        .push(tool(
            p,
            "WYSIWYG",
            ui,
            visual,
            Some(Command::SetMode(Mode::Wysiwyg)),
        ))
        .push(tool(
            p,
            tr("toolbar-source"),
            ui,
            !visual,
            Some(Command::SetMode(Mode::Source)),
        ));

    // Narrow editors: buttons continue on the next line instead of being cut off.
    bar.wrap().into()
}
