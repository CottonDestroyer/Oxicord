use std::collections::{HashMap, HashSet};

use gpui_kit::{Context, EventEmitter};
use oxicord_gateway::{ClientStatus, ConnectionState, GatewayEvent};
use oxicord_model::{Channel, Message, Profile, SearchPage, Snowflake, ThreadPage, gateway::DispatchEvent};
use oxicord_store::{Changes, Store};

use crate::{
    AccountEvent, AccountRequest, Settings,
    account::{AccountState, Load},
    backend::{Backend, Command},
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
    /// Pinned messages per channel, loaded when the panel asks.
    pub pins: HashMap<Snowflake, Load<Vec<Message>>>,
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
    profile_requested: HashSet<Snowflake>,
}

impl EventEmitter<Changes> for Session {}

/// Which list the side panel is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Pins,
    Search,
    Threads,
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
}

impl EventEmitter<ShowProfile> for Session {}

impl Session {
    pub fn new(settings: Settings) -> Self {
        Self {
            store: Store::new(),
            connection: ConnectionState::Connecting,
            history_errors: HashMap::new(),
            notice: None,
            settings,
            account: AccountState::default(),
            pins: HashMap::new(),
            search: None,
            archived_threads: HashMap::new(),
            friend_request: None,
            invite: Load::Idle,
            commands: HashMap::new(),
            invite_joined: None,
            previews: HashMap::new(),
            jump_target: None,
            gifs: Load::Idle,
            gif_query: None,
            users_requested: HashSet::new(),
            preview_requested: HashSet::new(),
            status: ClientStatus::default(),
            profiles: HashMap::new(),
            profile_requested: HashSet::new(),
        }
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

    /// Asks for the people in `ids` that we haven't asked about yet.
    pub fn request_users(&mut self, backend: &Backend, ids: Vec<Snowflake>) {
        let fresh: Vec<Snowflake> = ids
            .into_iter()
            .filter(|id| self.store.user(*id).is_none() && self.users_requested.insert(*id))
            .take(20)
            .collect();
        if !fresh.is_empty() {
            backend.send(Command::FetchUsers(fresh));
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

    pub fn handle(&mut self, event: GatewayEvent, cx: &mut Context<Self>) {
        match event {
            // The app never answered a click or command.
            GatewayEvent::Dispatch(DispatchEvent::Unknown { name, .. }) if name == "INTERACTION_FAILURE" => {
                self.set_notice(Some("That interaction failed. The app didn't respond.".into()), cx);
            }
            GatewayEvent::Dispatch(event) => {
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

    pub fn show_profile(&mut self, user: Snowflake, guild: Option<Snowflake>, cx: &mut Context<Self>) {
        cx.emit(ShowProfile { user, guild });
    }

    /// Returns true the first time a profile is wanted, which is when it is
    /// worth asking the server.
    pub fn should_fetch_profile(&mut self, user: Snowflake) -> bool {
        self.profile_requested.insert(user)
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
