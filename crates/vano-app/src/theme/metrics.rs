//! Named layout constants (heights, widths, radii) from the design system. Views read sizes only
//! from here, never as a literal scattered in a view, so every size traces back to one place.

/// Height of the title bar.
pub const TITLE_BAR_HEIGHT: f32 = 40.0;
/// Height of the status bar.
pub const STATUS_BAR_HEIGHT: f32 = 24.0;
/// Height of ordinary controls (inputs, buttons).
pub const CONTROL_HEIGHT: f32 = 30.0;
/// Height of the small title bar controls (target switcher, gear, Rescan).
pub const ENV_PILL_HEIGHT: f32 = 26.0;
/// Height of a segmented control's inner items.
pub const SEGMENTED_CONTROL_INNER_HEIGHT: f32 = 22.0;

/// Radius for chips and checkboxes.
pub const RADIUS_XS: f32 = 4.0;
/// Radius for segmented items, rows and badges.
pub const RADIUS_SM: f32 = 6.0;
/// Radius for inputs, buttons and small cards.
pub const RADIUS_MD: f32 = 8.0;
/// Radius for cards, menus and popovers.
pub const RADIUS_LG: f32 = 10.0;
/// Radius for modals.
pub const RADIUS_MODAL: f32 = 14.0;
/// Height of a modal's header and footer strips.
pub const MODAL_HEADER_HEIGHT: f32 = 52.0;
/// Width of the Move to Trash confirmation.
pub const CONFIRM_WIDTH: f32 = 482.0;

/// Height of a theme card's preview in Settings.
pub const THEME_PREVIEW_HEIGHT: f32 = 92.0;
/// Padding between a theme card's frame and its preview.
pub const THEME_PREVIEW_PADDING: f32 = 8.0;
/// Height of a `StatusBadge`.
pub const STATUS_BADGE_HEIGHT: f32 = 22.0;
/// Side length of an `IconButton`'s large size.
pub const ICON_BUTTON_LG: f32 = 28.0;

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
