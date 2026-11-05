use std::collections::{HashMap, HashSet};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, ImageSource, InteractiveElement as _,
    IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, img, px, rgb,
};
use oxicord_gateway::{ClientStatus, ConnectionState};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, RelationshipKind, Snowflake, Status};
use oxicord_store::Unread;

use crate::{
    backend::{Backend, Command},
    chat::ChatView,
    friends::FriendsView,
    icons::icon,
    media::{Media, acronym, avatar},
    member_list::{MemberList, presence_dot},
    session::{DmOpened, Session},
    theme,
};

struct GuildItem {
    id: Snowflake,
    name: String,
    icon: Option<String>,
    unread: Unread,
}

struct ChannelItem {
    id: Snowflake,
    name: String,
    kind: ChannelType,
    unread: Unread,
}

struct GroupItem {
    category: Option<(Snowflake, String)>,
    channels: Vec<ChannelItem>,
}

struct DmItem {
    id: Snowflake,
    name: String,
    avatar: String,
    /// Only for one-to-one conversations.
    status: Option<Status>,
    unread: Unread,
}

/// The signed in frame: guild rail, channel list, and the content area.
pub struct MainView {
    session: Entity<Session>,
    media: Entity<Media>,
    chat: Entity<ChatView>,
    friends: Entity<FriendsView>,
    members: Entity<MemberList>,
    backend: Backend,
    /// `None` is the home screen with the direct messages.
    guild: Option<Snowflake>,
    channel: Option<Snowflake>,
    /// The channel to return to when switching back to a guild.
    last_channel: HashMap<Snowflake, Snowflake>,
    collapsed: HashSet<Snowflake>,
    status_menu_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl MainView {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.subscribe_in(&session, window, |this, _, opened: &DmOpened, window, cx| {
                this.show_dm(opened.0, window, cx);
            }),
        ];
        let chat = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| ChatView::new(session, media, backend, window, cx))
        };
        let members = {
            let (session, media) = (session.clone(), media.clone());
            cx.new(|cx| MemberList::new(session, media, cx))
        };
        let friends = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| FriendsView::new(session, media, backend, cx))
        };
        Self {
            session,
            media,
            chat,
            friends,
            members,
            backend,
            guild: None,
            channel: None,
            last_channel: HashMap::new(),
            collapsed: HashSet::new(),
            status_menu_open: false,
            _subscriptions: subscriptions,
        }
    }

    fn select_guild(
        &mut self,
        guild: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.guild = guild;
        self.channel = guild.and_then(|g| {
            self.last_channel.get(&g).copied().or_else(|| {
                let store = &self.session.read(cx).store;
                store
                    .channel_groups(g)
                    .iter()
                    .flat_map(|group| group.channels.iter())
                    .find(|c| c.kind.has_messages() && !is_voice(c.kind))
                    .map(|c| c.id)
            })
        });
        self.sync_chat(window, cx);
        cx.notify();
    }

    fn select_channel(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.channel = Some(channel);
        if let Some(guild) = self.guild {
            self.last_channel.insert(guild, channel);
        }
        self.sync_chat(window, cx);
        cx.notify();
    }

    /// Goes to a direct message from wherever we are, for example after
    /// pressing the message button on the friends page.
    fn show_dm(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = None;
        self.select_channel(channel, window, cx);
    }

    /// The home screen without a conversation open.
    fn show_friends(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.channel = None;
        self.sync_chat(window, cx);
        cx.notify();
    }

    /// Points the message area at whatever channel is selected now.
    fn sync_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let channel = self.channel;
        self.chat
            .update(cx, |chat, cx| chat.set_channel(channel, window, cx));
        self.sync_members(cx);
    }

    /// Points the member list at the open guild and asks the server for it.
    fn sync_members(&mut self, cx: &mut Context<Self>) {
        let guild = self.guild;
        self.members.update(cx, |members, cx| members.set_guild(guild, cx));

        let show = self.session.read(cx).show_members;
        if let (true, Some(guild), Some(channel)) = (show, self.guild, self.channel) {
            self.backend.send(Command::SubscribeMembers { guild, channel });
        }
    }

    fn toggle_category(&mut self, category: Snowflake, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&category) {
            self.collapsed.insert(category);
        }
        cx.notify();
    }

    fn guild_items(&self, cx: &App) -> Vec<GuildItem> {
        let store = &self.session.read(cx).store;
        store
            .guilds()
            .map(|g| GuildItem {
                id: g.id,
                name: g.name.clone(),
                icon: g.icon.as_deref().map(|hash| cdn::guild_icon(g.id, hash, 96)),
                unread: store.guild_unread(g.id),
            })
            .collect()
    }

    fn dm_items(&self, cx: &App) -> Vec<DmItem> {
        let store = &self.session.read(cx).store;
        store
            .dm_channels()
            .into_iter()
            .map(|c| {
                let avatar = match (c.kind, c.recipients.first()) {
                    (ChannelType::Dm, Some(user)) => {
                        cdn::user_avatar(user.id, user.avatar.as_deref(), 64)
                    }
                    _ => match c.icon.as_deref() {
                        Some(hash) => cdn::group_icon(c.id, hash, 64),
                        None => cdn::default_avatar(c.id),
                    },
                };
                let status = match (c.kind, c.recipients.first()) {
                    (ChannelType::Dm, Some(user)) => Some(store.presence(user.id)),
                    _ => None,
                };
                DmItem {
                    id: c.id,
                    name: store.dm_name(c),
                    avatar,
                    status,
                    unread: store.unread(c.id),
                }
            })
            .collect()
    }

    fn group_items(&self, guild: Snowflake, cx: &App) -> Vec<GroupItem> {
        let store = &self.session.read(cx).store;
        store
            .channel_groups(guild)
            .into_iter()
            .map(|group| GroupItem {
                category: group
                    .category
                    .map(|c| (c.id, c.name.clone().unwrap_or_default())),
                channels: group
                    .channels
                    .into_iter()
                    .map(|c| ChannelItem {
                        id: c.id,
                        name: c.name.clone().unwrap_or_default(),
                        kind: c.kind,
                        unread: store.unread(c.id),
                    })
                    .collect(),
            })
            .collect()
    }

    fn home_unread(&self, cx: &App) -> Unread {
        let store = &self.session.read(cx).store;
        store
            .dm_channels()
            .into_iter()
            .map(|c| store.unread(c.id))
            .fold(Unread::default(), |acc, u| Unread {
                unread: acc.unread || u.unread,
                mentions: acc.mentions + u.mentions,
            })
    }

    fn render_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let guilds = self.guild_items(cx);
        let home_unread = self.home_unread(cx);
        let selected_guild = self.guild;

        let home = rail_button(
            "home",
            0,
            selected_guild.is_none(),
            home_unread,
            div()
                .size(px(48.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(if selected_guild.is_none() { theme::ACCENT } else { theme::BG_CHAT }))
                .rounded(px(if selected_guild.is_none() { 16. } else { 24. }))
                .child(icon("messages-square", 24., theme::TEXT_BRIGHT))
                .into_any_element(),
            cx.listener(|this, _, window, cx| this.select_guild(None, window, cx)),
        );

        let mut list = div().flex().flex_col().gap_2().items_center().w_full();
        for item in guilds {
            let selected = selected_guild == Some(item.id);
            let id = item.id;
            let body = guild_icon(&self.media, item.icon.as_deref(), &item.name, selected, cx);
            list = list.child(rail_button(
                "guild",
                id.0,
                selected,
                item.unread,
                body,
                cx.listener(move |this, _, window, cx| this.select_guild(Some(id), window, cx)),
            ));
        }

        div()
            .w(px(theme::RAIL_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .py_3()
            .bg(rgb(theme::BG_RAIL))
            .child(home)
            .child(div().w(px(32.)).h(px(2.)).rounded_full().bg(rgb(theme::BORDER)))
            .child(
                div()
                    .id("guild-list")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let title: SharedString = match self.guild {
            Some(guild) => self
                .session
                .read(cx)
                .store
                .guild(guild)
                .map(|g| g.name.clone())
                .unwrap_or_default()
                .into(),
            None => "Direct Messages".into(),
        };

        let mut list = div().flex().flex_col().gap(px(1.)).p_2();
        match self.guild {
            Some(guild) => {
                for group in self.group_items(guild, cx) {
                    list = list.child(self.render_group(group, cx));
                }
            }
            None => {
                list = list.child(self.render_friends_entry(cx));
                for dm in self.dm_items(cx) {
                    list = list.child(self.render_dm(dm, cx));
                }
            }
        }

        div()
            .w(px(theme::SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_SIDEBAR))
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .truncate()
                    .child(title),
            )
            .child(
                div()
                    .id("channel-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list),
            )
            .children(self.render_status_menu(cx))
            .child(self.render_user_panel(cx))
    }

    fn render_group(&mut self, group: GroupItem, cx: &mut Context<Self>) -> AnyElement {
        let mut column = div().flex().flex_col().gap(px(1.));

        let hidden = group
            .category
            .as_ref()
            .is_some_and(|(id, _)| self.collapsed.contains(id));

        if let Some((id, name)) = group.category {
            let chevron = if hidden { "chevron-right" } else { "chevron-down" };
            column = column.child(
                div()
                    .id(("category", id.0))
                    .flex()
                    .items_center()
                    .gap_1()
                    .mt_3()
                    .px_1()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .hover(|s| s.text_color(rgb(theme::TEXT)))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_category(id, cx)))
                    .child(icon(chevron, 12., theme::TEXT_MUTED))
                    .child(div().truncate().child(name.to_uppercase())),
            );
        }

        for channel in group.channels {
            // A collapsed category still shows the channel you are in and
            // any with unread messages, as the official client does.
            let selected = self.channel == Some(channel.id);
            if hidden && !selected && !channel.unread.unread {
                continue;
            }
            column = column.child(self.render_channel(channel, selected, cx));
        }
        column.into_any_element()
    }

    fn render_channel(
        &mut self,
        channel: ChannelItem,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = channel.id;
        let glyph = match channel.kind {
            ChannelType::GuildVoice | ChannelType::GuildStageVoice => "volume-2",
            ChannelType::GuildAnnouncement => "megaphone",
            ChannelType::GuildForum => "messages-square",
            ChannelType::GuildMedia => "image",
            _ => "hash",
        };
        let color = if selected || channel.unread.unread {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_MUTED
        };

        div()
            .id(("channel", id.0))
            .h(px(32.))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .rounded(px(6.))
            .cursor_pointer()
            .text_color(rgb(color))
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _, window, cx| this.select_channel(id, window, cx)))
            .child(icon(glyph, 18., color))
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .font_weight(if channel.unread.unread && !selected {
                        gpui_kit::FontWeight::SEMIBOLD
                    } else {
                        gpui_kit::FontWeight::NORMAL
                    })
                    .child(channel.name),
            )
            .children(mention_badge(channel.unread.mentions))
    }

    /// The row above the DM list that opens the friends page.
    fn render_friends_entry(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let selected = self.channel.is_none();
        let pending = self
            .session
            .read(cx)
            .store
            .relationships(RelationshipKind::IncomingRequest)
            .len();
        let color = if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED };

        div()
            .id("friends")
            .h(px(40.))
            .mb_2()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .rounded(px(6.))
            .cursor_pointer()
            .text_color(rgb(color))
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(|this, _, window, cx| this.show_friends(window, cx)))
            .child(icon("users", 20., color))
            .child(div().flex_1().child("Friends"))
            .children(mention_badge(u32::try_from(pending).unwrap_or(u32::MAX)))
    }

    fn render_dm(&mut self, dm: DmItem, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = dm.id;
        let selected = self.channel == Some(id);
        let color = if selected || dm.unread.unread {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_MUTED
        };

        div()
            .id(("dm", id.0))
            .h(px(44.))
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .rounded(px(6.))
            .cursor_pointer()
            .text_color(rgb(color))
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _, window, cx| this.select_channel(id, window, cx)))
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(avatar(&self.media, &dm.avatar, 32., &dm.name, cx))
                    .children(dm.status.map(presence_dot)),
            )
            .child(div().flex_1().truncate().child(dm.name))
            .children(mention_badge(dm.unread.mentions))
    }

    fn set_status(&mut self, status: ClientStatus, cx: &mut Context<Self>) {
        self.status_menu_open = false;
        self.session.update(cx, |session, cx| {
            session.status = status;
            cx.notify();
        });
        self.backend.send(Command::SetStatus(status));
        cx.notify();
    }

    /// The list of statuses that pops up above the user panel.
    fn render_status_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.status_menu_open {
            return None;
        }
        let current = self.session.read(cx).status;

        let mut menu = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(1.))
            .m_2()
            .p_1()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL));

        for status in [
            ClientStatus::Online,
            ClientStatus::Idle,
            ClientStatus::Dnd,
            ClientStatus::Invisible,
        ] {
            let color = status_color(status);
            menu = menu.child(
                div()
                    .id(status.as_str())
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(rgb(if status == current { theme::TEXT_BRIGHT } else { theme::TEXT }))
                    .bg(rgb(if status == current { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_status(status, cx)))
                    .child(div().size(px(10.)).rounded_full().bg(rgb(color)))
                    .child(status_label(status)),
            );
        }
        Some(menu.into_any_element())
    }

    fn render_user_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (name, url) = {
            let store = &self.session.read(cx).store;
            match store.me() {
                Some(me) => (
                    me.display_name().to_owned(),
                    cdn::user_avatar(me.id, me.avatar.as_deref(), 64),
                ),
                None => (String::new(), String::new()),
            }
        };
        let (connection, chosen) = {
            let session = self.session.read(cx);
            (session.connection.clone(), session.status)
        };
        // While connected the useful thing to show is our own status.
        let status = match connection {
            ConnectionState::Connected => status_label(chosen),
            other => connection_label(&other),
        };
        let backend = self.backend.clone();

        div()
            .h(px(56.))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .id("me")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.status_menu_open = !this.status_menu_open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .child(avatar(&self.media, &url, 36., &name, cx))
                            .child(presence_dot(dot_status(chosen))),
                    )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(name),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(status),
                    ),
            ),
            )
            .child(
                div()
                    .id("log-out")
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(move |_, _, _| backend.send(Command::Logout))
                    .child(icon("log-out", 18., theme::TEXT_MUTED)),
            )
    }
}

impl Render for MainView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rail = self.render_rail(cx);
        let sidebar = self.render_sidebar(cx);
        let content = if self.guild.is_none() && self.channel.is_none() {
            self.friends.clone().into_any_element()
        } else {
            self.chat.clone().into_any_element()
        };
        let members = (self.guild.is_some() && self.session.read(cx).show_members)
            .then(|| self.members.clone());

        div()
            .size_full()
            .flex()
            .bg(rgb(theme::BG_RAIL))
            .text_color(rgb(theme::TEXT))
            .child(rail)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .rounded_tl(px(theme::RADIUS))
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .child(sidebar)
                    .child(content)
                    .children(members),
            )
    }
}

/// One entry in the guild rail: the selection pill on the left, the icon,
/// and a red badge for mentions. An unread guild with no mentions gets a
/// small pill instead.
fn rail_button(
    kind: &'static str,
    key: u64,
    selected: bool,
    unread: Unread,
    body: AnyElement,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let pill_height = if selected {
        40.
    } else if unread.unread {
        8.
    } else {
        0.
    };

    div()
        .id((kind, key))
        .relative()
        .w_full()
        .h(px(48.))
        .flex()
        .justify_center()
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .absolute()
                .left(px(0.))
                .top(px((48. - pill_height) / 2.))
                .w(px(4.))
                .h(px(pill_height))
                .rounded_r(px(4.))
                .bg(rgb(theme::TEXT_BRIGHT)),
        )
        .child(body)
        .children(mention_badge(unread.mentions).map(|badge| {
            div().absolute().right(px(10.)).bottom(px(-2.)).child(badge)
        }))
}

fn guild_icon(
    media: &Entity<Media>,
    url: Option<&str>,
    name: &str,
    selected: bool,
    cx: &mut App,
) -> AnyElement {
    let radius = px(if selected { 16. } else { 24. });
    let base = div().size(px(48.)).flex_none().rounded(radius).overflow_hidden();

    let image = url.and_then(|url| media.update(cx, |media, _| media.get(url)));
    match image {
        Some(image) => base
            .child(img(ImageSource::Image(image)).size_full().rounded(radius))
            .into_any_element(),
        None => base
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(if selected { theme::ACCENT } else { theme::BG_CHAT }))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .text_size(px(16.))
            .child(acronym(name))
            .into_any_element(),
    }
}

fn mention_badge(count: u32) -> Option<AnyElement> {
    (count > 0).then(|| {
        div()
            .min_w(px(16.))
            .h(px(16.))
            .px(px(4.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(rgb(theme::BADGE))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .text_size(px(11.))
            .font_weight(gpui_kit::FontWeight::BOLD)
            .child(if count > 99 { "99+".to_owned() } else { count.to_string() })
            .into_any_element()
    })
}

fn is_voice(kind: ChannelType) -> bool {
    matches!(kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice)
}

fn connection_label(state: &ConnectionState) -> &'static str {
    match state {
        ConnectionState::Connecting => "Connecting...",
        ConnectionState::Connected => "Online",
        ConnectionState::Reconnecting { .. } => "Reconnecting...",
        ConnectionState::AuthFailed => "Signed out",
        ConnectionState::Closed(_) => "Disconnected",
    }
}

fn status_label(status: ClientStatus) -> &'static str {
    match status {
        ClientStatus::Online => "Online",
        ClientStatus::Idle => "Idle",
        ClientStatus::Dnd => "Do Not Disturb",
        ClientStatus::Invisible => "Invisible",
    }
}

fn status_color(status: ClientStatus) -> u32 {
    match status {
        ClientStatus::Online => theme::ONLINE,
        ClientStatus::Idle => theme::IDLE,
        ClientStatus::Dnd => theme::DND,
        ClientStatus::Invisible => theme::OFFLINE,
    }
}

/// How a chosen status looks as a dot. Invisible shows as offline, which is
/// what everyone else sees.
fn dot_status(status: ClientStatus) -> Status {
    match status {
        ClientStatus::Online => Status::Online,
        ClientStatus::Idle => Status::Idle,
        ClientStatus::Dnd => Status::Dnd,
        ClientStatus::Invisible => Status::Offline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invisible_looks_offline_and_the_rest_look_like_themselves() {
        assert_eq!(dot_status(ClientStatus::Invisible), Status::Offline);
        assert_eq!(dot_status(ClientStatus::Online), Status::Online);
        assert_eq!(dot_status(ClientStatus::Idle), Status::Idle);
        assert_eq!(dot_status(ClientStatus::Dnd), Status::Dnd);
    }

    #[test]
    fn every_status_has_its_own_label_and_color() {
        let all = [
            ClientStatus::Online,
            ClientStatus::Idle,
            ClientStatus::Dnd,
            ClientStatus::Invisible,
        ];
        let labels: HashSet<_> = all.iter().map(|s| status_label(*s)).collect();
        let colors: HashSet<_> = all.iter().map(|s| status_color(*s)).collect();
        assert_eq!(labels.len(), 4);
        assert_eq!(colors.len(), 4);
    }
}
