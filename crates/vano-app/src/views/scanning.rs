//! The scanning screen: the counters, the progress line, the live view (Treemap, Rings or
//! Ranking) and the recommendations found so far.

use std::collections::HashSet;

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use vano_core::treemap::Rect;
use vano_core::{NodeId, ROOT, Tree};
use vano_rules::Status;

use crate::state::format;
use crate::state::settings::LiveView;
use crate::state::treemap::{self, TileKey};
use crate::theme::metrics::{BREADCRUMB_HEIGHT, PAGE_PADDING, SIDE_PANEL_WIDTH};
use crate::theme::{Palette, PaletteExt};

use super::common::{Stat, card_header, panel, stats_row};
use super::components::{SecondaryButton, SegmentedControl, SegmentedItem};
use super::live_views::{RANK_ROWS, RankRow, ring_sums, set_ranking};
use super::root::{AppView, ColorBy, ScanSession};
use super::treemap_view::{TileView, apply_layout, render_treemap};

impl AppView {
    pub(crate) fn render_scanning(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.update_live_views();
        let reduce_motion = cx.reduce_motion();
        let running = self.scan.as_ref().is_some_and(|s| !s.finished);
        match self.live_view {
            LiveView::Treemap => AppView::tick(&mut self.live_treemap, window, reduce_motion),
            LiveView::Rings => {
                if self.ring.step(running && !reduce_motion) {
                    window.request_animation_frame();
                }
            }
            LiveView::Ranking => {
                if reduce_motion {
                    self.ranking.snap();
                } else if self.ranking.step(1.0 / 60.0) {
                    window.request_animation_frame();
                }
            }
        }

        let palette = cx.palette();

        let palette = &*palette;
        let mono = cx.theme().mono_font_family.clone();
        let Some(session) = self.scan.as_ref() else {
            return div().into_any_element();
        };
        let progress = &session.progress;
        let fraction = session.fraction();
        let elapsed = progress.elapsed.as_secs_f64();
        let stats = vec![
            Stat {
                label: t!("scan.files").into(),
                dot: None,
                value: format::count(progress.files).into(),
                sub: SharedString::default(),
                sub_color: palette.fg_subtle,
            },
            Stat {
                label: t!("scan.counted").into(),
                dot: None,
                value: format::size(progress.bytes).into(),
                sub: session
                    .estimate
                    .map(|e| t!("scan.of_estimate", size = format::size(e)).into_owned())
                    .unwrap_or_default()
                    .into(),
                sub_color: palette.fg_subtle,
            },
            Stat {
                label: t!("scan.elapsed").into(),
                dot: None,
                value: format::seconds(elapsed).into(),
                sub: SharedString::default(),
                sub_color: palette.fg_subtle,
            },
            Stat {
                label: t!("scan.throughput").into(),
                dot: None,
                value: format::throughput(progress.bytes, elapsed).into(),
                sub: SharedString::default(),
                sub_color: palette.fg_subtle,
            },
        ];

        // Done reading: the rules are being evaluated, a moment before the results open.
        let path_line = if session.finished {
            t!("scan.preparing").into_owned()
        } else {
            progress
                .current
                .as_ref()
                .map(|path| format::short_path(path, self.home.as_deref()))
                .unwrap_or_else(|| t!("scan.starting").into_owned())
        };
        let percent = fraction.map(|f| format!("{}%", (f * 100.0).floor() as u32));
        let bar_fraction = fraction.unwrap_or(0.0);

        let progress_row = h_flex()
            .h(px(36.0))
            .px(px(16.0))
            .gap(px(10.0))
            .items_center()
            .border_t_1()
            .border_color(palette.border)
            .child(Spinner::new().small().color(palette.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(mono.clone())
                    .text_size(px(12.0))
                    .text_color(palette.fg_muted)
                    .child(path_line),
            )
            .children(
                percent.map(|p| div().font_family(mono.clone()).text_size(px(12.0)).child(p)),
            );
        let bar = div()
            .h(px(2.0))
            .bg(palette.hover)
            .child(div().h_full().w(relative(bar_fraction)).bg(palette.accent));

        let live = self.render_live_card(session, palette, &mono, cx);
        let found = self.render_found(session, palette, &mono, cx);

        v_flex()
            .size_full()
            .p(px(PAGE_PADDING))
            .gap(px(PAGE_PADDING))
            .child(
                panel(palette)
                    .flex_none()
                    .child(stats_row(palette, &mono, stats))
                    .child(progress_row)
                    .child(bar),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(PAGE_PADDING))
                    .items_stretch()
                    .child(live)
                    .child(
                        panel(palette)
                            .flex_none()
                            .w(px(SIDE_PANEL_WIDTH))
                            .child(found),
                    ),
            )
            .into_any_element()
    }

    /// Lays out the live view again after each poll.
    fn update_live_views(&mut self) {
        let Some(session) = self.scan.as_ref() else {
            return;
        };
        let generation = session.generation;
        match self.live_view {
            LiveView::Treemap => {
                let Some(bounds) = self.live_treemap.bounds.get() else {
                    return;
                };
                let key = (bounds.size, ROOT, generation);
                if self.live_treemap.laid_out == Some(key) {
                    return;
                }
                let area = Rect::new(
                    0.0,
                    0.0,
                    bounds.size.width.into(),
                    bounds.size.height.into(),
                );
                let ctx = &self.rules_ctx;
                let current = session.progress.current.clone();
                let tiles = session.with_tree(|tree| {
                    let reading = current
                        .as_deref()
                        .and_then(|path| deepest_known(tree, path));
                    let chain: HashSet<NodeId> = reading
                        .map(|id| tree.ancestry(id).into_iter().collect())
                        .unwrap_or_default();
                    treemap::layout(tree, ROOT, area)
                        .into_iter()
                        .map(|tile| {
                            let more = matches!(tile.key, TileKey::More(_));
                            let view = TileView {
                                node: tile.node,
                                role: tile.role,
                                openable: false,
                                label: if more {
                                    format::counted("explorer.more", tile.grouped as u64).into()
                                } else {
                                    tree.display_name(tile.node).into()
                                },
                                size_label: format::size(tile.size).into(),
                                category: vano_rules::category_of(tree, ctx, tile.node),
                                status: if more {
                                    None
                                } else {
                                    found_status(session, tree, tile.node)
                                },
                                current: !session.finished
                                    && !more
                                    && tile.role != treemap::TileRole::Kid
                                    && chain.contains(&tile.node),
                                more,
                                reclaim: (0.0, 0.0),
                                reclaim_bytes: 0,
                            };
                            (tile, view)
                        })
                        .collect::<Vec<_>>()
                });
                apply_layout(&mut self.live_treemap, area, tiles, None);
                self.live_treemap.laid_out = Some(key);
            }
            LiveView::Rings => {
                let ctx = &self.rules_ctx;
                let sums = session.with_tree(|tree| {
                    ring_sums(tree.children(ROOT).flat_map(|child| {
                        // One level down, so a home folder's categories separate.
                        let node = tree.node(child);
                        if node.is_dir() && !tree.children(child).is_empty() {
                            tree.children(child)
                                .map(|kid| {
                                    (vano_rules::category_of(tree, ctx, kid), tree.node(kid).size)
                                })
                                .collect::<Vec<_>>()
                        } else {
                            vec![(vano_rules::category_of(tree, ctx, child), node.size)]
                        }
                    }))
                });
                let total = session.estimate.unwrap_or(session.progress.bytes);
                let total = if session.finished {
                    session.progress.bytes
                } else {
                    total
                };
                self.ring.set(sums, total);
            }
            LiveView::Ranking => {
                let ctx = &self.rules_ctx;
                let (rows, max) = session.with_tree(|tree| {
                    let top = tree.sorted_children(ROOT);
                    let max = top.first().map(|id| tree.node(*id).size).unwrap_or(1);
                    let rows = top
                        .into_iter()
                        .filter(|id| tree.node(*id).size > 0)
                        .take(RANK_ROWS)
                        .map(|id| {
                            let size = tree.node(id).size;
                            (
                                id,
                                size,
                                RankRow {
                                    name: tree.display_name(id).into(),
                                    size: format::size(size).into(),
                                    category: vano_rules::category_of(tree, ctx, id),
                                    openable: false,
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    (rows, max)
                });
                set_ranking(&mut self.ranking, rows, max);
            }
        }
    }

    fn set_live_view_now(&mut self, view: LiveView, cx: &mut Context<Self>) {
        self.live_view = view;
        self.live_treemap.laid_out = None;
        if let Some(session) = self.scan.as_mut() {
            session.generation += 1;
        }
        cx.notify();
    }

    fn render_live_card(
        &self,
        session: &ScanSession,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut segmented = SegmentedControl::new("live-view");
        for view in LiveView::ALL {
            let weak = cx.weak_entity();
            segmented = segmented.item(
                SegmentedItem::new(view.label())
                    .selected(self.live_view == view)
                    .on_click(move |_, cx| {
                        let _ = weak.update(cx, |this, cx| this.set_live_view_now(view, cx));
                    }),
            );
        }
        let counted = format::size(session.progress.bytes);
        let content = match self.live_view {
            LiveView::Treemap => div()
                .size_full()
                .px(px(14.0))
                .pb(px(14.0))
                .child(render_treemap(
                    "live-treemap",
                    cx.weak_entity(),
                    &self.live_treemap,
                    palette,
                    ColorBy::Category,
                    mono.clone(),
                    None,
                ))
                .into_any_element(),
            LiveView::Rings => {
                let sub = if session.finished {
                    format::counted("common.files", session.progress.files)
                } else {
                    session
                        .estimate
                        .map(|e| t!("scan.of_estimate", size = format::size(e)).into_owned())
                        .unwrap_or_default()
                };
                super::live_views::render_rings(
                    &self.ring,
                    palette,
                    mono,
                    counted.clone().into(),
                    sub.into(),
                    !session.finished,
                )
            }
            LiveView::Ranking => {
                super::live_views::render_ranking(&self.ranking, palette, mono, None)
            }
        };
        panel(palette)
            .flex_1()
            .min_w_0()
            .child(card_header(
                palette,
                t!("scan.live.title"),
                t!("scan.live.sub"),
                Some(segmented.into_any_element()),
            ))
            .child(
                h_flex()
                    .flex_none()
                    .h(px(BREADCRUMB_HEIGHT))
                    .px(px(16.0))
                    .items_center()
                    .justify_between()
                    .text_size(px(12.5))
                    .child(
                        div()
                            .text_color(palette.fg_muted)
                            .child(session.target.root_label()),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(12.0))
                            .text_color(palette.fg_muted)
                            .child(counted),
                    ),
            )
            .child(div().flex_1().min_h_0().child(content))
    }

    fn render_found(
        &self,
        session: &ScanSession,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let total: u64 = session
            .found
            .iter()
            .filter(|rec| !rec.info)
            .map(|rec| rec.size)
            .sum();
        let rows = session.found.iter().map(|rec| {
            let rule = rec.rule;
            h_flex()
                .h(px(56.0))
                .px(px(16.0))
                .gap(px(12.0))
                .items_center()
                .border_b_1()
                .border_color(palette.border)
                .child(
                    div()
                        .flex_none()
                        .size(px(6.0))
                        .rounded(px(3.0))
                        .bg(match rec.status {
                            Status::Safe => palette.success,
                            Status::Review => palette.warning,
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13.0))
                                .child(crate::state::format::tr(&format!("rules.{rule}.name"))),
                        )
                        .child(
                            div()
                                .truncate()
                                .font_family(mono.clone())
                                .text_size(px(11.5))
                                .text_color(palette.fg_subtle)
                                .child(rec.display_path.clone()),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(mono.clone())
                        .text_size(px(13.0))
                        .child(if rec.approx {
                            format::approx_size(rec.size)
                        } else {
                            format::size(rec.size)
                        }),
                )
        });
        let empty = session.found.is_empty().then(|| {
            div()
                .p(px(16.0))
                .text_size(px(12.5))
                .text_color(palette.fg_subtle)
                .child(t!("scan.found.empty"))
        });
        let button = SecondaryButton::new("cancel-scan", t!("scan.cancel"))
            .on_click(cx.listener(|view, _, _, cx| view.cancel_scan(cx)));
        v_flex()
            .size_full()
            .child(card_header(
                palette,
                t!("scan.found.title"),
                t!("scan.found.sub"),
                None,
            ))
            .child(
                div()
                    .id("found-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(v_flex().children(rows).children(empty)),
            )
            .child(
                h_flex()
                    .flex_none()
                    .px(px(16.0))
                    .py(px(12.0))
                    .gap(px(8.0))
                    .items_center()
                    .border_t_1()
                    .border_color(palette.border)
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(6.0))
                            .text_size(px(12.5))
                            .text_color(palette.fg_muted)
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_color(palette.fg)
                                    .child(format::size(total)),
                            )
                            .child(div().truncate().child(t!("scan.found.reclaimable"))),
                    )
                    .child(button),
            )
            .into_any_element()
    }
}

/// The deepest node of `path` already in the tree.
fn deepest_known(tree: &Tree, path: &std::path::Path) -> Option<NodeId> {
    let rest = path.strip_prefix(tree.root_path()).ok()?;
    let mut cur = ROOT;
    for part in rest.components() {
        let name = part.as_os_str();
        match tree
            .children(cur)
            .find(|child| tree.os_name(*child) == name)
        {
            Some(child) => cur = child,
            None => break,
        }
    }
    Some(cur)
}

/// The status of the found recommendation covering `id` or an ancestor of it.
fn found_status(session: &ScanSession, tree: &Tree, id: NodeId) -> Option<Status> {
    if session.found_status.is_empty() {
        return None;
    }
    let mut cur = id;
    loop {
        if let Some(status) = session.found_status.get(&cur) {
            return Some(*status);
        }
        if cur == ROOT {
            return None;
        }
        cur = tree.node(cur).parent;
    }
}
