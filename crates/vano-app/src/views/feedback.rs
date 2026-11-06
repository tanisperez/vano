//! The confirmation that asks whether to move items to the Trash or delete them for good, with the
//! batch's progress, and the toast or the report that follows it.

use std::sync::atomic::Ordering;

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use telaio_ui::Tone;
use vano_rules::Status;

use crate::state::format;
use crate::state::trash::{DeleteMode, SLOW_TRASH_FILES, TrashProgress, trash_key};
use crate::theme::metrics::{
    CONFIRM_LIST_MAX_HEIGHT, CONFIRM_WIDTH, MODAL_HEADER_HEIGHT, RADIUS_LG, RADIUS_MD,
};
use crate::theme::{Palette, PaletteExt};

use super::components::{
    DangerButton, IconButton, InlineMessage, InlineMessageKind, ModalPlacement, PrimaryButton,
    ProgressBar, SecondaryButton, framed_dialog, modal_body, modal_footer, modal_header,
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

/// What a confirmation is about, read again on every render: a batch that stops leaves less.
struct ConfirmContents {
    items: Vec<ListItem>,
    size: u64,
    files: u64,
    warning: Option<String>,
}

impl AppView {
    /// Asks what to do with `request`, and runs the batch inside the dialog.
    pub(crate) fn open_trash_confirm(
        &mut self,
        request: TrashRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirm_contents(&request).items.is_empty() {
            return;
        }
        self.batch_note = None;
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            let request = request.clone();
            let weak = weak.clone();
            let running = weak
                .upgrade()
                .is_some_and(|view| view.read(cx).trash_progress.is_some());
            // Nothing closes the dialog during a batch: Stop does, or the batch's end.
            let dialog = dialog.overlay_closable(!running).keyboard(!running);
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
                    let Some(view) = weak.upgrade() else {
                        return div().into_any_element();
                    };
                    view.read(cx).render_confirm(&request, weak.clone(), cx)
                },
            )
        });
    }

    fn confirm_contents(&self, request: &TrashRequest) -> ConfirmContents {
        let mut contents = ConfirmContents {
            items: Vec::new(),
            size: 0,
            files: 0,
            warning: None,
        };
        let Some(results) = self.results.as_ref() else {
            return contents;
        };
        let mut review = 0usize;
        for rule in &request.rules {
            if let Some(rec) = results
                .rec(rule)
                .filter(|_| !results.removed.contains(rule))
            {
                if rec.status == Status::Review {
                    review += 1;
                }
                contents.files += rec.files;
                contents.items.push(ListItem {
                    name: format::tr(&format!("rules.{rule}.name")),
                    path: rec.display_path.clone(),
                    size: rec.size,
                    dot: Dot::Status(rec.status),
                    note: None,
                });
            }
        }
        for node in &request.nodes {
            if results.tree.is_removed(*node) {
                continue;
            }
            let data = results.tree.node(*node);
            contents.files += u64::from(data.files);
            contents.items.push(ListItem {
                name: results.tree.display_name(*node),
                path: format::short_path(&results.tree.path(*node), self.home.as_deref()),
                size: data.size,
                dot: Dot::Plain,
                note: None,
            });
        }
        contents.size = contents.items.iter().map(|item| item.size).sum();
        contents.warning = if !request.nodes.is_empty() {
            Some(t!("confirm.uncovered").into_owned())
        } else if review > 0 {
            Some(format::counted("confirm.review", review as u64))
        } else {
            None
        };
        contents
    }

    fn render_confirm(
        &self,
        request: &TrashRequest,
        weak: WeakEntity<Self>,
        cx: &App,
    ) -> AnyElement {
        let palette = cx.palette();
        let palette = &*palette;
        let mono = cx.theme().mono_font_family.clone();
        let contents = self.confirm_contents(request);
        let progress = self.trash_progress.clone();
        let running = progress.is_some();
        let mode = self.delete_mode;
        let permanent = mode == DeleteMode::Permanent;
        let size = format::size(contents.size);

        let title = match (permanent, contents.items.as_slice()) {
            (false, [item]) => t!(trash_key!("confirm.title_one"), name = item.name.clone()),
            (false, items) => t!(
                trash_key!("confirm.title_many"),
                count = items.len().to_string()
            ),
            (true, [item]) => t!("confirm.perm_title_one", name = item.name.clone()),
            (true, items) => t!("confirm.perm_title_many", count = items.len().to_string()),
        }
        .into_owned();
        let header = confirm_header(title, !running, palette);

        let body = modal_body()
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(delete_option(
                        "confirm-mode-trash",
                        DeleteMode::Trash,
                        mode,
                        running,
                        t!(trash_key!("confirm.trash_title")).into(),
                        t!(trash_key!("confirm.trash_desc")).into(),
                        None,
                        weak.clone(),
                        palette,
                    ))
                    .child(delete_option(
                        "confirm-mode-permanent",
                        DeleteMode::Permanent,
                        mode,
                        running,
                        t!("confirm.perm_title").into(),
                        t!(trash_key!("confirm.perm_desc")).into(),
                        Some(t!("confirm.faster").into()),
                        weak.clone(),
                        palette,
                    )),
            )
            .when(!permanent && contents.files > SLOW_TRASH_FILES, |body| {
                body.child(InlineMessage::new(
                    InlineMessageKind::Info,
                    t!(trash_key!("confirm.slow_trash")),
                ))
            })
            .when(permanent && !running, |body| {
                body.child(InlineMessage::new(
                    InlineMessageKind::Danger,
                    t!("confirm.perm_warning"),
                ))
            })
            .children(
                progress
                    .as_deref()
                    .map(|progress| batch_progress(progress, palette, &mono)),
            )
            .children(
                self.batch_note
                    .clone()
                    .filter(|_| !running)
                    .map(|note| InlineMessage::new(InlineMessageKind::Warning, note)),
            )
            .child(item_list(
                "confirm-items",
                &contents.items,
                Some(
                    h_flex()
                        .px(px(12.0))
                        .py(px(10.0))
                        .gap(px(12.0))
                        .justify_between()
                        .bg(palette.surface)
                        .font_weight(FontWeight::MEDIUM)
                        .child(t!("confirm.total"))
                        .child(div().font_family(mono.clone()).child(format!(
                            "{size} \u{b7} ~{}",
                            format::counted("common.files", contents.files)
                        )))
                        .into_any_element(),
                ),
                false,
                cx,
            ))
            .children(
                contents
                    .warning
                    .map(|text| InlineMessage::new(InlineMessageKind::Warning, text)),
            );

        let stop = weak.clone();
        let stopping = progress
            .as_deref()
            .is_some_and(|progress| progress.cancel.load(Ordering::Relaxed));
        let leading = if running {
            SecondaryButton::new("confirm-stop", t!("confirm.stop"))
                .disabled(stopping)
                .on_click(move |_, _, cx| {
                    let _ = stop.update(cx, |view, cx| view.cancel_trash(cx));
                })
        } else {
            SecondaryButton::new("confirm-cancel", t!("common.cancel"))
                .on_click(|_, window, cx| window.close_dialog(cx))
        };
        let label = match (permanent, running) {
            (false, false) => t!(trash_key!("confirm.button"), size = size),
            (false, true) => t!("confirm.moving"),
            (true, false) => t!("confirm.perm_button", size = size),
            (true, true) => t!("confirm.deleting"),
        };
        let request = request.clone();
        let go = DangerButton::new("confirm-go", label)
            .disabled(running)
            .on_click(move |_, window, cx| {
                let request = request.clone();
                let started = weak
                    .update(cx, |view, cx| view.trash(request, window, cx))
                    .unwrap_or(false);
                if !started {
                    window.close_dialog(cx);
                }
            });

        // `min_h_0` down to the body lets it scroll on a short window, where the dialog stops at
        // the window's height, so the buttons stay in view.
        v_flex()
            .flex_1()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .id("confirm-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(body),
            )
            .child(modal_footer(cx).child(leading).child(go))
            .into_any_element()
    }

    /// Explains how a batch ended when some items stayed in place or it was stopped.
    pub(crate) fn open_trash_report(
        &mut self,
        report: TrashReport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let permanent = report.mode == DeleteMode::Permanent;
        let title = match (permanent, report.left.is_empty()) {
            (_, true) => t!(trash_key!("report.title_stopped")).into_owned(),
            (false, false) => t!("report.title_left").into_owned(),
            (true, false) => t!("report.perm_title_left").into_owned(),
        };
        let mut summary = vec![
            t!(
                if permanent {
                    "report.perm_moved"
                } else {
                    trash_key!("report.moved")
                },
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
                    // `min_h_0` down to the list lets it shrink on a short window, where the
                    // dialog stops at the window's height, so the buttons stay in view.
                    v_flex()
                        .min_h_0()
                        .child(modal_header(title.clone(), cx))
                        .child(
                            modal_body()
                                .min_h_0()
                                .child(
                                    div()
                                        .text_color(palette.fg_muted)
                                        .line_height(px(19.0))
                                        .child(summary.clone()),
                                )
                                .when(!items.is_empty(), |body| {
                                    body.child(item_list("report-items", &items, None, true, cx))
                                })
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .line_height(px(18.0))
                                        .text_color(palette.fg_subtle)
                                        .child(if permanent {
                                            t!("report.perm_partly")
                                        } else {
                                            t!("report.nothing_deleted")
                                        }),
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

/// The confirmation's header. As telaio's `modal_header`, except that a long title wraps instead
/// of pushing the close button out of the card, so "permanently" stays in sight, and that a
/// running batch has no close button.
fn confirm_header(title: String, closable: bool, palette: &Palette) -> impl IntoElement {
    h_flex()
        .min_h(px(MODAL_HEADER_HEIGHT))
        .flex_none()
        .items_center()
        .gap(px(12.0))
        .px(px(20.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .when(closable, |header| {
            header.child(
                IconButton::new("confirm-close", IconName::Close)
                    .large()
                    .tooltip(t!("common.close"))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            )
        })
}

/// One of the two choices of the confirmation.
#[allow(clippy::too_many_arguments)]
fn delete_option(
    id: &'static str,
    option: DeleteMode,
    current: DeleteMode,
    running: bool,
    title: SharedString,
    description: SharedString,
    tag: Option<SharedString>,
    weak: WeakEntity<AppView>,
    palette: &Palette,
) -> impl IntoElement {
    let on = option == current;
    let (accent, overlay, border_strong) = (palette.accent, palette.overlay, palette.border_strong);
    let pressed = palette.pressed;
    h_flex()
        .id(id)
        .items_start()
        .gap(px(10.0))
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(if on { accent } else { palette.border })
        .when(on, |card| card.bg(palette.accent_subtle))
        .when(!running, |card| {
            card.cursor_pointer()
                .hover(move |style| {
                    if on {
                        style
                    } else {
                        style.border_color(border_strong)
                    }
                })
                .active(move |style| if on { style } else { style.bg(pressed) })
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        view.delete_mode = option;
                        view.batch_note = None;
                        cx.notify();
                    });
                })
        })
        .child(
            div()
                .flex_none()
                .mt(px(2.0))
                .size(px(14.0))
                .rounded(px(7.0))
                .border_1()
                .border_color(if on { accent } else { border_strong })
                .flex()
                .items_center()
                .justify_center()
                .when(on, |dot| {
                    dot.child(div().size(px(8.0)).rounded(px(4.0)).bg(accent))
                })
                .when(!on, |dot| dot.bg(overlay)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .children(tag.map(|tag| {
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded(px(4.0))
                                .bg(palette.success_subtle)
                                .text_color(palette.success)
                                .text_size(px(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .child(tag)
                        })),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(17.0))
                        .text_color(palette.fg_muted)
                        .child(description),
                ),
        )
}

/// How far the running batch is: files for a permanent deletion, items for the Trash, which only
/// knows when each one has moved.
fn batch_progress(progress: &TrashProgress, palette: &Palette, mono: &SharedString) -> Div {
    let fraction = progress.fraction();
    let permanent = progress.mode == DeleteMode::Permanent;
    let bytes = format::size(progress.bytes_done());
    let total_bytes = format::size(progress.total_bytes);
    let detail = if permanent {
        t!(
            "confirm.progress_files",
            done = format::count(progress.files_done()),
            total = format::count(progress.total_files),
            size = bytes,
            total_size = total_bytes
        )
    } else {
        t!(
            "confirm.progress_items",
            done = format::count(progress.done.load(Ordering::Relaxed) as u64),
            total = format::count(progress.total as u64),
            size = bytes,
            total_size = total_bytes
        )
    };
    let bar = ProgressBar::new(fraction);
    v_flex()
        .gap(px(8.0))
        .p(px(12.0))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(palette.border)
        .bg(palette.surface)
        .child(
            h_flex()
                .items_baseline()
                .justify_between()
                .gap(px(12.0))
                .child(div().font_weight(FontWeight::MEDIUM).child(if permanent {
                    t!("confirm.deleting")
                } else {
                    t!("confirm.moving")
                }))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(px(12.0))
                        .text_color(palette.fg_muted)
                        .child(format!("{}%", format::count((fraction * 100.0) as u64))),
                ),
        )
        .child(h_flex().child(if permanent {
            bar.tone(Tone::Danger)
        } else {
            bar
        }))
        .child(
            div()
                .font_family(mono.clone())
                .text_size(px(11.5))
                .text_color(palette.fg_subtle)
                .child(detail),
        )
}

/// The bordered list of `items` with an optional `footer` row. When `capped`, it stops at about
/// five rows and scrolls past that, with the footer in view; otherwise it is as tall as its rows,
/// for a parent that scrolls.
fn item_list(
    id: &'static str,
    items: &[ListItem],
    footer: Option<AnyElement>,
    capped: bool,
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
        .min_h_0()
        .overflow_hidden()
        // The scroll area needs a definite height: the capped column gives it one, the layout
        // gpui-component's own max height test uses.
        .map(|list| {
            if capped {
                list.child(
                    v_flex().min_h_0().max_h(px(CONFIRM_LIST_MAX_HEIGHT)).child(
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
            } else {
                list.children(rows)
            }
        })
        .children(footer)
}
