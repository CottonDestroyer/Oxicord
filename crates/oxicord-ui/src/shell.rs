use gpui_kit::{Context, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px, rgb};

use crate::theme;

/// Empty four-pane frame. The real views replace each placeholder as they land.
pub struct Shell;

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .bg(rgb(theme::BG_RAIL))
            .text_color(rgb(theme::TEXT))
            .child(div().w(px(theme::RAIL_WIDTH)).h_full())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .h_full()
                    .rounded_tl(px(theme::RADIUS))
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .child(pane(theme::BG_SIDEBAR, "channels").w(px(theme::SIDEBAR_WIDTH)))
                    .child(pane(theme::BG_CHAT, "chat").flex_1())
                    .child(pane(theme::BG_MEMBERS, "members").w(px(theme::MEMBERS_WIDTH))),
            )
    }
}

fn pane(bg: u32, label: &'static str) -> gpui_kit::Div {
    div()
        .h_full()
        .bg(rgb(bg))
        .p_4()
        .text_color(rgb(theme::TEXT_MUTED))
        .child(label)
}
