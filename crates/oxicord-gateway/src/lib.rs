//! Gateway client: one long-lived websocket with heartbeats, resume and
//! reconnect handled in the background.
//!
//! [`spawn`] starts the task on the current tokio runtime and hands back a
//! handle for commands plus a stream of [`GatewayEvent`]s.

mod protocol;
mod session;
mod zlib;

use std::{fmt, time::Duration};

use oxicord_api::super_props::SuperProperties;
use oxicord_model::{Snowflake, gateway::DispatchEvent};
use tokio::sync::mpsc;

pub use protocol::{ClientStatus, DEFAULT_GATEWAY};

use session::{Command, Runtime};

#[derive(Clone)]
pub struct GatewayConfig {
    pub token: String,
    pub props: SuperProperties,
    pub status: ClientStatus,
    /// Base URL without the query string. Overridable so tests can point at
    /// a local server.
    pub url: String,
    /// Delay before the first reconnect attempt. Later attempts double it.
    pub reconnect_base: Duration,
}

impl GatewayConfig {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into().trim().to_owned(),
            props: SuperProperties::default(),
            status: ClientStatus::default(),
            url: DEFAULT_GATEWAY.to_owned(),
            reconnect_base: Duration::from_secs(1),
        }
    }
}

// Left out of derive so the token never lands in a log line.
impl fmt::Debug for GatewayConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayConfig")
            .field("url", &self.url)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
    /// The token was rejected. Retrying won't help, the user has to log in
    /// again.
    AuthFailed,
    /// Gave up for some other reason. The text is meant for the user.
    Closed(String),
}

#[derive(Clone, Debug)]
pub enum GatewayEvent {
    Dispatch(DispatchEvent),
    State(ConnectionState),
}

/// Dropping the handle shuts the connection down.
#[derive(Clone, Debug)]
pub struct GatewayHandle {
    tx: mpsc::UnboundedSender<Command>,
}

impl GatewayHandle {
    pub fn set_status(&self, status: ClientStatus) {
        let _ = self.tx.send(Command::SetStatus(status));
    }

    /// Asks the server for the member list of `channel` in `guild`. It is
    /// asked for again by itself after a reconnect.
    pub fn subscribe_members(&self, guild: Snowflake, channel: Snowflake) {
        let _ = self.tx.send(Command::SubscribeMembers { guild, channel });
    }

    /// Asks the server for these people of `guild`. They come back as a
    /// `GUILD_MEMBERS_CHUNK` event.
    pub fn request_members(&self, guild: Snowflake, ids: Vec<Snowflake>) {
        let _ = self.tx.send(Command::RequestMembers { guild, ids });
    }

    /// Joins `channel` (in `guild`, or a private call when `None`), moves
    /// there from another channel, or leaves voice when `channel` is `None`.
    /// The answers come back as `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE`.
    pub fn set_voice_state(
        &self,
        guild: Option<Snowflake>,
        channel: Option<Snowflake>,
        self_mute: bool,
        self_deaf: bool,
    ) {
        let _ = self.tx.send(Command::SetVoiceState { guild, channel, self_mute, self_deaf });
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

/// Must be called from inside a tokio runtime.
pub fn spawn(config: GatewayConfig) -> (GatewayHandle, mpsc::UnboundedReceiver<GatewayEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    tokio::spawn(Runtime::new(config, cmd_rx, event_tx).run());

    (GatewayHandle { tx: cmd_tx }, event_rx)
}
