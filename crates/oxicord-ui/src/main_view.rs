use std::collections::{HashMap, HashSet};

use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, ImageSource, InteractiveElement as _,
    IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    FocusHandle, MouseButton, MouseDownEvent, Pixels, Point, Subscription, Window, div, img, point, px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_gateway::{ClientStatus, ConnectionState};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, RelationshipKind, Snowflake, Status};
use oxicord_store::Unread;

use crate::{
    account::Load,
    backend::{Backend, Command},
    chat::ChatView,
    context_menu::{CloseMenu, ContextMenu, Target},
    dm_picker::{ClosePicker, DmPicker, Purpose},
    forum::ForumView,
    join_server::{CloseJoin, JoinServerView},
    friends::FriendsView,
    icons::icon,
    rail::{self, Entry},
    inbox::{CloseInbox, Inbox},
    keys::QuickSwitch,
    media::{Media, ViewImage, acronym, avatar},
    member_list::{MemberList, presence_dot},
    popup::{ClosePopup, PopupMenu},
    profile,
    session::{DmOpened, JumpTo, MentionUser, OpenInbox, OpenMenu, OpenPanel, OpenPicker, OpenPopup, OpenUserMenu, PanelKind, Session, ShowProfile},
    settings_view::{CloseSettings, SettingsView},
    side_panel::{ClosePanel, SidePanel},
    switcher::{Chosen, CloseSwitcher, Goal, QuickSwitcher},
    theme, user_menu,
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
    muted: bool,
    /// Threads shown beneath this channel.
    threads: Vec<ChannelItem>,
}

struct GroupItem {
    category: Option<(Snowflake, String)>,
    channels: Vec<ChannelItem>,
}

struct DmItem {
    /// People in a group conversation, us included.
    members: Option<usize>,
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
    forum: Entity<ForumView>,
    members: Entity<MemberList>,
    backend: Backend,
    /// `None` is the home screen with the direct messages.
    guild: Option<Snowflake>,
    channel: Option<Snowflake>,
    /// The channel to return to when switching back to a guild.
    last_channel: HashMap<Snowflake, Snowflake>,
    collapsed: HashSet<Snowflake>,
    status_menu_open: bool,
    /// The profile card on screen, if any.
    profile: Option<ShowProfile>,
    settings_open: bool,
    settings_view: Entity<SettingsView>,
    panel: Entity<SidePanel>,
    panel_open: bool,
    menu: Entity<ContextMenu>,
    menu_open: bool,
    /// The picture open in the full size viewer.
    viewer: Option<String>,
    join: Entity<JoinServerView>,
    join_open: bool,
    switcher: Entity<QuickSwitcher>,
    switcher_open: bool,
    picker: Entity<DmPicker>,
    popup: Entity<PopupMenu>,
    popup_open: bool,
    /// Folders opened in the rail.
    expanded: HashSet<u64>,
    picker_open: bool,
    inbox: Entity<Inbox>,
    inbox_open: bool,
    /// Held by the root so shortcuts work when nothing else has focus.
    focus: FocusHandle,
    /// Whether the member list was showing at the last redraw, so turning it
    /// on asks the server for the list.
    members_shown: bool,
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
        let settings_view = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| SettingsView::new(session, media, backend, window, cx))
        };
        // The chat reads several preferences, so fetch them up front.
        session.update(cx, |s, cx| {
            s.request_account(&backend, crate::AccountRequest::LoadSettings, cx);
        });
        let panel = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| SidePanel::new(session, media, backend, window, cx))
        };
        let menu = cx.new(|_| ContextMenu::new(session.clone(), backend.clone()));
        let join = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| JoinServerView::new(session, media, backend, window, cx))
        };
        let switcher = {
            let (session, media) = (session.clone(), media.clone());
            cx.new(|cx| QuickSwitcher::new(session, media, window, cx))
        };
        let picker = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| DmPicker::new(session, media, backend, window, cx))
        };
        let inbox = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| Inbox::new(session, media, backend, cx))
        };
        let popup = cx.new(|_| PopupMenu::default());
        let members_shown = session.read(cx).settings.show_members;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let subscriptions = vec![
            cx.subscribe(&session, |this, _, open: &OpenUserMenu, cx| this.open_user_menu(open, cx)),
            cx.subscribe(&popup, |this, _, _: &ClosePopup, cx| {
                this.popup_open = false;
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, open: &OpenPopup, cx| {
                this.popup_open = true;
                let (at, entries, pick) = (open.at, open.entries.clone(), open.pick.clone());
                this.popup.update(cx, |popup, cx| popup.show(at, entries, pick, cx));
                cx.notify();
            }),
            cx.subscribe(&inbox, |this, _, _: &CloseInbox, cx| {
                this.inbox_open = false;
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, _: &OpenInbox, cx| {
                this.inbox_open = true;
                this.inbox.update(cx, |inbox, cx| inbox.opened(cx));
                cx.notify();
            }),
            cx.subscribe(&picker, |this, _, _: &ClosePicker, cx| {
                this.picker_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenPicker, window, cx| {
                this.open_picker(open.purpose, open.at, window, cx);
            }),
            cx.subscribe(&switcher, |this, _, _: &CloseSwitcher, cx| {
                this.switcher_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&switcher, window, |this, _, chosen: &Chosen, window, cx| {
                this.go_to(chosen.0, window, cx);
            }),
            cx.observe(&session, |this, session, cx| {
                let show = session.read(cx).settings.show_members;
                if show && !this.members_shown {
                    this.sync_members(cx);
                }
                this.members_shown = show;
                cx.notify();
            }),
            cx.subscribe(&settings_view, |this, _, _: &CloseSettings, cx| {
                this.settings_open = false;
                cx.notify();
            }),
            cx.subscribe(&media, |this, _, shown: &ViewImage, cx| {
                this.viewer = Some(shown.0.clone());
                cx.notify();
            }),
            cx.subscribe(&join, |this, _, _: &CloseJoin, cx| {
                this.join_open = false;
                cx.notify();
            }),
            cx.subscribe(&menu, |this, _, _: &CloseMenu, cx| {
                this.menu_open = false;
                cx.notify();
            }),
            cx.subscribe(&panel, |this, _, _: &ClosePanel, cx| {
                this.panel_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenPanel, window, cx| {
                this.open_panel(open.0, window, cx);
            }),
            cx.subscribe_in(&session, window, |this, _, jump: &JumpTo, window, cx| {
                this.show_channel(jump.channel, jump.message, window, cx);
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.subscribe_in(&session, window, |this, _, opened: &DmOpened, window, cx| {
                this.show_dm(opened.0, window, cx);
            }),
            cx.subscribe(&session, |this, _, open: &OpenMenu, cx| {
                this.open_menu(open.target, open.at, cx);
            }),
            cx.subscribe(&session, |this, _, shown: &ShowProfile, cx| {
                this.open_profile(shown.user, shown.guild, cx);
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
            cx.new(|cx| FriendsView::new(session, media, backend, window, cx))
        };
        let forum = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| ForumView::new(session, media, backend, window, cx))
        };
        Self {
            session,
            media,
            chat,
            friends,
            forum,
            members,
            backend,
            guild: None,
            channel: None,
            last_channel: HashMap::new(),
            collapsed: HashSet::new(),
            status_menu_open: false,
            profile: None,
            settings_open: false,
            settings_view,
            members_shown,
            panel,
            panel_open: false,
            menu,
            menu_open: false,
            viewer: None,
            join,
            join_open: false,
            switcher,
            switcher_open: false,
            picker,
            popup,
            popup_open: false,
            expanded: HashSet::new(),
            picker_open: false,
            inbox,
            inbox_open: false,
            focus,
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

    fn open_picker(&mut self, purpose: Purpose, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.picker_open = true;
        self.picker.update(cx, |p, cx| p.show(purpose, at, window, cx));
        cx.notify();
    }

    /// Hides a conversation. When it is the one on screen, falls back to the
    /// friends page.
    fn close_dm(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.backend.send(Command::CloseDm(channel));
        if self.channel == Some(channel) {
            self.show_friends(window, cx);
        }
    }

    fn open_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.switcher_open = true;
        self.switcher.update(cx, |s, cx| s.opened(window, cx));
        cx.notify();
    }

    /// Follows a quick switcher result.
    fn go_to(&mut self, goal: Goal, window: &mut Window, cx: &mut Context<Self>) {
        self.switcher_open = false;
        match goal {
            Goal::Dm(channel) => self.show_dm(channel, window, cx),
            Goal::Channel(channel) => self.show_channel(channel, None, window, cx),
            Goal::Guild(guild) => self.select_guild(Some(guild), window, cx),
            // The conversation opens through `DmOpened` once it exists.
            Goal::User(user) => self.backend.send(Command::OpenDm(user)),
        }
        cx.notify();
    }

    fn open_menu(&mut self, target: Target, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.menu_open = true;
        self.menu.update(cx, |menu, cx| menu.show(target, at, cx));
        cx.notify();
    }

    /// The picture viewer: the image as large as fits, with a way to open
    /// the original in the browser.
    fn render_viewer(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let url = self.viewer.clone()?;
        let image = self.media.update(cx, |media, _| media.get(&url));

        let picture = match image {
            Some(image) => img(ImageSource::Image(image)).max_w_full().max_h_full().into_any_element(),
            None => div().text_color(rgb(theme::TEXT_MUTED)).child("Loading...").into_any_element(),
        };
        let open = url.clone();
        let content = div()
            .id("viewer")
            .occlude()
            .relative()
            .size_full()
            .p_8()
            .flex()
            .items_center()
            .justify_center()
            .child(picture)
            .child(
                div()
                    .absolute()
                    .top(px(16.))
                    .right(px(16.))
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("viewer-open")
                            .size(px(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(rgb(theme::BG_PANEL))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| cx.open_url(&open))
                            .child(icon("external-link", 18., theme::TEXT)),
                    )
                    .child(
                        div()
                            .id("viewer-close")
                            .size(px(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(rgb(theme::BG_PANEL))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.viewer = None;
                                cx.notify();
                            }))
                            .child(icon("x", 18., theme::TEXT)),
                    ),
            )
            .into_any_element();
        Some(profile::modal(
            content,
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.viewer = None;
                cx.notify();
            })),
        ))
    }

    /// Opens the pins or search panel for the channel on screen.
    fn open_panel(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) {
        let (guild, channel) = (self.guild, self.channel);
        self.panel_open = true;
        self.panel
            .update(cx, |panel, cx| panel.show(kind, guild, channel, window, cx));
        cx.notify();
    }

    /// Goes to any channel we know, for example from a search result.
    fn show_channel(
        &mut self,
        channel: Snowflake,
        message: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.guild = self
            .session
            .read(cx)
            .store
            .channel(channel)
            .and_then(|c| c.guild_id);
        self.select_channel(channel, window, cx);
        if let Some(message) = message {
            self.chat.update(cx, |chat, cx| chat.focus_message(message, cx));
        }
    }

    /// Goes to a direct message from wherever we are, for example after
    /// pressing the message button on the friends page.
    fn show_dm(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = None;
        self.select_channel(channel, window, cx);
    }

    fn open_profile(&mut self, user: Snowflake, guild: Option<Snowflake>, cx: &mut Context<Self>) {
        let fetch = self.session.update(cx, |s, _| s.should_fetch_profile(user));
        if fetch {
            self.backend.send(Command::LoadProfile { user, guild });
        }
        self.profile = Some(ShowProfile { user, guild });
        cx.notify();
    }

    /// The card for the person we are looking at, over everything else.
    fn render_profile(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (user, guild) = self.profile.as_ref().map(|p| (p.user, p.guild))?;
        let card = {
            let session = self.session.read(cx);
            let store = &session.store;
            let account = store.user(user)?;
            let member = guild.and_then(|g| store.member_in_guild(g, user));
            let profile = session.profiles.get(&user);

            profile::Card {
                name: member
                    .as_ref()
                    .and_then(|(nick, _)| nick.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| account.display_name().to_owned()),
                username: account.username.clone(),
                pronouns: profile.map(|p| p.pronouns.clone()).unwrap_or_default(),
                avatar_url: cdn::user_avatar(user, account.avatar.as_deref(), 128),
                status: store.presence(user),
                bio: profile.map(|p| p.bio.clone()).unwrap_or_default(),
                created: user.datetime().format("%b %d, %Y").to_string(),
                roles: member
                    .map(|(_, roles)| {
                        roles
                            .iter()
                            .map(|r| (r.name.clone(), (r.color != 0).then_some(r.color)))
                            .collect()
                    })
                    .unwrap_or_default(),
                is_me: store.me().is_some_and(|me| me.id == user),
                actions: Vec::new(),
            }
        };
        let mut card = card;
        if !card.is_me {
            let kind = self.session.read(cx).store.relationship(user);
            card.actions = self.relationship_actions(user, kind);
        }

        let on_message =
            cx.listener(move |this, _: &ClickEvent, _, cx| this.message_from_card(user, cx));
        let on_close = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.profile = None;
            cx.notify();
        });
        Some(profile::overlay(
            card,
            &self.media,
            Box::new(on_message),
            Box::new(on_close),
            cx,
        ))
    }

    /// The relationship buttons for someone's card, by how we know them.
    fn relationship_actions(
        &self,
        user: Snowflake,
        kind: Option<RelationshipKind>,
    ) -> Vec<profile::CardAction> {
        let send = |command: fn(Snowflake) -> Command| {
            let backend = self.backend.clone();
            Box::new(move |_: &ClickEvent, _: &mut Window, _: &mut App| backend.send(command(user)))
                as Box<profile::Handler>
        };
        let add = |label| profile::CardAction { label, danger: false, handler: send(Command::AcceptFriend) };
        let remove = |label| profile::CardAction { label, danger: true, handler: send(Command::RemoveRelationship) };
        let block = profile::CardAction { label: "Block", danger: true, handler: send(Command::BlockUser) };

        match kind {
            Some(RelationshipKind::Friend) => vec![remove("Remove Friend"), block],
            Some(RelationshipKind::IncomingRequest) => vec![add("Accept"), remove("Ignore"), block],
            Some(RelationshipKind::OutgoingRequest) => vec![remove("Cancel Request"), block],
            Some(RelationshipKind::Blocked) => vec![profile::CardAction {
                label: "Unblock",
                danger: false,
                handler: send(Command::RemoveRelationship),
            }],
            _ => vec![add("Add Friend"), block],
        }
    }

    /// Opens the menu for a person and carries out whatever is picked.
    fn open_user_menu(&mut self, open: &OpenUserMenu, cx: &mut Context<Self>) {
        let (tie, is_me) = {
            let store = &self.session.read(cx).store;
            (store.relationship(open.user), store.me().map(|u| u.id) == Some(open.user))
        };
        let list = user_menu::choices(tie, is_me, open.can_mention, open.dm.is_some());
        let entries = list.iter().map(|c| c.entry()).collect();
        let (user, guild, dm) = (open.user, open.guild, open.dm);
        let view = cx.entity().downgrade();
        let pick = std::rc::Rc::new(move |index: usize, window: &mut Window, cx: &mut App| {
            let Some(choice) = list.get(index).copied() else { return };
            view.update(cx, |this, cx| this.user_choice(user, guild, dm, choice, window, cx)).ok();
        });
        self.popup_open = true;
        self.popup.update(cx, |popup, cx| popup.show(open.at, entries, pick, cx));
        cx.notify();
    }

    fn user_choice(
        &mut self,
        user: Snowflake,
        guild: Option<Snowflake>,
        dm: Option<Snowflake>,
        choice: user_menu::Choice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use user_menu::Choice;
        match choice {
            Choice::Profile => self.open_profile(user, guild, cx),
            Choice::Mention => self.session.update(cx, |_, cx| cx.emit(MentionUser(user))),
            Choice::Message => self.message_from_card(user, cx),
            Choice::AddFriend => {
                let name = self.session.read(cx).store.user(user).map(|u| u.username.clone());
                if let Some(name) = name {
                    self.backend.send(Command::AddFriendByName(name));
                }
            }
            Choice::RemoveFriend | Choice::CancelRequest | Choice::Unblock => {
                self.backend.send(Command::RemoveRelationship(user));
            }
            Choice::AcceptRequest => self.backend.send(Command::AcceptFriend(user)),
            Choice::Block => self.backend.send(Command::BlockUser(user)),
            Choice::CloseDm => {
                if let Some(dm) = dm {
                    self.close_dm(dm, window, cx);
                }
            }
            Choice::CopyId => cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(user.to_string())),
            Choice::Separator => {}
        }
    }

    /// Opens the conversation with the person on the card.
    fn message_from_card(&mut self, user: Snowflake, cx: &mut Context<Self>) {
        self.profile = None;
        let existing = self.session.read(cx).store.dm_with(user);
        match existing {
            Some(channel) => self.session.update(cx, |_, cx| cx.emit(DmOpened(channel))),
            None => self.backend.send(Command::OpenDm(user)),
        }
        cx.notify();
    }

    /// The home screen without a conversation open.
    fn show_friends(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.channel = None;
        self.sync_chat(window, cx);
        cx.notify();
    }

    fn is_forum(&self, channel: Snowflake, cx: &App) -> bool {
        self.session
            .read(cx)
            .store
            .channel(channel)
            .is_some_and(|c| matches!(c.kind, ChannelType::GuildForum | ChannelType::GuildMedia))
    }

    /// Points the message area at whatever channel is selected now.
    fn sync_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let channel = self.channel;
        // A forum shows its posts, not a message timeline.
        let forum = channel.filter(|c| self.is_forum(*c, cx));
        self.forum.update(cx, |view, cx| view.set_forum(forum, cx));
        let timeline = if forum.is_some() { None } else { channel };
        self.chat
            .update(cx, |chat, cx| chat.set_channel(timeline, window, cx));
        let (guild, channel) = (self.guild, self.channel);
        self.panel.update(cx, |panel, cx| panel.follow(guild, channel, cx));
        self.sync_members(cx);
    }

    /// Points the member list at the open guild and asks the server for it.
    fn sync_members(&mut self, cx: &mut Context<Self>) {
        let guild = self.guild;
        self.members.update(cx, |members, cx| members.set_guild(guild, cx));

        let show = self.session.read(cx).settings.show_members;
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
                    members: (c.kind == ChannelType::GroupDm).then(|| c.recipients.len() + 1),
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
                        threads: thread_items(store, c.id, self.channel),
                        ..channel_item(store, c)
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

    /// One server in the rail.
    fn guild_button(&mut self, item: &GuildItem, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.guild == Some(item.id);
        let id = item.id;
        let body = guild_icon(&self.media, item.icon.as_deref(), &item.name, selected, cx);
        rail_button(
            "guild",
            id.0,
            selected,
            item.unread,
            body,
            cx.listener(move |this, _, window, cx| this.select_guild(Some(id), window, cx)),
        )
        .into_any_element()
    }

    /// What a folder looks like in the rail: a folder glyph when open, and
    /// a grid of its first servers when closed.
    fn folder_body(&mut self, members: &[&GuildItem], tint: u32, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let base = div()
            .size(px(48.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(16.))
            .bg(rgba((tint << 8) | 0x59));
        if open {
            return base.child(icon("folder-open", 24., tint)).into_any_element();
        }
        let mut grid = div().w(px(36.)).flex().flex_wrap().gap(px(2.));
        for item in members.iter().take(4) {
            let image = item.icon.as_deref().and_then(|url| self.media.update(cx, |m, _| m.get(url)));
            let cell = div().size(px(17.)).rounded(px(5.)).overflow_hidden().bg(rgb(theme::BG_CHAT));
            grid = grid.child(match image {
                Some(image) => cell.child(img(ImageSource::Image(image)).size_full()),
                None => cell,
            });
        }
        base.child(grid).into_any_element()
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

        let items = guilds;
        let known: Vec<Snowflake> = items.iter().map(|i| i.id).collect();
        let layout = match &self.session.read(cx).account.settings {
            Load::Ready(settings) => settings.folders().to_vec(),
            _ => Vec::new(),
        };

        let mut list = div().flex().flex_col().gap_2().items_center().w_full();
        for entry in rail::arrange(&layout, &known) {
            match entry {
                Entry::Guild(id) => {
                    if let Some(item) = items.iter().find(|i| i.id == id) {
                        list = list.child(self.guild_button(item, cx));
                    }
                }
                Entry::Folder { key, color, guilds, .. } => {
                    let members: Vec<&GuildItem> = guilds.iter().filter_map(|g| items.iter().find(|i| i.id == *g)).collect();
                    let open = self.expanded.contains(&key);
                    let tint = color.unwrap_or(theme::ACCENT);
                    let unread = members.iter().fold(Unread::default(), |sum, m| Unread {
                        unread: sum.unread || m.unread.unread,
                        mentions: sum.mentions + m.unread.mentions,
                    });
                    let body = self.folder_body(&members, tint, open, cx);
                    let header = rail_button(
                        "folder",
                        key,
                        false,
                        // An open folder shows its servers' own badges.
                        if open { Unread::default() } else { unread },
                        body,
                        cx.listener(move |this, _, _, cx| {
                            if !this.expanded.remove(&key) {
                                this.expanded.insert(key);
                            }
                            cx.notify();
                        }),
                    );
                    let mut group = div().w_full().flex().flex_col().gap_2().items_center().child(header);
                    if open {
                        group = group.bg(rgba((tint << 8) | 0x33)).rounded(px(16.)).pb_2();
                        for item in members {
                            group = group.child(self.guild_button(item, cx));
                        }
                    }
                    list = list.child(group);
                }
            }
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
            .child(
                div()
                    .id("add-server")
                    .size(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(24.))
                    .bg(rgb(theme::BG_CHAT))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_ACTIVE)).rounded(px(16.)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.join_open = true;
                        this.join.update(cx, |view, cx| view.opened(window, cx));
                        cx.notify();
                    }))
                    .child(icon("plus", 22., theme::ONLINE)),
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
                list = list.child(self.render_dm_heading(cx));
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
                    .id("sidebar-header")
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .when_some(self.guild, |header, guild| {
                        header.cursor_pointer().hover(|s| s.bg(rgb(theme::BG_HOVER))).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.open_menu(Target::Guild(guild), point(px(theme::RAIL_WIDTH + 8.), px(54.)), cx);
                            }),
                        )
                    })
                    .when(self.guild.is_none(), |header| {
                        header.px_2().child(
                            div()
                                .id("find-conversation")
                                .w_full()
                                .h(px(30.))
                                .px_2()
                                .flex()
                                .items_center()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .bg(rgb(theme::BG_PANEL))
                                .text_size(px(13.))
                                .font_weight(gpui_kit::FontWeight::NORMAL)
                                .text_color(rgb(theme::TEXT_MUTED))
                                .on_click(cx.listener(|this, _, window, cx| this.open_switcher(window, cx)))
                                .child("Find or start a conversation"),
                        )
                    })
                    .when(self.guild.is_some(), |header| header.px_4().child(
                        div().flex_1().min_w_0().truncate().child(title),
                    ))
                    .children(self.guild.map(|_| icon("chevron-down", 16., theme::TEXT_MUTED).mr_4()))
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

        for mut channel in group.channels {
            // A collapsed category still shows the channel you are in and
            // any with unread messages, as the official client does.
            let selected = self.channel == Some(channel.id);
            if hidden && !selected && !channel.unread.unread {
                continue;
            }
            let threads = std::mem::take(&mut channel.threads);
            column = column.child(self.render_channel(channel, selected, cx));
            for thread in threads {
                let open = self.channel == Some(thread.id);
                column = column.child(self.render_channel(thread, open, cx));
            }
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
            k if k.is_thread() => "corner-down-right",
            _ => "hash",
        };
        let color = if selected || channel.unread.unread {
            theme::TEXT_BRIGHT
        } else if channel.muted {
            theme::TEXT_FAINT
        } else {
            theme::TEXT_MUTED
        };

        let indent = if channel.kind.is_thread() { 18. } else { 0. };

        div()
            .id(("channel", id.0))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_menu(Target::Channel(id), event.position, cx);
                }),
            )
            .h(px(32.))
            .ml(px(indent))
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

    /// "Direct Messages" with the button that starts a new conversation.
    fn render_dm_heading(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .h(px(32.))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .pl_2()
            .pr_1()
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("DIRECT MESSAGES"),
            )
            .child(
                div()
                    .id("new-dm")
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .rounded(px(4.))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                        let at = point(event.position().x - px(40.), event.position().y + px(14.));
                        this.open_picker(Purpose::Create, at, window, cx);
                    }))
                    .child(icon("plus", 14., theme::TEXT_MUTED)),
            )
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
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                let Some(user) = this.session.read(cx).store.channel(id).filter(|c| c.kind == ChannelType::Dm).and_then(|c| c.recipients.first().map(|r| r.id)) else { return };
                this.session.update(cx, |_, cx| {
                    cx.emit(OpenUserMenu { user, guild: None, at: event.position, dm: Some(id), can_mention: false });
                });
            }))
            .group("dm-row")
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
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().child(dm.name))
                    .children(dm.members.map(|n| {
                        div().text_size(px(11.)).text_color(rgb(theme::TEXT_MUTED)).child(format!("{n} Members"))
                    })),
            )
            .children(mention_badge(dm.unread.mentions))
            .child(
                div()
                    .id(("close-dm", id.0))
                    .size(px(18.))
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .hidden()
                    .group_hover("dm-row", |s| s.flex())
                    .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_dm(id, window, cx);
                    }))
                    .child(icon("x", 14., theme::TEXT_MUTED)),
            )
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
                    .id("open-settings")
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings_open = true;
                        this.settings_view.update(cx, |view, cx| view.opened(cx));
                        cx.notify();
                    }))
                    .child(icon("settings", 18., theme::TEXT_MUTED)),
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
        } else if self.channel.is_some_and(|c| self.is_forum(c, cx)) {
            self.forum.clone().into_any_element()
        } else {
            self.chat.clone().into_any_element()
        };
        let in_forum = self.channel.is_some_and(|c| self.is_forum(c, cx));
        let members = (self.guild.is_some()
            && !in_forum
            && !self.panel_open
            && self.session.read(cx).settings.show_members)
            .then(|| self.members.clone());
        let panel = self.panel_open.then(|| self.panel.clone());
        let profile = self.render_profile(cx);
        let settings = self.settings_open.then(|| self.settings_view.clone());
        let menu = self.menu_open.then(|| self.menu.clone());
        let viewer = self.render_viewer(cx);
        let join = self.join_open.then(|| self.join.clone());
        let switcher = self.switcher_open.then(|| self.switcher.clone());
        let picker = self.picker_open.then(|| self.picker.clone());
        let popup = self.popup_open.then(|| self.popup.clone());
        let inbox = self.inbox_open.then(|| self.inbox.clone());

        div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &QuickSwitch, window, cx| this.open_switcher(window, cx)))
            // A click on bare background parks focus here, so Ctrl+K still
            // works afterwards.
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                if window.focused(cx).is_none() {
                    window.focus(&this.focus, cx);
                }
            }))
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
                    .children(members)
                    .children(panel),
            )
            .children(profile)
            .children(menu)
            .children(viewer)
            .children(join)
            .children(switcher)
            .children(picker)
            .children(popup)
            .children(inbox)
            .children(settings)
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

fn channel_item(store: &oxicord_store::Store, c: &oxicord_model::Channel) -> ChannelItem {
    ChannelItem {
        id: c.id,
        name: c.name.clone().unwrap_or_default(),
        kind: c.kind,
        unread: store.unread(c.id),
        muted: store.is_muted(c),
        threads: Vec::new(),
    }
}

/// The threads listed under a channel: the ones we joined, plus the one on
/// screen so you can always see where you are.
fn thread_items(
    store: &oxicord_store::Store,
    parent: Snowflake,
    open: Option<Snowflake>,
) -> Vec<ChannelItem> {
    let mut items: Vec<_> = store
        .joined_threads(parent)
        .into_iter()
        .map(|t| channel_item(store, t))
        .collect();
    if let Some(current) = open.and_then(|id| store.channel(id))
        && current.parent_id == Some(parent)
        && current.kind.is_thread()
        && !items.iter().any(|i| i.id == current.id)
    {
        items.push(channel_item(store, current));
    }
    items
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
