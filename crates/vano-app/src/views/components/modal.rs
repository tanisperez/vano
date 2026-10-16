//! The frame every modal shares: an `overlay` card with the modal radius, a 1 px `border` and the
//! design's shadow token (`docs/design-system.md`, "Modals"), inside gpui-component's `Dialog`.
//!
//! `Dialog` paints a surface and a shadow of its own. That shadow is fixed (black at 10%, set
//! inside the open animation, so a `.shadow(..)` on the dialog is overwritten on every frame) and
//! does not show over the scrim. [`framed_dialog`] makes the dialog's own box transparent and
//! larger than the card by the room the card's shadow needs, and draws the card inside it, so the
//! shadow fits in the dialog's content clip and slides and fades in with the dialog.
//!
//! The card's parts are shared too: [`modal_header`] (title and close button), [`modal_body`] and
//! [`modal_footer`] (the dialog buttons).

use gpui_kit::assets::IconName;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex, window_paddings};
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::theme::PaletteExt;
use crate::theme::metrics::{MODAL_HEADER_HEIGHT, RADIUS_MODAL};

use super::icon_button::IconButton;

/// Room kept around the card for its shadow (offset y 12, blur 32): the shadow fades out about
/// 1.5 times the blur past the card's edges, shifted down by the offset.
const SHADOW_ROOM_TOP: f32 = 28.0;
const SHADOW_ROOM_BOTTOM: f32 = 52.0;
const SHADOW_ROOM_X: f32 = 40.0;

/// The radius for a card's child that paints its own background into the card's corners (the
/// Settings nav column, the command palette's list): the modal radius minus the card's border.
pub const MODAL_INNER_RADIUS: f32 = RADIUS_MODAL - 1.0;

/// Least distance `Dialog` keeps between its box and the window edges (gpui-component's
/// `spacing.lg`). A box that does not fit is shrunk to it.
const WINDOW_EDGE: f32 = 16.0;

/// Where the card goes vertically.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModalPlacement {
    /// Centered in the window: Settings and Keyboard shortcuts.
    Centered,
    /// The card's top at `fraction` of the window height, and at least `min` px down: the command
    /// palette, and the small dialogs at gpui-component's own default of a tenth.
    Top { fraction: f32, min: f32 },
}

/// The dialog box for a card, worked out from the window's usable size.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ModalGeometry {
    /// Width of the dialog box, card plus side room.
    width: f32,
    /// Height of the dialog box, card plus top and bottom room. `None` for a card as tall as its
    /// content.
    height: Option<f32>,
    /// The box's distance from the top of the window's usable area.
    margin_top: f32,
    /// Room between the box and the card, top, bottom and each side.
    room_top: f32,
    room_bottom: f32,
    room_x: f32,
}

impl ModalGeometry {
    /// The box for a card of `card_width` x `card_height` in a usable area of `view_width` x
    /// `view_height`. A card that fits keeps its size and gets the full shadow room. On a smaller
    /// window the room shrinks first, then the card, never past [`WINDOW_EDGE`].
    fn new(
        view_width: f32,
        view_height: f32,
        card_width: f32,
        card_height: Option<f32>,
        placement: ModalPlacement,
    ) -> Self {
        let usable_width = (view_width - 2.0 * WINDOW_EDGE).max(0.0);
        let card_width = card_width.min(usable_width);
        let room_x = SHADOW_ROOM_X.min((usable_width - card_width) / 2.0);

        let usable_height = (view_height - 2.0 * WINDOW_EDGE).max(0.0);
        let (card_height, room_top, room_bottom) = match card_height {
            Some(height) => {
                let height = height.min(usable_height);
                let scale = ((usable_height - height) / (SHADOW_ROOM_TOP + SHADOW_ROOM_BOTTOM))
                    .clamp(0.0, 1.0);
                (
                    Some(height),
                    SHADOW_ROOM_TOP * scale,
                    SHADOW_ROOM_BOTTOM * scale,
                )
            }
            None => (None, SHADOW_ROOM_TOP, SHADOW_ROOM_BOTTOM),
        };
        let height = card_height.map(|card| card + room_top + room_bottom);

        let card_top = match placement {
            ModalPlacement::Centered => (view_height - card_height.unwrap_or(0.0)) / 2.0,
            ModalPlacement::Top { fraction, min } => (view_height * fraction).max(min),
        };
        let lowest = view_height - WINDOW_EDGE - height.unwrap_or(0.0);
        let margin_top = (card_top - room_top).min(lowest).max(WINDOW_EDGE);

        Self {
            width: card_width + 2.0 * room_x,
            height,
            margin_top,
            room_top,
            room_bottom,
            room_x,
        }
    }
}

/// Turns `dialog` into the shared modal frame around `build`'s content: a card of `width` and
/// `height` (`None`: as tall as its content) placed at `placement`. The dialog's own box loses
/// its fill, border, padding and close button, so a modal draws its own close button in its
/// header. Call it inside `open_dialog`'s builder, which runs on every render, so the frame
/// follows window resizes.
pub fn framed_dialog<F>(
    dialog: Dialog,
    width: f32,
    height: Option<f32>,
    placement: ModalPlacement,
    window: &Window,
    build: F,
) -> Dialog
where
    F: Fn(&mut Window, &mut App) -> AnyElement + 'static,
{
    let paddings = window_paddings(window);
    let viewport = window.viewport_size();
    let view_width: f32 = (viewport.width - paddings.left - paddings.right).into();
    let view_height: f32 = (viewport.height - paddings.top - paddings.bottom).into();
    let geometry = ModalGeometry::new(view_width, view_height, width, height, placement);

    let dialog = dialog
        .w(px(geometry.width))
        .margin_top(px(geometry.margin_top))
        .p_0()
        .border_0()
        .bg(transparent_black())
        .close_button(false);
    let dialog = match geometry.height {
        Some(height) => dialog.h(px(height)),
        None => dialog,
    };
    dialog.content(move |content, window, cx| {
        let palette = cx.palette();
        // `DialogContent` is `v_flex().flex_1()`, and a flex item's automatic minimum size is its
        // content's natural size: without `min_h_0()` on it and on the card, a tall modal grows
        // past its box instead of letting its own scrollable body (Settings, Keyboard shortcuts)
        // scroll.
        content
            .min_h_0()
            .pt(px(geometry.room_top))
            .pb(px(geometry.room_bottom))
            .px(px(geometry.room_x))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .bg(palette.overlay)
                    .rounded(px(RADIUS_MODAL))
                    .border_1()
                    .border_color(palette.border)
                    .shadow(palette.shadow.clone())
                    .child(build(window, cx)),
            )
    })
}

/// Horizontal padding of a modal's header and footer, and all around its body.
const MODAL_PADDING: f32 = 20.0;
/// Gap between the children of a modal's body.
const MODAL_BODY_GAP: f32 = 12.0;
/// Gap between the buttons of a modal's footer.
const MODAL_FOOTER_GAP: f32 = 8.0;

/// A modal's header strip: `title` at the left and a close button at the right that closes the
/// dialog, over a bottom border. The frame removes gpui-component's own close button.
pub fn modal_header(title: impl Into<SharedString>, cx: &App) -> Div {
    let palette = cx.palette();
    h_flex()
        .h(px(MODAL_HEADER_HEIGHT))
        .flex_none()
        .items_center()
        .justify_between()
        .px(px(MODAL_PADDING))
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .child(
            IconButton::new("modal-close", IconName::Close)
                .large()
                .tooltip(t!("common.close"))
                .on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                }),
        )
}

/// A modal's body: a column padded on every side, for the content between the header and the
/// footer.
pub fn modal_body() -> Div {
    v_flex().p(px(MODAL_PADDING)).gap(px(MODAL_BODY_GAP))
}

/// A modal's footer strip, as tall as the header, under a top border: its children (the dialog
/// buttons) sit at the end of the row.
pub fn modal_footer(cx: &App) -> Div {
    let palette = cx.palette();
    h_flex()
        .h(px(MODAL_HEADER_HEIGHT))
        .flex_none()
        .items_center()
        .justify_end()
        .gap(px(MODAL_FOOTER_GAP))
        .px(px(MODAL_PADDING))
        .border_t_1()
        .border_color(palette.border)
}
