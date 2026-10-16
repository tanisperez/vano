//! Vano, the disk analyzer. Binary entry point: initializes `gpui`, the theme and the main window.

// A GUI application on Windows: never open a console window behind the main window.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod actions;
mod logging;
mod state;
mod theme;
mod views;

rust_i18n::i18n!("locales", fallback = "en");

use std::path::Path;

use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::*;

use actions::{ChooseFolder, CloseDetail, OpenSettings, StartScan, ZoomOut};
use state::settings::ThemeChoice;
use views::AppView;

/// The application id: Wayland `app_id`, X11 `WM_CLASS`, Windows AppUserModelID. The Linux
/// `codes.tanis.vano.desktop` file uses the same value.
const APP_ID: &str = "codes.tanis.vano";

/// The user-visible application name.
const APP_NAME: &str = "Vano";

fn main() {
    let settings = state::settings::load_settings();
    let env_level = std::env::var(logging::ENV_VAR).ok();
    let log_level = logging::startup_level(settings.log_level, env_level.as_deref());
    let log_path = logging::init(log_level.to_filter());
    let from_env = env_level
        .as_deref()
        .and_then(state::settings::LogLevel::parse)
        .is_some();
    log_startup(log_level, from_env, log_path.as_deref());

    let language = state::locale::resolve(settings.language);
    state::locale::apply(language);
    log::info!("language: {}", language.code());

    let debug = std::env::var("VANO_OPEN")
        .ok()
        .and_then(|value| state::debug_open::parse(&value));
    let theme_choice = settings.theme;

    let app = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
    app.run(move |cx| {
        cx.set_app_identity(APP_ID, APP_NAME);
        gpui_kit::init(cx);
        bind_keys(cx);
        theme::install(cx, &settings);

        // Centered at 1400x900, capped to 90% of the display. Some Wayland compositors report no
        // display before the first configure event, hence the fallback.
        let window_bounds = match cx.primary_display() {
            Some(display) => {
                let visible = display.visible_bounds();
                let cap = size(visible.size.width * 0.9, visible.size.height * 0.9);
                let target = size(px(1400.0), px(900.0)).min(&cap);
                WindowBounds::Windowed(Bounds::centered_at(visible.center(), target))
            }
            None => WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(1280.0), px(780.0)),
            }),
        };
        let window_options = WindowOptions {
            window_bounds: Some(window_bounds),
            window_min_size: Some(size(px(900.), px(560.))),
            window_decorations: Some(WindowDecorations::Client),
            app_id: Some(APP_ID.to_string()),
            ..TitleBar::window_options()
        };

        cx.spawn(async move |cx| {
            let opened = cx.open_window(window_options, move |window, cx| {
                match theme_choice {
                    ThemeChoice::System => Theme::sync_system_appearance(Some(window), cx),
                    ThemeChoice::Light => Theme::change(ThemeMode::Light, Some(window), cx),
                    ThemeChoice::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
                }
                let view = cx.new(|cx| AppView::new(settings, debug, window, cx));
                // "System" follows the OS live; checked on every change so a later choice of
                // Light or Dark simply stops it.
                let weak = view.downgrade();
                window
                    .observe_window_appearance(move |window, cx| {
                        let follows_system = weak.upgrade().is_some_and(|view| {
                            view.read(cx).state.settings.theme == ThemeChoice::System
                        });
                        if follows_system {
                            Theme::sync_system_appearance(Some(window), cx);
                        }
                    })
                    .detach();
                cx.new(|cx| Root::new(view, window, cx))
            });
            #[allow(clippy::expect_used)]
            opened.expect("failed to open the main window");
        })
        .detach();
    });
}

/// The first lines of every log.
fn log_startup(level: state::settings::LogLevel, from_env: bool, log_path: Option<&Path>) {
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
        if from_env {
            format!(" (from {})", logging::ENV_VAR)
        } else {
            String::new()
        },
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
        KeyBinding::new("enter", StartScan, context),
        KeyBinding::new("backspace", ZoomOut, context),
        KeyBinding::new("alt-left", ZoomOut, context),
        KeyBinding::new("escape", CloseDetail, context),
    ]);
}
