//! The bundled fonts.
//!
//! The same font files are registered with Iced (for drawing) and measured
//! by the core. Because text is drawn with basic shaping, the measured
//! advances match the drawn glyph positions exactly, which keeps the caret,
//! selection and mouse hit-testing precise.

use iced::Font;
use iced::font::{Style as FontStyle, Weight};
use markdown_editor_core::fonts::{Face, Fonts};
use std::sync::{Arc, OnceLock};

pub const SANS_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
pub const SANS_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");
pub const SANS_ITALIC_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Oblique.ttf");
pub const SANS_BOLD_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/fonts/DejaVuSans-BoldOblique.ttf");
pub const MONO_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");
pub const MONO_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono-Bold.ttf");

/// All bundled font files. Register them with your application:
///
/// ```ignore
/// let mut app = iced::application(App::default, App::update, App::view);
/// for font in iced_markdown_editor::FONTS {
///     app = app.font(font);
/// }
/// ```
pub const FONTS: [&[u8]; 6] = [
    SANS_REGULAR_BYTES,
    SANS_BOLD_BYTES,
    SANS_ITALIC_BYTES,
    SANS_BOLD_ITALIC_BYTES,
    MONO_REGULAR_BYTES,
    MONO_BOLD_BYTES,
];

pub const SANS: Font = Font::with_name("DejaVu Sans");
pub const SANS_BOLD: Font = Font {
    weight: Weight::Bold,
    ..SANS
};
pub const SANS_ITALIC: Font = Font {
    style: FontStyle::Italic,
    ..SANS
};
pub const SANS_BOLD_ITALIC: Font = Font {
    weight: Weight::Bold,
    style: FontStyle::Italic,
    ..SANS
};
pub const MONO: Font = Font::with_name("DejaVu Sans Mono");
pub const MONO_BOLD: Font = Font {
    weight: Weight::Bold,
    ..MONO
};

/// The Iced font of a face.
pub(crate) fn font(face: Face) -> Font {
    match face {
        Face::Sans => SANS,
        Face::SansBold => SANS_BOLD,
        Face::SansItalic => SANS_ITALIC,
        Face::SansBoldItalic => SANS_BOLD_ITALIC,
        Face::Mono => MONO,
        Face::MonoBold => MONO_BOLD,
    }
}

/// The process-wide metrics of the bundled fonts.
pub(crate) fn shared() -> &'static Arc<Fonts> {
    static SHARED: OnceLock<Arc<Fonts>> = OnceLock::new();
    SHARED.get_or_init(|| Arc::new(Fonts::new(FONTS)))
}
