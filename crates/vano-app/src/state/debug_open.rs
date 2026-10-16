//! Parses the `VANO_OPEN` debug hook: a hidden way to start in a given UI state for screenshots
//! and manual tests, not a supported feature, not surfaced in any menu.

use std::path::PathBuf;

/// A UI state `VANO_OPEN` can request at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugOpenTarget {
    /// The Settings modal on one of its panes.
    Settings(crate::state::settings::SettingsCategory),
    /// Scans this folder right away (`scan:<path>`).
    Scan(PathBuf),
    /// Scans the home folder right away.
    ScanHome,
    /// Opens the results of the most recent scan.
    Results,
}

/// Parses `VANO_OPEN`'s value. An unrecognized value is `None`, silently ignored.
pub fn parse(value: &str) -> Option<DebugOpenTarget> {
    use crate::state::settings::SettingsCategory;
    match value {
        "settings" => Some(DebugOpenTarget::Settings(SettingsCategory::Appearance)),
        "settings-scanning" => Some(DebugOpenTarget::Settings(SettingsCategory::Scanning)),
        "settings-advanced" => Some(DebugOpenTarget::Settings(SettingsCategory::Advanced)),
        "settings-about" => Some(DebugOpenTarget::Settings(SettingsCategory::About)),
        "scan" => Some(DebugOpenTarget::ScanHome),
        "results" => Some(DebugOpenTarget::Results),
        other => other
            .strip_prefix("scan:")
            .filter(|path| !path.is_empty())
            .map(|path| DebugOpenTarget::Scan(PathBuf::from(path))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_targets() {
        assert_eq!(parse("scan"), Some(DebugOpenTarget::ScanHome));
        assert_eq!(
            parse("scan:/tmp"),
            Some(DebugOpenTarget::Scan(PathBuf::from("/tmp")))
        );
        assert_eq!(parse("scan:"), None);
        assert_eq!(parse("nope"), None);
    }
}
