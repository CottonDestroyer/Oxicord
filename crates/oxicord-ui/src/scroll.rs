//! Shared vertical scroll viewport.

use gpui_kit::{
    Div, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    ScrollHandle, StatefulInteractiveElement as _, Styled as _, div, linear_color_stop,
    linear_gradient, px, rgba,
};

/// A vertical scroll area that takes the remaining height of its flex column,
/// with a fade at either edge while content continues past it.
pub fn vertical(
    id: impl Into<ElementId>,
    handle: &ScrollHandle,
    content: impl IntoElement,
) -> Div {
    let offset = handle.offset().y;
    let more_above = offset < px(0.);
    let more_below = offset > -handle.max_offset().y;
    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .relative()
        .flex()
        .flex_col()
        .child(
            div()
                .id(id)
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .track_scroll(handle)
                .child(content),
        )
        .children(more_above.then(|| edge(true)))
        .children(more_below.then(|| edge(false)))
}

fn edge(top: bool) -> Div {
    /// Read from the web client's scrolled screening dialog.
    const HEIGHT: f32 = 170.;
    let solid = rgba(0x0a0a0cff);
    let clear = rgba(0x0a0a0c00);
    let gradient = if top {
        linear_gradient(180., linear_color_stop(solid, 0.), linear_color_stop(clear, 1.))
    } else {
        linear_gradient(180., linear_color_stop(clear, 0.), linear_color_stop(solid, 1.))
    };
    let edge = div().absolute().left_0().right_0().h(px(HEIGHT)).bg(gradient);
    if top { edge.top_0() } else { edge.bottom_0() }
}
