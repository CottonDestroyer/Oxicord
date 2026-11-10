//! The channel settings screen, opened full screen like the live client's
//! "Edit Channel". Only the Permissions tab is built here: the sync banner,
//! the Private Channel card, and under "Advanced permissions" the list of
//! roles and members that have an overwrite beside the allow, inherit and deny
//! controls for the selected one. Measured from the live page at 1920 x 1080
//! (stable 670026): a 264 wide navigation column, a 660 wide content column 48
//! to its right, and the close button 40 past that.

use gpui_kit::{
    AnyElement, ClickEvent, Context, Div, ElementId, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_model::{ChannelType, Overwrite, Snowflake};

use crate::{
    channel_permissions::{self as perms, Tri},
    icons::icon,
    scroll,
    session::Session,
    theme,
};

const LINK: u32 = 0x00a8fc;
const HELP_URL: &str = "https://support.discord.com/hc/en-us/articles/206029707";
const GREEN: u32 = 0x008545;
const RED: u32 = 0xd22d39;

/// What the screen asks of whoever hosts it.
pub enum ChannelSettingsEvent {
    Close,
    /// The Overview tab: the name and topic form, which is the older dialog.
    Overview(Snowflake),
}

impl EventEmitter<ChannelSettingsEvent> for ChannelSettings {}

/// Asked of the session to open this screen on a channel.
pub struct OpenChannelPermissions(pub Snowflake);

impl EventEmitter<OpenChannelPermissions> for Session {}

pub struct ChannelSettings {
    session: Entity<Session>,
    channel: Option<Snowflake>,
    /// The overwrite shown on the right: a role or member id, `@everyone`
    /// being the server's own id.
    selected: Option<Snowflake>,
    advanced: bool,
    scroll: ScrollHandle,
}

/// One row of the Roles/Members list.
struct Entry {
    id: Snowflake,
    name: String,
    /// A role's colour, drawn as a dot. Members and plain roles have none.
    color: Option<u32>,
}

impl ChannelSettings {
    pub fn new(session: Entity<Session>) -> Self {
        Self { session, channel: None, selected: None, advanced: false, scroll: ScrollHandle::new() }
    }

    pub fn opened(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        self.channel = Some(channel);
        // `@everyone` is always there to pick, and it is the id of the server.
        self.selected = self.guild_of(channel, cx);
        self.advanced = false;
        self.scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        cx.notify();
    }

    fn guild_of(&self, channel: Snowflake, cx: &Context<Self>) -> Option<Snowflake> {
        self.session.read(cx).store.channel(channel).and_then(|c| c.guild_id)
    }

    /// The roles and members that carry an overwrite, roles first and in the
    /// server's order, then members by name, with `@everyone` always last.
    fn entries(&self, channel: Snowflake, guild: Snowflake, overwrites: &[Overwrite], cx: &Context<Self>) -> Vec<Entry> {
        let session = self.session.read(cx);
        let roles = session.store.guild(guild).map(|g| g.roles.clone()).unwrap_or_default();
        let mut listed: Vec<(i32, Entry)> = Vec::new();
        let mut members: Vec<Entry> = Vec::new();
        for overwrite in overwrites {
            if perms::is_role(overwrite) {
                if overwrite.id == guild {
                    continue;
                }
                let role = roles.iter().find(|r| r.id == overwrite.id);
                listed.push((
                    role.map_or(0, |r| r.position),
                    Entry {
                        id: overwrite.id,
                        name: role.map_or_else(|| "Unknown role".to_owned(), |r| r.name.clone()),
                        color: role.and_then(|r| (r.color != 0).then_some(r.color)),
                    },
                ));
            } else {
                members.push(Entry {
                    id: overwrite.id,
                    name: session
                        .store
                        .user(overwrite.id)
                        .map_or_else(|| "Unknown member".to_owned(), |u| u.display_name().to_owned()),
                    color: None,
                });
            }
        }
        let _ = channel;
        listed.sort_by(|a, b| b.0.cmp(&a.0));
        members.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        let mut all: Vec<Entry> = listed.into_iter().map(|(_, entry)| entry).collect();
        all.extend(members);
        all.push(Entry { id: guild, name: "@everyone".to_owned(), color: None });
        all
    }

    fn nav_item(label: &'static str, selected: bool) -> Div {
        div()
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .rounded(px(4.))
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
            .when(selected, |item| item.bg(rgba(0x97979f3d)))
            .child(label)
    }

    /// The segmented control: deny, inherit, allow. Display only for now.
    fn tri(state: Tri, disabled: bool) -> Div {
        let segment = |glyph: AnyElement, picked: bool, fill: u32, left: bool, right: bool| {
            div()
                .w(px(31.))
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .bg(if picked { rgb(fill) } else { rgb(0x000000) })
                .when(left, |s| s.rounded_l(px(8.)))
                .when(right, |s| s.rounded_r(px(8.)))
                .child(glyph)
        };
        let deny = state == Tri::Deny;
        let allow = state == Tri::Allow;
        let inherit = state == Tri::Inherit;
        let cross = icon("x", 16., if deny { 0xffffff } else { theme::DANGER }).into_any_element();
        let slash = div()
            .text_size(px(18.))
            .line_height(px(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child("/")
            .into_any_element();
        let tick = icon("check", 16., if allow { 0xffffff } else { theme::STATUS_POSITIVE }).into_any_element();
        div()
            .w(px(96.))
            .h(px(28.))
            .flex_none()
            .flex()
            .overflow_hidden()
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f29))
            .when(disabled, |group| group.opacity(0.4))
            .child(segment(cross, deny, RED, true, false))
            .child(segment(slash, inherit, 0x2f2f33, false, false))
            .child(segment(tick, allow, GREEN, false, true))
    }

    fn permission_row(perm: &'static perms::Perm, state: Tri, disabled: bool) -> Div {
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(24.))
            .child(
                div()
                    .w(px(293.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(perm.name),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(18.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(perm.description),
                    )
                    .when(disabled, |text| {
                        text.child(
                            div()
                                .mt(px(4.))
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(theme::DANGER))
                                .child("You cannot modify this permission because none of your roles have it."),
                        )
                    }),
            )
            .child(Self::tri(state, disabled))
    }

    fn divider() -> Div {
        div().h(px(1.)).w_full().flex_none().bg(rgba(0x97979f29))
    }

    fn switch(on: bool) -> Div {
        div()
            .w(px(48.))
            .h(px(24.))
            .flex_none()
            .flex()
            .items_center()
            .px(px(3.))
            .rounded(px(16.))
            .border_1()
            .border_color(rgba(0x97979f33))
            .bg(if on { rgb(theme::STATUS_POSITIVE) } else { rgba(0x0000001f) })
            .when(on, |track| track.justify_end())
            .child(div().size(px(16.)).rounded_full().bg(rgb(0xffffff)))
    }

    fn card() -> Div {
        div()
            .w_full()
            .p(px(16.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f29))
            .bg(rgb(theme::BG_CARD))
    }

    fn permissions(&mut self, channel: Snowflake, cx: &mut Context<Self>) -> Div {
        let (kind, guild, overwrites, category, mine) = {
            let store = &self.session.read(cx).store;
            let Some(c) = store.channel(channel) else { return div() };
            let category = c.parent_id.and_then(|id| store.channel(id)).map(|p| {
                (p.name.clone().unwrap_or_default(), p.permission_overwrites.clone())
            });
            (c.kind, c.guild_id, c.permission_overwrites.clone(), category, store.permissions_in(c))
        };
        let Some(guild) = guild else { return div() };
        let private = perms::is_private(&overwrites, guild);
        let entries = self.entries(channel, guild, &overwrites, cx);
        let selected = self.selected.filter(|id| entries.iter().any(|e| e.id == *id)).unwrap_or(guild);
        let current = perms::find(&overwrites, selected);

        let banner = category.as_ref().map(|(name, category_overwrites)| {
            let synced = perms::synced_with(&overwrites, category_overwrites);
            Self::card()
                .mt(px(16.))
                .py(px(16.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(icon(if synced { "check-check" } else { "triangle-alert" }, 20., if synced { theme::STATUS_POSITIVE } else { 0xf0b232 }))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .gap(px(4.))
                        .text_size(px(16.))
                        .line_height(px(20.))
                        .text_color(rgb(theme::TEXT))
                        .child(if synced { "Permissions synced with category:" } else { "Permissions not synced with category:" })
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(name.clone())),
                )
                .child(
                    div()
                        .h(px(32.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .rounded(px(8.))
                        .bg(rgba(0x97979f1f))
                        .text_size(px(14.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xdcdcdf))
                        .when(synced, |button| button.opacity(0.5))
                        .child("Sync Now"),
                )
        });

        let private_card = Self::card()
            .mt(px(16.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon("lock", 20., theme::TEXT_BRIGHT))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT))
                            .child("Private Channel"),
                    )
                    .child(Self::switch(private)),
            )
            .child(
                div()
                    .mt(px(8.))
                    .pl(px(4.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(theme::TEXT))
                    .child("By making a channel private, only select members and roles will be able to view this channel."),
            );

        let heading = div()
            .id("advanced-permissions")
            .mt(px(32.))
            .flex()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.advanced = !this.advanced;
                cx.notify();
            }))
            .child(
                div()
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .child("Advanced permissions"),
            )
            .child(icon(if self.advanced { "chevron-down" } else { "chevron-right" }, 20., theme::TEXT_MUTED));

        let advanced = self.advanced.then(|| {
            let list = entries.iter().fold(div().mt(px(6.)).flex().flex_col(), |list, entry| {
                let picked = entry.id == selected;
                let id = entry.id;
                list.child(
                    div()
                        .id(ElementId::Name(format!("overwrite-{}", entry.id).into()))
                        .h(px(32.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(16.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(if picked { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                        .when(picked, |row| row.bg(rgba(0x97979f3d)))
                        .hover(|row| row.bg(rgba(0x97979f1f)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.selected = Some(id);
                            cx.notify();
                        }))
                        .children(entry.color.map(|color| div().size(px(12.)).flex_none().rounded_full().bg(rgb(color))))
                        .child(div().truncate().child(SharedString::from(entry.name.clone()))),
                )
            });

            let left = div()
                .w(px(209.))
                .flex_none()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0x8d8e95))
                        .child("Roles/Members")
                        .child(icon("circle-plus", 16., 0x8d8e95)),
                )
                .child(list)
                .child(div().mt(px(20.)).child(Self::divider()))
                .child(
                    div()
                        .id("permissions-help")
                        .mt(px(15.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .line_height(px(18.))
                        .text_color(rgb(LINK))
                        .on_click(|_: &ClickEvent, _, cx| cx.open_url(HELP_URL))
                        .child("Need help with permissions?"),
                );

            let mut right = div().w(px(413.)).flex_none().flex().flex_col();
            for (index, group) in perms::catalog(kind).iter().enumerate() {
                right = right.child(
                    div()
                        .when(index > 0, |legend| legend.mt(px(40.)))
                        .mb(px(12.))
                        .text_size(px(20.))
                        .line_height(px(24.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(group.title),
                );
                for (at, perm) in group.perms.iter().enumerate() {
                    if at > 0 {
                        right = right.child(div().my(px(24.)).child(Self::divider()));
                    }
                    // Nobody can hand out what they lack, which the client
                    // says on the row instead of hiding it.
                    let disabled = !mine.contains(perm.flag);
                    right = right.child(Self::permission_row(perm, perms::state_of(current, perm.flag), disabled));
                }
            }

            div().mt(px(24.)).flex().gap(px(23.)).child(left).child(right)
        });

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Channel Permissions"),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::TEXT))
                    .child("Use permissions to customize who can do what in this channel."),
            )
            .children(banner)
            .child(private_card)
            .child(div().mt(px(40.)).child(Self::divider()))
            .child(heading)
            .children(advanced)
            .pb(px(60.))
    }
}

impl Render for ChannelSettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(channel) = self.channel else { return div().into_any_element() };
        let (name, category, kind) = {
            let store = &self.session.read(cx).store;
            let Some(c) = store.channel(channel) else { return div().into_any_element() };
            (
                c.name.clone().unwrap_or_default(),
                c.parent_id.and_then(|id| store.channel(id)).and_then(|p| p.name.clone()),
                c.kind,
            )
        };
        let glyph = if matches!(kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice) { "volume-2" } else { "hash" };

        let nav = div()
            .w(px(264.))
            .flex_none()
            .pt(px(60.))
            .pl(px(20.))
            .pr(px(6.))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(16.))
                    .mb(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(icon(glyph, 12., theme::TEXT_MUTED))
                    .child(SharedString::from(name))
                    .children(category.map(|text| {
                        div().font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(SharedString::from(text))
                    })),
            )
            .child(
                div()
                    .id("channel-overview")
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(ChannelSettingsEvent::Overview(channel))))
                    .child(Self::nav_item("Overview", false)),
            )
            .child(div().mt(px(2.)).child(Self::nav_item("Permissions", true)))
            .child(div().mt(px(2.)).child(Self::nav_item("Invites", false)))
            .child(div().mt(px(2.)).child(Self::nav_item("Integrations", false)))
            .child(div().my(px(8.)).mx(px(10.)).child(Self::divider()))
            .child(
                div()
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::DANGER))
                    .child("Delete Channel")
                    .child(icon("trash", 16., theme::DANGER)),
            );

        let body = self.permissions(channel, cx);
        let close = div()
            .pt(px(60.))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .child(
                div()
                    .id("close-channel-settings")
                    .size(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(theme::TEXT_MUTED))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0x97979f1f)))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(ChannelSettingsEvent::Close)))
                    .child(icon("x", 18., theme::TEXT_MUTED)),
            )
            .child(div().text_size(px(12.)).line_height(px(16.)).font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_MUTED)).child("ESC"));

        div()
            .id("channel-settings")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .w(px(1080.))
                    .h_full()
                    .flex()
                    .child(nav)
                    .child(
                        div()
                            .w(px(660.))
                            .ml(px(48.))
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(scroll::vertical("channel-settings-body", &self.scroll, div().pt(px(60.)).child(body))),
                    )
                    .child(div().ml(px(40.)).child(close)),
            )
            .into_any_element()
    }
}
