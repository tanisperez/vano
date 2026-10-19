//! Reusable building blocks of Vano's design system. The standard ones come from telaio
//! (re-exported here, so views keep importing them from `super::components`); the preview cards
//! of Settings are Vano's own. Each one reads colors only from [`crate::theme::PaletteExt`] and
//! sizes only from [`crate::theme::metrics`].

pub mod preview_card;

pub use preview_card::PreviewCard;
pub use telaio_ui::components::{
    DangerButton, GhostButton, IconButton, MODAL_INNER_RADIUS, ModalPlacement, PrimaryButton,
    SecondaryButton, SegmentedControl, SegmentedItem, StatusBadge, ThemeCard, framed_dialog,
    modal_body, modal_footer, modal_header,
};
