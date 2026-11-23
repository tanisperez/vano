//! The home screen: the connected disks, the WSL 2 distributions on Windows, the home folder and
//! the folder picker, the recent scans, and a footer with the Scan button.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use vano_system::wsl::Distro;
use vano_system::{Drive, Media};

use crate::state::format;
use crate::state::target::{Target, TargetKind, disk_name, drive_label};
use crate::state::trash::trash_key;
use crate::state::usage::Severity;
use crate::theme::metrics::{
    DRIVE_BAR_HEIGHT, FOLDER_CARD_HEIGHT, HOME_COLUMN_WIDTH, HOME_LOGO_SIZE, RADIUS_LG,
    RECENT_ROW_HEIGHT, WSL_ROW_HEIGHT,
};
use crate::theme::{Palette, PaletteExt, VanoColors};

use super::common::eyebrow;
use super::components::{IconButton, InlineMessage, InlineMessageKind, PrimaryButton, UsageBar};
use super::root::AppView;
use super::title_bar::target_icon;

/// The Full Disk Access list of System Settings, Privacy and Security, on macOS.
const FULL_DISK_ACCESS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// From this fraction a disk shows "Almost full".
const ALMOST_FULL: f32 = 0.9;

impl AppView {
    pub(crate) fn render_home(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = cx.palette();
        let palette = &*palette;
        let mono = cx.theme().mono_font_family.clone();

        let column = v_flex()
            .w_full()
            .max_w(px(HOME_COLUMN_WIDTH))
            .pt(px(32.0))
            .pb(px(16.0))
            .gap(px(20.0))
            .child(render_header(palette))
            .child(self.render_drives(palette, &mono, cx))
            .children((!self.distros.is_empty()).then(|| self.render_wsl(palette, &mono, cx)))
            .child(self.render_folders(palette, &mono, cx))
            .children((!self.recents.is_empty()).then(|| self.render_recents(palette, &mono, cx)));

        v_flex()
            .size_full()
            .child(
                div()
                    .id("home")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(h_flex().justify_center().px(px(40.0)).child(column)),
            )
            .child(self.render_home_footer(palette, cx))
            .into_any_element()
    }

    /// The disks, two to a row, with how full each one is.
    fn render_drives(
        &self,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Until the first read comes back, the system disk from what is already known.
        let drives: Vec<DriveCard> = if self.drives.is_empty() {
            self.disk
                .map(|usage| DriveCard {
                    target: Target::disk(),
                    title: disk_name(),
                    kind: media_name(Media::Unknown, false),
                    removable: false,
                    used: usage.used(),
                    total: usage.total,
                    free: usage.available,
                })
                .into_iter()
                .collect()
        } else {
            self.drives.iter().map(DriveCard::new).collect()
        };
        let used: u64 = drives.iter().map(|drive| drive.used).sum();
        let total: u64 = drives.iter().map(|drive| drive.total).sum();
        let summary = format!(
            "{} \u{b7} {}",
            format::counted("home.drive_count", drives.len() as u64),
            t!(
                "home.disk_caption",
                used = format::size(used),
                total = format::size(total)
            )
        );
        let mut cards: Vec<AnyElement> = drives
            .into_iter()
            .enumerate()
            .map(|(index, drive)| self.drive_card(index, drive, palette, mono, cx))
            .collect();
        let mut rows = Vec::new();
        while !cards.is_empty() {
            let rest = cards.split_off(cards.len().min(2));
            let mut row = h_flex().gap(px(10.0)).items_stretch();
            let filled = cards.len();
            for card in cards {
                row = row.child(card);
            }
            if filled == 1 {
                row = row.child(div().flex_1());
            }
            rows.push(row);
            cards = rest;
        }
        // macOS keeps protected folders (Mail, Messages, Safari...) from apps without Full Disk
        // Access (#36): a disk scan undercounts, so say why and where to grant it.
        let disk_selected = matches!(self.target.kind, TargetKind::Disk | TargetKind::Drive);
        let full_disk_note = (cfg!(target_os = "macos") && disk_selected).then(|| {
            InlineMessage::new(InlineMessageKind::Info, t!("home.full_disk_access"))
                .action(t!("home.full_disk_access_open"), |_, cx| {
                    cx.open_url(FULL_DISK_ACCESS_URL)
                })
        });
        section(
            eyebrow(palette, t!("home.drives")),
            Some(summary),
            palette,
            mono,
        )
        .child(v_flex().gap(px(10.0)).children(rows))
        .children(full_disk_note)
        .into_any_element()
    }

    fn drive_card(
        &self,
        index: usize,
        drive: DriveCard,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.target.same_place(&drive.target);
        let fraction = drive.used as f32 / drive.total.max(1) as f32;
        let icon = if drive.removable {
            Lucide::Usb
        } else {
            Lucide::HardDrive
        };
        let target = drive.target.clone();
        let double = drive.target;
        v_flex()
            .id(("drive", index))
            .flex_1()
            .min_w_0()
            .py(px(12.0))
            .px(px(14.0))
            .gap(px(8.0))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(if selected {
                palette.accent
            } else {
                palette.border
            })
            .when(selected, |card| card.border_2().py(px(11.0)).px(px(13.0)))
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
                    .min_w_0()
                    .child(Icon::new(icon).small().text_color(if selected {
                        palette.accent_text
                    } else {
                        palette.fg_subtle
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(drive.title),
                    )
                    .when(fraction >= ALMOST_FULL, |row| {
                        row.child(tag(
                            t!("home.almost_full"),
                            palette.danger,
                            palette.danger_subtle,
                            None,
                        ))
                    })
                    .child(tag(
                        drive.kind,
                        palette.fg_muted,
                        palette.surface,
                        Some(palette.border),
                    )),
            )
            .child(UsageBar::new(fraction, DRIVE_BAR_HEIGHT).round_fill())
            .child(
                h_flex()
                    .items_baseline()
                    .justify_between()
                    .gap(px(8.0))
                    .text_size(px(12.0))
                    .text_color(palette.fg_muted)
                    .child(div().min_w_0().truncate().child(t!(
                        "home.drive_caption",
                        used = format::size(drive.used),
                        total = format::size(drive.total),
                        free = format::size(drive.free)
                    )))
                    .child(
                        div()
                            .flex_none()
                            .font_family(mono.clone())
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.usage_severity(Severity::of(fraction)))
                            .child(t!(
                                "home.disk_full",
                                pct = format::count((fraction * 100.0).round() as u64)
                            )),
                    ),
            )
            .on_click(cx.listener(move |view, event: &ClickEvent, _, cx| {
                view.set_target(target.clone(), cx);
                if event.click_count() >= 2 {
                    view.target = double.clone();
                    view.start_scan(cx);
                }
            }))
            .into_any_element()
    }

    /// The WSL 2 distributions: one row each, with the size of its virtual disk.
    fn render_wsl(
        &self,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on_disk: u64 = self.distros.iter().map(|distro| distro.vhdx_size).sum();
        let summary = format!(
            "{} \u{b7} {}",
            format::counted("home.wsl_count", self.distros.len() as u64),
            t!("home.wsl_on_disk", size = format::size(on_disk))
        );
        let rows = self
            .distros
            .iter()
            .enumerate()
            .map(|(index, distro)| self.wsl_row(index, distro, palette, mono, cx));
        section(
            eyebrow(palette, t!("home.wsl")),
            Some(summary),
            palette,
            mono,
        )
        .child(
            v_flex()
                .gap(px(1.0))
                .bg(palette.border)
                .border_1()
                .border_color(palette.border)
                .rounded(px(RADIUS_LG))
                .overflow_hidden()
                .children(rows),
        )
        .into_any_element()
    }

    fn wsl_row(
        &self,
        index: usize,
        distro: &Distro,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = Target::wsl(&distro.name);
        let selected = self.target.same_place(&target);
        let kind = if distro.is_docker() {
            "Docker"
        } else {
            "WSL 2"
        };
        let (state, dot) = if distro.running {
            (t!("home.wsl_running"), palette.success)
        } else {
            (t!("home.wsl_stopped"), palette.fg_subtle)
        };
        let inside = distro
            .used_inside
            .map(|used| t!("home.wsl_inside", size = format::size(used)).into_owned());
        h_flex()
            .id(("wsl", index))
            .h(px(WSL_ROW_HEIGHT))
            .px(px(14.0))
            .gap(px(10.0))
            .items_center()
            .min_w_0()
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
                    style.bg(palette.hover)
                }
            })
            .child(Icon::new(Lucide::Terminal).small().text_color(if selected {
                palette.accent_text
            } else {
                palette.fg_subtle
            }))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .child(distro.name.clone()),
            )
            .child(tag(
                kind,
                palette.fg_muted,
                palette.surface,
                Some(palette.border),
            ))
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(5.0))
                    .items_center()
                    .text_size(px(12.0))
                    .text_color(palette.fg_subtle)
                    .child(div().size(px(6.0)).rounded_full().bg(dot))
                    .child(state),
            )
            .child(div().flex_1())
            .children(inside.map(|inside| {
                div()
                    .flex_none()
                    .text_size(px(12.0))
                    .text_color(palette.fg_subtle)
                    .child(inside)
            }))
            .child(
                div()
                    .flex_none()
                    .w(px(64.0))
                    .text_right()
                    .font_family(mono.clone())
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(format::size(distro.vhdx_size)),
            )
            .on_click(cx.listener(move |view, event: &ClickEvent, _, cx| {
                view.set_target(target.clone(), cx);
                if event.click_count() >= 2 {
                    view.start_scan(cx);
                }
            }))
            .into_any_element()
    }

    /// The home folder and the folder picker (or the folder picked).
    fn render_folders(
        &self,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let home_path = self
            .home
            .as_deref()
            .map(|home| format!("~  {}", home.display()))
            .unwrap_or_default();
        let home = Target::home().map(|target| {
            self.folder_card(
                "folder-home",
                Lucide::House,
                target.label(),
                home_path,
                Some(target),
                false,
                palette,
                mono,
                cx,
            )
        });
        let picked = self.target.kind == TargetKind::Folder;
        let picker = if picked {
            self.folder_card(
                "folder-pick",
                Lucide::FolderOpen,
                self.target.label(),
                format::short_path(&self.target.path, self.home.as_deref()),
                None,
                true,
                palette,
                mono,
                cx,
            )
        } else {
            self.folder_card(
                "folder-pick",
                Lucide::FolderOpen,
                t!("home.choose").into_owned(),
                t!("home.folder_path").into_owned(),
                None,
                false,
                palette,
                mono,
                cx,
            )
        };
        section(eyebrow(palette, t!("home.folders")), None, palette, mono)
            .child(h_flex().gap(px(10.0)).children(home).child(picker))
            .into_any_element()
    }

    /// A folder card. Without a target it opens the folder picker; `picked` marks the folder
    /// already chosen.
    #[allow(clippy::too_many_arguments)]
    fn folder_card(
        &self,
        id: &'static str,
        icon: Lucide,
        title: String,
        path: String,
        target: Option<Target>,
        picked: bool,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = picked
            || target
                .as_ref()
                .is_some_and(|target| self.target.same_place(target));
        let dashed = target.is_none() && !picked;
        h_flex()
            .id(id)
            .flex_1()
            .min_w_0()
            .h(px(FOLDER_CARD_HEIGHT))
            .px(px(14.0))
            .gap(px(8.0))
            .items_center()
            .rounded(px(RADIUS_LG))
            .border_1()
            .when(dashed, |card| card.border_dashed())
            .border_color(if selected {
                palette.accent
            } else {
                palette.border
            })
            .when(selected, |card| card.border_2().px(px(13.0)))
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
            .child(Icon::new(icon).small().text_color(if selected {
                palette.accent_text
            } else {
                palette.fg_subtle
            }))
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(mono.clone())
                    .text_size(px(12.0))
                    .text_color(palette.fg_subtle)
                    .child(path),
            )
            .on_click(
                cx.listener(move |view, event: &ClickEvent, _, cx| match &target {
                    Some(target) => {
                        view.set_target(target.clone(), cx);
                        if event.click_count() >= 2 {
                            view.start_scan(cx);
                        }
                    }
                    None => view.choose_folder(false, cx),
                }),
            )
            .into_any_element()
    }

    fn render_recents(
        &self,
        palette: &Palette,
        mono: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let now = format::now();
        let rows = self
            .recents
            .clone()
            .into_iter()
            .enumerate()
            .map(|(index, recent)| {
                let meta = format!(
                    "{} \u{b7} {}",
                    format::size(recent.size),
                    format::age(recent.scanned_at, now)
                );
                let name = recent.target.label();
                let forgotten = recent.clone();
                h_flex()
                    .id(("recent", index))
                    .h(px(RECENT_ROW_HEIGHT))
                    .pl(px(4.0))
                    .gap(px(12.0))
                    .items_center()
                    .rounded(px(7.0))
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
                            .text_size(px(12.0))
                            .text_color(palette.accent_text)
                            .cursor_pointer()
                            .hover(|style| style.text_color(palette.fg))
                            .child(t!("home.open_results"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.open_recent(recent.clone(), cx);
                            })),
                    )
                    .child(
                        IconButton::new(("recent-forget", index), IconName::Close)
                            .tooltip(t!("home.forget_recent"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.forget_recent(&forgotten, cx);
                            })),
                    )
            });
        section(eyebrow(palette, t!("home.recent")), None, palette, mono)
            .gap(px(2.0))
            .children(rows)
            .into_any_element()
    }

    /// The footer pinned under the list: what Vano does with the files, and the Scan button.
    fn render_home_footer(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let scan_label = t!("home.scan", target = self.target.label()).into_owned();
        h_flex()
            .flex_none()
            .justify_center()
            .px(px(40.0))
            .border_t_1()
            .border_color(palette.border)
            .bg(palette.bg)
            .child(
                h_flex()
                    .w_full()
                    .max_w(px(HOME_COLUMN_WIDTH))
                    .py(px(10.0))
                    .gap(px(16.0))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.0))
                            .line_height(px(17.0))
                            .text_color(palette.fg_muted)
                            .child(t!(trash_key!("home.footer"))),
                    )
                    .child(
                        PrimaryButton::new("scan", scan_label)
                            .key_hint("\u{21b5}")
                            .on_click(cx.listener(|view, _, _, cx| view.start_scan(cx))),
                    ),
            )
            .into_any_element()
    }
}

/// What a disk card shows.
struct DriveCard {
    target: Target,
    title: String,
    kind: String,
    removable: bool,
    used: u64,
    total: u64,
    free: u64,
}

impl DriveCard {
    fn new(drive: &Drive) -> Self {
        let target = Target::drive(drive);
        let title = if target.kind == TargetKind::Disk {
            disk_name()
        } else {
            drive_label(drive.label.as_deref(), &drive.mount_point)
        };
        DriveCard {
            target,
            title,
            kind: media_name(drive.media, drive.removable),
            removable: drive.removable,
            used: drive.used(),
            total: drive.total,
            free: drive.available,
        }
    }
}

/// `NVMe SSD`, `External HDD`, in the language of the app.
fn media_name(media: Media, removable: bool) -> String {
    let name = match media {
        Media::Nvme => "NVMe SSD".to_string(),
        Media::Ssd => "SSD".to_string(),
        Media::Hdd => "HDD".to_string(),
        Media::Unknown => t!("home.kind_disk").into_owned(),
    };
    if removable {
        t!("home.kind_external", kind = name).into_owned()
    } else {
        name
    }
}

/// A section of the home screen: its title, an optional summary on the right, then the content.
fn section(title: Div, summary: Option<String>, palette: &Palette, mono: &SharedString) -> Div {
    v_flex().gap(px(6.0)).child(
        h_flex()
            .px(px(4.0))
            .pb(px(4.0))
            .items_baseline()
            .justify_between()
            .gap(px(12.0))
            .child(title)
            .children(summary.map(|summary| {
                div()
                    .font_family(mono.clone())
                    .text_size(px(12.0))
                    .text_color(palette.fg_subtle)
                    .child(summary)
            })),
    )
}

/// A small label: the kind of a disk, "Almost full", "Docker".
fn tag(text: impl Into<SharedString>, fg: Hsla, bg: Hsla, border: Option<Hsla>) -> Div {
    div()
        .flex_none()
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(4.0))
        .bg(bg)
        .when_some(border, |tag, border| tag.border_1().border_color(border))
        .text_color(fg)
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .child(text.into())
}

/// The "V" logo, the name and the tagline.
fn render_header(palette: &Palette) -> impl IntoElement {
    h_flex()
        .gap(px(14.0))
        .items_center()
        .child(
            div()
                .flex_none()
                .size(px(HOME_LOGO_SIZE))
                .rounded(px(10.0))
                .bg(palette.accent)
                .text_color(palette.accent_fg)
                .text_size(px(20.0))
                .font_weight(FontWeight::BOLD)
                .flex()
                .items_center()
                .justify_center()
                .child("V"),
        )
        .child(
            v_flex()
                .gap(px(3.0))
                .child(
                    div()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Vano"),
                )
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(palette.fg_muted)
                        .child(t!("home.tagline")),
                ),
        )
}
