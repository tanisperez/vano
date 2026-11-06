//! The Settings selects: language, the two font families and the log level. Every one is telaio's
//! [`SearchableSelect`], with a search box for the long font lists.
//!
//! A select needs a state entity, so they are created when the dialog opens
//! ([`AppView::open_settings`]) and kept in [`AppView`]. Each option has a stable string id (the
//! family name, the position in the enum's `ALL`) apart from its translated label.

use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use telaio_ui::components::{
    SearchableSelect, SelectHandle, SelectOption, on_pick, pick, select_state, set_options,
};

use crate::state;
use crate::state::locale::LanguageChoice;
use crate::state::settings::{LogLevel, Settings};

use super::AppView;

/// The entity behind one Settings select.
pub(super) type SettingsSelect = SelectHandle;

/// The four selects, cheap to clone into the dialog's render.
#[derive(Clone)]
pub(super) struct SelectHandles {
    pub(super) language: SettingsSelect,
    pub(super) ui_font: SettingsSelect,
    pub(super) mono_font: SettingsSelect,
    pub(super) log_level: SettingsSelect,
}

/// The selects plus the subscriptions that route their choices to [`AppView`].
pub(crate) struct SettingsSelects {
    pub(super) handles: SelectHandles,
    /// The installed font families, read when the dialog opens, to rebuild the font lists when
    /// the language changes their texts.
    installed: Vec<String>,
    _subscriptions: Vec<Subscription>,
}

/// The id of the `index`th value of an enum's `ALL`.
fn index_id(index: usize) -> SharedString {
    index.to_string().into()
}

/// The enum value an [`index_id`] stands for.
fn from_index_id<T: Copy>(all: &[T], id: &str) -> Option<T> {
    id.parse::<usize>()
        .ok()
        .and_then(|index| all.get(index).copied())
}

fn language_options() -> Vec<SelectOption> {
    LanguageChoice::ALL
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            SelectOption::new(index_id(index), super::language_choice_label(*choice))
        })
        .collect()
}

fn log_level_options() -> Vec<SelectOption> {
    LogLevel::ALL
        .iter()
        .enumerate()
        .map(|(index, level)| SelectOption::new(index_id(index), level.label()))
        .collect()
}

/// The installed families with the bundled one first (labelled as such), plus the current one when
/// it is no longer installed, so the box never shows empty.
fn font_options(bundled: &str, installed: &[String], current: &str) -> Vec<SelectOption> {
    let mut names = state::settings::font_options(bundled, installed);
    if !names.iter().any(|name| name == current) {
        names.push(current.to_string());
    }
    names
        .into_iter()
        .map(|name| {
            let label = if name == bundled {
                t!("settings.font_bundled", name = name.as_str()).into_owned()
            } else {
                name.clone()
            };
            SelectOption::new(name, label)
        })
        .collect()
}

/// Creates one select showing `selected`, and routes its confirmed choices to `handler`.
fn create_select(
    options: Vec<SelectOption>,
    selected: &str,
    searchable: bool,
    handler: impl Fn(&mut AppView, &str, &mut Window, &mut Context<AppView>) + 'static,
    subscriptions: &mut Vec<Subscription>,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> SettingsSelect {
    let select = select_state(options, Some(selected), searchable, window, cx);
    subscriptions.push(on_pick(&select, window, cx, handler));
    select
}

impl SettingsSelects {
    /// Builds the selects from the current settings and the installed font families.
    pub(super) fn new(
        settings: &Settings,
        installed: &[String],
        window: &mut Window,
        cx: &mut Context<AppView>,
    ) -> Self {
        let mut subscriptions = Vec::new();
        let language = create_select(
            language_options(),
            &index_id(language_index(settings.language)),
            false,
            |view, id, window, cx| {
                if let Some(choice) = from_index_id(&LanguageChoice::ALL, id) {
                    view.set_language(choice, window, cx);
                }
            },
            &mut subscriptions,
            window,
            cx,
        );
        let ui_font = create_select(
            font_options("Geist", installed, &settings.ui_font),
            &settings.ui_font,
            true,
            |view, id, window, cx| view.set_ui_font(id.to_string(), window, cx),
            &mut subscriptions,
            window,
            cx,
        );
        let mono_font = create_select(
            font_options("Geist Mono", installed, &settings.mono_font),
            &settings.mono_font,
            true,
            |view, id, window, cx| view.set_mono_font(id.to_string(), window, cx),
            &mut subscriptions,
            window,
            cx,
        );
        let log_level = create_select(
            log_level_options(),
            &index_id(log_level_index(settings.log_level)),
            false,
            |view, id, window, cx| {
                if let Some(level) = from_index_id(&LogLevel::ALL, id) {
                    view.set_log_level(level, window, cx);
                }
            },
            &mut subscriptions,
            window,
            cx,
        );
        Self {
            handles: SelectHandles {
                language,
                ui_font,
                mono_font,
                log_level,
            },
            installed: installed.to_vec(),
            _subscriptions: subscriptions,
        }
    }

    /// Shows `settings` in every select, for a change made outside them (Reset to defaults).
    /// With `relabel`, the translated labels are rebuilt too (the language changed), the font lists
    /// included, whose bundled family is labelled as such.
    pub(super) fn sync(
        &self,
        settings: &Settings,
        relabel: bool,
        window: &mut Window,
        cx: &mut Context<AppView>,
    ) {
        let handles = &self.handles;
        if relabel {
            set_options(&handles.language, language_options(), false, window, cx);
            set_options(&handles.log_level, log_level_options(), false, window, cx);
            set_options(
                &handles.ui_font,
                font_options("Geist", &self.installed, &settings.ui_font),
                true,
                window,
                cx,
            );
            set_options(
                &handles.mono_font,
                font_options("Geist Mono", &self.installed, &settings.mono_font),
                true,
                window,
                cx,
            );
        }
        pick(
            &handles.language,
            &index_id(language_index(settings.language)),
            window,
            cx,
        );
        pick(&handles.ui_font, &settings.ui_font, window, cx);
        pick(&handles.mono_font, &settings.mono_font, window, cx);
        pick(
            &handles.log_level,
            &index_id(log_level_index(settings.log_level)),
            window,
            cx,
        );
    }
}

fn language_index(choice: LanguageChoice) -> usize {
    LanguageChoice::ALL
        .iter()
        .position(|candidate| *candidate == choice)
        .unwrap_or(0)
}

fn log_level_index(level: LogLevel) -> usize {
    LogLevel::ALL
        .iter()
        .position(|candidate| *candidate == level)
        .unwrap_or(0)
}

/// The select's box, with its label in the monospace family `mono` if given.
pub(super) fn render_select(
    id: &'static str,
    select: &SettingsSelect,
    mono: Option<SharedString>,
) -> AnyElement {
    let field = SearchableSelect::new(id, select);
    match mono {
        Some(family) => field.mono(family),
        None => field,
    }
    .into_any_element()
}
