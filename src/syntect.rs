//! Code highlighter based on [syntect](https://github.com/trishume/syntect)
//! (Sublime Text grammars). Available with the `syntect` feature.
//!
//! ```ignore
//! use iced_markdown_editor::{EditorState, SyntectHighlighter};
//!
//! let editor = EditorState::with_markdown(text).highlighter(SyntectHighlighter::new());
//! ```
//!
//! The light theme is used when the editor's code background is light,
//! the dark theme otherwise.

use crate::highlight::{CodeHighlighter, Color, HighlightRequest, HighlightSpan};
use std::collections::BTreeSet;

use ::syntect::easy::HighlightLines;
use ::syntect::util::LinesWithEndings;

/// syntect types used by [`SyntectHighlighter::from_parts`], re-exported so
/// you do not need a direct dependency on syntect.
pub use ::syntect::highlighting::{Theme, ThemeSet};
pub use ::syntect::parsing::{SyntaxReference, SyntaxSet};

/// Default theme for light code backgrounds.
pub const DEFAULT_LIGHT_THEME: &str = "InspiredGitHub";
/// Default theme for dark code backgrounds.
pub const DEFAULT_DARK_THEME: &str = "base16-ocean.dark";

/// Fence names that syntect does not know directly, mapped to a token it
/// does know (an extension or syntax name).
const ALIASES: &[(&str, &str)] = &[
    ("ts", "js"),
    ("tsx", "js"),
    ("typescript", "js"),
    ("jsx", "js"),
    ("jsonc", "json"),
    ("json5", "json"),
    ("shell", "sh"),
    ("console", "sh"),
    ("zsh", "sh"),
    ("powershell", "sh"),
    ("ps1", "sh"),
    ("csharp", "cs"),
    ("c#", "cs"),
    ("c++", "cpp"),
    ("golang", "go"),
    ("py3", "py"),
    ("python3", "py"),
    ("yml", "yaml"),
    ("objc", "m"),
    ("html5", "html"),
    ("svg", "xml"),
];

/// Highlighter based on syntect's bundled grammars and themes.
pub struct SyntectHighlighter {
    syntaxes: SyntaxSet,
    light: Theme,
    dark: Theme,
}

impl Default for SyntectHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntectHighlighter {
    /// Bundled grammars, `InspiredGitHub` (light) and `base16-ocean.dark` (dark).
    pub fn new() -> Self {
        Self::with_themes(DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME)
    }

    /// Bundled grammars with the named bundled themes. Unknown names fall
    /// back to the defaults.
    pub fn with_themes(light: &str, dark: &str) -> Self {
        let themes = ThemeSet::load_defaults();
        let pick = |name: &str, fallback: &str| {
            themes
                .themes
                .get(name)
                .or_else(|| themes.themes.get(fallback))
                .cloned()
                .unwrap_or_default()
        };
        Self {
            syntaxes: SyntaxSet::load_defaults_newlines(),
            light: pick(light, DEFAULT_LIGHT_THEME),
            dark: pick(dark, DEFAULT_DARK_THEME),
        }
    }

    /// Your own grammars and themes. The syntax set must be built for lines
    /// **with** newlines.
    pub fn from_parts(syntaxes: SyntaxSet, light: Theme, dark: Theme) -> Self {
        Self {
            syntaxes,
            light,
            dark,
        }
    }

    /// Names of the bundled themes.
    pub fn theme_names() -> Vec<String> {
        ThemeSet::load_defaults().themes.keys().cloned().collect()
    }

    fn find(&self, language: &str) -> Option<&SyntaxReference> {
        let language = language.trim().to_lowercase();
        let token = ALIASES
            .iter()
            .find(|(alias, _)| *alias == language)
            .map(|(_, token)| *token)
            .unwrap_or(language.as_str());

        self.syntaxes
            .find_syntax_by_token(token)
            .or_else(|| self.syntaxes.find_syntax_by_extension(token))
            .filter(|syntax| syntax.name != "Plain Text")
    }
}

fn to_color(color: ::syntect::highlighting::Color) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0)
}

impl CodeHighlighter for SyntectHighlighter {
    fn highlight(&self, request: &HighlightRequest<'_>) -> Vec<HighlightSpan> {
        let Some(syntax) = self.find(request.language) else {
            return Vec::new();
        };
        let theme = if request.dark { &self.dark } else { &self.light };
        let mut lines = HighlightLines::new(syntax, theme);
        let mut spans = Vec::new();
        let mut offset = 0usize;

        for line in LinesWithEndings::from(request.code) {
            let Ok(ranges) = lines.highlight_line(line, &self.syntaxes) else {
                break;
            };
            for (style, piece) in ranges {
                let len = piece.chars().count();
                spans.push(HighlightSpan {
                    range: offset..offset + len,
                    color: to_color(style.foreground),
                });
                offset += len;
            }
        }

        spans
    }

    fn languages(&self) -> Vec<String> {
        let mut names = BTreeSet::new();
        for syntax in self.syntaxes.syntaxes() {
            if syntax.hidden || syntax.name == "Plain Text" {
                continue;
            }
            let name = syntax.name.to_lowercase();
            let short = if name.contains(' ') || name.contains('(') {
                match syntax.file_extensions.first() {
                    Some(extension) => extension.to_lowercase(),
                    None => continue,
                }
            } else {
                name
            };
            names.insert(short);
        }
        names.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_and_ignores_unknown_languages() {
        let highlighter = SyntectHighlighter::new();
        let request = |language| HighlightRequest {
            language,
            code: "fn main() {}\n",
            background: Color::WHITE,
            dark: false,
        };
        assert!(!highlighter.highlight(&request("rust")).is_empty());
        assert!(!highlighter.highlight(&request("rs")).is_empty());
        assert!(highlighter.highlight(&request("no-such-language")).is_empty());
        assert!(highlighter.languages().contains(&"rust".to_string()));
    }
}
