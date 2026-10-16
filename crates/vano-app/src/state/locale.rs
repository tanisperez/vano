//! The UI language: which languages exist, how a system locale tag maps to one, how the `language`
//! setting resolves to a concrete language, and the plural helper. Plain Rust, no `gpui`. The
//! translations themselves live in `crates/vano-app/locales/` and are looked up with `t!`.
//!
//! The locale is process wide (`rust_i18n::set_locale`), so no test may call [`apply`]: tests
//! that need a language pass `locale = "xx"` to `t!` instead.

use std::sync::OnceLock;

use serde::Serialize;

/// A language the UI is translated into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// English, the source language and the fallback.
    English,
    /// Spanish.
    Spanish,
    /// Galician.
    Galician,
    /// Italian.
    Italian,
}

impl Language {
    /// Every language, in the order the Settings dropdown lists them.
    #[allow(dead_code)] // Only the tests enumerate languages for now.
    pub const ALL: [Language; 4] = [
        Language::English,
        Language::Spanish,
        Language::Galician,
        Language::Italian,
    ];

    /// The locale code used by the translation tables (`en`, `es`, `gl`, `it`).
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
            Language::Galician => "gl",
            Language::Italian => "it",
        }
    }

    /// The language's name written in itself, shown in the Settings dropdown.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Espa\u{f1}ol",
            Language::Galician => "Galego",
            Language::Italian => "Italiano",
        }
    }
}

/// The `language` setting: follow the system or force one language. Stored in `settings.toml`
/// as `language = "auto" | "en" | "es" | "gl" | "it"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageChoice {
    /// Use the first system language Vano supports, English if none.
    #[default]
    Auto,
    /// Always English.
    En,
    /// Always Spanish.
    Es,
    /// Always Galician.
    Gl,
    /// Always Italian.
    It,
}

impl LanguageChoice {
    /// Every choice, in the order the Settings dropdown lists them.
    pub const ALL: [LanguageChoice; 5] = [
        LanguageChoice::Auto,
        LanguageChoice::En,
        LanguageChoice::Es,
        LanguageChoice::Gl,
        LanguageChoice::It,
    ];

    /// Parses a value as read from the settings file, case-insensitive. Returns `None` for
    /// anything else, so the caller can fall back to the default.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(LanguageChoice::Auto),
            "en" => Some(LanguageChoice::En),
            "es" => Some(LanguageChoice::Es),
            "gl" => Some(LanguageChoice::Gl),
            "it" => Some(LanguageChoice::It),
            _ => None,
        }
    }

    /// The forced language, `None` for [`LanguageChoice::Auto`].
    pub fn language(self) -> Option<Language> {
        match self {
            LanguageChoice::Auto => None,
            LanguageChoice::En => Some(Language::English),
            LanguageChoice::Es => Some(Language::Spanish),
            LanguageChoice::Gl => Some(Language::Galician),
            LanguageChoice::It => Some(Language::Italian),
        }
    }
}

/// The language of a BCP 47 or POSIX locale tag (`es-ES`, `es_ES.UTF-8`, `gl`, `it_CH@euro`),
/// by its primary subtag, case-insensitive. `None` for an unsupported language and for `C` and
/// `POSIX`.
pub fn match_language(tag: &str) -> Option<Language> {
    let base = tag.split(['.', '@']).next().unwrap_or_default();
    let primary = base.split(['-', '_']).next().unwrap_or_default();
    match primary.trim().to_ascii_lowercase().as_str() {
        "en" => Some(Language::English),
        "es" => Some(Language::Spanish),
        "gl" => Some(Language::Galician),
        "it" => Some(Language::Italian),
        _ => None,
    }
}

/// The first supported language among `preferred` (the user's languages, most wanted first),
/// English when none is supported or the list is empty. The caller passes
/// [`system_locales`]; tests pass their own list.
pub fn detect(preferred: impl IntoIterator<Item = String>) -> Language {
    preferred
        .into_iter()
        .find_map(|tag| match_language(&tag))
        .unwrap_or(Language::English)
}

/// The OS preferred languages, most wanted first, read once per process.
pub fn system_locales() -> &'static [String] {
    static LOCALES: OnceLock<Vec<String>> = OnceLock::new();
    LOCALES.get_or_init(|| sys_locale::get_locales().collect())
}

/// The language the app speaks for `choice`: the forced one, or the system's.
pub fn resolve(choice: LanguageChoice) -> Language {
    choice
        .language()
        .unwrap_or_else(|| detect(system_locales().iter().cloned()))
}

/// Makes `language` the process wide UI language, for Vano's strings and gpui-component's.
pub fn apply(language: Language) {
    rust_i18n::set_locale(language.code());
}

/// The key of the plural form to use for `count`: `<key>.one` for exactly one, `<key>.other`
/// otherwise. Enough for English, Spanish, Galician and Italian.
pub fn plural_key(key: &str, count: usize) -> String {
    let form = if count == 1 { "one" } else { "other" };
    format!("{key}.{form}")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|tag| tag.to_string()).collect()
    }

    #[test]
    fn match_language_reads_bcp47_and_posix_tags() {
        assert_eq!(match_language("es"), Some(Language::Spanish));
        assert_eq!(match_language("es-ES"), Some(Language::Spanish));
        assert_eq!(match_language("es_MX"), Some(Language::Spanish));
        assert_eq!(match_language("es_ES.UTF-8"), Some(Language::Spanish));
        assert_eq!(match_language("gl-ES"), Some(Language::Galician));
        assert_eq!(match_language("it_CH@euro"), Some(Language::Italian));
        assert_eq!(match_language("EN-us"), Some(Language::English));
        assert_eq!(match_language("en_US.utf8"), Some(Language::English));
    }

    #[test]
    fn match_language_rejects_unsupported_and_posix_defaults() {
        assert_eq!(match_language("fr-FR"), None);
        assert_eq!(match_language("pt-BR"), None);
        assert_eq!(match_language("C"), None);
        assert_eq!(match_language("POSIX"), None);
        assert_eq!(match_language("C.UTF-8"), None);
        assert_eq!(match_language(""), None);
        assert_eq!(match_language("esperanto"), None);
    }

    #[test]
    fn detect_takes_the_first_supported_language() {
        assert_eq!(detect(tags(&["pt-BR", "gl-ES"])), Language::Galician);
        assert_eq!(
            detect(tags(&["fr", "it_IT.UTF-8", "es"])),
            Language::Italian
        );
        assert_eq!(detect(tags(&["en-US", "es-ES"])), Language::English);
    }

    #[test]
    fn detect_falls_back_to_english() {
        assert_eq!(detect(tags(&[])), Language::English);
        assert_eq!(detect(tags(&["C"])), Language::English);
        assert_eq!(detect(tags(&["fr-FR", "de"])), Language::English);
        assert_eq!(detect(tags(&["C", "es_ES.UTF-8"])), Language::Spanish);
    }

    #[test]
    fn a_forced_choice_ignores_the_system() {
        assert_eq!(resolve(LanguageChoice::Gl), Language::Galician);
        assert_eq!(resolve(LanguageChoice::En), Language::English);
        assert_eq!(LanguageChoice::Auto.language(), None);
    }

    #[test]
    fn language_choice_parses_every_value() {
        for choice in LanguageChoice::ALL {
            let name = format!("{choice:?}").to_uppercase();
            assert_eq!(LanguageChoice::parse(&name), Some(choice));
        }
        assert_eq!(LanguageChoice::parse(" Es "), Some(LanguageChoice::Es));
        assert_eq!(LanguageChoice::parse("fr"), None);
    }

    #[test]
    fn codes_and_names_match_the_choices() {
        for language in Language::ALL {
            assert_eq!(match_language(language.code()), Some(language));
        }
        let forced: Vec<_> = LanguageChoice::ALL
            .into_iter()
            .filter_map(LanguageChoice::language)
            .collect();
        assert_eq!(forced, Language::ALL.to_vec());
        assert_eq!(Language::Spanish.native_name(), "Espa\u{f1}ol");
    }

    #[test]
    fn plural_picks_one_for_exactly_one() {
        assert_eq!(plural_key("files", 1), "files.one");
        assert_eq!(plural_key("files", 0), "files.other");
        assert_eq!(plural_key("files", 2), "files.other");
    }

    /// Every message of `locale` defined by Vano's own tables (not gpui-component's).
    fn messages(locale: &str) -> BTreeMap<String, String> {
        crate::_rust_i18n_backend()
            .messages_for_locale(locale)
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    /// The `%{name}` placeholders of `text`, sorted.
    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find("%{") {
            let after = &rest[start + 2..];
            let Some(end) = after.find('}') else { break };
            found.insert(after[..end].to_string());
            rest = &after[end + 1..];
        }
        found
    }

    #[test]
    fn every_language_is_available() {
        let backend = crate::_rust_i18n_backend();
        let available: Vec<String> = backend
            .available_locales()
            .iter()
            .map(|locale| locale.to_string())
            .collect();
        for language in Language::ALL {
            assert!(
                available.contains(&language.code().to_string()),
                "{available:?}"
            );
        }
    }

    #[test]
    fn every_key_is_translated_in_every_language() {
        let english = messages("en");
        assert!(!english.is_empty(), "no English messages were loaded");
        for language in Language::ALL {
            let table = messages(language.code());
            for (key, source) in &english {
                let text = table
                    .get(key)
                    .unwrap_or_else(|| panic!("{key} is missing in {}", language.code()));
                assert!(
                    !text.trim().is_empty(),
                    "{key} is empty in {}",
                    language.code()
                );
                assert_eq!(
                    placeholders(text),
                    placeholders(source),
                    "{key} has different placeholders in {}",
                    language.code()
                );
            }
            for key in table.keys() {
                assert!(
                    english.contains_key(key),
                    "{key} exists in {} but not in English",
                    language.code()
                );
            }
        }
    }

    #[test]
    fn no_translation_contains_the_em_dash() {
        for language in Language::ALL {
            for (key, text) in messages(language.code()) {
                assert!(!text.contains('\u{2014}'), "{key} in {}", language.code());
            }
        }
    }

    #[test]
    fn plural_keys_come_in_pairs() {
        let english = messages("en");
        for key in english.keys() {
            if let Some(base) = key.strip_suffix(".one") {
                assert!(english.contains_key(&format!("{base}.other")), "{key}");
            }
            // `category.other` is the "Other" category, not a plural form.
            if let Some(base) = key.strip_suffix(".other")
                && base != "category"
            {
                assert!(english.contains_key(&format!("{base}.one")), "{key}");
            }
        }
    }

    /// Every `.rs` file under `dir`, recursively.
    fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    /// The literal keys of every `t!` call in `source` whose first argument is a string literal.
    fn literal_keys(source: &str) -> Vec<String> {
        let mut keys = Vec::new();
        let mut from = 0;
        while let Some(found) = source[from..].find("t!(\"") {
            let start = from + found;
            let body = start + 4;
            from = body;
            // `format!(`, `assert!(` ... also end in `t!(`: only a standalone `t!` counts.
            let previous = source[..start].chars().next_back();
            if previous.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            if let Some(end) = source[body..].find('"') {
                keys.push(source[body..body + end].to_string());
            }
        }
        keys
    }

    #[test]
    fn every_key_used_in_the_sources_exists() {
        let english = messages("en");
        let mut files = Vec::new();
        rust_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        assert!(!files.is_empty());
        let mut missing = Vec::new();
        for file in files {
            let source = fs::read_to_string(&file).expect("read source");
            for key in literal_keys(&source) {
                if !english.contains_key(&key) {
                    missing.push(format!("{}: {key}", file.display()));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "keys missing from locales/: {missing:#?}"
        );
    }

    #[test]
    fn literal_keys_skips_other_macros() {
        let source = "t!(\"a.b\") format!(\"x\") t!(\"c.d\", n = 1) assert!(\"y\")";
        assert_eq!(
            literal_keys(source),
            vec!["a.b".to_string(), "c.d".to_string()]
        );
    }
}
