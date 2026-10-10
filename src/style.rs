//! Theming.
//!
//! An [`EditorStyle`] combines a colour [`Palette`] and [`Typography`].
//! Use a preset ([`EditorStyle::light`], [`EditorStyle::dark`]), derive
//! one from an Iced theme ([`EditorStyle::from_theme`]) or change
//! individual fields.

use iced::{Color, Theme};

/// All colours used by the editor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Area around the pages.
    pub background: Color,
    /// The sheet.
    pub page: Color,
    pub page_border: Color,
    pub page_shadow: Color,
    /// Body text.
    pub ink: Color,
    pub heading: Color,
    pub quote: Color,
    pub quote_bar: Color,
    /// Text inside code blocks (also the default colour for highlighted code).
    pub code: Color,
    pub code_background: Color,
    pub inline_code_background: Color,
    pub link: Color,
    /// List bullets, numbers and empty task checkboxes.
    pub marker: Color,
    /// Horizontal rule.
    pub rule: Color,
    pub selection: Color,
    pub caret: Color,
    pub toolbar_background: Color,
    pub toolbar_border: Color,
    pub toolbar_text: Color,
    pub toolbar_hover: Color,
    pub toolbar_active: Color,
    /// Active buttons, checked task checkboxes.
    pub accent: Color,
    /// Secondary text (status bar, labels, raw HTML, image descriptions).
    pub muted: Color,
}

/// Font sizes and spacing in logical pixels at 100 % zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    /// Space between pages and around them (not zoomed).
    pub page_gap: f32,
    /// Inner padding of the text in full-width mode; not zoomed.
    pub fill_padding: f32,
    pub body_size: f32,
    pub body_line_height: f32,
    pub paragraph_spacing: f32,
    /// Sizes of heading levels 1, 2 and 3 (levels 4-6 use level 3).
    pub heading_sizes: [f32; 3],
    pub code_size: f32,
    pub code_line_height: f32,
    /// Indentation per list level.
    pub list_indent: f32,
    /// Indentation per block quote level.
    pub quote_indent: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            page_gap: 28.0,
            fill_padding: 16.0,
            body_size: 14.5,
            body_line_height: 22.0,
            paragraph_spacing: 10.0,
            heading_sizes: [27.0, 21.0, 17.0],
            code_size: 13.0,
            code_line_height: 20.0,
            list_indent: 30.0,
            quote_indent: 22.0,
        }
    }
}

/// Complete editor appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorStyle {
    pub palette: Palette,
    pub typography: Typography,
}

impl Default for EditorStyle {
    fn default() -> Self {
        Self::light()
    }
}

impl Palette {
    /// White page on a grey desk, like Word.
    pub fn light() -> Self {
        Self {
            background: Color::from_rgb(0.86, 0.87, 0.89),
            page: Color::WHITE,
            page_border: Color::from_rgb(0.80, 0.81, 0.84),
            page_shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
            ink: Color::from_rgb(0.11, 0.11, 0.12),
            heading: Color::from_rgb(0.07, 0.17, 0.33),
            quote: Color::from_rgb(0.36, 0.38, 0.42),
            quote_bar: Color::from_rgb(0.78, 0.80, 0.84),
            code: Color::from_rgb(0.16, 0.18, 0.22),
            code_background: Color::from_rgb(0.955, 0.958, 0.965),
            inline_code_background: Color::from_rgb(0.93, 0.935, 0.945),
            link: Color::from_rgb(0.02, 0.39, 0.76),
            marker: Color::from_rgb(0.25, 0.26, 0.29),
            rule: Color::from_rgb(0.76, 0.77, 0.80),
            selection: Color::from_rgba(0.16, 0.42, 0.86, 0.28),
            caret: Color::from_rgb(0.11, 0.11, 0.12),
            toolbar_background: Color::from_rgb(0.97, 0.97, 0.98),
            toolbar_border: Color::from_rgb(0.82, 0.83, 0.86),
            toolbar_text: Color::from_rgb(0.20, 0.21, 0.24),
            toolbar_hover: Color::from_rgb(0.91, 0.92, 0.94),
            toolbar_active: Color::from_rgb(0.84, 0.89, 0.97),
            accent: Color::from_rgb(0.10, 0.36, 0.78),
            muted: Color::from_rgb(0.42, 0.44, 0.48),
        }
    }

    /// Dark page for night work.
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb(0.08, 0.085, 0.10),
            page: Color::from_rgb(0.145, 0.15, 0.17),
            page_border: Color::from_rgb(0.22, 0.23, 0.26),
            page_shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.40),
            ink: Color::from_rgb(0.88, 0.89, 0.91),
            heading: Color::from_rgb(0.66, 0.80, 1.0),
            quote: Color::from_rgb(0.68, 0.70, 0.74),
            quote_bar: Color::from_rgb(0.35, 0.37, 0.42),
            code: Color::from_rgb(0.86, 0.87, 0.90),
            code_background: Color::from_rgb(0.11, 0.115, 0.13),
            inline_code_background: Color::from_rgb(0.22, 0.23, 0.26),
            link: Color::from_rgb(0.45, 0.68, 1.0),
            marker: Color::from_rgb(0.75, 0.76, 0.80),
            rule: Color::from_rgb(0.35, 0.37, 0.42),
            selection: Color::from_rgba(0.35, 0.55, 0.95, 0.35),
            caret: Color::from_rgb(0.92, 0.93, 0.95),
            toolbar_background: Color::from_rgb(0.12, 0.125, 0.14),
            toolbar_border: Color::from_rgb(0.22, 0.23, 0.26),
            toolbar_text: Color::from_rgb(0.85, 0.86, 0.88),
            toolbar_hover: Color::from_rgb(0.20, 0.21, 0.24),
            toolbar_active: Color::from_rgb(0.18, 0.26, 0.40),
            accent: Color::from_rgb(0.45, 0.68, 1.0),
            muted: Color::from_rgb(0.55, 0.57, 0.62),
        }
    }
}

fn luminance(c: Color) -> f32 {
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

impl EditorStyle {
    pub fn light() -> Self {
        Self {
            palette: Palette::light(),
            typography: Typography::default(),
        }
    }

    pub fn dark() -> Self {
        Self {
            palette: Palette::dark(),
            typography: Typography::default(),
        }
    }

    /// Derives a style from an Iced theme, so the editor matches the rest
    /// of the application.
    pub fn from_theme(theme: &Theme) -> Self {
        let p = theme.palette();
        let dark = luminance(p.background) < 0.5;
        let mut style = if dark { Self::dark() } else { Self::light() };
        let palette = &mut style.palette;
        palette.page = p.background;
        palette.ink = p.text;
        palette.caret = p.text;
        palette.code = p.text;
        palette.marker = p.text;
        palette.toolbar_text = p.text;
        palette.link = p.primary;
        palette.accent = p.primary;
        palette.heading = p.primary;
        palette.selection = with_alpha(p.primary, 0.30);
        palette.toolbar_active = with_alpha(p.primary, 0.20);
        style
    }

    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    pub fn with_typography(mut self, typography: Typography) -> Self {
        self.typography = typography;
        self
    }
}
