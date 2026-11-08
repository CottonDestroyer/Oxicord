use std::collections::{HashMap, HashSet};

use gpui_kit::{Context, EventEmitter, Pixels, Point};
use oxicord_gateway::{ClientStatus, ConnectionState, GatewayEvent};
use oxicord_model::{Channel, Message, Profile, SavedMessage, SearchPage, Snowflake, ThreadPage, gateway::DispatchEvent};
use oxicord_store::{Changes, Store};

use crate::{
    AccountEvent, AccountRequest, Settings,
    account::{AccountState, Load},
    backend::{Backend, Command, VoiceNotice},
};

/// The signed in client state. Views observe this entity and listen for the
/// `Changes` it emits to redraw only what moved.
pub struct Session {
    pub store: Store,
    pub connection: ConnectionState,
    /// Channels whose last history request failed, with the reason to show.
    pub history_errors: HashMap<Snowflake, String>,
    /// The latest thing that went wrong, shown until dismissed.
    pub notice: Option<String>,
    pub settings: Settings,
    pub account: AccountState,
    pub admin: crate::admin::AdminState,
    /// The first few people to react, per message, emoji and count. The
    /// count is part of the key so a tooltip never lists stale people.
    pub reactors: HashMap<(Snowflake, String, u32), Vec<String>>,
    /// Our nickname per server, as far as we have asked.
    pub own_nicks: HashMap<Snowflake, Option<String>>,
    /// Pinned messages per channel, loaded when the panel asks.
    pub pins: HashMap<Snowflake, Load<Vec<Message>>>,
    /// Recent mentions for the inbox.
    pub mentions: Load<Vec<Message>>,
    pub saved_messages: Load<Vec<SavedMessage>>,
    pub scheduled_messages: Load<Vec<Message>>,
    pub bookmarked: HashSet<Snowflake>,
    /// The latest search: what was asked and how it went.
    pub search: Option<(String, Load<SearchPage>)>,
    /// The thread list last asked for per channel: open threads and, once
    /// loaded, archived ones.
    pub archived_threads: HashMap<Snowflake, Load<ThreadPage>>,
    /// How the last friend request sent by name went, with who it was for.
    pub friend_request: Option<(String, Result<(), String>)>,
    /// The invite being looked at in the join dialog.
    pub invite: Load<oxicord_model::InvitePreview>,
    /// Slash commands per channel, with the names of the apps that own them.
    pub commands: HashMap<Snowflake, Load<oxicord_model::CommandIndex>>,
    /// Set once a join attempt has finished.
    pub invite_joined: Option<Result<(), String>>,
    /// The link last made from the invite dialog.
    pub created_invite: Load<String>,
    /// Opening message of each forum post we have seen, by post id.
    pub previews: HashMap<Snowflake, Message>,
    /// A message a view should scroll to once it is on screen.
    pub jump_target: Option<(Snowflake, Snowflake)>,
    /// The GIF picker's results and the search they answer (`None` is the
    /// trending list).
    pub gifs: Load<Vec<oxicord_model::Gif>>,
    pub gif_query: Option<String>,
    /// People already looked up, so each is asked about once.
    users_requested: HashSet<Snowflake>,
    /// Posts whose preview was already asked for.
    preview_requested: HashSet<Snowflake>,
    /// The status we last chose, which the server won't echo back to us.
    pub status: ClientStatus,
    /// Profiles fetched so far, so reopening a card is instant.
    pub profiles: HashMap<Snowflake, Profile>,
    /// People whose profile was asked for already, answered or not.
    /// Whose profile was asked for, and in which server, since a server's
    /// roles and join date come only with the profile asked for there.
    profile_requested: HashMap<Snowflake, Option<Snowflake>>,
    /// The voice call we are in or joining.
    pub voice: VoiceCall,
    /// The microphone and speakers picked in the voice menus, by name.
    /// `None` is the system default.
    pub audio_input: Option<String>,
    pub audio_output: Option<String>,
}

/// Where our own voice call stands.
#[derive(Clone, Debug, Default)]
pub struct VoiceCall {
    /// The server (none for a private call) and channel we asked for, set
    /// from the moment we ask until the call ends.
    pub target: Option<(Option<Snowflake>, Snowflake)>,
    /// False while still joining.
    pub connected: bool,
    pub muted: bool,
    pub deafened: bool,
    /// Whether the call is end-to-end encrypted, with the code to compare.
    pub encrypted: bool,
    pub code: Option<String>,
    /// Things that do not work, like no speakers or no microphone.
    pub warnings: Vec<String>,
    /// Who is talking right now.
    pub speaking: HashSet<Snowflake>,
    /// When the call went live, for the clock beside the channel.
    pub since: Option<std::time::Instant>,
}

impl EventEmitter<Changes> for Session {}

/// Which list the side panel is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Pins,
    Search,
    Threads,
    VoiceDetails,
}

/// Someone asked to open the side panel.
pub struct OpenPanel(pub PanelKind);

impl EventEmitter<OpenPanel> for Session {}

/// A view wants the server or channel menu at a point on screen.
pub struct OpenMenu {
    pub target: crate::context_menu::Target,
    pub at: gpui_kit::Point<gpui_kit::Pixels>,
}

impl EventEmitter<OpenMenu> for Session {}

/// A view wants a menu of its own entries at a point on screen.
pub struct OpenPopup {
    pub at: gpui_kit::Point<gpui_kit::Pixels>,
    pub entries: Vec<crate::popup::Entry>,
    pub pick: crate::popup::Pick,
}

impl EventEmitter<OpenPopup> for Session {}

/// A view wants the menu for a person at a point on screen.
pub struct OpenUserMenu {
    pub user: Snowflake,
    pub guild: Option<Snowflake>,
    pub at: gpui_kit::Point<gpui_kit::Pixels>,
    /// Set for a conversation in the DM list, which adds closing it.
    pub dm: Option<Snowflake>,
    /// Whether a composer is on screen to take a mention.
    pub can_mention: bool,
}

impl EventEmitter<OpenUserMenu> for Session {}

/// Put a mention of this person into the composer.
pub struct MentionUser(pub Snowflake);

impl EventEmitter<MentionUser> for Session {}

/// Someone wants to invite people to the server `channel` belongs to.
pub struct OpenInviteDialog(pub Snowflake);

impl EventEmitter<OpenInviteDialog> for Session {}

/// Someone wants to change their nickname in this server.
pub struct OpenNickname(pub Snowflake);

impl EventEmitter<OpenNickname> for Session {}

/// Someone wants to ban this member from this server.
pub struct OpenBanMember(pub Snowflake, pub Snowflake);

impl EventEmitter<OpenBanMember> for Session {}

/// Someone wants to kick this member from this server.
pub struct OpenKickMember(pub Snowflake, pub Snowflake);

impl EventEmitter<OpenKickMember> for Session {}

/// Someone wants to time this member out in this server.
pub struct OpenTimeoutMember(pub Snowflake, pub Snowflake);

impl EventEmitter<OpenTimeoutMember> for Session {}

/// Someone wants to edit this member's roles in this server.
pub struct OpenRolesMember(pub Snowflake, pub Snowflake);

impl EventEmitter<OpenRolesMember> for Session {}

/// Someone wants the settings of this server.
pub struct OpenServerSettings(pub Snowflake);

impl EventEmitter<OpenServerSettings> for Session {}

/// Someone wants to edit this channel (name and topic), from its sidebar row.
pub struct OpenChannelSettings(pub Snowflake);

impl EventEmitter<OpenChannelSettings> for Session {}

/// Someone wants to create a channel in `guild`, optionally under the named
/// category (`parent`). Emitted from a category header's "Create Channel" button.
pub struct OpenCreateChannel(pub Snowflake, pub Option<Snowflake>);

impl EventEmitter<OpenCreateChannel> for Session {}

/// Someone wants to open the nitro gift dialog from the composer's "Send a gift"
/// button. Oxicord does not process payments, so this is a read-only upsell view.
pub struct OpenGift;

impl EventEmitter<OpenGift> for Session {}

/// Someone wants to open the "Application Launcher" from the composer's "Apps"
/// button. Discord fills this with its global app directory and server activities;
/// Oxicord has no such backend yet, so the launcher shows an honest empty state.
pub struct OpenApps;

impl EventEmitter<OpenApps> for Session {}

/// Someone wants the privacy settings.
pub struct OpenPrivacySettings;

impl EventEmitter<OpenPrivacySettings> for Session {}

/// Someone wants to edit their per-server profile.
pub struct OpenServerProfile(pub Snowflake);

impl EventEmitter<OpenServerProfile> for Session {}

/// Someone wants to forward a message: its channel, server and id.
pub struct OpenForward {
    pub channel: Snowflake,
    pub guild: Option<Snowflake>,
    pub message: Snowflake,
}

impl EventEmitter<OpenForward> for Session {}

/// The inbox button was pressed.
pub struct OpenInbox;

impl EventEmitter<OpenInbox> for Session {}

/// A view wants the friend picker for starting or growing a conversation.
pub struct OpenPicker {
    pub purpose: crate::dm_picker::Purpose,
    pub at: gpui_kit::Point<gpui_kit::Pixels>,
}

impl EventEmitter<OpenPicker> for Session {}

/// Someone picked a message in a list and wants to go to its channel, and
/// to the message itself when one is named.
pub struct JumpTo {
    pub channel: Snowflake,
    pub message: Option<Snowflake>,
}

impl EventEmitter<JumpTo> for Session {}

/// A conversation the user asked to open is ready to be shown.
pub struct DmOpened(pub Snowflake);

impl EventEmitter<DmOpened> for Session {}

/// Someone asked to see a person's profile card.
pub struct ShowProfile {
    pub user: Snowflake,
    /// The server the click happened in, for server specific details.
    pub guild: Option<Snowflake>,
    /// Where the click was, so the small card can open beside it.
    pub at: Option<Point<Pixels>>,
    /// Open the large profile instead of the small card.
    pub full: bool,
}

impl EventEmitter<ShowProfile> for Session {}

impl Session {
    pub fn new(settings: Settings) -> Self {
        Self {
            store: Store::new(),
            connection: ConnectionState::Connecting,
            history_errors: HashMap::new(),
            reactors: HashMap::new(),
            own_nicks: HashMap::new(),
            notice: None,
            settings,
            account: AccountState::default(),
            admin: crate::admin::AdminState::default(),
            pins: HashMap::new(),
            search: None,
            archived_threads: HashMap::new(),
            friend_request: None,
            invite: Load::Idle,
            commands: HashMap::new(),
            mentions: Load::Idle,
            saved_messages: Load::Idle,
            scheduled_messages: Load::Idle,
            bookmarked: HashSet::new(),
            invite_joined: None,
            created_invite: Load::Idle,
            previews: HashMap::new(),
            jump_target: None,
            gifs: Load::Idle,
            gif_query: None,
            users_requested: HashSet::new(),
            preview_requested: HashSet::new(),
            status: ClientStatus::default(),
            profiles: HashMap::new(),
            profile_requested: HashMap::new(),
            voice: VoiceCall::default(),
            audio_input: None,
            audio_output: None,
        }
    }

    /// Joins a voice channel, leaving the call we were in if any.
    pub fn join_voice(
        &mut self,
        backend: &Backend,
        guild: Option<Snowflake>,
        channel: Snowflake,
        cx: &mut Context<Self>,
    ) {
        if self.voice.target == Some((guild, channel)) {
            return;
        }
        self.voice = VoiceCall {
            target: Some((guild, channel)),
            muted: self.voice.muted,
            deafened: self.voice.deafened,
            ..VoiceCall::default()
        };
        backend.send(Command::JoinVoice { guild, channel });
        cx.notify();
    }

    pub fn leave_voice(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        if self.voice.target.is_none() {
            return;
        }
        backend.send(Command::LeaveVoice);
        self.voice = VoiceCall { muted: self.voice.muted, deafened: self.voice.deafened, ..VoiceCall::default() };
        cx.notify();
    }

    pub fn set_audio_input(&mut self, backend: &Backend, device: Option<String>, cx: &mut Context<Self>) {
        self.audio_input = device;
        self.send_audio_devices(backend, cx);
    }

    pub fn set_audio_output(&mut self, backend: &Backend, device: Option<String>, cx: &mut Context<Self>) {
        self.audio_output = device;
        self.send_audio_devices(backend, cx);
    }

    fn send_audio_devices(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        backend.send(Command::SetAudioDevices {
            input: self.audio_input.clone(),
            output: self.audio_output.clone(),
        });
        cx.notify();
    }

    pub fn toggle_voice_mute(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        self.voice.muted = !self.voice.muted;
        self.send_voice_flags(backend, cx);
    }

    pub fn toggle_voice_deafen(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        self.voice.deafened = !self.voice.deafened;
        self.send_voice_flags(backend, cx);
    }

    fn send_voice_flags(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        // Deafening implies muting, as in the official client.
        backend.send(Command::SetVoiceFlags {
            mute: self.voice.muted || self.voice.deafened,
            deafen: self.voice.deafened,
        });
        cx.notify();
    }

    pub fn apply_voice(&mut self, notice: VoiceNotice, cx: &mut Context<Self>) {
        match notice {
            VoiceNotice::Connected => {
                self.voice.connected = true;
                self.voice.since = Some(std::time::Instant::now());
            }
            VoiceNotice::Warning(text) => {
                if !self.voice.warnings.contains(&text) {
                    self.voice.warnings.push(text);
                }
            }
            VoiceNotice::Speaking { user, speaking } => {
                if speaking {
                    self.voice.speaking.insert(user);
                } else {
                    self.voice.speaking.remove(&user);
                }
            }
            VoiceNotice::Encryption { active, code } => {
                self.voice.encrypted = active;
                self.voice.code = code;
            }
            VoiceNotice::Left(reason) => {
                self.voice = VoiceCall {
                    muted: self.voice.muted,
                    deafened: self.voice.deafened,
                    ..VoiceCall::default()
                };
                if reason.is_some() {
                    self.notice = reason;
                }
            }
        }
        cx.notify();
    }

    pub fn open_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        cx.emit(OpenPanel(kind));
    }

    pub fn jump_to(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        cx.emit(JumpTo { channel, message: None });
    }

    pub fn jump_to_message(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        cx.emit(JumpTo { channel, message: Some(message) });
    }

    /// Takes in the window of history around a message and remembers where
    /// the view should scroll to.
    pub fn apply_around(
        &mut self,
        channel: Snowflake,
        message: Snowflake,
        result: Result<Vec<Message>, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(page) => {
                let changes = self.store.show_window(channel, page);
                self.jump_target = Some((channel, message));
                cx.emit(changes);
                cx.notify();
            }
            Err(error) => self.set_notice(Some(format!("Could not load that message. {error}")), cx),
        }
    }

    pub fn request_mentions(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        self.mentions = Load::Loading;
        backend.send(Command::LoadMentions);
        cx.notify();
    }

    pub fn apply_mentions(&mut self, result: Result<Vec<Message>, String>, cx: &mut Context<Self>) {
        self.mentions = match result {
            Ok(list) => Load::Ready(list),
            Err(error) => Load::Failed(error),
        };
        cx.notify();
    }

    pub fn request_saved_messages(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        self.saved_messages = Load::Loading;
        backend.send(Command::LoadSavedMessages);
        cx.notify();
    }

    pub fn apply_saved_messages(&mut self, result: Result<Vec<SavedMessage>, String>, cx: &mut Context<Self>) {
        self.saved_messages = match result {
            Ok(list) => {
                self.bookmarked = list.iter().map(|saved| saved.message.id).collect();
                Load::Ready(list)
            }
            Err(error) => Load::Failed(error),
        };
        cx.notify();
    }

    pub fn request_scheduled_messages(&mut self, backend: &Backend, cx: &mut Context<Self>) {
        self.scheduled_messages = Load::Loading;
        backend.send(Command::LoadScheduledMessages);
        cx.notify();
    }

    pub fn apply_scheduled_messages(&mut self, result: Result<Vec<Message>, String>, cx: &mut Context<Self>) {
        self.scheduled_messages = match result {
            Ok(list) => Load::Ready(list),
            Err(error) => Load::Failed(error),
        };
        cx.notify();
    }

    pub fn is_bookmarked(&self, message: Snowflake) -> bool {
        self.bookmarked.contains(&message)
    }

    pub fn set_bookmarked(&mut self, message: Snowflake, bookmarked: bool, cx: &mut Context<Self>) {
        if bookmarked {
            self.bookmarked.insert(message);
        } else {
            self.bookmarked.remove(&message);
        }
        cx.notify();
    }

    pub fn request_pins(&mut self, backend: &Backend, channel: Snowflake, cx: &mut Context<Self>) {
        self.pins.insert(channel, Load::Loading);
        backend.send(Command::LoadPins(channel));
        cx.notify();
    }

    pub fn apply_pins(
        &mut self,
        channel: Snowflake,
        result: Result<Vec<Message>, String>,
        cx: &mut Context<Self>,
    ) {
        let state = match result {
            Ok(pins) => Load::Ready(pins),
            Err(error) => Load::Failed(error),
        };
        self.pins.insert(channel, state);
        cx.notify();
    }

    /// Starts a search. `guild` is `None` inside a conversation.
    pub fn start_search(
        &mut self,
        backend: &Backend,
        guild: Option<Snowflake>,
        channel: Snowflake,
        raw: String,
        cx: &mut Context<Self>,
    ) {
        let query = crate::search_query::parse(&self.store, guild, channel, &raw);
        self.search = Some((raw.clone(), Load::Loading));
        backend.send(Command::Search { guild, channel, raw, query, offset: 0 });
        cx.notify();
    }

    /// Ignores answers to a search the user has already replaced.
    pub fn apply_search(
        &mut self,
        query: String,
        result: Result<SearchPage, String>,
        cx: &mut Context<Self>,
    ) {
        if self.search.as_ref().is_some_and(|(asked, _)| *asked == query) {
            let state = match result {
                Ok(page) => Load::Ready(page),
                Err(error) => Load::Failed(error),
            };
            self.search = Some((query, state));
            cx.notify();
        }
    }

    /// Asks for a channel's slash commands the first time they are needed.
    pub fn ensure_commands(&mut self, backend: &Backend, channel: Snowflake) {
        if let std::collections::hash_map::Entry::Vacant(slot) = self.commands.entry(channel) {
            slot.insert(Load::Loading);
            backend.send(Command::LoadCommands(channel));
        }
    }

    pub fn apply_commands(
        &mut self,
        channel: Snowflake,
        result: Result<oxicord_model::CommandIndex, String>,
        cx: &mut Context<Self>,
    ) {
        let state = match result {
            Ok(index) => Load::Ready(index),
            Err(error) => Load::Failed(error),
        };
        self.commands.insert(channel, state);
        cx.notify();
    }

    pub fn apply_invite_preview(&mut self, result: Result<oxicord_model::InvitePreview, String>, cx: &mut Context<Self>) {
        self.invite = match result {
            Ok(preview) => Load::Ready(preview),
            Err(error) => Load::Failed(error),
        };
        cx.notify();
    }

    pub fn request_invite(&mut self, backend: &Backend, channel: Snowflake, max_age: u32, max_uses: u32, cx: &mut Context<Self>) {
        self.created_invite = Load::Loading;
        backend.send(Command::CreateInvite { channel, max_age, max_uses });
        cx.notify();
    }

    pub fn apply_created_invite(&mut self, result: Result<String, String>, cx: &mut Context<Self>) {
        self.created_invite = match result {
            Ok(code) => Load::Ready(code),
            Err(error) => Load::Failed(error),
        };
        cx.notify();
    }

    pub fn apply_invite_joined(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.invite_joined = Some(result);
        cx.notify();
    }

    pub fn apply_friend_request(&mut self, username: String, result: Result<(), String>, cx: &mut Context<Self>) {
        self.friend_request = Some((username, result));
        cx.notify();
    }

    pub fn request_threads(&mut self, backend: &Backend, channel: Snowflake, cx: &mut Context<Self>) {
        self.archived_threads.insert(channel, Load::Loading);
        backend.send(Command::LoadThreads { channel, archived: true, offset: 0 });
        cx.notify();
    }

    /// Remembers a page of threads, and teaches the store about them so
    /// picking one can open it.
    pub fn apply_threads(
        &mut self,
        channel: Snowflake,
        result: Result<ThreadPage, String>,
        cx: &mut Context<Self>,
    ) {
        let state = match result {
            Ok(page) => {
                for message in &page.first_messages {
                    self.previews.insert(message.channel_id, message.clone());
                }
                for thread in &page.threads {
                    let joined = page.joined.contains(&thread.id);
                    self.store.add_thread(thread.clone(), joined);
                }
                Load::Ready(page)
            }
            Err(error) => Load::Failed(error),
        };
        self.archived_threads.insert(channel, state);
        cx.notify();
    }

    /// Asks for the previews of posts we haven't asked about yet.
    pub fn request_previews(&mut self, backend: &Backend, forum: Snowflake, posts: &[Snowflake]) {
        let fresh: Vec<Snowflake> = posts
            .iter()
            .copied()
            .filter(|p| !self.previews.contains_key(p) && self.preview_requested.insert(*p))
            .collect();
        for chunk in fresh.chunks(25) {
            backend.send(Command::LoadForumPreviews { forum, posts: chunk.to_vec() });
        }
    }

    /// Asks for the people in `ids` that we haven't asked about yet. Inside a
    /// server the gateway answers for all of them at once, elsewhere each one
    /// costs a request.
    pub fn request_users(&mut self, backend: &Backend, guild: Option<Snowflake>, ids: Vec<Snowflake>) {
        let fresh: Vec<Snowflake> = ids
            .into_iter()
            .filter(|id| self.store.user(*id).is_none() && self.users_requested.insert(*id))
            .take(20)
            .collect();
        if fresh.is_empty() {
            return;
        }
        match guild {
            Some(guild) => backend.send(Command::RequestMembers { guild, ids: fresh }),
            None => backend.send(Command::FetchUsers(fresh)),
        }
    }

    pub fn apply_users(&mut self, users: Vec<oxicord_model::User>, cx: &mut Context<Self>) {
        self.store.add_users(users);
        cx.emit(Changes::full());
        cx.notify();
    }

    pub fn request_gifs(&mut self, backend: &Backend, query: Option<String>, cx: &mut Context<Self>) {
        self.gifs = Load::Loading;
        self.gif_query.clone_from(&query);
        backend.send(Command::LoadGifs(query));
        cx.notify();
    }

    /// Ignores answers to a search the user has already replaced.
    pub fn apply_gifs(
        &mut self,
        query: Option<String>,
        result: Result<Vec<oxicord_model::Gif>, String>,
        cx: &mut Context<Self>,
    ) {
        if self.gif_query == query {
            self.gifs = match result {
                Ok(list) => Load::Ready(list),
                Err(error) => Load::Failed(error),
            };
            cx.notify();
        }
    }

    pub fn apply_previews(&mut self, messages: Vec<Message>, cx: &mut Context<Self>) {
        for message in messages {
            self.previews.insert(message.channel_id, message);
        }
        cx.notify();
    }

    /// A thread we just created: add it and go there.
    pub fn apply_new_thread(&mut self, result: Result<Channel, String>, cx: &mut Context<Self>) {
        match result {
            Ok(thread) => {
                let id = thread.id;
                self.store.add_thread(thread, true);
                cx.emit(Changes::full());
                cx.emit(JumpTo { channel: id, message: None });
                cx.notify();
            }
            Err(error) => self.set_notice(Some(format!("Couldn't start a thread. {error}")), cx),
        }
    }

    /// Pins or unpins a message, shown at once and sent in the background.
    pub fn set_pinned(
        &mut self,
        backend: &Backend,
        channel: Snowflake,
        message: Snowflake,
        pinned: bool,
        cx: &mut Context<Self>,
    ) {
        let changes = self.store.set_pinned(channel, message, pinned);
        backend.send(Command::SetPinned { channel, message, pinned });
        cx.emit(changes);
        cx.notify();
    }

    pub fn apply_admin(&mut self, event: crate::AdminEvent, cx: &mut Context<Self>) {
        self.admin.apply(event);
        cx.notify();
    }

    /// Sends a server management request. A list request first resets that
    /// list to loading so the page shows a spinner instead of stale rows.
    pub fn request_admin(&mut self, backend: &Backend, request: crate::AdminRequest, cx: &mut Context<Self>) {
        use crate::{AdminRequest as R, account::Load};
        match &request {
            R::LoadInvites(g) => {
                self.admin.switch_to(*g);
                self.admin.invites = Load::Loading;
            }
            R::LoadBans(g) => {
                self.admin.switch_to(*g);
                self.admin.bans = Load::Loading;
            }
            R::LoadAudit(g) => {
                self.admin.switch_to(*g);
                self.admin.audit = Load::Loading;
            }
            _ => {}
        }
        backend.send(Command::Admin(request));
        cx.notify();
    }

    pub fn apply_account(&mut self, event: AccountEvent, cx: &mut Context<Self>) {
        self.account.apply(event);
        cx.notify();
    }

    /// Asks the backend for a piece of account data, remembering that it is
    /// on its way so screens can show a spinner.
    pub fn request_account(&mut self, backend: &Backend, request: AccountRequest, cx: &mut Context<Self>) {
        match &request {
            AccountRequest::LoadDetails => self.account.details = Load::Loading,
            AccountRequest::LoadSettings => self.account.settings = Load::Loading,
            AccountRequest::LoadApps | AccountRequest::RevokeApp(_) => self.account.apps = Load::Loading,
            AccountRequest::LoadConnections => self.account.connections = Load::Loading,
            AccountRequest::LoadSessions => self.account.sessions = Load::Loading,
            _ => {}
        }
        backend.send(Command::Account(request));
        cx.notify();
    }

    /// Changes account preferences: shown at once, saved in the background.
    pub fn change_prefs(
        &mut self,
        backend: &Backend,
        changes: &[(oxicord_settings::Key, oxicord_settings::Value)],
        cx: &mut Context<Self>,
    ) {
        if let Some(patch) = self.account.change(changes) {
            backend.send(Command::Account(AccountRequest::SaveSettings(patch)));
        }
        cx.notify();
    }

    /// Sets or clears our custom status: shown at once, saved in the background.
    pub fn set_custom_status(
        &mut self,
        backend: &Backend,
        status: Option<&oxicord_settings::CustomStatus>,
        cx: &mut Context<Self>,
    ) {
        if let Some(patch) = self.account.change_custom_status(status, chrono::Utc::now().timestamp_millis().max(0) as u64) {
            backend.send(Command::Account(AccountRequest::SaveSettings(patch)));
        }
        cx.notify();
    }

    /// Applies a change to the preferences and persists it.
    pub fn change_settings(
        &mut self,
        backend: &Backend,
        change: impl FnOnce(&mut Settings),
        cx: &mut Context<Self>,
    ) {
        change(&mut self.settings);
        backend.send(Command::SaveSettings(self.settings.clone()));
        cx.notify();
    }

    pub fn set_notice(&mut self, notice: Option<String>, cx: &mut Context<Self>) {
        self.notice = notice;
        cx.notify();
    }

    pub fn handle(&mut self, backend: &Backend, event: GatewayEvent, cx: &mut Context<Self>) {
        match event {
            GatewayEvent::Dispatch(DispatchEvent::Unknown { name, data })
                if name == "GUILD_BAN_ADD" || name == "GUILD_BAN_REMOVE" =>
            {
                let guild = serde_json::from_value::<Snowflake>(data["guild_id"].clone()).ok();
                let user = serde_json::from_value::<oxicord_model::User>(data["user"].clone()).ok();
                if let (Some(guild), Some(user)) = (guild, user) {
                    self.admin.apply_ban(guild, user, name == "GUILD_BAN_ADD");
                    cx.notify();
                }
            }
            // Preferences, folders or status changed on another device (or by
            // us, echoed). Type 1 is the main blob. Reading it again beats
            // merging a partial message, and the old values stay up meanwhile.
            GatewayEvent::Dispatch(DispatchEvent::Unknown { name, data })
                if name == "USER_SETTINGS_PROTO_UPDATE" && data["settings"]["type"] == 1 =>
            {
                if matches!(self.account.settings, Load::Ready(_)) {
                    backend.send(Command::Account(AccountRequest::LoadSettings));
                }
            }
            // Cleared from the inbox on another device.
            GatewayEvent::Dispatch(DispatchEvent::Unknown { name, data }) if name == "RECENT_MENTION_DELETE" => {
                let gone = serde_json::from_value::<Snowflake>(data["message_id"].clone()).ok();
                if let (Some(gone), Load::Ready(list)) = (gone, &mut self.mentions) {
                    list.retain(|m| m.id != gone);
                    cx.notify();
                }
            }
            // Somebody pinned or unpinned something. Only channels whose pins
            // we already fetched are worth asking about again.
            GatewayEvent::Dispatch(DispatchEvent::Unknown { name, data }) if name == "CHANNEL_PINS_UPDATE" => {
                let channel = serde_json::from_value::<Snowflake>(data["channel_id"].clone()).ok();
                if let Some(channel) = channel.filter(|c| self.pins.contains_key(c)) {
                    // The old list stays up until the new one lands.
                    backend.send(Command::LoadPins(channel));
                }
            }
            // Our own note about someone changed on another device.
            GatewayEvent::Dispatch(DispatchEvent::Unknown { name, data }) if name == "USER_NOTE_UPDATE" => {
                #[derive(serde::Deserialize)]
                struct NoteUpdate {
                    id: Snowflake,
                    #[serde(default)]
                    note: String,
                }
                if let Ok(update) = serde_json::from_value::<NoteUpdate>(data.clone()) {
                    if let Some(profile) = self.profiles.get_mut(&update.id) {
                        profile.note = update.note;
                        cx.notify();
                    }
                }
            }
            // The app never answered a click or command.
            GatewayEvent::Dispatch(DispatchEvent::Unknown { name, .. }) if name == "INTERACTION_FAILURE" => {
                self.set_notice(Some("That interaction failed. The app didn't respond.".into()), cx);
            }
            GatewayEvent::Dispatch(event) => {
                // People who left the server aren't members any more, but
                // their account can still be looked up.
                if let DispatchEvent::MemberChunk(chunk) = &event
                    && !chunk.not_found.is_empty()
                {
                    backend.send(Command::FetchUsers(chunk.not_found.clone()));
                }
                let changes = self.store.apply(event);
                if !changes.is_empty() {
                    cx.emit(changes);
                    cx.notify();
                }
            }
            GatewayEvent::State(state) => {
                self.connection = state;
                cx.notify();
            }
        }
    }

    /// Asks for the small card beside `at`.
    pub fn show_profile(
        &mut self,
        user: Snowflake,
        guild: Option<Snowflake>,
        at: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        cx.emit(ShowProfile { user, guild, at, full: false });
    }

    /// Asks for the large profile.
    pub fn show_full_profile(&mut self, user: Snowflake, guild: Option<Snowflake>, cx: &mut Context<Self>) {
        cx.emit(ShowProfile { user, guild, at: None, full: true });
    }

    /// Returns true the first time a profile is wanted, which is when it is
    /// worth asking the server.
    pub fn should_fetch_profile(&mut self, user: Snowflake, guild: Option<Snowflake>) -> bool {
        match self.profile_requested.insert(user, guild) {
            None => true,
            // Asked before outside a server, now inside one: ask again.
            Some(before) => before != guild && guild.is_some(),
        }
    }

    pub fn apply_profile(
        &mut self,
        user: Snowflake,
        result: Result<Profile, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(profile) => {
                self.profiles.insert(user, profile);
            }
            // Let a later click try again.
            Err(_) => {
                self.profile_requested.remove(&user);
            }
        }
        cx.notify();
    }

    /// Takes in the conversation the server created or found for us, then
    /// tells the views to switch to it.
    pub fn apply_dm(&mut self, result: Result<Channel, String>, cx: &mut Context<Self>) {
        match result {
            Ok(channel) => {
                let id = channel.id;
                let changes = self
                    .store
                    .apply(DispatchEvent::ChannelCreate(Box::new(channel)));
                cx.emit(changes);
                cx.emit(DmOpened(id));
                cx.notify();
            }
            Err(error) => self.set_notice(Some(format!("Couldn't open that conversation. {error}")), cx),
        }
    }

    /// Folds in a page of history. A failure is remembered and announced the
    /// same way, so a view waiting on the channel stops waiting.
    pub fn apply_history(
        &mut self,
        channel: Snowflake,
        result: Result<Vec<Message>, String>,
        requested: usize,
        cx: &mut Context<Self>,
    ) {
        let changes = match result {
            Ok(page) => {
                self.history_errors.remove(&channel);
                self.store.set_history(channel, page, requested)
            }
            Err(error) => {
                self.history_errors.insert(channel, error);
                let mut changes = Changes::default();
                changes.messages.insert(channel);
                changes
            }
        };
        cx.emit(changes);
        cx.notify();
    }
}
