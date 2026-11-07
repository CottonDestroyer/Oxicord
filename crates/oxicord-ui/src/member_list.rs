use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Div, Entity, FontWeight, InteractiveElement as _, IntoElement, StyledImage as _,
    ListAlignment, ListOffset, ListState, MouseButton, MouseDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, list, px, rgb,
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_media::cdn;
use oxicord_model::{Snowflake, Status};
use oxicord_store::{Changes, Entry};

use crate::{
    icons::icon,
    media::{Media, avatar},
    session::{OpenUserMenu, Session},
    theme,
};

const GROUP_HEIGHT: f32 = 42.;
const MEMBER_HEIGHT: f32 = 44.;

/// The people in the open guild, grouped by role or online state, as the
/// server lays the list out.
pub struct MemberList {
    session: Entity<Session>,
    media: Entity<Media>,
    guild: Option<Snowflake>,
    list: ListState,
    /// How many slots the list state believes it has.
    slots: usize,
    _subscriptions: Vec<Subscription>,
}

impl MemberList {
    pub fn new(session: Entity<Session>, media: Entity<Media>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.subscribe(&session, |this, _, changes: &Changes, cx| {
                if changes.full || this.guild.is_some_and(|g| changes.members.contains(&g)) {
                    this.sync(cx);
                } else if changes.presences {
                    cx.notify();
                }
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
        ];
        Self {
            session,
            media,
            guild: None,
            list: ListState::new(0, ListAlignment::Top, px(400.)),
            slots: 0,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_guild(&mut self, guild: Option<Snowflake>, cx: &mut Context<Self>) {
        if self.guild != guild {
            self.guild = guild;
            self.slots = 0;
            self.list.reset(0);
        }
        self.sync(cx);
    }

    /// Resizes the list to the slots the store holds. Rows are drawn from
    /// the store on demand, so only the count and the measurements matter.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let len = self
            .guild
            .and_then(|g| self.session.read(cx).store.member_list(g))
            .map_or(0, |l| l.entries().len());

        match len.cmp(&self.slots) {
            std::cmp::Ordering::Greater => self.list.splice(self.slots..self.slots, len - self.slots),
            std::cmp::Ordering::Less => self.list.splice(len..self.slots, 0),
            std::cmp::Ordering::Equal => {}
        }
        // A slot may have changed from a header to a member or back.
        self.list.remeasure();
        // A list that has just been filled starts at its first row.
        if self.slots == 0 && len > 0 {
            self.list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
        }
        self.slots = len;
        cx.notify();
    }

    fn render_entry(&mut self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else {
            return div().into_any_element();
        };
        let entry = self
            .session
            .read(cx)
            .store
            .member_list(guild)
            .and_then(|l| l.entries().get(ix).cloned())
            .flatten();

        match entry {
            Some(Entry::Group { id, count }) => {
                let count = self
                    .session
                    .read(cx)
                    .store
                    .member_list(guild)
                    .map_or(count, |l| l.group_count(&id, count));
                self.render_group(guild, &id, count, cx)
            }
            Some(Entry::Member { user, nick, roles }) => {
                self.render_member(guild, user, nick, &roles, cx)
            }
            // Not sent yet. Keep the height so the scrollbar doesn't jump.
            None => div().h(px(MEMBER_HEIGHT)).into_any_element(),
        }
    }

    fn render_group(&self, guild: Snowflake, id: &str, count: u64, cx: &App) -> AnyElement {
        let name = match id {
            "online" => "Online".to_owned(),
            "offline" => "Offline".to_owned(),
            role => role
                .parse::<u64>()
                .ok()
                .and_then(|role| {
                    let store = &self.session.read(cx).store;
                    let role = store.guild(guild)?.roles.iter().find(|r| r.id.0 == role)?;
                    Some(role.name.clone())
                })
                .unwrap_or_else(|| "Members".to_owned()),
        };

        div()
            .h(px(GROUP_HEIGHT))
            .flex()
            .items_end()
            .pl(px(16.))
            .pb(px(4.))
            .text_size(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::CHANNELS_DEFAULT))
            .child(format!("{name} \u{2014} {count}"))
            .into_any_element()
    }

    fn render_member(
        &mut self,
        guild: Snowflake,
        user: Snowflake,
        nick: Option<String>,
        roles: &[Snowflake],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (name, avatar_url, status, color, bot, line, in_voice, decoration, nameplate) = {
            let store = &self.session.read(cx).store;
            let Some(account) = store.user(user) else {
                return div().h(px(MEMBER_HEIGHT)).into_any_element();
            };
            (
                nick.filter(|n| !n.is_empty())
                    .unwrap_or_else(|| account.display_name().to_owned()),
                cdn::user_avatar(account.id, account.avatar.as_deref(), 64),
                store.presence(user),
                store.member_color(guild, roles),
                account.bot,
                crate::friends::status_line(store.activities(user)),
                store.voice_of(user).is_some_and(|v| v.channel_id.is_some()),
                account.decoration(),
                account.nameplate(),
            )
        };

        let offline = status == Status::Offline;
        let name_color = color.unwrap_or(theme::CHANNELS_DEFAULT);

        // The 44px slot holds a 42px row with a pixel of air above and
        // below, as the official list does.
        div()
            .h(px(MEMBER_HEIGHT))
            .px(px(8.))
            .py(px(1.))
            .child(
                div()
                    .id(("member", user.0))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.session.update(cx, |_, cx| {
                            cx.emit(OpenUserMenu { user, guild: Some(guild), at: event.position, dm: None, can_mention: true });
                        });
                    }))
                    .size_full()
                    .relative()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(8.))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    // A nameplate is a banner behind the name, from the avatar's middle to the edge.
                    .children(nameplate.and_then(|sku| {
                        let image = self.media.update(cx, |m, _| m.get(&cdn::collectible(sku)))?;
                        Some(
                            div().absolute().top_0().bottom_0().left(px(24.)).right_0().child(
                                gpui_kit::img(gpui_kit::ImageSource::Image(image))
                                    .size_full()
                                    .object_fit(gpui_kit::ObjectFit::Cover),
                            ),
                        )
                    }))
                    .font_weight(FontWeight::MEDIUM)
                    // Offline people fade out until the pointer is on them.
                    .when(offline, |row| row.opacity(0.3).hover(|s| s.opacity(1.).bg(rgb(theme::BG_HOVER))))
                    .when(!offline, |row| row.hover(|s| s.bg(rgb(theme::BG_HOVER))))
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        this.session
                            .update(cx, |session, cx| session.show_profile(user, Some(guild), Some(event.position()), cx));
                    }))
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .child(avatar(&self.media, &avatar_url, 32., &name, cx))
                            .children(decoration.and_then(|sku| crate::media::decoration(&self.media, sku, 32., cx)))
                            .child(presence_dot(status)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(div().min_w_0().truncate().text_color(rgb(name_color)).child(name))
                                    .children(bot.then(app_badge))
                                    .children(in_voice.then(|| icon("volume-2", 12., theme::TEXT_MUTED))),
                            )
                            .children(line.map(|line| {
                                div()
                                    .truncate()
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(line)
                            })),
                    ),
            )
            .into_any_element()
    }
}

impl Render for MemberList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(theme::MEMBERS_WIDTH))
            .h_full()
            .flex_none()
            .bg(rgb(theme::BG_MEMBERS))
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .child(
                list(
                    self.list.clone(),
                    cx.processor(|this, ix, _, cx| this.render_entry(ix, cx)),
                )
                .size_full(),
            )
    }
}

/// The tag after a bot's name.
pub fn app_badge() -> Div {
    div()
        .flex_none()
        .h(px(15.))
        .px(px(4.))
        .flex()
        .items_center()
        .rounded(px(4.))
        .bg(rgb(theme::ACCENT))
        .text_size(px(10.))
        .line_height(px(15.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child("APP")
}

/// The small status mark on the corner of an avatar.
pub fn presence_dot(status: Status) -> impl IntoElement {
    div()
        .absolute()
        .right(px(-2.))
        .bottom(px(-2.))
        .size(px(12.))
        .rounded_full()
        .bg(rgb(theme::BG_MEMBERS))
        .p(px(2.))
        .child(status_shape(status, 8., theme::BG_MEMBERS))
}

/// A status as Discord draws it: online is a full disc, idle a crescent,
/// do not disturb a disc with a bar and offline a ring. The cut-outs are
/// painted in `backdrop`, the color behind the mark.
pub fn status_shape(status: Status, size: f32, backdrop: u32) -> Div {
    let color = match status {
        Status::Online => theme::ONLINE,
        Status::Idle => theme::IDLE,
        Status::Dnd => theme::DND,
        Status::Offline => theme::OFFLINE,
    };
    let disc = div().relative().flex_none().size(px(size)).rounded_full().bg(rgb(color)).overflow_hidden();
    let cut = |x: f32, y: f32, w: f32, h: f32| {
        div().absolute().left(px(size * x)).top(px(size * y)).w(px(size * w)).h(px(size * h)).rounded_full().bg(rgb(backdrop))
    };
    match status {
        Status::Online => disc,
        Status::Idle => disc.child(cut(-0.15, -0.15, 0.65, 0.65)),
        // Do Not Disturb: a solid red disc with a white bar in the middle,
        // matching Discord (the bar is white, not a hole in the background).
        Status::Dnd => disc.child(
            div()
                .absolute()
                .left(px(size * 0.2))
                .top(px(size * 0.42))
                .w(px(size * 0.6))
                .h(px(size * 0.16))
                .rounded_full()
                .bg(rgb(0xffffff)),
        ),
        Status::Offline => disc.child(cut(0.25, 0.25, 0.5, 0.5)),
    }
}
