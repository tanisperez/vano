//! Vano, the disk analyzer. Binary entry point: initializes `gpui`, the theme and the main window.

// A GUI application on Windows: never open a console window behind the main window.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod actions;
mod state;
mod theme;
mod views;

rust_i18n::i18n!("locales", fallback = "en");

use std::path::Path;

use gpui_kit::component::Root;
use gpui_kit::*;
use telaio_app::window::WindowSizes;
use telaio_app::{logging, window};

use actions::{
    ChooseFolder, CloseDetail, OpenSettings, OpenShortcuts, StartScan, ZoomForward, ZoomOut,
};
use state::about::IDENTITY;
use state::settings::ThemeChoice;
use views::AppView;

fn main() {
    let settings = state::settings::load_settings();
    let log_env_var = logging::env_var(&IDENTITY);
    let env_level = std::env::var(&log_env_var).ok();
    let log_level = logging::startup_level(settings.log_level, env_level.as_deref());
    let log_path = logging::init(&IDENTITY, log_level.to_filter());
    let from_env = env_level
        .as_deref()
        .and_then(state::settings::LogLevel::parse)
        .is_some();
    log_startup(
        log_level,
        from_env.then_some(log_env_var.as_str()),
        log_path.as_deref(),
    );

    let language = state::locale::resolve(settings.language);
    state::locale::apply(language);
    log::info!("language: {}", language.code());

    let debug = std::env::var("VANO_OPEN")
        .ok()
        .and_then(|value| state::debug_open::parse(&value));
    let theme_choice = settings.theme;

    let app = window::application();
    app.run(move |cx| {
        window::init(cx, &IDENTITY);
        bind_keys(cx);
        theme::install(cx, &settings);

        // Centered at 1400x900, capped to 90% of the display (see `window::options`).
        let window_options = window::options(
            cx,
            &IDENTITY,
            WindowSizes {
                default: (1400.0, 900.0),
                min: (900.0, 560.0),
            },
        );

        cx.spawn(async move |cx| {
            let opened = cx.open_window(window_options, move |window, cx| {
                telaio_ui::ThemeChoice::from(theme_choice).apply(Some(window), cx);
                let view = cx.new(|cx| AppView::new(settings, debug, window, cx));
                // Always listening, whatever the theme choice at startup: the Settings view can
                // switch to "System" at any time (see `window::follow_system_appearance`).
                let weak = view.downgrade();
                window::follow_system_appearance(window, move |cx| {
                    weak.upgrade().is_some_and(|view| {
                        view.read(cx).state.settings.theme == ThemeChoice::System
                    })
                });
                cx.new(|cx| Root::new(view, window, cx))
            });
            #[allow(clippy::expect_used)]
            opened.expect("failed to open the main window");
        })
        .detach();
    });
}

/// The first lines of every log.
fn log_startup(level: state::settings::LogLevel, from_env: Option<&str>, log_path: Option<&Path>) {
    log::info!(
        "Vano {} starting ({} build) on {} {}",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    log::info!(
        "log level {level:?}{}, log file {}",
        from_env.map_or_else(String::new, |variable| format!(" (from {variable})")),
        log_path.map_or_else(|| "none".to_string(), |path| path.display().to_string()),
    );
}

/// The key bindings. `Vano` is the root view's key context, so none of them fire inside a
/// dialog's text field.
fn bind_keys(cx: &mut App) {
    let context = Some("Vano");
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-o", ChooseFolder, context),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-,", OpenSettings, None),
        KeyBinding::new("ctrl-o", ChooseFolder, context),
    ]);
    cx.bind_keys([
        KeyBinding::new(
            telaio_ui::components::OPEN_SHORTCUTS_KEYSTROKE,
            OpenShortcuts,
            None,
        ),
        KeyBinding::new("enter", StartScan, context),
        KeyBinding::new("backspace", ZoomOut, context),
        KeyBinding::new("alt-left", ZoomOut, context),
        KeyBinding::new("alt-right", ZoomForward, context),
        KeyBinding::new("escape", CloseDetail, context),
    ]);
}
