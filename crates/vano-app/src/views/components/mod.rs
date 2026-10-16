//! Reusable building blocks of Vano's design system. Each one reads colors only from
//! [`crate::theme::PaletteExt`] and sizes only from [`crate::theme::metrics`].

pub mod buttons;
pub mod icon_button;
pub mod modal;
pub mod segmented_control;
pub mod status_badge;
pub mod theme_card;

pub use buttons::{DangerButton, GhostButton, PrimaryButton, SecondaryButton};
pub use icon_button::IconButton;
pub use modal::{
    MODAL_INNER_RADIUS, ModalPlacement, framed_dialog, modal_body, modal_footer, modal_header,
};
pub use segmented_control::{SegmentedControl, SegmentedItem};
pub use status_badge::StatusBadge;
pub use theme_card::ThemeCard;
