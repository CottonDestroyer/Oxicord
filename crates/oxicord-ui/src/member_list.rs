use gpui_kit::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ListAlignment, ListState,
    ParentElement as _, Render, Styled as _, Subscription, Window, div, list, px, rgb,
};
use oxicord_media::cdn;
use oxicord_model::{Snowflake, Status};
use oxicord_store::{Changes, Entry};

use crate::{
    media::{Media, avatar},
    session::Session,
    theme,
};

const GROUP_HEIGHT: f32 = 36.;
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
            .px_4()
            .pb_1()
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT_MUTED))
            .child(format!("{} \u{2014} {count}", name.to_uppercase()))
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
        let (name, avatar_url, status, color) = {
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
            )
        };

        let offline = status == Status::Offline;
        let name_color = match (offline, color) {
            (true, _) => theme::TEXT_FAINT,
            (false, Some(color)) => color,
            (false, None) => theme::TEXT,
        };

        div()
            .h(px(MEMBER_HEIGHT))
            .flex()
            .items_center()
            .gap_3()
            .mx_2()
            .px_2()
            .rounded(px(6.))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .child(
                div()
                    .relative()
                    .flex_none()
                    .opacity(if offline { 0.5 } else { 1. })
                    .child(avatar(&self.media, &avatar_url, 32., &name, cx))
                    .child(presence_dot(status)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(name_color))
                    .child(name),
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

/// The small colored circle on the corner of an avatar.
pub fn presence_dot(status: Status) -> impl IntoElement {
    let color = match status {
        Status::Online => theme::ONLINE,
        Status::Idle => theme::IDLE,
        Status::Dnd => theme::DND,
        Status::Offline => theme::OFFLINE,
    };
    div()
        .absolute()
        .right(px(-2.))
        .bottom(px(-2.))
        .size(px(12.))
        .rounded_full()
        .border_2()
        .border_color(rgb(theme::BG_MEMBERS))
        .bg(rgb(color))
}
