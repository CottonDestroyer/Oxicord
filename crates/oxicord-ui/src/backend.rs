//! The boundary between the views and whatever talks to Discord.
//!
//! The UI only knows these two message types and a pair of channels. The
//! binary supplies the other end, which keeps tokio and the HTTP stack out
//! of the view code.

use async_channel::{Receiver, Sender};
use oxicord_gateway::GatewayEvent;
use oxicord_model::{Message, Snowflake, User};

/// Things the UI asks for.
#[derive(Debug)]
pub enum Command {
    /// Sign in with a token the user already has.
    Login(String),
    /// Open Discord's own login page in a temporary window.
    BrowserLogin,
    Logout,
    /// Download a picture. The answer arrives as [`Event::Image`].
    FetchImage(String),
    /// Load a page of messages, the newest ones when `before` is `None`.
    /// The answer arrives as [`Event::History`].
    LoadHistory {
        channel: Snowflake,
        before: Option<Snowflake>,
    },
    /// `guild` is `None` in a DM. A failure comes back as [`Event::Notice`],
    /// success arrives through the gateway like any other message.
    SendMessage {
        channel: Snowflake,
        guild: Option<Snowflake>,
        content: String,
        reply_to: Option<Snowflake>,
    },
    EditMessage {
        channel: Snowflake,
        message: Snowflake,
        content: String,
    },
    DeleteMessage {
        channel: Snowflake,
        message: Snowflake,
    },
}

/// Things the backend reports.
#[derive(Debug)]
pub enum Event {
    Authenticated(User),
    LoginFailed(String),
    LoggedOut,
    Gateway(GatewayEvent),
    /// `None` when the download failed.
    Image { url: String, bytes: Option<Vec<u8>> },
    /// Something went wrong that the user should hear about.
    Notice(String),
    /// A page of history, newest first. `requested` is the page size that
    /// was asked for.
    History {
        channel: Snowflake,
        result: Result<Vec<Message>, String>,
        requested: usize,
    },
}

#[derive(Clone)]
pub struct Backend {
    pub commands: Sender<Command>,
    pub events: Receiver<Event>,
}

impl Backend {
    /// The command channel is unbounded, so this only fails once the
    /// backend is gone, in which case there is nobody left to tell.
    pub fn send(&self, command: Command) {
        let _ = self.commands.try_send(command);
    }
}
