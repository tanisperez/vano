//! The results screen: the stats strip, the explorer (a treemap you zoom into folder by folder)
//! and the recommendations panel with its detail view.

use std::rc::Rc;

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use vano_core::treemap::Rect;
use vano_core::{NodeId, ROOT};
use vano_rules::{Recommendation, Status};

use crate::state::format;
use crate::state::results::Results;
use crate::state::settings::LiveView;
use crate::state::target::TargetKind;
use crate::state::treemap::{self, TileKey};
use crate::theme::metrics::{
    BREADCRUMB_HEIGHT, PAGE_PADDING, RADIUS_MD, RADIUS_SM, RADIUS_XS, REC_ROW_HEIGHT,
    SIDE_PANEL_WIDTH,
};
use crate::theme::{Palette, PaletteExt, VanoColors};

use super::common::{Stat, card_header, eyebrow, legend_item, panel, stats_row};
use super::components::{
    PrimaryButton, SecondaryButton, SegmentedControl, SegmentedItem, StatusBadge,
};
use super::live_views::{
    RANK_ROWS, RankClick, RankRow, render_ranking, render_rings, ring_sums, set_ranking,
};
use super::root::{AppView, ColorBy, Detail, Transition, TrashRequest};
use super::treemap_view::{TileClick, TileView, apply_layout, render_treemap};

/// Entries listed under "Largest inside".
const LARGEST_INSIDE: usize = 5;

impl AppView {
    pub(crate) fn render_results(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let reduce_motion = cx.reduce_motion();
        match self.explorer.view {
            LiveView::Treemap => {
                self.update_explorer_layout();
                AppView::tick(&mut self.explorer.treemap, window, reduce_motion);
            }
            LiveView::Rings => {
                self.update_explorer_views();
                if self.ring.step(false) && !reduce_motion {
                    window.request_animation_frame();
                }
            }
            LiveView::Ranking => {
                self.update_explorer_views();
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
        let Some(results) = self.results.as_ref() else {
            return div().into_any_element();
        };
        let stats = self.results_stats(results, palette);
        let explorer = self.render_explorer(results, palette, &mono, cx);
        let side = match self.detail {
            Some(detail) => self.render_detail(results, detail, palette, &mono, cx),
            None => self.render_recommendations(results, palette, &mono, cx),
        };

        v_flex()
            .size_full()
            .p(px(PAGE_PADDING))
            .gap(px(PAGE_PADDING))
            .child(
                panel(palette)
                    .flex_none()
                    .child(stats_row(palette, &mono, stats)),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(PAGE_PADDING))
                    .items_stretch()
                    .child(explorer)
                    .child(
                        panel(palette)
                            .flex_none()
                            .w(px(SIDE_PANEL_WIDTH))
                            .child(side),
                    ),
            )
            .into_any_element()
    }

    fn results_stats(&self, results: &Results, palette: &Palette) -> Vec<Stat> {
        let selected = results.selected_size();
        let (safe, safe_count) = results.total(Status::Safe);
        let (review, review_count) = results.total(Status::Review);
        let scanned = results.tree.node(ROOT).size;
        let free = results.fs.map(|disk| disk.available);
        // What files take on the disk but the scan did not count: folders it could not read,
        // the folders' own blocks and metadata. The administrator's reserve is not in it.
        let not_scanned = results.fs.map(|disk| disk.used().saturating_sub(scanned));
        vec![
            Stat {
                label: t!("results.free_after").into(),
                dot: None,
                value: free
                    .map_or("-".to_string(), |f| format::size(f + selected))
                    .into(),
                sub: if selected > 0 {
                    format!("+{}", format::size(selected)).into()
                } else {
                    t!("results.nothing_selected").into()
                },
                sub_color: if selected > 0 {
                    palette.success
                } else {
                    palette.fg_subtle
                },
            },
            Stat {
                label: t!("results.safe").into(),
                dot: Some(palette.success),
                value: format::size(safe).into(),
                sub: format::counted("common.items", safe_count as u64).into(),
                sub_color: palette.fg_subtle,
            },
            Stat {
                label: t!("results.review").into(),
                dot: Some(palette.warning),
                value: format::size(review).into(),
                sub: format::counted("common.items", review_count as u64).into(),
                sub_color: palette.fg_subtle,
            },
            Stat {
                label: t!("results.not_scanned").into(),
                dot: None,
                value: not_scanned.map_or("-".to_string(), format::size).into(),
                sub: match results.target.kind {
                    TargetKind::Home => t!("results.not_scanned_home"),
                    TargetKind::Disk => t!("results.not_scanned_disk"),
                    TargetKind::Folder => t!("results.not_scanned_folder"),
                }
                .into(),
                sub_color: palette.fg_subtle,
            },
        ]
    }

    /// Lays the explorer out again when its area, focus or data changed.
    fn update_explorer_layout(&mut self) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        let Some(bounds) = self.explorer.treemap.bounds.get() else {
            return;
        };
        let key = (bounds.size, self.explorer.focus, self.explorer.generation);
        if self.explorer.treemap.laid_out == Some(key) {
            return;
        }
        let area = Rect::new(
            0.0,
            0.0,
            bounds.size.width.into(),
            bounds.size.height.into(),
        );
        let tiles = treemap::layout(&results.tree, self.explorer.focus, area)
            .into_iter()
            .map(|tile| {
                let view = tile_view(results, &tile);
                (tile, view)
            })
            .collect();
        let transition = self.explorer.treemap.pending.take();
        apply_layout(&mut self.explorer.treemap, area, tiles, transition);
        self.explorer.treemap.laid_out = Some(key);
    }

    /// Sets the targets of the rings or the ranking when the focus, the data or the view changed.
    fn update_explorer_views(&mut self) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        let focus = self.explorer.focus;
        let key = (focus, self.explorer.generation, self.explorer.view);
        if self.explorer.views_key == Some(key) {
            return;
        }
        self.explorer.views_key = Some(key);
        let tree = &results.tree;
        match self.explorer.view {
            LiveView::Treemap => {}
            LiveView::Rings => {
                let bytes = results.category_bytes(focus);
                let sums = ring_sums(vano_core::Category::ALL.into_iter().zip(bytes));
                self.ring.set(sums, tree.node(focus).size);
            }
            LiveView::Ranking => {
                let top = tree.sorted_children(focus);
                let max = top.first().map(|id| tree.node(*id).size).unwrap_or(1);
                let rows = top
                    .into_iter()
                    .filter(|id| tree.node(*id).size > 0)
                    .take(RANK_ROWS)
                    .map(|id| {
                        let node = tree.node(id);
                        let row = RankRow {
                            name: tree.display_name(id).into(),
                            size: format::size(node.size).into(),
                            category: results.dominant(id),
                            openable: node.is_dir() && !tree.children(id).is_empty(),
                        };
                        (id, node.size, row)
                    })
                    .collect();
                set_ranking(&mut self.ranking, rows, max);
            }
        }
    }

    /// Shows the explorer as a treemap, rings or a ranking.
    fn set_explorer_view(&mut self, view: LiveView, cx: &mut Context<Self>) {
        self.explorer.view = view;
        self.explorer.views_key = None;
        // The treemap lays out again from where its tiles were.
        self.explorer.treemap.laid_out = None;
        cx.notify();
    }

    /// Zooms into `node`, whose tile is `key`, growing it out of its tile.
    pub(crate) fn zoom_into(&mut self, node: NodeId, key: TileKey, cx: &mut Context<Self>) {
        let from = self.explorer.treemap.motion.rect_of(&key);
        self.explorer.treemap.pending = from.map(Transition::In);
        self.enter(node);
        log::debug!("explorer zoomed into node {node}");
        cx.notify();
    }

    /// Makes `node` the focus, keeping the forward history only when it is the next step of it.
    fn enter(&mut self, node: NodeId) {
        if self.explorer.forward.last() == Some(&node) {
            self.explorer.forward.pop();
        } else {
            self.explorer.forward.clear();
        }
        self.explorer.focus = node;
    }

    /// Goes back into the folder last zoomed out of (the mouse forward button).
    pub(crate) fn zoom_forward(&mut self, cx: &mut Context<Self>) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        let Some(&node) = self.explorer.forward.last() else {
            return;
        };
        // Moved to the Trash since, or no longer under the focus: the history is stale.
        if results.tree.is_removed(node) || results.tree.node(node).parent != self.explorer.focus {
            self.explorer.forward.clear();
            return;
        }
        self.zoom_into(node, TileKey::Node(node), cx);
    }

    /// Zooms out one level.
    pub(crate) fn zoom_out(&mut self, cx: &mut Context<Self>) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        let focus = self.explorer.focus;
        if focus == ROOT {
            return;
        }
        self.explorer.focus = results.tree.node(focus).parent;
        self.explorer.forward.push(focus);
        self.explorer.treemap.pending = Some(Transition::Out(focus));
        cx.notify();
    }

    /// Zooms out to `ancestor` of the current focus (a breadcrumb click).
    fn zoom_to(&mut self, ancestor: NodeId, cx: &mut Context<Self>) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        let chain = results.tree.ancestry(self.explorer.focus);
        let Some(index) = chain.iter().position(|id| *id == ancestor) else {
            return;
        };
        let Some(child) = chain.get(index + 1).copied() else {
            return;
        };
        // Forward goes back down the same way, one level at a time.
        self.explorer
            .forward
            .extend(chain[index + 1..].iter().rev().copied());
        self.explorer.focus = ancestor;
        self.explorer.treemap.pending = Some(Transition::Out(child));
        cx.notify();
    }

    fn on_tile(&mut self, key: TileKey, view: &TileView, secondary: bool, cx: &mut Context<Self>) {
        log::debug!("tile clicked: {key:?} (secondary: {secondary})");
        if secondary {
            self.detail = Some(Detail::Node(view.node));
            cx.notify();
            return;
        }
        match key {
            TileKey::More(folder) => {
                if folder != self.explorer.focus {
                    self.zoom_into(folder, TileKey::Node(folder), cx);
                }
            }
            TileKey::Node(node) => {
                if view.openable {
                    self.zoom_into(node, key, cx);
                } else {
                    self.detail = Some(Detail::Node(node));
                    cx.notify();
                }
            }
        }
    }

    fn render_explorer(
        &self,
        results: &Results,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let color_by = self.explorer.color_by;
        let view = self.explorer.view;
        let mut view_control = SegmentedControl::new("explorer-view");
        for option in LiveView::ALL {
            view_control = view_control.item(
                SegmentedItem::new(option.label())
                    .selected(view == option)
                    .on_click(
                        self.listener_fn(cx, move |this, cx| this.set_explorer_view(option, cx)),
                    ),
            );
        }
        let segmented = SegmentedControl::new("explorer-color")
            .item(
                SegmentedItem::new(t!("results.color.category"))
                    .selected(color_by == ColorBy::Category)
                    .on_click(self.listener_fn(cx, |view, cx| {
                        view.explorer.color_by = ColorBy::Category;
                        cx.notify();
                    })),
            )
            .item(
                SegmentedItem::new(t!("results.color.cleanup"))
                    .selected(color_by == ColorBy::Cleanup)
                    .on_click(self.listener_fn(cx, |view, cx| {
                        view.explorer.color_by = ColorBy::Cleanup;
                        cx.notify();
                    })),
            );
        let subtitle = if view == LiveView::Ranking {
            t!("results.explorer.sub_ranking")
        } else if view == LiveView::Rings {
            t!("results.explorer.sub_rings")
        } else {
            t!("results.explorer.sub_treemap")
        };

        let weak = cx.weak_entity();
        let on_click: TileClick = Rc::new(move |key, view, secondary, _window, cx| {
            let _ = weak.update(cx, |this, cx| this.on_tile(key, view, secondary, cx));
        });
        let content = match view {
            LiveView::Treemap => render_treemap(
                "explorer",
                cx.weak_entity(),
                &self.explorer.treemap,
                palette,
                color_by,
                mono.clone(),
                Some(on_click),
            ),
            LiveView::Rings => {
                let focus = self.explorer.focus;
                // For the whole disk, relate the total to what the disk reports as used.
                let sub = match results.fs {
                    Some(fs) if focus == ROOT && results.target.kind == TargetKind::Disk => {
                        t!("results.rings_of_used", size = format::size(fs.used())).into_owned()
                    }
                    _ => format::counted("common.files", u64::from(results.tree.node(focus).files)),
                };
                render_rings(
                    &self.ring,
                    palette,
                    mono,
                    format::size(results.tree.node(focus).size).into(),
                    sub.into(),
                    false,
                )
            }
            LiveView::Ranking => {
                let weak = cx.weak_entity();
                let on_row: RankClick = Rc::new(move |node, openable, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if openable {
                            this.explorer.treemap.pending = None;
                            this.enter(node);
                            cx.notify();
                        } else {
                            this.detail = Some(Detail::Node(node));
                            cx.notify();
                        }
                    });
                });
                div()
                    .size_full()
                    .pt(px(6.0))
                    .child(render_ranking(&self.ranking, palette, mono, Some(on_row)))
                    .into_any_element()
            }
        };
        let treemap = div()
            .flex_1()
            .min_h_0()
            .px(px(14.0))
            .pb(px(10.0))
            .child(content);

        panel(palette)
            .flex_1()
            .min_w_0()
            .child(card_header(
                palette,
                t!("results.explorer.title"),
                subtitle,
                Some(
                    h_flex()
                        .gap(px(8.0))
                        // The view selector stays at the right edge: the coloring one comes and
                        // goes on its left.
                        .when(view == LiveView::Treemap, |row| row.child(segmented))
                        .child(view_control)
                        .into_any_element(),
                ),
            ))
            .child(self.render_breadcrumb(results, palette, mono, cx))
            .child(treemap)
            .when(view == LiveView::Treemap, |card| {
                card.child(render_legend(palette, color_by))
            })
    }

    fn render_breadcrumb(
        &self,
        results: &Results,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let focus = self.explorer.focus;
        let chain = results.tree.ancestry(focus);
        let last = chain.len() - 1;
        let mut row = h_flex()
            .flex_1()
            .min_w_0()
            .gap(px(4.0))
            .items_center()
            .overflow_hidden();
        for (index, id) in chain.iter().enumerate() {
            let label = if *id == ROOT {
                results.target.root_label()
            } else {
                results.tree.display_name(*id)
            };
            if index > 0 {
                row = row.child(div().flex_none().text_color(palette.fg_subtle).child("/"));
            }
            let id = *id;
            let current = index == last;
            row = row.child(
                div()
                    .id(("crumb", index))
                    .flex_shrink(1.0)
                    .min_w(px(24.0))
                    .truncate()
                    .px(px(4.0))
                    .rounded(px(RADIUS_XS))
                    .text_color(if current {
                        palette.fg
                    } else {
                        palette.fg_muted
                    })
                    .when(current, |crumb| crumb.font_weight(FontWeight::MEDIUM))
                    .when(!current, |crumb| {
                        crumb
                            .cursor_pointer()
                            .hover(|style| style.bg(palette.hover).text_color(palette.fg))
                            .on_click(cx.listener(move |view, _, _, cx| view.zoom_to(id, cx)))
                    })
                    .child(label),
            );
        }
        let info_target = focus;
        h_flex()
            .flex_none()
            .h(px(BREADCRUMB_HEIGHT))
            .px(px(12.0))
            .gap(px(8.0))
            .items_center()
            .text_size(px(12.5))
            .when(focus != ROOT, |bar| {
                bar.child(
                    div()
                        .id("zoom-out")
                        .flex_none()
                        .size(px(22.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(RADIUS_SM))
                        .text_color(palette.fg_muted)
                        .cursor_pointer()
                        .hover(|style| style.bg(palette.hover))
                        .child(Icon::new(IconName::ArrowLeft).small())
                        .on_click(cx.listener(|view, _, _, cx| view.zoom_out(cx))),
                )
            })
            .child(row)
            .child(
                div()
                    .id("focus-info")
                    .flex_none()
                    .px(px(6.0))
                    .rounded(px(RADIUS_XS))
                    .font_family(mono.clone())
                    .text_size(px(12.0))
                    .text_color(palette.fg_muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(palette.hover))
                    .child(format::size(results.tree.node(focus).size))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.detail = Some(Detail::Node(info_target));
                        cx.notify();
                    })),
            )
    }

    /// A click handler that updates this view, for components that take `Fn(&mut Window, &mut
    /// App)`.
    fn listener_fn(
        &self,
        cx: &mut Context<Self>,
        f: impl Fn(&mut AppView, &mut Context<AppView>) + 'static,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let weak = cx.weak_entity();
        move |_, cx| {
            let _ = weak.update(cx, |view, cx| f(view, cx));
        }
    }

    fn render_recommendations(
        &self,
        results: &Results,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let safe: Vec<&Recommendation> = results
            .active()
            .filter(|rec| rec.status == Status::Safe)
            .collect();
        let review: Vec<&Recommendation> = results
            .active()
            .filter(|rec| rec.status == Status::Review)
            .collect();
        let selection = results.selection();
        let selected_size: u64 = selection.iter().map(|rec| rec.size).sum();
        let selected_count = selection.len();

        let mut list = v_flex().pb(px(8.0));
        for (status, recs) in [(Status::Safe, &safe), (Status::Review, &review)] {
            let total: u64 = recs
                .iter()
                .filter(|rec| !rec.info)
                .map(|rec| rec.size)
                .sum();
            let (label, fg, bg) = match status {
                Status::Safe => (
                    t!("results.safe_group"),
                    palette.success,
                    palette.success_subtle,
                ),
                Status::Review => (
                    t!("results.review_group"),
                    palette.warning,
                    palette.warning_subtle,
                ),
            };
            let (ticked, count) = results.group_selection(status);
            let check = match ticked {
                0 => Check::Off,
                ticked if ticked == count => Check::On,
                _ => Check::Mixed,
            };
            list = list.child(
                h_flex()
                    .h(px(44.0))
                    .px(px(16.0))
                    .gap(px(12.0))
                    .items_center()
                    .border_b_1()
                    .border_color(palette.border)
                    .when(count > 0, |header| {
                        header.child(checkbox(
                            SharedString::from(format!("check-group-{status:?}")),
                            check,
                            palette,
                            cx.listener(move |view, _, _, cx| {
                                if let Some(results) = view.results.as_mut() {
                                    results.toggle_group(status);
                                }
                                cx.notify();
                            }),
                        ))
                    })
                    .child(StatusBadge::new(label).colors(fg, fg, bg))
                    .child(div().flex_1())
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(12.0))
                            .text_color(palette.fg_muted)
                            .child(format::size(total)),
                    ),
            );
            if recs.is_empty() {
                list = list.child(
                    div()
                        .px(px(16.0))
                        .py(px(14.0))
                        .text_size(px(12.5))
                        .text_color(palette.fg_subtle)
                        .border_b_1()
                        .border_color(palette.border)
                        .child(t!("results.group_empty")),
                );
            }
            for rec in recs.iter() {
                let on = results.selected.contains(rec.rule);
                list = list.child(self.render_rec_row(rec, on, palette, mono, cx));
            }
        }

        let can_trash = selected_count > 0 && !self.trashing;
        let selection_rules: Vec<&'static str> = selection.iter().map(|rec| rec.rule).collect();
        let weak = cx.weak_entity();
        v_flex()
            .size_full()
            .child(card_header(
                palette,
                t!("results.recs.title"),
                t!("results.recs.sub"),
                None,
            ))
            .child(
                div()
                    .id("recs-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(list),
            )
            .child(
                v_flex()
                    .flex_none()
                    .px(px(16.0))
                    .py(px(12.0))
                    .gap(px(10.0))
                    .border_t_1()
                    .border_color(palette.border)
                    .child(
                        h_flex()
                            .gap(px(6.0))
                            .text_size(px(12.5))
                            .text_color(palette.fg_muted)
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_color(palette.fg)
                                    .child(format::size(selected_size)),
                            )
                            .child(format!(
                                "{} \u{b7} {}",
                                t!("results.selected"),
                                format::counted("common.items", selected_count as u64)
                            )),
                    )
                    .child(
                        h_flex().justify_end().child(
                            PrimaryButton::new("trash-selected", t!("results.move_to_trash"))
                                .icon(Icon::new(IconName::Delete))
                                .disabled(!can_trash)
                                .on_click(move |_, window, cx| {
                                    let request = TrashRequest {
                                        rules: selection_rules.clone(),
                                        nodes: Vec::new(),
                                    };
                                    let _ = weak.update(cx, |view, cx| {
                                        view.open_trash_confirm(request, window, cx)
                                    });
                                }),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn render_rec_row(
        &self,
        rec: &Recommendation,
        on: bool,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rule = rec.rule;
        let size = if rec.approx {
            format::approx_size(rec.size)
        } else {
            format::size(rec.size)
        };
        h_flex()
            .id(SharedString::from(format!("rec-{rule}")))
            .h(px(REC_ROW_HEIGHT))
            .pl(px(16.0))
            .pr(px(10.0))
            .gap(px(12.0))
            .items_center()
            .border_b_1()
            .border_color(palette.border)
            .cursor_pointer()
            .hover(|style| style.bg(palette.hover))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.detail = Some(Detail::Rec(rule));
                cx.notify();
            }))
            .map(|row| {
                if rec.info {
                    // Never selected: an info mark in the checkbox's place.
                    row.child(
                        div()
                            .flex_none()
                            .size(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(palette.fg_subtle)
                            .child(Icon::new(gpui_kit::assets::IconName::Info).small()),
                    )
                } else {
                    row.child(checkbox(
                        SharedString::from(format!("check-{rule}")),
                        if on { Check::On } else { Check::Off },
                        palette,
                        cx.listener(move |view, _, _, cx| {
                            if let Some(results) = view.results.as_mut() {
                                results.toggle(rule);
                            }
                            cx.notify();
                        }),
                    ))
                }
            })
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
                    .child(size),
            )
            .child(
                Icon::new(IconName::ChevronRight)
                    .small()
                    .text_color(palette.fg_subtle),
            )
            .into_any_element()
    }

    fn render_detail(
        &self,
        results: &Results,
        detail: Detail,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = match detail {
            Detail::Rec(rule) => results
                .rec(rule)
                .map(|rec| self.rec_detail(results, rec, palette, mono, cx)),
            Detail::Node(node) => Some(self.node_detail(results, node, palette, mono, cx)),
        };
        let back = h_flex()
            .flex_none()
            .h(px(46.0))
            .px(px(10.0))
            .items_center()
            .border_b_1()
            .border_color(palette.border)
            .child(
                h_flex()
                    .id("detail-back")
                    .h(px(26.0))
                    .pl(px(4.0))
                    .pr(px(8.0))
                    .gap(px(4.0))
                    .items_center()
                    .rounded(px(RADIUS_SM))
                    .text_color(palette.fg_muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(palette.hover))
                    .child(Icon::new(IconName::ChevronLeft).small())
                    .child(t!("results.recs.title"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.detail = None;
                        cx.notify();
                    })),
            );
        v_flex()
            .size_full()
            .child(back)
            .children(content)
            .into_any_element()
    }

    fn rec_detail(
        &self,
        results: &Results,
        rec: &Recommendation,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let rule = rec.rule;
        let (badge, color, bg) = match rec.status {
            Status::Safe => (
                t!("results.safe_group"),
                palette.success,
                palette.success_subtle,
            ),
            Status::Review => (
                t!("results.review_group"),
                palette.warning,
                palette.warning_subtle,
            ),
        };
        let reason = (rec.status == Status::Review)
            .then(|| crate::state::format::tr(&format!("rules.{rule}.reason")));
        let inside = largest_inside_rec(results, rec);
        let on = results.selected.contains(rule);
        let first = rec.nodes.first().map(|node| results.tree.path(*node));
        let size = if rec.approx {
            format::approx_size(rec.size)
        } else {
            format::size(rec.size)
        };
        let meta = vec![
            (
                t!("results.meta.files").into_owned(),
                format::count(rec.files),
            ),
            (
                t!("results.meta.last_used").into_owned(),
                format::age(u64::from(rec.newest_mtime), format::now()),
            ),
        ];
        let body = detail_body(
            palette,
            mono,
            crate::state::format::tr(&format!("rules.{rule}.name")),
            rec.display_path.clone(),
            size,
            StatusBadge::new(badge)
                .colors(color, color, bg)
                .into_any_element(),
            crate::state::format::tr(&format!("rules.{rule}.desc")),
            reason,
            inside,
            color,
            Vec::new(),
            meta,
        );
        let weak = cx.weak_entity();
        let footer = detail_footer(palette)
            .when_some(first, |footer, path| {
                footer.child(
                    SecondaryButton::new("detail-reveal", t!("results.show_in_folder"))
                        .on_click(move |_, _, cx| cx.reveal_path(&path)),
                )
            })
            // Shown for information only: nothing to select or move to the Trash.
            .when(!rec.info, |footer| {
                footer
                    .child(
                        SecondaryButton::new(
                            "detail-select",
                            if on {
                                t!("results.deselect")
                            } else {
                                t!("results.select")
                            },
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if let Some(results) = view.results.as_mut() {
                                results.toggle(rule);
                            }
                            cx.notify();
                        })),
                    )
                    .child(div().flex_1())
                    .child(
                        PrimaryButton::new("detail-trash", t!("results.move_to_trash"))
                            .icon(Icon::new(IconName::Delete))
                            .disabled(self.trashing)
                            .on_click(move |_, window, cx| {
                                let request = TrashRequest {
                                    rules: vec![rule],
                                    nodes: Vec::new(),
                                };
                                let _ = weak.update(cx, |view, cx| {
                                    view.open_trash_confirm(request, window, cx)
                                });
                            }),
                    )
            });
        v_flex().flex_1().min_h_0().child(body).child(footer)
    }

    fn node_detail(
        &self,
        results: &Results,
        node: NodeId,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let tree = &results.tree;
        let category = results.category(node);
        let colors = palette.category(category);
        let badge = StatusBadge::new(crate::state::format::tr(&format!(
            "category.{}",
            category.id()
        )))
        .colors(colors.dot, palette.fg, colors.fill);
        let links: Vec<(&'static str, Status, u64)> = results
            .recs_inside(node)
            .into_iter()
            .map(|(rec, size)| (rec.rule, rec.status, size))
            .collect();
        let covering = results
            .covering(node)
            .filter(|rec| !rec.nodes.contains(&node));
        let mut links_all = links.clone();
        if let Some(rec) = covering
            && !links_all.iter().any(|(rule, _, _)| *rule == rec.rule)
        {
            links_all.push((rec.rule, rec.status, tree.node(node).size));
        }
        let desc = if links_all.is_empty() {
            t!("results.node.uncovered")
        } else {
            t!("results.node.covered")
        };
        let inside: Vec<(String, u64)> = tree
            .sorted_children(node)
            .into_iter()
            .take(LARGEST_INSIDE)
            .map(|child| (tree.display_name(child), tree.node(child).size))
            .collect();
        let path = tree.path(node);
        let shown_path = format::short_path(&path, self.home.as_deref());
        let data = tree.node(node);
        let meta = vec![
            (
                t!("results.meta.files").into_owned(),
                format::count(u64::from(data.files)),
            ),
            (
                t!("results.meta.category").into_owned(),
                crate::state::format::tr(&format!("category.{}", category.id())),
            ),
            (
                t!("results.meta.modified").into_owned(),
                format::age(u64::from(data.mtime), format::now()),
            ),
        ];
        let name = if node == ROOT {
            results.target.label()
        } else {
            tree.display_name(node)
        };
        let weak_links = cx.weak_entity();
        let link_rows: Vec<AnyElement> = links_all
            .iter()
            .map(|(rule, status, size)| {
                let rule = *rule;
                let weak = weak_links.clone();
                h_flex()
                    .id(SharedString::from(format!("link-{rule}")))
                    .gap(px(10.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .items_center()
                    .border_1()
                    .border_color(palette.border)
                    .rounded(px(RADIUS_MD))
                    .cursor_pointer()
                    .hover(|style| style.bg(palette.hover))
                    .child(div().size(px(6.0)).rounded(px(3.0)).bg(match status {
                        Status::Safe => palette.success,
                        Status::Review => palette.warning,
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(crate::state::format::tr(&format!("rules.{rule}.name"))),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(12.0))
                            .text_color(palette.fg_muted)
                            .child(format::size(*size)),
                    )
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.detail = Some(Detail::Rec(rule));
                            cx.notify();
                        });
                    })
                    .into_any_element()
            })
            .collect();
        let body = detail_body(
            palette,
            mono,
            name,
            shown_path,
            format::size(data.size),
            badge.into_any_element(),
            desc.into_owned(),
            None,
            inside,
            colors.dot,
            link_rows,
            meta,
        );
        let openable =
            data.is_dir() && node != self.explorer.focus && !tree.children(node).is_empty();
        let in_info_rec = results.covering(node).is_some_and(|rec| rec.info);
        let weak = cx.weak_entity();
        let footer = detail_footer(palette)
            .child(
                SecondaryButton::new("detail-reveal", t!("results.show_in_folder"))
                    .on_click(move |_, _, cx| cx.reveal_path(&path)),
            )
            .when(openable, |footer| {
                footer.child(
                    SecondaryButton::new("detail-open", t!("results.open")).on_click(cx.listener(
                        move |view, _, _, cx| {
                            view.zoom_into(node, TileKey::Node(node), cx);
                        },
                    )),
                )
            })
            .child(div().flex_1())
            // Not for a WSL virtual disk: moving it would delete the whole distribution.
            .when(node != ROOT && !in_info_rec, |footer| {
                footer.child(
                    PrimaryButton::new("detail-trash", t!("results.move_to_trash"))
                        .icon(Icon::new(IconName::Delete))
                        .disabled(self.trashing)
                        .on_click(move |_, window, cx| {
                            let request = TrashRequest {
                                rules: Vec::new(),
                                nodes: vec![node],
                            };
                            let _ = weak.update(cx, |view, cx| {
                                view.open_trash_confirm(request, window, cx)
                            });
                        }),
                )
            });
        v_flex().flex_1().min_h_0().child(body).child(footer)
    }
}

/// The view data of one tile.
fn tile_view(results: &Results, tile: &treemap::Tile) -> TileView {
    let tree = &results.tree;
    let more = matches!(tile.key, TileKey::More(_));
    let (safe, review) = if more {
        (0, 0)
    } else {
        results.reclaimable(tile.node)
    };
    TileView {
        node: tile.node,
        role: tile.role,
        openable: tile.openable,
        label: if more {
            format::counted("explorer.more", tile.grouped as u64).into()
        } else {
            tree.display_name(tile.node).into()
        },
        size_label: format::size(tile.size).into(),
        category: results.dominant(tile.node),
        status: if more {
            None
        } else {
            results.status_of(tile.node)
        },
        current: false,
        more,
        reclaim: if more || tile.size == 0 {
            (0.0, 0.0)
        } else {
            (
                safe as f32 / tile.size as f32,
                review as f32 / tile.size as f32,
            )
        },
        reclaim_bytes: safe + review,
    }
}

/// The biggest entries a recommendation covers: its nodes when it has several, else the
/// children of its one node.
fn largest_inside_rec(results: &Results, rec: &Recommendation) -> Vec<(String, u64)> {
    let tree = &results.tree;
    let home = dirs::home_dir();
    if rec.nodes.len() > 1 {
        return rec
            .nodes
            .iter()
            .filter(|node| !tree.is_removed(**node))
            .take(LARGEST_INSIDE)
            .map(|node| {
                let path = tree.path(*node);
                let short = format::short_path(&path, home.as_deref());
                (short, tree.node(*node).size)
            })
            .collect();
    }
    let Some(node) = rec.nodes.first() else {
        return Vec::new();
    };
    tree.sorted_children(*node)
        .into_iter()
        .take(LARGEST_INSIDE)
        .map(|child| (tree.display_name(child), tree.node(child).size))
        .collect()
}

/// The scrolled body of a detail view.
#[allow(clippy::too_many_arguments)]
fn detail_body(
    palette: &Palette,
    mono: &SharedString,
    name: String,
    path: String,
    size: String,
    badge: AnyElement,
    desc: String,
    reason: Option<String>,
    inside: Vec<(String, u64)>,
    bar_color: Hsla,
    links: Vec<AnyElement>,
    meta: Vec<(String, String)>,
) -> AnyElement {
    let max = inside
        .iter()
        .map(|(_, size)| *size)
        .max()
        .unwrap_or(1)
        .max(1);
    div()
        .id("detail-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .child(
            v_flex()
                .p(px(16.0))
                .gap(px(16.0))
                .child(
                    v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name),
                        )
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_size(px(12.0))
                                .text_color(palette.fg_subtle)
                                .child(path),
                        ),
                )
                .child(
                    h_flex()
                        .gap(px(12.0))
                        .items_center()
                        .flex_wrap()
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_size(px(28.0))
                                .font_weight(FontWeight::MEDIUM)
                                .child(size),
                        )
                        .child(badge),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(px(19.0))
                        .text_color(palette.fg_muted)
                        .child(desc),
                )
                .children(reason.map(|reason| {
                    h_flex()
                        .gap(px(8.0))
                        .items_start()
                        .px(px(12.0))
                        .py(px(9.0))
                        .rounded(px(RADIUS_MD))
                        .bg(palette.warning_subtle)
                        .text_color(palette.warning)
                        .text_size(px(12.5))
                        .line_height(px(17.5))
                        .child(
                            div()
                                .flex_none()
                                .pt(px(1.0))
                                .child(Icon::new(IconName::TriangleAlert).small()),
                        )
                        .child(div().flex_1().child(reason))
                }))
                .when(!inside.is_empty(), |body| {
                    body.child(
                        v_flex()
                            .gap(px(8.0))
                            .child(eyebrow(palette, t!("results.largest_inside")))
                            .children(inside.into_iter().map(|(name, size)| {
                                let fraction = size as f32 / max as f32;
                                v_flex()
                                    .gap(px(4.0))
                                    .child(
                                        h_flex()
                                            .gap(px(10.0))
                                            .justify_between()
                                            .text_size(px(12.0))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .font_family(mono.clone())
                                                    .text_color(palette.fg_muted)
                                                    .child(name),
                                            )
                                            .child(
                                                div()
                                                    .flex_none()
                                                    .font_family(mono.clone())
                                                    .child(format::size(size)),
                                            ),
                                    )
                                    .child(
                                        div().h(px(4.0)).rounded(px(2.0)).bg(palette.hover).child(
                                            div()
                                                .h_full()
                                                .w(relative(fraction))
                                                .rounded(px(2.0))
                                                .bg(bar_color),
                                        ),
                                    )
                            })),
                    )
                })
                .when(!links.is_empty(), |body| {
                    body.child(
                        v_flex()
                            .gap(px(6.0))
                            .child(eyebrow(palette, t!("results.covered_by")))
                            .children(links),
                    )
                })
                .child(
                    v_flex()
                        .gap(px(6.0))
                        .children(meta.into_iter().map(|(k, v)| {
                            h_flex()
                                .gap(px(16.0))
                                .text_size(px(12.5))
                                .child(div().w(px(80.0)).text_color(palette.fg_subtle).child(k))
                                .child(div().font_family(mono.clone()).text_size(px(12.0)).child(v))
                        })),
                ),
        )
        .into_any_element()
}

fn detail_footer(palette: &Palette) -> Div {
    h_flex()
        .flex_none()
        .px(px(16.0))
        .py(px(12.0))
        .gap(px(8.0))
        .flex_wrap()
        .border_t_1()
        .border_color(palette.border)
}

/// What a checkbox shows. `Mixed` is a group with only part of it ticked.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    Off,
    On,
    Mixed,
}

fn checkbox(
    id: SharedString,
    check: Check,
    palette: &Palette,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on = check != Check::Off;
    div()
        .id(id)
        .flex_none()
        .size(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS_XS))
        .border_1()
        .border_color(if on {
            palette.accent
        } else {
            palette.border_strong
        })
        .bg(if on { palette.accent } else { palette.raised })
        .text_color(palette.accent_fg)
        .when(check == Check::On, |check| {
            check.child(Icon::new(IconName::Check).xsmall())
        })
        // No minus icon in the set: a short bar, like the platforms' mixed checkbox.
        .when(check == Check::Mixed, |check| {
            check.child(
                div()
                    .w(px(8.0))
                    .h(px(2.0))
                    .rounded(px(1.0))
                    .bg(palette.accent_fg),
            )
        })
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
}

/// The legend under the explorer.
fn render_legend(palette: &Palette, color_by: ColorBy) -> Div {
    let safe = legend_item(
        palette,
        palette.safe_fill(),
        palette.safe_line(),
        Some(palette.safe_hatch()),
        t!("results.legend.safe"),
    );
    let items: Vec<Div> = match color_by {
        ColorBy::Category => vano_core::Category::ALL
            .into_iter()
            .map(|category| {
                let colors = palette.category(category);
                legend_item(
                    palette,
                    colors.dot,
                    colors.dot,
                    None,
                    crate::state::format::tr(&format!("category.{}", category.id())),
                )
            })
            .chain(std::iter::once(safe))
            .collect(),
        ColorBy::Cleanup => vec![
            safe,
            legend_item(
                palette,
                palette.review_fill(),
                palette.review_line(),
                None,
                t!("results.legend.review"),
            ),
            legend_item(
                palette,
                palette.surface,
                palette.border_strong,
                None,
                t!("results.legend.none"),
            ),
        ],
    };
    h_flex()
        .flex_none()
        .flex_wrap()
        .px(px(16.0))
        .py(px(10.0))
        .gap_x(px(16.0))
        .gap_y(px(6.0))
        .border_t_1()
        .border_color(palette.border)
        .children(items)
}
