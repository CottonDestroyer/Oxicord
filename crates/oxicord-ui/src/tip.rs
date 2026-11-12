//! A plain text tooltip in the menu colors.

use gpui_kit::{Context, FontWeight, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px, rgb};

use crate::theme;

pub struct Tip(pub String);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_MENU))
            .text_size(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(self.0.clone())
    }
}
