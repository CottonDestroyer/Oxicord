//! A bot's modal from `INTERACTION_MODAL_CREATE`: its title and text inputs.
//! Submitting is the next roadmap item; this view owns the fields and their
//! required/min/max validation.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollHandle, StatefulInteractiveElement as _, Styled as _,
    Window,
    component::input::{Input, InputState, Textarea, TextareaState},
    div, px, rgb,
};
use oxicord_model::{InteractionModal, ModalInput};

use crate::{icons::icon, profile::modal, theme};

pub struct CloseInteractionModal;

impl EventEmitter<CloseInteractionModal> for InteractionModalView {}

enum Field {
    Short(Entity<InputState>),
    Paragraph(Entity<TextareaState>),
}

struct Row {
    input: ModalInput,
    field: Field,
}

pub struct InteractionModalView {
    modal: Option<InteractionModal>,
    rows: Vec<Row>,
    scroll: ScrollHandle,
}

impl InteractionModalView {
    pub fn new() -> Self {
        Self {
            modal: None,
            rows: Vec::new(),
            scroll: ScrollHandle::new(),
        }
    }

    pub fn opened(&mut self, modal: InteractionModal, window: &mut Window, cx: &mut Context<Self>) {
        self.scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        self.rows = modal
            .inputs()
            .map(|input| {
                let placeholder = input.placeholder.clone().unwrap_or_default();
                let field = if input.paragraph() {
                    let state = cx.new(|cx| {
                        TextareaState::new(window, cx)
                            .auto_grow(3, 8)
                            .placeholder(placeholder)
                    });
                    Field::Paragraph(state)
                } else {
                    let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                    Field::Short(state)
                };
                Row { input: input.clone(), field }
            })
            .collect();
        self.modal = Some(modal);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.modal = None;
        self.rows.clear();
        cx.emit(CloseInteractionModal);
    }

    fn text(&self, field: &Field, cx: &Context<Self>) -> String {
        match field {
            Field::Short(state) => state.read(cx).value().to_string(),
            Field::Paragraph(state) => state.read(cx).value().to_string(),
        }
    }

    /// Whether every input has an acceptable answer. `min_length` and
    /// `max_length` count characters.
    fn valid(&self, cx: &Context<Self>) -> bool {
        self.rows.iter().all(|row| {
            let text = self.text(&row.field, cx);
            let length = text.chars().count() as u32;
            if row.input.required && text.trim().is_empty() {
                return false;
            }
            if let Some(min) = row.input.min_length
                && !text.trim().is_empty()
                && length < min
            {
                return false;
            }
            row.input.max_length.is_none_or(|max| length <= max)
        })
    }

    fn row(&self, row: &Row, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let text = self.text(&row.field, cx);
        let length = text.chars().count() as u32;
        let too_short = row
            .input
            .min_length
            .is_some_and(|min| !text.trim().is_empty() && length < min);
        let too_long = row.input.max_length.is_some_and(|max| length > max);
        let required = row.input.required;
        let hint = match (row.input.min_length, row.input.max_length) {
            (Some(min), Some(max)) => Some(format!("{min}-{max} characters")),
            (Some(min), None) => Some(format!("At least {min} characters")),
            (None, Some(max)) => Some(format!("At most {max} characters")),
            (None, None) => None,
        }
        .or_else(|| required.then(|| "Required".to_owned()));
        let counter = row
            .input
            .max_length
            .map(|max| format!("{length} / {max}"));

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(row.input.label.clone())
                    .children(required.then(|| {
                        div()
                            .text_color(rgb(theme::DANGER))
                            .child("*")
                    })),
            )
            .child(match &row.field {
                Field::Short(state) => div()
                    .w_full()
                    .child(Input::new(state).h(px(44.)))
                    .into_any_element(),
                Field::Paragraph(state) => div().w_full().child(Textarea::new(state)).into_any_element(),
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(if too_short || too_long { theme::DANGER } else { theme::TEXT_MUTED }))
                    .children(hint.map(|hint| div().child(hint)))
                    .children(counter.map(|counter| div().flex_none().child(counter))),
            )
    }
}

impl Default for InteractionModalView {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for InteractionModalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(sheet) = &self.modal else {
            return div().into_any_element();
        };
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .px(px(24.))
            .pt(px(16.))
            .pb(px(8.));
        for row in &self.rows {
            body = body.child(self.row(row, cx));
        }
        let body = div()
            .id("interaction-modal-body")
            .flex_none()
            .w_full()
            .max_h(px(688.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(body);

        let valid = self.valid(cx);
        let cancel = div()
            .id("interaction-modal-cancel")
            .h(px(40.))
            .px(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::SECONDARY))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .hover(|s| s.bg(rgb(theme::SECONDARY_HOVER)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
            .child("Cancel");
        let submit = div()
            .id("interaction-modal-submit")
            .h(px(40.))
            .px(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::ACCENT))
            .cursor_pointer()
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xffff_ffff))
            .when(valid, |button| button.hover(|s| s.bg(rgb(theme::ACCENT_HOVER))))
            .when(!valid, |button| button.opacity(0.5).cursor_default())
            .child("Submit");

        let content = div()
            .id("interaction-modal")
            .occlude()
            .w(px(480.))
            .max_h(px(800.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(theme::RADIUS_MD))
            .bg(rgb(theme::BG_PANEL))
            .border_1()
            .border_color(rgb(theme::MODAL_BORDER))
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(px(24.))
                    .pr(px(8.))
                    .pt(px(16.))
                    .pb(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pr(px(12.))
                            .truncate()
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(sheet.title.clone()),
                    )
                    .child(
                        div()
                            .id("close-interaction-modal")
                            .size(px(40.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
                            .child(icon("x", 20., theme::TEXT_MUTED)),
                    ),
            )
            .child(body)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(24.))
                    .pt(px(16.))
                    .pb(px(8.))
                    .child(cancel)
                    .child(submit),
            );

        modal(
            content.into_any_element(),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
        )
    }
}
