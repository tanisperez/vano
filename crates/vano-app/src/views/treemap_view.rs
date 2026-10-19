//! Draws a treemap from its spring animation: one absolutely positioned `div` per tile, at the
//! rectangle and opacity the springs give for this frame. Shared by the live view of the scan
//! and the explorer of the results.
//!
//! The area's size is only known while painting, so a zero-size `canvas` behind the tiles
//! records it; when it changes, the window draws again and the caller lays out for the new size.

use std::rc::Rc;

use gpui_kit::component::h_flex;
use gpui_kit::prelude::*;
use gpui_kit::*;
use vano_core::treemap::Rect;
use vano_core::{Category, NodeId};
use vano_rules::Status;

use crate::state::motion::Frame;
use crate::state::treemap::{PARENT_HEADER, Tile, TileKey, TileRole};
use crate::theme::{Palette, VanoColors};

use super::root::{AppView, ColorBy, Transition, TreemapState};

/// What a tile shows, captured when the layout is computed so drawing never touches the tree.
#[derive(Debug, Clone)]
pub struct TileView {
    pub node: NodeId,
    pub role: TileRole,
    pub openable: bool,
    pub label: SharedString,
    pub size_label: SharedString,
    pub category: Category,
    pub status: Option<Status>,
    /// The folder being read right now, outlined in the accent color.
    pub current: bool,
    /// A "N more" tile.
    pub more: bool,
    /// Share of the tile's bytes that Safe and Review recommendations inside it cover, for
    /// the Cleanup coloring of tiles that are not a recommendation themselves.
    pub reclaim: (f32, f32),
    /// The same in bytes, for a parent's header in Cleanup coloring.
    pub reclaim_bytes: u64,
}

/// A click on a tile: its key, what it shows, and whether it was the secondary button.
pub type TileClick = Rc<dyn Fn(TileKey, &TileView, bool, &mut Window, &mut App)>;

/// Sets the new layout as the springs' targets. `transition` says where new tiles come from and
/// where old ones go; without one, new tiles grow from their center and old ones shrink away.
pub fn apply_layout(
    treemap: &mut TreemapState,
    area: Rect,
    tiles: Vec<(Tile, TileView)>,
    transition: Option<Transition>,
) {
    let targets: Vec<(TileKey, Rect, TileView)> = tiles
        .iter()
        .map(|(tile, view)| (tile.key, tile.rect, view.clone()))
        .collect();
    match transition {
        Some(Transition::In(from)) => treemap.motion.set_targets(
            targets,
            |_, target| target.map(area, from),
            |_, current| current.map(from, area),
        ),
        Some(Transition::Out(child)) => {
            let into = tiles
                .iter()
                .find(|(tile, _)| tile.key == TileKey::Node(child))
                .map(|(tile, _)| tile.rect)
                .unwrap_or_else(|| area.collapsed());
            treemap.motion.set_targets(
                targets,
                |_, target| target.map(into, area),
                |_, current| current.map(area, into),
            )
        }
        None => treemap.motion.set_targets(
            targets,
            |_, target| target.collapsed(),
            |_, current| current.collapsed(),
        ),
    }
}

/// The treemap element, filling its parent.
pub fn render_treemap(
    id: &'static str,
    owner: WeakEntity<AppView>,
    treemap: &TreemapState,
    palette: &Palette,
    color_by: ColorBy,
    mono_font: SharedString,
    on_click: Option<TileClick>,
) -> AnyElement {
    let bounds_cell = treemap.bounds.clone();
    let measure = canvas(
        move |bounds, _, cx| {
            let previous = bounds_cell.replace(Some(bounds));
            // The layout depends on this size: draw the view again once it is known or changed.
            // Deferred, since a notify while painting is folded into the frame being drawn.
            if previous.map(|b| b.size) != Some(bounds.size) {
                cx.defer(move |cx| {
                    let _ = owner.update(cx, |_, cx| cx.notify());
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();

    let tiles = treemap
        .motion
        .frames()
        .filter(|frame| frame.rect.w >= 1.0 && frame.rect.h >= 1.0 && frame.opacity > 0.01)
        .map(|frame| render_tile(frame, palette, color_by, &mono_font, on_click.clone()))
        .collect::<Vec<_>>();

    div()
        .id(id)
        .relative()
        .size_full()
        .overflow_hidden()
        .child(measure)
        .children(tiles)
        .into_any_element()
}

/// The fill, border and hatch of a tile.
fn tile_colors(view: &TileView, palette: &Palette, color_by: ColorBy) -> (Hsla, Hsla, bool) {
    let cat = palette.category(view.category);
    let parent = view.role == TileRole::Parent;
    let (bg, line, hatch) = if view.more {
        (palette.surface, palette.border, false)
    } else if parent {
        match color_by {
            ColorBy::Category => (cat.bg, cat.line, false),
            ColorBy::Cleanup => (palette.raised, palette.border_strong, false),
        }
    } else {
        match (view.status, color_by) {
            (Some(Status::Safe), _) => (palette.safe_fill(), palette.safe_line(), true),
            (Some(Status::Review), ColorBy::Cleanup) => {
                (palette.review_fill(), palette.review_line(), false)
            }
            (_, ColorBy::Cleanup) => (palette.surface, palette.border, false),
            (_, ColorBy::Category) => (cat.fill, cat.line, false),
        }
    };
    let line = if view.current { palette.accent } else { line };
    (bg, line, hatch)
}

fn render_tile(
    frame: Frame<'_, TileKey, TileView>,
    palette: &Palette,
    color_by: ColorBy,
    mono_font: &SharedString,
    on_click: Option<TileClick>,
) -> AnyElement {
    let view = frame.data;
    let key = *frame.key;
    let r = frame.rect;
    let (bg, line, hatch) = tile_colors(view, palette, color_by);
    let parent = view.role == TileRole::Parent;
    let cleanup_band = color_by == ColorBy::Cleanup
        && !parent
        && view.status.is_none()
        && !view.more
        && view.reclaim.0 + view.reclaim.1 > 0.0;
    // In Cleanup coloring a parent's header tells how much could be freed inside it.
    let head_size: SharedString = if parent && color_by == ColorBy::Cleanup {
        if view.reclaim_bytes > 0 {
            format!(
                "\u{2193} {}",
                crate::state::format::size(view.reclaim_bytes)
            )
            .into()
        } else {
            SharedString::default()
        }
    } else {
        view.size_label.clone()
    };
    let head_size_color = if parent && color_by == ColorBy::Cleanup {
        palette.success
    } else {
        palette.fg_subtle
    };
    let radius = if view.role == TileRole::Kid { 5.0 } else { 6.0 };
    let show_head = r.w >= 46.0 && r.h >= 24.0;
    let show_head_size = parent && r.w >= 150.0;
    let big = !parent && !view.more && r.w >= 58.0 && r.h >= 50.0;
    let big_font = (r.w * r.h).sqrt() / 8.0;
    let big_font = big_font.clamp(12.0, 26.0);
    let interactive = on_click.is_some() && !frame.exiting;
    let hover_line = if view.openable || !view.more {
        palette.accent
    } else {
        line
    };

    let id = match key {
        TileKey::Node(node) => ElementId::from(("tile", node as usize)),
        TileKey::More(node) => ElementId::from(("tile-more", node as usize)),
    };

    let mut tile = div()
        .id(id)
        .absolute()
        .left(px(r.x))
        .top(px(r.y))
        .w(px(r.w))
        .h(px(r.h))
        .rounded(px(radius))
        .border_1()
        .border_color(line)
        .bg(bg)
        .opacity(frame.opacity)
        .overflow_hidden()
        .when(hatch, |tile| {
            tile.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(pattern_slash(palette.safe_hatch(), 1.5, 7.0)),
            )
        })
        .when(cleanup_band, |tile| {
            // What could be freed inside, as bands rising from the bottom: Safe (hatched
            // green) first, then Review (amber) above it.
            let (safe, review) = view.reclaim;
            let safe_h = (r.h * safe.clamp(0.0, 1.0)).max(if safe > 0.0 { 2.0 } else { 0.0 });
            let review_h = r.h * review.clamp(0.0, 1.0);
            tile.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(safe_h))
                    .h(px(review_h))
                    .bg(palette.review_fill())
                    .when(review_h > 0.0, |band| {
                        band.border_t_1().border_color(palette.review_line())
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(safe_h))
                    .bg(palette.safe_fill())
                    .when(safe_h > 0.0, |band| {
                        band.border_t_1().border_color(palette.safe_line())
                    })
                    .child(div().absolute().inset_0().bg(pattern_slash(
                        palette.safe_hatch(),
                        1.5,
                        7.0,
                    ))),
            )
        })
        .when(show_head, |tile| {
            tile.child(
                h_flex()
                    .relative()
                    .h(px(if parent { PARENT_HEADER - 4.0 } else { 22.0 }))
                    .px(px(8.0))
                    .pt(px(2.0))
                    .gap(px(6.0))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(if parent { 12.5 } else { 12.0 }))
                            .when(parent, |label| label.font_weight(FontWeight::MEDIUM))
                            .text_color(if view.more {
                                palette.fg_subtle
                            } else {
                                palette.fg
                            })
                            .child(view.label.clone()),
                    )
                    .when(show_head_size, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .font_family(mono_font.clone())
                                .text_size(px(11.0))
                                .text_color(head_size_color)
                                .child(head_size.clone()),
                        )
                    }),
            )
        })
        .when(big, |tile| {
            tile.child(
                div()
                    .absolute()
                    .left(px(8.0))
                    .bottom(px(5.0))
                    .right(px(6.0))
                    .truncate()
                    .font_family(mono_font.clone())
                    .text_size(px(big_font))
                    .line_height(px(big_font * 1.25))
                    .text_color(palette.fg)
                    .child(view.size_label.clone()),
            )
        });

    if interactive && let Some(on_click) = on_click {
        let data = view.clone();
        let secondary_data = view.clone();
        let secondary_click = on_click.clone();
        // Kids are drawn over their parent: occluding keeps a click on a kid from also
        // reaching the parent under it.
        tile = tile
            .occlude()
            .cursor_pointer()
            .hover(move |style| style.border_color(hover_line))
            .on_click(move |_, window, cx| on_click(key, &data, false, window, cx))
            .on_mouse_down(MouseButton::Right, move |_, window, cx| {
                cx.stop_propagation();
                secondary_click(key, &secondary_data, true, window, cx)
            });
    }
    tile.into_any_element()
}
