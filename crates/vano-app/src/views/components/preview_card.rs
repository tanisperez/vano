//! A preview card with a radio and a name: the accent cards (Indigo, Graphite, Electric blue) and
//! the live view cards (Treemap, Rings, Ranking) of Settings. The theme cards (System, Light,
//! Dark) are telaio's `ThemeCard`.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::state::settings::{Accent, LiveView};
use crate::theme::metrics::{RADIUS_LG, RADIUS_SM, THEME_PREVIEW_HEIGHT, THEME_PREVIEW_PADDING};
use crate::theme::palette::accent_palette;
use crate::theme::{Palette, PaletteExt, VanoColors};

/// What a card previews.
#[derive(Debug, Clone, Copy)]
enum CardKind {
    Accent(Accent),
    LiveView(LiveView),
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// One preview card. It fills the width its parent gives it (`flex_1`).
#[derive(IntoElement)]
pub struct PreviewCard {
    id: ElementId,
    kind: CardKind,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl PreviewCard {
    /// The card of an accent palette, previewed in the current mode.
    pub fn accent(id: impl Into<ElementId>, accent: Accent) -> Self {
        Self::of(id, CardKind::Accent(accent))
    }

    /// The card of a live view, with a small drawing of it.
    pub fn live_view(id: impl Into<ElementId>, view: LiveView) -> Self {
        Self::of(id, CardKind::LiveView(view))
    }

    fn of(id: impl Into<ElementId>, kind: CardKind) -> Self {
        Self {
            id: id.into(),
            kind,
            selected: false,
            on_click: None,
        }
    }

    /// Whether this is the current theme: accent frame and a filled radio.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Runs on a click anywhere on the card.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for PreviewCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.palette();
        let selected = self.selected;
        let dark = palette.dark;
        let (label, left, right, accent_line) = match self.kind {
            CardKind::Accent(accent) => {
                let p = Rc::new(accent_palette(accent, dark));
                (accent.label(), p.clone(), p.clone(), p.accent)
            }
            CardKind::LiveView(view) => (
                view.label(),
                palette.clone(),
                palette.clone(),
                palette.accent,
            ),
        };

        let ring_color = if selected {
            palette.accent
        } else {
            palette.border_strong
        };

        let mut radio = div().size(px(14.0)).rounded_full().border_color(ring_color);
        radio = if selected {
            radio.border_4()
        } else {
            radio.border_1()
        };

        // A real border, not a `box-shadow` ring (an `N px` spread shadow): a spread, zero-blur,
        // zero-offset shadow on a `rounded()` + `overflow_hidden()` box rendered as a filled halo
        // covering the box's own content here, not a thin outline as CSS would, so the border
        // achieves the same "ring" look without that. The frame is a recessed well (`surface`,
        // `accent_subtle` when selected) around the mini window, which keeps its own rounded
        // corners and border. The selected frame's thicker border eats one pixel of padding, so
        // the mini window stays put when the selection changes.
        let mut preview_box = div()
            .flex()
            .h(px(THEME_PREVIEW_HEIGHT))
            .rounded(px(RADIUS_LG))
            .border_color(ring_color)
            .bg(if selected {
                palette.accent_subtle
            } else {
                palette.surface
            });
        preview_box = if selected {
            preview_box.border_2().p(px(THEME_PREVIEW_PADDING - 1.0))
        } else {
            preview_box.border_1().p(px(THEME_PREVIEW_PADDING))
        };
        // gpui clips children to a rectangle, not to rounded corners, so each half rounds its own
        // outer corners inside the mini window's 1 px border.
        let mini_radius = px(RADIUS_SM - 1.0);
        let mini_window = h_flex()
            .flex_1()
            .items_stretch()
            .rounded(px(RADIUS_SM))
            .border_1()
            .border_color(palette.border_strong)
            .child(
                v_flex()
                    .flex_1()
                    .rounded_l(mini_radius)
                    .bg(left.surface)
                    .p(px(8.0))
                    .gap(px(5.0))
                    .child(preview_line(0.7, left.border_strong))
                    .child(preview_line(0.5, accent_line))
                    .child(preview_line(0.6, left.border_strong)),
            )
            .child(
                v_flex()
                    .flex_grow(2.0)
                    .flex_shrink(1.0)
                    .flex_basis(relative(0.0))
                    .rounded_r(mini_radius)
                    .bg(right.bg)
                    .p(px(8.0))
                    .gap(px(6.0))
                    .child(
                        div()
                            .h(px(12.0))
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(right.border_strong),
                    )
                    .child(preview_line(0.8, right.border_strong))
                    .child(preview_line(0.55, right.border_strong)),
            );

        let mini_window = match self.kind {
            CardKind::LiveView(view) => live_view_drawing(view, &palette).into_any_element(),
            CardKind::Accent(_) => mini_window.into_any_element(),
        };
        let on_click = self.on_click;
        v_flex()
            .id(self.id)
            .flex_1()
            .min_w_0()
            .gap(px(8.0))
            .cursor_pointer()
            .hover(|style| style.opacity(0.85))
            .active(|style| style.opacity(0.7))
            .child(preview_box.child(mini_window))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(radio)
                    .child(label),
            )
            .when_some(on_click, |card, on_click| {
                card.on_click(move |event, window, cx| on_click(event, window, cx))
            })
    }
}

/// A mini preview "line": a thin, colored, rounded bar `width_fraction` of its parent's width.
fn preview_line(width_fraction: f32, color: Hsla) -> impl IntoElement {
    div()
        .h(px(5.0))
        .w(relative(width_fraction))
        .rounded(px(3.0))
        .bg(color)
}

/// A small drawing of a live view, in the category colors.
fn live_view_drawing(view: LiveView, palette: &Palette) -> impl IntoElement {
    use vano_core::Category;
    let cat = |c: Category| palette.category(c);
    let tile = |c: Category, grow: f32| {
        div()
            .flex_grow(grow)
            .flex_basis(relative(0.0))
            .rounded(px(3.0))
            .border_1()
            .border_color(cat(c).line)
            .bg(cat(c).fill)
    };
    let body = div()
        .flex_1()
        .flex()
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(palette.border_strong)
        .bg(palette.raised)
        .p(px(6.0));
    match view {
        LiveView::Treemap => body.gap(px(3.0)).child(tile(Category::Code, 3.0)).child(
            v_flex()
                .flex_grow(2.0)
                .flex_basis(relative(0.0))
                .gap(px(3.0))
                .child(tile(Category::Ai, 2.0))
                .child(
                    h_flex()
                        .flex_grow(1.0)
                        .flex_basis(relative(0.0))
                        .gap(px(3.0))
                        .child(tile(Category::Caches, 1.0))
                        .child(tile(Category::Sdk, 1.0)),
                ),
        ),
        LiveView::Rings => body.items_center().justify_center().child(
            div()
                .size(px(52.0))
                .rounded_full()
                .border(px(7.0))
                .border_color(cat(Category::Code).dot),
        ),
        LiveView::Ranking => body.flex_col().justify_center().gap(px(5.0)).children(
            [
                (Category::Code, 0.9),
                (Category::Ai, 0.65),
                (Category::Caches, 0.45),
                (Category::Sdk, 0.3),
            ]
            .into_iter()
            .map(|(c, w)| {
                div()
                    .h(px(5.0))
                    .w(relative(w))
                    .rounded(px(3.0))
                    .bg(cat(c).dot)
            }),
        ),
    }
}
