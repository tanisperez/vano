//! Vano's palette: telaio's [`Palette`] built from one [`Tokens`] per accent (Indigo, Graphite,
//! Electric blue) and mode, plus the colors only Vano has, through [`VanoColors`]: the treemap
//! categories and the cleanup fills.
//!
//! The accent changes the accent color and the neutrals; the status colors, the treemap
//! categories and the cleanup fills are shared by the three accents. Indigo is telaio's own
//! default tokens. Every hex literal of Vano is written once, below (keep them in sync with
//! `docs/design-system.md`).

use std::sync::OnceLock;

use gpui_kit::{Hsla, Rgba, white};
use telaio_ui::Tokens;
use telaio_ui::palette::hex;
pub use telaio_ui::{Palette, PaletteExt};
use vano_core::Category;

use crate::state::usage::Severity;

use crate::state::settings::Accent;

const GRAPHITE_LIGHT: Tokens = Tokens {
    bg: "#fcfcfc",
    surface: "#f5f5f5",
    border: "#e5e5e5",
    border_strong: "#d0d0d0",
    fg: "#171717",
    fg_muted: "#5e5e5e",
    fg_subtle: "#8f8f8f",
    // `fg` at 7.5%, like telaio's own light tokens.
    hover: "#17171713",
    accent: "#171717",
    accent_text: "#171717",
    accent_subtle: "#ececec",
    ..Tokens::DEFAULT_LIGHT
};

const GRAPHITE_DARK: Tokens = Tokens {
    bg: "#141414",
    surface: "#1a1a1a",
    raised: "#232323",
    overlay: "#262626",
    border: "#2d2d2d",
    border_strong: "#3d3d3d",
    fg: "#ededed",
    fg_muted: "#a3a3a3",
    fg_subtle: "#717171",
    hover: "#ffffff0d",
    accent: "#ededed",
    accent_fg: "#141414",
    accent_text: "#ffffff",
    accent_subtle: "#2e2e2e",
    ..Tokens::DEFAULT_DARK
};

const ELECTRIC_LIGHT: Tokens = Tokens {
    bg: "#fbfcfe",
    surface: "#f2f5f9",
    border: "#e0e6ee",
    border_strong: "#c9d2de",
    fg: "#111827",
    fg_muted: "#556070",
    fg_subtle: "#8792a2",
    hover: "#11182713",
    accent: "#0b63f6",
    accent_text: "#0052d6",
    accent_subtle: "#e3edff",
    ..Tokens::DEFAULT_LIGHT
};

const ELECTRIC_DARK: Tokens = Tokens {
    bg: "#0f131a",
    surface: "#141922",
    raised: "#1b222d",
    overlay: "#1f2632",
    border: "#263040",
    border_strong: "#344156",
    fg: "#e8edf5",
    fg_muted: "#9aa6b8",
    fg_subtle: "#677489",
    hover: "#ffffff0d",
    accent: "#2f7bff",
    accent_text: "#8db8ff",
    accent_subtle: "#15284a",
    ..Tokens::DEFAULT_DARK
};

/// The tokens of `accent` in light or dark mode, for telaio's theme family. Indigo is telaio's
/// defaults.
pub fn tokens(accent: Accent, dark: bool) -> Tokens {
    match (accent, dark) {
        (Accent::Indigo, false) => Tokens::DEFAULT_LIGHT,
        (Accent::Indigo, true) => Tokens::DEFAULT_DARK,
        (Accent::Graphite, false) => GRAPHITE_LIGHT,
        (Accent::Graphite, true) => GRAPHITE_DARK,
        (Accent::Electric, false) => ELECTRIC_LIGHT,
        (Accent::Electric, true) => ELECTRIC_DARK,
    }
}

/// The palette of `accent` in light or dark mode, for previews of an accent other than the
/// active one (the accent cards of Settings).
pub fn accent_palette(accent: Accent, dark: bool) -> Palette {
    Palette::new(&tokens(accent, dark), dark)
}

/// The strengths of the cleanup fills and lines of one mode.
struct Cleanup {
    /// `--safe-fill`: `success` mixed into `raised` at this strength.
    safe_fill_mix: f32,
    /// `--safe-hatch`: `success` at this alpha.
    safe_hatch_alpha: f32,
    /// `--safe-line`: `success` at this alpha.
    safe_line_alpha: f32,
    /// `--review-line`: `warning` at this alpha.
    review_line_alpha: f32,
}

const CLEANUP_LIGHT: Cleanup = Cleanup {
    safe_fill_mix: 0.09,
    safe_hatch_alpha: 0.38,
    safe_line_alpha: 0.62,
    review_line_alpha: 0.50,
};

const CLEANUP_DARK: Cleanup = Cleanup {
    safe_fill_mix: 0.12,
    safe_hatch_alpha: 0.34,
    safe_line_alpha: 0.55,
    review_line_alpha: 0.45,
};

/// The `--cat-*` colors of one mode, in [`Category::ALL`] order: background of a parent tile, fill
/// of a leaf tile, border, legend dot. The design's `oklch()` values converted to sRGB hex.
type CategoryTokens = [[&'static str; 4]; 6];

const CATEGORIES_LIGHT: CategoryTokens = [
    ["#fff4ec", "#ffe2ce", "#ebaf86", "#eb7c33"],
    ["#f5f7f9", "#e9ebef", "#c0c4cc", "#858d99"],
    ["#f5f5ff", "#e2e3ff", "#b1b0ef", "#796ce0"],
    ["#ebfafe", "#cbeff9", "#7cc8db", "#01aac6"],
    ["#fff2f8", "#ffdcec", "#eba6c6", "#db509c"],
    ["#f0f8ff", "#d5e8ff", "#95baea", "#2a80e2"],
];

const CATEGORIES_DARK: CategoryTokens = [
    ["#2a1d15", "#412714", "#7a4b29", "#f0995b"],
    ["#1e1f21", "#292b2f", "#494d54", "#8b939f"],
    ["#1e1d2d", "#2b294b", "#535088", "#9c96f8"],
    ["#132226", "#0c333c", "#1d6270", "#4dc3dd"],
    ["#2a1a21", "#422232", "#7d415f", "#f080b8"],
    ["#17202c", "#1c2f46", "#365987", "#6aa7f4"],
];

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

/// The category colors of light (`false`) or dark (`true`) mode, parsed once.
fn categories(dark: bool) -> &'static [CategoryColors; 6] {
    static ALL: OnceLock<[[CategoryColors; 6]; 2]> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        let parse = |tokens: &CategoryTokens| {
            tokens.map(|[bg, fill, line, dot]| CategoryColors {
                bg: hex(bg),
                fill: hex(fill),
                line: hex(line),
                dot: hex(dot),
            })
        };
        [parse(&CATEGORIES_LIGHT), parse(&CATEGORIES_DARK)]
    });
    &all[usize::from(dark)]
}

/// Vano's domain colors, read from the active [`Palette`] (`cx.palette().category(..)`).
pub trait VanoColors {
    /// The colors of a treemap category.
    fn category(&self, category: Category) -> CategoryColors;
    /// Fill of a tile that is safe to remove.
    fn safe_fill(&self) -> Hsla;
    /// The diagonal hatch over a safe tile.
    fn safe_hatch(&self) -> Hsla;
    /// Border of a safe tile.
    fn safe_line(&self) -> Hsla;
    /// Fill of a tile that needs review (Cleanup coloring).
    fn review_fill(&self) -> Hsla;
    /// Border of a tile that needs review.
    fn review_line(&self) -> Hsla;
    /// The diagonal hatch over the part of a disk usage bar that a cleanup would free.
    fn usage_hatch(&self) -> Hsla;
    /// The colors of a disk usage bar at [`crate::state::usage::GRADIENT_STOPS`].
    fn usage_gradient(&self) -> [Hsla; 5];
    /// The color of a disk usage figure of `severity`.
    fn usage_severity(&self, severity: Severity) -> Hsla;
}

impl VanoColors for Palette {
    fn usage_hatch(&self) -> Hsla {
        // On top of the gradient, whatever the mode.
        white().opacity(0.75)
    }

    fn usage_gradient(&self) -> [Hsla; 5] {
        [
            self.success,
            mix_oklab(self.success, self.warning, 0.55),
            self.warning,
            mix_oklab(self.warning, self.danger, 0.6),
            self.danger,
        ]
    }

    fn usage_severity(&self, severity: Severity) -> Hsla {
        match severity {
            Severity::Low => self.success,
            Severity::Mid => self.warning,
            Severity::High => self.danger,
        }
    }

    fn category(&self, category: Category) -> CategoryColors {
        let index = Category::ALL
            .iter()
            .position(|c| *c == category)
            .unwrap_or(1);
        categories(self.dark)[index]
    }

    fn safe_fill(&self) -> Hsla {
        let mix = cleanup(self.dark).safe_fill_mix;
        self.raised.blend(self.success.opacity(mix))
    }

    fn safe_hatch(&self) -> Hsla {
        self.success.opacity(cleanup(self.dark).safe_hatch_alpha)
    }

    fn safe_line(&self) -> Hsla {
        self.success.opacity(cleanup(self.dark).safe_line_alpha)
    }

    fn review_fill(&self) -> Hsla {
        self.warning_subtle
    }

    fn review_line(&self) -> Hsla {
        self.warning.opacity(cleanup(self.dark).review_line_alpha)
    }
}

fn cleanup(dark: bool) -> &'static Cleanup {
    if dark { &CLEANUP_DARK } else { &CLEANUP_LIGHT }
}

/// `from` and `to` mixed in the Oklab color space, `amount` of the way to `to`.
pub fn mix_oklab(from: Hsla, to: Hsla, amount: f32) -> Hsla {
    let (a, b) = (to_oklab(from), to_oklab(to));
    let t = amount.clamp(0.0, 1.0);
    let mixed = [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
    let [r, g, blue] = from_oklab(mixed);
    Hsla::from(Rgba {
        r,
        g,
        b: blue,
        a: from.a + (to.a - from.a) * t,
    })
}

/// The transfer function of sRGB, one channel to linear light.
fn to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// One channel of linear light back to sRGB, clamped to its gamut.
fn from_linear(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

fn to_oklab(color: Hsla) -> [f32; 3] {
    let rgba = Rgba::from(color);
    let (r, g, b) = (to_linear(rgba.r), to_linear(rgba.g), to_linear(rgba.b));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab([big_l, a, b]: [f32; 3]) -> [f32; 3] {
    let l = (big_l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (big_l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (big_l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        from_linear(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        from_linear(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s),
        from_linear(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Round-trips an [`Hsla`] back to a `#rrggbb` string, sidestepping float precision noise.
    fn to_hex(color: Hsla) -> String {
        let rgba = Rgba::from(color);
        format!(
            "#{:02x}{:02x}{:02x}",
            (rgba.r * 255.0).round() as u8,
            (rgba.g * 255.0).round() as u8,
            (rgba.b * 255.0).round() as u8,
        )
    }

    #[test]
    fn oklab_mixing_keeps_its_ends_and_passes_through_the_middle() {
        let (black, white) = (hex("#000000"), hex("#ffffff"));
        assert_eq!(to_hex(mix_oklab(black, white, 0.0)), "#000000");
        assert_eq!(to_hex(mix_oklab(black, white, 1.0)), "#ffffff");
        // Half way in Oklab is lighter than half way in sRGB (#808080).
        assert_eq!(to_hex(mix_oklab(black, white, 0.5)), "#636363");
    }

    #[test]
    fn palettes_match_design_tokens() {
        assert_eq!(
            to_hex(accent_palette(Accent::Indigo, false).accent),
            "#4f57d8"
        );
        assert_eq!(to_hex(accent_palette(Accent::Indigo, true).bg), "#15161a");
        assert_eq!(
            to_hex(accent_palette(Accent::Graphite, true).accent),
            "#ededed"
        );
        assert_eq!(
            to_hex(accent_palette(Accent::Electric, false).accent),
            "#0b63f6"
        );
    }

    #[test]
    fn categories_are_shared_by_every_accent() {
        let a = accent_palette(Accent::Indigo, false).category(Category::Ai);
        let b = accent_palette(Accent::Electric, false).category(Category::Ai);
        assert_eq!(a, b);
        assert_eq!(to_hex(a.dot), "#db509c");
    }

    #[test]
    fn hover_shades_go_the_right_way() {
        let light = accent_palette(Accent::Indigo, false);
        assert!(light.accent_hover.l < light.accent.l);
        let dark = accent_palette(Accent::Indigo, true);
        assert!(dark.accent_hover.l > dark.accent.l);
    }

    #[test]
    fn every_accent_keeps_the_status_colors() {
        for accent in Accent::ALL {
            let palette = accent_palette(accent, false);
            assert_eq!(to_hex(palette.success), "#1d8657");
            assert_eq!(to_hex(palette.warning), "#9a6500");
        }
    }
}
