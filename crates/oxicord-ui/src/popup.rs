//! A small menu that opens at a point, for right clicks and "more" buttons.
//! The owner describes the entries and gets the index of the one picked, so
//! the same menu serves messages, people and anything else.

use std::rc::Rc;

use gpui_kit::{
    App, ClickEvent, Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, point, px, rgb,
};

use crate::{icons::icon, theme};

#[derive(Clone)]
pub enum Entry {
    Item { label: String, glyph: &'static str, danger: bool },
    Separator,
}

impl Entry {
    pub fn item(label: impl Into<String>, glyph: &'static str) -> Self {
        Self::Item { label: label.into(), glyph, danger: false }
    }

    pub fn danger(label: impl Into<String>, glyph: &'static str) -> Self {
        Self::Item { label: label.into(), glyph, danger: true }
    }
}

/// Called with the index of the picked entry, counting separators.
pub type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

pub struct ClosePopup;

impl EventEmitter<ClosePopup> for PopupMenu {}

const WIDTH: f32 = 220.;
const ITEM: f32 = 32.;
const SEPARATOR: f32 = 9.;

#[derive(Default)]
pub struct PopupMenu {
    at: Point<Pixels>,
    entries: Vec<Entry>,
    pick: Option<Pick>,
}

impl PopupMenu {
    pub fn show(&mut self, at: Point<Pixels>, entries: Vec<Entry>, pick: Pick, cx: &mut Context<Self>) {
        self.at = at;
        self.entries = entries;
        self.pick = Some(pick);
        cx.notify();
    }

    fn height(&self) -> f32 {
        8. + self
            .entries
            .iter()
            .map(|e| if matches!(e, Entry::Separator) { SEPARATOR } else { ITEM })
            .sum::<f32>()
    }
}

/// Keeps a box of `size` inside a window of `bounds` when opened at `at`.
fn clamp(at: f32, size: f32, bounds: f32) -> f32 {
    at.min(bounds - size - 8.).max(8.)
}

impl Render for PopupMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let at = point(
            px(clamp(f32::from(self.at.x), WIDTH, f32::from(viewport.width))),
            px(clamp(f32::from(self.at.y), self.height(), f32::from(viewport.height))),
        );

        let mut card = div()
            .id("popup-card")
            .occlude()
            .absolute()
            .left(at.x)
            .top(at.y)
            .w(px(WIDTH))
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL));

        for (index, entry) in self.entries.iter().enumerate() {
            card = match entry {
                Entry::Separator => card.child(div().my(px(4.)).h(px(1.)).bg(rgb(theme::BORDER))),
                Entry::Item { label, glyph, danger } => {
                    let color = if *danger { theme::DANGER } else { theme::TEXT };
                    card.child(
                        div()
                            .id(("popup-item", index as u64))
                            .h(px(ITEM))
                            .px_2()
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .text_color(rgb(color))
                            .hover(|s| s.bg(rgb(if *danger { theme::BADGE } else { theme::ACCENT })).text_color(rgb(theme::TEXT_BRIGHT)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                let pick = this.pick.take();
                                cx.emit(ClosePopup);
                                if let Some(pick) = pick {
                                    pick(index, window, cx);
                                }
                            }))
                            .child(label.clone())
                            .child(icon(glyph, 16., color)),
                    )
                }
            };
        }

        div()
            .id("popup-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("popup-scrim")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(ClosePopup))),
            )
            .child(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_menu_near_the_edge_is_pulled_back_inside() {
        assert_eq!(clamp(100., 220., 1000.), 100.);
        assert_eq!(clamp(900., 220., 1000.), 772.);
        assert_eq!(clamp(-5., 220., 1000.), 8.);
    }
}
