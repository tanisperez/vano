//! The Settings selects: language, the two font families, the JSON indentation, invalid TLS
//! certificates, the largest response and the log level. Every one is a gpui-component [`Select`], which brings a scrolling
//! list with a scrollbar, a placement under the box and, for the long font lists, a search box
//! that also gives keyboard access (type to filter, arrows, Enter).
//!
//! A [`Select`] needs a [`SelectState`] entity, so they are created when the dialog opens
//! ([`AppView::open_settings`]) and kept in [`AppView`]. Each option has a stable string id (the
//! family name, the megabytes, the position in the enum's `ALL`) apart from its translated label.

use gpui_kit::component::Icon;
use gpui_kit::component::IndexPath;
use gpui_kit::component::searchable_list::{
    SearchableListDelegate, SearchableListItem, SearchableVec,
};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::state;
use crate::state::locale::LanguageChoice;
use crate::state::settings::{LogLevel, Settings};
use crate::theme::Palette;
use crate::theme::metrics::{CONTROL_HEIGHT, RADIUS_MD};

use super::AppView;

/// Width of a select box.
pub(super) const SELECT_WIDTH: f32 = 220.0;
/// Rows a searchable select always lists, filtered or not (see [`PaddedOptions`]).
const SEARCH_ROWS: usize = 12;
/// Maximum height of a select's list, which scrolls beyond it. Just above [`SEARCH_ROWS`] rows
/// (20px each, plus the list's padding), so a padded list never scrolls.
const MENU_MAX_HEIGHT: f32 = 252.0;
/// Size of the monospace picker's label, as in the editors.
const MONO_LABEL_SIZE: f32 = 12.5;

/// One row of a Settings select.
#[derive(Clone)]
pub(super) struct SelectOption {
    value: SharedString,
    label: SharedString,
    /// A filler row, not a choice (see [`PaddedOptions`]).
    filler: bool,
}

impl SelectOption {
    fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            filler: false,
        }
    }

    fn filler(index: usize, label: impl Into<SharedString>) -> Self {
        Self {
            value: format!("\u{0}filler-{index}").into(),
            label: label.into(),
            filler: true,
        }
    }
}

impl SearchableListItem for SelectOption {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &SharedString {
        &self.value
    }

    fn disabled(&self) -> bool {
        self.filler
    }
}

/// The options of a select, with empty filler rows after the matches so that a filtered list is
/// as tall as the full one. gpui-component places the list under the box when it fits and above
/// it when it does not, deciding again on every change of the list: a list that shrinks while the
/// user types would jump from above the box to below it. A constant height keeps it in place.
/// With no match at all, the first filler reads "No results".
pub(super) struct PaddedOptions {
    options: SearchableVec<SelectOption>,
    fillers: Vec<SelectOption>,
    no_results: SelectOption,
    matches: usize,
}

impl PaddedOptions {
    /// `min_rows` is 0 for a select without search, which needs no padding.
    fn new(options: Vec<SelectOption>, min_rows: usize) -> Self {
        let matches = options.len();
        Self {
            options: SearchableVec::new(options),
            fillers: (0..min_rows).map(|i| SelectOption::filler(i, "")).collect(),
            no_results: SelectOption::filler(min_rows, t!("settings.select.no_results")),
            matches,
        }
    }
}

impl SearchableListDelegate for PaddedOptions {
    type Item = SelectOption;

    fn items_count(&self, section: usize) -> usize {
        self.options.items_count(section).max(self.fillers.len())
    }

    fn item(&self, ix: IndexPath) -> Option<&SelectOption> {
        if ix.row < self.matches {
            return self.options.item(ix);
        }
        if self.matches == 0 && ix.row == 0 && !self.fillers.is_empty() {
            return Some(&self.no_results);
        }
        self.fillers.get(ix.row - self.matches)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        SelectOption: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.options.position(value)
    }

    fn perform_search(&mut self, query: &str, window: &mut Window, cx: &mut App) -> Task<()> {
        let task = self.options.perform_search(query, window, cx);
        self.matches = self.options.items_count(0);
        task
    }
}

type OptionsState = SelectState<PaddedOptions>;

/// The entity behind one Settings select.
pub(super) type SettingsSelect = Entity<OptionsState>;

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

/// Creates one select showing `selected`, and routes its confirmed choices to `on_pick`.
fn create_select(
    options: Vec<SelectOption>,
    selected: &str,
    searchable: bool,
    on_pick: impl Fn(&mut AppView, &str, &mut Window, &mut Context<AppView>) + 'static,
    subscriptions: &mut Vec<Subscription>,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> SettingsSelect {
    let selected: SharedString = selected.to_string().into();
    let rows = if searchable { SEARCH_ROWS } else { 0 };
    let select = cx.new(|cx| {
        let mut state = SelectState::new(PaddedOptions::new(options, rows), None, window, cx)
            .searchable(searchable);
        state.set_selected_value(&selected, window, cx);
        state
    });
    subscriptions.push(cx.subscribe_in(
        &select,
        window,
        move |view, _, event: &SelectEvent<PaddedOptions>, window, cx| {
            if let SelectEvent::Confirm(Some(value)) = event {
                on_pick(view, value, window, cx);
            }
        },
    ));
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
            _subscriptions: subscriptions,
        }
    }

    /// Shows `settings` in every select, for a change made outside them (Reset to defaults).
    /// With `relabel`, the translated labels are rebuilt too (the language changed). The font lists
    /// are not rebuilt: they are long and only their selection can change.
    pub(super) fn sync(
        &self,
        settings: &Settings,
        relabel: bool,
        window: &mut Window,
        cx: &mut Context<AppView>,
    ) {
        let handles = &self.handles;
        if relabel {
            set_items(&handles.language, language_options(), window, cx);
            set_items(&handles.log_level, log_level_options(), window, cx);
        }
        select_value(
            &handles.language,
            &index_id(language_index(settings.language)),
            window,
            cx,
        );
        select_value(&handles.ui_font, &settings.ui_font, window, cx);
        select_value(&handles.mono_font, &settings.mono_font, window, cx);
        select_value(
            &handles.log_level,
            &index_id(log_level_index(settings.log_level)),
            window,
            cx,
        );
    }
}

fn set_items(
    select: &SettingsSelect,
    options: Vec<SelectOption>,
    window: &mut Window,
    cx: &mut Context<AppView>,
) {
    select.update(cx, |state, cx| {
        state.set_items(PaddedOptions::new(options, 0), window, cx)
    });
}

/// Selects `value`, unless it is already the selected one.
fn select_value(
    select: &SettingsSelect,
    value: &str,
    window: &mut Window,
    cx: &mut Context<AppView>,
) {
    let value: SharedString = value.to_string().into();
    select.update(cx, |state, cx| {
        if state.selected_value() != Some(&value) {
            state.set_selected_value(&value, window, cx);
        }
    });
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

/// A select box styled as the design system's select (`select_trigger`): the same height, border,
/// background and chevrons icon. `mono` renders the label in that family at the editor size.
pub(super) fn render_select(
    id: &'static str,
    select: &SettingsSelect,
    palette: &Palette,
    mono: Option<SharedString>,
) -> AnyElement {
    let mut field = Select::new(select)
        .id(id)
        .appearance(false)
        .icon(Icon::new(gpui_kit::assets::IconName::ChevronsUpDown).text_color(palette.fg_subtle))
        .search_placeholder(t!("settings.select.search"))
        .menu_max_h(px(MENU_MAX_HEIGHT))
        .h(px(CONTROL_HEIGHT))
        .px(px(10.0))
        .rounded(px(RADIUS_MD - 1.0))
        .border_1()
        .border_color(palette.border_strong)
        .bg(palette.raised)
        .text_color(palette.fg);
    if let Some(family) = mono {
        field = field.font_family(family).text_size(px(MONO_LABEL_SIZE));
    }
    div()
        .w(px(SELECT_WIDTH))
        .h(px(CONTROL_HEIGHT))
        .child(field)
        .into_any_element()
}
