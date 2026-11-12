//! The boundary between the views and whatever talks to Discord.
//!
//! The UI only knows these two message types and a pair of channels. The
//! binary supplies the other end, which keeps tokio and the HTTP stack out
//! of the view code.

use std::path::PathBuf;

use async_channel::{Receiver, Sender};
use oxicord_gateway::{ClientStatus, GatewayEvent};
use crate::{AccountEvent, AccountRequest, AdminEvent, AdminRequest, Settings};
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
        /// False to answer without pinging the author of `reply_to`.
        ping_reply: bool,
        /// Local files to attach, each paired with whether to send it as a
        /// spoiler (the backend prefixes the filename with `SPOILER_`).
        /// Reading the bytes is the backend's job.
        files: Vec<(PathBuf, bool, String)>,
        stickers: Vec<Snowflake>,
        /// A poll to attach, as (question, answers, duration_hours, multiselect).
        poll: Option<(String, Vec<String>, u32, bool)>,
    },
    /// Sends a copy of `message` to each of `targets`, then `note` after it.
    ForwardMessage {
        from: Snowflake,
        guild: Option<Snowflake>,
        message: Snowflake,
        targets: Vec<Snowflake>,
        note: String,
    },
    /// Who reacted with `emoji`. The answer arrives as [`Event::Reactors`].
    LoadReactors {
        channel: Snowflake,
        message: Snowflake,
        emoji: oxicord_model::Emoji,
        count: u32,
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
    /// Mark everything from `message` on as unread. `message` is the one just
    /// before the first that should count as new.
    MarkUnread {
        channel: Snowflake,
        message: Snowflake,
    },
    /// Write the preferences to disk.
    SaveSettings(Settings),
    /// The pinned messages of a channel. The answer arrives as [`Event::Pins`].
    LoadPins(Snowflake),
    /// Recent messages that mention us. The answer arrives as [`Event::Mentions`].
    LoadMentions,
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
        /// What was typed, echoed back so answers can be matched to it.
        raw: String,
        query: oxicord_model::SearchQuery,
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
        /// Sent with the request; the gateway echoes it in the interaction
        /// events so the click can be tracked.
        nonce: String,
    },
    /// Send the answers to a bot's modal. A failure is reported as
    /// [`Event::ModalSubmitFailed`]; the app's answer arrives through the
    /// gateway.
    SubmitModal {
        guild: Option<Snowflake>,
        channel: Snowflake,
        application: Snowflake,
        session_id: String,
        custom_id: String,
        /// Input custom id and what was typed in it.
        answers: Vec<(String, String)>,
        nonce: String,
    },
    /// Set our votes on a poll to exactly these answers.
    VotePoll {
        channel: Snowflake,
        message: Snowflake,
        answers: Vec<u32>,
    },
    /// Publish an announcement message to every channel following this one.
    CrosspostMessage {
        channel: Snowflake,
        message: Snowflake,
    },
    /// End a poll early.
    EndPoll {
        channel: Snowflake,
        message: Snowflake,
    },
    /// Load the people who voted for `answer` on a poll. The answer arrives
    /// as [`Event::PollVoters`].
    LoadPollVoters {
        channel: Snowflake,
        message: Snowflake,
        answer: u32,
    },
    /// Make an announcement channel's messages arrive in another channel.
    FollowChannel {
        source: Snowflake,
        target: Snowflake,
    },
    /// Accept a message request. The row hides at once; the gateway
    /// CHANNEL_UPDATE clears the flag behind it.
    AcceptMessageRequest { channel: Snowflake },
    /// Ignore a message request. The server closes the conversation and
    /// answers with a CHANNEL_DELETE.
    IgnoreMessageRequest { channel: Snowflake },
    /// Change notification settings for a server (see `menus::settings_body`).
    PatchGuildSettings {
        guild: Snowflake,
        body: serde_json::Value,
    },
    /// Mark channels read, each up to its newest message.
    AckBulk(Vec<(Snowflake, Snowflake)>),
    /// We opened the pins of a channel.
    AckPins(Snowflake),
    LeaveGuild(Snowflake),
    /// Wear the tag of a server after our name. The gateway's user update
    /// carries the change back, so there is no answer event.
    AdoptServerTag { guild: Snowflake, enabled: bool },
    /// Create a new server with the given name. `GUILD_CREATE` from the
    /// gateway adds it to the rail; this carries no answer event.
    CreateGuild { name: String },
    /// Our nickname in a server. The answer arrives as [`Event::OwnNick`].
    LoadOwnNick(Snowflake),
    /// `None` clears the nickname.
    SetNickname { guild: Snowflake, nick: Option<String> },
    /// Create an invite link. The code arrives as [`Event::InviteCreated`].
    CreateInvite {
        channel: Snowflake,
        max_age: u32,
        max_uses: u32,
    },
    /// Look an invite up. The answer arrives as [`Event::InvitePreview`].
    ResolveInvite(String),
    /// Join the server an invite leads to. The outcome arrives as
    /// [`Event::InviteJoined`].
    JoinInvite {
        code: String,
        session_id: String,
    },
    /// Fetch the slash commands usable in a channel. The answer arrives as
    /// [`Event::Commands`].
    LoadCommands(Snowflake),
    /// Ask a bot for an option's live choices. The answer arrives as
    /// [`Event::Gateway`] with `APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE`;
    /// a failed request is [`Event::AutocompleteFailed`].
    Autocomplete {
        guild: Option<Snowflake>,
        channel: Snowflake,
        session_id: String,
        command: oxicord_model::SlashCommand,
        /// The option being typed in.
        focused: String,
        query: String,
        /// The other options already filled in.
        answers: Vec<(String, u8, serde_json::Value)>,
        nonce: String,
    },
    /// Run a slash command. A failure is reported as [`Event::Notice`].
    RunCommand {
        guild: Option<Snowflake>,
        channel: Snowflake,
        session_id: String,
        command: oxicord_model::SlashCommand,
        answers: Vec<(String, u8, serde_json::Value)>,
    },
    /// Look up people we only know by id. The answer arrives as
    /// [`Event::Users`].
    FetchUsers(Vec<Snowflake>),
    /// Load GIFs for the picker: the trending ones, or a search. The answer
    /// arrives as [`Event::Gifs`].
    LoadGifs(Option<String>),
    /// Load the messages around one, for jumping to it. The answer arrives
    /// as [`Event::Around`].
    LoadAround {
        channel: Snowflake,
        message: Snowflake,
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
    /// Anything about managing a server: invites, bans, roles and so on.
    Admin(AdminRequest),
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
    /// Look up people of a server by id through the gateway.
    RequestMembers {
        guild: Snowflake,
        ids: Vec<Snowflake>,
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
    /// Start a group conversation with these people. Answers like `OpenDm`.
    OpenGroupDm(Vec<Snowflake>),
    /// Add friends to a group conversation.
    AddRecipients { channel: Snowflake, users: Vec<Snowflake> },
    /// Remove a conversation from the list, or leave the group.
    CloseDm(Snowflake),
    /// Accept a friend request. The new state arrives through the gateway.
    AcceptFriend(Snowflake),
    /// Unfriend, decline or cancel, depending on what the tie is now.
    RemoveRelationship(Snowflake),
    BlockUser(Snowflake),
    /// Remove a member from a server we moderate. The gateway drops them from
    /// the member list. Requires the `Kick Members` permission.
    KickMember { guild: Snowflake, user: Snowflake },
    /// Ban a member, optionally deleting their messages from the last few days.
    /// Requires the `Ban Members` permission.
    BanMember {
        guild: Snowflake,
        user: Snowflake,
        delete_message_days: u32,
    },
    /// Time a member out until an RFC3339 instant, or lift it with `None`.
    /// Requires the `Timeout Members` permission.
    TimeoutMember {
        guild: Snowflake,
        user: Snowflake,
        until: Option<String>,
    },
    /// Replace a member's roles. Requires the `Manage Roles` permission.
    SetMemberRoles {
        guild: Snowflake,
        user: Snowflake,
        roles: Vec<Snowflake>,
    },
    /// Send a friend request to this username. The outcome arrives as
    /// [`Event::FriendRequest`].
    AddFriendByName(String),
    /// Fetch the details behind a profile card. The answer arrives as
    /// [`Event::Profile`].
    LoadProfile {
        user: Snowflake,
        guild: Option<Snowflake>,
    },
    /// Save the private note we keep about a user. A failure arrives as an
    /// [`Event::Notice`].
    SetUserNote {
        user: Snowflake,
        note: String,
    },
    /// Set the personal nickname for a friend, or clear it with `None`. A
    /// failure arrives as an [`Event::Notice`].
    SetFriendNick {
        user: Snowflake,
        nick: Option<String>,
    },
    /// Bookmark a message. The server records it under saved messages.
    BookmarkMessage {
        channel: Snowflake,
        message: Snowflake,
    },
    /// Remove a bookmarked message.
    UnbookmarkMessage {
        channel: Snowflake,
        message: Snowflake,
    },
    /// Load our saved/bookmarked messages. The answer arrives as
    /// [`Event::SavedMessages`].
    LoadSavedMessages,
    /// Load our scheduled messages. The answer arrives as
    /// [`Event::ScheduledMessages`].
    LoadScheduledMessages,
    /// The server's welcome screen. The answer arrives as
    /// [`Event::WelcomeScreen`].
    LoadWelcomeScreen {
        guild: Snowflake,
    },
    /// The server's onboarding questions. The answer arrives as
    /// [`Event::Onboarding`].
    LoadOnboarding {
        guild: Snowflake,
    },
    /// How many people are interested in an event. The answer arrives as
    /// [`Event::EventCount`].
    LoadEventCount {
        guild: Snowflake,
        event: Snowflake,
    },
    /// The events in a server that we are interested in. The answer arrives
    /// as [`Event::EventRsvps`].
    LoadEventRsvps {
        guild: Snowflake,
    },
    /// Mark us interested in an event or take it back. A failure arrives as
    /// [`Event::EventInterestFailed`].
    SetEventInterest {
        guild: Snowflake,
        event: Snowflake,
        interested: bool,
    },
    /// Schedule a new event. The gateway's `GUILD_SCHEDULED_EVENT_CREATE`
    /// adds it to the server; a refusal arrives as a notice.
    CreateEvent {
        guild: Snowflake,
        event: oxicord_model::NewEvent,
    },
    /// Store the options picked in the Server Guide. The answer arrives as
    /// [`Event::OnboardingSaved`].
    SaveOnboarding {
        guild: Snowflake,
        submission: oxicord_model::OnboardingSubmission,
    },
    /// Join a voice channel (a private call when `guild` is `None`), or move
    /// to it from another. How it goes is reported as [`Event::Voice`].
    JoinVoice {
        guild: Option<Snowflake>,
        channel: Snowflake,
    },
    /// Leave the call we are in.
    LeaveVoice,
    /// Our mute and deafen switches, which also decide whether we hear anyone.
    SetVoiceFlags { mute: bool, deafen: bool },
    /// The microphone and speakers to use, by name. `None` is the default.
    SetAudioDevices { input: Option<String>, output: Option<String> },
    /// The Input Volume setting, 0 to 100 percent of the microphone.
    SetInputVolume(u8),
    /// Whether the microphone is cleaned of background noise.
    SetNoiseSuppression(bool),
    /// The key that opens the microphone while held, or none to talk by voice
    /// activity.
    SetPushToTalk(Option<String>),
    /// How loud one person is played to us, 0 to 200 percent.
    SetUserVolume { user: Snowflake, percent: u16 },
}

/// How the call we are in is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceNotice {
    /// Keys are agreed and the call is live.
    Connected,
    /// Part of the audio does not work, like no speakers or no microphone.
    /// The text is a sentence for the user.
    Warning(String),
    Speaking { user: Snowflake, speaking: bool },
    /// Whether the call is end-to-end encrypted, and the code two people can
    /// compare to confirm nobody is in the middle.
    Encryption { active: bool, code: Option<String> },
    /// The call is over. A reason means it was not our own doing.
    Left(Option<String>),
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
    Admin(AdminEvent),
    Pins {
        channel: Snowflake,
        result: Result<Vec<Message>, String>,
    },
    Mentions(Result<Vec<Message>, String>),
    Search {
        raw: String,
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
    Around {
        channel: Snowflake,
        message: Snowflake,
        result: Result<Vec<Message>, String>,
    },
    Gifs {
        query: Option<String>,
        result: Result<Vec<oxicord_model::Gif>, String>,
    },
    Users(Vec<User>),
    OwnNick { guild: Snowflake, nick: Option<String> },
    /// The first few people to react, for the tooltip on a reaction.
    Reactors {
        message: Snowflake,
        key: String,
        count: u32,
        names: Vec<String>,
    },
    Commands {
        channel: Snowflake,
        result: Result<oxicord_model::CommandIndex, String>,
    },
    /// The submit request never reached the app; the modal can be retried.
    ModalSubmitFailed { nonce: String },
    /// An autocomplete request never reached the app.
    AutocompleteFailed { nonce: String },
    InvitePreview(Result<oxicord_model::InvitePreview, String>),
    InviteCreated(Result<String, String>),
    InviteJoined(Result<(), String>),
    /// Whether a friend request sent by name went through.
    FriendRequest { username: String, result: Result<(), String> },
    ForumPreviews(Vec<Message>),
    Profile {
        user: Snowflake,
        result: Result<Profile, String>,
    },
    PollVoters {
        message: Snowflake,
        answer: u32,
        users: Vec<User>,
    },
    SavedMessages(Result<Vec<oxicord_model::SavedMessage>, String>),
    ScheduledMessages(Result<Vec<Message>, String>),
    WelcomeScreen {
        guild: Snowflake,
        result: Result<oxicord_model::WelcomeScreen, String>,
    },
    Onboarding {
        guild: Snowflake,
        result: Result<oxicord_model::Onboarding, String>,
    },
    OnboardingSaved {
        guild: Snowflake,
        result: Result<oxicord_model::OnboardingUpdate, String>,
    },
    EventCount {
        event: Snowflake,
        result: Result<u64, String>,
    },
    EventRsvps {
        guild: Snowflake,
        result: Result<Vec<Snowflake>, String>,
    },
    EventInterestFailed {
        event: Snowflake,
        interested: bool,
    },
    Voice(VoiceNotice),
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
