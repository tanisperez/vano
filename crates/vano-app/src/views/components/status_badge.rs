//! [`StatusBadge`]: a dot plus a label on a subtle background, like "Safe to remove" or a
//! category name in the detail panel.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::metrics::{RADIUS_SM, STATUS_BADGE_HEIGHT};

/// A pill with a colored dot and a label.
#[derive(IntoElement)]
pub struct StatusBadge {
    label: SharedString,
    dot: Hsla,
    fg: Hsla,
    bg: Hsla,
}

impl StatusBadge {
    /// A badge with `label` in `fg`, a dot of `dot` and a `bg` background.
    pub fn new(label: impl Into<SharedString>, dot: Hsla, fg: Hsla, bg: Hsla) -> Self {
        Self {
            label: label.into(),
            dot,
            fg,
            bg,
        }
    }
}

impl RenderOnce for StatusBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(5.0))
            .h(px(STATUS_BADGE_HEIGHT))
            .px(px(7.0))
            .rounded(px(RADIUS_SM))
            .bg(self.bg)
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.fg)
            .child(
                div()
                    .flex_none()
                    .size(px(6.0))
                    .rounded(px(3.0))
                    .bg(self.dot),
            )
            .child(self.label)
    }
}
