//! The Settings modal: opened from the title bar gear (`views/title_bar.rs`), the command palette
//! and `Ctrl ,` / `Cmd ,` (`main.rs`'s `bind_keys`,
//! `actions::OpenSettings`). Closed with the `x`, Escape, or a click on the dimmed backdrop, all
//! handled by gpui-component's own `Dialog`.
//!
//! Built with telaio's [`SettingsLayout`], framed by [`settings_dialog`]. Every control writes
//! straight to `AppState::settings` and calls [`AppView::apply_settings_live`], so it updates the
//! global `Theme` and persists to `settings.toml` immediately: there is no "Save" button.

use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::state;
use crate::state::about::IDENTITY;
use crate::state::locale::{self, LanguageChoice};
use crate::state::number::format_decimal;
use crate::state::settings::{Accent, LiveView, LogLevel, Settings, SettingsCategory, ThemeChoice};
use crate::theme::{self, Palette, PaletteExt};
use telaio_app::logging;

use super::components::{
    GhostButton, LabeledRow, NavCategory, NumberStepper, PreviewCard, SectionHeading,
    SettingsLayout, ThemeCard, settings_dialog,
};
use super::root::AppView;

mod about;
mod selects;

pub(crate) use selects::SettingsSelects;
use selects::{SelectHandles, SettingsSelect, render_select};

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
            settings_dialog(dialog, window, move |_window, cx| {
                render_settings_body(weak.clone(), cx)
            })
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

    /// Sets the UI font size.
    fn set_ui_font_size(&mut self, size: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.state.settings.ui_font_size = size;
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

/// Renders the dialog's whole content with telaio's [`SettingsLayout`]. `weak` lets every control
/// reach back into [`AppView`] to change a setting; `cx` is re-read fresh every time the window
/// repaints the dialog (the content builder gpui-component calls is a plain `Fn`, not a `FnMut`,
/// so nothing here is memoized across frames), which is what makes every change appear
/// immediately.
fn render_settings_body(weak: WeakEntity<AppView>, cx: &mut App) -> AnyElement {
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

    let path_label = state::settings::settings_path()
        .map(|path| state::format::short_path(&path, dirs::home_dir().as_deref()))
        .unwrap_or_else(|| t!("common.unknown").into_owned());
    let categories = SettingsCategory::ALL.map(|category| {
        let icon = match category {
            SettingsCategory::Appearance => gpui_kit::assets::IconName::Palette,
            SettingsCategory::Scanning => gpui_kit::assets::IconName::ScanSearch,
            SettingsCategory::Advanced => gpui_kit::assets::IconName::Wrench,
            SettingsCategory::About => gpui_kit::assets::IconName::Info,
        };
        NavCategory::new(icon, category.label())
    });
    let selected = SettingsCategory::ALL
        .iter()
        .position(|candidate| *candidate == category)
        .unwrap_or(0);
    let select_weak = weak.clone();

    let layout = SettingsLayout::new("settings")
        .title(t!("settings.title"))
        .categories(categories)
        .selected(selected)
        .on_select(move |index, _, cx| {
            if let Some(&category) = SettingsCategory::ALL.get(index) {
                let _ = select_weak.update(cx, |view, cx| view.set_settings_category(category, cx));
            }
        })
        .nav_footer(
            v_flex().child(t!("settings.saved_to")).child(
                div()
                    .font_family(mono_font_family.clone())
                    .child(path_label),
            ),
        )
        .heading(category.label())
        .sections(render_sections(
            weak.clone(),
            category,
            palette,
            &settings,
            &selects,
            &mono_font_family,
            about.as_ref(),
        ));
    let layout = if about.is_some() {
        layout.footer(about::footer_text(), div())
    } else {
        layout.footer(
            t!("settings.footer.applied"),
            GhostButton::new("settings-reset", t!("settings.footer.reset")).on_click(
                move |_, window, cx| {
                    let _ = weak.update(cx, |view, cx| view.reset_settings_to_defaults(window, cx));
                },
            ),
        )
    };
    layout.into_any_element()
}

/// The selected category's sections, which the layout splits with dividers.
fn render_sections(
    weak: WeakEntity<AppView>,
    category: SettingsCategory,
    palette: &Palette,
    settings: &Settings,
    selects: &SelectHandles,
    mono_font_family: &SharedString,
    about: Option<&about::AboutPane>,
) -> Vec<AnyElement> {
    match category {
        SettingsCategory::Appearance => vec![
            render_language_section(&selects.language),
            render_theme_section(weak.clone(), settings.theme),
            render_accent_section(weak.clone(), settings.accent),
            render_interface_section(weak, settings, selects),
            render_mono_section(selects, mono_font_family),
        ],
        SettingsCategory::Scanning => {
            vec![render_live_view_section(weak, palette, settings.live_view)]
        }
        SettingsCategory::Advanced => vec![render_logging_section(
            palette,
            &selects.log_level,
            mono_font_family,
        )],
        SettingsCategory::About => about
            .map(|pane| about::render_about(weak, palette, mono_font_family, pane))
            .unwrap_or_default(),
    }
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
fn render_language_section(select: &SettingsSelect) -> AnyElement {
    v_flex()
        .gap(px(14.0))
        .child(SectionHeading::new(t!("settings.language.title")))
        .child(
            LabeledRow::new(
                t!("settings.language.label"),
                render_select("settings-language", select, None),
            )
            .description(t!("settings.language.description")),
        )
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

fn render_theme_section(weak: WeakEntity<AppView>, current: ThemeChoice) -> AnyElement {
    v_flex()
        .gap(px(10.0))
        .child(
            SectionHeading::new(t!("settings.theme.title"))
                .description(t!("settings.theme.description")),
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

/// The "Interface" section: the UI font picker and its size stepper.
fn render_interface_section(
    weak: WeakEntity<AppView>,
    settings: &Settings,
    selects: &SelectHandles,
) -> AnyElement {
    v_flex()
        .gap(px(14.0))
        .child(SectionHeading::new(t!("settings.interface.title")))
        .child(
            LabeledRow::new(
                t!("settings.font"),
                render_select("settings-ui-font", &selects.ui_font, None),
            )
            .description(t!("settings.interface.font_description")),
        )
        .child(LabeledRow::new(
            t!("settings.font_size"),
            NumberStepper::new("settings-ui-size", settings.ui_font_size)
                .step(state::settings::UI_FONT_SIZE_STEP)
                .range(
                    *state::settings::UI_FONT_SIZE_RANGE.start(),
                    *state::settings::UI_FONT_SIZE_RANGE.end(),
                )
                .format(|value| {
                    format!(
                        "{} px",
                        format_decimal(f64::from(value), usize::from(value.fract() != 0.0))
                    )
                })
                .on_change(move |size, window, cx| {
                    let _ = weak.update(cx, |view, cx| view.set_ui_font_size(size, window, cx));
                }),
        ))
        .into_any_element()
}

/// The "Sizes and paths" section: the monospace font picker.
fn render_mono_section(selects: &SelectHandles, mono_font_family: &SharedString) -> AnyElement {
    v_flex()
        .gap(px(14.0))
        .child(SectionHeading::new(t!("settings.mono.title")))
        .child(
            LabeledRow::new(
                t!("settings.mono.font"),
                render_select(
                    "settings-mono-font",
                    &selects.mono_font,
                    Some(mono_font_family.clone()),
                ),
            )
            .description(t!("settings.mono.font_description")),
        )
        .into_any_element()
}

/// The "Accent" section: one card per accent palette.
fn render_accent_section(weak: WeakEntity<AppView>, current: Accent) -> AnyElement {
    v_flex()
        .gap(px(10.0))
        .child(
            SectionHeading::new(t!("settings.accent.title"))
                .description(t!("settings.accent.description")),
        )
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
        .child(
            SectionHeading::new(t!("settings.live_view.title"))
                .description(t!("settings.live_view.description")),
        )
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
    let log_dir = logging::log_dir(&IDENTITY);
    let path_label = logging::log_path(&IDENTITY)
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
        .child(SectionHeading::new(t!("settings.logging.title")))
        .child(
            LabeledRow::new(
                t!("settings.logging.level"),
                render_select("settings-log-level", select, None),
            )
            .description(t!("settings.logging.level_description")),
        )
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
                    variable = logging::env_var(&IDENTITY),
                )),
        )
        .into_any_element()
}
