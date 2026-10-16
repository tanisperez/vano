#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

use vano_rules::{Status, rule_ids, rule_status};

const LANGS: [&str; 4] = ["en", "es", "gl", "it"];

/// Parses the rust-i18n v2 file: `"key":` lines followed by `  lang: "text"` lines.
fn load() -> HashMap<String, HashMap<String, String>> {
    let text = include_str!("../locales/rules.yml");
    let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut key = String::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with('_') || line.trim().is_empty() {
            continue;
        }
        if let Some(k) = line.strip_prefix('"').and_then(|l| l.strip_suffix("\":")) {
            key = k.to_string();
            map.entry(key.clone()).or_default();
        } else {
            let (lang, value) = line.trim().split_once(": ").expect("lang line");
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .expect("quoted value");
            let value = value.replace("\\\"", "\"").replace("\\\\", "\\");
            map.get_mut(&key).unwrap().insert(lang.to_string(), value);
        }
    }
    map
}

fn assert_complete(map: &HashMap<String, HashMap<String, String>>, key: &str) {
    let entry = map.get(key).unwrap_or_else(|| panic!("missing key {key}"));
    for lang in LANGS {
        let v = entry
            .get(lang)
            .unwrap_or_else(|| panic!("{key} lacks {lang}"));
        assert!(!v.trim().is_empty(), "{key} {lang} is empty");
    }
    assert_eq!(entry.len(), LANGS.len(), "{key} has unknown languages");
}

#[test]
fn every_rule_has_its_texts_in_four_languages() {
    let map = load();
    for id in rule_ids() {
        assert_complete(&map, &format!("rules.{id}.name"));
        assert_complete(&map, &format!("rules.{id}.desc"));
        let reason = format!("rules.{id}.reason");
        if rule_status(id) == Some(Status::Review) {
            assert_complete(&map, &reason);
        } else {
            assert!(!map.contains_key(&reason), "{id} is Safe and has a reason");
        }
    }
    for cat in ["media", "other", "code", "sdk", "ai", "caches"] {
        assert_complete(&map, &format!("category.{cat}"));
    }
}

#[test]
fn no_orphan_keys_and_no_em_dash() {
    let text = include_str!("../locales/rules.yml");
    assert!(!text.contains('\u{2014}'));
    for key in load().keys() {
        let ok = key.starts_with("category.")
            || key
                .strip_prefix("rules.")
                .and_then(|r| r.rsplit_once('.'))
                .is_some_and(|(id, _)| rule_ids().contains(&id));
        assert!(ok, "orphan key {key}");
    }
}

#[test]
fn no_exclamation_marks() {
    assert!(!include_str!("../locales/rules.yml").contains('!'));
}
