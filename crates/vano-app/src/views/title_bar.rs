//! The title bar: the "V" logo, "Vano", the scan target switcher, Rescan (on the results) and
//! the settings gear. The window controls are drawn by gpui-kit's `TitleBar` itself.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::state::target::{Target, TargetKind};
use crate::theme::metrics::{ENV_PILL_HEIGHT, RADIUS_MD, TITLE_BAR_HEIGHT};
use crate::theme::{Palette, PaletteExt};

use super::root::{AppView, Screen};

impl AppView {
    /// Renders the whole title bar's content.
    pub(crate) fn render_title_bar(
        &mut self,
        weak: WeakEntity<Self>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = cx.palette();
        let palette = &*palette;
        let rescan = (self.screen == Screen::Results).then(|| {
            let weak = weak.clone();
            no_drag(
                Button::new("rescan")
                    .ghost()
                    .h(px(ENV_PILL_HEIGHT))
                    .px_2()
                    .rounded(px(RADIUS_MD - 2.0))
                    .text_color(palette.fg_muted)
                    .icon(Icon::new(Lucide::RefreshCw).small())
                    .label(t!("title.rescan"))
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            if let Some(results) = view.results.as_ref() {
                                view.target = results.target.clone();
                            }
                            view.start_scan(cx);
                        });
                    }),
            )
        });
        TitleBar::new()
            .h(px(TITLE_BAR_HEIGHT))
            .child(
                h_flex()
                    .size_full()
                    .items_center()
                    .gap_2()
                    .child(no_drag(render_logo(palette, weak.clone())))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Vano"))
                    .child(div().w(px(1.0)).h(px(16.0)).mx(px(4.0)).bg(palette.border))
                    .child(no_drag(self.render_target_switcher(weak.clone(), palette)))
                    .child(div().flex_1())
                    .children(rescan)
                    .child(no_drag(render_settings_gear(weak, palette))),
            )
            .into_any_element()
    }

    /// The target switcher: the current target and a menu to pick another one.
    fn render_target_switcher(&self, weak: WeakEntity<Self>, palette: &Palette) -> AnyElement {
        let target = match self.screen {
            Screen::Scanning => self.scan.as_ref().map(|s| s.target.clone()),
            Screen::Results => self.results.as_ref().map(|r| r.target.clone()),
            Screen::Home => None,
        }
        .unwrap_or_else(|| self.target.clone());
        let icon = target_icon(target.kind);
        let busy = self.screen == Screen::Scanning || self.trashing;
        Button::new("target-switcher")
            .ghost()
            .h(px(ENV_PILL_HEIGHT))
            .px_2()
            .rounded(px(RADIUS_MD - 2.0))
            .text_color(palette.fg_muted)
            .icon(Icon::new(icon).small())
            .label(target.label())
            .dropdown_caret(true)
            .disabled(busy)
            .dropdown_menu(move |mut menu, _, _| {
                let mut choices = Vec::new();
                if let Some(home) = Target::home() {
                    choices.push(home);
                }
                choices.push(Target::disk());
                for choice in choices {
                    let weak = weak.clone();
                    let label = choice.label();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        let choice = choice.clone();
                        let _ = weak.update(cx, |view, cx| {
                            view.target = choice;
                            view.screen = Screen::Home;
                            cx.notify();
                        });
                    }));
                }
                let weak = weak.clone();
                menu.separator()
                    .item(
                        PopupMenuItem::new(t!("home.choose")).on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |view, cx| {
                                view.screen = Screen::Home;
                                view.choose_folder(cx);
                            });
                        }),
                    )
            })
            .into_any_element()
    }
}

/// The icon of a target kind.
pub(crate) fn target_icon(kind: TargetKind) -> Lucide {
    match kind {
        TargetKind::Home => Lucide::House,
        TargetKind::Disk => Lucide::HardDrive,
        TargetKind::Folder => Lucide::Folder,
    }
}

/// Keeps the title bar's drag area out of an interactive control (Windows hit testing).
fn no_drag(control: impl IntoElement) -> impl IntoElement {
    div().occlude().child(control)
}

/// The "V" logo square; a click goes back home.
fn render_logo(palette: &Palette, weak: WeakEntity<AppView>) -> impl IntoElement {
    div()
        .id("logo")
        .size(px(18.0))
        .rounded(px(5.0))
        .bg(palette.accent)
        .text_color(palette.accent_fg)
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child("V")
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |view, cx| view.go_home(cx));
        })
}

/// The settings gear.
fn render_settings_gear(weak: WeakEntity<AppView>, palette: &Palette) -> impl IntoElement {
    div()
        .id("title-bar-settings-gear")
        .size(px(ENV_PILL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .rounded(px(RADIUS_MD - 2.0))
        .text_color(palette.fg_muted)
        .hover(|style| style.bg(palette.hover))
        .active(|style| style.bg(palette.pressed))
        .child(Icon::new(Lucide::Settings).with_size(px(16.0)))
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |view, cx| view.open_settings(window, cx));
        })
}
