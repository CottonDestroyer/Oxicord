//! The membership screening dialog: the server's card, its rules and the
//! checkbox that lets a new member talk. Values follow the web client's
//! "Before you can talk here..." modal.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, ClickEvent, Context, Entity, EventEmitter, FontWeight, ImageSource,
    InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Subscription, Window, div, img,
    linear_color_stop, linear_gradient, px, rgb, rgba,
};
use oxicord_media::cdn;
use oxicord_model::{GuildProfile, Snowflake, VerificationForm, VerificationInfo};

use crate::{
    account::Load,
    backend::Backend,
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

pub struct CloseRulesScreening;

impl EventEmitter<CloseRulesScreening> for RulesScreeningDialog {}

pub struct RulesScreeningDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    guild: Option<Snowflake>,
    agreed: bool,
    submitting: bool,
    _subscriptions: Vec<Subscription>,
}

impl RulesScreeningDialog {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![cx.observe(&session, |this, session, cx| {
            let (done, failed) = {
                let s = session.read(cx);
                match s.verification.as_ref().and_then(|v| v.submit.as_ref()) {
                    Some(Ok(())) => (true, false),
                    Some(Err(_)) => (false, true),
                    None => (false, false),
                }
            };
            if failed {
                this.submitting = false;
            }
            if done {
                cx.emit(CloseRulesScreening);
            }
            cx.notify();
        })];
        Self {
            session,
            media,
            backend,
            guild: None,
            agreed: false,
            submitting: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn opened(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.agreed = false;
        self.submitting = false;
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(CloseRulesScreening);
    }

    fn toggle_agreed(&mut self, cx: &mut Context<Self>) {
        self.agreed = !self.agreed;
        cx.notify();
    }

    fn submit(&mut self, form: VerificationForm, cx: &mut Context<Self>) {
        let Some(guild) = self.guild else { return };
        if !self.agreed || self.submitting || !form.only_terms() {
            return;
        }
        self.submitting = true;
        let backend = self.backend.clone();
        self.session.update(cx, |session, cx| {
            session.submit_verification(&backend, guild, form, self.agreed, cx);
        });
        cx.notify();
    }

    fn guild_card(
        &mut self,
        guild: Snowflake,
        profile: &GuildProfile,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon_url = profile
            .icon_hash
            .as_deref()
            .map(|hash| cdn::guild_icon(guild, hash, 128))
            .unwrap_or_default();
        let banner_url = profile
            .custom_banner_hash
            .as_deref()
            .map(|hash| cdn::discovery_splash(guild, hash, 512));
        let clan_badge = profile.badge_hash.as_deref().and_then(|hash| {
            self.media
                .update(cx, |media, _| media.get(&cdn::clan_badge(guild, hash, 16)))
        });

        let mut banner = div().h(px(120.)).w_full().overflow_hidden();
        if let Some(url) = banner_url {
            let image = self.media.update(cx, |media, _| media.get(&url));
            if let Some(image) = image {
                banner = banner.child(
                    img(ImageSource::Image(image))
                        .w_full()
                        .h(px(146.))
                        .mt(px(-13.))
                        .object_fit(gpui_kit::ObjectFit::Cover),
                );
            } else {
                banner = banner.bg(rgb(theme::BG_INPUT));
            }
        } else {
            banner = banner.bg(rgb(theme::BG_INPUT));
        }

        let mut name_row = div().h(px(21.)).flex().items_center().gap(px(4.)).child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(16.))
                .line_height(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0xffffff))
                .child(profile.name.clone()),
        );
        if matches!(profile.badge, Some(8) | Some(16) | Some(32) | Some(64)) {
            name_row = name_row.child(
                div()
                    .relative()
                    .size(px(18.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("badge-discoverable", 18., theme::TEXT_BRIGHT))
                    .child(div().absolute().size(px(10.)).rounded_full().bg(rgb(theme::BG_CARD)))
                    .child(div().absolute().w(px(2.)).h(px(7.)).bg(rgb(theme::TEXT_BRIGHT))),
            );
        }
        if let Some(tag) = profile.tag.as_deref().filter(|t| !t.is_empty()) {
            name_row = name_row.child(
                div()
                    .h(px(16.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .rounded(px(4.))
                    .bg(rgba(0x97979f3d))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .children(clan_badge.clone().map(|image| {
                        img(ImageSource::Image(image))
                            .size(px(14.))
                            .object_fit(gpui_kit::ObjectFit::Contain)
                    }))
                    .children(
                        clan_badge
                            .is_none()
                            .then(|| icon("tag-award", 14., theme::TEXT)),
                    )
                    .child(tag.to_owned()),
            );
        }

        let mut counts = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(14.))
            .line_height(px(18.))
            .text_color(rgb(theme::TEXT_MUTED));
        if let Some(online) = profile.online_count {
            counts = counts.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(div().size(px(8.)).rounded_full().bg(rgb(theme::ONLINE)))
                    .child(format!("{} Online", group(online))),
            );
        }
        if let Some(members) = profile.member_count {
            counts = counts.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(div().size(px(8.)).rounded_full().bg(rgb(theme::OFFLINE)))
                    .child(format!("{} Members", group(members))),
            );
        }

        let mut body = div()
            .flex()
            .flex_col()
            .px(px(16.))
            .pt(px(6.))
            .pb(px(16.))
            .child(name_row)
            .child(counts)
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(format!("Est. {}", guild.datetime().format("%b %Y"))),
            );
        if let Some(description) = profile.description.as_deref().filter(|d| !d.is_empty()) {
            body = body.child(
                div()
                    .mt(px(10.))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(description.to_owned()),
            );
        }

        div()
            .w(px(282.))
            .flex()
            .flex_col()
            .rounded(px(16.))
            .border_1()
            .border_color(rgba(0x9696a033))
            .bg(rgb(theme::BG_CARD))
            .overflow_hidden()
            .child(banner)
            .child(
                div()
                    .mt(px(-35.))
                    .h(px(70.))
                    .pl(px(19.))
                    .flex()
                    .items_center()
                    .child(avatar(&self.media, &icon_url, 64., &profile.name, cx)),
            )
            .child(body)
            .into_any_element()
    }

    fn terms_body(&self, form: &VerificationForm) -> AnyElement {
        let mut rows = div().flex().flex_col();
        let list: Vec<&str> = form
            .form_fields
            .iter()
            .filter(|f| f.field_type == "TERMS")
            .flat_map(|f| f.values.iter().map(String::as_str))
            .collect();
        let last = list.len().saturating_sub(1);
        for (index, text) in list.iter().enumerate() {
            let mut row = div()
                .px(px(4.))
                .py(px(16.))
                .flex()
                .text_size(px(16.))
                .line_height(px(20.))
                .child(
                    div()
                        .flex_none()
                        .text_color(rgb(theme::TEXT_FAINT))
                        .child(format!("{}.", index + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pl(px(12.))
                        .text_size(px(15.9))
                        .text_color(rgb(theme::TEXT))
                        .child((*text).to_owned()),
                );
            if index == 0 {
                row = row.pt(px(8.));
            }
            if index == last {
                row = row.pb(px(8.));
            } else {
                row = row.border_b_1().border_color(rgba(0x9696a033));
            }
            rows = rows.child(row);
        }

        div()
            .py(px(10.))
            .px(px(17.))
            .rounded(px(8.))
            .bg(rgba(0x97979f1f))
            .child(rows)
            .into_any_element()
    }

    /// The checkbox under the rules. It sits outside the field's padding, so
    /// it lines up with the question title.
    fn agreement_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("screening-agree")
            .h(px(20.))
            .mb(px(8.))
            .opacity(0.63)
            .flex()
            .items_center()
            .gap(px(12.))
            .relative()
            .cursor_pointer()
            .text_size(px(16.))
            .line_height(px(20.))
            .text_color(rgb(theme::TEXT))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_agreed(cx)))
            .child(
                div()
                    .flex_none()
                    .size(px(20.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(if self.agreed {
                        rgba(0x97979f00)
                    } else {
                        rgba(0x97979fcc)
                    })
                    .bg(rgb(if self.agreed {
                        theme::CHECKBOX
                    } else {
                        0x09090b
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(self.agreed.then(|| icon("check", 13., 0xffffff))),
            )
            .child("I have read and agree to the rules")
            .into_any_element()
    }
}

impl Render for RulesScreeningDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(guild) = self.guild else {
            return div().into_any_element();
        };

        let (info, submit_error) = {
            let session = self.session.read(cx);
            let Some(state) = session.verification.as_ref().filter(|v| v.guild == guild) else {
                return div().into_any_element();
            };
            (state.info.clone(), state.submit.clone())
        };

        let Load::Ready(VerificationInfo { profile, form }) = &info else {
            let message = match &info {
                Load::Failed(error) => error.clone(),
                _ => "Loading...".to_owned(),
            };
            let card = div()
                .id("screening-loading")
                .occlude()
                .w(px(1280.))
                .max_w_full()
                .h(px(968.))
                .max_h_full()
                .rounded(px(12.))
                .border_1()
                .border_color(rgba(0x9696a033))
                .bg(rgb(theme::BG_PANEL))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(message);
            return modal(
                card.into_any_element(),
                Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
            );
        };

        let icon_url = profile
            .icon_hash
            .as_deref()
            .map(|hash| cdn::guild_icon(guild, hash, 128))
            .unwrap_or_default();
        let gradient = profile
            .brand_color_primary
            .as_deref()
            .and_then(parse_hex)
            .map(|brand| (theme::BORDER, brand))
            .or_else(|| {
                self.media
                    .update(cx, |media, _| media.gradient_colors(&icon_url))
            })
            .unwrap_or((theme::TEXT_MUTED, theme::STATUS_POSITIVE));

        let card = self.guild_card(guild, profile, cx);
        let can_submit = self.agreed && form.only_terms() && !self.submitting;

        let submit = div()
            .id("screening-submit")
            .w(px(100.))
            .h(px(40.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0xffffff14))
            .bg(rgb(theme::SUCCESS))
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xffffff))
            .when(can_submit, |s| {
                s.cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::SUCCESS_HOVER)))
            })
            .when(!can_submit, |s| s.opacity(0.5))
            .child("Submit")
            .when(can_submit, |s| {
                s.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    let form = this
                        .session
                        .read(cx)
                        .verification
                        .as_ref()
                        .and_then(|v| v.info.ready().map(|i| i.form.clone()));
                    if let Some(form) = form {
                        this.submit(form, cx);
                    }
                }))
            });

        let mut footer = div()
            .h(px(56.))
            .py(px(8.))
            .flex()
            .items_center()
            .justify_end()
            .child(submit);
        if let Some(Err(error)) = submit_error {
            footer = footer.child(
                div()
                    .mr(px(12.))
                    .flex_1()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(theme::DANGER))
                    .child(error),
            );
        }

        let right = div()
            .flex_1()
            .h_full()
            .mr(px(8.))
            .relative()
            .flex()
            .justify_center()
            .child(
                div()
                    .w(px(688.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .pt(px(60.))
                    .pb(px(36.))
                    .px(px(24.))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .relative()
                            .child(
                                div()
                                    .id("screening-scroll")
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .size_full()
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_col()
                                    .pb(px(12.))
                                    .child(
                                        div()
                                            .mt(px(8.))
                                            .mb(px(8.))
                                            .text_size(px(32.))
                                            .line_height(px(40.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(theme::TEXT_BRIGHT))
                                            .child("Before you can talk here..."),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .line_height(px(18.))
                                            .text_color(rgb(theme::TEXT))
                                            .child(
                                                "You\u{2019}ll need to complete the required steps below.",
                                            ),
                                    )
                                    .child(
                                        div()
                                            .mt(px(40.))
                                            .p(px(1.))
                                            .child(
                                                div()
                                                    .mb(px(12.))
                                                    .text_size(px(16.))
                                                    .line_height(px(20.))
                                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                                    .child("Agree to rules"),
                                            )
                                            .child(self.terms_body(form)),
                                    ),
                            ),
                    )
                    .child(self.agreement_row(cx))
                    .child(footer),
            );

        let overlay = div()
            .id("screening-card")
            .occlude()
            .relative()
            .w(px(1280.))
            .max_w_full()
            .h(px(968.))
            .max_h_full()
            .rounded(px(12.))
            .border_1()
            .border_color(rgba(0x9696a033))
            .shadow_lg()
            .bg(rgb(theme::BG_PANEL))
            .flex()
            .child(
                div()
                    .relative()
                    .w(px(419.))
                    .h(px(950.))
                    .mx(px(16.))
                    .my(px(8.))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .opacity(0.33)
                            .bg(linear_gradient(
                                315.,
                                linear_color_stop(rgb(gradient.0), 0.),
                                linear_color_stop(rgb(gradient.1), 1.),
                            )),
                    )
                    .child(card),
            )
            .child(right)
            .child(
                div()
                    .id("screening-close")
                    .absolute()
                    .top(px(32.))
                    .right(px(32.))
                    .size(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
                    .child(icon("x", 20., theme::TEXT_MUTED)),
            );

        modal(
            overlay.into_any_element(),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
        )
    }
}

fn parse_hex(text: &str) -> Option<u32> {
    let text = text.trim_start_matches('#');
    (text.len() == 6)
        .then(|| u32::from_str_radix(text, 16).ok())
        .flatten()
}

/// 182411 becomes "182,411", as the client writes member counts.
fn group(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
