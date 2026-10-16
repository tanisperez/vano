//! The status bar: the disk's free space at the left, what Vano is doing at the right, and a
//! "Restart to update" link when an update is ready.

use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use vano_core::ROOT;

use crate::state::format;
use crate::state::target::disk_name;
use crate::theme::PaletteExt;
use crate::theme::metrics::STATUS_BAR_HEIGHT;

use super::root::{AppView, Screen};

impl AppView {
    /// Renders the status bar.
    pub(crate) fn render_status_bar(&self, cx: &Context<Self>) -> AnyElement {
        let palette = cx.palette();
        let left = match self.disk {
            Some(disk) => {
                let text = t!(
                    "status.disk",
                    disk = disk_name(),
                    free = format::size(disk.available),
                    total = format::size(disk.total)
                )
                .into_owned();
                // Space the file system keeps for the administrator (ext4: 5%): neither free for
                // the user nor taken by any file, so it is said apart.
                if disk.reserved() >= 1_000_000 {
                    format!(
                        "{text} \u{b7} {}",
                        t!("status.reserved", size = format::size(disk.reserved()))
                    )
                } else {
                    text
                }
            }
            None => disk_name(),
        };
        let right = match self.screen {
            Screen::Home => t!("status.ready").into_owned(),
            Screen::Scanning => match self.scan.as_ref() {
                Some(session) if session.finished => t!("status.complete").into_owned(),
                Some(session) => match session.fraction() {
                    Some(fraction) => t!(
                        "status.scanning_pct",
                        target = session.target.label(),
                        pct = ((fraction * 100.0).floor() as u32).to_string()
                    )
                    .into_owned(),
                    None => t!("status.scanning", target = session.target.label()).into_owned(),
                },
                None => String::new(),
            },
            Screen::Results => self
                .results
                .as_ref()
                .map(|results| {
                    t!(
                        "status.results",
                        files = format::counted(
                            "common.files",
                            u64::from(results.tree.node(ROOT).files)
                        ),
                        seconds = format::seconds(results.seconds)
                    )
                    .into_owned()
                })
                .unwrap_or_default(),
        };
        let right = if self.trashing {
            t!("status.trashing").into_owned()
        } else {
            right
        };
        let ready = self.state.update.ready().map(|ready| ready.version.clone());

        h_flex()
            .flex_none()
            .h(px(STATUS_BAR_HEIGHT))
            .items_center()
            .gap(px(16.0))
            .px_3()
            .bg(palette.surface)
            .border_t_1()
            .border_color(palette.border)
            .text_size(px(11.5))
            .text_color(palette.fg_subtle)
            .child(
                h_flex()
                    .min_w_0()
                    .gap(px(6.0))
                    .items_center()
                    .child(Icon::new(IconName::HardDrive).small())
                    .child(div().truncate().child(left)),
            )
            .child(div().flex_1())
            .children(ready.map(|version| {
                div()
                    .id("status-update")
                    .flex_none()
                    .text_color(palette.accent_text)
                    .cursor_pointer()
                    .hover(|style| style.text_color(palette.fg))
                    .child(t!("status.update_ready", version = version))
                    .on_click(
                        cx.listener(|view, _, window, cx| view.request_update_restart(window, cx)),
                    )
            }))
            .child(div().flex_none().truncate().child(right))
            .into_any_element()
    }
}
