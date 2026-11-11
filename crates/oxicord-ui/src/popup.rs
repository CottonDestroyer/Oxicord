//! A small menu that opens at a point, for right clicks and "more" buttons.
//! The owner describes the entries and gets the index of the one picked, so
//! the same menu serves messages, people and anything else.

use std::rc::Rc;

use gpui_kit::{
    App, ClickEvent, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Point, Render, StatefulInteractiveElement as _, Styled as _, Window,
    component::slider::{Slider, SliderState},
    div, point, px, rgb,
};

use crate::{icons::icon, theme};

#[derive(Clone)]
pub enum Entry {
    Item { label: String, glyph: &'static str, danger: bool },
    /// A labelled slider that sets its value as it moves, with the value
    /// shown beside the label as a percent. Picking nothing closes nothing.
    Slider { label: String, state: Entity<SliderState> },
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
const SEPARATOR: f32 = 5.;
const GAP: f32 = 4.;
const SLIDER: f32 = 52.;

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
        16. + GAP * self.entries.len().saturating_sub(1) as f32 + self
            .entries
            .iter()
            .map(|e| match e {
                Entry::Separator => SEPARATOR,
                Entry::Slider { .. } => SLIDER,
                Entry::Item { .. } => ITEM,
            })
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
            .p(px(8.))
            .gap(px(GAP))
            .flex()
            .flex_col()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_MENU));

        for (index, entry) in self.entries.iter().enumerate() {
            card = match entry {
                Entry::Separator => card.child(div().my(px(2.)).h(px(1.)).bg(rgb(theme::BORDER))),
                Entry::Slider { label, state } => {
                    let percent = state.read(cx).value().start().round();
                    card.child(
                        div()
                            .id(("popup-slider", index as u64))
                            .h(px(SLIDER))
                            .px_2()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_size(px(14.))
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(label.clone())
                                    .child(div().text_color(rgb(theme::TEXT_MUTED)).child(format!("{percent}%"))),
                            )
                            .child(Slider::new(state).accessibility_label(label.clone())),
                    )
                }
                Entry::Item { label, glyph, danger } => {
                    let color = if *danger { theme::DANGER } else { theme::TEXT_BRIGHT };
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
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(rgb(color))
                            .hover(|s| s.bg(rgb(if *danger { theme::DANGER_HOVER } else { theme::BG_HOVER })))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                let pick = this.pick.take();
                                cx.emit(ClosePopup);
                                if let Some(pick) = pick {
                                    pick(index, window, cx);
                                }
                            }))
                            .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                            .child(div().ml_2().flex_none().child(icon(glyph, 16., color))),
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
