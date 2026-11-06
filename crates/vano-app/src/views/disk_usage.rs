//! The results' disk usage panel: the disk now and after the selected cleanup, next to the stats.

use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use vano_scan::DiskUsage;

use crate::state::format;
use crate::state::number::format_decimal;
use crate::state::results::Results;
use crate::state::target::disk_name;
use crate::state::usage::{Severity, UsagePreview};
use crate::theme::metrics::{
    USAGE_BAR_HEIGHT, USAGE_LABEL_WIDTH, USAGE_PERCENT_WIDTH, USAGE_STATS_MIN_WIDTH,
};
use crate::theme::{Palette, VanoColors};

use super::common::Stat;
use super::components::{UsageBar, UsageMark};
use super::root::AppView;

impl AppView {
    /// The panel's content for the disk `fs` that holds `results`, with `stats` next to the bars.
    pub(crate) fn render_disk_usage(
        &self,
        results: &Results,
        fs: DiskUsage,
        stats: Vec<Stat>,
        palette: &Palette,
        mono: &SharedString,
    ) -> Div {
        let preview = UsagePreview::new(fs, results.selected_size());
        // The system's disk by its name; anything else (an external drive) by the scan's target.
        let name = if self.disk.is_some_and(|disk| disk.total == fs.total) {
            disk_name()
        } else {
            results.target.label()
        };
        let (now, after) = (preview.fraction(), preview.fraction_after());
        let selected = preview.freed > 0;
        let (delta, delta_color) = if selected {
            (
                t!(
                    "results.usage_delta",
                    points = format_decimal(f64::from(now - after) * 100.0, 1),
                    size = format::size(preview.freed)
                )
                .into_owned(),
                palette.success,
            )
        } else {
            (t!("results.usage_hint").into_owned(), palette.fg_subtle)
        };
        let header = h_flex()
            .flex_wrap()
            .items_baseline()
            .gap_x(px(10.0))
            .gap_y(px(2.0))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("results.usage_title")),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(palette.fg_muted)
                    .child(format!("{name} \u{b7} {}", format::size(preview.total))),
            )
            .child(
                h_flex()
                    .gap(px(6.0))
                    .items_center()
                    .text_size(px(11.5))
                    .text_color(palette.fg_subtle)
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .w(px(18.0))
                            .h(px(8.0))
                            .rounded(px(2.0))
                            .bg(palette.danger)
                            .overflow_hidden()
                            .child(div().absolute().inset_0().bg(pattern_slash(
                                palette.usage_hatch(),
                                1.5,
                                5.0,
                            ))),
                    )
                    .child(t!("results.usage_legend")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .font_family(mono.clone())
                    .text_size(px(12.0))
                    .text_color(delta_color)
                    .child(delta),
            );
        let bars = v_flex()
            .flex_1()
            .min_w_0()
            .px(px(16.0))
            .pt(px(12.0))
            .pb(px(14.0))
            .gap(px(10.0))
            .border_r_1()
            .border_color(palette.border)
            .child(header)
            .child(usage_row(
                t!("results.usage_now").into(),
                preview.used,
                preview.free,
                now,
                selected.then_some((after, now, UsageMark::Hatch)),
                palette,
                mono,
            ))
            .child(usage_row(
                t!("results.usage_after").into(),
                preview.used_after,
                preview.free_after,
                after,
                selected.then_some((after, now, UsageMark::Outline)),
                palette,
                mono,
            ));
        let count = stats.len();
        let cells = h_flex()
            .flex_none()
            .w(relative(0.38))
            .min_w(px(USAGE_STATS_MIN_WIDTH))
            .items_stretch()
            .children(stats.into_iter().enumerate().map(|(index, stat)| {
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .px(px(14.0))
                    .py(px(12.0))
                    .justify_center()
                    .gap(px(6.0))
                    .when(index + 1 < count, |cell| {
                        cell.border_r_1().border_color(palette.border)
                    })
                    .child(
                        h_flex()
                            .gap(px(6.0))
                            .items_center()
                            .min_w_0()
                            .text_size(px(12.0))
                            .text_color(palette.fg_muted)
                            .children(stat.dot.map(|dot| {
                                div().flex_none().size(px(6.0)).rounded(px(3.0)).bg(dot)
                            }))
                            .child(div().truncate().child(stat.label)),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(22.0))
                            .line_height(px(26.0))
                            .whitespace_nowrap()
                            .child(stat.value),
                    )
                    .child(
                        div()
                            .truncate()
                            .font_family(mono.clone())
                            .text_size(px(11.5))
                            .text_color(stat.sub_color)
                            .child(stat.sub),
                    )
            }));
        h_flex()
            .flex_none()
            .items_stretch()
            .child(bars)
            .child(cells)
    }
}

/// One bar with its label, what it means in bytes, and its percentage.
fn usage_row(
    label: SharedString,
    used: u64,
    free: u64,
    fraction: f32,
    mark: Option<(f32, f32, UsageMark)>,
    palette: &Palette,
    mono: &SharedString,
) -> Div {
    let mut bar = UsageBar::new(fraction, USAGE_BAR_HEIGHT);
    if let Some((from, to, kind)) = mark {
        bar = bar.mark(from, to, kind);
    }
    h_flex()
        .items_center()
        .gap(px(14.0))
        .child(
            v_flex()
                .flex_none()
                .w(px(USAGE_LABEL_WIDTH))
                .gap(px(1.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(mono.clone())
                        .text_size(px(11.0))
                        .text_color(palette.fg_subtle)
                        .child(t!(
                            "results.usage_figures",
                            used = format::size(used),
                            free = format::size(free)
                        )),
                ),
        )
        .child(div().flex_1().min_w_0().child(bar))
        .child(
            div()
                .flex_none()
                .w(px(USAGE_PERCENT_WIDTH))
                .text_right()
                .font_family(mono.clone())
                .text_size(px(16.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.usage_severity(Severity::of(fraction)))
                .child(format!(
                    "{}%",
                    format_decimal(f64::from(fraction) * 100.0, 1)
                )),
        )
}
