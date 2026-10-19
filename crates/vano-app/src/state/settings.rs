//! Application settings, persisted to `<config dir>/vano/settings.toml`
//! and applied live: there is no explicit
//! "Save" action, every change made through the Settings view writes
//! the file immediately.

use std::fs;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use rust_i18n::t;
use serde::Serialize;

pub use telaio_app::logging::LogLevel;

use super::locale::LanguageChoice;

/// The path, relative to the OS config directory, of the settings file.
const SETTINGS_FILE: &str = "vano/settings.toml";

/// Valid range of [`Settings::ui_font_size`], in points.
pub const UI_FONT_SIZE_RANGE: RangeInclusive<f32> = 11.0..=16.0;

/// Step of the UI font size stepper in the Settings view.
pub const UI_FONT_SIZE_STEP: f32 = 1.0;

/// Which theme mode the app follows, chosen in Settings, "Appearance".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// Follow the OS light/dark setting.
    #[default]
    System,
    /// Always use the light theme.
    Light,
    /// Always use the dark theme.
    Dark,
}

impl From<ThemeChoice> for telaio_ui::ThemeChoice {
    fn from(choice: ThemeChoice) -> Self {
        match choice {
            ThemeChoice::System => telaio_ui::ThemeChoice::System,
            ThemeChoice::Light => telaio_ui::ThemeChoice::Light,
            ThemeChoice::Dark => telaio_ui::ThemeChoice::Dark,
        }
    }
}

impl ThemeChoice {
    /// Parses a theme name as read from the settings file, case-insensitive. Returns `None` for
    /// anything else, so the caller can fall back to the default.
    fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "system" => Some(ThemeChoice::System),
            "light" => Some(ThemeChoice::Light),
            "dark" => Some(ThemeChoice::Dark),
            _ => None,
        }
    }
}

/// A category of the Settings view's left nav.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsCategory {
    /// Theme, fonts and sizes.
    #[default]
    Appearance,
    /// What the scan shows while it runs.
    Scanning,
    /// Diagnostics: the log level and the log file.
    Advanced,
    /// The version, the release links and the updater.
    About,
}

impl SettingsCategory {
    /// Every category, in the order the nav lists them.
    pub const ALL: [SettingsCategory; 4] = [
        SettingsCategory::Appearance,
        SettingsCategory::Scanning,
        SettingsCategory::Advanced,
        SettingsCategory::About,
    ];

    /// The nav item label, also the pane's title.
    pub fn label(self) -> String {
        match self {
            SettingsCategory::Appearance => t!("settings.category.appearance"),
            SettingsCategory::Scanning => t!("settings.category.scanning"),
            SettingsCategory::Advanced => t!("settings.category.advanced"),
            SettingsCategory::About => t!("settings.category.about"),
        }
        .into_owned()
    }
}

/// The accent palette, chosen in Settings, "Appearance". It changes the accent and the neutrals;
/// treemap and status colors stay the same. Stored as `accent = "indigo" | "graphite" |
/// "electric"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Indigo,
    Graphite,
    Electric,
}

impl Accent {
    /// Every accent, in the order the Settings view lists them.
    pub const ALL: [Accent; 3] = [Accent::Indigo, Accent::Graphite, Accent::Electric];

    fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "indigo" => Some(Accent::Indigo),
            "graphite" => Some(Accent::Graphite),
            "electric" => Some(Accent::Electric),
            _ => None,
        }
    }

    /// The label shown under the accent's card.
    pub fn label(self) -> String {
        match self {
            Accent::Indigo => t!("settings.accent.indigo"),
            Accent::Graphite => t!("settings.accent.graphite"),
            Accent::Electric => t!("settings.accent.electric"),
        }
        .into_owned()
    }
}

/// What the left panel shows while a scan runs, chosen in Settings, "Scanning", and switchable
/// during a scan. Stored as `live_view = "treemap" | "rings" | "ranking"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveView {
    /// Tiles appear and grow as each folder is read.
    #[default]
    Treemap,
    /// A ring split by category fills up as bytes are counted.
    Rings,
    /// The ten largest folders so far, reordering as sizes change.
    Ranking,
}

impl LiveView {
    /// Every view, in the order the segmented control and Settings list them.
    pub const ALL: [LiveView; 3] = [LiveView::Treemap, LiveView::Rings, LiveView::Ranking];

    fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "treemap" => Some(LiveView::Treemap),
            "rings" => Some(LiveView::Rings),
            "ranking" => Some(LiveView::Ranking),
            _ => None,
        }
    }

    /// The view's short name.
    pub fn label(self) -> String {
        match self {
            LiveView::Treemap => t!("scan.view.treemap"),
            LiveView::Rings => t!("scan.view.rings"),
            LiveView::Ranking => t!("scan.view.ranking"),
        }
        .into_owned()
    }

    /// The view's one line description, in Settings.
    pub fn description(self) -> String {
        match self {
            LiveView::Treemap => t!("settings.live_view.treemap_desc"),
            LiveView::Rings => t!("settings.live_view.rings_desc"),
            LiveView::Ranking => t!("settings.live_view.ranking_desc"),
        }
        .into_owned()
    }
}

/// User-configurable settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settings {
    /// The UI language: automatic (the system's) or a fixed one.
    pub language: LanguageChoice,
    /// The theme mode: system, light or dark.
    pub theme: ThemeChoice,
    /// The accent palette.
    pub accent: Accent,
    /// The UI font family name.
    pub ui_font: String,
    /// The UI font size, in points, clamped to [`UI_FONT_SIZE_RANGE`].
    pub ui_font_size: f32,
    /// The monospace font family, used for sizes, paths and counters.
    pub mono_font: String,
    /// What the scan shows while it runs.
    pub live_view: LiveView,
    /// How much is written to the log file.
    pub log_level: LogLevel,
    /// Whether to check for a new version once at startup. Only used by builds that have an
    /// updater (`state::update::updater_enabled`).
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: LanguageChoice::default(),
            theme: ThemeChoice::default(),
            accent: Accent::default(),
            ui_font: "Geist".to_string(),
            ui_font_size: 13.0,
            mono_font: "Geist Mono".to_string(),
            live_view: LiveView::default(),
            log_level: LogLevel::default(),
            check_updates: true,
        }
    }
}

impl Settings {
    /// The monospace size that goes with the UI size: Geist Mono looks a touch larger than Geist
    /// at the same size, so it is half a point smaller, like the design.
    pub fn mono_font_size(&self) -> f32 {
        self.ui_font_size - 0.5
    }
}

/// Loads the settings from `<config dir>/vano/settings.toml`, or [`Settings::default`] if the
/// OS config directory is unknown, the file is missing, or its content is not valid TOML.
pub fn load_settings() -> Settings {
    match dirs::config_dir() {
        Some(base) => read_settings(&base),
        None => Settings::default(),
    }
}

/// Saves `settings` to `<config dir>/vano/settings.toml`, creating the folder if needed.
/// Failing to persist this is not worth interrupting the user over, so any error (a missing
/// config directory, a read-only filesystem, ...) is only logged.
pub fn save_settings(settings: &Settings) {
    if let Some(base) = dirs::config_dir() {
        write_settings(&base, settings);
    }
}

/// The real path of the settings file, `None` when the OS config directory is unknown. Used by the
/// Settings view's "Saved to" line, shortened with `~` by `state::format::shorten_path`.
pub fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join(SETTINGS_FILE))
}

/// Steps `current` by `step` (positive or negative), clamped to `range`. Used by the Settings
/// view's font size stepper: the `-`/`+` buttons pass `-step`/`step` of [`UI_FONT_SIZE_STEP`],
/// with [`UI_FONT_SIZE_RANGE`].
pub fn step_size(current: f32, step: f32, range: RangeInclusive<f32>) -> f32 {
    (current + step).clamp(*range.start(), *range.end())
}

/// Builds the options for a font picker: `bundled` first, always, then every other entry of
/// `installed` (as returned by `cx.text_system().all_font_names()`, already sorted), skipping the
/// bundled family's own name if it repeats there and de-duplicating.
pub fn font_options(bundled: &str, installed: &[String]) -> Vec<String> {
    let mut options = vec![bundled.to_string()];
    for name in installed {
        if name != bundled && !options.contains(name) {
            options.push(name.clone());
        }
    }
    options
}

/// Reads and parses `<base>/vano/settings.toml`. Split out from [`load_settings`] so tests can
/// point `base` at a temporary directory instead of the real, per-user OS config directory.
///
/// Every field is read and validated independently: a missing file, a field of the wrong type,
/// or an out-of-range number falls back to its own default instead of discarding the whole file,
/// and a key this version does not know about is silently ignored.
fn read_settings(base: &Path) -> Settings {
    let Ok(content) = fs::read_to_string(base.join(SETTINGS_FILE)) else {
        return Settings::default();
    };
    parse_settings(&content)
}

/// Parses the content of a settings file into [`Settings`], falling back field by field to
/// [`Settings::default`] as documented on [`read_settings`].
fn parse_settings(content: &str) -> Settings {
    let defaults = Settings::default();
    let Ok(value) = content.parse::<toml::Value>() else {
        return defaults;
    };
    let table = value.as_table();
    let get = |key: &str| table.and_then(|table| table.get(key));
    let get_str = |key: &str| get(key).and_then(toml::Value::as_str);

    Settings {
        language: get_str("language")
            .and_then(LanguageChoice::parse)
            .unwrap_or(defaults.language),
        theme: get_str("theme")
            .and_then(ThemeChoice::parse)
            .unwrap_or(defaults.theme),
        accent: get_str("accent")
            .and_then(Accent::parse)
            .unwrap_or(defaults.accent),
        ui_font: get_str("ui_font")
            .map(str::to_string)
            .unwrap_or(defaults.ui_font),
        ui_font_size: get("ui_font_size")
            .and_then(as_f32)
            .map(|size| size.clamp(*UI_FONT_SIZE_RANGE.start(), *UI_FONT_SIZE_RANGE.end()))
            .unwrap_or(defaults.ui_font_size),
        mono_font: get_str("mono_font")
            .map(str::to_string)
            .unwrap_or(defaults.mono_font),
        live_view: get_str("live_view")
            .and_then(LiveView::parse)
            .unwrap_or(defaults.live_view),
        log_level: get_str("log_level")
            .and_then(LogLevel::parse)
            .unwrap_or(defaults.log_level),
        check_updates: get("check_updates")
            .and_then(toml::Value::as_bool)
            .unwrap_or(defaults.check_updates),
    }
}

/// Reads a TOML value as an `f32`, accepting both a float and an integer.
fn as_f32(value: &toml::Value) -> Option<f32> {
    value
        .as_float()
        .map(|value| value as f32)
        .or_else(|| value.as_integer().map(|value| value as f32))
}

/// Writes `settings` to `<base>/vano/settings.toml`, creating the folder if needed. Split out
/// from [`save_settings`] for the same testing reason as [`read_settings`].
fn write_settings(base: &Path, settings: &Settings) {
    let path = base.join(SETTINGS_FILE);
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(error) = fs::create_dir_all(parent) {
        log::warn!("could not create {}: {error}", parent.display());
        return;
    }
    let text = match toml::to_string_pretty(settings) {
        Ok(text) => text,
        Err(error) => {
            log::warn!("could not serialize the settings: {error}");
            return;
        }
    };
    if let Err(error) = fs::write(&path, text) {
        log::warn!("could not save {}: {error}", path.display());
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn defaults_are_the_documented_values() {
        let settings = Settings::default();
        assert_eq!(settings.language, LanguageChoice::Auto);
        assert_eq!(settings.theme, ThemeChoice::System);
        assert_eq!(settings.accent, Accent::Indigo);
        assert_eq!(settings.ui_font, "Geist");
        assert_eq!(settings.ui_font_size, 13.0);
        assert_eq!(settings.mono_font, "Geist Mono");
        assert_eq!(settings.live_view, LiveView::Treemap);
        assert_eq!(settings.log_level, LogLevel::Info);
        assert!(settings.check_updates);
    }

    #[test]
    fn missing_file_reads_as_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(read_settings(dir.path()), Settings::default());
    }

    #[test]
    fn round_trips_through_a_config_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let settings = Settings {
            language: LanguageChoice::Gl,
            theme: ThemeChoice::Dark,
            accent: Accent::Electric,
            ui_font: "Inter".to_string(),
            ui_font_size: 14.0,
            mono_font: "Fira Code".to_string(),
            live_view: LiveView::Ranking,
            log_level: LogLevel::Trace,
            check_updates: false,
        };
        write_settings(dir.path(), &settings);
        assert_eq!(read_settings(dir.path()), settings);
    }

    #[test]
    fn invalid_toml_reads_as_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("vano")).expect("create vano dir");
        fs::write(dir.path().join(SETTINGS_FILE), "not valid toml {{{").expect("write file");
        assert_eq!(read_settings(dir.path()), Settings::default());
    }

    #[test]
    fn an_invalid_field_falls_back_on_its_own_without_discarding_the_rest() {
        let settings = parse_settings("theme = \"dark\"\nui_font_size = \"big\"\naccent = 3\n");
        assert_eq!(settings.theme, ThemeChoice::Dark);
        assert_eq!(settings.ui_font_size, Settings::default().ui_font_size);
        assert_eq!(settings.accent, Accent::Indigo);
    }

    #[test]
    fn ui_font_size_is_clamped_to_its_range() {
        assert_eq!(parse_settings("ui_font_size = 40").ui_font_size, 16.0);
        assert_eq!(parse_settings("ui_font_size = 2").ui_font_size, 11.0);
    }
}
