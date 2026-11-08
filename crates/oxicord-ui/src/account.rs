//! Account level data the settings screens show: details, preferences and
//! the lists of apps, linked accounts and sessions. Each piece loads on its
//! own, the first time a screen needs it.

use oxicord_model::{AccountDetails, AuthorizedApp, Connection, LoginSession, Snowflake};
use oxicord_settings::{AccountSettings, CustomStatus, Key, Patch, Value};

/// A value that may still be on its way.
#[derive(Clone, Debug, Default)]
pub enum Load<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn from_result(result: Result<T, String>) -> Self {
        match result {
            Ok(value) => Self::Ready(value),
            Err(error) => Self::Failed(error),
        }
    }
}

/// What the UI asks the backend to do about the account.
#[derive(Debug)]
pub enum AccountRequest {
    LoadDetails,
    LoadSettings,
    SaveSettings(Patch),
    LoadApps,
    RevokeApp(Snowflake),
    LoadConnections,
    LoadSessions,
    LogoutOtherSessions,
    LoadNotificationCenter,
    SetDisplayName(String),
    EditProfile { bio: String, pronouns: String },
    SetAvatar(Option<String>),
    SetBanner(Option<String>),
}

/// The answers.
#[derive(Debug)]
pub enum AccountEvent {
    Details(Result<AccountDetails, String>),
    Settings(Result<AccountSettings, String>),
    Apps(Result<Vec<AuthorizedApp>, String>),
    Connections(Result<Vec<Connection>, String>),
    Sessions(Result<Vec<LoginSession>, String>),
    NotificationCenter(Result<Vec<serde_json::Value>, String>),
}

#[derive(Default)]
pub struct AccountState {
    pub details: Load<AccountDetails>,
    pub settings: Load<AccountSettings>,
    pub apps: Load<Vec<AuthorizedApp>>,
    pub connections: Load<Vec<Connection>>,
    pub sessions: Load<Vec<LoginSession>>,
    pub notification_center: Load<Vec<serde_json::Value>>,
}

impl AccountState {
    /// A preference, or Discord's default while settings haven't loaded.
    pub fn pref(&self, key: Key) -> Value {
        match &self.settings {
            Load::Ready(settings) => settings.get(key),
            _ => AccountSettings::default().get(key),
        }
    }

    pub fn apply(&mut self, event: AccountEvent) {
        match event {
            AccountEvent::Details(r) => self.details = Load::from_result(r),
            AccountEvent::Settings(r) => self.settings = Load::from_result(r),
            AccountEvent::Apps(r) => self.apps = Load::from_result(r),
            AccountEvent::Connections(r) => self.connections = Load::from_result(r),
            AccountEvent::Sessions(r) => self.sessions = Load::from_result(r),
            AccountEvent::NotificationCenter(r) => self.notification_center = Load::from_result(r),
        }
    }

    /// Our own custom status, if one is set and has not run out.
    pub fn custom_status(&self, now_ms: u64) -> Option<CustomStatus> {
        self.settings.ready()?.custom_status(now_ms)
    }

    /// Sets or clears the custom status here at once and returns the patch
    /// to send. `None` until the preferences have loaded.
    pub fn change_custom_status(&mut self, status: Option<&CustomStatus>, now_ms: u64) -> Option<Patch> {
        let Load::Ready(settings) = &mut self.settings else {
            return None;
        };
        settings.set_custom_status(status, now_ms).ok()
    }

    /// Applies preference changes here at once and returns the patch to
    /// send. `None` when the preferences haven't loaded, because a patch
    /// built without them could wipe what the server holds.
    pub fn change(&mut self, changes: &[(Key, Value)]) -> Option<Patch> {
        let Load::Ready(settings) = &mut self.settings else {
            return None;
        };
        let patch = settings.patch(changes).ok()?;
        settings.apply(changes).ok()?;
        Some(patch)
    }
}
