use gpui_kit::{Styled as _, Svg, px, rgb, svg};

/// A Lucide icon from the bundled catalog, by file name without extension.
pub fn icon(name: &str, size: f32, color: u32) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(rgb(color))
}
