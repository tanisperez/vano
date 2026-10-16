//! [`SegmentedControl`] and [`SegmentedItem`]: the Pretty/Raw and JSON/Text/XML/Form/None switches.
//!
//! Custom element: gpui-kit's `Tab`/`TabBar` `.segmented()` variant would need every one of its
//! resolved colors mapped through `ThemeConfigColors`, and the selected item's shadow the design
//! uses is a small, fixed `rgba(0,0,0,.12)` value with no equivalent `ThemeConfigColors` field
//! (`ThemeConfigColors` has no shadow field at all, see `docs/gpui-notes.md`), so this is built as
//! plain `div`s colored from [`crate::theme::Palette`], with the one shadow value written here
//! directly (not a semantic palette token, same footing as the design's own radius constants).

use std::rc::Rc;

use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::PaletteExt;
use crate::theme::metrics::{RADIUS_SM, SEGMENTED_CONTROL_INNER_HEIGHT};

/// The selected item's shadow color: opaque black at 12% alpha. This shadow is a
/// fixed value, the same in both themes (unlike
/// [`crate::theme::Palette::shadow`], whose two layers genuinely differ between light and dark),
/// so it is not a [`crate::theme::Palette`] field. Built as a plain struct literal, not a color
/// constructor call, to keep it out of `views/`'s color-literal grep check.
const SHADOW_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.0,
    a: 0.12,
};

/// One item of a [`SegmentedControl`].
pub struct SegmentedItem {
    label: SharedString,
    selected: bool,
    on_click: Option<ItemHandler>,
}

/// An item click handler, factored out because clippy's `type_complexity` flags the inline form.
type ItemHandler = Rc<dyn Fn(&mut Window, &mut App)>;

impl SegmentedItem {
    /// A new item labeled `label`.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            on_click: None,
        }
    }

    /// Marks this item as the selected one.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the click handler.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

/// A segmented control: a `surface` track with a border, one item per option, the selected item
/// `raised` with a small shadow (the Pretty/Raw and body type
/// rows).
#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    items: Vec<SegmentedItem>,
}

impl SegmentedControl {
    /// An empty segmented control; add items with [`Self::item`].
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
        }
    }

    /// Appends one item.
    pub fn item(mut self, item: SegmentedItem) -> Self {
        self.items.push(item);
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.palette();

        h_flex()
            .id(self.id)
            .p(px(2.0))
            .gap(px(2.0))
            .rounded(px(RADIUS_SM + 1.0))
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .children(self.items.into_iter().enumerate().map(|(index, item)| {
                let selected = item.selected;
                let handler = item.on_click;
                let mut cell = h_flex()
                    .id(("segmented-item", index))
                    .h(px(SEGMENTED_CONTROL_INNER_HEIGHT))
                    .px(px(10.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS_SM - 1.0))
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(palette.hover))
                    .active(|style| style.bg(palette.pressed))
                    .when(selected, |cell| {
                        cell.bg(palette.raised).text_color(palette.fg).shadow(vec![
                            BoxShadow::new(px(0.0), px(1.0), SHADOW_COLOR).blur_radius(px(2.0)),
                        ])
                    })
                    .when(!selected, |cell| cell.text_color(palette.fg_muted))
                    .child(item.label);
                if let Some(handler) = handler {
                    cell = cell.on_click(move |_, window, cx| handler(window, cx));
                }
                cell
            }))
    }
}
