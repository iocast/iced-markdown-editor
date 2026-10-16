//! Interface language of the built-in toolbar, link bar and status bar.
//!
//! The texts are [Fluent](https://projectfluent.org) messages compiled into
//! the crate (`locales/*.ftl`). Messages missing in a translation fall back
//! to English. The document itself is never translated.
//!
//! ```
//! use iced_markdown_editor::{EditorState, Language};
//!
//! // E.g. from the operating system locale.
//! let language = Language::from_tag("de-CH").unwrap_or_default();
//! let editor = EditorState::new().language(language);
//! assert_eq!(editor.current_language(), Language::German);
//! ```

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use std::fmt;
use std::sync::OnceLock;
use unic_langid::LanguageIdentifier;

/// A language of the editor interface. English by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    #[default]
    English,
    German,
    French,
    Spanish,
    Italian,
}

impl Language {
    /// Every supported language.
    pub const ALL: [Language; 5] = [
        Language::English,
        Language::German,
        Language::French,
        Language::Spanish,
        Language::Italian,
    ];

    /// The BCP 47 language tag, e.g. `de`.
    pub fn tag(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::German => "de",
            Language::French => "fr",
            Language::Spanish => "es",
            Language::Italian => "it",
        }
    }

    /// The language's own name, e.g. `Deutsch`.
    pub fn name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::German => "Deutsch",
            Language::French => "Français",
            Language::Spanish => "Español",
            Language::Italian => "Italiano",
        }
    }

    /// The language of a locale such as `fr`, `de-CH`, `es_ES.UTF-8` or
    /// `it-IT`; the region and encoding are ignored. `None` for languages
    /// without a translation.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let primary = tag.split(['-', '_', '.', '@']).next()?;
        Self::ALL
            .into_iter()
            .find(|language| language.tag().eq_ignore_ascii_case(primary))
    }

    fn source(self) -> &'static str {
        match self {
            Language::English => include_str!("../locales/en.ftl"),
            Language::German => include_str!("../locales/de.ftl"),
            Language::French => include_str!("../locales/fr.ftl"),
            Language::Spanish => include_str!("../locales/es.ftl"),
            Language::Italian => include_str!("../locales/it.ftl"),
        }
    }

    /// The parsed messages, built on first use.
    fn bundle(self) -> &'static FluentBundle<FluentResource> {
        static BUNDLES: [OnceLock<FluentBundle<FluentResource>>; 5] =
            [const { OnceLock::new() }; 5];
        let index = Self::ALL.iter().position(|l| *l == self).unwrap_or(0);
        BUNDLES[index].get_or_init(|| {
            let tag: LanguageIdentifier = self.tag().parse().expect("valid language tag");
            let mut bundle = FluentBundle::new_concurrent(vec![tag]);
            // No Unicode isolation marks around arguments: the bundled fonts
            // have no glyphs for them.
            bundle.set_use_isolating(false);
            let resource = FluentResource::try_new(self.source().to_string())
                .unwrap_or_else(|(resource, _)| resource);
            let _ = bundle.add_resource(resource);
            bundle
        })
    }

    /// The message `id` with `args`, in English if this language lacks it.
    fn format(self, id: &str, args: Option<&FluentArgs>) -> Option<String> {
        let bundle = self.bundle();
        let pattern = bundle.get_message(id)?.value()?;
        let mut errors = Vec::new();
        Some(
            bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned(),
        )
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The text of message `id`.
pub(crate) fn text(language: Language, id: &str) -> String {
    text_with(language, id, &[])
}

/// The text of message `id` with named arguments.
pub(crate) fn text_with(
    language: Language,
    id: &str,
    args: &[(&'static str, FluentValue<'_>)],
) -> String {
    let args = (!args.is_empty()).then(|| {
        let mut fluent = FluentArgs::with_capacity(args.len());
        for (name, value) in args {
            fluent.set(*name, value.clone());
        }
        fluent
    });
    language
        .format(id, args.as_ref())
        .or_else(|| Language::English.format(id, args.as_ref()))
        .unwrap_or_else(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_syntax::ast;

    fn message_ids(language: Language) -> Vec<String> {
        let resource = FluentResource::try_new(language.source().to_string())
            .unwrap_or_else(|(_, errors)| panic!("{language:?}: {errors:?}"));
        resource
            .entries()
            .filter_map(|entry| match entry {
                ast::Entry::Message(message) => Some(message.id.name.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn translations_are_complete() {
        let mut english = message_ids(Language::English);
        english.sort();
        for language in Language::ALL {
            let mut ids = message_ids(language);
            ids.sort();
            assert_eq!(ids, english, "messages of {language:?}");
        }
    }

    #[test]
    fn messages_format_without_errors() {
        let mut args = FluentArgs::new();
        for name in ["current", "total", "count", "checked", "size", "max"] {
            args.set(name, 2);
        }
        args.set("mime", "image/x-test");
        args.set("error", "broken");
        for language in Language::ALL {
            let bundle = language.bundle();
            for id in message_ids(language) {
                let pattern = bundle.get_message(&id).and_then(|m| m.value());
                let pattern = pattern.unwrap_or_else(|| panic!("{language:?}: {id}"));
                let mut errors = Vec::new();
                let text = bundle.format_pattern(pattern, Some(&args), &mut errors);
                assert!(errors.is_empty(), "{language:?} {id}: {errors:?}");
                assert!(!text.contains(['\u{2068}', '\u{2069}']));
            }
        }
    }

    #[test]
    fn plurals() {
        let words = |language, count: usize| {
            text_with(language, "status-words", &[("count", count.into())])
        };
        assert_eq!(words(Language::English, 1), "1 word");
        assert_eq!(words(Language::English, 3), "3 words");
        assert_eq!(words(Language::German, 1), "1 Wort");
        assert_eq!(words(Language::German, 0), "0 Wörter");
        assert_eq!(words(Language::French, 0), "0 mot");
        assert_eq!(words(Language::Spanish, 2), "2 palabras");
        assert_eq!(words(Language::Italian, 2), "2 parole");
    }

    #[test]
    fn tags() {
        assert_eq!(Language::from_tag("de-CH"), Some(Language::German));
        assert_eq!(Language::from_tag("fr_FR.UTF-8"), Some(Language::French));
        assert_eq!(Language::from_tag("ES"), Some(Language::Spanish));
        assert_eq!(Language::from_tag("ja-JP"), None);
        for language in Language::ALL {
            assert_eq!(Language::from_tag(language.tag()), Some(language));
        }
    }

    #[test]
    fn unknown_messages_fall_back_to_the_id() {
        assert_eq!(text(Language::German, "no-such-message"), "no-such-message");
    }
}
