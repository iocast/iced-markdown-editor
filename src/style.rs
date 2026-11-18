//! Theming.
//!
//! An [`EditorStyle`] combines a colour [`Palette`] and [`Typography`].
//! Use a preset ([`EditorStyle::light`], [`EditorStyle::dark`]), derive
//! one from an Iced theme ([`FromTheme::from_theme`]) or change
//! individual fields.

use crate::convert::color_from_iced;
use iced::Theme;
use markdown_editor_core::style::{luminance, with_alpha};

pub use markdown_editor_core::style::{EditorStyle, Palette, Typography};

/// Derives a style from an Iced theme.
pub trait FromTheme {
    /// The style of `theme`, so the editor matches the rest of the
    /// application.
    fn from_theme(theme: &Theme) -> Self;
}

impl FromTheme for EditorStyle {
    fn from_theme(theme: &Theme) -> Self {
        let p = theme.palette();
        let background = color_from_iced(p.background);
        let text = color_from_iced(p.text);
        let primary = color_from_iced(p.primary);
        let dark = luminance(background) < 0.5;
        let mut style = if dark { Self::dark() } else { Self::light() };
        let palette = &mut style.palette;
        palette.page = background;
        palette.ink = text;
        palette.caret = text;
        palette.code = text;
        palette.marker = text;
        palette.toolbar_text = text;
        palette.link = primary;
        palette.accent = primary;
        palette.heading = primary;
        palette.selection = with_alpha(primary, 0.30);
        palette.toolbar_active = with_alpha(primary, 0.20);
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dark_theme_gives_a_dark_style() {
        let style = EditorStyle::from_theme(&Theme::Dark);
        let page = Theme::Dark.palette().background;
        assert_eq!(style.palette.page, color_from_iced(page));
        assert_eq!(
            style.palette.background,
            EditorStyle::dark().palette.background
        );
    }
}
