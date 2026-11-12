use std::{collections::{HashMap, HashSet}, rc::Rc};

use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, ImageSource, InteractiveElement as _,
    IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    FocusHandle, MouseButton, ScrollHandle, StyledImage as _, component::{input::{Escape, InputEvent, InputState}, slider::{SliderEvent, SliderState}}, MouseDownEvent, Pixels, Point, Subscription, Window, div, img, point, px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_gateway::{ClientStatus, ConnectionState};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, EventStatus, RelationshipKind, Snowflake, Status, WelcomeScreen};
use oxicord_store::Unread;

mod call_window;

use crate::{
    account::Load,
    backend::{Backend, Command},
    chat::{ChatView, Shortcut},
    context_menu::{self, CloseMenu, ContextMenu, Target},
    dm_picker::{ClosePicker, DmPicker, Purpose},
    forum::ForumView,
    forward_dialog::{CloseForward, ForwardDialog},
    friend_nick_dialog::{CloseFriendNick, FriendNickDialog},
    ban_dialog::{CloseBan, BanDialog},
    create_poll_dialog::{CloseCreatePoll, CreatePollDialog},
    create_server_dialog::{CloseCreateServer, CreateServerDialog},
    danger_dialog::{CloseDanger, DangerDialog, OpenDanger},
    password_dialog::{ClosePassword, PasswordDialog},
    totp_dialog::{CloseTotp, TotpDialog},
    decoration_dialog::{CloseDecorationDialog, DecorationDialog},
    effect_dialog::{CloseEffectDialog, EffectDialog},
    edit_profile_modal::{CloseEditProfileModal, EditProfileModal, OpenDecoration, OpenEffect, OpenFrame, OpenNameplate},
    frame_dialog::{CloseFrameDialog, FrameDialog},
    nameplate_dialog::{CloseNameplateDialog, NameplateDialog},
    kick_dialog::{CloseKick, KickDialog},
    timeout_dialog::{CloseTimeout, TimeoutDialog},
    roles_dialog::{CloseRoles, RolesDialog},
    nickname_dialog::{CloseNickname, NicknameDialog},
    join_server::{CloseJoin, JoinServerView},
    rules_screening::{CloseRulesScreening, RulesScreeningDialog},
    friends::FriendsView,
    icons::icon,
    message_requests::MessageRequestsView,
    rail::{self, Entry},
    inbox::{CloseInbox, Inbox},
    create_channel_dialog::{CloseCreateChannel, CreateChannelDialog},
    channel_settings::{ChannelSettings, ChannelSettingsEvent, OpenChannelPermissions},
    edit_channel_dialog::{CloseEditChannel, EditChannelDialog},
    gift_dialog::{CloseGift, GiftDialog},
    invite_dialog::{CloseInvite, InviteDialog},
    interaction_modal::{CloseInteractionModal, InteractionModalView},
    keys::{self, QuickSwitch},
    events_dialog::{CloseEvents, EventsDialog},
    onboarding_dialog::{CloseOnboarding, OnboardingDialog},
    media::{Media, ViewImage, acronym, avatar},
    member_list::{MemberList, presence_dot, status_shape},
    popup::{ClosePopup, PopupMenu},
    profile,
    session::{OpenChangePassword, OpenTotpSetup, DmOpened, JumpTo, MentionUser, OpenBanMember, OpenChannelSettings, OpenCreateServer, OpenKickMember, OpenCreateChannel, OpenCreatePoll, OpenEvents, OpenForward, OpenGift, OpenInbox, OpenInteractionModal, OpenPollVoters, OpenInviteDialog, OpenMenu, OpenNickname, OpenPanel, OpenPicker, OpenPopup, OpenPrivacySettings, OpenRolesMember, OpenRulesScreening, OpenServerGuide, OpenServerProfile, OpenServerSettings, OpenTimeoutMember, OpenUserMenu, PanelKind, Session, ShowProfile},
    settings_view::{CloseSettings, OpenEditProfile, SettingsView},
    server_settings::{CloseServerSettings, ServerSettings},
    side_panel::{ClosePanel, SidePanel},
    status_dialog::{CloseStatusDialog, StatusDialog},
    switcher::{Chosen, CloseSwitcher, Goal, QuickSwitcher},
    theme, title_bar, user_menu,
};

/// What the top of a server's channel list shows.
struct GuildHead {
    name: SharedString,
    /// Where the banner picture lives, when the server has one.
    banner: Option<String>,
    tier: u8,
    boosts: u64,
    /// Whether the server shows its Boost Goal bar.
    goal: bool,
}

/// Height of the banner, which runs behind the header and below it.
const BANNER_HEIGHT: f32 = 130.;

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
    /// The tag the other person wears, for one-to-one conversations.
    tag: Option<oxicord_model::ServerTag>,
}

/// The signed in frame: guild rail, channel list, and the content area.
pub struct MainView {
    session: Entity<Session>,
    media: Entity<Media>,
    chat: Entity<ChatView>,
    friends: Entity<FriendsView>,
    message_requests: Entity<MessageRequestsView>,
    forum: Entity<ForumView>,
    members: Entity<MemberList>,
    backend: Backend,
    /// `None` is the home screen with the direct messages.
    guild: Option<Snowflake>,
    channel: Option<Snowflake>,
    /// The welcome screen of the selected server, when it has none selected.
    welcome: Option<WelcomeScreen>,
    /// The server `welcome` belongs to, so a stale answer is ignored.
    welcome_for: Option<Snowflake>,
    welcome_failed: bool,
    /// The message requests page instead of the friends page.
    requests_open: bool,
    /// The channel to return to when switching back to a guild.
    last_channel: HashMap<Snowflake, Snowflake>,
    collapsed: HashSet<Snowflake>,
    /// Where the channel list is scrolled to, which decides whether the
    /// server banner still sits behind the header.
    sidebar_scroll: ScrollHandle,
    status_menu_open: bool,
    /// The list of statuses beside the account popout.
    status_submenu_open: bool,
    status_dialog: Entity<StatusDialog>,
    status_dialog_open: bool,
    edit_profile: Entity<EditProfileModal>,
    edit_profile_open: bool,
    nameplate_dialog: Entity<NameplateDialog>,
    nameplate_open: bool,
    decoration_dialog: Entity<DecorationDialog>,
    decoration_open: bool,
    effect_dialog: Entity<EffectDialog>,
    effect_open: bool,
    frame_dialog: Entity<FrameDialog>,
    frame_open: bool,
    channel_status_modal: bool,
    /// The profile card on screen, if any.
    profile: Option<ShowProfile>,
    /// The tab of the full profile.
    profile_tab: profile::Tab,
    /// The effect playing over that card, kept so it is not started over
    /// every time the card is drawn: whose card, whether the full one, and the view.
    profile_effect: Option<(Snowflake, bool, Entity<profile::EffectView>)>,
    /// The editable note box shown in a profile card.
    note_input: Entity<InputState>,
    /// Which profile the note box was last filled for, so we don't overwrite
    /// what the user is typing when the profile reloads.
    note_prefilled_for: Option<Snowflake>,
    /// The message box at the bottom of the small card.
    card_message: Entity<InputState>,
    /// Whose card the message box was last emptied for.
    card_message_for: Option<Snowflake>,
    /// A message typed on a card for someone we have no conversation with
    /// yet, sent once the conversation exists.
    pending_card_message: Option<(Snowflake, String)>,
    settings_open: bool,
    /// Whether the voice channel's text chat is shown beside the call.
    call_chat_open: bool,
    /// The window the call was popped out into, while there is one.
    popout: Option<gpui_kit::AnyWindowHandle>,
    /// Redraws once a second while in a call, for the clock.
    _clock: gpui_kit::Task<()>,
    settings_view: Entity<SettingsView>,
    panel: Entity<SidePanel>,
    panel_open: bool,
    menu: Entity<ContextMenu>,
    menu_open: bool,
    /// The picture open in the full size viewer.
    viewer: Option<String>,
    join: Entity<JoinServerView>,
    join_open: bool,
    screening: Entity<RulesScreeningDialog>,
    screening_open: bool,
    onboarding: Entity<OnboardingDialog>,
    onboarding_open: bool,
    events: Entity<EventsDialog>,
    events_open: bool,
    interaction_modal: Entity<InteractionModalView>,
    interaction_modal_open: bool,
    forward: Entity<ForwardDialog>,
    forward_open: bool,
    nickname: Entity<NicknameDialog>,
    nickname_open: bool,
    friend_nick: Entity<FriendNickDialog>,
    friend_nick_open: bool,
    ban_dialog: Entity<BanDialog>,
    ban_dialog_open: bool,
    create_server_dialog: Entity<CreateServerDialog>,
    create_server_open: bool,
    password_dialog: Entity<PasswordDialog>,
    password_open: bool,
    totp_dialog: Entity<TotpDialog>,
    totp_open: bool,
    danger_dialog: Entity<DangerDialog>,
    danger_open: bool,
    pub poll_voters_open: bool,
    pub poll_voters: Vec<oxicord_model::User>,
    create_poll_dialog: Entity<CreatePollDialog>,
    create_poll_open: bool,
    kick_dialog: Entity<KickDialog>,
    kick_dialog_open: bool,
    timeout_dialog: Entity<TimeoutDialog>,
    timeout_dialog_open: bool,
    roles_dialog: Entity<RolesDialog>,
    roles_dialog_open: bool,
    switcher: Entity<QuickSwitcher>,
    switcher_open: bool,
    picker: Entity<DmPicker>,
    popup: Entity<PopupMenu>,
    popup_open: bool,
    /// Listens to the volume slider in the open user menu.
    volume_sub: Option<Subscription>,
    /// Folders opened in the rail.
    expanded: HashSet<u64>,
    picker_open: bool,
    inbox: Entity<Inbox>,
    inbox_open: bool,
    invite: Entity<InviteDialog>,
    invite_open: bool,
    edit_channel: Entity<EditChannelDialog>,
    edit_channel_open: bool,
    channel_settings: Entity<ChannelSettings>,
    channel_settings_open: bool,
    create_channel: Entity<CreateChannelDialog>,
    create_channel_open: bool,
    gift_dialog: Entity<GiftDialog>,
    gift_open: bool,
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
            s.request_saved_messages(&backend, cx);
        });
        let panel = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| SidePanel::new(session, media, backend, window, cx))
        };
        let menu = cx.new(|_| ContextMenu::new(session.clone(), media.clone(), backend.clone()));
        let join = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| JoinServerView::new(session, media, backend, window, cx))
        };
        let screening = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| RulesScreeningDialog::new(session, media, backend, cx))
        };
        let onboarding = {
            let backend = backend.clone();
            cx.new(|_| OnboardingDialog::new(backend))
        };
        let events = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|_| EventsDialog::new(session, media, backend))
        };
        let interaction_modal = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| InteractionModalView::new(session, backend, cx))
        };
        let status_dialog = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| StatusDialog::new(session, media, backend, window, cx))
        };
        let edit_profile = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| EditProfileModal::new(session, media, backend, window, cx))
        };
        let nameplate_dialog = {
            let session = session.clone();
            let media = media.clone();
            cx.new(|_| NameplateDialog::new(session, media))
        };
        let decoration_dialog = {
            let session = session.clone();
            let media = media.clone();
            cx.new(|_| DecorationDialog::new(session, media))
        };
        let effect_dialog = {
            let session = session.clone();
            let media = media.clone();
            cx.new(|_| EffectDialog::new(session, media))
        };
        let frame_dialog = {
            let session = session.clone();
            let media = media.clone();
            cx.new(|_| FrameDialog::new(session, media))
        };
        let forward = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| ForwardDialog::new(session, media, backend, window, cx))
        };
        let nickname = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| NicknameDialog::new(session, backend, window, cx))
        };
        let friend_nick = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| FriendNickDialog::new(session, backend, window, cx))
        };
        let ban_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| BanDialog::new(session, backend, window, cx))
        };
        let create_server_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| CreateServerDialog::new(session, backend, window, cx))
        };
        let password_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| PasswordDialog::new(session, backend, window, cx))
        };
        let totp_dialog = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| TotpDialog::new(session, media, backend, window, cx))
        };
        let danger_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| DangerDialog::new(session, backend, window, cx))
        };
        let create_poll_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| CreatePollDialog::new(session, backend, window, cx))
        };
        let kick_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| KickDialog::new(session, backend, window, cx))
        };
        let timeout_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| TimeoutDialog::new(session, backend, window, cx))
        };
        let roles_dialog = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| RolesDialog::new(session, backend, window, cx))
        };
        let note_input = cx.new(|cx| InputState::new(window, cx).placeholder("Click to add a note"));
        let card_message = cx.new(|cx| InputState::new(window, cx));
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
        let edit_channel = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| EditChannelDialog::new(session, backend, window, cx))
        };
        let channel_settings = cx.new(|_| ChannelSettings::new(session.clone(), backend.clone()));
        let create_channel = {
            let (session, backend) = (session.clone(), backend.clone());
            cx.new(|cx| CreateChannelDialog::new(session, backend, window, cx))
        };
        let gift_dialog = {
            let session = session.clone();
            cx.new(|cx| GiftDialog::new(session, window, cx))
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
            cx.subscribe(&session, |this, _, _: &OpenPrivacySettings, cx| {
                this.settings_open = true;
                this.settings_view.update(cx, |view, cx| view.open_data_privacy(cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenServerProfile, window, cx| {
                this.server_settings_open = true;
                this.server_settings.update(cx, |view, cx| view.opened_profile(open.0, window, cx));
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
            cx.subscribe(&edit_channel, |this, _, _: &CloseEditChannel, cx| {
                this.edit_channel_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenChannelSettings, window, cx| {
                this.edit_channel_open = true;
                this.edit_channel.update(cx, |dialog, cx| dialog.opened(open.0, window, cx));
                cx.notify();
            }),
            cx.subscribe(&channel_settings, |this, _, event: &ChannelSettingsEvent, cx| {
                this.channel_settings_open = false;
                // The Overview tab is the older name and topic dialog.
                if let ChannelSettingsEvent::Overview(channel) = event {
                    let channel = *channel;
                    this.session.update(cx, |_, cx| cx.emit(OpenChannelSettings(channel)));
                }
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, open: &OpenChannelPermissions, cx| {
                this.channel_settings_open = true;
                this.channel_settings.update(cx, |screen, cx| screen.opened(open.0, open.1, cx));
                cx.notify();
            }),
            cx.subscribe(&create_channel, |this, _, _: &CloseCreateChannel, cx| {
                this.create_channel_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenCreateChannel, window, cx| {
                this.create_channel_open = true;
                this.create_channel.update(cx, |dialog, cx| dialog.opened(open.0, open.1, window, cx));
                cx.notify();
            }),
            cx.subscribe(&gift_dialog, |this, _, _: &CloseGift, cx| {
                this.gift_open = false;
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, _: &OpenGift, cx| {
                this.gift_open = true;
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
            cx.subscribe_in(&settings_view, window, |this, _, _: &OpenEditProfile, window, cx| {
                this.settings_open = false;
                cx.defer_in(window, |this, window, cx| {
                    this.edit_profile_open = true;
                    this.edit_profile.update(cx, |modal, cx| modal.opened(window, cx));
                    cx.notify();
                });
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
            cx.subscribe(&nameplate_dialog, |this, _, CloseNameplateDialog(applied): &CloseNameplateDialog, cx| {
                this.nameplate_open = false;
                let applied = applied.clone();
                this.edit_profile.update(cx, |modal, cx| modal.set_applied(applied, cx));
                cx.notify();
            }),
            cx.subscribe(&edit_profile, |this, _, _: &OpenNameplate, cx| {
                this.nameplate_open = true;
                let applied = this.edit_profile.read(cx).applied_nameplate();
                this.nameplate_dialog.update(cx, |dialog, cx| dialog.opened(applied, cx));
                cx.notify();
            }),
            cx.subscribe(
                &decoration_dialog,
                |this, _, CloseDecorationDialog(applied): &CloseDecorationDialog, cx| {
                    this.decoration_open = false;
                    let applied = applied.clone();
                    this.edit_profile.update(cx, |modal, cx| modal.set_applied_decoration(applied, cx));
                    cx.notify();
                },
            ),
            cx.subscribe(&edit_profile, |this, _, _: &OpenDecoration, cx| {
                this.decoration_open = true;
                let applied = this.edit_profile.read(cx).applied_decoration();
                this.decoration_dialog.update(cx, |dialog, cx| dialog.opened(applied, cx));
                cx.notify();
            }),
            cx.subscribe(&effect_dialog, |this, _, CloseEffectDialog(applied): &CloseEffectDialog, cx| {
                this.effect_open = false;
                let applied = applied.clone();
                this.edit_profile.update(cx, |modal, cx| modal.set_applied_effect(applied, cx));
                cx.notify();
            }),
            cx.subscribe(&edit_profile, |this, _, _: &OpenEffect, cx| {
                this.effect_open = true;
                let applied = this.edit_profile.read(cx).applied_effect();
                this.effect_dialog.update(cx, |dialog, cx| dialog.opened(applied, cx));
                cx.notify();
            }),
            cx.subscribe(&frame_dialog, |this, _, CloseFrameDialog(applied): &CloseFrameDialog, cx| {
                this.frame_open = false;
                let applied = applied.clone();
                this.edit_profile.update(cx, |modal, cx| modal.set_applied_frame(applied, cx));
                cx.notify();
            }),
            cx.subscribe(&edit_profile, |this, _, _: &OpenFrame, cx| {
                this.frame_open = true;
                let applied = this.edit_profile.read(cx).applied_frame();
                this.frame_dialog.update(cx, |dialog, cx| dialog.opened(applied, cx));
                cx.notify();
            }),
            cx.subscribe(&edit_profile, |this, _, _: &CloseEditProfileModal, cx| {
                this.edit_profile_open = false;
                cx.notify();
            }),
            cx.subscribe(&nickname, |this, _, _: &CloseNickname, cx| {
                this.nickname_open = false;
                cx.notify();
            }),
            cx.subscribe(&friend_nick, |this, _, _: &CloseFriendNick, cx| {
                this.friend_nick_open = false;
                cx.notify();
            }),
            cx.subscribe(&ban_dialog, |this, _, _: &CloseBan, cx| {
                this.ban_dialog_open = false;
                cx.notify();
            }),
            cx.subscribe(&create_server_dialog, |this, _, _: &CloseCreateServer, cx| {
                this.create_server_open = false;
                cx.notify();
            }),
            cx.subscribe(&password_dialog, |this, _, _: &ClosePassword, cx| {
                this.password_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, _: &OpenChangePassword, window, cx| {
                this.password_open = true;
                this.password_dialog.update(cx, |dialog, cx| dialog.opened(window, cx));
                cx.notify();
            }),
            cx.subscribe(&totp_dialog, |this, _, _: &CloseTotp, cx| {
                this.totp_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, _: &OpenTotpSetup, window, cx| {
                this.totp_open = true;
                this.totp_dialog.update(cx, |dialog, cx| dialog.opened(window, cx));
                cx.notify();
            }),
            cx.subscribe(&danger_dialog, |this, _, _: &CloseDanger, cx| {
                this.danger_open = false;
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenDanger, window, cx| {
                this.danger_open = true;
                this.danger_dialog.update(cx, |dialog, cx| dialog.opened(open.0, window, cx));
                cx.notify();
            }),
            cx.subscribe(&create_poll_dialog, |this, _, _: &CloseCreatePoll, cx| {
                this.create_poll_open = false;
                cx.notify();
            }),
            cx.subscribe(&kick_dialog, |this, _, _: &CloseKick, cx| {
                this.kick_dialog_open = false;
                cx.notify();
            }),
            cx.subscribe(&timeout_dialog, |this, _, _: &CloseTimeout, cx| {
                this.timeout_dialog_open = false;
                cx.notify();
            }),
            cx.subscribe(&roles_dialog, |this, _, _: &CloseRoles, cx| {
                this.roles_dialog_open = false;
                cx.notify();
            }),
            cx.subscribe(&note_input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save_note(cx);
                }
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenNickname, window, cx| {
                this.nickname_open = true;
                this.nickname.update(cx, |dialog, cx| dialog.opened(open.0, window, cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, _: &OpenCreateServer, window, cx| {
                this.create_server_open = true;
                this.create_server_dialog.update(cx, |dialog, cx| dialog.opened(window, cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenPollVoters, _, cx| {
                this.poll_voters_open = true;
                this.poll_voters.clear();
                this.backend.send(Command::LoadPollVoters { channel: open.channel, message: open.message, answer: open.answer });
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenCreatePoll, window, cx| {
                this.create_poll_open = true;
                this.create_poll_dialog.update(cx, |dialog, cx| dialog.opened(open.channel, open.guild, window, cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenBanMember, window, cx| {
                this.ban_dialog_open = true;
                let name = this
                    .session
                    .read(cx)
                    .store
                    .user(open.1)
                    .map(|u| u.display_name().to_owned())
                    .unwrap_or_default();
                this.ban_dialog.update(cx, |dialog, cx| dialog.opened(open.0, open.1, name, window, cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenKickMember, window, cx| {
                this.kick_dialog_open = true;
                let name = this
                    .session
                    .read(cx)
                    .store
                    .user(open.1)
                    .map(|u| u.display_name().to_owned())
                    .unwrap_or_default();
                this.kick_dialog.update(cx, |dialog, cx| dialog.opened(open.0, open.1, name, window, cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenTimeoutMember, window, cx| {
                this.timeout_dialog_open = true;
                let name = this
                    .session
                    .read(cx)
                    .store
                    .user(open.1)
                    .map(|u| u.display_name().to_owned())
                    .unwrap_or_default();
                this.timeout_dialog.update(cx, |dialog, cx| dialog.opened(open.0, open.1, name, window, cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenRolesMember, window, cx| {
                this.roles_dialog_open = true;
                let name = this
                    .session
                    .read(cx)
                    .store
                    .user(open.1)
                    .map(|u| u.display_name().to_owned())
                    .unwrap_or_default();
                this.roles_dialog.update(cx, |dialog, cx| dialog.opened(open.0, open.1, name, window, cx));
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
            cx.subscribe(&screening, |this, _, _: &CloseRulesScreening, cx| {
                this.screening_open = false;
                this.session.update(cx, |session, cx| session.close_verification(cx));
                cx.notify();
            }),
            cx.subscribe(&onboarding, |this, _, _: &CloseOnboarding, cx| {
                this.onboarding_open = false;
                cx.notify();
            }),
            cx.subscribe(&interaction_modal, |this, _, _: &CloseInteractionModal, cx| {
                this.interaction_modal_open = false;
                this.session
                    .update(cx, |session, cx| session.dismiss_interaction_modal(cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenInteractionModal, window, cx| {
                this.interaction_modal_open = true;
                this.interaction_modal
                    .update(cx, |view, cx| view.opened(open.0.clone(), window, cx));
                cx.notify();
            }),
            cx.subscribe(&events, |this, _, _: &CloseEvents, cx| {
                this.events_open = false;
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, open: &OpenEvents, cx| {
                this.events_open = true;
                this.events.update(cx, |dialog, cx| dialog.opened(open.0, cx));
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, open: &OpenServerGuide, cx| {
                this.onboarding_open = true;
                this.onboarding.update(cx, |dialog, cx| dialog.opened(open.0, cx));
                cx.notify();
            }),
            cx.subscribe_in(&session, window, |this, _, open: &OpenRulesScreening, window, cx| {
                this.open_screening(open.guild, window, cx);
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
                if this.send_pending_card_message(opened.0, cx) {
                    return;
                }
                this.show_dm(opened.0, window, cx);
            }),
            cx.subscribe_in(&card_message, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.send_card_message(window, cx);
                }
            }),
            cx.subscribe(&session, |this, _, open: &OpenMenu, cx| {
                this.open_menu(open.target, open.at, cx);
            }),
            cx.subscribe(&session, |this, _, shown: &ShowProfile, cx| {
                this.show_profile(shown.user, shown.guild, shown.at, shown.full, cx);
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
        let message_requests = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| MessageRequestsView::new(session, media, backend, cx))
        };
        let forum = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| ForumView::new(session, media, backend, window, cx))
        };
        let clock = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
            let alive = this.update(cx, |this, cx| {
                if this.session.read(cx).voice.since.is_some() {
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        });

        Self {
            session,
            media,
            chat,
            friends,
            message_requests,
            forum,
            members,
            backend,
            guild: None,
            channel: None,
            welcome: None,
            welcome_for: None,
            welcome_failed: false,
            requests_open: false,
            last_channel: HashMap::new(),
            collapsed: HashSet::new(),
            sidebar_scroll: ScrollHandle::new(),
            status_menu_open: false,
            status_submenu_open: false,
            status_dialog,
            status_dialog_open: false,
            edit_profile,
            edit_profile_open: false,
            nameplate_dialog,
            nameplate_open: false,
            decoration_dialog,
            decoration_open: false,
            effect_dialog,
            effect_open: false,
            frame_dialog,
            frame_open: false,
            channel_status_modal: false,
            profile: None,
            profile_tab: profile::Tab::Board,
            settings_open: false,
            call_chat_open: false,
            popout: None,
            _clock: clock,
            settings_view,
            members_shown,
            panel,
            panel_open: false,
            menu,
            menu_open: false,
            viewer: None,
            join,
            join_open: false,
            screening,
            screening_open: false,
            onboarding,
            onboarding_open: false,
            events,
            events_open: false,
            interaction_modal,
            interaction_modal_open: false,
            forward,
            forward_open: false,
            nickname,
            nickname_open: false,
            friend_nick,
            friend_nick_open: false,
            ban_dialog,
            ban_dialog_open: false,
            create_server_dialog,
            create_server_open: false,
            password_dialog,
            password_open: false,
            totp_dialog,
            totp_open: false,
            danger_dialog,
            danger_open: false,
            poll_voters_open: false,
            poll_voters: Vec::new(),
            create_poll_dialog,
            create_poll_open: false,
            kick_dialog,
            kick_dialog_open: false,
            timeout_dialog,
            timeout_dialog_open: false,
            roles_dialog,
            roles_dialog_open: false,
            note_input,
            profile_effect: None,
            note_prefilled_for: None,
            card_message,
            card_message_for: None,
            pending_card_message: None,
            switcher,
            switcher_open: false,
            picker,
            popup,
            popup_open: false,
            volume_sub: None,
            expanded: HashSet::new(),
            picker_open: false,
            inbox,
            inbox_open: false,
            invite,
            invite_open: false,
            edit_channel,
            edit_channel_open: false,
            channel_settings,
            channel_settings_open: false,
            create_channel,
            create_channel_open: false,
            gift_dialog,
            gift_open: false,
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
        self.requests_open = false;
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
        if let Some(guild) = guild
            && self.channel.is_none()
        {
            self.ensure_welcome(guild, cx);
        }
        self.sync_chat(window, cx);
        cx.notify();
    }

    /// Asks for the server's welcome screen the first time the server is
    /// shown without a selected channel.
    fn ensure_welcome(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        if self.welcome_for == Some(guild) {
            return;
        }
        self.welcome_for = Some(guild);
        self.welcome = None;
        self.welcome_failed = false;
        self.backend.send(Command::LoadWelcomeScreen { guild });
        cx.notify();
    }

    /// The answer to [`Command::LoadWelcomeScreen`], if it is still the
    /// server on screen.
    pub(crate) fn apply_welcome_screen(
        &mut self,
        guild: Snowflake,
        result: Result<WelcomeScreen, String>,
        cx: &mut Context<Self>,
    ) {
        if self.guild != Some(guild) {
            return;
        }
        match result {
            Ok(screen) => {
                self.welcome = Some(screen);
                self.welcome_failed = false;
            }
            Err(_) => {
                self.welcome = None;
                self.welcome_failed = true;
            }
        }
        cx.notify();
    }

    fn select_channel(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.channel = Some(channel);
        self.requests_open = false;
        if let Some(guild) = self.guild {
            self.last_channel.insert(guild, channel);
        }
        self.sync_chat(window, cx);
        cx.notify();
    }

    /// Opens the membership screening dialog for `guild` and asks for its
    /// form and card.
    fn open_screening(&mut self, guild: Snowflake, _: &mut Window, cx: &mut Context<Self>) {
        self.screening_open = true;
        self.screening.update(cx, |view, cx| view.opened(guild, cx));
        let backend = self.backend.clone();
        self.session.update(cx, |session, cx| session.request_verification(&backend, guild, cx));
        cx.notify();
    }

    /// What the title bar names: the server, the conversation, or Friends.
    fn window_title(&self, cx: &App) -> SharedString {
        let store = &self.session.read(cx).store;
        let name = match (self.guild, self.channel) {
            (Some(guild), _) => store.guild(guild).map(|g| g.name.clone()),
            (None, Some(channel)) => store.channel(channel).map(|c| store.dm_name(c)),
            (None, None) if self.requests_open => Some("Message Requests".to_owned()),
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

    /// Every server in the order the rail shows them, folders opened out.
    fn server_order(&self, cx: &App) -> Vec<Snowflake> {
        let known: Vec<Snowflake> = self.session.read(cx).store.guilds().map(|g| g.id).collect();
        let layout = match &self.session.read(cx).account.settings {
            Load::Ready(settings) => settings.folders().to_vec(),
            _ => Vec::new(),
        };
        let mut order = Vec::new();
        for entry in rail::arrange(&layout, &known) {
            match entry {
                Entry::Guild(id) => order.push(id),
                Entry::Folder { guilds, .. } => order.extend(guilds),
            }
        }
        order
    }

    /// Moves along the rail, home first, then servers in folder order.
    fn step_server(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let order: Vec<Option<Snowflake>> =
            std::iter::once(None).chain(self.server_order(cx).into_iter().map(Some)).collect();
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
            .on_action(cx.listener(|this, _: &Escape, _, cx| {
                // An open profile card is what Escape closes first.
                if this.screening_open {
                    this.screening_open = false;
                    this.session.update(cx, |session, cx| session.close_verification(cx));
                    cx.notify();
                } else if this.profile.take().is_some() {
                    cx.notify();
                } else {
                    this.mark_channel_read(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &keys::MarkServerRead, _, cx| {
                if let Some(guild) = this.guild {
                    context_menu::mark_guild_read(&this.session, &this.backend, guild, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &keys::TogglePins, window, cx| this.toggle_panel(PanelKind::Pins, window, cx)))
            .on_action(cx.listener(|this, _: &keys::Search, window, cx| {
                if this.channel.is_some() {
                    this.chat.update(cx, |chat, cx| chat.focus_search(window, cx));
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

    /// Joins a voice channel of the open server and shows the call panel.
    fn join_voice(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        let (guild, backend) = (self.guild, self.backend.clone());
        self.session
            .update(cx, |session, cx| session.join_voice(&backend, guild, channel, cx));
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

    /// Runs one command from the debug control file, see `dev_control`.
    #[cfg(debug_assertions)]
    pub(crate) fn dev_command(
        &mut self,
        command: crate::dev_control::DevCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::dev_control::DevCommand;
        let backend = self.backend.clone();
        match command {
            DevCommand::Goto(channel) => self.show_channel(channel, None, window, cx),
            DevCommand::Join(channel) => {
                self.show_channel(channel, None, window, cx);
                self.join_voice(channel, cx);
            }
            DevCommand::Leave => self.session.update(cx, |s, cx| s.leave_voice(&backend, cx)),
            DevCommand::Mute => self.session.update(cx, |s, cx| s.toggle_voice_mute(&backend, cx)),
            DevCommand::Deafen => self.session.update(cx, |s, cx| s.toggle_voice_deafen(&backend, cx)),
            DevCommand::Profile(user, guild) | DevCommand::Full(user, guild) if user == Snowflake(0) => {
                let me = self.session.read(cx).store.me().map(|u| u.id);
                if let Some(me) = me {
                    let full = matches!(command, DevCommand::Full(..));
                    let at = gpui_kit::point(window.viewport_size().width - px(100.), px(146.));
                    self.show_profile(me, guild, (!full).then_some(at), full, cx);
                }
            }
            DevCommand::Profile(user, guild) => {
                // As if the person was clicked in the member list.
                let at = gpui_kit::point(window.viewport_size().width - px(100.), px(146.));
                self.show_profile(user, guild, Some(at), false, cx);
            }
            DevCommand::Full(user, guild) => self.show_profile(user, guild, None, true, cx),
            DevCommand::UserMenu(user) => {
                // `me` stands for the first friend, since the menu for ourselves has no volume.
                let user = if user == Snowflake(0) {
                    let session = self.session.read(cx);
                    let friend = session.store.relationships(oxicord_model::RelationshipKind::Friend).first().map(|u| u.id);
                    match friend {
                        Some(friend) => friend,
                        None => return,
                    }
                } else {
                    user
                };
                let at = gpui_kit::point(window.viewport_size().width / 2., px(300.));
                let open = OpenUserMenu { user, guild: None, at, dm: None, can_mention: false };
                self.show_user_menu(&open, true, cx);
            }
            DevCommand::Rules(guild) => self.open_screening(guild, window, cx),
            DevCommand::Danger(kind) => {
                use crate::danger_dialog::Danger;
                let kind = [Danger::DisableAccount, Danger::DeleteAccount, Danger::RemoveTotp][usize::from(kind).min(2)];
                self.danger_open = true;
                self.danger_dialog.update(cx, |dialog, cx| dialog.opened(kind, window, cx));
                cx.notify();
            }
            DevCommand::ChannelPermissions(channel, dry) => {
                self.session.update(cx, |_, cx| cx.emit(OpenChannelPermissions(channel, dry)));
            }
            DevCommand::Automod(guild) => {
                self.server_settings_open = true;
                self.server_settings.update(cx, |view, cx| view.opened_automod(guild, window, cx));
                cx.notify();
            }
            DevCommand::Settings(guild, page) => {
                self.server_settings_open = true;
                self.server_settings.update(cx, |view, cx| view.opened_page(guild, &page, window, cx));
                cx.notify();
            }
            DevCommand::Webhooks(guild) => {
                self.server_settings_open = true;
                self.server_settings.update(cx, |view, cx| view.opened_webhooks(guild, window, cx));
                cx.notify();
            }
            DevCommand::Manageable => {
                let lines: Vec<String> = {
                    let s = self.session.read(cx);
                    s.store
                        .guilds()
                        .filter(|g| crate::server_settings::can_manage(s.store.guild_permissions(g.id)))
                        .map(|g| format!("{} {}", g.id.0, g.name))
                        .collect()
                };
                let _ = std::fs::write(std::env::temp_dir().join("oxicord-manageable.txt"), lines.join("
"));
            }
            DevCommand::TotpCodes => {
                self.totp_open = true;
                self.totp_dialog.update(cx, |dialog, cx| dialog.preview_codes(window, cx));
                cx.notify();
            }
            DevCommand::Interested(event) => {
                self.session.update(cx, |session, cx| session.set_event_interest(event, true, cx));
            }
            DevCommand::Tab(tab) => {
                self.profile_tab = [profile::Tab::Board, profile::Tab::Activity, profile::Tab::Friends, profile::Tab::Servers]
                    [usize::from(tab).min(3)];
                cx.notify();
            }
            DevCommand::Close => {
                self.profile = None;
                cx.notify();
            }
            DevCommand::Resize(w, h) => {
                window.resize(gpui_kit::size(gpui_kit::px(w as f32), gpui_kit::px(h as f32)));
            }
            DevCommand::FakeRequest(channel, user) => {
                let data = serde_json::json!({
                    "id": channel.0.to_string(), "type": 1,
                    "recipients": [{ "id": user.0.to_string(), "username": "stranger", "global_name": "Stranger" }],
                    "last_message_id": channel.0.to_string(),
                    "is_message_request": true,
                    "is_message_request_timestamp": "2026-11-09T09:00:00.000+00:00"
                });
                let Ok(channel) = serde_json::from_value(data) else { return };
                self.session.update(cx, |session, cx| {
                    let changes = session.store.apply(oxicord_model::gateway::DispatchEvent::ChannelCreate(
                        Box::new(channel),
                    ));
                    cx.emit(changes);
                });
                cx.notify();
            }
            DevCommand::FakeSpam(channel, user) => {
                let data = serde_json::json!({
                    "id": channel.0.to_string(), "type": 1,
                    "recipients": [{ "id": user.0.to_string(), "username": "stranger", "global_name": "Stranger" }],
                    "last_message_id": channel.0.to_string(),
                    "is_message_request": true,
                    "is_spam": true,
                    "is_message_request_timestamp": "2026-11-09T09:00:00.000+00:00"
                });
                let Ok(channel) = serde_json::from_value(data) else { return };
                self.session.update(cx, |session, cx| {
                    let changes = session.store.apply(oxicord_model::gateway::DispatchEvent::ChannelCreate(
                        Box::new(channel),
                    ));
                    cx.emit(changes);
                });
                cx.notify();
            }
            DevCommand::Modal => {
                if let Ok(text) = std::fs::read_to_string("fixtures/interaction-modal.json")
                    && let Ok(modal) = serde_json::from_str(&text)
                {
                    self.session
                        .update(cx, |session, cx| session.open_interaction_modal(modal, cx));
                }
            }
            DevCommand::ModalFail => {
                if self.interaction_modal_open {
                    self.interaction_modal
                        .update(cx, |view, cx| view.fail(cx));
                }
            }
            DevCommand::Form(name) => {
                self.chat
                    .update(cx, |chat, cx| chat.open_named_form(&name, window, cx));
            }
        }
    }

    /// Goes to a direct message from wherever we are, for example after
    /// pressing the message button on the friends page.
    fn show_dm(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = None;
        self.select_channel(channel, window, cx);
    }

    /// Opens the small card for a person, beside `at` when we know where the
    /// click was.
    fn open_profile(&mut self, user: Snowflake, guild: Option<Snowflake>, at: Option<Point<Pixels>>, cx: &mut Context<Self>) {
        self.show_profile(user, guild, at, false, cx);
    }

    fn show_profile(
        &mut self,
        user: Snowflake,
        guild: Option<Snowflake>,
        at: Option<Point<Pixels>>,
        full: bool,
        cx: &mut Context<Self>,
    ) {
        let fetch = self.session.update(cx, |s, _| s.should_fetch_profile(user, guild));
        if fetch {
            self.backend.send(Command::LoadProfile { user, guild });
        }
        self.profile = Some(ShowProfile { user, guild, at, full });
        // A card opened again plays its effect from the start.
        self.profile_effect = None;
        // Let the note box refill from the stored note when it next renders.
        self.note_prefilled_for = None;
        cx.notify();
    }

    /// The date as the profile cards write it, in our own time zone:
    /// `Sep 7, 2025`.
    fn card_date(date: chrono::DateTime<chrono::FixedOffset>) -> String {
        date.with_timezone(&chrono::Local).format("%b %-d, %Y").to_string()
    }

    /// Gathers what a person's card shows. `guild` adds their nickname and roles there.
    fn build_card(&self, user: Snowflake, guild: Option<Snowflake>, cx: &mut App) -> Option<profile::Card> {
        let rail_order = self.server_order(cx);
        let avatar_url;
        let mut card = {
            let session = self.session.read(cx);
            let store = &session.store;
            let account = store.user(user)?;
            let member = guild.and_then(|g| store.member_in_guild(g, user));
            let profile = session.profiles.get(&user);
            // Roles, nickname, join date and the server's own picture and
            // banner belong to the server the profile was asked for in.
            let scoped = profile.filter(|p| p.member_guild == guild);
            avatar_url = match (guild, scoped.and_then(|p| p.server_avatar.as_deref())) {
                (Some(guild), Some(hash)) => cdn::member_avatar(guild, user, hash, 128),
                _ => cdn::user_avatar(user, account.avatar.as_deref(), 128),
            };
            let banner_url = match (guild, scoped.and_then(|p| p.server_banner.as_deref())) {
                (Some(guild), Some(hash)) => Some(cdn::member_banner(guild, user, hash, 600)),
                _ => profile.and_then(|p| p.banner.as_deref()).map(|hash| cdn::user_banner(user, hash, 600)),
            };

            // The member list knows roles only for people it has scrolled past,
            // so the profile's own list fills in for the rest.
            let roles: Vec<(String, Option<u32>)> = match (&member, guild.and_then(|g| store.guild(g)), profile) {
                (Some((_, roles)), _, _) => {
                    roles.iter().map(|r| (r.name.clone(), (r.color != 0).then_some(r.color))).collect()
                }
                (None, Some(server), Some(profile)) if profile.member_guild == guild => {
                    let mut listed: Vec<_> =
                        server.roles.iter().filter(|r| r.id != server.id && profile.member_roles.contains(&r.id)).collect();
                    listed.sort_by_key(|r| std::cmp::Reverse(r.position));
                    listed.iter().map(|r| (r.name.clone(), (r.color != 0).then_some(r.color))).collect()
                }
                _ => Vec::new(),
            };

            let date = |text: &str| {
                chrono::DateTime::parse_from_rfc3339(text)
                    .ok()
                    .or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S")
                            .ok()
                            .map(|d| d.and_utc().fixed_offset())
                    })
                    .map(Self::card_date)
            };
            let game = |entry: &oxicord_model::GameEntry| {
                let known = profile.and_then(|p| p.games.get(&entry.id));
                profile::Game {
                    name: known.map(|g| g.name.clone()).unwrap_or_default(),
                    cover: known.and_then(|g| g.cover_url.clone()),
                    comment: entry.comment.clone(),
                    tags: entry.tags.clone(),
                }
            };
            let widget = |title: &'static str, featured: bool, entries: Option<&Vec<oxicord_model::GameEntry>>| {
                entries.filter(|e| !e.is_empty()).map(|entries| profile::Widget {
                    title,
                    featured,
                    games: entries.iter().map(game).collect(),
                })
            };
            let board: Vec<profile::Widget> = profile
                .map(|p| {
                    [
                        widget("Favorite Game", true, Some(&p.favorite_games)),
                        widget("Games I Like", false, Some(&p.played_games)),
                        widget("Want to Play", false, Some(&p.want_to_play_games)),
                    ]
                    .into_iter()
                    .flatten()
                    .collect()
                })
                .unwrap_or_default();

            let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
            let activities = store
                .activities(user)
                .iter()
                .filter(|a| !a.is_custom_status())
                .map(|a| profile::ActivityCard {
                    header: match a.verb() {
                        Some(verb) => format!("{verb} {}", a.name),
                        None => a.name.clone(),
                    },
                    title: a.details.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| a.name.clone()),
                    line: a.state.clone().unwrap_or_default(),
                    image: a.image_url(),
                    progress: a
                        .timestamps
                        .and_then(|t| Some((t.start?, t.end?)))
                        .map(|(start, end)| (now.saturating_sub(start), end.saturating_sub(start))),
                })
                .collect();

            profile::Card {
                name: member
                    .as_ref()
                    .and_then(|(nick, _)| nick.clone())
                    .or_else(|| scoped.and_then(|p| p.member_nick.clone()))
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| account.display_name().to_owned()),
                username: account.username.clone(),
                bot: account.bot,
                pronouns: profile.map(|p| p.pronouns.clone()).unwrap_or_default(),
                avatar_url: avatar_url.clone(),
                decoration: account.decoration(),
                status: store.presence(user),
                bio: profile.map(|p| p.bio.clone()).unwrap_or_default(),
                created: Self::card_date(user.datetime().fixed_offset()),
                roles,
                banner_url,
                accent: profile.and_then(|p| p.accent_color).unwrap_or(0),
                badges: profile
                    .map(|p| p.badges.iter().map(|b| (b.description.clone(), cdn::badge_icon(&b.icon))).collect())
                    .unwrap_or_default(),
                tag: profile.and_then(|p| p.server_tag.as_ref()).map(|t| {
                    (t.tag.clone(), t.badge.as_deref().map(|b| cdn::clan_badge(t.guild_id, b, 32)))
                }),
                joined_server: scoped
                    .and_then(|p| p.joined_server.as_deref())
                    .and_then(date)
                    .unwrap_or_default(),
                server: guild.and_then(|g| store.guild(g)).map(|g| {
                    (g.name.clone(), g.icon.as_deref().map(|h| cdn::guild_icon(g.id, h, 32)))
                }),
                // In the order of our own server list, as the official client has them.
                mutual_servers: profile
                    .map(|p| {
                        rail_order
                            .iter()
                            .filter_map(|id| {
                                let g = store.guild(*id)?;
                                let m = p.mutual_guilds.iter().find(|m| m.id == g.id)?;
                                Some(profile::Mutual {
                                    name: g.name.clone(),
                                    picture: g.icon.as_deref().map(|h| cdn::guild_icon(g.id, h, 64)),
                                    detail: m.nick.clone().filter(|n| !n.is_empty()),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                mutual_friends: profile
                    .map(|p| {
                        p.mutual_friend_users
                            .iter()
                            .map(|u| profile::Mutual {
                                name: u.display_name().to_owned(),
                                picture: Some(cdn::user_avatar(u.id, u.avatar.as_deref(), 64)),
                                detail: Some(u.username.clone()),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                mutual_friend_count: profile.map_or(0, |p| p.mutual_friends),
                accounts: profile
                    .map(|p| {
                        p.accounts
                            .iter()
                            .map(|a| profile::Connection {
                                kind: a.kind.clone(),
                                name: a.name.clone(),
                                since: a.metadata.created_at.as_deref().and_then(date),
                                games: a.metadata.game_count.clone().filter(|n| n.parse::<i64>().is_ok_and(|n| n >= 0)),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                board,
                activities,
                custom_status: crate::friends::custom_status(store.activities(user)),
                is_me: store.me().is_some_and(|me| me.id == user),
                actions: Vec::new(),
                note_input: self.note_input.clone(),
                message_input: self.card_message.clone(),
                effect: None,
                frame: profile.and_then(|p| p.frame_art.clone()),
            }
        };
        // With no accent or banner the card takes after the avatar, as the
        // official client does.
        if card.accent == 0 {
            card.accent = self.media.update(cx, |m, _| m.dominant_color(&avatar_url)).unwrap_or(0x232428);
        }
        if !card.is_me {
            card.actions = self.card_actions(user, guild, cx);
        }
        Some(card)
    }

    /// The buttons of someone's card: what to do about the relationship, and
    /// in a server, each moderation action their permissions allow. None of
    /// them can target the server owner.
    fn card_actions(&self, user: Snowflake, guild: Option<Snowflake>, cx: &App) -> Vec<profile::CardAction> {
        let kind = self.session.read(cx).store.relationship(user);
        let mut actions = self.relationship_actions(user, kind);
        let Some(guild) = guild else { return actions };
        let (permissions, is_owner) = {
            let store = &self.session.read(cx).store;
            (store.guild_permissions(guild), store.guild(guild).and_then(|g| g.owner_id) == Some(user))
        };
        if is_owner {
            return actions;
        }
        // Role hierarchy: we may only moderate people whose highest role sits
        // below our own. Administrators are exempt from the check.
        let can_manage = {
            let store = &self.session.read(cx).store;
            if permissions.contains(oxicord_model::Permissions::ADMINISTRATOR) {
                true
            } else {
                let my_position = store
                    .my_roles(guild)
                    .iter()
                    .filter_map(|id| store.guild(guild)?.roles.iter().find(|r| r.id == *id))
                    .map(|r| r.position)
                    .max()
                    .unwrap_or(i32::MIN);
                let their_position = store
                    .member_in_guild(guild, user)
                    .map(|(_, roles)| roles.iter().map(|r| r.position).max().unwrap_or(i32::MIN))
                    .unwrap_or(i32::MIN);
                my_position > their_position
            }
        };
        let session = self.session.downgrade();
        let emit = |label: &'static str, glyph: &'static str, danger: bool, run: profile::Action| profile::CardAction {
            label,
            glyph,
            danger,
            run,
        };
        if permissions.contains(oxicord_model::Permissions::KICK_MEMBERS) && can_manage {
            let session = session.clone();
            actions.push(emit(
                "Kick",
                "log-out",
                true,
                Rc::new(move |_: &mut Window, cx: &mut App| {
                    if let Some(session) = session.upgrade() {
                        session.update(cx, |_, cx| cx.emit(OpenKickMember(guild, user)));
                    }
                }),
            ));
        }
        if permissions.contains(oxicord_model::Permissions::BAN_MEMBERS) && can_manage {
            let session = session.clone();
            actions.push(emit(
                "Ban",
                "ban",
                true,
                Rc::new(move |_: &mut Window, cx: &mut App| {
                    if let Some(session) = session.upgrade() {
                        session.update(cx, |_, cx| cx.emit(OpenBanMember(guild, user)));
                    }
                }),
            ));
        }
        if permissions.contains(oxicord_model::Permissions::MODERATE_MEMBERS) && can_manage {
            let session = session.clone();
            actions.push(emit(
                "Timeout",
                "clock",
                true,
                Rc::new(move |_: &mut Window, cx: &mut App| {
                    if let Some(session) = session.upgrade() {
                        session.update(cx, |_, cx| cx.emit(OpenTimeoutMember(guild, user)));
                    }
                }),
            ));
        }
        // Editing roles needs its own permission and opens a dialog.
        if permissions.contains(oxicord_model::Permissions::MANAGE_ROLES) && can_manage {
            actions.push(emit(
                "Roles",
                "shield",
                false,
                Rc::new(move |_: &mut Window, cx: &mut App| {
                    if let Some(session) = session.upgrade() {
                        session.update(cx, |_, cx| cx.emit(OpenRolesMember(guild, user)));
                    }
                }),
            ));
        }
        actions
    }

    /// Sends what was typed in the small card's message box to that person,
    /// starting the conversation first if there is none.
    fn send_card_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(user) = self.profile.as_ref().map(|p| p.user) else { return };
        let text = self.card_message.read(cx).value().trim().to_owned();
        if text.is_empty() {
            return;
        }
        match self.session.read(cx).store.dm_with(user) {
            Some(channel) => self.backend.send(Command::SendMessage {
                channel,
                guild: None,
                content: text,
                reply_to: None,
                ping_reply: true,
                files: Vec::new(),
                stickers: Vec::new(),
                poll: None,
            }),
            None => {
                self.pending_card_message = Some((user, text));
                self.backend.send(Command::OpenDm(user));
            }
        }
        self.card_message.update(cx, |input, cx| input.set_value("", window, cx));
        self.profile = None;
        cx.notify();
    }

    /// Sends a message that was waiting for `channel` to exist. True when
    /// there was one, so the caller can stay where it is instead of
    /// switching to the conversation.
    fn send_pending_card_message(&mut self, channel: Snowflake, cx: &mut Context<Self>) -> bool {
        let Some((user, _)) = &self.pending_card_message else { return false };
        let is_theirs = self
            .session
            .read(cx)
            .store
            .channel(channel)
            .is_some_and(|c| c.kind == ChannelType::Dm && c.recipients.first().is_some_and(|r| r.id == *user));
        if !is_theirs {
            return false;
        }
        let Some((_, content)) = self.pending_card_message.take() else { return false };
        self.backend.send(Command::SendMessage {
            channel,
            guild: None,
            content,
            reply_to: None,
            ping_reply: true,
            files: Vec::new(),
            stickers: Vec::new(),
            poll: None,
        });
        true
    }

    /// Sends the note typed in the open profile's box to Discord.
    fn save_note(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = &self.profile else { return };
        let note: String = self.note_input.read(cx).value().trim().to_string();
        self.backend.send(Command::SetUserNote { user: profile.user, note });
    }

    /// The menu behind the card's three dots: everything the buttons don't
    /// already offer, plus copying the id.
    fn open_card_menu(&mut self, user: Snowflake, guild: Option<Snowflake>, at: Point<Pixels>, cx: &mut Context<Self>) {
        let actions: Vec<profile::CardAction> = self
            .card_actions(user, guild, cx)
            .into_iter()
            .filter(|a| !matches!(a.label, "Add Friend" | "Accept" | "Unblock"))
            .collect();
        // View Full Profile goes first, as the official client has it.
        let view_full_index = 0usize;
        let mut entries: Vec<crate::popup::Entry> = vec![crate::popup::Entry::item("View Full Profile", "user")];
        let actions_start = if actions.is_empty() {
            // No actions: separator then Copy User ID.
            entries.push(crate::popup::Entry::Separator);
            None
        } else {
            entries.push(crate::popup::Entry::Separator);
            let start = entries.len();
            entries.extend(actions.iter().map(|a| {
                if a.danger {
                    crate::popup::Entry::danger(a.label, a.glyph)
                } else {
                    crate::popup::Entry::item(a.label, a.glyph)
                }
            }));
            entries.push(crate::popup::Entry::Separator);
            Some(start)
        };
        let copy = entries.len();
        entries.push(crate::popup::Entry::item("Copy User ID", "hash"));
        let view = cx.entity().downgrade();
        let pick = Rc::new(move |index: usize, window: &mut Window, cx: &mut App| {
            if index == view_full_index {
                if let Some(view) = view.upgrade() {
                    view.update(cx, |this, cx| this.show_profile(user, guild, None, true, cx));
                }
                return;
            }
            if index == copy {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(user.to_string()));
                return;
            }
            if let Some(start) = actions_start {
                let ai = index.checked_sub(start);
                if let Some(ai) = ai {
                    if ai < actions.len() {
                        (actions[ai].run)(window, cx);
                    }
                }
            }
        });
        self.popup_open = true;
        self.popup.update(cx, |popup, cx| popup.show(at, entries, pick, cx));
        cx.notify();
    }

    /// The effect for a card, made once when the card opens and once more if
    /// it grows into the full profile. Nothing until we know what it looks like.
    fn profile_effect_view(&mut self, user: Snowflake, full: bool, cx: &mut Context<Self>) -> Option<Entity<profile::EffectView>> {
        let art = self.session.read(cx).profiles.get(&user)?.effect_art.clone()?;
        if let Some((who, was_full, view)) = &self.profile_effect
            && *who == user
            && *was_full == full
        {
            return Some(view.clone());
        }
        let media = self.media.clone();
        // The small card is 300 wide, the left side of the full one 400.
        let width = if full { 400. } else { 300. };
        let view = cx.new(|cx| profile::EffectView::new(media, art, width, cx));
        self.profile_effect = Some((user, full, view.clone()));
        Some(view)
    }

    /// The card for the person we are looking at, over everything else.
    fn render_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (user, guild, at, full) = self.profile.as_ref().map(|p| (p.user, p.guild, p.at, p.full))?;
        let mut card = self.build_card(user, guild, cx)?;
        card.effect = self.profile_effect_view(user, full, cx);

        // Fill the note box from the stored note the first time we show a user,
        // so we don't overwrite what the user is typing.
        if self.note_prefilled_for != Some(user) {
            if let Some(note) = self.session.read(cx).profiles.get(&user).map(|p| p.note.clone()) {
                self.note_input.update(cx, |input, cx| input.set_value(note, window, cx));
            }
            self.note_prefilled_for = Some(user);
        }
        if self.card_message_for != Some(user) {
            self.card_message.update(cx, |input, cx| input.set_value("", window, cx));
            self.card_message_for = Some(user);
        }

        let view = cx.entity().downgrade();
        let act = |run: fn(&mut Self, Snowflake, Option<Snowflake>, &mut Window, &mut Context<Self>)| -> profile::Action {
            let view = view.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| run(this, user, guild, window, cx)).ok();
            })
        };
        let handlers = profile::Handlers {
            message: act(|this, user, _, _, cx| this.message_from_card(user, cx)),
            full: act(|this, user, guild, _, cx| this.show_profile(user, guild, None, true, cx)),
            close: act(|this, _, _, _, cx| {
                this.profile = None;
                cx.notify();
            }),
            edit: act(|this, _, _, window, cx| {
                this.profile = None;
                this.edit_profile_open = true;
                this.edit_profile.update(cx, |modal, cx| modal.opened(window, cx));
                cx.notify();
            }),
            status: act(|this, _, _, window, cx| {
                this.profile = None;
                this.status_dialog_open = true;
                this.status_dialog.update(cx, |dialog, cx| dialog.opened(window, cx));
                cx.notify();
            }),
            more: {
                let view = view.clone();
                Rc::new(move |at: Point<Pixels>, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| this.open_card_menu(user, guild, at, cx)).ok();
                })
            },
            tab: {
                let view = view.clone();
                Rc::new(move |tab: profile::Tab, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| {
                        this.profile_tab = tab;
                        if let Some(shown) = &mut this.profile {
                            shown.full = true;
                            shown.at = None;
                        }
                        cx.notify();
                    })
                    .ok();
                })
            },
        };
        let media = self.media.clone();
        Some(if full {
            profile::full(card, &handlers, self.profile_tab, &media, window, cx)
        } else {
            profile::popout(card, &handlers, at, &media, window, cx)
        })
    }

    /// The relationship buttons for someone's card, by how we know them.
    fn relationship_actions(
        &self,
        user: Snowflake,
        kind: Option<RelationshipKind>,
    ) -> Vec<profile::CardAction> {
        let send = |command: fn(Snowflake) -> Command| -> profile::Action {
            let backend = self.backend.clone();
            Rc::new(move |_: &mut Window, _: &mut App| backend.send(command(user)))
        };
        let add = |label| profile::CardAction {
            label,
            glyph: if label == "Accept" { "user-check" } else { "user-plus" },
            danger: false,
            run: send(Command::AcceptFriend),
        };
        let remove = |label, glyph| profile::CardAction { label, glyph, danger: true, run: send(Command::RemoveRelationship) };
        let block = profile::CardAction { label: "Block", glyph: "ban", danger: true, run: send(Command::BlockUser) };

        match kind {
            Some(RelationshipKind::Friend) => vec![remove("Remove Friend", "user-minus"), block],
            Some(RelationshipKind::IncomingRequest) => vec![add("Accept"), remove("Ignore", "user-x"), block],
            Some(RelationshipKind::OutgoingRequest) => vec![remove("Cancel Request", "user-x"), block],
            Some(RelationshipKind::Blocked) => vec![profile::CardAction {
                label: "Unblock",
                glyph: "ban",
                danger: false,
                run: send(Command::RemoveRelationship),
            }],
            _ => vec![add("Add Friend"), block],
        }
    }

    /// Opens the menu for a person and carries out whatever is picked.
    fn open_user_menu(&mut self, open: &OpenUserMenu, cx: &mut Context<Self>) {
        let in_our_call = {
            let session = self.session.read(cx);
            session
                .voice
                .target
                .is_some_and(|(_, channel)| session.store.voice_in(channel).iter().any(|v| v.user_id == open.user))
        };
        self.show_user_menu(open, in_our_call, cx);
    }

    fn show_user_menu(&mut self, open: &OpenUserMenu, in_our_call: bool, cx: &mut Context<Self>) {
        let (tie, is_me, saved_volume) = {
            let session = self.session.read(cx);
            let store = &session.store;
            (
                store.relationship(open.user),
                store.me().map(|u| u.id) == Some(open.user),
                session.settings.user_volume(open.user.0),
            )
        };
        let list = user_menu::choices(tie, is_me, open.can_mention, open.dm.is_some(), in_our_call);
        let volume = list.contains(&user_menu::Choice::Volume).then(|| {
            let state = cx.new(|_| {
                SliderState::new().min(0.).max(f32::from(oxicord_voice::MAX_USER_VOLUME)).step(1.).default_value(f32::from(saved_volume))
            });
            let user = open.user;
            self.volume_sub = Some(cx.subscribe(&state, move |this, _, event: &SliderEvent, cx| {
                let (value, save) = match event {
                    SliderEvent::Change(value) => (value, false),
                    SliderEvent::Release(value) => (value, true),
                };
                let percent = value.start().round().clamp(0., f32::from(oxicord_voice::MAX_USER_VOLUME)) as u16;
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| s.set_user_volume(&backend, user, percent, save, cx));
            }));
            state
        });
        let entries = list
            .iter()
            .map(|c| match (c, &volume) {
                (user_menu::Choice::Volume, Some(state)) => {
                    crate::popup::Entry::Slider { label: "User Volume".into(), state: state.clone() }
                }
                _ => c.entry(),
            })
            .collect();
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
            Choice::Profile => self.show_profile(user, guild, None, true, cx),
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
            Choice::ChangeFriendNickname => {
                self.friend_nick_open = true;
                self.friend_nick.update(cx, |dialog, cx| dialog.opened(user, window, cx));
                cx.notify();
            }
            Choice::CopyId => cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(user.to_string())),
            Choice::Volume | Choice::Separator => {}
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
        self.requests_open = false;
        self.sync_chat(window, cx);
        cx.notify();
    }

    fn show_requests(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.channel = None;
        self.requests_open = true;
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
        if self.session.update(cx, |s, _| s.should_fetch_profile(user, None)) {
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
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| this.open_profile(user, None, Some(event.position()), cx)))
                    .child(div().relative().flex_none().child(avatar(&self.media, &url, 32., &name, cx)).child(presence_dot(status)))
                    .child(div().flex_1().min_w_0().truncate().child(name)),
            );
        }
        Some(column.into_any_element())
    }

    /// The server's welcome screen: its icon and name, the description and
    /// the featured channels. Shown when a server is selected but no channel
    /// is, which is the state the official client reserves for newcomers.
    fn render_welcome(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild_id) = self.guild else {
            return div().into_any_element();
        };
        let (name, icon_url) = {
            let store = &self.session.read(cx).store;
            match store.guild(guild_id) {
                Some(guild) => (
                    guild.name.clone(),
                    guild.icon.as_deref().map(|hash| cdn::guild_icon(guild_id, hash, 96)),
                ),
                None => (String::new(), None),
            }
        };
        let screen = self.welcome.clone();
        let mut column = div()
            .w(px(480.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .child(match icon_url {
                Some(url) => avatar(&self.media, &url, 64., &name, cx).into_any_element(),
                None => div()
                    .w(px(64.))
                    .h(px(64.))
                    .flex_none()
                    .rounded_full()
                    .bg(rgb(theme::BG_ACTIVE))
                    .into_any_element(),
            })
            .child(
                div()
                    .mt(px(8.))
                    .text_size(px(24.))
                    .line_height(px(30.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(format!("Welcome to {name}!")),
            );
        if let Some(description) = screen
            .as_ref()
            .and_then(|screen| screen.description.as_deref())
            .filter(|description| !description.is_empty())
        {
            column = column.child(
                div()
                    .w(px(440.))
                    .mt(px(4.))
                    .text_size(px(16.))
                    .line_height(px(22.))
                    .text_center()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(description.to_owned()),
            );
        }
        let channels = screen.map(|screen| screen.welcome_channels).unwrap_or_default();
        if channels.is_empty() {
            let note = if self.welcome_failed {
                "This server has no welcome screen."
            } else if self.welcome.is_none() {
                "Loading the welcome screen..."
            } else {
                "Pick a channel to get started."
            };
            column = column.child(
                div()
                    .mt(px(32.))
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(note),
            );
        } else {
            let mut list = div()
                .w_full()
                .mt(px(32.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .mb(px(4.))
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(rgb(theme::TEXT_FAINT))
                        .child("FEATURED CHANNELS"),
                );
            for channel in channels {
                let id = channel.channel_id;
                // The official list only names channels we can see, so skip
                // any the store does not know about.
                let Some(channel_name) = self
                    .session
                    .read(cx)
                    .store
                    .channel(id)
                    .and_then(|c| c.name.clone())
                else {
                    continue;
                };
                let glyph = channel
                    .emoji_name
                    .clone()
                    .filter(|emoji| !emoji.is_ascii());
                let card = div()
                    .id(("welcome-channel", id.0))
                    .w_full()
                    .min_h(px(56.))
                    .px(px(12.))
                    .py(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(theme::BG_CARD))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.select_channel(id, window, cx);
                    }))
                    .child(match glyph {
                        Some(emoji) => div()
                            .w(px(20.))
                            .flex_none()
                            .text_size(px(18.))
                            .line_height(px(20.))
                            .text_center()
                            .child(emoji)
                            .into_any_element(),
                        None => div()
                            .w(px(20.))
                            .flex_none()
                            .child(icon("hash", 20., theme::CHANNELS_DEFAULT))
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .line_height(px(20.))
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(channel_name),
                            )
                            .children(
                                (!channel.description.is_empty()).then(|| {
                                    div()
                                        .text_size(px(14.))
                                        .line_height(px(18.))
                                        .text_color(rgb(theme::TEXT_MUTED))
                                        .child(channel.description)
                                }),
                            ),
                    );
                list = list.child(card);
            }
            column = column.child(list);
        }
        div()
            .id("welcome-screen")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(96.))
            .pb(px(32.))
            .bg(rgb(theme::BG_CHAT))
            .child(column)
            .into_any_element()
    }

    /// The answer to [`Command::LoadOnboarding`], passed to the guide.
    pub(crate) fn apply_onboarding(
        &mut self,
        guild: Snowflake,
        result: Result<oxicord_model::Onboarding, String>,
        cx: &mut Context<Self>,
    ) {
        self.onboarding
            .update(cx, |dialog, cx| dialog.apply(guild, result, cx));
    }

    /// The answer to [`Command::SaveOnboarding`], passed to the guide.
    pub(crate) fn apply_onboarding_saved(
        &mut self,
        guild: Snowflake,
        result: Result<oxicord_model::OnboardingUpdate, String>,
        cx: &mut Context<Self>,
    ) {
        self.onboarding
            .update(cx, |dialog, cx| dialog.saved(guild, result, cx));
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
                    tag: match (c.kind, c.recipients.first()) {
                        (ChannelType::Dm, Some(user)) => store.user(user.id).unwrap_or(user).server_tag(),
                        _ => None,
                    },
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

        let space = self.user_card_space(cx);
        div()
            .w(px(theme::RAIL_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .pb(px(space))
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
            .child(
                div()
                    .id("create-server")
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
                        this.session.update(cx, |s, cx| cx.emit(OpenCreateServer));
                        this.create_server_open = true;
                        this.create_server_dialog.update(cx, |d, cx| d.opened(window, cx));
                        cx.notify();
                    }))
                    .child(icon("plus", 22., theme::ACCENT)),
            )
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let head: Option<GuildHead> = self.guild.and_then(|guild| {
            let session = self.session.read(cx);
            let g = session.store.guild(guild)?;
            Some(GuildHead {
                name: g.name.clone().into(),
                banner: g.banner.as_deref().map(|hash| cdn::guild_banner(guild, hash, 640)),
                tier: g.premium_tier,
                boosts: g.premium_subscription_count,
                goal: g.premium_progress_bar_enabled,
            })
        });
        let title: SharedString = head.as_ref().map_or_else(|| "Direct Messages".into(), |h| h.name.clone());
        let banner_url = head.as_ref().and_then(|h| h.banner.clone());
        let has_banner = banner_url.is_some();
        let header_menu_open = self.menu_open && self.menu.read(cx).is_header();
        // How far the list has scrolled up. The header goes solid once the
        // banner has slid out from under it.
        let scrolled = -f32::from(self.sidebar_scroll.offset().y);
        let scrolled = scrolled.clamp(0., f32::from(self.sidebar_scroll.max_offset().y).max(0.));
        let header_solid = !has_banner || scrolled >= BANNER_HEIGHT - theme::CHANNEL_HEADER_HEIGHT;

        let mut list = div().flex().flex_col().gap(px(2.)).p_2();
        if has_banner {
            // Live starts the channels 3px under the banner.
            list = list.pt(px(3.));
        }
        match self.guild {
            Some(guild) => {
                if let Some(head) = head.as_ref().filter(|h| h.goal) {
                    list = list.child(boost_goal(head.tier, head.boosts, has_banner));
                }
                if let Some(row) = self.render_events_entry(guild, cx) {
                    list = list.child(row);
                }
                for group in self.group_items(guild, cx) {
                    list = list.child(self.render_group(group, cx));
                }
            }
            None => {
                list = list.child(self.render_friends_entry(cx));
                if let Some(entry) = self.render_requests_entry(cx) {
                    list = list.child(entry);
                }
                // A line between the shortcuts and the conversations.
                list = list.child(div().mx(px(8.)).my(px(10.)).h(px(1.)).bg(rgb(theme::BORDER)));
                list = list.child(self.render_dm_heading(cx));
                for dm in self.dm_items(cx) {
                    list = list.child(self.render_dm(dm, cx));
                }
            }
        }

        // A banner runs behind the header, so the header floats over the top
        // of the list instead of taking a row of its own.
        let header = div()
                    .id("sidebar-header")
                    .h(px(theme::CHANNEL_HEADER_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_color(rgb(if has_banner { 0xffffff } else { theme::TEXT_BRIGHT }))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .when(has_banner, |header| {
                        header.absolute().top_0().left_0().w_full().when(header_solid, |h| h.bg(rgb(theme::BG_SIDEBAR)))
                    })
                    .when(!has_banner, |header| header.border_b_1().border_color(rgb(theme::BORDER)))
                    .when_some(self.guild, |header, guild| {
                        header
                            .cursor_pointer()
                            .when(!has_banner, |h| h.hover(|s| s.bg(rgb(theme::BG_HOVER))))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.session.update(cx, |session, _| session.request_guild_tag(&this.backend, guild));
                                this.open_menu(Target::GuildHeader(guild), point(px(theme::RAIL_WIDTH + 42.), px(85.)), cx);
                            }))
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
                    .when(self.guild.is_some(), |header| {
                        let tier = head.as_ref().map_or(0, |h| h.tier);
                        header.pl_4().pr_2().child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .when(tier > 0, |row| row.child(boost_badge()))
                                .child(div().min_w_0().truncate().text_size(px(16.)).child(title))
                                .child(
                                    icon(
                                        if header_menu_open { "chevron-up" } else { "chevron-down" },
                                        16.,
                                        if has_banner { 0xffffff } else { theme::TEXT_MUTED },
                                    )
                                    .ml(px(-6.))
                                    .flex_none(),
                                ),
                        )
                    })
                    .children(self.channel.filter(|_| self.guild.is_some()).map(|channel| {
                        div()
                            .id("invite-server")
                            .size(px(32.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.session.update(cx, |_, cx| cx.emit(OpenInviteDialog(channel)));
                            }))
                            .child(icon("user-plus", 16., if has_banner { 0xffffff } else { theme::TEXT_MUTED }))
                    }));

        let mut header = Some(header);
        let banner = banner_url.map(|url| {
            let picture = self.media.update(cx, |media, _| media.get(&url));
            div()
                .relative()
                .w_full()
                .h(px(BANNER_HEIGHT))
                .flex_none()
                .overflow_hidden()
                .children(picture.map(|image| {
                    img(ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover)
                }))
                // Darkens the top so the name reads on any picture.
                .child(
                    div().absolute().top_0().left_0().w_full().h(px(theme::CHANNEL_HEADER_HEIGHT + 16.)).bg(
                        gpui_kit::linear_gradient(
                            180.,
                            gpui_kit::linear_color_stop(rgba(0x00000085), 0.),
                            gpui_kit::linear_color_stop(rgba(0x00000000), 1.),
                        ),
                    ),
                )
        });

        div()
            .relative()
            .w(px(theme::SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_SIDEBAR))
            .when(!has_banner, |column| column.children(header.take()))
            .child(
                div()
                    .id("channel-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.sidebar_scroll)
                    .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                    .children(banner)
                    .child(list),
            )
            .when(has_banner, |column| column.children(header.take()))
            // Room for the user card, which floats over the bottom of the rail and the list.
            .child(div().h(px(self.user_card_space(cx))).flex_none())
    }

    /// The Events row above a server's channels, with the line under it that
    /// sets it apart from the first category. It reads "N Events" while some
    /// are running or still to come, and plain "Events" otherwise.
    fn render_events_entry(&mut self, guild: Snowflake, cx: &mut Context<Self>) -> Option<AnyElement> {
        let store = &self.session.read(cx).store;
        let count = store
            .guild(guild)?
            .scheduled_events
            .iter()
            .filter(|event| matches!(event.status(), EventStatus::Active | EventStatus::Scheduled))
            .count();
        if !shows_events_row(count, store.guild_permissions(guild)) {
            return None;
        }
        let row = div()
            .id("events-entry")
            .h(px(32.))
            .flex()
            .items_center()
            .gap_2()
            .px(px(8.))
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(rgb(theme::CHANNELS_DEFAULT))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.session.update(cx, |_, cx| cx.emit(OpenEvents(guild)));
            }))
            .child(icon("calendar", 20., theme::CHANNELS_DEFAULT))
            .child(events_label(count));
        // Live: the line sits 12px under the row, 16px in from the left edge
        // and 7px from the right, and the first category follows it at once.
        // The list puts a 2px gap between children, which live does not have
        // here, hence the pull back on both sides.
        Some(
            div()
                .mt(px(-1.))
                .mb(px(-1.))
                .child(row)
                .child(
                    div()
                        .mt(px(12.))
                        .ml(px(8.))
                        .mr(px(7.))
                        .h(px(1.))
                        .bg(rgba(0x96979e33)),
                )
                .into_any_element(),
        )
    }

    /// Height the user card takes at the bottom of the rail and the list,
    /// margins included: the row, and the call block above it in a call.
    fn user_card_space(&self, cx: &App) -> f32 {
        let in_call = self.session.read(cx).voice.target.is_some();
        58. + if in_call { 100. } else { 0. } + 16.
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
            let guild = self.guild;
            column = column.child(
                div()
                    .id(("category", id.0))
                    .h(px(40.))
                    // Live stacks the first channel right under the header.
                    .mb(px(-2.))
                    .pt(px(16.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_category(id, cx)))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(rgb(theme::CHANNELS_DEFAULT))
                            .hover(|s| s.text_color(rgb(theme::TEXT_BRIGHT)))
                            .child(div().truncate().child(name))
                            .child(icon(chevron, 12., theme::CHANNELS_DEFAULT)),
                    )
                    // Discord shows a "Create Channel" button on the right of the
                    // category header. It opens a dialog scoped to this category.
                    .when_some(guild, |header, guild| {
                        header.child(
                            div()
                                .id(("category-create", id.0))
                                .size(px(18.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .text_color(rgb(theme::TEXT_MUTED))
                                .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.session.update(cx, |_, cx| cx.emit(OpenCreateChannel(guild, Some(id))));
                                }))
                                .child(icon("plus", 16., theme::TEXT_MUTED)),
                        )
                    }),
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
                // A green ring around the avatar of whoever is talking.
                let speaking = self.session.read(cx).voice.speaking.contains(&m.user);
                let ring = if speaking { theme::STATUS_POSITIVE } else { theme::BG_SIDEBAR };
                let mut row = div()
                    .id(("voice-member", m.user.0))
                    .h(px(32.))
                    .ml(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(rgb(theme::CHANNELS_DEFAULT))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT)))
                    .child(
                        div()
                            .flex_none()
                            .rounded(px(12.))
                            .border_2()
                            .border_color(rgb(ring))
                            .child(avatar(&self.media, &m.avatar, 20., &m.name, cx)),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(m.name.clone()));
                let user = m.user;
                row = row.on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    this.open_profile(user, this.guild, Some(event.position()), cx)
                }));
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
        // The voice channel we sit in is drawn green, with its status prompt.
        let in_call = self.session.read(cx).voice.target.is_some_and(|(_, call)| call == id);
        let color = if in_call {
            theme::STATUS_POSITIVE
        } else if selected || channel.unread.unread {
            theme::TEXT_BRIGHT
        } else if channel.muted {
            theme::CHANNEL_MUTED
        } else {
            theme::CHANNELS_DEFAULT
        };

        let indent = if channel.kind.is_thread() { 18. } else { 0. };
        let joins_voice = is_voice(channel.kind);
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
            .text_color(rgb(if in_call { theme::TEXT_BRIGHT } else { color }))
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| {
                let s = s.bg(rgb(theme::BG_HOVER));
                if channel.muted { s } else { s.text_color(rgb(theme::TEXT_BRIGHT)) }
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_channel(id, window, cx);
                if joins_voice {
                    this.join_voice(id, cx);
                }
            }))
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
            .children(in_call.then(|| {
                let elapsed = self.session.read(cx).voice.since.map(|at| at.elapsed());
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(theme::STATUS_POSITIVE))
                    .child(call_clock(elapsed.unwrap_or_default()))
            }))
            .children(channel.user_limit.map(|max| {
                let n = channel.voice.len();
                div()
                    .px(px(4.))
                    .rounded(px(3.))
                    .bg(rgb(theme::BADGE))
                    .text_size(px(10.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(format!("{n}/{max}"))
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
            .children((!channel.kind.is_dm()).then(|| {
                div()
                    .id(("edit-channel", id.0))
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
                        this.session.update(cx, |_, cx| cx.emit(OpenChannelSettings(id)));
                    }))
                    .child(icon("pencil", 15., theme::TEXT_MUTED))
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

        let set_status = (in_call && is_voice(channel.kind)).then(|| {
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
        let selected = self.channel.is_none() && !self.requests_open;
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

    /// The row that opens the message requests page. The official client
    /// draws it only while there is something to show, and its badge counts
    /// the plain requests, not the spam ones.
    fn render_requests_entry(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let store = &self.session.read(cx).store;
        let count = store.message_requests().len();
        let spam = store.spam_channels().len();
        (count > 0 || spam > 0).then(|| {
            let selected = self.requests_open && self.channel.is_none();
            let color = if selected { theme::TEXT } else { theme::TEXT_FAINT };
            div()
                .id("message-requests")
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
                .on_click(cx.listener(|this, _, window, cx| this.show_requests(window, cx)))
                .child(div().w(px(20.)).flex_none().flex().justify_center().child(icon("message-requests", 20., color)))
                .child(div().flex_1().child("Message Requests"))
                .children(mention_badge(u32::try_from(count).unwrap_or(u32::MAX)))
                .into_any_element()
        })
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(div().truncate().child(dm.name))
                            .children(dm.tag.map(|t| crate::member_list::server_tag_chip(&t, 14., &self.media, cx))),
                    )
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
                                                this.edit_profile_open = true;
                                                cx.notify();
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

        // Deafening shows the microphone as off too, as the official panel does.
        let (mic_off, deafened) = {
            let call = &self.session.read(cx).voice;
            (call.muted || call.deafened, call.deafened)
        };

        let call_panel = self.render_call_panel(cx);

        // A card floating 8px in from the edges, as the official panels are.
        // While in a call the connection panel sits on top, inside the card.
        div().flex_none().child(
        div()
            .flex()
            .flex_col()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_SIDEBAR))
            .children(call_panel)
            .child(
        div()
            .h(px(56.))
            .flex()
            .items_center()
            .gap(px(8.))
            .p(px(8.))
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
            // Each of mute and deafen is a split button: the switch and, beside
            // it, an arrow that opens the device list.
            .child(
                div()
                    .flex()
                    .flex_none()
                    .child(
                        div()
                            .id("mute")
                            .size(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_l(px(theme::RADIUS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let backend = this.backend.clone();
                                this.session.update(cx, |session, cx| session.toggle_voice_mute(&backend, cx));
                            }))
                            .child(icon(
                                if mic_off { "mic-off" } else { "mic" },
                                20.,
                                if mic_off { theme::DANGER } else { theme::TEXT_MUTED },
                            )),
                    )
                    .child(
                        device_arrow("input-options")
                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| this.open_device_menu(true, event, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .child(
                        div()
                            .id("deafen")
                            .size(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_l(px(theme::RADIUS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let backend = this.backend.clone();
                                this.session.update(cx, |session, cx| session.toggle_voice_deafen(&backend, cx));
                            }))
                            .child(icon(
                                if deafened { "headphone-off" } else { "headphones" },
                                20.,
                                if deafened { theme::DANGER } else { theme::TEXT_MUTED },
                            )),
                    )
                    .child(
                        device_arrow("output-options")
                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| this.open_device_menu(false, event, cx))),
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
            ),
            ),
        )
    }

    /// Opens the list of microphones (`input`) or speakers under their arrow.
    fn open_device_menu(&mut self, input: bool, event: &ClickEvent, cx: &mut Context<Self>) {
        use crate::popup::Entry;
        let devices = if input { oxicord_voice::input_devices() } else { oxicord_voice::output_devices() };
        let chosen = {
            let session = self.session.read(cx);
            if input { session.audio_input.clone() } else { session.audio_output.clone() }
        };
        let kind = if input { "mic" } else { "headphones" };
        let mark = |on: bool| if on { "check" } else { kind };
        let mut entries = vec![Entry::item("System Default", mark(chosen.is_none()))];
        entries.extend(devices.iter().map(|name| Entry::item(name.clone(), mark(chosen.as_deref() == Some(name)))));

        let (session, backend) = (self.session.clone(), self.backend.clone());
        let pick: crate::popup::Pick = std::rc::Rc::new(move |index, _, cx| {
            let device = index.checked_sub(1).and_then(|ix| devices.get(ix)).cloned();
            session.update(cx, |session, cx| {
                if input {
                    session.set_audio_input(&backend, device, cx);
                } else {
                    session.set_audio_output(&backend, device, cx);
                }
            });
        });
        let at = event.position();
        self.session.update(cx, |_, cx| cx.emit(OpenPopup { at, entries, pick }));
    }

    /// Whether the open channel is the voice channel we are in. Then the
    /// main area shows the call, as the official client does, not a chat.
    fn in_call_view(&self, cx: &App) -> bool {
        let Some(channel) = self.channel else { return false };
        let session = self.session.read(cx);
        session.voice.target.is_some_and(|(guild, call)| guild.is_some() && call == channel)
            && session.store.channel(channel).is_some_and(|c| is_voice(c.kind))
    }

    /// The call: a tile per person, and the controls along the bottom.
    fn render_call_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        const GAP: f32 = 8.;
        let Some(channel) = self.channel else { return div().into_any_element() };
        let (name, people, call) = {
            let session = self.session.read(cx);
            let store = &session.store;
            let item = store.channel(channel).map(|c| channel_item(store, c));
            match item {
                Some(item) => (item.name, item.voice, session.voice.clone()),
                None => (String::new(), Vec::new(), session.voice.clone()),
            }
        };
        if self.popout.is_some() {
            return self.render_call_popped_out(name, cx);
        }

        // The biggest 16:9 tiles that fit the area, whatever the count.
        let viewport = window.viewport_size();
        let chat_w = if self.call_chat_open { CALL_CHAT_WIDTH } else { 0. };
        let area_w = f32::from(viewport.width) - theme::RAIL_WIDTH - theme::SIDEBAR_WIDTH - chat_w - 2. * GAP;
        let area_h = f32::from(viewport.height) - title_bar::HEIGHT - theme::CHANNEL_HEADER_HEIGHT - 50. - 48.;
        let (tile_w, tile_h) = call_tile_size(people.len(), area_w, area_h, GAP);

        let tiles: Vec<AnyElement> = people
            .iter()
            .map(|person| {
                let user = person.user;
                call_tile(&self.media, person, call.speaking.contains(&user), (tile_w, tile_h), cx)
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let guild = this.guild;
                        this.session.update(cx, |_, cx| {
                            cx.emit(OpenUserMenu { user, guild, at: event.position, dm: None, can_mention: false });
                        });
                    }))
                    .into_any_element()
            })
            .collect();

        // A control the official bar has but we cannot do yet is drawn all
        // the same, so the bar looks right.
        let button = |id: &'static str, glyph: &'static str, color: u32| {
            div()
                .id(id)
                .size(px(40.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .child(icon(glyph, 20., color))
        };
        let backend = self.backend.clone();
        let (mute_backend, leave_backend) = (backend.clone(), backend);
        let (mute_session, leave_session) = (self.session.clone(), self.session.clone());
        let mic_off = call.muted || call.deafened;
        let cluster = || div().flex().gap(px(GAP)).p(px(4.)).rounded(px(12.)).bg(rgb(theme::BG_PANEL));
        let caret = |id: &'static str| {
            div()
                .id(id)
                .w(px(21.))
                .h(px(40.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_r(px(theme::RADIUS))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                .child(icon("chevron-down", 16., theme::TEXT_BRIGHT))
        };

        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_CHAT))
            .child(self.call_header(name, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_center()
                    .content_center()
                    .gap(px(GAP))
                    .p(px(GAP))
                    .children(tiles),
            )
            .child(
                div()
                    .flex_none()
                    .h(px(50. + 16.))
                    .pb(px(16.))
                    .flex()
                    .items_start()
                    .justify_center()
                    .gap(px(12.))
                    .child(
                        cluster()
                            // Mute and camera each carry an arrow, as in the official bar.
                            .child(
                                div()
                                    .flex()
                                    .child(
                                        button("call-bar-mute", if mic_off { "mic-off" } else { "mic" }, if mic_off { theme::DANGER } else { theme::TEXT_BRIGHT })
                                            .rounded_r(px(0.))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                            .on_click(move |_, _, cx| {
                                                mute_session.update(cx, |s, cx| s.toggle_voice_mute(&mute_backend, cx));
                                            }),
                                    )
                                    .child(
                                        caret("call-input-options")
                                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| this.open_device_menu(true, event, cx))),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .child(button("call-bar-camera", "camera-off", theme::TEXT_BRIGHT).rounded_r(px(0.)))
                                    .child(caret("call-camera-options")),
                            ),
                    )
                    .child(
                        cluster()
                            .child(button("call-bar-share", "screen-share", theme::TEXT_BRIGHT))
                            .child(button("call-bar-activity", "activity", theme::TEXT_BRIGHT))
                            .child(button("call-bar-soundboard", "soundboard", theme::TEXT_BRIGHT))
                            .child(button("call-bar-more", "ellipsis", theme::TEXT_BRIGHT)),
                    )
                    .child(
                        div()
                            .id("call-bar-hangup")
                            .w(px(66.))
                            .h(px(50.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(12.))
                            .bg(rgb(theme::BADGE))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                leave_session.update(cx, |s, cx| s.leave_voice(&leave_backend, cx));
                            })
                            .child(icon("voice-hangup", 20., 0xffffff)),
                    ),
            )
            // Pop out and full screen, at the right end of the bar, when
            // there is room for them beside the centered controls.
            .children((area_w >= 940.).then(|| {
                div()
                    .absolute()
                    .right(px(16.))
                    .bottom(px(29.))
                    .flex()
                    .items_center()
                    .gap(px(20.))
                    .child(
                        div()
                            .id("call-pop-out")
                            .size(px(24.))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.pop_out_call(cx)))
                            .child(icon("external-link", 24., theme::TEXT_BRIGHT)),
                    )
                    .child(
                        div()
                            .id("call-fullscreen")
                            .size(px(24.))
                            .cursor_pointer()
                            .on_click(|_, window, _| window.toggle_fullscreen())
                            .child(icon("maximize", 24., theme::TEXT_BRIGHT)),
                    )
            }))
            .into_any_element()
    }

    /// Moves the call into a window of its own, or brings that window forward.
    fn pop_out_call(&mut self, cx: &mut Context<Self>) {
        if let Some(window) = self.popout {
            if window.update(cx, |_, window, _| window.activate_window()).is_ok() {
                return;
            }
            self.popout = None;
        }
        let (session, media, backend) = (self.session.clone(), self.media.clone(), self.backend.clone());
        let Some((handle, view)) = call_window::open(session, media, backend, cx) else { return };
        self.popout = Some(handle);
        // The view goes with its window, which is when the call comes back here.
        cx.observe_release(&view, |this, _, cx| {
            this.popout = None;
            cx.notify();
        })
        .detach();
        cx.notify();
    }

    /// What the main window shows of a call that lives in its own window.
    fn render_call_popped_out(&mut self, name: String, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_CHAT))
            .child(self.call_header(name, cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(16.))
                    .child(icon("external-link", 48., theme::TEXT_MUTED))
                    .child(
                        div()
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Your call is in another window"),
                    )
                    .child(
                        div()
                            .id("call-bring-back")
                            .h(px(40.))
                            .px(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .bg(rgb(theme::ACCENT))
                            .text_size(px(16.))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(window) = this.popout.take() {
                                    let _ = window.update(cx, |_, window, _| window.remove_window());
                                }
                                cx.notify();
                            }))
                            .child("Bring it back"),
                    ),
            )
            .into_any_element()
    }

    /// The bar above a call: the channel and the button for its text chat.
    fn call_header(&mut self, name: String, cx: &mut Context<Self>) -> AnyElement {
        let open = self.call_chat_open;
        div()
            .h(px(theme::CHANNEL_HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(16.))
            .child(icon("volume-2", 24., theme::TEXT_MUTED))
            .child(
                div()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(name),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("call-chat-toggle")
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.call_chat_open = !this.call_chat_open;
                        cx.notify();
                    }))
                    .child(icon("message-circle", 20., if open { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED })),
            )
            .into_any_element()
    }

    /// The voice channel on screen, if the open channel is one.
    fn voice_view_channel(&self, cx: &App) -> Option<Snowflake> {
        let channel = self.channel?;
        self.guild?;
        self.session
            .read(cx)
            .store
            .channel(channel)
            .is_some_and(|c| is_voice(c.kind))
            .then_some(channel)
    }

    /// A voice channel's screen: the call when we are in it, else the page
    /// that invites us in, with its chat beside it when asked for.
    fn render_voice_area(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let main = if self.in_call_view(cx) {
            self.render_call_view(window, cx)
        } else {
            self.render_voice_splash(cx)
        };
        if !self.call_chat_open {
            return main;
        }
        div()
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(main)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .w(px(CALL_CHAT_WIDTH))
                    .h_full()
                    .border_l_1()
                    .border_color(rgb(theme::BORDER))
                    .child(self.chat.clone()),
            )
            .into_any_element()
    }

    /// What a voice channel shows before we join: who is in it and a button.
    fn render_voice_splash(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(channel) = self.channel else { return div().into_any_element() };
        let (name, people) = {
            let store = &self.session.read(cx).store;
            match store.channel(channel).map(|c| channel_item(store, c)) {
                Some(item) => (item.name, item.voice),
                None => (String::new(), Vec::new()),
            }
        };
        let names: Vec<&str> = people.iter().map(|p| p.name.as_str()).collect();
        let subtitle = splash_subtitle(&names);

        let tiles: Vec<AnyElement> = people
            .iter()
            .map(|person| {
                let color = self
                    .media
                    .update(cx, |media, _| media.dominant_color(&person.avatar))
                    .unwrap_or(theme::CALL_TILE);
                div()
                    .id(("splash-tile", person.user.0))
                    .w(px(160.))
                    .h(px(90.))
                    .flex_none()
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(color))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(avatar(&self.media, &person.avatar, 40., &person.name, cx))
                    .into_any_element()
            })
            .collect();

        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(gpui_kit::linear_gradient(
                180.,
                gpui_kit::linear_color_stop(rgb(0x111221), 0.),
                gpui_kit::linear_color_stop(rgb(0x25276b), 1.),
            ))
            // The soft glow on the bottom edge, as a stack of narrowing
            // arches that each add a little light.
            .children((0..22).map(|step| {
                let share = 1.0 - step as f32 * 0.04;
                div()
                    .absolute()
                    .bottom_0()
                    .left(gpui_kit::relative((1.0 - share) / 2.))
                    .w(gpui_kit::relative(share))
                    .h(px(360. - step as f32 * 12.))
                    .rounded_t(px(1200.))
                    .bg(gpui_kit::linear_gradient(
                        180.,
                        gpui_kit::linear_color_stop(rgba(0x6470d800), 0.),
                        gpui_kit::linear_color_stop(rgba(0x6470d810), 1.),
                    ))
            }))
            .child(self.call_header(name.clone(), cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(div().flex().gap(px(8.)).children(tiles))
                    .child(
                        div()
                            .mt(px(32.))
                            .text_size(px(32.))
                            .line_height(px(40.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(name),
                    )
                    .child(
                        div()
                            .mt(px(8.))
                            .text_size(px(14.))
                            .line_height(px(18.))
                            .text_color(rgb(theme::TEXT))
                            .child(subtitle),
                    )
                    .child(
                        div()
                            .id("join-voice")
                            .mt(px(24.))
                            .h(px(40.))
                            .px(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .bg(rgb(0xffffff))
                            .text_size(px(16.))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(rgb(0x000000))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| this.join_voice(channel, cx)))
                            .child("Join Voice"),
                    ),
            )
            .into_any_element()
    }

    /// The "Voice Connected" block above the user row: where we are, the
    /// hang up button and the four call actions (camera, share, activity,
    /// soundboard), which are not supported yet and so only show.
    fn render_call_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (call, place) = {
            let session = self.session.read(cx);
            let (guild, channel) = session.voice.target?;
            let store = &session.store;
            let channel_name = store.channel(channel).map_or_else(String::new, |c| match &c.name {
                Some(name) => name.clone(),
                None => store.dm_name(c),
            });
            let place = match guild.and_then(|g| store.guild(g)) {
                Some(guild) => format!("{channel_name} / {}", guild.name),
                None => channel_name,
            };
            (session.voice.clone(), place)
        };
        let backend = self.backend.clone();
        let session = self.session.clone();

        let (label, label_color) = if call.connected {
            ("Voice Connected", theme::STATUS_POSITIVE)
        } else {
            ("Voice Connecting", theme::TEXT_MUTED)
        };
        let action = |id: &'static str, glyph: &'static str| {
            div()
                .id(id)
                .flex_1()
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER_MUTED))
                .bg(rgb(theme::BG_HOVER))
                .child(icon(glyph, 20., theme::TEXT_BRIGHT))
        };

        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .size(px(32.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon("voice-signal", 20., label_color)),
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
                                        .text_size(px(16.))
                                        .line_height(px(20.))
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(rgb(label_color))
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(12.))
                                        .line_height(px(16.))
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(rgb(theme::TEXT_MUTED))
                                        .child(place),
                                ),
                        )
                        .child(
                            div()
                                .id("disconnect-call")
                                .size(px(32.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(theme::RADIUS))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                .on_click(move |_, _, cx| {
                                    session.update(cx, |session, cx| session.leave_voice(&backend, cx));
                                })
                                .child(icon("voice-hangup", 20., theme::TEXT_MUTED)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(action("call-camera", "camera-off"))
                        .child(action("call-share", "screen-share"))
                        .child(action("call-activity", "activity"))
                        .child(action("call-soundboard", "soundboard")),
                )
                .into_any_element(),
        )
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rail = self.render_rail(cx);
        let sidebar = self.render_sidebar(cx);
        let in_call = self.voice_view_channel(cx).is_some();
        let content = if in_call {
            self.render_voice_area(window, cx)
        } else if self.guild.is_none() && self.channel.is_none() && self.requests_open {
            self.message_requests.clone().into_any_element()
        } else if self.guild.is_none() && self.channel.is_none() {
            self.friends.clone().into_any_element()
        } else if self.guild.is_some() && self.channel.is_none() {
            self.render_welcome(cx)
        } else if self.channel.is_some_and(|c| self.is_forum(c, cx)) {
            self.forum.clone().into_any_element()
        } else {
            self.chat.clone().into_any_element()
        };
        let in_forum = self.channel.is_some_and(|c| self.is_forum(c, cx));
        let members = (self.guild.is_some()
            && self.channel.is_some()
            && !in_forum
            && !in_call
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
        let over_chat = !(self.guild.is_none() && self.channel.is_none())
            && !(self.guild.is_some() && self.channel.is_none())
            && !in_forum
            && !in_call;
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
        let edit_profile = self.edit_profile_open.then(|| self.edit_profile.clone());
        let forward = self.forward_open.then(|| self.forward.clone());
        let nickname = self.nickname_open.then(|| self.nickname.clone());
        let friend_nick = self.friend_nick_open.then(|| self.friend_nick.clone());
        let ban_dialog = self.ban_dialog_open.then(|| self.ban_dialog.clone());
        let create_server_dialog = self.create_server_open.then(|| self.create_server_dialog.clone());
        let password_dialog = self.password_open.then(|| self.password_dialog.clone());
        let totp_dialog = self.totp_open.then(|| self.totp_dialog.clone());
        let danger_dialog = self.danger_open.then(|| self.danger_dialog.clone());
        let poll_voters_modal = self.poll_voters_open.then(|| {
            let users = self.poll_voters.clone();
            let on_close = cx.listener(|this, _: &ClickEvent, _, cx| {
                this.poll_voters_open = false;
                cx.notify();
            });
            profile::modal(
                div()
                    .w(px(320.))
                    .max_h(px(400.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(16.))
                    .bg(rgb(theme::BG_PANEL))
                    .rounded(px(theme::RADIUS))
                    .children(users.iter().map(|u| div().child(u.display_name().to_owned())))
                    .into_any_element(),
                Box::new(on_close),
            )
        });
        let create_poll_dialog = self.create_poll_open.then(|| self.create_poll_dialog.clone());
        let kick_dialog = self.kick_dialog_open.then(|| self.kick_dialog.clone());
        let timeout_dialog = self.timeout_dialog_open.then(|| self.timeout_dialog.clone());
        let roles_dialog = self.roles_dialog_open.then(|| self.roles_dialog.clone());
        let join = self.join_open.then(|| self.join.clone());
        let screening = self.screening_open.then(|| self.screening.clone());
        let onboarding = self.onboarding_open.then(|| self.onboarding.clone());
        let events = self.events_open.then(|| self.events.clone());
        let interaction_modal = self.interaction_modal_open.then(|| self.interaction_modal.clone());
        let switcher = self.switcher_open.then(|| self.switcher.clone());
        let picker = self.picker_open.then(|| self.picker.clone());
        let popup = self.popup_open.then(|| self.popup.clone());
        let invite = self.invite_open.then(|| self.invite.clone());
        let edit_channel = self.edit_channel_open.then(|| self.edit_channel.clone());
        let channel_settings = self.channel_settings_open.then(|| self.channel_settings.clone());
        let create_channel = self.create_channel_open.then(|| self.create_channel.clone());
        let gift_dialog = self.gift_open.then(|| self.gift_dialog.clone());
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
        // The user card floats across the bottom of the rail and the channel list.
        let user_card = div()
            .absolute()
            .left(px(8.))
            .bottom(px(8.))
            .w(px(theme::RAIL_WIDTH + theme::SIDEBAR_WIDTH - 16.))
            .child(self.render_user_panel(cx));
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
            .child(user_card)
            .children(profile)
            .children(account)
            .children(edit_profile)
            .children(self.nameplate_open.then(|| self.nameplate_dialog.clone()))
            .children(self.decoration_open.then(|| self.decoration_dialog.clone()))
            .children(self.effect_open.then(|| self.effect_dialog.clone()))
            .children(self.frame_open.then(|| self.frame_dialog.clone()))
            .children(status_dialog)
            .children(menu)
            .children(viewer)
            .children(forward)
            .children(nickname)
            .children(friend_nick)
            .children(ban_dialog)
            .children(kick_dialog)
            .children(create_server_dialog)
            .children(poll_voters_modal)
            .children(create_poll_dialog)
            .children(timeout_dialog)
            .children(roles_dialog)
            .children(join)
            .children(screening)
            .children(onboarding)
            .children(events)
            .children(interaction_modal)
            .children(invite)
            .children(edit_channel)
            .children(channel_settings)
            .children(create_channel)
            .children(gift_dialog)
            .children(server_settings)
            .children(switcher)
            .children(picker)
            .children(popup)
            .children(inbox)
            .children(channel_status_modal)
            .children(settings)
            // Opened from the settings page, so above it.
            .children(password_dialog)
            .children(totp_dialog)
            .children(danger_dialog)
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

/// Whether the Events row heads the channel list: when something is running
/// or coming, and for anyone who can put an event on, so the way in is there
/// before the first one exists.
fn shows_events_row(upcoming: usize, perms: oxicord_model::Permissions) -> bool {
    use oxicord_model::Permissions;
    upcoming > 0 || perms.intersects(Permissions::CREATE_EVENTS | Permissions::MANAGE_EVENTS)
}

fn events_label(upcoming: usize) -> String {
    match upcoming {
        0 => "Events".to_owned(),
        1 => "1 Event".to_owned(),
        n => format!("{n} Events"),
    }
}

/// The scalloped badge before the name of a boosted server, with the gem in it.
fn boost_badge() -> impl IntoElement {
    div()
        .relative()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(icon("flower-star", 18., 0x808189))
        .child(div().absolute().left(px(5.)).top(px(4.)).child(icon("boost-gem", 12., 0xff73fa)))
}

/// How full the Boost Goal bar is and what it says. The goals are 2, 7 and 14
/// boosts for levels 1 to 3, and a server at level 3 just shows its count.
fn boost_progress(tier: u8, boosts: u64) -> (f32, String) {
    let goal = match tier {
        0 => 2,
        1 => 7,
        2 => 14,
        _ => return (1.0, format!("{boosts} Boosts")),
    };
    ((boosts as f32 / goal as f32).clamp(0., 1.), format!("{boosts}/{goal} Boosts"))
}

/// The "Boost Goal" pill under the server banner: how far the server is
/// toward its next boost level, and how many boosts it has.
fn boost_goal(tier: u8, boosts: u64, below_banner: bool) -> impl IntoElement {
    let (fraction, label) = boost_progress(tier, boosts);
    let text = |content: String| div().text_size(px(12.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT)).child(content);
    div()
        .w_full()
        .flex_none()
        .mt(px(if below_banner { 12. } else { 0. }))
        .mb(px(12.))
        .px(px(4.))
        .pt(px(4.))
        .pb(px(12.))
        .child(
            div()
                .relative()
                .h(px(26.))
                .w_full()
                .rounded_full()
                .bg(rgb(0x000000))
                .child(
                    div().absolute().inset(px(2.)).child(
                        div().h_full().w(gpui_kit::relative(fraction)).rounded_full().bg(gpui_kit::linear_gradient(
                            270.,
                            gpui_kit::linear_color_stop(rgba(0xff4cd266), 0.),
                            gpui_kit::linear_color_stop(rgba(0xff4cd21a), 1.),
                        )),
                    ),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(div().flex().items_center().gap(px(2.)).child(text("Boost Goal".into())).child(text("\u{1f389}".into())))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(2.))
                                .child(text(label))
                                .child(icon("chevron-right", 12., theme::TEXT)),
                        ),
                ),
        )
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

/// One person's tile in a call, ringed while they speak.
fn call_tile(
    media: &Entity<Media>,
    person: &VoiceItem,
    speaking: bool,
    (tile_w, tile_h): (f32, f32),
    cx: &mut App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let tile_color = media
        .update(cx, |media, _| media.dominant_color(&person.avatar))
        .unwrap_or(theme::CALL_TILE);
    let avatar_size = (tile_h * 0.27).clamp(40., 80.);
    let mut pill = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .h(px(32.))
        .px(px(8.))
        .rounded(px(theme::RADIUS))
        .bg(rgba(0x0000_008f))
        .text_size(px(16.))
        .line_height(px(20.))
        .text_color(rgb(0xffffff))
        .child(person.name.clone());
    if person.deafened {
        pill = pill.child(icon("headphone-off", 16., 0xffffff));
    } else if person.muted {
        pill = pill.child(icon("mic-off", 16., 0xffffff));
    }
    div()
        .id(("call-tile", person.user.0))
        .relative()
        .flex_none()
        .w(px(tile_w))
        .h(px(tile_h))
        .rounded(px(theme::RADIUS))
        .overflow_hidden()
        .bg(rgb(tile_color))
        .flex()
        .items_center()
        .justify_center()
        .child(avatar(media, &person.avatar, avatar_size, &person.name, cx))
        .child(div().absolute().left(px(12.)).bottom(px(12.)).child(pill))
        // The ring goes on top so the speaker's tile is easy to spot.
        .child(
            div()
                .absolute()
                .size_full()
                .rounded(px(theme::RADIUS))
                .border_2()
                .border_color(if speaking { rgb(theme::STATUS_POSITIVE) } else { rgba(0) }),
        )
}

/// The little arrow beside mute and deafen that opens their device lists.
fn device_arrow(id: &'static str) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .w(px(16.))
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_r(px(theme::RADIUS))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
        .child(icon("chevron-down", 16., theme::TEXT_MUTED))
}

/// Width of the text chat opened beside a call.
const CALL_CHAT_WIDTH: f32 = 420.;

/// "A and B are currently in voice", as the page before joining says it.
fn splash_subtitle(names: &[&str]) -> String {
    match names {
        [] => "No one is currently in voice".to_owned(),
        [one] => format!("{one} is currently in voice"),
        [a, b] => format!("{a} and {b} are currently in voice"),
        [a, b, c] => format!("{a}, {b} and {c} are currently in voice"),
        [a, b, rest @ ..] => format!("{a}, {b} and {} others are currently in voice", rest.len()),
    }
}

/// A running call's length the way the channel list shows it: 4:07, or 1:15:43.
fn call_clock(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();
    let (hours, minutes, seconds) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The size of the biggest 16:9 tiles that fit `count` of them into an area.
/// Like the official client it fills rows before stacking: never fewer columns
/// than the square root of the count, so two people sit side by side.
fn call_tile_size(count: usize, area_w: f32, area_h: f32, gap: f32) -> (f32, f32) {
    let mut best = 0f32;
    let count = count.max(1);
    let fewest = (count as f32).sqrt().ceil() as usize;
    for columns in fewest..=count {
        let rows = count.div_ceil(columns);
        let by_width = (area_w - gap * (columns - 1) as f32) / columns as f32;
        let by_height = (area_h - gap * (rows - 1) as f32) / rows as f32 * 16. / 9.;
        best = best.max(by_width.min(by_height));
    }
    let width = best.max(120.);
    (width, width * 9. / 16.)
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
        user_limit: c.user_limit.filter(|&limit| limit > 0),
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
    fn the_boost_bar_fills_toward_the_next_level_and_tops_out_at_three() {
        assert_eq!(boost_progress(0, 1), (0.5, "1/2 Boosts".to_string()));
        assert_eq!(boost_progress(1, 7), (1.0, "7/7 Boosts".to_string()));
        assert_eq!(boost_progress(2, 7), (0.5, "7/14 Boosts".to_string()));
        assert_eq!(boost_progress(3, 62), (1.0, "62 Boosts".to_string()));
    }

    #[test]
    fn the_events_row_shows_for_upcoming_events_or_for_people_who_can_make_them() {
        use oxicord_model::Permissions;
        assert!(!shows_events_row(0, Permissions::SEND_MESSAGES));
        assert!(shows_events_row(2, Permissions::empty()));
        assert!(shows_events_row(0, Permissions::CREATE_EVENTS));
        assert!(shows_events_row(0, Permissions::MANAGE_EVENTS));
        assert_eq!(events_label(0), "Events");
        assert_eq!(events_label(1), "1 Event");
        assert_eq!(events_label(3), "3 Events");
    }

    #[test]
    fn the_page_before_joining_names_who_is_there() {
        assert_eq!(splash_subtitle(&[]), "No one is currently in voice");
        assert_eq!(splash_subtitle(&["Cotton"]), "Cotton is currently in voice");
        assert_eq!(splash_subtitle(&["Cotton", "Mervo"]), "Cotton and Mervo are currently in voice");
        assert_eq!(splash_subtitle(&["A", "B", "C"]), "A, B and C are currently in voice");
        assert_eq!(splash_subtitle(&["A", "B", "C", "D", "E"]), "A, B and 3 others are currently in voice");
    }

    #[test]
    fn the_call_clock_adds_hours_only_when_needed() {
        use std::time::Duration;
        assert_eq!(call_clock(Duration::from_secs(7)), "0:07");
        assert_eq!(call_clock(Duration::from_secs(247)), "4:07");
        assert_eq!(call_clock(Duration::from_secs(4543)), "1:15:43");
    }

    #[test]
    fn two_people_in_a_wide_area_sit_side_by_side_like_the_official_client() {
        // The measured web client: 1064 wide area with 8px padding gave
        // two 520 wide tiles of 16:9.
        let (w, h) = call_tile_size(2, 1048., 700., 8.);
        assert!((w - 520.).abs() < 0.5, "{w}");
        assert!((h - 292.5).abs() < 0.5, "{h}");
    }

    #[test]
    fn a_short_area_shrinks_the_tiles_to_fit_their_height() {
        let (w, h) = call_tile_size(1, 1000., 300., 8.);
        assert!((h - 300.).abs() < 0.5, "{h}");
        assert!((w / h - 16. / 9.).abs() < 0.01);
    }

    #[test]
    fn many_people_wrap_into_rows() {
        let (w, _) = call_tile_size(6, 1000., 600., 8.);
        // Three columns of two rows beat one row of six.
        assert!(w > 1000. / 6., "{w}");
    }

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
