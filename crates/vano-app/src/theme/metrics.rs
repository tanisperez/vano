//! Named layout constants (heights, widths, radii) from the design system. Views read sizes only
//! from here, never as a literal scattered in a view, so every size traces back to one place.
//!
//! The sizes of the standard components and the radius scale come from telaio; the rest are
//! Vano's own layout.
#![allow(dead_code)]

pub use telaio_ui::metrics::*;

/// Height of the title bar.
pub const TITLE_BAR_HEIGHT: f32 = 40.0;
/// Height of the status bar.
pub const STATUS_BAR_HEIGHT: f32 = 24.0;
/// Height of the small title bar controls (target switcher, gear, Rescan).
pub const ENV_PILL_HEIGHT: f32 = 26.0;

/// Width of the Move to Trash confirmation.
pub const CONFIRM_WIDTH: f32 = 482.0;

/// Padding around the screen content, and the gap between its cards.
pub const PAGE_PADDING: f32 = 16.0;
/// Width of the right column (Recommendations, Found so far).
pub const SIDE_PANEL_WIDTH: f32 = 342.0;
/// Height of a stat cell of the stats strip.
pub const STAT_HEIGHT: f32 = 90.0;
/// Height of a card header (title and subtitle).
pub const CARD_HEADER_HEIGHT: f32 = 68.0;
/// Height of the explorer's breadcrumb row.
pub const BREADCRUMB_HEIGHT: f32 = 38.0;
/// Height of a recommendation row.
pub const REC_ROW_HEIGHT: f32 = 56.0;
/// Height of a ranking row.
pub const RANK_ROW_HEIGHT: f32 = 42.0;
/// Outer diameter of the live view's ring.
pub const RING_SIZE: f32 = 250.0;
/// Thickness of the ring.
pub const RING_THICKNESS: f32 = 20.0;

/// Width of the home screen's column.
pub const HOME_COLUMN_WIDTH: f32 = 680.0;
/// Side length of the home screen's logo square.
pub const HOME_LOGO_SIZE: f32 = 48.0;
/// Height of a location card on the home screen.
pub const LOCATION_CARD_HEIGHT: f32 = 110.0;
/// Height of a recent scan row.
pub const RECENT_ROW_HEIGHT: f32 = 40.0;
