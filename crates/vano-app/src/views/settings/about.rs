//! The "About" pane of the Settings modal: the version, the release links and the updater. The
//! updater logic is `state::update` and `views/update.rs`; this only renders it and forwards the
//! clicks to the same flows the status bar and the command palette use. Everything it shows is
//! captured once per frame from [`AppView`] as plain data ([`AboutPane`]), with no IO.

use gpui_kit::component::switch::Switch;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::state::about::{AboutInfo, IDENTITY};
use crate::state::update::{CheckStatus, ReadyUpdate, updater_enabled};
use crate::theme::Palette;
use crate::theme::metrics::{CONTROL_HEIGHT, RADIUS_LG};
use crate::views::components::{PrimaryButton, SecondaryButton};
use crate::views::root::AppView;

use super::{divider, labeled_row};

/// Height of the About footer strip.
const FOOTER_HEIGHT: f32 = 48.0;

/// What the pane needs from [`AppView`], cloned out so nothing borrows the view while rendering.
pub(super) struct AboutPane {
    info: AboutInfo,
    enabled: bool,
    check_updates: bool,
    ready: Option<ReadyUpdate>,
    status: CheckStatus,
}

impl AboutPane {
    /// Captures the pane's data from `view`.
    pub(super) fn capture(view: &AppView) -> Self {
        Self {
            info: view.about_info.clone(),
            enabled: updater_enabled(),
            check_updates: view.state.settings.check_updates,
            ready: view.state.update.ready().cloned(),
            status: view.state.update.status(),
        }
    }
}

/// The pane's body sections: the header row, a divider and the updates section.
pub(super) fn render_about(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    mono_font_family: &SharedString,
    pane: &AboutPane,
) -> Vec<AnyElement> {
    vec![
        telaio_app::about::header(&IDENTITY, &pane.info.platform, palette, mono_font_family),
        divider(palette),
        render_updates(weak, palette, mono_font_family, pane),
    ]
}

/// The pane's footer: what Vano sends over the network, which depends on the updater.
pub(super) fn render_about_footer(palette: &Palette) -> AnyElement {
    let text = if updater_enabled() {
        t!("settings.about.footer.updater")
    } else {
        t!("settings.about.footer.offline")
    };
    h_flex()
        .h(px(FOOTER_HEIGHT))
        .flex_none()
        .items_center()
        .px(px(20.0))
        .border_t_1()
        .border_color(palette.border)
        .text_color(palette.fg_subtle)
        .text_size(px(12.0))
        .child(text)
        .into_any_element()
}

/// The "Updates" section: variant 3a with an updater, 3b without.
fn render_updates(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    mono_font_family: &SharedString,
    pane: &AboutPane,
) -> AnyElement {
    let title = div()
        .font_weight(FontWeight::MEDIUM)
        .child(t!("update.setting.title"));
    let body = if pane.enabled {
        render_managed_updates(weak, palette, mono_font_family, pane)
    } else {
        render_distro_updates(palette, mono_font_family, &pane.info)
    };
    v_flex()
        .gap(px(12.0))
        .child(title)
        .children(body)
        .into_any_element()
}

/// Variant 3a: the ready banner or the check status, the automatic check switch and "Check now".
fn render_managed_updates(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    mono_font_family: &SharedString,
    pane: &AboutPane,
) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if let Some(ready) = &pane.ready {
        rows.push(render_ready_banner(weak.clone(), palette, ready));
    } else if let Some(row) = render_status_row(palette, pane.status, pane.info.version) {
        rows.push(row);
    }

    let auto_weak = weak.clone();
    let switch = Switch::new("settings-check-updates")
        .checked(pane.check_updates)
        .on_click(move |checked, window, cx| {
            let checked = *checked;
            let _ = auto_weak.update(cx, |view, cx| view.set_check_updates(checked, window, cx));
        })
        .into_any_element();
    rows.push(labeled_row(
        palette,
        t!("update.setting.check"),
        Some(t!("update.setting.check_description")),
        switch,
    ));

    let busy = pane.status == CheckStatus::Checking || pane.ready.is_some();
    let check = SecondaryButton::new("about-check-now", t!("shell.palette.check_for_updates"))
        .icon(Icon::new(gpui_kit::assets::IconName::RefreshCw))
        .disabled(busy)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |view, cx| view.check_for_updates_manually(window, cx));
        });
    let mut leading = v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(2.0))
        .child(div().child(t!("settings.about.check_now")));
    // No download directory (a portable copy) means there is nothing to say about downloads.
    if let Some(dir) = &pane.info.download_dir {
        leading = leading.child(
            h_flex()
                .gap(px(4.0))
                .text_color(palette.fg_muted)
                .text_size(px(12.0))
                .child(t!("settings.about.downloads_to"))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(mono_font_family.clone())
                        .child(dir.display().to_string()),
                ),
        );
    }
    rows.push(
        h_flex()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(leading)
            .child(div().flex_none().h(px(CONTROL_HEIGHT)).child(check))
            .into_any_element(),
    );
    rows
}

/// The accent banner shown once an update is downloaded and verified, or, for a portable copy,
/// as soon as a newer version exists.
fn render_ready_banner(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    ready: &ReadyUpdate,
) -> AnyElement {
    let version = ready.version.as_str();
    let (title, text) = if ready.is_manual_download() {
        (
            t!("settings.about.available.title", version = version),
            t!("settings.about.available.text"),
        )
    } else {
        (
            t!("settings.about.ready.title", version = version),
            t!("settings.about.ready.text"),
        )
    };
    let button = if ready.is_manual_download() {
        let notes_url = ready.notes_url.clone();
        PrimaryButton::new("about-download", t!("settings.about.available.download"))
            .on_click(move |_, _, cx| cx.open_url(&notes_url))
    } else {
        PrimaryButton::new("about-restart", t!("settings.about.ready.restart")).on_click(
            move |_, window, cx| {
                let _ = weak.update(cx, |view, cx| view.request_update_restart(window, cx));
            },
        )
    };
    h_flex()
        .items_center()
        .gap(px(12.0))
        .p(px(14.0))
        .rounded(px(RADIUS_LG))
        .bg(palette.accent_subtle)
        .border_1()
        .border_color(palette.accent.opacity(0.35))
        .child(
            Icon::new(gpui_kit::assets::IconName::Download)
                .size(px(18.0))
                .text_color(palette.accent_text),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(
                    div()
                        .text_color(palette.fg_muted)
                        .text_size(px(12.0))
                        .child(text),
                ),
        )
        .child(button)
        .into_any_element()
}

/// A neutral row with the last check's result. `None` before any check finished.
fn render_status_row(palette: &Palette, status: CheckStatus, version: &str) -> Option<AnyElement> {
    let (icon, text) = match status {
        CheckStatus::Idle => return None,
        CheckStatus::Checking => (
            gpui_kit::assets::IconName::RefreshCw,
            t!("update.checking").into_owned(),
        ),
        CheckStatus::UpToDate => (
            gpui_kit::assets::IconName::Check,
            t!("update.up_to_date", version = version).into_owned(),
        ),
        CheckStatus::Failed => (
            gpui_kit::assets::IconName::CircleAlert,
            t!("settings.about.status.failed").into_owned(),
        ),
    };
    Some(
        h_flex()
            .items_center()
            .gap(px(10.0))
            .px(px(14.0))
            .py(px(10.0))
            .rounded(px(RADIUS_LG))
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .text_color(palette.fg_muted)
            .child(Icon::new(icon).small())
            .child(text)
            .into_any_element(),
    )
}

/// Variant 3b: no updater in this build, so updates come from somewhere else.
fn render_distro_updates(
    palette: &Palette,
    mono_font_family: &SharedString,
    info: &AboutInfo,
) -> Vec<AnyElement> {
    let (title, text) = if cfg!(target_os = "linux") {
        (
            t!("settings.about.managed.title"),
            t!("settings.about.managed.text"),
        )
    } else {
        (
            t!("settings.about.unavailable.title"),
            t!("settings.about.unavailable.text"),
        )
    };
    let mut rows = vec![
        h_flex()
            .items_start()
            .gap(px(12.0))
            .p(px(14.0))
            .rounded(px(RADIUS_LG))
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .child(
                div().mt(px(1.0)).child(
                    Icon::new(gpui_kit::assets::IconName::Package)
                        .size(px(18.0))
                        .text_color(palette.fg_muted),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.0))
                    .child(div().font_weight(FontWeight::MEDIUM).child(title))
                    .child(
                        div()
                            .text_color(palette.fg_muted)
                            .line_height(relative(1.5))
                            .child(text),
                    )
                    .when(cfg!(target_os = "linux"), |this| {
                        this.child(
                            div().pt(px(6.0)).child(
                                SecondaryButton::new(
                                    "about-install-instructions",
                                    t!("settings.about.managed.link"),
                                )
                                .icon(Icon::new(gpui_kit::assets::IconName::ExternalLink))
                                .on_click(|_, _, cx| cx.open_url(crate::state::about::INSTALL_URL)),
                            ),
                        )
                    }),
            )
            .into_any_element(),
    ];
    if let Some(exe) = &info.exe {
        rows.push(
            h_flex()
                .items_center()
                .gap(px(8.0))
                .text_color(palette.fg_muted)
                .text_size(px(12.0))
                .child(Icon::new(gpui_kit::assets::IconName::Info).small())
                .child(t!("settings.about.installed_from"))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(mono_font_family.clone())
                        .text_color(palette.fg)
                        .child(exe.display().to_string()),
                )
                .into_any_element(),
        );
    }
    rows
}
