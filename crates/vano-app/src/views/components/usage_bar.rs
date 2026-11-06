//! [`UsageBar`]: how full a disk is, on a gradient from green to red that belongs to the track.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::state::usage::segments;
use crate::theme::palette::mix_oklab;
use crate::theme::{PaletteExt, VanoColors};

/// What marks the part of the bar a cleanup would free.
#[derive(Debug, Clone, Copy)]
pub enum UsageMark {
    /// Hatched on top of the fill: the bytes still there.
    Hatch,
    /// Outlined on the empty track: the room the cleanup would leave.
    Outline,
}

/// A disk usage bar filled `fraction` of the way.
#[derive(IntoElement)]
pub struct UsageBar {
    fraction: f32,
    height: f32,
    round_fill: bool,
    mark: Option<(f32, f32, UsageMark)>,
}

impl UsageBar {
    pub fn new(fraction: f32, height: f32) -> Self {
        Self {
            fraction,
            height,
            round_fill: false,
            mark: None,
        }
    }

    /// Rounds the end of the fill too, as the track's.
    pub fn round_fill(mut self) -> Self {
        self.round_fill = true;
        self
    }

    /// Marks the bar between `from` and `to`, both from 0 to 1.
    pub fn mark(mut self, from: f32, to: f32, mark: UsageMark) -> Self {
        self.mark = Some((from, to, mark));
        self
    }
}

impl RenderOnce for UsageBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.palette();
        let colors = palette.usage_gradient();
        let radius = px(self.height / 2.0);
        let pieces = segments(self.fraction);
        let last = pieces.len().saturating_sub(1);
        let hatch = palette.usage_hatch();
        let (raised, success, success_subtle) =
            (palette.raised, palette.success, palette.success_subtle);
        div()
            .relative()
            .w_full()
            .h(px(self.height))
            .rounded(radius)
            .bg(palette.hover)
            .overflow_hidden()
            .children(pieces.into_iter().enumerate().map(move |(index, piece)| {
                let start = colors[piece.stop];
                let end = mix_oklab(start, colors[piece.stop + 1], piece.reach);
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(piece.from))
                    .w(relative(piece.to - piece.from))
                    .when(index == 0, |piece| {
                        piece.rounded_tl(radius).rounded_bl(radius)
                    })
                    // gpui clips to a rectangle, so the end of a full bar rounds itself.
                    .when(
                        index == last && (self.round_fill || self.fraction >= 1.0),
                        |piece| piece.rounded_tr(radius).rounded_br(radius),
                    )
                    .bg(linear_gradient(
                        90.,
                        linear_color_stop(start, 0.),
                        linear_color_stop(end, 1.),
                    )
                    .color_space(ColorSpace::Oklab))
            }))
            .children(
                self.mark
                    .filter(|(from, to, _)| to > from)
                    .map(|(from, to, mark)| {
                        let area = div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(relative(from))
                            .w(relative(to - from));
                        match mark {
                            UsageMark::Hatch => area
                                .border_l_1()
                                .border_color(raised)
                                .bg(pattern_slash(hatch, 2.0, 6.0)),
                            UsageMark::Outline => {
                                area.border_1().border_color(success).bg(success_subtle)
                            }
                        }
                    }),
            )
    }
}
