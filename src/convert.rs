//! Conversions between the core's types and Iced's.

use markdown_editor_core::geometry::{Color, Point, Rectangle, Size};
use markdown_editor_core::keyboard::{Key, Modifiers, Named, ScrollDelta};
use markdown_editor_core::style;

/// A colour for Iced.
pub(crate) fn color(color: Color) -> iced::Color {
    iced::Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

/// The editor's colour of an Iced colour, e.g. for a custom
/// [`Palette`](crate::Palette).
pub fn color_from_iced(color: iced::Color) -> Color {
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

pub(crate) fn point(point: Point) -> iced::Point {
    iced::Point::new(point.x, point.y)
}

pub(crate) fn point_from_iced(point: iced::Point) -> Point {
    Point::new(point.x, point.y)
}

pub(crate) fn size(size: Size) -> iced::Size {
    iced::Size::new(size.width, size.height)
}

pub(crate) fn size_from_iced(size: iced::Size) -> Size {
    Size::new(size.width, size.height)
}

pub(crate) fn rectangle(rect: Rectangle) -> iced::Rectangle {
    iced::Rectangle {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

pub(crate) fn modifiers(modifiers: iced::keyboard::Modifiers) -> Modifiers {
    let mut result = Modifiers::empty();
    for (held, flag) in [
        (modifiers.shift(), Modifiers::SHIFT),
        (modifiers.control(), Modifiers::CTRL),
        (modifiers.alt(), Modifiers::ALT),
        (modifiers.logo(), Modifiers::LOGO),
    ] {
        if held {
            result |= flag;
        }
    }
    result
}

pub(crate) fn scroll_delta(delta: iced::mouse::ScrollDelta) -> ScrollDelta {
    match delta {
        iced::mouse::ScrollDelta::Lines { x, y } => ScrollDelta::Lines { x, y },
        iced::mouse::ScrollDelta::Pixels { x, y } => ScrollDelta::Pixels { x, y },
    }
}

/// A logical key. Keys the editor has no use for are `Unidentified`.
pub(crate) fn key(key: &iced::keyboard::Key) -> Key {
    use iced::keyboard::key::Named as IcedNamed;

    let named = match key {
        iced::keyboard::Key::Character(c) => return Key::Character(c.to_string()),
        iced::keyboard::Key::Unidentified => return Key::Unidentified,
        iced::keyboard::Key::Named(named) => named,
    };
    Key::Named(match named {
        IcedNamed::Enter => Named::Enter,
        IcedNamed::Tab => Named::Tab,
        IcedNamed::Space => Named::Space,
        IcedNamed::Backspace => Named::Backspace,
        IcedNamed::Delete => Named::Delete,
        IcedNamed::Insert => Named::Insert,
        IcedNamed::Escape => Named::Escape,
        IcedNamed::ArrowLeft => Named::ArrowLeft,
        IcedNamed::ArrowRight => Named::ArrowRight,
        IcedNamed::ArrowUp => Named::ArrowUp,
        IcedNamed::ArrowDown => Named::ArrowDown,
        IcedNamed::Home => Named::Home,
        IcedNamed::End => Named::End,
        IcedNamed::PageUp => Named::PageUp,
        IcedNamed::PageDown => Named::PageDown,
        IcedNamed::Copy => Named::Copy,
        IcedNamed::Cut => Named::Cut,
        IcedNamed::Paste => Named::Paste,
        IcedNamed::F1 => Named::F1,
        IcedNamed::F2 => Named::F2,
        IcedNamed::F3 => Named::F3,
        IcedNamed::F4 => Named::F4,
        IcedNamed::F5 => Named::F5,
        IcedNamed::F6 => Named::F6,
        IcedNamed::F7 => Named::F7,
        IcedNamed::F8 => Named::F8,
        IcedNamed::F9 => Named::F9,
        IcedNamed::F10 => Named::F10,
        IcedNamed::F11 => Named::F11,
        IcedNamed::F12 => Named::F12,
        _ => return Key::Unidentified,
    })
}

/// The colours the widgets use, as Iced colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Palette {
    pub toolbar_background: iced::Color,
    pub toolbar_border: iced::Color,
    pub toolbar_text: iced::Color,
    pub toolbar_hover: iced::Color,
    pub toolbar_active: iced::Color,
    pub accent: iced::Color,
    pub muted: iced::Color,
}

pub(crate) fn palette(palette: &style::Palette) -> Palette {
    Palette {
        toolbar_background: color(palette.toolbar_background),
        toolbar_border: color(palette.toolbar_border),
        toolbar_text: color(palette.toolbar_text),
        toolbar_hover: color(palette.toolbar_hover),
        toolbar_active: color(palette.toolbar_active),
        accent: color(palette.accent),
        muted: color(palette.muted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_round_trip() {
        let original = Color::from_rgba(0.1, 0.2, 0.3, 0.4);
        assert_eq!(color_from_iced(color(original)), original);
    }

    #[test]
    fn keys_keep_their_meaning() {
        use iced::keyboard::Key as IcedKey;
        use iced::keyboard::key::Named as IcedNamed;

        assert_eq!(
            key(&IcedKey::Named(IcedNamed::ArrowLeft)),
            Key::Named(Named::ArrowLeft)
        );
        assert_eq!(
            key(&IcedKey::Character("a".into())),
            Key::Character("a".to_string())
        );
        assert_eq!(key(&IcedKey::Named(IcedNamed::CapsLock)), Key::Unidentified);
    }

    #[test]
    fn modifiers_keep_their_meaning() {
        let held = iced::keyboard::Modifiers::SHIFT | iced::keyboard::Modifiers::COMMAND;
        let converted = modifiers(held);
        assert!(converted.shift());
        assert!(converted.command());
        assert!(!converted.alt());
    }

    #[test]
    fn wheel_movements_keep_their_unit() {
        let lines = iced::mouse::ScrollDelta::Lines { x: 1.0, y: -2.0 };
        assert_eq!(scroll_delta(lines), ScrollDelta::Lines { x: 1.0, y: -2.0 });
        let pixels = iced::mouse::ScrollDelta::Pixels { x: 3.0, y: 4.0 };
        assert_eq!(scroll_delta(pixels), ScrollDelta::Pixels { x: 3.0, y: 4.0 });
    }
}
