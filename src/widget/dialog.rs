//! The link dialog, opened with "Link…" in the context menu: a modal box
//! in the middle of the editor with the text to display and the address,
//! as in word processors. Enter applies, Escape cancels, Tab switches
//! between the fields.

use super::toolbar::{button_style, fill};
use super::{Action, EditorState, Inner};
use crate::fonts;
use crate::i18n;
use iced::keyboard::{self, Key, key::Named};
use iced::mouse;
use iced::widget::canvas::{self, Canvas, Geometry};
use iced::widget::{Id, button, column, container, opaque, row, text, text_input};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Rectangle, Renderer, Shadow, Theme,
    Vector,
};

/// State of the open link dialog.
#[derive(Debug, Clone)]
pub(crate) struct LinkDialog {
    /// Text to display; `None` when the selection cannot be retyped
    /// (several paragraphs, images, raw HTML).
    pub text: Option<String>,
    /// The selected text when the dialog opened.
    pub original: String,
    /// The selection already is a link.
    pub editing: bool,
    pub text_id: Id,
    pub url_id: Id,
}

/// Width of the dialog box.
const WIDTH: f32 = 440.0;

fn label<'a>(content: String, size: u32) -> text::Text<'a> {
    text(content).size(size).font(fonts::SANS)
}

/// The dialog layers: a dimmed backdrop that blocks the editor, the box,
/// and an invisible layer that reads Escape and Tab first.
pub(crate) fn view<'a>(
    state: &EditorState,
    dialog: &LinkDialog,
    url: &str,
) -> [Element<'a, Action>; 2] {
    let palette = state.style.palette;
    let tr = |id| i18n::text(state.language, id);

    let title = if dialog.editing {
        tr("link-dialog-edit")
    } else {
        tr("link-dialog-insert")
    };
    let field = |caption: String, input: text_input::TextInput<'a, Action>| {
        column![
            label(caption, 12).color(palette.muted),
            input.font(fonts::SANS).padding(6).size(13),
        ]
        .spacing(4)
    };

    let mut body = column![label(title, 15).font(fonts::SANS_BOLD)].spacing(14);
    if let Some(display) = &dialog.text {
        body = body.push(field(
            tr("link-text"),
            text_input("", display)
                .id(dialog.text_id.clone())
                .on_input(|text| Action(Inner::LinkText(text)))
                .on_submit(Action(Inner::LinkApply)),
        ));
    }
    body = body.push(field(
        tr("link-address"),
        text_input("https://", url)
            .id(dialog.url_id.clone())
            .on_input(|url| Action(Inner::LinkUrl(url)))
            .on_submit(Action(Inner::LinkApply)),
    ));

    let action = |caption: String, action: Inner, primary: bool| {
        button(label(caption, 12))
            .on_press(Action(action))
            .padding([5, 14])
            .style(button_style(palette, primary))
    };
    let mut buttons = row![].spacing(8).align_y(Alignment::Center);
    if dialog.editing {
        buttons = buttons.push(action(tr("link-remove"), Inner::LinkRemove, false));
    }
    buttons = buttons
        .push(fill())
        .push(action(tr("link-ok"), Inner::LinkApply, true))
        .push(action(tr("link-cancel"), Inner::LinkCancel, false));
    body = body.push(buttons);

    let dialog_box = container(body)
        .padding(18)
        .width(Length::Fixed(WIDTH))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(palette.toolbar_background)),
            text_color: Some(palette.toolbar_text),
            border: Border {
                color: palette.toolbar_border,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.3,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 6.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        });

    let backdrop = container(dialog_box)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.2,
                ..Color::BLACK
            })),
            ..container::Style::default()
        });

    let keys = Canvas::new(Keys).width(Length::Fill).height(Length::Fill);

    [opaque(backdrop), keys.into()]
}

/// Reads Escape and Tab before the text fields do (a focused field
/// swallows Escape and ignores Tab).
struct Keys;

impl canvas::Program<Action> for Keys {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Action>> {
        let canvas::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event else {
            return None;
        };
        let inner = match key.as_ref() {
            Key::Named(Named::Escape) => Inner::LinkCancel,
            Key::Named(Named::Tab) => Inner::LinkTab,
            _ => return None,
        };
        Some(canvas::Action::publish(Action(inner)).and_capture())
    }

    fn draw(
        &self,
        _state: &(),
        _renderer: &Renderer,
        _theme: &Theme,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        Vec::new()
    }
}
