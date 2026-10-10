//! Bundled fonts.
//!
//! The same font files are registered with Iced (for drawing) and parsed
//! with `ab_glyph` (for measuring). Because text is drawn with basic
//! shaping, the measured advances match the drawn glyph positions exactly,
//! which keeps the caret, selection and mouse hit-testing precise.

use ab_glyph::{Font as _, FontRef};
use iced::font::{Style as FontStyle, Weight};
use iced::Font;

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

/// A concrete font face used by the document. The discriminant is the
/// index into [`FONTS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Sans = 0,
    SansBold = 1,
    SansItalic = 2,
    SansBoldItalic = 3,
    Mono = 4,
    MonoBold = 5,
}

impl Face {
    pub fn select(bold: bool, italic: bool, mono: bool) -> Face {
        match (mono, bold, italic) {
            (true, true, _) => Face::MonoBold,
            (true, false, _) => Face::Mono,
            (false, true, true) => Face::SansBoldItalic,
            (false, true, false) => Face::SansBold,
            (false, false, true) => Face::SansItalic,
            (false, false, false) => Face::Sans,
        }
    }

    pub fn font(self) -> Font {
        match self {
            Face::Sans => SANS,
            Face::SansBold => SANS_BOLD,
            Face::SansItalic => SANS_ITALIC,
            Face::SansBoldItalic => SANS_BOLD_ITALIC,
            Face::Mono => MONO,
            Face::MonoBold => MONO_BOLD,
        }
    }
}

/// Glyph metrics for all bundled faces.
pub struct Fonts {
    faces: Vec<FontRef<'static>>,
    units_per_em: Vec<f32>,
}

impl Fonts {
    pub fn load() -> Self {
        let mut faces = Vec::with_capacity(FONTS.len());
        let mut units_per_em = Vec::with_capacity(FONTS.len());

        for bytes in FONTS {
            let face = FontRef::try_from_slice(bytes).expect("bundled font must be valid");
            units_per_em.push(face.units_per_em().unwrap_or(2048.0));
            faces.push(face);
        }

        Self {
            faces,
            units_per_em,
        }
    }

    /// Horizontal advance of `c` in logical pixels.
    pub fn advance(&self, face: Face, c: char, size: f32) -> f32 {
        let index = face as usize;
        let font = &self.faces[index];
        let glyph = font.glyph_id(c);
        font.h_advance_unscaled(glyph) * size / self.units_per_em[index]
    }

    /// Width of a whole string in logical pixels.
    pub fn width(&self, face: Face, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.advance(face, c, size)).sum()
    }
}
