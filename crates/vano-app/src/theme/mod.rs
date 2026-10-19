//! Vano's visual foundations, on top of telaio: the palette (telaio's default tokens are the
//! Indigo accent, plus the domain colors in [`palette`]), layout metrics, and the "Vano
//! Light"/"Vano Dark" theme family built from the settings' accent and fonts.
//!
//! [`install`] does the startup work through `telaio_ui::install`: register the bundled fonts,
//! load the theme family into gpui-kit's `ThemeRegistry` and install the palettes `cx.palette()`
//! reads. [`apply_settings`] does the same live after a Settings change.

pub mod metrics;
pub mod palette;

// Re-exported for views (`cx.palette()`).
pub use palette::{Palette, PaletteExt, VanoColors};

use gpui_kit::App;

use crate::state::settings::Settings;

/// Loads the Vano Light/Dark theme family built from `settings` and makes it the registry's
/// default light and dark themes. Call once, after `gpui_kit::init`, before opening any window;
/// the window's own `Theme::change`/`Theme::sync_system_appearance` call is what makes the loaded
/// family actually visible.
pub fn install(cx: &mut App, settings: &Settings) {
    telaio_ui::install(cx, &theme_family(settings));
}

/// Rebuilds the "Vano Light"/"Vano Dark" themes from `settings` and assigns them as the active
/// `Theme`'s light/dark themes, live (every Settings change updates the global `Theme`
/// immediately). The caller still has to apply the theme choice afterwards to repaint the window
/// with the new config.
pub fn apply_settings(cx: &mut App, settings: &Settings) {
    telaio_ui::apply(cx, &theme_family(settings));
}

/// The theme family: the tokens of the settings' accent and its fonts.
fn theme_family(settings: &Settings) -> telaio_ui::ThemeFamily {
    telaio_ui::ThemeFamily::new("Vano")
        .tokens(
            palette::tokens(settings.accent, false),
            palette::tokens(settings.accent, true),
        )
        .fonts(telaio_ui::Fonts {
            ui_family: settings.ui_font.clone().into(),
            ui_size: settings.ui_font_size,
            mono_family: settings.mono_font.clone().into(),
            mono_size: settings.mono_font_size(),
        })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use gpui_kit::component::ThemeSet;
    use pretty_assertions::assert_eq;

    #[test]
    fn family_json_parses_as_a_theme_set_with_both_modes() {
        let settings = Settings::default();
        let json = theme_family(&settings).to_json().to_string();

        let theme_set: ThemeSet =
            serde_json::from_str(&json).expect("the theme family must parse as a ThemeSet");

        assert_eq!(theme_set.themes.len(), 2);
        assert_eq!(theme_set.themes[0].name.as_ref(), "Vano Light");
        assert!(!theme_set.themes[0].mode.is_dark());
        assert_eq!(theme_set.themes[1].name.as_ref(), "Vano Dark");
        assert!(theme_set.themes[1].mode.is_dark());
    }

    #[test]
    fn family_json_carries_the_settings_fonts() {
        let settings = Settings::default();
        let json = theme_family(&settings).to_json();

        assert_eq!(json["themes"][0]["font.family"], "Geist");
        assert_eq!(json["themes"][0]["mono_font.family"], "Geist Mono");
        assert_eq!(json["themes"][0]["font.size"], 13.0);
        assert_eq!(json["themes"][0]["mono_font.size"], 12.5);
    }

    #[test]
    fn family_json_colors_follow_the_accent() {
        let mut settings = Settings::default();
        let json = theme_family(&settings).to_json();
        assert_eq!(json["themes"][0]["colors"]["background"], "#fcfcfd");
        assert_eq!(json["themes"][0]["colors"]["primary.background"], "#4f57d8");
        assert_eq!(json["themes"][1]["colors"]["background"], "#15161a");
        assert_eq!(json["themes"][1]["colors"]["primary.background"], "#5b63e6");

        settings.accent = crate::state::settings::Accent::Graphite;
        let json = theme_family(&settings).to_json();
        assert_eq!(json["themes"][0]["colors"]["primary.background"], "#171717");
        assert_eq!(json["themes"][1]["colors"]["primary.background"], "#ededed");
    }
}
