//! Vano's keyboard and mouse shortcuts, for telaio's cheat sheet (`F1`, or `Cmd+Shift+/` on
//! macOS). It mirrors `main.rs`'s `bind_keys` and the explorer's mouse gestures, so a new binding
//! gets a row here. Plain data, unit tested without a window.

use rust_i18n::t;
use telaio_ui::components::{MODIFIER_KEY, Shortcut, ShortcutGroup};

/// Every shortcut, grouped. Built when the cheat sheet opens, in the current language.
pub fn groups() -> Vec<ShortcutGroup> {
    let modifier = MODIFIER_KEY;
    vec![
        ShortcutGroup {
            title: t!("shortcuts.group.general").to_string(),
            shortcuts: vec![
                Shortcut::new(t!("shortcuts.action.choose_folder"), &[modifier, "O"]),
                Shortcut::new(t!("shortcuts.action.start_scan"), &["\u{21b5}"]),
                Shortcut::new(t!("shortcuts.action.settings"), &[modifier, ","]),
                Shortcut::open_sheet(),
            ],
        },
        ShortcutGroup {
            title: t!("shortcuts.group.explorer").to_string(),
            shortcuts: vec![
                Shortcut::new(t!("shortcuts.action.up"), &["Backspace"]),
                Shortcut::new(t!("shortcuts.action.back"), &["Alt", "\u{2190}"]),
                Shortcut::new(t!("shortcuts.action.forward"), &["Alt", "\u{2192}"]),
                Shortcut::new(t!("shortcuts.action.close_detail"), &["Esc"]),
            ],
        },
        ShortcutGroup {
            title: t!("shortcuts.group.mouse").to_string(),
            shortcuts: vec![
                Shortcut::new(t!("shortcuts.action.open"), &[&t!("shortcuts.key.click")]),
                Shortcut::new(
                    t!("shortcuts.action.details"),
                    &[&t!("shortcuts.key.right_click")],
                ),
                Shortcut::new(
                    t!("shortcuts.action.back"),
                    &[&t!("shortcuts.key.mouse_back")],
                ),
                Shortcut::new(
                    t!("shortcuts.action.forward"),
                    &[&t!("shortcuts.key.mouse_forward")],
                ),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_group_and_shortcut_has_text_and_keys() {
        for group in groups() {
            assert!(!group.title.is_empty());
            assert!(!group.shortcuts.is_empty());
            for shortcut in &group.shortcuts {
                assert!(!shortcut.label.is_empty());
                assert!(!shortcut.keys.is_empty());
            }
        }
    }

    #[test]
    fn no_two_shortcuts_share_the_same_keys() {
        let mut seen = std::collections::HashSet::new();
        for group in groups() {
            for shortcut in group.shortcuts {
                assert!(
                    seen.insert(shortcut.keys.join("+")),
                    "duplicate keys for {:?}",
                    shortcut.label
                );
            }
        }
    }

    #[test]
    fn the_cheat_sheet_lists_its_own_keys() {
        assert!(
            groups()
                .iter()
                .flat_map(|group| &group.shortcuts)
                .any(|shortcut| shortcut.keys == telaio_ui::components::open_keys())
        );
    }
}
