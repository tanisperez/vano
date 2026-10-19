//! The Settings modal: opened from the title bar gear (`views/title_bar.rs`), the command palette
//! and `Ctrl ,` / `Cmd ,` (`main.rs`'s `bind_keys`,
//! `actions::OpenSettings`). Closed with the `x`, Escape, or a click on the dimmed backdrop, all
//! handled by gpui-component's own `Dialog`.
//!
//! Built as a `Dialog` with fully custom content: a left nav ("Settings" title, the single
//! "Appearance" item) and a right column (header, scrollable body, footer), styled per
//! `docs/design-system.md`. Every control writes straight to
//! `AppState::settings` and calls [`AppView::apply_settings_live`], so it updates the global
//! `Theme` and persists to `settings.toml` immediately: there is no "Save" button.

use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::logging;
use crate::state;
use crate::state::locale::{self, LanguageChoice};
use crate::state::number::format_decimal;
use crate::state::settings::{Accent, LiveView, LogLevel, Settings, SettingsCategory, ThemeChoice};
use crate::theme::metrics::{CONTROL_HEIGHT, RADIUS_MD};
use crate::theme::{self, Palette, PaletteExt};

use super::components::{
    GhostButton, IconButton, MODAL_INNER_RADIUS, ModalPlacement, PreviewCard, ThemeCard,
    framed_dialog,
};
use super::root::AppView;

mod about;
mod selects;

pub(crate) use selects::SettingsSelects;
use selects::{SelectHandles, SettingsSelect, render_select};

/// Width of the modal itself.
const MODAL_WIDTH: f32 = 800.0;
/// Height of the modal itself.
const MODAL_HEIGHT: f32 = 720.0;
/// Width of the modal's left nav column.
const NAV_WIDTH: f32 = 188.0;
/// Height of the right column's header and footer strips.
const HEADER_FOOTER_HEIGHT: f32 = 52.0;
/// Width of a font size stepper's numeric readout.
const STEPPER_VALUE_WIDTH: f32 = 64.0;

impl AppView {
    /// Opens the Settings modal. A no-op when one is already open, so the gear and `Ctrl ,` / `Cmd
    /// ,` never stack a second modal on top (GitHub #18).
    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let installed = cx.text_system().all_font_names();
        self.settings_selects = Some(SettingsSelects::new(
            &self.state.settings,
            &installed,
            window,
            cx,
        ));
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, _cx| {
            let weak = weak.clone();
            // Centered by the shared frame, which keeps the requested height on any window tall
            // enough for it and only shrinks (via the scrollable body) on one that is not.
            framed_dialog(
                dialog,
                MODAL_WIDTH,
                Some(MODAL_HEIGHT),
                ModalPlacement::Centered,
                window,
                move |window, cx| {
                    let scroll_handle = window
                        .use_keyed_state("settings-body-scroll", cx, |_, _| ScrollHandle::default())
                        .read(cx)
                        .clone();
                    render_settings_body(weak.clone(), scroll_handle, cx)
                },
            )
        });
    }

    /// Applies `self.state.settings` to the live `Theme` (colors, fonts and sizes) and to the theme
    /// mode, refreshes `window`, and persists the file. Called after every control in the Settings
    /// view changes a value; there is no separate "Save" action.
    fn apply_settings_live(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        theme::apply_settings(cx, &self.state.settings);
        match self.state.settings.theme {
            ThemeChoice::System => Theme::sync_system_appearance(Some(window), cx),
            ThemeChoice::Light => Theme::change(ThemeMode::Light, Some(window), cx),
            ThemeChoice::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
        }
        log::info!("settings changed: {:?}", self.state.settings);
        state::settings::save_settings(&self.state.settings);
        self.sync_settings_selects(false, window, cx);
        cx.notify();
    }

    /// Picks the Settings category shown in the right column.
    fn set_settings_category(&mut self, category: SettingsCategory, cx: &mut Context<Self>) {
        self.settings_category = category;
        cx.notify();
    }

    /// Picks the UI language: applies it process wide, lets every view re-apply the strings it
    /// cached outside render ([`Self::relocalize`]), and repaints every window.
    fn set_language(
        &mut self,
        choice: LanguageChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.settings.language = choice;
        self.apply_language(window, cx);
        self.apply_settings_live(window, cx);
    }

    /// Applies `self.state.settings.language` live, without persisting it.
    fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = locale::resolve(self.state.settings.language);
        locale::apply(language);
        log::info!("language changed to {}", language.code());
        self.relocalize(window, cx);
        self.sync_settings_selects(true, window, cx);
        cx.refresh_windows();
    }

    /// Shows the current settings in the Settings selects, when the modal has been opened.
    fn sync_settings_selects(
        &mut self,
        relabel: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(selects) = self.settings_selects.take() {
            selects.sync(&self.state.settings, relabel, window, cx);
            self.settings_selects = Some(selects);
        }
    }

    /// Picks the log level, applied to the logger at once.
    fn set_log_level(&mut self, level: LogLevel, window: &mut Window, cx: &mut Context<Self>) {
        self.state.settings.log_level = level;
        logging::set_level(level.to_filter());
        self.apply_settings_live(window, cx);
    }

    /// Turns the automatic update check at startup on or off.
    fn set_check_updates(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.state.settings.check_updates = enabled;
        self.apply_settings_live(window, cx);
    }

    /// Picks a theme card, in Settings or in the welcome tab.
    pub(crate) fn set_theme_choice(
        &mut self,
        choice: ThemeChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.settings.theme = choice;
        self.apply_settings_live(window, cx);
    }

    /// Picks the UI font family.
    fn set_ui_font(&mut self, font: String, window: &mut Window, cx: &mut Context<Self>) {
        self.state.settings.ui_font = font;
        self.apply_settings_live(window, cx);
    }

    /// Steps the UI font size by `delta`.
    fn step_ui_font_size(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.state.settings.ui_font_size = state::settings::step_size(
            self.state.settings.ui_font_size,
            delta,
            state::settings::UI_FONT_SIZE_RANGE,
        );
        self.apply_settings_live(window, cx);
    }

    /// Picks the monospace font family.
    fn set_mono_font(&mut self, font: String, window: &mut Window, cx: &mut Context<Self>) {
        self.state.settings.mono_font = font;
        self.apply_settings_live(window, cx);
    }

    /// Picks the accent palette.
    pub(crate) fn set_accent(
        &mut self,
        accent: Accent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.settings.accent = accent;
        self.apply_settings_live(window, cx);
    }

    /// Picks the default live view, and shows it now if a scan runs.
    pub(crate) fn set_live_view(
        &mut self,
        view: LiveView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.settings.live_view = view;
        self.live_view = view;
        self.apply_settings_live(window, cx);
    }

    /// Restores `Settings::default()` ("Reset to
    /// defaults").
    fn reset_settings_to_defaults(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.settings = Settings::default();
        logging::set_level(self.state.settings.log_level.to_filter());
        self.apply_language(window, cx);
        self.apply_settings_live(window, cx);
    }
}

/// Renders the dialog's whole content: the left nav plus the right column. `weak` lets every
/// control reach back into [`AppView`] to change a setting; `cx` is re-read fresh every time the
/// window repaints the dialog (the content builder gpui-component calls is a plain `Fn`, not a
/// `FnMut`, so nothing here is memoized across frames), which is what makes every change appear
/// immediately.
fn render_settings_body(
    weak: WeakEntity<AppView>,
    scroll_handle: ScrollHandle,
    cx: &mut App,
) -> AnyElement {
    let Some(view) = weak.upgrade() else {
        return div().into_any_element();
    };
    let settings = view.read(cx).state.settings.clone();
    let category = view.read(cx).settings_category;
    let about =
        (category == SettingsCategory::About).then(|| about::AboutPane::capture(view.read(cx)));
    let palette = cx.palette();
    let palette = &*palette;
    let mono_font_family = cx.theme().mono_font_family.clone();
    let Some(selects) = view
        .read(cx)
        .settings_selects
        .as_ref()
        .map(|selects| selects.handles.clone())
    else {
        return div().into_any_element();
    };
    // Inside the modal card's 1 px border.
    let modal_radius = px(MODAL_INNER_RADIUS);

    h_flex()
        .size_full()
        .child(render_nav(
            weak.clone(),
            palette,
            &mono_font_family,
            category,
            modal_radius,
        ))
        .child(render_right_column(
            weak,
            scroll_handle,
            category,
            palette,
            &settings,
            &selects,
            &mono_font_family,
            about.as_ref(),
        ))
        .into_any_element()
}

/// The left nav: "Settings" title, one item per category, and the "Saved to" footer. Its left
/// corners repeat the modal card's inner `radius`: the dialog's `overflow_hidden` clips to its
/// bounds but not to rounded corners, so the nav's background would otherwise paint square ones.
fn render_nav(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    mono_font_family: &SharedString,
    current: SettingsCategory,
    radius: Pixels,
) -> AnyElement {
    let path_label = state::settings::settings_path()
        .map(|path| state::format::short_path(&path, dirs::home_dir().as_deref()))
        .unwrap_or_else(|| t!("common.unknown").into_owned());

    v_flex()
        .flex_none()
        .w(px(NAV_WIDTH))
        .h_full()
        .rounded_l(radius)
        .bg(palette.surface)
        .border_r_1()
        .border_color(palette.border)
        .px(px(10.0))
        .py(px(14.0))
        .gap(px(2.0))
        .child(
            div()
                .px(px(8.0))
                .pt(px(2.0))
                .pb(px(10.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0))
                .child(t!("settings.title")),
        )
        .children(
            SettingsCategory::ALL.into_iter().map(|category| {
                render_nav_item(weak.clone(), palette, category, category == current)
            }),
        )
        .child(div().flex_1())
        .child(
            v_flex()
                .p(px(8.0))
                .text_color(palette.fg_subtle)
                .text_size(px(11.5))
                .line_height(relative(1.5))
                .child(t!("settings.saved_to"))
                .child(
                    div()
                        .font_family(mono_font_family.clone())
                        .child(path_label),
                ),
        )
        .into_any_element()
}

/// One nav item: highlighted when `selected`, hover and pressed backgrounds otherwise.
fn render_nav_item(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    category: SettingsCategory,
    selected: bool,
) -> AnyElement {
    let (id, icon) = match category {
        SettingsCategory::Appearance => ("settings-nav-appearance", Icon::new(IconName::Palette)),
        SettingsCategory::Scanning => (
            "settings-nav-scanning",
            Icon::new(gpui_kit::assets::IconName::ScanSearch),
        ),
        SettingsCategory::Advanced => (
            "settings-nav-advanced",
            Icon::new(gpui_kit::assets::IconName::Wrench),
        ),
        SettingsCategory::About => (
            "settings-nav-about",
            Icon::new(gpui_kit::assets::IconName::Info),
        ),
    };
    let item = h_flex()
        .id(id)
        .h(px(28.0))
        .items_center()
        .gap_2()
        .px(px(8.0))
        .rounded(px(RADIUS_MD - 1.0))
        .cursor_pointer();
    let item = if selected {
        item.bg(palette.accent_subtle)
            .text_color(palette.accent_text)
            .font_weight(FontWeight::MEDIUM)
    } else {
        let (hover, pressed) = (palette.hover, palette.pressed);
        item.hover(move |style| style.bg(hover))
            .active(move |style| style.bg(pressed))
    };
    item.child(icon.small())
        .child(category.label())
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |view, cx| view.set_settings_category(category, cx));
        })
        .into_any_element()
}

/// The right column: header, scrollable body (the selected category's sections) and footer.
/// The body's scrollbar is always shown (while the content overflows), unlike the theme's
/// default that only shows it while scrolling: on a short window nothing else tells the user
/// that more settings lie below.
#[allow(clippy::too_many_arguments)]
fn render_right_column(
    weak: WeakEntity<AppView>,
    scroll_handle: ScrollHandle,
    category: SettingsCategory,
    palette: &Palette,
    settings: &Settings,
    selects: &SelectHandles,
    mono_font_family: &SharedString,
    about: Option<&about::AboutPane>,
) -> AnyElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .child(render_header(palette, category))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(render_body(
                    weak.clone(),
                    &scroll_handle,
                    category,
                    palette,
                    settings,
                    selects,
                    mono_font_family,
                    about,
                ))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(settings_scrollbar(&scroll_handle, palette)),
                ),
        )
        .child(if about.is_some() {
            about::render_about_footer(palette)
        } else {
            render_footer(weak, palette)
        })
        .into_any_element()
}

/// The body's always-shown scrollbar. Its track is transparent with no border, so only the thumb
/// stands out: the theme's track color is the window's `bg`, which shows as a darker strip on the
/// modal, and an opaque one would square off the modal's rounded corner where the body reaches
/// the bottom (Keyboard shortcuts).
pub(super) fn settings_scrollbar(scroll_handle: &ScrollHandle, palette: &Palette) -> Scrollbar {
    let track = palette.overlay.opacity(0.0);
    let border = track;
    Scrollbar::vertical(scroll_handle)
        .mode(ScrollbarMode::Always)
        .styles(|styles| {
            styles
                .track(|style| style.bg(track).border_color(border))
                .track_hover(|style| style.bg(track).border_color(border))
                .track_active(|style| style.bg(track).border_color(border))
        })
}

/// The scrolled part of the right column: the selected category's sections.
#[allow(clippy::too_many_arguments)]
fn render_body(
    weak: WeakEntity<AppView>,
    scroll_handle: &ScrollHandle,
    category: SettingsCategory,
    palette: &Palette,
    settings: &Settings,
    selects: &SelectHandles,
    mono_font_family: &SharedString,
    about: Option<&about::AboutPane>,
) -> AnyElement {
    v_flex()
        .id("settings-body")
        .size_full()
        .overflow_y_scroll()
        .track_scroll(scroll_handle)
        .p(px(20.0))
        .gap(px(22.0))
        .children(match category {
            SettingsCategory::Appearance => vec![
                render_language_section(palette, &selects.language),
                divider(palette),
                render_theme_section(weak.clone(), palette, settings.theme),
                divider(palette),
                render_accent_section(weak.clone(), palette, settings.accent),
                divider(palette),
                render_interface_section(
                    weak.clone(),
                    palette,
                    settings,
                    selects,
                    mono_font_family,
                ),
                divider(palette),
                render_mono_section(palette, selects, mono_font_family),
            ],
            SettingsCategory::Scanning => vec![render_live_view_section(
                weak.clone(),
                palette,
                settings.live_view,
            )],
            SettingsCategory::Advanced => vec![render_logging_section(
                palette,
                &selects.log_level,
                mono_font_family,
            )],
            SettingsCategory::About => about
                .map(|pane| about::render_about(weak.clone(), palette, mono_font_family, pane))
                .unwrap_or_default(),
        })
        .into_any_element()
}

/// A full-width 1px divider between sections. `flex_none`, or the scrolled body, whose content
/// overflows it, shrinks this contentless line to nothing.
fn divider(palette: &Palette) -> AnyElement {
    div()
        .flex_none()
        .h(px(1.0))
        .bg(palette.border)
        .into_any_element()
}

/// The right column's header: the category title and the close button.
fn render_header(palette: &Palette, category: SettingsCategory) -> AnyElement {
    h_flex()
        .h(px(HEADER_FOOTER_HEIGHT))
        .flex_none()
        .items_center()
        .justify_between()
        .px(px(20.0))
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(category.label()),
        )
        .child(
            IconButton::new("settings-close", IconName::Close)
                .large()
                .tooltip(t!("common.close"))
                .on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                }),
        )
        .into_any_element()
}

/// The right column's footer: "Changes apply immediately." and "Reset to defaults".
fn render_footer(weak: WeakEntity<AppView>, palette: &Palette) -> AnyElement {
    h_flex()
        .h(px(HEADER_FOOTER_HEIGHT))
        .flex_none()
        .items_center()
        .justify_between()
        .px(px(20.0))
        .border_t_1()
        .border_color(palette.border)
        .child(
            div()
                .text_color(palette.fg_subtle)
                .text_size(px(12.0))
                .child(t!("settings.footer.applied")),
        )
        .child(
            GhostButton::new("settings-reset", t!("settings.footer.reset")).on_click(
                move |_, window, cx| {
                    let _ = weak.update(cx, |view, cx| view.reset_settings_to_defaults(window, cx));
                },
            ),
        )
        .into_any_element()
}

/// The dropdown label of `choice`: a language's name in itself, or "Automatic" with the language
/// the system resolves to.
fn language_choice_label(choice: LanguageChoice) -> String {
    match choice.language() {
        Some(language) => language.native_name().to_string(),
        None => t!(
            "settings.language.auto",
            language = locale::resolve(LanguageChoice::Auto).native_name()
        )
        .into_owned(),
    }
}

/// The "Language" section: a dropdown with Automatic and the four languages.
fn render_language_section(palette: &Palette, select: &SettingsSelect) -> AnyElement {
    let control = render_select("settings-language", select, palette, None);

    v_flex()
        .gap(px(14.0))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .child(t!("settings.language.title")),
        )
        .child(labeled_row(
            palette,
            t!("settings.language.label"),
            Some(t!("settings.language.description")),
            control,
        ))
        .into_any_element()
}

/// The "Theme" section: the description and the three theme cards.
/// The [`ThemeCard`] for `choice`, selected when it is `current`, that picks it on click. Shared by
/// Settings and the welcome tab; `id_prefix` keeps their element ids apart.
pub(crate) fn theme_card(
    weak: WeakEntity<AppView>,
    id_prefix: &str,
    choice: ThemeChoice,
    current: ThemeChoice,
) -> ThemeCard {
    let id = match choice {
        ThemeChoice::System => "system",
        ThemeChoice::Light => "light",
        ThemeChoice::Dark => "dark",
    };
    ThemeCard::new(
        SharedString::from(format!("{id_prefix}-{id}")),
        choice.into(),
    )
    .selected(choice == current)
    .on_click(move |_, window, cx| {
        let _ = weak.update(cx, |view, cx| view.set_theme_choice(choice, window, cx));
    })
}

fn render_theme_section(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    current: ThemeChoice,
) -> AnyElement {
    v_flex()
        .gap(px(10.0))
        .child(
            v_flex()
                .gap(px(2.0))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child(t!("settings.theme.title")),
                )
                .child(
                    div()
                        .text_color(palette.fg_muted)
                        .text_size(px(12.0))
                        .child(t!("settings.theme.description")),
                ),
        )
        .child(
            h_flex().gap(px(12.0)).children(
                [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark]
                    .into_iter()
                    .map(|choice| theme_card(weak.clone(), "settings-theme", choice, current)),
            ),
        )
        .into_any_element()
}

/// The "Interface" section: the UI font picker, its size stepper and the welcome tab switch.
fn render_interface_section(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    settings: &Settings,
    selects: &SelectHandles,
    mono_font_family: &SharedString,
) -> AnyElement {
    v_flex()
        .gap(px(14.0))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .child(t!("settings.interface.title")),
        )
        .child(labeled_row(
            palette,
            t!("settings.font"),
            Some(t!("settings.interface.font_description")),
            render_select("settings-ui-font", &selects.ui_font, palette, None),
        ))
        .child(labeled_row(
            palette,
            t!("settings.font_size"),
            None::<SharedString>,
            render_size_stepper(
                "settings-ui-size",
                weak.clone(),
                palette,
                settings.ui_font_size,
                mono_font_family.clone(),
                state::settings::UI_FONT_SIZE_STEP,
                |view, delta, window, cx| view.step_ui_font_size(delta, window, cx),
            ),
        ))
        .into_any_element()
}

/// The "Sizes and paths" section: the monospace font picker.
fn render_mono_section(
    palette: &Palette,
    selects: &SelectHandles,
    mono_font_family: &SharedString,
) -> AnyElement {
    v_flex()
        .gap(px(14.0))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .child(t!("settings.mono.title")),
        )
        .child(labeled_row(
            palette,
            t!("settings.mono.font"),
            Some(t!("settings.mono.font_description")),
            render_select(
                "settings-mono-font",
                &selects.mono_font,
                palette,
                Some(mono_font_family.clone()),
            ),
        ))
        .into_any_element()
}

/// A section title with its one line description.
fn section_heading(palette: &Palette, title: SharedString, description: SharedString) -> Div {
    v_flex()
        .gap(px(2.0))
        .child(div().font_weight(FontWeight::MEDIUM).child(title))
        .child(
            div()
                .text_color(palette.fg_muted)
                .text_size(px(12.0))
                .child(description),
        )
}

/// The "Accent" section: one card per accent palette.
fn render_accent_section(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    current: Accent,
) -> AnyElement {
    v_flex()
        .gap(px(10.0))
        .child(section_heading(
            palette,
            t!("settings.accent.title").into(),
            t!("settings.accent.description").into(),
        ))
        .child(
            h_flex()
                .gap(px(12.0))
                .children(Accent::ALL.into_iter().map(|accent| {
                    let weak = weak.clone();
                    PreviewCard::accent(
                        SharedString::from(format!("settings-accent-{accent:?}")),
                        accent,
                    )
                    .selected(accent == current)
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |view, cx| view.set_accent(accent, window, cx));
                    })
                })),
        )
        .into_any_element()
}

/// The "Live view" section of Scanning: what the left panel shows while a scan runs.
fn render_live_view_section(
    weak: WeakEntity<AppView>,
    palette: &Palette,
    current: LiveView,
) -> AnyElement {
    v_flex()
        .gap(px(10.0))
        .child(section_heading(
            palette,
            t!("settings.live_view.title").into(),
            t!("settings.live_view.description").into(),
        ))
        .child(
            h_flex()
                .gap(px(12.0))
                .children(LiveView::ALL.into_iter().map(|view_kind| {
                    let weak = weak.clone();
                    PreviewCard::live_view(
                        SharedString::from(format!("settings-live-{view_kind:?}")),
                        view_kind,
                    )
                    .selected(view_kind == current)
                    .on_click(move |_, window, cx| {
                        let _ =
                            weak.update(cx, |view, cx| view.set_live_view(view_kind, window, cx));
                    })
                })),
        )
        .child(
            div()
                .text_color(palette.fg_muted)
                .text_size(px(12.5))
                .child(current.description()),
        )
        .into_any_element()
}

/// The "Logging" section of the Advanced pane: the log level and where the log file is.
fn render_logging_section(
    palette: &Palette,
    select: &SettingsSelect,
    mono_font_family: &SharedString,
) -> AnyElement {
    let control = render_select("settings-log-level", select, palette, None);

    let log_dir = logging::log_dir();
    let path_label = logging::log_path()
        .map(|path| state::format::short_path(&path, dirs::home_dir().as_deref()))
        .unwrap_or_else(|| t!("common.unknown").into_owned());
    let open_folder = GhostButton::new(
        "settings-open-log-folder",
        t!("settings.logging.open_folder"),
    )
    .icon(Icon::new(gpui_kit::assets::IconName::FolderOpen))
    .disabled(log_dir.is_none())
    .on_click(move |_, _, cx| {
        if let Some(dir) = &log_dir {
            cx.open_with_system(dir);
        }
    });

    v_flex()
        .gap(px(14.0))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .child(t!("settings.logging.title")),
        )
        .child(labeled_row(
            palette,
            t!("settings.logging.level"),
            Some(t!("settings.logging.level_description")),
            control,
        ))
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap(px(16.0))
                .child(
                    v_flex()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(div().child(t!("settings.logging.file")))
                        .child(
                            div()
                                .text_color(palette.fg_muted)
                                .text_size(px(12.0))
                                .font_family(mono_font_family.clone())
                                .child(path_label),
                        ),
                )
                .child(open_folder),
        )
        .child(
            div()
                .text_color(palette.fg_subtle)
                .text_size(px(12.0))
                .child(t!(
                    "settings.logging.rotation",
                    size = logging::MAX_FILE_SIZE / (1024 * 1024),
                    files = logging::MAX_FILES,
                    variable = logging::ENV_VAR,
                )),
        )
        .into_any_element()
}

/// A label (with an optional muted description) on the left, an arbitrary control on the right.
fn labeled_row(
    palette: &Palette,
    label: impl Into<SharedString>,
    description: Option<impl Into<SharedString>>,
    control: AnyElement,
) -> AnyElement {
    let label: SharedString = label.into();
    let leading = match description {
        Some(text) => v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.0))
            .child(div().child(label))
            .child(
                div()
                    .text_color(palette.fg_muted)
                    .text_size(px(12.0))
                    .child(text.into()),
            )
            .into_any_element(),
        None => div().flex_1().min_w_0().child(label).into_any_element(),
    };

    // The label column shrinks and wraps a long description instead of pushing the control
    // past the right edge, which kept its natural width and overflowed the modal.
    h_flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .child(leading)
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// A `-`/`N px`/`+` stepper.
fn render_size_stepper(
    id_prefix: &'static str,
    weak: WeakEntity<AppView>,
    palette: &Palette,
    value: f32,
    mono_font_family: SharedString,
    step: f32,
    on_step: impl Fn(&mut AppView, f32, &mut Window, &mut Context<AppView>) + Clone + 'static,
) -> AnyElement {
    let minus_weak = weak.clone();
    let minus_on_step = on_step.clone();
    let plus_on_step = on_step;

    h_flex()
        .items_center()
        .gap(px(8.0))
        .child(stepper_button(
            format!("{id_prefix}-minus"),
            palette,
            IconName::Minus,
            move |window, cx| {
                let _ = minus_weak.update(cx, |view, cx| minus_on_step(view, -step, window, cx));
            },
        ))
        .child(
            div()
                .w(px(STEPPER_VALUE_WIDTH))
                .text_center()
                .font_family(mono_font_family)
                .child(format!(
                    "{} px",
                    format_decimal(f64::from(value), usize::from(value.fract() != 0.0))
                )),
        )
        .child(stepper_button(
            format!("{id_prefix}-plus"),
            palette,
            IconName::Plus,
            move |window, cx| {
                let _ = weak.update(cx, |view, cx| plus_on_step(view, step, window, cx));
            },
        ))
        .into_any_element()
}

/// One `-`/`+` square button of a [`render_size_stepper`].
fn stepper_button(
    id: impl Into<ElementId>,
    palette: &Palette,
    icon: IconName,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .rounded(px(RADIUS_MD - 1.0))
        .border_1()
        .border_color(palette.border_strong)
        .bg(palette.raised)
        .hover(|style| style.bg(palette.hover))
        .active(|style| style.bg(palette.pressed))
        .child(Icon::new(icon).small())
        .on_click(move |_, window, cx| on_click(window, cx))
}
