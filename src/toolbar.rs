//! The built-in formatting toolbar.
//!
//! All text uses the bundled fonts explicitly, so labels render even where
//! no system fonts exist (the browser).

use crate::doc::BlockKind;
use crate::fonts;
use crate::style::Palette;
use crate::widget::{Action, Command, Formatting, Mode};
use iced::widget::{button, pick_list, row, text};
use iced::{Alignment, Background, Border, Color, Element, Font, Length};

/// Entry in the language list that means "no language".
pub(crate) const PLAIN: &str = "plain text";

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
                color: if on { palette.accent } else { Color::TRANSPARENT },
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
fn language_list(format: &Formatting, languages: Vec<String>) -> Element<'static, Action> {
    let mut options = vec![PLAIN.to_string()];
    options.extend(languages);
    let current = format
        .code_language
        .clone()
        .unwrap_or_else(|| PLAIN.to_string());
    if !options.contains(&current) {
        options.push(current.clone());
    }

    pick_list(options, Some(current), |language: String| {
        let language = (language != PLAIN).then_some(language);
        Action::from(Command::SetCodeLanguage(language))
    })
    .font(fonts::SANS)
    .text_size(12)
    .padding([3, 6])
    .width(Length::Fixed(130.0))
    .into()
}

pub(crate) fn view(
    palette: Palette,
    format: &Formatting,
    mode: Mode,
    can_undo: bool,
    can_redo: bool,
    languages: Vec<String>,
    images: bool,
) -> Element<'static, Action> {
    let visual = mode == Mode::Wysiwyg;
    let block = format.block;
    let ui = fonts::SANS;
    let p = palette;
    let task = format.task.is_some();
    let set = |kind| Some(Command::SetBlock(kind));

    let mut bar = row![
        tool(p, "\u{21B6}", ui, false, can_undo.then_some(Command::Undo)),
        tool(p, "\u{21B7}", ui, false, can_redo.then_some(Command::Redo)),
        separator(p),
        tool(p, "Normal", ui, visual && block == BlockKind::Paragraph, set(BlockKind::Paragraph)),
        tool(p, "H1", fonts::SANS_BOLD, visual && block == BlockKind::Heading(1), set(BlockKind::Heading(1))),
        tool(p, "H2", fonts::SANS_BOLD, visual && block == BlockKind::Heading(2), set(BlockKind::Heading(2))),
        tool(p, "H3", fonts::SANS_BOLD, visual && block == BlockKind::Heading(3), set(BlockKind::Heading(3))),
        separator(p),
        tool(p, "B", fonts::SANS_BOLD, visual && format.bold, Some(Command::ToggleBold)),
        tool(p, "I", fonts::SANS_ITALIC, visual && format.italic, Some(Command::ToggleItalic)),
        tool(p, "S", ui, visual && format.strike, Some(Command::ToggleStrike)),
        tool(p, "</>", fonts::MONO, visual && format.code, Some(Command::ToggleCode)),
        tool(p, "Link", ui, visual && format.link.is_some(), Some(Command::EditLink)),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    if images {
        bar = bar.push(tool(p, "Image", ui, false, Some(Command::RequestImage)));
    }

    bar = bar
        .push(separator(p))
        .push(tool(p, "\u{2022} \u{2014}", ui, visual && !task && format.list == Some(false), set(BlockKind::Bullet)))
        .push(tool(p, "1. \u{2014}", ui, visual && !task && format.list == Some(true), set(BlockKind::Ordered)))
        .push(tool(p, "\u{2611} \u{2014}", ui, visual && task, set(BlockKind::Task)))
        .push(tool(p, "\u{201C} \u{201D}", ui, visual && format.quote, set(BlockKind::Quote)))
        .push(tool(p, "{ }", ui, visual && block == BlockKind::Code, set(BlockKind::Code)));

    if visual && block == BlockKind::Code {
        bar = bar.push(language_list(format, languages));
    }

    bar = bar
        .push(tool(p, "\u{2015}", ui, false, Some(Command::InsertRule)))
        .push(separator(p))
        .push(tool(p, "WYSIWYG", ui, visual, Some(Command::SetMode(Mode::Wysiwyg))))
        .push(tool(p, "Source", ui, !visual, Some(Command::SetMode(Mode::Source))));

    // Narrow editors: buttons continue on the next line instead of being cut off.
    bar.wrap().into()
}
