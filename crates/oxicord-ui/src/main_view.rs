use std::collections::{HashMap, HashSet};

use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, ImageSource, InteractiveElement as _,
    IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    FocusHandle, MouseButton, component::input::Escape, MouseDownEvent, Pixels, Point, Subscription, Window, div, img, point, px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_gateway::{ClientStatus, ConnectionState};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, RelationshipKind, Snowflake, Status};
use oxicord_store::Unread;

use crate::{
    account::Load,
    backend::{Backend, Command},
    chat::{ChatView, Shortcut},
    context_menu::{self, CloseMenu, ContextMenu, Target},
    dm_picker::{ClosePicker, DmPicker, Purpose},
    forum::ForumView,
    forward_dialog::{CloseForward, ForwardDialog},
    nickname_dialog::{CloseNickname, NicknameDialog},
    join_server::{CloseJoin, JoinServerView},
    friends::FriendsView,
    icons::icon,
    rail::{self, Entry},
    inbox::{CloseInbox, Inbox},
    invite_dialog::{CloseInvite, InviteDialog},
    keys::{self, QuickSwitch},
    media::{Media, ViewImage, acronym, avatar},
    member_list::{MemberList, presence_dot, status_shape},
    popup::{ClosePopup, PopupMenu},
    profile,
    session::{DmOpened, JumpTo, MentionUser, OpenForward, OpenInbox, OpenInviteDialog, OpenMenu, OpenNickname, OpenPanel, OpenPicker, OpenPopup, OpenServerSettings, OpenUserMenu, PanelKind, Session, ShowProfile},
    settings_view::{CloseSettings, SettingsView},
    server_settings::{CloseServerSettings, ServerSettings},
    side_panel::{ClosePanel, SidePanel},
    status_dialog::{CloseStatusDialog, StatusDialog},
    switcher::{Chosen, CloseSwitcher, Goal, QuickSwitcher},
    theme, title_bar, user_menu,
};

struct GuildItem {
    id: Snowflake,
    name: String,
    icon: Option<String>,
    unread: Unread,
}

/// Someone sitting in a voice channel.
struct VoiceItem {
    user: Snowflake,
    name: String,
    avatar: String,
    muted: bool,
    deafened: bool,
    video: bool,
    streaming: bool,
}

struct ChannelItem {
    id: Snowflake,
    name: String,
    kind: ChannelType,
    unread: Unread,
    /// Whether we may make an invite through it, which adds a shortcut on hover.
    can_invite: bool,
    /// Who is in it, for voice channels.
    voice: Vec<VoiceItem>,
    /// What the people in a voice channel say they are doing.
    status: Option<String>,
    /// User limit for voice channels (None = unlimited).
    user_limit: Option<u32>,
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
    /// The list of statuses beside the account popout.
    status_submenu_open: bool,
    status_dialog: Entity<StatusDialog>,
    status_dialog_open: bool,
    channel_status_modal: bool,
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
    forward: Entity<ForwardDialog>,
    forward_open: bool,
    nickname: Entity<NicknameDialog>,
    nickname_open: bool,
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
    invite: Entity<InviteDialog>,
    invite_open: bool,
    server_settings: Entity<ServerSettings>,
    server_settings_open: bool,
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
        let status_dialog = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| StatusDialog::new(session, media, backend, window, cx))
        };
        let forward = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| ForwardDialog::new(session, media, backend, window, cx))
        };
        let nickname = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| NicknameDialog::new(session, backend, window, cx))
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
        let server_settings = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| ServerSettings::new(session, media, backend, window, cx))
        };
        let invite = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| InviteDialog::new(session, media, backend, cx))
        };
        let members_shown = session.read(cx).settings.show_members;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let subscriptions = vec![
            cx.subscribe(&server_settings, |this, _, _: &CloseServerSettings, cx| {
                this.server_settings_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenServerSettings, window, cx| {
                this.server_settings_open = true;
                this.server_settings.update(cx, |view, cx| view.opened(open.0, window, cx));
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, open: &OpenUserMenu, cx| this.open_user_menu(open, cx)),
            cx.subscribe(&invite, |this, _, _: &CloseInvite, cx| {
                this.invite_open = false;
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, open: &OpenInviteDialog, cx| {
                this.invite_open = true;
                this.invite.update(cx, |dialog, cx| dialog.opened(open.0, cx));
                cx.notify();
            }),
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
            cx.subscribe(&status_dialog, |this, _, _: &CloseStatusDialog, cx| {
                this.status_dialog_open = false;
                cx.notify();
            }),
            cx.subscribe(&nickname, |this, _, _: &CloseNickname, cx| {
                this.nickname_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenNickname, window, cx| {
                this.nickname_open = true;
                this.nickname.update(cx, |dialog, cx| dialog.opened(open.0, window, cx));
                cx.notify();
            }),
            cx.subscribe(&forward, |this, _, _: &CloseForward, cx| {
                this.forward_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenForward, window, cx| {
                this.forward_open = true;
                this.forward.update(cx, |dialog, cx| dialog.opened(open.channel, open.guild, open.message, window, cx));
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
            status_submenu_open: false,
            status_dialog,
            status_dialog_open: false,
            channel_status_modal: false,
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
            forward,
            forward_open: false,
            nickname,
            nickname_open: false,
            switcher,
            switcher_open: false,
            picker,
            popup,
            popup_open: false,
            expanded: HashSet::new(),
            picker_open: false,
            inbox,
            inbox_open: false,
            invite,
            invite_open: false,
            server_settings,
            server_settings_open: false,
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

    /// What the title bar names: the server, the conversation, or Friends.
    fn window_title(&self, cx: &App) -> SharedString {
        let store = &self.session.read(cx).store;
        let name = match (self.guild, self.channel) {
            (Some(guild), _) => store.guild(guild).map(|g| g.name.clone()),
            (None, Some(channel)) => store.channel(channel).map(|c| store.dm_name(c)),
            (None, None) => Some("Friends".to_owned()),
        };
        name.unwrap_or_default().into()
    }

    /// Channels in sidebar order, for stepping through with the keyboard.
    fn nav_channels(&self, cx: &App) -> Vec<Snowflake> {
        let store = &self.session.read(cx).store;
        match self.guild {
            Some(guild) => store
                .channel_groups(guild)
                .iter()
                .flat_map(|group| group.channels.iter())
                .filter(|c| c.kind.has_messages() && !is_voice(c.kind))
                .map(|c| c.id)
                .collect(),
            None => store.dm_channels().into_iter().map(|c| c.id).collect(),
        }
    }

    /// Moves to the channel above or below, or the nearest unread one,
    /// wrapping around the list.
    fn step_channel(&mut self, forward: bool, unread_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        let list = self.nav_channels(cx);
        let n = list.len();
        if n == 0 {
            return;
        }
        let at = self.channel.and_then(|c| list.iter().position(|&x| x == c));
        let start = at.unwrap_or(if forward { n - 1 } else { 0 });
        let target = {
            let store = &self.session.read(cx).store;
            (1..=n)
                .map(|i| list[if forward { (start + i) % n } else { (start + n - i) % n }])
                .filter(|&c| Some(c) != self.channel)
                .find(|&c| !unread_only || {
                    let u = store.unread(c);
                    u.unread || u.mentions > 0
                })
        };
        if let Some(channel) = target {
            self.select_channel(channel, window, cx);
        }
    }

    /// Moves along the rail, home first, then servers in folder order.
    fn step_server(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let known: Vec<Snowflake> = self.session.read(cx).store.guilds().map(|g| g.id).collect();
        let layout = match &self.session.read(cx).account.settings {
            Load::Ready(settings) => settings.folders().to_vec(),
            _ => Vec::new(),
        };
        let mut order = vec![None];
        for entry in rail::arrange(&layout, &known) {
            match entry {
                Entry::Guild(id) => order.push(Some(id)),
                Entry::Folder { guilds, .. } => order.extend(guilds.into_iter().map(Some)),
            }
        }
        let n = order.len();
        let at = order.iter().position(|&g| g == self.guild).unwrap_or(0);
        let next = if forward { (at + 1) % n } else { (at + n - 1) % n };
        self.select_guild(order[next], window, cx);
    }

    fn toggle_panel(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel_open && self.panel.read(cx).kind() == kind {
            self.panel_open = false;
            cx.notify();
        } else if self.channel.is_some() {
            self.open_panel(kind, window, cx);
        }
    }

    fn mark_channel_read(&mut self, cx: &mut Context<Self>) {
        if let Some(channel) = self.channel {
            context_menu::mark_channel_read(&self.session, &self.backend, channel, cx);
        }
    }

    /// Sends a composer shortcut to the chat, when the chat is what's showing.
    fn chat_shortcut(&mut self, action: Shortcut, window: &mut Window, cx: &mut Context<Self>) {
        if self.channel.is_some_and(|c| !self.is_forum(c, cx)) {
            self.chat.update(cx, |chat, cx| chat.shortcut(action, window, cx));
        }
    }

    /// Hooks every shortcut in [`keys::SHORTCUTS`] to what it does.
    fn on_shortcuts(&mut self, root: gpui_kit::Div, cx: &mut Context<Self>) -> gpui_kit::Div {
        root.on_action(cx.listener(|this, _: &QuickSwitch, window, cx| this.open_switcher(window, cx)))
            .on_action(cx.listener(|this, _: &keys::PrevChannel, window, cx| this.step_channel(false, false, window, cx)))
            .on_action(cx.listener(|this, _: &keys::NextChannel, window, cx| this.step_channel(true, false, window, cx)))
            .on_action(cx.listener(|this, _: &keys::PrevUnread, window, cx| this.step_channel(false, true, window, cx)))
            .on_action(cx.listener(|this, _: &keys::NextUnread, window, cx| this.step_channel(true, true, window, cx)))
            .on_action(cx.listener(|this, _: &keys::PrevServer, window, cx| this.step_server(false, window, cx)))
            .on_action(cx.listener(|this, _: &keys::NextServer, window, cx| this.step_server(true, window, cx)))
            .on_action(cx.listener(|this, _: &keys::MarkChannelRead, _, cx| this.mark_channel_read(cx)))
            // A text field hands Escape on when it has nothing to close.
            .on_action(cx.listener(|this, _: &Escape, _, cx| this.mark_channel_read(cx)))
            .on_action(cx.listener(|this, _: &keys::MarkServerRead, _, cx| {
                if let Some(guild) = this.guild {
                    context_menu::mark_guild_read(&this.session, &this.backend, guild, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &keys::TogglePins, window, cx| this.toggle_panel(PanelKind::Pins, window, cx)))
            .on_action(cx.listener(|this, _: &keys::Search, window, cx| {
                if this.channel.is_some() {
                    this.open_panel(PanelKind::Search, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &keys::ToggleInbox, _, cx| {
                this.inbox_open = !this.inbox_open;
                if this.inbox_open {
                    this.inbox.update(cx, |inbox, cx| inbox.opened(cx));
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &keys::ToggleMembers, _, cx| {
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| s.change_settings(&backend, |s| s.show_members = !s.show_members, cx));
            }))
            .on_action(cx.listener(|this, _: &keys::EmojiPicker, window, cx| this.chat_shortcut(Shortcut::Emoji, window, cx)))
            .on_action(cx.listener(|this, _: &keys::GifPicker, window, cx| this.chat_shortcut(Shortcut::Gifs, window, cx)))
            .on_action(cx.listener(|this, _: &keys::StickerPicker, window, cx| this.chat_shortcut(Shortcut::Stickers, window, cx)))
            .on_action(cx.listener(|this, _: &keys::Upload, window, cx| this.chat_shortcut(Shortcut::Upload, window, cx)))
            .on_action(cx.listener(|this, _: &keys::NewGroupDm, window, cx| {
                // Where the button for it would drop the picker.
                let at = point(px(theme::RAIL_WIDTH + 40.), px(130.));
                this.open_picker(Purpose::Create, at, window, cx);
            }))
            .on_action(cx.listener(|this, _: &keys::ShowKeybinds, _, cx| {
                this.settings_open = true;
                this.settings_view.update(cx, |view, cx| view.open_keybinds(cx));
                cx.notify();
            }))
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

    /// Gathers what a person's card shows. `guild` adds their nickname and roles there.
    fn build_card(&self, user: Snowflake, guild: Option<Snowflake>, cx: &App) -> Option<profile::Card> {
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
                banner_url: profile
                    .and_then(|p| p.banner.as_deref())
                    .map(|hash| cdn::user_banner(user, hash, 600)),
                accent: profile.and_then(|p| p.accent_color).unwrap_or(theme::ACCENT),
                badges: profile
                    .map(|p| p.badges.iter().map(|b| (b.description.clone(), cdn::badge_icon(&b.icon))).collect())
                    .unwrap_or_default(),
                joined_server: profile
                    .and_then(|p| p.joined_server.as_deref())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.format("%b %d, %Y").to_string())
                    .unwrap_or_default(),
                mutual_servers: profile
                    .map(|p| {
                        p.mutual_guilds
                            .iter()
                            .filter_map(|m| store.guild(m.id))
                            .map(|g| (g.name.clone(), g.icon.as_deref().map(|h| cdn::guild_icon(g.id, h, 64))))
                            .collect()
                    })
                    .unwrap_or_default(),
                mutual_friends: profile.map_or(0, |p| p.mutual_friends),
                accounts: profile
                    .map(|p| p.accounts.iter().map(|a| format!("{}: {}", a.kind, a.name)).collect())
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
        Some(card)
    }

    /// The card for the person we are looking at, over everything else.
    fn render_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (user, guild) = self.profile.as_ref().map(|p| (p.user, p.guild))?;
        let card = self.build_card(user, guild, cx)?;

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
            window,
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
    /// Asks for the profile of the person on the other end of the open
    /// conversation, which the docked card shows.
    fn fetch_dm_profile(&mut self, cx: &mut Context<Self>) {
        let Some(user) = self.dm_peer(cx) else { return };
        if self.session.update(cx, |s, _| s.should_fetch_profile(user)) {
            self.backend.send(Command::LoadProfile { user, guild: None });
        }
    }

    /// The other person in a one-to-one conversation.
    fn dm_peer(&self, cx: &App) -> Option<Snowflake> {
        let channel = self.channel?;
        let c = self.session.read(cx).store.channel(channel)?;
        (c.kind == ChannelType::Dm).then(|| c.recipients.first().map(|r| r.id))?
    }

    /// The docked column beside a conversation: the person's card for a
    /// one-to-one chat, the participants for a group.
    fn render_dm_column(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.channel?;
        if let Some(user) = self.dm_peer(cx) {
            let card = self.build_card(user, None, cx)?;
            let on_message = cx.listener(move |this, _: &ClickEvent, _, cx| this.message_from_card(user, cx));
            return Some(profile::column(card, &self.media, Box::new(on_message), window, cx));
        }

        let members: Vec<_> = {
            let store = &self.session.read(cx).store;
            let c = store.channel(channel).filter(|c| c.kind == ChannelType::GroupDm)?;
            c.recipients
                .iter()
                .map(|u| (u.id, u.display_name().to_owned(), cdn::user_avatar(u.id, u.avatar.as_deref(), 64), store.presence(u.id)))
                .collect()
        };
        let mut column = div()
            .id("group-members")
            .w(px(theme::MEMBERS_WIDTH))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .bg(rgb(theme::BG_MEMBERS))
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .px_4()
                    .pt_4()
                    .pb_1()
                    .text_size(px(11.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(format!("MEMBERS \u{2014} {}", members.len() + 1)),
            );
        for (user, name, url, status) in members {
            column = column.child(
                div()
                    .id(("group-member", user.0))
                    .h(px(44.))
                    .mx_2()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_profile(user, None, cx)))
                    .child(div().relative().flex_none().child(avatar(&self.media, &url, 32., &name, cx)).child(presence_dot(status)))
                    .child(div().flex_1().min_w_0().truncate().child(name)),
            );
        }
        Some(column.into_any_element())
    }

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
        self.fetch_dm_profile(cx);
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
            .size(px(40.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS_MD))
            .bg(rgba((tint << 8) | 0x59));
        if open {
            return base.child(icon("folder-open", 22., tint)).into_any_element();
        }
        // A 4px inset and 2px gap, so each preview is 15px.
        let mut grid = div().w(px(32.)).flex().flex_wrap().gap(px(2.));
        for item in members.iter().take(4) {
            let image = item.icon.as_deref().and_then(|url| self.media.update(cx, |m, _| m.get(url)));
            let cell = div().size(px(15.)).rounded(px(4.)).overflow_hidden().bg(rgb(theme::BG_HOVER));
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
                .size(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(if selected_guild.is_none() { theme::ACCENT } else { theme::BG_HOVER }))
                .rounded(px(if selected_guild.is_none() { 12. } else { 20. }))
                .child(icon("clyde", 24., theme::TEXT_BRIGHT))
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
            .pb(px(12.))
            .bg(rgb(theme::BG_RAIL))
            .child(home)
            .child(div().w(px(32.)).h(px(1.)).bg(rgb(theme::BORDER)))
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
                    .size(px(40.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(20.))
                    .bg(rgb(theme::BG_HOVER))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::ADD_SERVER_HOVER)).rounded(px(12.)))
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

        let mut list = div().flex().flex_col().gap(px(2.)).p_2();
        match self.guild {
            Some(guild) => {
                for group in self.group_items(guild, cx) {
                    list = list.child(self.render_group(group, cx));
                }
            }
            None => {
                list = list.child(self.render_friends_entry(cx));
                // A line between the shortcuts and the conversations.
                list = list.child(div().mx(px(8.)).my(px(10.)).h(px(1.)).bg(rgb(theme::BORDER)));
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
                    .h(px(theme::CHANNEL_HEADER_HEIGHT))
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
            .child(self.render_user_panel(cx))
    }

    fn render_group(&mut self, group: GroupItem, cx: &mut Context<Self>) -> AnyElement {
        let mut column = div().flex().flex_col().gap(px(2.));

        let hidden = group
            .category
            .as_ref()
            .is_some_and(|(id, _)| self.collapsed.contains(id));

        if let Some((id, name)) = group.category {
            // Sentence case with the chevron after the name, 16px of air above.
            let chevron = if hidden { "chevron-right" } else { "chevron-down" };
            column = column.child(
                div()
                    .id(("category", id.0))
                    .h(px(40.))
                    .pt(px(16.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(rgb(theme::CHANNELS_DEFAULT))
                    .hover(|s| s.text_color(rgb(theme::TEXT_BRIGHT)))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_category(id, cx)))
                    .child(div().truncate().child(name))
                    .child(icon(chevron, 12., theme::CHANNELS_DEFAULT)),
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

    /// The people sitting in a voice channel, listed under it.
    fn render_voice_members(&mut self, members: &[VoiceItem], cx: &mut Context<Self>) -> Vec<AnyElement> {
        members
            .iter()
            .map(|m| {
                let mut row = div()
                    .id(("voice-member", m.user.0))
                    .h(px(28.))
                    .ml(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT)))
                    .child(avatar(&self.media, &m.avatar, 20., &m.name, cx))
                    .child(div().flex_1().min_w_0().truncate().child(m.name.clone()));
                let user = m.user;
                row = row.on_click(cx.listener(move |this, _, _, cx| this.open_profile(user, this.guild, cx)));
                if m.streaming {
                    row = row.child(
                        div()
                            .px(px(5.))
                            .rounded(px(3.))
                            .bg(rgb(theme::BADGE))
                            .text_size(px(10.))
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("LIVE"),
                    );
                } else if m.video {
                    row = row.child(icon("video", 14., theme::TEXT_MUTED));
                }
                if m.deafened {
                    row = row.child(icon("headphone-off", 14., theme::DANGER));
                } else if m.muted {
                    row = row.child(icon("mic-off", 14., theme::DANGER));
                }
                row.into_any_element()
            })
            .collect()
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
            theme::CHANNEL_MUTED
        } else {
            theme::CHANNELS_DEFAULT
        };

        let indent = if channel.kind.is_thread() { 18. } else { 0. };
        let participants = self.render_voice_members(&channel.voice, cx);

        let row = div()
            .id(("channel", id.0))
            .group("channel-row")
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
            .px(px(8.))
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(rgb(color))
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| {
                let s = s.bg(rgb(theme::BG_HOVER));
                if channel.muted { s } else { s.text_color(rgb(theme::TEXT_BRIGHT)) }
            })
            .on_click(cx.listener(move |this, _, window, cx| this.select_channel(id, window, cx)))
            .child(icon(glyph, 20., color))
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
            .children(channel.user_limit.map(|_| {
                div()
                    .px(px(4.))
                    .rounded(px(3.))
                    .bg(rgb(theme::BADGE))
                    .text_size(px(10.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("LIMITED")
            }))
            .children(channel.can_invite.then(|| {
                div()
                    .id(("invite-channel", id.0))
                    .size(px(18.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .invisible()
                    .group_hover("channel-row", |s| s.visible())
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.session.update(cx, |_, cx| cx.emit(OpenInviteDialog(id)));
                    }))
                    .child(icon("user-plus", 15., theme::TEXT_MUTED))
            }))
            .children(mention_badge(channel.unread.mentions));

        let status = channel.status.map(|text| {
            div()
                .ml(px(30.))
                .px_2()
                .pb_1()
                .truncate()
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(text)
        });

        let set_status = (channel.kind == ChannelType::GuildVoice || channel.kind == ChannelType::GuildStageVoice).then(|| {
            div()
                .id(("set-status", channel.id.0))
                .ml(px(30.))
                .px_2()
                .pb_1()
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_MUTED))
                .cursor_pointer()
                .hover(|s| s.text_color(rgb(theme::TEXT_BRIGHT)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.channel_status_modal = true;
                    cx.notify();
                }))
                .child("Set a channel status")
        });

        div().flex().flex_col().child(row).children(status).children(set_status).children(participants)
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
        let color = if selected { theme::TEXT } else { theme::TEXT_FAINT };

        div()
            .id("friends")
            .h(px(38.))
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(8.))
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(rgb(color))
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT)))
            .on_click(cx.listener(|this, _, window, cx| this.show_friends(window, cx)))
            .child(div().w(px(20.)).flex_none().flex().justify_center().child(icon("friends", 20., color)))
            .child(div().flex_1().child("Friends"))
            .children(mention_badge(u32::try_from(pending).unwrap_or(u32::MAX)))
    }

    /// "Direct Messages" with the button that starts a new conversation.
    fn render_dm_heading(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .h(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .pl_2()
            .pr_1()
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(rgb(theme::CHANNELS_DEFAULT))
                    .child("Direct Messages"),
            )
            .child(
                div()
                    .id("new-dm")
                    .size(px(24.))
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
                    .child(icon("plus", 16., theme::TEXT_MUTED)),
            )
    }

    fn render_dm(&mut self, dm: DmItem, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = dm.id;
        let selected = self.channel == Some(id);
        let color = if selected {
            theme::TEXT
        } else if dm.unread.unread {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_FAINT
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
            .h(px(42.))
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(8.))
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(rgb(color))
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT)))
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
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .invisible()
                    .group_hover("dm-row", |s| s.visible())
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
        self.status_submenu_open = false;
        self.session.update(cx, |session, cx| {
            session.status = status;
            cx.notify();
        });
        self.backend.send(Command::SetStatus(status));
        cx.notify();
    }

    fn close_account_popout(&mut self, cx: &mut Context<Self>) {
        self.status_menu_open = false;
        self.status_submenu_open = false;
        cx.notify();
    }

    /// The card that opens from the user panel: custom status, profile,
    /// the status list and a few account shortcuts.
    fn render_account_popout(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.status_menu_open {
            return None;
        }
        let (me, current, now) = {
            let session = self.session.read(cx);
            (session.store.me().cloned(), session.status, chrono::Utc::now().timestamp_millis().max(0) as u64)
        };
        let me = me?;
        let (custom, bio, pronouns) = {
            let account = &self.session.read(cx).account;
            let details = account.details.ready();
            (
                account.custom_status(now),
                details.and_then(|d| d.bio.clone()).filter(|b| !b.is_empty()),
                details.and_then(|d| d.pronouns.clone()).filter(|p| !p.is_empty()),
            )
        };
        let avatar_url = cdn::user_avatar(me.id, me.avatar.as_deref(), 128);
        let name = me.display_name().to_owned();
        let user_id = me.id;

        let bubble_text = custom.map(|c| {
            let glyph = c.emoji_id.is_none().then_some(c.emoji_name).flatten();
            [glyph, Some(c.text).filter(|t| !t.is_empty())].into_iter().flatten().collect::<Vec<_>>().join(" ")
        });
        let bubble = div()
            .id("custom-status-bubble")
            .absolute()
            .left(px(109.))
            .top(px(98.))
            .h(px(36.))
            .max_w(px(179.))
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(16.))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(0x17181b))
            .text_size(px(14.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| {
                this.close_account_popout(cx);
                this.status_dialog_open = true;
                this.status_dialog.update(cx, |dialog, cx| dialog.opened(window, cx));
            }))
            .map(|b| match bubble_text {
                Some(text) => b.text_color(rgb(theme::TEXT)).child(div().truncate().child(text)),
                None => b
                    .text_color(rgb(theme::TEXT_FAINT))
                    .italic()
                    .child(icon("circle-plus", 16., theme::TEXT_FAINT))
                    .child(div().truncate().child("What\u{2019}s your hot take?")),
            });

        let row = |id: &'static str, glyph: AnyElement, label: String, chevron: bool| {
            div()
                .id(id)
                .h(px(32.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .text_size(px(14.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_MUTED))
                .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT)))
                .child(div().size(px(16.)).flex().items_center().justify_center().child(glyph))
                .child(div().flex_1().child(label))
                .when(chevron, |r| r.child(icon("chevron-right", 16., theme::TEXT_MUTED)))
        };
        let line = || div().h(px(1.)).mx(px(8.)).my(px(8.)).bg(rgb(theme::BORDER_MUTED));

        let submenu = self.status_submenu_open.then(|| {
            let mut menu = div()
                .absolute()
                .left(px(288.))
                .bottom(px(56.))
                .w(px(300.))
                .p(px(8.))
                .flex()
                .flex_col()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER_MUTED))
                .bg(rgb(theme::BG_MENU));
            for (index, (status, hint)) in [
                (ClientStatus::Online, None),
                (ClientStatus::Idle, None),
                (ClientStatus::Dnd, Some("You will not receive desktop notifications")),
                (ClientStatus::Invisible, Some("You will appear offline")),
            ]
            .into_iter()
            .enumerate()
            {
                if index == 1 {
                    menu = menu.child(line().mx(px(0.)));
                }
                menu = menu.child(
                    div()
                        .id(status.as_str())
                        .h(px(if hint.is_some() { 52. } else { 36. }))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _, _, cx| this.set_status(status, cx)))
                        .child(
                            div()
                                .size(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(status_shape(dot_status(status), 10., theme::BG_MENU)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .line_height(px(18.))
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(rgb(theme::TEXT_BRIGHT))
                                        .child(status_label(status)),
                                )
                                .children(hint.map(|hint| {
                                    div().text_size(px(12.)).line_height(px(16.)).text_color(rgb(theme::TEXT_FAINT)).child(hint)
                                })),
                        ),
                );
            }
            menu
        });

        let card = div()
            .id("account-popout")
            .occlude()
            .absolute()
            .left(px(8.))
            .bottom(px(72.))
            .w(px(300.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .relative()
                    .h(px(140.))
                    .child(div().h(px(105.)).rounded_t(px(theme::RADIUS)).bg(rgb(theme::ACCENT)))
                    .child(
                        div()
                            .absolute()
                            .left(px(10.))
                            .top(px(55.))
                            .p(px(6.))
                            .rounded_full()
                            .bg(rgb(theme::BG_PANEL))
                            .child(
                                div()
                                    .relative()
                                    .child(avatar(&self.media, &avatar_url, 80., &name, cx))
                                    .child(
                                        div()
                                            .absolute()
                                            .right(px(-4.))
                                            .bottom(px(-4.))
                                            .p(px(3.))
                                            .rounded_full()
                                            .bg(rgb(theme::BG_PANEL))
                                            .child(status_shape(dot_status(current), 14., theme::BG_PANEL)),
                                    ),
                            ),
                    )
                    .child(bubble),
            )
            .children(submenu)
            .child(
                div()
                    .px(px(16.))
                    .pt(px(4.))
                    .pb(px(16.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(24.))
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT))
                            .child(name),
                    )
                    .child(
                        div()
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(me.username.clone())
                            .children(pronouns.map(|p| {
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().size(px(4.)).rounded_full().bg(rgb(theme::TEXT)))
                                    .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(p))
                            })),
                    )
                    .children(bio.map(|bio| {
                        div().mt(px(8.)).text_size(px(14.)).text_color(rgb(theme::TEXT_BRIGHT)).child(bio)
                    }))
                    .child(
                        div()
                            .mt(px(12.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(
                                div()
                                    .p(px(8.))
                                    .flex()
                                    .flex_col()
                                    .rounded(px(theme::RADIUS))
                                    .bg(rgb(0x17181b))
                                    .child(
                                        row("account-edit-profile", icon("pencil", 16., theme::TEXT_MUTED).into_any_element(), "Edit Profile".into(), false)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.close_account_popout(cx);
                                                this.settings_open = true;
                                                this.settings_view.update(cx, |view, cx| view.open_profile(cx));
                                            })),
                                    )
                                    .child(line())
                                    .child(
                                        row(
                                            "account-status",
                                            status_shape(dot_status(current), 12., 0x17181b).into_any_element(),
                                            status_label(current).to_owned(),
                                            true,
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.status_submenu_open = !this.status_submenu_open;
                                            cx.notify();
                                        })),
                                    ),
                            )
                            .child(
                                div()
                                    .p(px(8.))
                                    .flex()
                                    .flex_col()
                                    .rounded(px(theme::RADIUS))
                                    .bg(rgb(0x17181b))
                                    .child(
                                        row("account-copy-id", icon("copy", 16., theme::TEXT_MUTED).into_any_element(), "Copy User ID".into(), false)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(user_id.to_string()));
                                                this.close_account_popout(cx);
                                            })),
                                    ),
                            ),
                    ),
            );

        // A click anywhere else closes it.
        Some(
            div()
                .id("account-popout-layer")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.close_account_popout(cx)))
                .child(card)
                .into_any_element(),
        )
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

        // A card floating 8px in from the edges, as the official panels are.
        div().flex_none().p(px(8.)).child(
        div()
            .h(px(56.))
            .flex()
            .items_center()
            .gap(px(8.))
            .p(px(8.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_SIDEBAR))
            .child(
                div()
                    .id("me")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded(px(theme::RADIUS))
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
                            .child(avatar(&self.media, &url, 32., &name, cx))
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
                            .text_size(px(16.)).line_height(px(18.)).font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(name),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.)).line_height(px(13.)).font_weight(gpui_kit::FontWeight::MEDIUM)
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
                    .rounded(px(theme::RADIUS))
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
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(move |_, _, _| backend.send(Command::Logout))
                    .child(icon("log-out", 18., theme::TEXT_MUTED)),
            ),
        )
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let dm_column = if self.guild.is_none()
            && self.channel.is_some()
            && !self.panel_open
            && self.session.read(cx).settings.show_members
        {
            self.render_dm_column(window, cx)
        } else {
            None
        };
        let panel = self.panel_open.then(|| self.panel.clone());
        // The chat header runs over the column on its right, so the column
        // is laid over the chat and the chat leaves room for it.
        let over_chat = !(self.guild.is_none() && self.channel.is_none()) && !in_forum;
        let aside_width = if members.is_some() {
            theme::MEMBERS_WIDTH
        } else if dm_column.is_some() {
            if self.dm_peer(cx).is_some() { profile::COLUMN_WIDTH } else { theme::MEMBERS_WIDTH }
        } else if panel.is_some() {
            theme::MEMBERS_WIDTH + 100.
        } else {
            0.
        };
        if over_chat {
            self.chat.update(cx, |chat, cx| chat.set_aside_width(aside_width, cx));
        }
        let aside = div()
            .flex()
            .h_full()
            .children(members)
            .children(dm_column)
            .children(panel);
        let profile = self.render_profile(window, cx);
        let settings = self.settings_open.then(|| self.settings_view.clone());
        let menu = self.menu_open.then(|| self.menu.clone());
        let viewer = self.render_viewer(cx);
        let account = self.render_account_popout(cx);
        let status_dialog = self.status_dialog_open.then(|| self.status_dialog.clone());
        let forward = self.forward_open.then(|| self.forward.clone());
        let nickname = self.nickname_open.then(|| self.nickname.clone());
        let join = self.join_open.then(|| self.join.clone());
        let switcher = self.switcher_open.then(|| self.switcher.clone());
        let picker = self.picker_open.then(|| self.picker.clone());
        let popup = self.popup_open.then(|| self.popup.clone());
        let invite = self.invite_open.then(|| self.invite.clone());
        let server_settings = self.server_settings_open.then(|| self.server_settings.clone());
        let inbox = self.inbox_open.then(|| self.inbox.clone());
        let channel_status_modal = self.channel_status_modal.then(|| {
            let on_close = cx.listener(|this, _: &ClickEvent, _, cx| {
                this.channel_status_modal = false;
                cx.notify();
            });
            profile::modal(
                div()
                    .w(px(400.))
                    .h(px(200.))
                    .bg(rgb(theme::BG_PANEL))
                    .rounded(px(theme::RADIUS))
                    .p_4()
                    .child("Set a channel status")
                    .into_any_element(),
                Box::new(on_close),
            )
        });

        let title = self.window_title(cx);
        let root = div().size_full().relative().track_focus(&self.focus);
        self.on_shortcuts(root, cx)
            // A click on bare background parks focus here, so Ctrl+K still
            // works afterwards.
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                if window.focused(cx).is_none() {
                    window.focus(&this.focus, cx);
                }
            }))
            .flex()
            .pt(px(title_bar::HEIGHT))
            .bg(rgb(theme::BG_RAIL))
            .text_color(rgb(theme::TEXT))
            .child(rail)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .rounded_tl(px(theme::RADIUS_MD))
                    .overflow_hidden()
                    .border_t_1().border_l_1()
                    .border_color(rgb(theme::BORDER))
                    .child(sidebar)
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(content)
                            .child(if over_chat {
                                div().absolute().top(px(theme::CHANNEL_HEADER_HEIGHT)).right_0().bottom_0().child(aside).into_any_element()
                            } else {
                                aside.into_any_element()
                            }),
                    ),
            )
            .children(profile)
            .children(account)
            .children(status_dialog)
            .children(menu)
            .children(viewer)
            .children(forward)
            .children(nickname)
            .children(join)
            .children(invite)
            .children(server_settings)
            .children(switcher)
            .children(picker)
            .children(popup)
            .children(inbox)
            .children(channel_status_modal)
            .children(settings)
            // Last, so it stays usable above any dialog.
            .child(title_bar::title_bar(title, window))
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
        .group("rail-item")
        .relative()
        .w_full()
        .h(px(40.))
        .flex()
        .justify_center()
        .cursor_pointer()
        .on_click(on_click)
        // Hovering an item that isn't open stretches its pill to 20px.
        .children((!selected).then(|| {
            div()
                .absolute()
                .left(px(0.))
                .top(px(10.))
                .w(px(4.))
                .h(px(20.))
                .rounded_r(px(4.))
                .bg(rgb(theme::TEXT_BRIGHT))
                .invisible()
                .group_hover("rail-item", |s| s.visible())
        }))
        .child(
            div()
                .absolute()
                .left(px(0.))
                .top(px((40. - pill_height) / 2.))
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
    let radius = px(if selected { 12. } else { 20. });
    let base = div().size(px(40.)).flex_none().rounded(radius).overflow_hidden();

    let image = url.and_then(|url| media.update(cx, |media, _| media.get(url)));
    match image {
        Some(image) => base
            .child(img(ImageSource::Image(image)).size_full().rounded(radius))
            .into_any_element(),
        None => base
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(if selected { theme::ACCENT } else { theme::BG_HOVER }))
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

/// How a chosen status looks as a dot. Invisible shows as offline, which is
/// what everyone else sees.
pub(crate) fn dot_status(status: ClientStatus) -> Status {
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
        status: c.status.clone(),
        user_limit: c.user_limit,
        can_invite: !c.kind.is_thread()
            && store.permissions_in(c).contains(oxicord_model::Permissions::CREATE_INSTANT_INVITE),
        voice: store
            .voice_in(c.id)
            .iter()
            .map(|v| {
                let user = store.user(v.user_id);
                VoiceItem {
                    user: v.user_id,
                    name: user.map_or_else(|| "Unknown".to_owned(), |u| u.display_name().to_owned()),
                    avatar: cdn::user_avatar(v.user_id, user.and_then(|u| u.avatar.as_deref()), 64),
                    muted: v.muted(),
                    deafened: v.deafened(),
                    video: v.self_video,
                    streaming: v.self_stream,
                }
            })
            .collect(),
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
        let marks: Vec<_> = all.iter().map(|s| dot_status(*s)).collect();
        assert_eq!(labels.len(), 4);
        assert!(marks.iter().enumerate().all(|(i, m)| marks.iter().skip(i + 1).all(|n| n != m)));
    }
}
