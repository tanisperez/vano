//! Pieces shared by the screens: the panel card, its header, the stats strip and the legend.

use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::Palette;
use crate::theme::metrics::{CARD_HEADER_HEIGHT, RADIUS_LG, STAT_HEIGHT};

/// A raised, bordered panel with radius 10 that clips its content.
pub fn panel(palette: &Palette) -> Div {
    v_flex()
        .min_h_0()
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(palette.border)
        .bg(palette.raised)
        .overflow_hidden()
}

/// A panel's header: title, subtitle, and an optional control on the right.
pub fn card_header(
    palette: &Palette,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    right: Option<AnyElement>,
) -> Div {
    h_flex()
        .flex_none()
        .h(px(CARD_HEADER_HEIGHT))
        .px(px(16.0))
        .gap(px(12.0))
        .items_center()
        .border_b_1()
        .border_color(palette.border)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(3.0))
                .child(
                    div()
                        .text_size(px(14.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.into()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.0))
                        .text_color(palette.fg_muted)
                        .child(subtitle.into()),
                ),
        )
        .children(right)
}

/// One cell of the stats strip.
pub struct Stat {
    pub label: SharedString,
    pub dot: Option<Hsla>,
    pub value: SharedString,
    pub sub: SharedString,
    pub sub_color: Hsla,
}

/// The row of four stats at the top of the scanning and results screens.
pub fn stats_row(palette: &Palette, mono: &SharedString, stats: Vec<Stat>) -> Div {
    let count = stats.len();
    h_flex()
        .flex_none()
        .h(px(STAT_HEIGHT))
        .children(stats.into_iter().enumerate().map(|(index, stat)| {
            v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .px(px(16.0))
                .justify_center()
                .gap(px(10.0))
                .when(index + 1 < count, |cell| {
                    cell.border_r_1().border_color(palette.border)
                })
                .child(
                    h_flex()
                        .gap(px(6.0))
                        .items_center()
                        .text_size(px(12.5))
                        .text_color(palette.fg_muted)
                        .children(
                            stat.dot
                                .map(|dot| div().size(px(6.0)).rounded(px(3.0)).bg(dot)),
                        )
                        .child(stat.label),
                )
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_baseline()
                        .min_w_0()
                        .child(
                            div()
                                .flex_none()
                                .font_family(mono.clone())
                                .text_size(px(24.0))
                                .line_height(px(28.0))
                                .child(stat.value),
                        )
                        .child(
                            div()
                                .truncate()
                                .font_family(mono.clone())
                                .text_size(px(12.0))
                                .text_color(stat.sub_color)
                                .child(stat.sub),
                        ),
                )
        }))
}

/// A small uppercase label above a group.
pub fn eyebrow(palette: &Palette, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.fg_subtle)
        .child(text.into().to_uppercase())
}

/// A legend item: a small swatch and a name.
pub fn legend_item(
    palette: &Palette,
    swatch_bg: Hsla,
    swatch_line: Hsla,
    hatch: Option<Hsla>,
    name: impl Into<SharedString>,
) -> Div {
    h_flex()
        .gap(px(6.0))
        .items_center()
        .text_size(px(12.0))
        .text_color(palette.fg_muted)
        .child(
            div()
                .relative()
                .size(px(10.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(swatch_line)
                .bg(swatch_bg)
                .overflow_hidden()
                .children(hatch.map(|color| {
                    div()
                        .absolute()
                        .inset_0()
                        .bg(pattern_slash(color, 1.0, 3.0))
                })),
        )
        .child(name.into())
}
