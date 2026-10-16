//! Plain Rust state behind the views: no `gpui` rendering, unit tested.

pub mod about;
pub mod debug_open;
pub mod format;
pub mod locale;
pub mod motion;
pub mod number;
pub mod recents;
pub mod results;
pub mod settings;
pub mod target;
pub mod trash;
pub mod treemap;
pub mod update;

use update::UpdateState;

/// The state the Settings and update views share with the root view.
pub struct AppState {
    pub settings: settings::Settings,
    pub update: UpdateState,
}
