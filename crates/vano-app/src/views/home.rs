//! The home screen: what to scan (home folder, the whole disk, a folder), the Scan button, and
//! the recent scans.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::state::format;
use crate::state::target::{Target, TargetKind};
use crate::theme::metrics::{
    HOME_COLUMN_WIDTH, HOME_LOGO_SIZE, LOCATION_CARD_HEIGHT, RADIUS_LG, RECENT_ROW_HEIGHT,
};
use crate::theme::{Palette, PaletteExt};

use super::common::eyebrow;
use super::components::PrimaryButton;
use super::root::AppView;
use super::title_bar::target_icon;

impl AppView {
    pub(crate) fn render_home(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = cx.palette();
        let palette = &*palette;
        let mono = cx.theme().mono_font_family.clone();
        let home = self.home.clone();
        let selected = self.target.kind;

        let home_card = Target::home().map(|target| {
            let path = home
                .as_deref()
                .map(|h| format!("~  {}", h.display()))
                .unwrap_or_default();
            self.location_card(
                "loc-home",
                target,
                selected == TargetKind::Home,
                path,
                t!("home.home_caption").into_owned(),
                None,
                false,
                palette,
                &mono,
                cx,
            )
        });
        let disk = Target::disk();
        let (disk_caption, disk_fraction) = match self.disk {
            Some(usage) => {
                let used = usage.used();
                (
                    t!(
                        "home.disk_caption",
                        used = format::size(used),
                        total = format::size(usage.total)
                    )
                    .into_owned(),
                    Some(used as f32 / usage.total.max(1) as f32),
                )
            }
            None => (String::new(), None),
        };
        let disk_path = disk.path.display().to_string();
        let disk_card = self.location_card(
            "loc-disk",
            disk,
            selected == TargetKind::Disk,
            disk_path,
            disk_caption,
            disk_fraction,
            false,
            palette,
            &mono,
            cx,
        );
        let folder_card = if selected == TargetKind::Folder {
            let target = self.target.clone();
            let path = format::short_path(&target.path, home.as_deref());
            self.location_card(
                "loc-folder",
                target,
                true,
                path,
                t!("home.folder_change").into_owned(),
                None,
                true,
                palette,
                &mono,
                cx,
            )
        } else {
            self.location_card(
                "loc-folder",
                Target::folder(Default::default()),
                false,
                t!("home.folder_path").into_owned(),
                t!("home.folder_caption").into_owned(),
                None,
                true,
                palette,
                &mono,
                cx,
            )
        };

        let scan_label = t!("home.scan", target = self.target.label()).into_owned();
        let recents = self.recents.clone();
        let now = format::now();

        let column = v_flex()
            .w_full()
            .max_w(px(HOME_COLUMN_WIDTH))
            .pt(px(56.0))
            .pb(px(24.0))
            .gap(px(28.0))
            .child(
                h_flex()
                    .gap(px(16.0))
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .size(px(HOME_LOGO_SIZE))
                            .rounded(px(12.0))
                            .bg(palette.accent)
                            .text_color(palette.accent_fg)
                            .text_size(px(24.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("V"),
                    )
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(26.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Vano"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(palette.fg_muted)
                                    .child(t!("home.tagline")),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(12.0))
                    .child(div().px(px(10.0)).child(eyebrow(palette, t!("home.what"))))
                    .child(
                        h_flex()
                            .gap(px(12.0))
                            .items_stretch()
                            .children(home_card)
                            .child(disk_card)
                            .child(folder_card),
                    ),
            )
            .child(
                h_flex().child(
                    PrimaryButton::new("scan", scan_label)
                        .key_hint("\u{21b5}")
                        .on_click(cx.listener(|view, _, _, cx| view.start_scan(cx))),
                ),
            )
            .when(!recents.is_empty(), |column| {
                column.child(
                    v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .px(px(10.0))
                                .pb(px(6.0))
                                .child(eyebrow(palette, t!("home.recent"))),
                        )
                        .children(recents.into_iter().enumerate().map(|(index, recent)| {
                            let meta = format!(
                                "{} \u{b7} {}",
                                format::size(recent.size),
                                format::age(recent.scanned_at, now)
                            );
                            let name = recent.target.label();
                            h_flex()
                                .id(("recent", index))
                                .h(px(RECENT_ROW_HEIGHT))
                                .px(px(10.0))
                                .gap(px(16.0))
                                .items_center()
                                .rounded(px(8.0))
                                .hover(|style| style.bg(palette.hover))
                                .child(
                                    Icon::new(target_icon(recent.target.kind))
                                        .small()
                                        .text_color(palette.fg_subtle),
                                )
                                .child(div().w(px(150.0)).truncate().child(name))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_family(mono.clone())
                                        .text_size(px(12.0))
                                        .text_color(palette.fg_subtle)
                                        .child(meta),
                                )
                                .child(
                                    div()
                                        .id(("recent-open", index))
                                        .text_size(px(12.5))
                                        .text_color(palette.accent_text)
                                        .cursor_pointer()
                                        .hover(|style| style.text_color(palette.fg))
                                        .child(t!("home.open_results"))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.open_recent(recent.clone(), cx);
                                        })),
                                )
                        })),
                )
            })
            .child(div().flex_1())
            .child(
                div()
                    .pt(px(16.0))
                    .border_t_1()
                    .border_color(palette.border)
                    .text_size(px(12.5))
                    .text_color(palette.fg_muted)
                    .child(t!("home.footer")),
            );

        div()
            .id("home")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                h_flex()
                    .min_h_full()
                    .items_stretch()
                    .justify_center()
                    .px(px(24.0))
                    .child(column),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn location_card(
        &self,
        id: &'static str,
        target: Target,
        selected: bool,
        path: String,
        caption: String,
        fraction: Option<f32>,
        picker: bool,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = if picker && !selected {
            t!("home.choose").into_owned()
        } else {
            target.label()
        };
        let icon = if picker {
            Lucide::FolderOpen
        } else {
            target_icon(target.kind)
        };
        v_flex()
            .id(id)
            .flex_1()
            .min_w_0()
            .min_h(px(LOCATION_CARD_HEIGHT))
            .p(px(16.0))
            .gap(px(8.0))
            .rounded(px(RADIUS_LG))
            .border_1()
            .when(picker && !selected, |card| card.border_dashed())
            .border_color(if selected {
                palette.accent
            } else {
                palette.border
            })
            .when(selected, |card| card.border_2().p(px(15.0)))
            .bg(if selected {
                palette.accent_subtle
            } else {
                palette.raised
            })
            .cursor_pointer()
            .hover(move |style| {
                if selected {
                    style
                } else {
                    style.border_color(palette.border_strong)
                }
            })
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(Icon::new(icon).small().text_color(if selected {
                        palette.accent_text
                    } else {
                        palette.fg_subtle
                    }))
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    ),
            )
            .child(
                div()
                    .font_family(mono.clone())
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(palette.fg_subtle)
                    .child(path),
            )
            .children(fraction.map(|f| {
                div().h(px(4.0)).rounded(px(2.0)).bg(palette.hover).child(
                    div()
                        .h_full()
                        .w(relative(f.clamp(0.0, 1.0)))
                        .rounded(px(2.0))
                        .bg(palette.fg_muted),
                )
            }))
            .child(
                div()
                    .text_size(px(12.5))
                    .line_height(px(17.0))
                    .text_color(palette.fg_muted)
                    .child(caption),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                if picker {
                    view.choose_folder(cx);
                } else {
                    view.set_target(target.clone(), cx);
                }
            }))
            .into_any_element()
    }
}
