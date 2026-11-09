//! Shared vertical scroll viewport.

use gpui_kit::{
    Div, ElementId, InteractiveElement as _, StatefulInteractiveElement as _, Styled as _, div,
};

/// A vertical scroll area that takes the remaining height of its flex column.
pub fn vertical(id: impl Into<ElementId>) -> gpui_kit::Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_h_0()
        .w_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
}
