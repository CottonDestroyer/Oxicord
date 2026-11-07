//! Voice calls: the voice gateway, encrypted UDP audio, end-to-end
//! encryption (DAVE) and playback.
//!
//! [`spawn`] starts a call on the current tokio runtime and hands back a
//! handle plus a stream of [`VoiceEvent`]s. Dropping the handle leaves the
//! call.

mod capture;
mod connection;
mod dave;
mod opus;
mod playback;
mod transport;

pub use capture::input_devices;
pub use playback::output_devices;

use std::fmt;

use tokio::sync::mpsc;

/// What the main gateway told us about the call we are joining.
#[derive(Clone)]
pub struct VoiceParams {
    /// From `VOICE_SERVER_UPDATE`, like `c-ams01-1234.discord.media:443`.
    pub endpoint: String,
    /// The guild, or the channel itself for a private call.
    pub server_id: u64,
    pub channel_id: u64,
    pub user_id: u64,
    /// The main gateway's session id.
    pub session_id: String,
    pub token: String,
    /// The microphone and speakers to use, the system defaults when `None`.
    pub input_device: Option<String>,
    pub output_device: Option<String>,
}

// The token must not land in a log line.
impl fmt::Debug for VoiceParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceParams")
            .field("endpoint", &self.endpoint)
            .field("server_id", &self.server_id)
            .field("channel_id", &self.channel_id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceEvent {
    /// Keys are agreed and audio can flow.
    Connected,
    /// Part of the audio does not work, like no speakers or no microphone.
    /// The text is a sentence for the user.
    Warning(String),
    Speaking { user: u64, speaking: bool },
    /// Whether the call is end-to-end encrypted right now, and the code that
    /// two people can compare to confirm nobody is listening in.
    Encryption { active: bool, code: Option<String> },
    /// The call is over. The code is the websocket close code, if any.
    Closed { code: Option<u16>, reason: String },
}

pub(crate) enum Command {
    SetDeafened(bool),
    SetMuted(bool),
    SetInputDevice(Option<String>),
    SetOutputDevice(Option<String>),
    Leave,
}

#[derive(Clone, Debug)]
pub struct VoiceHandle {
    tx: mpsc::UnboundedSender<Command>,
}

impl VoiceHandle {
    /// Stops playing what others say.
    pub fn set_deafened(&self, deafened: bool) {
        let _ = self.tx.send(Command::SetDeafened(deafened));
    }

    /// Stops sending our microphone. The device is released while muted.
    pub fn set_muted(&self, muted: bool) {
        let _ = self.tx.send(Command::SetMuted(muted));
    }

    /// Records from this microphone, or the default one for `None`.
    pub fn set_input_device(&self, device: Option<String>) {
        let _ = self.tx.send(Command::SetInputDevice(device));
    }

    /// Plays on these speakers, or the default ones for `None`.
    pub fn set_output_device(&self, device: Option<String>) {
        let _ = self.tx.send(Command::SetOutputDevice(device));
    }

    pub fn leave(&self) {
        let _ = self.tx.send(Command::Leave);
    }
}

/// Must be called from inside a tokio runtime.
pub fn spawn(params: VoiceParams) -> (VoiceHandle, mpsc::UnboundedReceiver<VoiceEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    tokio::spawn(connection::run(params, cmd_rx, event_tx));
    (VoiceHandle { tx: cmd_tx }, event_rx)
}
