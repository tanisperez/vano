//! The Move to Trash confirmation, and the toast or the report that follows it.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use vano_rules::Status;

use crate::state::format;
use crate::theme::PaletteExt;
use crate::theme::metrics::{CONFIRM_LIST_MAX_HEIGHT, CONFIRM_WIDTH, RADIUS_LG, RADIUS_MD};

use super::components::{
    DangerButton, ModalPlacement, PrimaryButton, SecondaryButton, framed_dialog, modal_body,
    modal_footer, modal_header,
};
use super::root::{AppView, TrashReport, TrashRequest};

/// One line of the confirmation's or the report's list.
#[derive(Clone)]
struct ListItem {
    name: String,
    path: String,
    size: u64,
    dot: Dot,
    /// Why the item is listed, under its path (the report's reasons).
    note: Option<String>,
}

/// The color of a line's dot.
#[derive(Clone, Copy)]
enum Dot {
    Status(Status),
    Plain,
    Failed,
}

impl AppView {
    /// Asks before moving `request` to the Trash.
    pub(crate) fn open_trash_confirm(
        &mut self,
        request: TrashRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        let home = self.home.clone();
        let mut items = Vec::new();
        let mut review = 0usize;
        for rule in &request.rules {
            if let Some(rec) = results.rec(rule) {
                if rec.status == Status::Review {
                    review += 1;
                }
                items.push(ListItem {
                    name: crate::state::format::tr(&format!("rules.{rule}.name")),
                    path: rec.display_path.clone(),
                    size: rec.size,
                    dot: Dot::Status(rec.status),
                    note: None,
                });
            }
        }
        for node in &request.nodes {
            items.push(ListItem {
                name: results.tree.display_name(*node),
                path: format::short_path(&results.tree.path(*node), home.as_deref()),
                size: results.tree.node(*node).size,
                dot: Dot::Plain,
                note: None,
            });
        }
        if items.is_empty() {
            return;
        }
        let total: u64 = items.iter().map(|item| item.size).sum();
        let title = if items.len() == 1 {
            t!("confirm.title_one", name = items[0].name.clone()).into_owned()
        } else {
            t!("confirm.title_many", count = items.len().to_string()).into_owned()
        };
        let warning = if !request.nodes.is_empty() {
            Some(t!("confirm.uncovered").into_owned())
        } else if review > 0 {
            Some(format::counted("confirm.review", review as u64))
        } else {
            None
        };
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, _cx| {
            let items = items.clone();
            let title = title.clone();
            let warning = warning.clone();
            let request = request.clone();
            let weak = weak.clone();
            let dialog = dialog.overlay_closable(true);
            framed_dialog(
                dialog,
                CONFIRM_WIDTH,
                None,
                ModalPlacement::Top {
                    fraction: 0.16,
                    min: 40.0,
                },
                window,
                move |_window, cx| {
                    let palette = cx.palette();
                    let mono = cx.theme().mono_font_family.clone();
                    let request = request.clone();
                    let weak = weak.clone();
                    v_flex()
                        .child(modal_header(title.clone(), cx))
                        .child(
                            modal_body()
                                .child(
                                    div()
                                        .text_color(palette.fg_muted)
                                        .line_height(px(19.0))
                                        .child(t!("confirm.text")),
                                )
                                .child(item_list(
                                    "confirm-items",
                                    &items,
                                    Some(
                                        h_flex()
                                            .px(px(12.0))
                                            .py(px(10.0))
                                            .justify_between()
                                            .bg(palette.surface)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(t!("confirm.total"))
                                            .child(
                                                div()
                                                    .font_family(mono.clone())
                                                    .child(format::size(total)),
                                            )
                                            .into_any_element(),
                                    ),
                                    cx,
                                ))
                                .children(warning.clone().map(|text| {
                                    h_flex()
                                        .items_start()
                                        .gap(px(8.0))
                                        .px(px(12.0))
                                        .py(px(10.0))
                                        .rounded(px(RADIUS_MD))
                                        .bg(palette.warning_subtle)
                                        .text_color(palette.warning)
                                        .text_size(px(12.5))
                                        .line_height(px(18.0))
                                        .child(
                                            div()
                                                .flex_none()
                                                .h(px(18.0))
                                                .flex()
                                                .items_center()
                                                .child(Icon::new(Lucide::TriangleAlert).small()),
                                        )
                                        .child(div().flex_1().min_w_0().child(text))
                                })),
                        )
                        .child(
                            modal_footer(cx)
                                .child(
                                    SecondaryButton::new("confirm-cancel", t!("common.cancel"))
                                        .on_click(|_, window, cx| window.close_dialog(cx)),
                                )
                                .child(
                                    DangerButton::new(
                                        "confirm-trash",
                                        t!("confirm.button", size = format::size(total)),
                                    )
                                    .on_click(
                                        move |_, window, cx| {
                                            let request = request.clone();
                                            let _ = weak.update(cx, |view, cx| {
                                                view.trash(request, window, cx);
                                            });
                                            window.close_dialog(cx);
                                        },
                                    ),
                                ),
                        )
                        .into_any_element()
                },
            )
        });
    }

    /// Explains how a batch ended when some items stayed in place or it was stopped.
    pub(crate) fn open_trash_report(
        &mut self,
        report: TrashReport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = if report.left.is_empty() {
            t!("report.title_stopped").into_owned()
        } else {
            t!("report.title_left").into_owned()
        };
        let mut summary = vec![
            t!(
                "report.moved",
                moved = format::count(report.moved as u64),
                total = format::count(report.total as u64),
                size = format::size(report.freed)
            )
            .into_owned(),
        ];
        let untouched = report.total - report.moved - report.left.len();
        if untouched > 0 {
            summary.push(format::counted("report.untouched", untouched as u64));
        }
        let summary = summary.join(" ");
        let items: Vec<ListItem> = report
            .left
            .iter()
            .map(|item| ListItem {
                name: item.name.clone(),
                path: item.path.clone(),
                size: item.size,
                dot: Dot::Failed,
                note: Some(item.reason.clone()),
            })
            .collect();
        let undo = report.undo;
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, _cx| {
            let title = title.clone();
            let summary = summary.clone();
            let items = items.clone();
            let undo = undo.clone();
            let weak = weak.clone();
            let dialog = dialog.overlay_closable(true);
            framed_dialog(
                dialog,
                CONFIRM_WIDTH,
                None,
                ModalPlacement::Top {
                    fraction: 0.16,
                    min: 40.0,
                },
                window,
                move |_window, cx| {
                    let palette = cx.palette();
                    let undo = undo.clone();
                    let weak = weak.clone();
                    v_flex()
                        .child(modal_header(title.clone(), cx))
                        .child(
                            modal_body()
                                .child(
                                    div()
                                        .text_color(palette.fg_muted)
                                        .line_height(px(19.0))
                                        .child(summary.clone()),
                                )
                                .when(!items.is_empty(), |body| {
                                    body.child(item_list("report-items", &items, None, cx))
                                })
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .line_height(px(18.0))
                                        .text_color(palette.fg_subtle)
                                        .child(t!("report.nothing_deleted")),
                                ),
                        )
                        .child(
                            modal_footer(cx)
                                .when_some(undo, |footer, batch| {
                                    footer.child(
                                        SecondaryButton::new("report-undo", t!("toast.undo"))
                                            .on_click(move |_, window, cx| {
                                                let batch = batch.clone();
                                                let _ = weak.update(cx, |view, cx| {
                                                    view.restore(batch, cx);
                                                });
                                                window.close_dialog(cx);
                                            }),
                                    )
                                })
                                .child(
                                    PrimaryButton::new("report-close", t!("common.close"))
                                        .on_click(|_, window, cx| window.close_dialog(cx)),
                                ),
                        )
                        .into_any_element()
                },
            )
        });
    }

    /// The toast, floating at the bottom center.
    pub(crate) fn render_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        let palette = cx.palette();
        let can_undo = toast.undo.is_some();
        let icon = if toast.error {
            Icon::new(Lucide::TriangleAlert).text_color(palette.danger)
        } else {
            Icon::new(Lucide::CircleCheck).text_color(palette.success)
        };
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(16.0))
                .flex()
                .justify_center()
                .child(
                    h_flex()
                        .occlude()
                        .max_w(px(640.0))
                        .px(px(14.0))
                        .py(px(9.0))
                        .gap(px(12.0))
                        .items_center()
                        .rounded(px(RADIUS_LG))
                        .border_1()
                        .border_color(palette.border)
                        .bg(palette.overlay)
                        .shadow(palette.shadow.clone())
                        .text_size(px(13.0))
                        .child(icon.small())
                        .child(div().min_w_0().child(toast.text.clone()))
                        .when(can_undo, |row| {
                            row.child(
                                div()
                                    .id("toast-undo")
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.accent_text)
                                    .cursor_pointer()
                                    .hover(|style| style.text_color(palette.fg))
                                    .child(t!("toast.undo"))
                                    .on_click(cx.listener(|view, _, _, cx| view.undo_trash(cx))),
                            )
                        })
                        .child(
                            div()
                                .id("toast-close")
                                .text_color(palette.fg_subtle)
                                .cursor_pointer()
                                .hover(|style| style.text_color(palette.fg))
                                .child(Icon::new(Lucide::X).xsmall())
                                .on_click(cx.listener(|view, _, _, cx| view.close_toast(cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// The bordered list of `items`, capped at about five rows and scrolled past that, with an
/// optional `footer` row that stays in view.
fn item_list(
    id: &'static str,
    items: &[ListItem],
    footer: Option<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let palette = cx.palette();
    let mono = cx.theme().mono_font_family.clone();
    let rows = items.iter().map(|item| {
        h_flex()
            .px(px(12.0))
            .py(px(8.0))
            .gap(px(10.0))
            .items_center()
            .border_b_1()
            .border_color(palette.border)
            .child(
                div()
                    .flex_none()
                    .size(px(6.0))
                    .rounded(px(3.0))
                    .bg(match item.dot {
                        Dot::Status(Status::Safe) => palette.success,
                        Dot::Status(Status::Review) => palette.warning,
                        Dot::Plain => palette.fg_subtle,
                        Dot::Failed => palette.danger,
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.0))
                    .child(div().truncate().child(item.name.clone()))
                    .child(
                        div()
                            .truncate()
                            .font_family(mono.clone())
                            .text_size(px(11.5))
                            .text_color(palette.fg_subtle)
                            .child(item.path.clone()),
                    )
                    .children(item.note.clone().map(|note| {
                        div()
                            .text_size(px(12.0))
                            .line_height(px(17.0))
                            .text_color(palette.danger)
                            .child(note)
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .font_family(mono.clone())
                    .child(format::size(item.size)),
            )
    });
    v_flex()
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        // The scroll area needs a definite height: the capped column gives it one, the layout
        // gpui-component's own max height test uses.
        .child(
            v_flex().max_h(px(CONFIRM_LIST_MAX_HEIGHT)).child(
                v_flex().flex_1().overflow_hidden().child(
                    div().flex_1().overflow_hidden().child(
                        v_flex()
                            .id(id)
                            .size_full()
                            .overflow_y_scrollbar()
                            .children(rows),
                    ),
                ),
            ),
        )
        .children(footer)
}
