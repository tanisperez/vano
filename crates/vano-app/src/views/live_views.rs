//! The Rings and Ranking live views of the scan. (The Treemap live view is `treemap_view`.)
//!
//! Both animate toward the numbers of the last poll: the ring's segments ease toward the new
//! sums while a bright comet runs around it, the ranking rows spring to their new places.

use std::f32::consts::TAU;
use std::time::Instant;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use vano_core::Category;
use vano_core::treemap::Rect;

use crate::state::format;
use crate::state::motion::Motion;
use crate::theme::metrics::{RANK_ROW_HEIGHT, RING_SIZE, RING_THICKNESS};
use crate::theme::{Palette, VanoColors};

/// The ring's segment order, like the design.
const RING_ORDER: [Category; 6] = [
    Category::Code,
    Category::Ai,
    Category::Caches,
    Category::Sdk,
    Category::Media,
    Category::Other,
];

/// How fast the segments catch up with the counted bytes (per second).
const EASE_RATE: f32 = 7.0;
/// Comet speed, in turns per second.
const COMET_SPEED: f32 = 0.45;

/// The animated state of the ring.
#[derive(Debug, Clone, Default)]
pub struct RingState {
    /// Bytes per category as drawn now, eased toward `target`.
    shown: [f32; 6],
    target: [f32; 6],
    /// The total the ring is a fraction of.
    pub total: f32,
    /// The comet's angle, in turns.
    comet: f32,
    last: Option<Instant>,
}

impl RingState {
    /// Sets the counted bytes per category and the expected total.
    pub fn set(&mut self, sums: [u64; 6], total: u64) {
        self.target = sums.map(|v| v as f32);
        let counted: f32 = self.target.iter().sum();
        self.total = (total as f32).max(counted).max(1.0);
    }

    /// Advances the easing and the comet. Returns whether another frame is needed.
    pub fn step(&mut self, spinning: bool) -> bool {
        let now = Instant::now();
        let dt = self
            .last
            .map(|last| now.duration_since(last).as_secs_f32())
            .unwrap_or(1.0 / 60.0)
            .min(0.1);
        let k = 1.0 - (-EASE_RATE * dt).exp();
        let mut moving = spinning;
        for (shown, target) in self.shown.iter_mut().zip(self.target) {
            *shown += (target - *shown) * k;
            if (target - *shown).abs() > target.max(1.0) * 0.001 {
                moving = true;
            } else {
                *shown = target;
            }
        }
        if spinning {
            self.comet = (self.comet + dt * COMET_SPEED).fract();
        }
        self.last = moving.then_some(now);
        moving
    }

    /// The bytes drawn now for a category.
    fn shown(&self, category: Category) -> f32 {
        RING_ORDER
            .iter()
            .position(|c| *c == category)
            .map(|i| self.shown[i])
            .unwrap_or(0.0)
    }
}

/// Sums `(category, bytes)` pairs into the ring order.
pub fn ring_sums(items: impl IntoIterator<Item = (Category, u64)>) -> [u64; 6] {
    let mut sums = [0u64; 6];
    for (category, size) in items {
        if let Some(i) = RING_ORDER.iter().position(|c| *c == category) {
            sums[i] += size;
        }
    }
    sums
}

/// The ring, its center text and the legend.
pub fn render_rings(
    ring: &RingState,
    palette: &Palette,
    mono: &SharedString,
    center: SharedString,
    sub: SharedString,
    spinning: bool,
) -> AnyElement {
    let segments: Vec<(f32, Hsla)> = RING_ORDER
        .iter()
        .map(|c| (ring.shown(*c) / ring.total, palette.category(*c).dot))
        .collect();
    let track = palette.hover;
    let comet = spinning.then_some(ring.comet);
    let comet_color = palette.raised;
    let drawing = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let size = f32::from(bounds.size.width.min(bounds.size.height));
            let cx = f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0;
            let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0;
            let outer = size / 2.0;
            let inner = outer - RING_THICKNESS;
            paint_arc(window, cx, cy, inner, outer, 0.0, 1.0, track);
            let mut start = 0.0;
            for (fraction, color) in &segments {
                if *fraction > 0.0005 {
                    paint_arc(
                        window,
                        cx,
                        cy,
                        inner,
                        outer,
                        start,
                        start + fraction,
                        *color,
                    );
                }
                start += fraction;
            }
            if let Some(angle) = comet {
                // A tail of short arcs fading out behind the head.
                let steps = 24;
                for i in 0..steps {
                    let t = i as f32 / steps as f32;
                    let a0 = angle - 0.16 * (1.0 - t);
                    let a1 = a0 + 0.16 / steps as f32 + 0.002;
                    let color = comet_color.opacity(0.7 * t * t);
                    paint_arc(window, cx, cy, inner - 0.5, outer + 0.5, a0, a1, color);
                }
            }
        },
    )
    .absolute()
    .inset_0();

    let legend = h_flex()
        .flex_wrap()
        .justify_center()
        .max_w(px(760.0))
        .gap_x(px(20.0))
        .gap_y(px(8.0))
        .children(Category::ALL.into_iter().map(|category| {
            h_flex()
                .w(px(230.0))
                .gap(px(8.0))
                .items_center()
                .text_size(px(12.5))
                .child(
                    div()
                        .flex_none()
                        .size(px(8.0))
                        .rounded(px(2.0))
                        .bg(palette.category(category).dot),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(palette.fg_muted)
                        .child(crate::state::format::tr(&format!(
                            "category.{}",
                            category.id()
                        ))),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(mono.clone())
                        .child(format::size(ring.shown(category) as u64)),
                )
        }));

    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(28.0))
        .child(
            div()
                .relative()
                .size(px(RING_SIZE))
                .flex_none()
                .child(drawing)
                .child(
                    v_flex()
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_size(px(28.0))
                                .child(center),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(palette.fg_subtle)
                                .child(sub),
                        ),
                ),
        )
        .child(legend)
        .into_any_element()
}

/// Fills the ring segment between `from` and `to` (in turns, 0 at the top, clockwise).
#[allow(clippy::too_many_arguments)]
fn paint_arc(
    window: &mut Window,
    cx: f32,
    cy: f32,
    inner: f32,
    outer: f32,
    from: f32,
    to: f32,
    color: Hsla,
) {
    let steps = (((to - from) * 160.0).ceil() as usize).max(2);
    let point = |radius: f32, turn: f32| {
        let angle = turn * TAU - TAU / 4.0;
        gpui_kit::point(px(cx + radius * angle.cos()), px(cy + radius * angle.sin()))
    };
    let mut path = PathBuilder::fill();
    path.move_to(point(outer, from));
    for i in 1..=steps {
        path.line_to(point(outer, from + (to - from) * i as f32 / steps as f32));
    }
    for i in (0..=steps).rev() {
        path.line_to(point(inner, from + (to - from) * i as f32 / steps as f32));
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// One row of the ranking.
#[derive(Debug, Clone)]
pub struct RankRow {
    pub name: SharedString,
    pub size: SharedString,
    pub category: Category,
    /// A folder with something inside: clicking the row opens it (results only).
    pub openable: bool,
}

/// A click on a ranking row: its node and whether it can be opened.
pub type RankClick = std::rc::Rc<dyn Fn(u32, bool, &mut Window, &mut App)>;

/// Rows shown by the ranking.
pub const RANK_ROWS: usize = 10;

/// Sets the ranking targets: row `i` sits at `i * RANK_ROW_HEIGHT`, and the rectangle's width
/// is the bar's fraction of the biggest entry.
pub fn set_ranking(motion: &mut Motion<u32, RankRow>, rows: Vec<(u32, u64, RankRow)>, max: u64) {
    let max = max.max(1) as f32;
    let targets = rows
        .into_iter()
        .enumerate()
        .map(|(rank, (key, size, row))| {
            let y = rank as f32 * RANK_ROW_HEIGHT;
            (
                key,
                Rect::new(0.0, y, size as f32 / max, RANK_ROW_HEIGHT),
                row,
            )
        });
    let below = RANK_ROWS as f32 * RANK_ROW_HEIGHT;
    motion.set_targets(
        targets,
        move |_, target| Rect::new(0.0, below, 0.0, target.h),
        move |_, current| Rect::new(0.0, below, current.w, current.h),
    );
}

/// The ranking rows at their animated places.
pub fn render_ranking(
    motion: &Motion<u32, RankRow>,
    palette: &Palette,
    mono: &SharedString,
    on_click: Option<RankClick>,
) -> AnyElement {
    let rank_of = |y: f32| (y / RANK_ROW_HEIGHT).round() as usize + 1;
    div()
        .relative()
        .size_full()
        .px(px(16.0))
        .children(motion.frames().map(|frame| {
            let rect = frame.rect;
            let row = frame.data;
            let rank = rank_of(rect.y);
            let color = palette.category(row.category).dot;
            let key = *frame.key;
            let openable = row.openable;
            let click = on_click.clone().filter(|_| !frame.exiting);
            h_flex()
                .id(("rank-row", key as usize))
                .absolute()
                .left(px(16.0))
                .right(px(16.0))
                .top(px(rect.y))
                .h(px(RANK_ROW_HEIGHT))
                .gap(px(12.0))
                .items_center()
                .rounded(px(6.0))
                .opacity(frame.opacity)
                .when_some(click, |row, click| {
                    row.cursor_pointer()
                        .hover(|style| style.bg(palette.hover))
                        .on_click(move |_, window, cx| click(key, openable, window, cx))
                })
                .child(
                    div()
                        .w(px(18.0))
                        .flex_none()
                        .text_right()
                        .font_family(mono.clone())
                        .text_size(px(11.0))
                        .text_color(palette.fg_subtle)
                        .child(if frame.exiting {
                            String::new()
                        } else {
                            rank.to_string()
                        }),
                )
                .child(div().flex_none().size(px(8.0)).rounded(px(2.0)).bg(color))
                .child(
                    div()
                        .w(px(120.0))
                        .flex_none()
                        .truncate()
                        .text_size(px(13.0))
                        .child(row.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(8.0))
                        .rounded(px(4.0))
                        .bg(palette.hover)
                        .child(
                            div()
                                .h_full()
                                .w(relative(rect.w.clamp(0.0, 1.0)))
                                .rounded(px(4.0))
                                .bg(color),
                        ),
                )
                .child(
                    div()
                        .w(px(76.0))
                        .flex_none()
                        .text_right()
                        .font_family(mono.clone())
                        .text_size(px(13.0))
                        .child(row.size.clone()),
                )
        }))
        .into_any_element()
}
