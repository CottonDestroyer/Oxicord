//! The boundary between the views and whatever talks to Discord.
//!
//! The UI only knows these two message types and a pair of channels. The
//! binary supplies the other end, which keeps tokio and the HTTP stack out
//! of the view code.

use std::path::PathBuf;

use async_channel::{Receiver, Sender};
use oxicord_gateway::{ClientStatus, GatewayEvent};
use crate::{AccountEvent, AccountRequest, Settings};
use oxicord_model::{Channel, Emoji, Message, Profile, SearchPage, Snowflake, ThreadPage, User};

/// Things the UI asks for.
#[derive(Debug)]
pub enum Command {
    /// Sign in with a token the user already has.
    Login(String),
    /// Open Discord's own login page in a temporary window.
    BrowserLogin,
    Logout,
    /// Change the status shown to others.
    SetStatus(ClientStatus),
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
        /// Local files to attach. Reading them is the backend's job.
        files: Vec<PathBuf>,
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
    /// Mark everything up to `message` as read.
    AckMessage {
        channel: Snowflake,
        message: Snowflake,
    },
    /// Write the preferences to disk.
    SaveSettings(Settings),
    /// The pinned messages of a channel. The answer arrives as [`Event::Pins`].
    LoadPins(Snowflake),
    /// Pin or unpin a message. The refreshed pins arrive as [`Event::Pins`].
    SetPinned {
        channel: Snowflake,
        message: Snowflake,
        pinned: bool,
    },
    /// Search messages in a server, or in a conversation when `guild` is
    /// `None`. The answer arrives as [`Event::Search`].
    Search {
        guild: Option<Snowflake>,
        channel: Snowflake,
        query: String,
        offset: u32,
    },
    /// Load a page of a channel's threads. The answer arrives as
    /// [`Event::Threads`].
    LoadThreads {
        channel: Snowflake,
        archived: bool,
        offset: u32,
    },
    /// Fetch the opening messages of forum posts for the list. The answer
    /// arrives as [`Event::ForumPreviews`].
    LoadForumPreviews {
        forum: Snowflake,
        posts: Vec<Snowflake>,
    },
    /// Start a forum post. The answer arrives as [`Event::ThreadCreated`].
    CreateForumPost {
        forum: Snowflake,
        title: String,
        content: String,
        tags: Vec<Snowflake>,
    },
    /// Press a bot's button or pick from its menu. Failures are reported as
    /// [`Event::Notice`], the app's answer arrives through the gateway.
    ClickComponent {
        guild: Option<Snowflake>,
        channel: Snowflake,
        message: Snowflake,
        message_flags: u64,
        application: Snowflake,
        session_id: String,
        component_type: u8,
        custom_id: String,
        values: Vec<String>,
    },
    /// Set our votes on a poll to exactly these answers.
    VotePoll {
        channel: Snowflake,
        message: Snowflake,
        answers: Vec<u32>,
    },
    JoinThread(Snowflake),
    LeaveThread(Snowflake),
    /// Start a thread on a message. The answer arrives as
    /// [`Event::ThreadCreated`].
    CreateThread {
        channel: Snowflake,
        message: Snowflake,
        name: String,
    },
    /// Anything about the account: details, preferences, linked apps.
    Account(AccountRequest),
    /// Raise a desktop notification.
    Notify {
        title: String,
        body: String,
    },
    /// Show the "is typing" indicator to others for a few seconds.
    Typing(Snowflake),
    /// Add (or with `add` false, remove) our reaction. The new count arrives
    /// through the gateway.
    /// Show the member list of `channel` in `guild`. Updates arrive through
    /// the gateway.
    SubscribeMembers {
        guild: Snowflake,
        channel: Snowflake,
    },
    React {
        channel: Snowflake,
        message: Snowflake,
        emoji: Emoji,
        add: bool,
    },
    /// Open a conversation with `user`, or find the one that exists. The
    /// answer arrives as [`Event::DmOpened`].
    OpenDm(Snowflake),
    /// Accept a friend request. The new state arrives through the gateway.
    AcceptFriend(Snowflake),
    /// Unfriend, decline or cancel, depending on what the tie is now.
    RemoveRelationship(Snowflake),
    /// Fetch the details behind a profile card. The answer arrives as
    /// [`Event::Profile`].
    LoadProfile {
        user: Snowflake,
        guild: Option<Snowflake>,
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
    DmOpened(Result<Channel, String>),
    Account(AccountEvent),
    Pins {
        channel: Snowflake,
        result: Result<Vec<Message>, String>,
    },
    Search {
        query: String,
        offset: u32,
        result: Result<SearchPage, String>,
    },
    Threads {
        channel: Snowflake,
        archived: bool,
        offset: u32,
        result: Result<ThreadPage, String>,
    },
    ThreadCreated(Result<Channel, String>),
    ForumPreviews(Vec<Message>),
    Profile {
        user: Snowflake,
        result: Result<Profile, String>,
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
