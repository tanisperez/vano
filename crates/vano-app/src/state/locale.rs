//! The UI language: `telaio-app` has the languages, the system detection and the `language`
//! setting; this adds the plural helper, which reads Vano's own translation tables. The
//! translations live in `crates/vano-app/locales/` and are looked up with `t!`.
//!
//! The locale is process wide (`rust_i18n::set_locale`), so no test may call [`apply`]: tests
//! that need a language pass `locale = "xx"` to `t!` instead.

pub use telaio_app::language::*;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;

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
