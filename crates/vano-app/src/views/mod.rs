//! The `gpui` views. Logic lives in `state`; these modules only build elements.

pub mod common;
pub mod components;
pub mod feedback;
pub mod home;
pub mod live_views;
pub mod results;
pub mod root;
pub mod scanning;
pub mod settings;
pub mod status_bar;
pub mod title_bar;
pub mod treemap_view;
pub mod update;

pub use root::AppView;
