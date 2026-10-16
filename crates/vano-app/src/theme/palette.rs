//! Vano's color palette: one [`Palette`] per accent (Indigo, Graphite, Electric blue) and mode
//! (light, dark), built from the design tokens of `docs/design-system.md`, and
//! [`PaletteExt`], the extension trait views use to reach the active one.
//!
//! Every hex literal is written once, below. The accent changes the accent color and the
//! neutrals; the status colors, the treemap categories and the cleanup fills are shared by the
//! three accents. Both [`Palette`] and the gpui-kit theme JSON (built in `theme::mod` through
//! [`theme_colors`]) derive from the same tokens.

use std::sync::OnceLock;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, BoxShadow, Global, Hsla, Rgba, px};
use vano_core::Category;

use crate::state::settings::Accent;

/// The neutrals and the accent of one accent palette in one mode.
struct Base {
    bg: &'static str,
    surface: &'static str,
    raised: &'static str,
    overlay: &'static str,
    border: &'static str,
    border_strong: &'static str,
    fg: &'static str,
    fg_muted: &'static str,
    fg_subtle: &'static str,
    hover: &'static str,
    accent: &'static str,
    accent_fg: &'static str,
    accent_text: &'static str,
    accent_subtle: &'static str,
}

const INDIGO_LIGHT: Base = Base {
    bg: "#fcfcfd",
    surface: "#f4f5f7",
    raised: "#ffffff",
    overlay: "#ffffff",
    border: "#e3e5ea",
    border_strong: "#cdd0d8",
    fg: "#1b1c22",
    fg_muted: "#5c606e",
    fg_subtle: "#8b8e9a",
    hover: "#eceef2",
    accent: "#4f57d8",
    accent_fg: "#ffffff",
    accent_text: "#3f47c4",
    accent_subtle: "#e8e9fd",
};

const INDIGO_DARK: Base = Base {
    bg: "#15161a",
    surface: "#1b1c21",
    raised: "#23252c",
    overlay: "#262830",
    border: "#2c2e36",
    border_strong: "#3b3e48",
    fg: "#e9eaee",
    fg_muted: "#a0a3ae",
    fg_subtle: "#6f7280",
    hover: "#25272e",
    accent: "#5b63e6",
    accent_fg: "#ffffff",
    accent_text: "#a3a9ff",
    accent_subtle: "#272b4d",
};

const GRAPHITE_LIGHT: Base = Base {
    bg: "#fcfcfc",
    surface: "#f5f5f5",
    raised: "#ffffff",
    overlay: "#ffffff",
    border: "#e5e5e5",
    border_strong: "#d0d0d0",
    fg: "#171717",
    fg_muted: "#5e5e5e",
    fg_subtle: "#8f8f8f",
    hover: "#eeeeee",
    accent: "#171717",
    accent_fg: "#ffffff",
    accent_text: "#171717",
    accent_subtle: "#ececec",
};

const GRAPHITE_DARK: Base = Base {
    bg: "#141414",
    surface: "#1a1a1a",
    raised: "#232323",
    overlay: "#262626",
    border: "#2d2d2d",
    border_strong: "#3d3d3d",
    fg: "#ededed",
    fg_muted: "#a3a3a3",
    fg_subtle: "#717171",
    hover: "#262626",
    accent: "#ededed",
    accent_fg: "#141414",
    accent_text: "#ffffff",
    accent_subtle: "#2e2e2e",
};

const ELECTRIC_LIGHT: Base = Base {
    bg: "#fbfcfe",
    surface: "#f2f5f9",
    raised: "#ffffff",
    overlay: "#ffffff",
    border: "#e0e6ee",
    border_strong: "#c9d2de",
    fg: "#111827",
    fg_muted: "#556070",
    fg_subtle: "#8792a2",
    hover: "#e9eef5",
    accent: "#0b63f6",
    accent_fg: "#ffffff",
    accent_text: "#0052d6",
    accent_subtle: "#e3edff",
};

const ELECTRIC_DARK: Base = Base {
    bg: "#0f131a",
    surface: "#141922",
    raised: "#1b222d",
    overlay: "#1f2632",
    border: "#263040",
    border_strong: "#344156",
    fg: "#e8edf5",
    fg_muted: "#9aa6b8",
    fg_subtle: "#677489",
    hover: "#1f2733",
    accent: "#2f7bff",
    accent_fg: "#ffffff",
    accent_text: "#8db8ff",
    accent_subtle: "#15284a",
};

/// Status colors, the cleanup fills' strengths and the elevation shadow of one mode.
struct Shared {
    success: &'static str,
    success_subtle: &'static str,
    warning: &'static str,
    warning_subtle: &'static str,
    danger: &'static str,
    info: &'static str,
    scrim: &'static str,
    /// `--safe-fill`: `success` mixed into `raised` at this strength.
    safe_fill_mix: f32,
    /// `--safe-hatch`: `success` at this alpha.
    safe_hatch_alpha: f32,
    /// `--safe-line`: `success` at this alpha.
    safe_line_alpha: f32,
    /// `--review-line`: `warning` at this alpha.
    review_line_alpha: f32,
    shadow: [ShadowLayer; 2],
    /// The `--cat-*` colors, in [`Category::ALL`] order: background of a parent tile, fill of a
    /// leaf tile, border, legend dot.
    categories: [[&'static str; 4]; 6],
}

/// One layer of the `--shadow` token.
struct ShadowLayer {
    offset_y: f32,
    blur: f32,
    spread: f32,
    color: (u8, u8, u8, f32),
}

// The category colors are the design's `oklch()` values converted to sRGB hex.
const SHARED_LIGHT: Shared = Shared {
    success: "#1d8657",
    success_subtle: "#e2f4ea",
    warning: "#9a6500",
    warning_subtle: "#fbf0d9",
    danger: "#cf3535",
    info: "#1c6fb0",
    scrim: "#0a0b1073",
    safe_fill_mix: 0.09,
    safe_hatch_alpha: 0.38,
    safe_line_alpha: 0.62,
    review_line_alpha: 0.50,
    shadow: [
        ShadowLayer {
            offset_y: 12.0,
            blur: 32.0,
            spread: 0.0,
            color: (20, 22, 40, 0.14),
        },
        ShadowLayer {
            offset_y: 0.0,
            blur: 0.0,
            spread: 1.0,
            color: (20, 22, 40, 0.06),
        },
    ],
    categories: [
        ["#fff4ec", "#ffe2ce", "#ebaf86", "#eb7c33"],
        ["#f5f7f9", "#e9ebef", "#c0c4cc", "#858d99"],
        ["#f5f5ff", "#e2e3ff", "#b1b0ef", "#796ce0"],
        ["#ebfafe", "#cbeff9", "#7cc8db", "#01aac6"],
        ["#fff2f8", "#ffdcec", "#eba6c6", "#db509c"],
        ["#f0f8ff", "#d5e8ff", "#95baea", "#2a80e2"],
    ],
};

const SHARED_DARK: Shared = Shared {
    success: "#4fc88f",
    success_subtle: "#15302a",
    warning: "#e8b75a",
    warning_subtle: "#352b15",
    danger: "#f27171",
    info: "#62b3f0",
    scrim: "#0a0b1073",
    safe_fill_mix: 0.12,
    safe_hatch_alpha: 0.34,
    safe_line_alpha: 0.55,
    review_line_alpha: 0.45,
    shadow: [
        ShadowLayer {
            offset_y: 12.0,
            blur: 32.0,
            spread: 0.0,
            color: (0, 0, 0, 0.45),
        },
        ShadowLayer {
            offset_y: 0.0,
            blur: 0.0,
            spread: 1.0,
            color: (255, 255, 255, 0.04),
        },
    ],
    categories: [
        ["#2a1d15", "#412714", "#7a4b29", "#f0995b"],
        ["#1e1f21", "#292b2f", "#494d54", "#8b939f"],
        ["#1e1d2d", "#2b294b", "#535088", "#9c96f8"],
        ["#132226", "#0c333c", "#1d6270", "#4dc3dd"],
        ["#2a1a21", "#422232", "#7d415f", "#f080b8"],
        ["#17202c", "#1c2f46", "#365987", "#6aa7f4"],
    ],
};

/// The four colors of one treemap category.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CategoryColors {
    /// Background of a parent tile (one that shows its children).
    pub bg: Hsla,
    /// Fill of a leaf tile.
    pub fill: Hsla,
    /// Border of both.
    pub line: Hsla,
    /// Legend dot, ring segment and ranking bar.
    pub dot: Hsla,
}

/// One [`Hsla`] field per design token. Views read colors only from here, through
/// [`PaletteExt::palette`], never from `cx.theme()` and never from a literal.
#[derive(Debug, Clone)]
pub struct Palette {
    /// Whether this is a dark palette.
    pub dark: bool,
    /// Background of the window's content.
    pub bg: Hsla,
    /// Title bar, status bar and panel headers.
    pub surface: Hsla,
    /// Cards, inputs, tiles without a category.
    pub raised: Hsla,
    /// Menus, popovers and modals.
    pub overlay: Hsla,
    /// The dimmed backdrop behind a modal.
    pub scrim: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub fg: Hsla,
    pub fg_muted: Hsla,
    pub fg_subtle: Hsla,
    pub accent: Hsla,
    pub accent_fg: Hsla,
    pub accent_text: Hsla,
    pub accent_subtle: Hsla,
    pub hover: Hsla,
    /// Pressed background of hand-rolled clickable rows: `hover` two thirds of the way toward
    /// `border_strong` (the design defines no `:active` state).
    pub pressed: Hsla,
    pub accent_hover: Hsla,
    pub accent_pressed: Hsla,
    pub danger_hover: Hsla,
    pub danger_pressed: Hsla,
    pub success: Hsla,
    pub success_subtle: Hsla,
    pub warning: Hsla,
    pub warning_subtle: Hsla,
    pub danger: Hsla,
    /// Fill of a tile that is safe to remove.
    pub safe_fill: Hsla,
    /// The diagonal hatch over a safe tile.
    pub safe_hatch: Hsla,
    /// Border of a safe tile.
    pub safe_line: Hsla,
    /// Fill of a tile that needs review (Cleanup coloring).
    pub review_fill: Hsla,
    /// Border of a tile that needs review.
    pub review_line: Hsla,
    /// The elevation shadow of menus and modals.
    pub shadow: Vec<BoxShadow>,
    categories: [CategoryColors; 6],
}

impl Palette {
    fn build(base: &Base, shared: &Shared, dark: bool) -> Self {
        let sign = if dark { 1.0 } else { -1.0 };
        let accent = hex(base.accent);
        let danger = hex(shared.danger);
        let success = hex(shared.success);
        let warning = hex(shared.warning);
        let raised = hex(base.raised);
        let categories = shared
            .categories
            .map(|[bg, fill, line, dot]| CategoryColors {
                bg: hex(bg),
                fill: hex(fill),
                line: hex(line),
                dot: hex(dot),
            });
        Self {
            dark,
            bg: hex(base.bg),
            surface: hex(base.surface),
            raised,
            overlay: hex(base.overlay),
            scrim: hex(shared.scrim),
            border: hex(base.border),
            border_strong: hex(base.border_strong),
            fg: hex(base.fg),
            fg_muted: hex(base.fg_muted),
            fg_subtle: hex(base.fg_subtle),
            accent,
            accent_fg: hex(base.accent_fg),
            accent_text: hex(base.accent_text),
            accent_subtle: hex(base.accent_subtle),
            hover: hex(base.hover),
            pressed: hex(base.hover).blend(hex(base.border_strong).opacity(2.0 / 3.0)),
            accent_hover: shade(accent, sign * SHADE_STEP),
            accent_pressed: shade(accent, sign * SHADE_STEP * 2.0),
            danger_hover: shade(danger, sign * SHADE_STEP),
            danger_pressed: shade(danger, sign * SHADE_STEP * 2.0),
            success,
            success_subtle: hex(shared.success_subtle),
            warning,
            warning_subtle: hex(shared.warning_subtle),
            danger,
            safe_fill: raised.blend(success.opacity(shared.safe_fill_mix)),
            safe_hatch: success.opacity(shared.safe_hatch_alpha),
            safe_line: success.opacity(shared.safe_line_alpha),
            review_fill: hex(shared.warning_subtle),
            review_line: warning.opacity(shared.review_line_alpha),
            shadow: shared.shadow.iter().map(shadow_layer).collect(),
            categories,
        }
    }

    /// The palette of `accent` in light or dark mode, built once and cached.
    pub fn get(accent: Accent, dark: bool) -> &'static Palette {
        static ALL: OnceLock<[Palette; 6]> = OnceLock::new();
        let all = ALL.get_or_init(|| {
            [
                Palette::build(&INDIGO_LIGHT, &SHARED_LIGHT, false),
                Palette::build(&INDIGO_DARK, &SHARED_DARK, true),
                Palette::build(&GRAPHITE_LIGHT, &SHARED_LIGHT, false),
                Palette::build(&GRAPHITE_DARK, &SHARED_DARK, true),
                Palette::build(&ELECTRIC_LIGHT, &SHARED_LIGHT, false),
                Palette::build(&ELECTRIC_DARK, &SHARED_DARK, true),
            ]
        });
        let index = match accent {
            Accent::Indigo => 0,
            Accent::Graphite => 2,
            Accent::Electric => 4,
        } + usize::from(dark);
        &all[index]
    }

    /// The colors of a treemap category.
    pub fn category(&self, category: Category) -> CategoryColors {
        let index = Category::ALL
            .iter()
            .position(|c| *c == category)
            .unwrap_or(1);
        self.categories[index]
    }
}

/// The accent the app shows, a gpui global set from the settings at startup and on every change.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActiveAccent(pub Accent);

impl Global for ActiveAccent {}

/// One-call access to the palette of the active accent and theme mode.
pub trait PaletteExt {
    /// The active [`Palette`].
    fn palette(&self) -> &'static Palette;
}

impl PaletteExt for App {
    fn palette(&self) -> &'static Palette {
        let accent = self
            .try_global::<ActiveAccent>()
            .map(|a| a.0)
            .unwrap_or_default();
        Palette::get(accent, self.theme().is_dark())
    }
}

/// The `"colors"` JSON of one mode's gpui-kit `ThemeConfig`, for the stock widgets (title bar,
/// buttons, dialogs, scrollbars, inputs, switches). See `docs/gpui-notes.md` of Vano for why
/// each key is set the way it is.
pub(super) fn theme_colors(accent: Accent, dark: bool) -> serde_json::Value {
    let base = match (accent, dark) {
        (Accent::Indigo, false) => &INDIGO_LIGHT,
        (Accent::Indigo, true) => &INDIGO_DARK,
        (Accent::Graphite, false) => &GRAPHITE_LIGHT,
        (Accent::Graphite, true) => &GRAPHITE_DARK,
        (Accent::Electric, false) => &ELECTRIC_LIGHT,
        (Accent::Electric, true) => &ELECTRIC_DARK,
    };
    let shared = if dark { &SHARED_DARK } else { &SHARED_LIGHT };
    let palette = Palette::get(accent, dark);
    serde_json::json!({
        "background": base.bg,
        "sidebar.background": base.surface,
        "title_bar.background": base.surface,
        "tab_bar.background": base.surface,
        "secondary.background": base.raised,
        "popover.background": base.overlay,
        // The dialog backdrop: transparent, the app paints `Palette::scrim` itself.
        "overlay": "#00000000",
        "border": base.border,
        "scrollbar.background": "#00000000",
        "scrollbar.thumb.background": base.border_strong,
        "scrollbar.thumb.hover.background": base.fg_subtle,
        "input.border": base.border_strong,
        "foreground": base.fg,
        "muted.foreground": base.fg_muted,
        "primary.background": base.accent,
        "primary.foreground": base.accent_fg,
        "link": base.accent_text,
        "accent.background": base.accent_subtle,
        "selection.background": format!("{}{SELECTION_ALPHA_HEX}", base.accent),
        "list.active.background": base.accent_subtle,
        "list.hover.background": base.hover,
        "button.hover.background": base.hover,
        "table.hover.background": base.hover,
        "success.background": shared.success,
        "warning.background": shared.warning,
        "danger.background": shared.danger,
        "info.background": shared.info,
        "button.primary.background": base.accent,
        "button.primary.foreground": base.accent_fg,
        "button.primary.hover.background": hex_string(palette.accent_hover),
        "button.primary.active.background": hex_string(palette.accent_pressed),
        "button.secondary.background": base.raised,
        "button.secondary.foreground": base.fg,
        "button.secondary.hover.background": base.hover,
        "button.secondary.active.background": hex_string(palette.pressed),
        "button.danger.background": shared.danger,
        "button.danger.foreground": "#ffffff",
        "button.danger.hover.background": hex_string(palette.danger_hover),
        "button.danger.active.background": hex_string(palette.danger_pressed),
        "switch.background": base.border_strong,
        "switch.thumb.background": "#ffffff",
    })
}

/// Builds a [`BoxShadow`] from one `--shadow` layer. gpui's `blur_radius` is the gaussian's
/// standard deviation, half of what the same number means in CSS.
fn shadow_layer(layer: &ShadowLayer) -> BoxShadow {
    let (r, g, b, a) = layer.color;
    BoxShadow::new(px(0.0), px(layer.offset_y), rgba(r, g, b, a))
        .blur_radius(px(layer.blur / 2.0))
        .spread_radius(px(layer.spread))
}

/// Parses `#rrggbb` or `#rrggbbaa`. Every call site passes a constant of this file; a bad
/// literal falls back to black instead of panicking.
fn hex(value: &str) -> Hsla {
    let digits = value.trim_start_matches('#');
    let parsed = u32::from_str_radix(digits, 16).unwrap_or(0);
    let (rgb, alpha) = if digits.len() == 8 {
        (parsed >> 8, (parsed & 0xff) as f32 / 255.0)
    } else {
        (parsed, 1.0)
    };
    rgba(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
        alpha,
    )
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
    .into()
}

/// `4d` is 0.3, the cap gpui-component applies to `selection.background`.
const SELECTION_ALPHA_HEX: &str = "4d";

/// One hover shade step for solid buttons, in HSL lightness.
const SHADE_STEP: f32 = 0.07;

/// Shifts `color`'s lightness by `delta`, keeping hue, saturation and alpha.
fn shade(color: Hsla, delta: f32) -> Hsla {
    Hsla {
        l: (color.l + delta).clamp(0.0, 1.0),
        ..color
    }
}

/// Formats an opaque [`Hsla`] as `#rrggbb`.
fn hex_string(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    format!(
        "#{:02x}{:02x}{:02x}",
        (rgba.r * 255.0).round() as u8,
        (rgba.g * 255.0).round() as u8,
        (rgba.b * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn palettes_match_design_tokens() {
        assert_eq!(
            hex_string(Palette::get(Accent::Indigo, false).accent),
            "#4f57d8"
        );
        assert_eq!(hex_string(Palette::get(Accent::Indigo, true).bg), "#15161a");
        assert_eq!(
            hex_string(Palette::get(Accent::Graphite, true).accent),
            "#ededed"
        );
        assert_eq!(
            hex_string(Palette::get(Accent::Electric, false).accent),
            "#0b63f6"
        );
    }

    #[test]
    fn categories_are_shared_by_every_accent() {
        let a = Palette::get(Accent::Indigo, false).category(Category::Ai);
        let b = Palette::get(Accent::Electric, false).category(Category::Ai);
        assert_eq!(a, b);
        assert_eq!(hex_string(a.dot), "#db509c");
    }

    #[test]
    fn hover_shades_go_the_right_way() {
        let light = Palette::get(Accent::Indigo, false);
        assert!(light.accent_hover.l < light.accent.l);
        let dark = Palette::get(Accent::Indigo, true);
        assert!(dark.accent_hover.l > dark.accent.l);
    }
}
