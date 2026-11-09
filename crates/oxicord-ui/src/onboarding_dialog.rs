//! The Server Guide's onboarding questions. The view holds the local
//! selection only; applying the answers is the next roadmap item.

use std::collections::{HashMap, HashSet};

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, ClickEvent, Context, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window,
    div, px, rgb,
};
use oxicord_model::{Onboarding, OnboardingPrompt, PromptType, Snowflake};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    profile::modal,
    theme,
};

pub struct CloseOnboarding;

impl EventEmitter<CloseOnboarding> for OnboardingDialog {}

pub struct OnboardingDialog {
    backend: Backend,
    guild: Option<Snowflake>,
    onboarding: Option<Onboarding>,
    /// Picked options per prompt.
    selected: HashMap<Snowflake, HashSet<Snowflake>>,
    loading: bool,
    failed: bool,
}

impl OnboardingDialog {
    pub fn new(backend: Backend) -> Self {
        Self {
            backend,
            guild: None,
            onboarding: None,
            selected: HashMap::new(),
            loading: false,
            failed: false,
        }
    }

    /// Shows the guide for `guild` and asks for its questions.
    pub fn opened(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.onboarding = None;
        self.selected.clear();
        self.loading = true;
        self.failed = false;
        self.backend.send(Command::LoadOnboarding { guild });
        cx.notify();
    }

    /// The answer to [`Command::LoadOnboarding`], if it is still the guild on
    /// screen.
    pub fn apply(&mut self, guild: Snowflake, result: Result<Onboarding, String>, cx: &mut Context<Self>) {
        if self.guild != Some(guild) {
            return;
        }
        self.loading = false;
        match result {
            Ok(onboarding) => {
                self.onboarding = Some(onboarding);
                self.failed = false;
            }
            Err(_) => {
                self.onboarding = None;
                self.failed = true;
            }
        }
        cx.notify();
    }

    /// The onboarding event for this dialog, routed by the app.
    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(CloseOnboarding);
    }

    fn pick(&mut self, prompt_id: Snowflake, single_select: bool, option: Snowflake, cx: &mut Context<Self>) {
        let picked = self.selected.entry(prompt_id).or_default();
        if single_select {
            picked.clear();
            picked.insert(option);
        } else if !picked.remove(&option) {
            picked.insert(option);
        }
        cx.notify();
    }

    fn prompt(&mut self, prompt: &OnboardingPrompt, cx: &mut Context<Self>) -> AnyElement {
        let picked = self.selected.get(&prompt.id).cloned().unwrap_or_default();
        let (prompt_id, single_select) = (prompt.id, prompt.single_select);
        let mut options = div().flex().flex_col().gap(px(8.));
        for option in &prompt.options {
            let on = picked.contains(&option.id);
            let id = option.id;
            let control = if prompt.single_select {
                div()
                    .size(px(20.))
                    .flex_none()
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(if on { theme::ACCENT } else { theme::TEXT_FAINT }))
                    .when(on, |dot| dot.p(px(3.)).child(div().size_full().rounded_full().bg(rgb(theme::ACCENT))))
                    .into_any_element()
            } else {
                div()
                    .size(px(20.))
                    .flex_none()
                    .rounded(px(4.))
                    .border_2()
                    .border_color(rgb(if on { theme::ACCENT } else { theme::TEXT_FAINT }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(on, |boxy| boxy.bg(rgb(theme::ACCENT)).child(icon("check", 14., 0xffff_ffff)))
                    .into_any_element()
            };
            let mut card = div()
                .id(("onboarding-option", option.id.0))
                .w_full()
                .px(px(12.))
                .py(px(10.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(theme::RADIUS))
                .bg(rgb(if on { theme::BG_ACTIVE } else { theme::BG_CARD }))
                .border_1()
                .border_color(rgb(if on { theme::ACCENT } else { theme::BORDER }))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick(prompt_id, single_select, id, cx)))
                .child(control);
            if let Some(emoji) = &option.emoji {
                let glyph = match (&emoji.id, &emoji.name) {
                    (None, Some(name)) => Some(name.clone()),
                    (Some(_), Some(name)) => Some(name.chars().next().map(|c| c.to_string()).unwrap_or_default()),
                    _ => None,
                };
                if let Some(glyph) = glyph.filter(|glyph| !glyph.is_empty()) {
                    card = card.child(
                        div()
                            .w(px(20.))
                            .flex_none()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .text_center()
                            .child(glyph),
                    );
                }
            }
            card = card.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .line_height(px(19.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(option.title.clone()),
                    )
                    .children((!option.description.is_empty()).then(|| {
                        div()
                            .text_size(px(13.))
                            .line_height(px(17.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(option.description.clone())
                    })),
            );
            options = options.child(card);
        }
        let kind = match prompt.kind {
            PromptType::Dropdown => "Drop-down",
            _ => "Multiple choice",
        };
        div()
            .w_full()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .rounded(px(theme::RADIUS_MD))
            .bg(rgb(theme::BG_PANEL))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(prompt.title.clone()),
                    )
                    .children(prompt.required.then(|| {
                        div()
                            .flex_none()
                            .px(px(6.))
                            .h(px(18.))
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .bg(rgb(theme::BG_ACTIVE))
                            .text_size(px(11.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Required")
                    })),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_FAINT))
                    .child(kind),
            )
            .child(options)
            .into_any_element()
    }
}

impl Render for OnboardingDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prompts: Vec<_> = self
            .onboarding
            .as_ref()
            .map(|onboarding| {
                onboarding
                    .prompts
                    .iter()
                    .filter(|prompt| prompt.in_onboarding)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let mut body = div()
            .id("onboarding-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.));
        if self.loading {
            body = body.child(
                div()
                    .py(px(24.))
                    .text_center()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Loading the server guide..."),
            );
        } else if self.failed {
            body = body.child(
                div()
                    .py(px(24.))
                    .text_center()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("The server guide could not be loaded."),
            );
        } else if prompts.is_empty() {
            body = body.child(
                div()
                    .py(px(24.))
                    .text_center()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("This server has no onboarding questions."),
            );
        } else {
            for prompt in &prompts {
                body = body.child(self.prompt(prompt, cx));
            }
        }

        let done = div()
            .id("onboarding-done")
            .h(px(32.))
            .w(px(60.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::ACCENT))
            .cursor_pointer()
            .text_size(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xffff_ffff))
            .hover(|s| s.bg(rgb(theme::ACCENT_HOVER)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
            .child("Done");

        let content = div()
            .id("onboarding-dialog")
            .occlude()
            .w(px(620.))
            .max_h(px(700.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(theme::RADIUS_MD))
            .bg(rgb(theme::BG_PANEL))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Server Guide"),
                    )
                    .child(
                        div()
                            .id("close-onboarding")
                            .size(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
                            .child(icon("x", 18., theme::TEXT_MUTED)),
                    ),
            )
            .child(
                div()
                    .px(px(16.))
                    .pt(px(12.))
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Answer these to shape the channels and roles you see in this server."),
            )
            .child(body)
            .child(
                div()
                    .h(px(56.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(px(16.))
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .child(done),
            );

        modal(
            content.into_any_element(),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
        )
    }
}
