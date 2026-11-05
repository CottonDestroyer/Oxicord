use std::collections::{HashMap, HashSet};

use gpui_kit::{Context, EventEmitter};
use oxicord_gateway::{ClientStatus, ConnectionState, GatewayEvent};
use oxicord_model::{Channel, Message, Profile, Snowflake, gateway::DispatchEvent};
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
    /// The status we last chose, which the server won't echo back to us.
    pub status: ClientStatus,
    /// Profiles fetched so far, so reopening a card is instant.
    pub profiles: HashMap<Snowflake, Profile>,
    /// People whose profile was asked for already, answered or not.
    profile_requested: HashSet<Snowflake>,
}

impl EventEmitter<Changes> for Session {}

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
            status: ClientStatus::default(),
            profiles: HashMap::new(),
            profile_requested: HashSet::new(),
        }
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
