//! The Move to Trash confirmation and the toast that follows it.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use vano_rules::Status;

use crate::state::format;
use crate::theme::PaletteExt;
use crate::theme::metrics::{CONFIRM_WIDTH, RADIUS_LG, RADIUS_MD};

use super::components::{
    DangerButton, ModalPlacement, SecondaryButton, framed_dialog, modal_body, modal_footer,
    modal_header,
};
use super::root::{AppView, TrashRequest};

/// One line of the confirmation's list.
#[derive(Clone)]
struct ConfirmItem {
    name: String,
    path: String,
    size: u64,
    dot: Option<Status>,
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
                items.push(ConfirmItem {
                    name: crate::state::format::tr(&format!("rules.{rule}.name")),
                    path: rec.display_path.clone(),
                    size: rec.size,
                    dot: Some(rec.status),
                });
            }
        }
        for node in &request.nodes {
            items.push(ConfirmItem {
                name: results.tree.display_name(*node),
                path: format::short_path(&results.tree.path(*node), home.as_deref()),
                size: results.tree.node(*node).size,
                dot: None,
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
                    let rows = items.iter().map(|item| {
                        h_flex()
                            .px(px(12.0))
                            .py(px(8.0))
                            .gap(px(10.0))
                            .items_center()
                            .border_b_1()
                            .border_color(palette.border)
                            .child(div().flex_none().size(px(6.0)).rounded(px(3.0)).bg(
                                match item.dot {
                                    Some(Status::Safe) => palette.success,
                                    Some(Status::Review) => palette.warning,
                                    None => palette.fg_subtle,
                                },
                            ))
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
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_family(mono.clone())
                                    .child(format::size(item.size)),
                            )
                    });
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
                                .child(
                                    v_flex()
                                        .rounded(px(RADIUS_LG))
                                        .border_1()
                                        .border_color(palette.border)
                                        .overflow_hidden()
                                        .children(rows)
                                        .child(
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
                                                ),
                                        ),
                                )
                                .children(warning.clone().map(|text| {
                                    h_flex()
                                        .gap(px(8.0))
                                        .px(px(12.0))
                                        .py(px(10.0))
                                        .rounded(px(RADIUS_MD))
                                        .bg(palette.warning_subtle)
                                        .text_color(palette.warning)
                                        .text_size(px(12.5))
                                        .child(Icon::new(Lucide::TriangleAlert).small())
                                        .child(div().flex_1().child(text))
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
                                            let _ =
                                                weak.update(cx, |view, cx| view.trash(request, cx));
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
                        .child(div().child(toast.text.clone()))
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
