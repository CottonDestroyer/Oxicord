//! Runs the network side on its own thread with a tokio runtime, because
//! gpui has its own executor and reqwest and the gateway need tokio.

use std::path::PathBuf;

use async_channel::{Receiver, Sender};
use oxicord_api::{
    ApiClient, ApiError, Credentials,
    routes::{AllowedMentions, CommandRun, ComponentClick, IndexOutcome, MessagePage, ReplyTarget, SearchOutcome, SearchScope, SendMessage, Upload},
    super_props::SuperProperties,
};
use oxicord_browser_login::{Capture, Identity, Outcome};
use oxicord_media::Fetcher;
use oxicord_gateway::{ConnectionState, GatewayConfig, GatewayEvent, GatewayHandle};
use oxicord_model::{SearchPage, Snowflake, User, gateway::DispatchEvent};
use oxicord_ui::{AccountEvent, AccountRequest, AdminEvent, AdminRequest, Backend, Command, Event};

use crate::credentials;

pub fn start() -> Backend {
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();

    // The sign-in window runs on a thread of its own and reports back here.
    let (signin_tx, signin_rx) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("oxicord-net".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("failed to start the network runtime");
            runtime.block_on(run(command_rx, signin_tx, signin_rx, event_tx));
        })
        .expect("failed to spawn the network thread");

    Backend {
        commands: command_tx,
        events: event_rx,
    }
}

async fn run(
    commands: Receiver<Command>,
    signins: Sender<Capture>,
    signed_in: Receiver<Capture>,
    events: Sender<Event>,
) {
    let mut live: Option<Live> = None;
    let mut browser_open = false;
    let fetcher = Fetcher::new().expect("failed to build the image fetcher");

    loop {
        tokio::select! {
            command = commands.recv() => {
                let Ok(command) = command else { break };
                match command {
                    Command::Login(token) => {
                        browser_open = false;
                        replace(&mut live, begin_login(token, None, &events).await);
                    }
                    Command::BrowserLogin => {
                        if !browser_open {
                            browser_open = true;
                            open_browser(signins.clone(), events.clone());
                        }
                    }
                    Command::FetchImage(url) => fetch_image(&fetcher, url, events.clone()),
                    Command::Notify { title, body } => show_notification(title, body),
                    Command::SaveSettings(settings) => {
                        tokio::task::spawn_blocking(move || crate::settings::save(&settings));
                    }
                    Command::Logout => {
                        browser_open = false;
                        replace(&mut live, None);
                        credentials::clear();
                        let _ = events.send(Event::LoggedOut).await;
                    }
                    other => {
                        if let Some(live) = &live {
                            act(live, other, &events);
                        }
                    }
                }
            }
            capture = signed_in.recv() => {
                let Ok(capture) = capture else { break };
                browser_open = false;
                let started = begin_login(capture.token, capture.identity, &events).await;
                replace(&mut live, started);
            }
        }
    }
}

/// Downloads a picture in the background and reports it back. A failure is
/// reported too, so the UI stops waiting and shows its placeholder.
fn fetch_image(fetcher: &Fetcher, url: String, events: Sender<Event>) {
    let fetcher = fetcher.clone();
    tokio::spawn(async move {
        let bytes = match fetcher.fetch(&url).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                tracing::info!(%err, host = url.split('/').nth(2).unwrap_or_default(), "image fetch failed");
                None
            }
        };
        let _ = events.send(Event::Image { url, bytes }).await;
    });
}

/// Everything that exists only while signed in.
struct Live {
    gateway: GatewayHandle,
    api: ApiClient,
}

/// Swaps in a new session, closing the previous gateway connection.
fn replace(slot: &mut Option<Live>, new: Option<Live>) {
    if let Some(old) = std::mem::replace(slot, new) {
        old.gateway.shutdown();
    }
}

const HISTORY_PAGE: u8 = 50;

/// Runs a request on the API in the background and reports the outcome.
fn act(live: &Live, command: Command, events: &Sender<Event>) {
    let (api, events) = (live.api.clone(), events.clone());

    match command {
        Command::LoadHistory { channel, before } => {
            tokio::spawn(async move {
                let result = load_history(&api, channel, before).await;
                let requested = usize::from(HISTORY_PAGE);
                let _ = events.send(Event::History { channel, result, requested }).await;
            });
        }
        Command::SendMessage {
            channel,
            guild,
            content,
            reply_to,
            ping_reply,
            files,
            stickers,
        } => {
            tokio::spawn(async move {
                let message = SendMessage {
                    content,
                    nonce: Some(nonce()),
                    message_reference: reply_to.map(|message_id| ReplyTarget {
                        message_id,
                        channel_id: channel,
                        guild_id: guild,
                        kind: None,
                    }),
                    sticker_ids: stickers.iter().map(ToString::to_string).collect(),
                    allowed_mentions: (reply_to.is_some() && !ping_reply).then(AllowedMentions::without_reply_ping),
                };
                let sent = if files.is_empty() {
                    api.send_message(channel, &message).await
                } else {
                    match read_files(&files).await {
                        Ok(uploads) => api.send_message_with_files(channel, &message, &uploads).await,
                        Err(problem) => {
                            let _ = events.send(Event::Notice(problem)).await;
                            return;
                        }
                    }
                };
                if let Err(err) = sent {
                    notify(&events, "send your message", &err).await;
                }
            });
        }
        Command::ForwardMessage { from, guild, message, targets, note } => {
            for target in targets {
                let (api, events, note) = (api.clone(), events.clone(), note.clone());
                tokio::spawn(async move {
                    let forward = SendMessage {
                        nonce: Some(nonce()),
                        message_reference: Some(ReplyTarget {
                            message_id: message,
                            channel_id: from,
                            guild_id: guild,
                            kind: Some(1),
                        }),
                        ..SendMessage::default()
                    };
                    if let Err(err) = api.send_message(target, &forward).await {
                        notify(&events, "forward that message", &err).await;
                        return;
                    }
                    if !note.is_empty() {
                        let text = SendMessage { content: note, nonce: Some(nonce()), ..SendMessage::default() };
                        if let Err(err) = api.send_message(target, &text).await {
                            notify(&events, "send your note", &err).await;
                        }
                    }
                });
            }
        }
        Command::LoadReactors { channel, message, emoji, count } => {
            tokio::spawn(async move {
                if let Ok(users) = api.reactors(channel, message, &emoji, 3).await {
                    let key = emoji.key();
                    let names = users.iter().map(|u| u.display_name().to_owned()).collect();
                    let _ = events.send(Event::Reactors { message, key, count, names }).await;
                }
            });
        }
        Command::EditMessage {
            channel,
            message,
            content,
        } => {
            tokio::spawn(async move {
                if let Err(err) = api.edit_message(channel, message, &content).await {
                    notify(&events, "edit that message", &err).await;
                }
            });
        }
        Command::DeleteMessage { channel, message } => {
            tokio::spawn(async move {
                if let Err(err) = api.delete_message(channel, message).await {
                    notify(&events, "delete that message", &err).await;
                }
            });
        }
        // Acks and typing pings are best effort, a failure isn't worth
        // interrupting the user over.
        Command::AckMessage { channel, message } => {
            tokio::spawn(async move {
                if let Err(err) = api.ack_message(channel, message).await {
                    tracing::debug!(?err, "ack failed");
                }
            });
        }
        Command::MarkUnread { channel, message } => {
            tokio::spawn(async move {
                if let Err(err) = api.mark_unread(channel, message).await {
                    notify(&events, "mark that unread", &err).await;
                }
            });
        }
        Command::Typing(channel) => {
            tokio::spawn(async move {
                if let Err(err) = api.trigger_typing(channel).await {
                    tracing::debug!(?err, "typing ping failed");
                }
            });
        }
        Command::React {
            channel,
            message,
            emoji,
            add,
        } => {
            tokio::spawn(async move {
                let result = if add {
                    api.add_reaction(channel, message, &emoji).await
                } else {
                    api.remove_reaction(channel, message, &emoji).await
                };
                if let Err(err) = result {
                    notify(&events, "change that reaction", &err).await;
                }
            });
        }
        Command::LoadThreads { channel, archived, offset } => {
            tokio::spawn(async move {
                let result = api.threads(channel, archived, offset).await.map_err(|err| {
                    tracing::warn!(?err, "could not load threads");
                    describe_request(&err)
                });
                let _ = events.send(Event::Threads { channel, archived, offset, result }).await;
            });
        }
        Command::LoadForumPreviews { forum, posts } => {
            tokio::spawn(async move {
                match api.forum_previews(forum, &posts).await {
                    Ok(messages) => {
                        let _ = events.send(Event::ForumPreviews(messages)).await;
                    }
                    Err(err) => tracing::debug!(?err, "could not load post previews"),
                }
            });
        }
        Command::CreateForumPost { forum, title, content, tags } => {
            tokio::spawn(async move {
                let result = api
                    .create_forum_post(forum, &title, &content, &tags)
                    .await
                    .map_err(|err| {
                        tracing::warn!(?err, "could not create a post");
                        describe_request(&err)
                    });
                let _ = events.send(Event::ThreadCreated(result)).await;
            });
        }
        Command::ClickComponent {
            guild,
            channel,
            message,
            message_flags,
            application,
            session_id,
            component_type,
            custom_id,
            values,
        } => {
            tokio::spawn(async move {
                let click = ComponentClick {
                    guild,
                    channel,
                    message,
                    message_flags,
                    application,
                    session_id,
                    component_type,
                    custom_id,
                    values,
                    nonce: nonce(),
                };
                if let Err(err) = api.click_component(&click).await {
                    notify(&events, "use that button", &err).await;
                }
            });
        }
        Command::VotePoll { channel, message, answers } => {
            tokio::spawn(async move {
                if let Err(err) = api.vote_poll(channel, message, &answers).await {
                    notify(&events, "cast that vote", &err).await;
                }
            });
        }
        Command::PatchGuildSettings { guild, body } => {
            tokio::spawn(async move {
                if let Err(err) = api.patch_guild_settings(guild, &body).await {
                    notify(&events, "change those settings", &err).await;
                }
            });
        }
        Command::AckBulk(channels) => {
            tokio::spawn(async move {
                if let Err(err) = api.ack_bulk(&channels).await {
                    tracing::debug!(?err, "bulk ack failed");
                }
            });
        }
        Command::LoadOwnNick(guild) => {
            tokio::spawn(async move {
                if let Ok(nick) = api.own_nickname(guild).await {
                    let _ = events.send(Event::OwnNick { guild, nick }).await;
                }
            });
        }
        Command::SetNickname { guild, nick } => {
            tokio::spawn(async move {
                if let Err(err) = api.set_nickname(guild, nick.as_deref()).await {
                    notify(&events, "change your nickname", &err).await;
                }
            });
        }
        Command::LeaveGuild(guild) => {
            tokio::spawn(async move {
                if let Err(err) = api.leave_guild(guild).await {
                    notify(&events, "leave that server", &err).await;
                }
            });
        }
        Command::CreateInvite { channel, max_age, max_uses } => {
            tokio::spawn(async move {
                let result = api.create_invite(channel, max_age, max_uses).await.map_err(|err| {
                    tracing::warn!(?err, "could not create an invite");
                    describe_request(&err)
                });
                let _ = events.send(Event::InviteCreated(result)).await;
            });
        }
        Command::ResolveInvite(code) => {
            tokio::spawn(async move {
                let result = api.resolve_invite(&code).await.map_err(|err| invite_failure(&err));
                let _ = events.send(Event::InvitePreview(result)).await;
            });
        }
        Command::JoinInvite { code, session_id } => {
            tokio::spawn(async move {
                let result = api.accept_invite(&code, &session_id).await.map_err(|err| invite_failure(&err));
                let _ = events.send(Event::InviteJoined(result)).await;
            });
        }
        Command::LoadCommands(channel) => {
            tokio::spawn(async move {
                let result = load_commands(&api, channel).await;
                let _ = events.send(Event::Commands { channel, result }).await;
            });
        }
        Command::RunCommand { guild, channel, session_id, command, answers } => {
            tokio::spawn(async move {
                let run = CommandRun { guild, channel, session_id, command, answers, nonce: nonce() };
                if let Err(err) = api.run_command(&run).await {
                    notify(&events, "run that command", &err).await;
                }
            });
        }
        Command::FetchUsers(ids) => {
            tokio::spawn(async move {
                let mut found = Vec::new();
                for id in ids {
                    match api.user(id).await {
                        Ok(user) => found.push(user),
                        Err(err) => tracing::debug!(?err, %id, "could not look a user up"),
                    }
                }
                if !found.is_empty() {
                    let _ = events.send(Event::Users(found)).await;
                }
            });
        }
        Command::LoadGifs(query) => {
            tokio::spawn(async move {
                let result = match &query {
                    Some(q) => api.gifs_search(q).await,
                    None => api.gifs_trending().await,
                }
                .map_err(|err| {
                    tracing::debug!(?err, "could not load gifs");
                    describe_request(&err)
                });
                let _ = events.send(Event::Gifs { query, result }).await;
            });
        }
        Command::LoadAround { channel, message } => {
            tokio::spawn(async move {
                let page = MessagePage { around: Some(message), limit: HISTORY_PAGE, ..MessagePage::default() };
                let result = api.messages(channel, page).await.map_err(|err| {
                    tracing::warn!(?err, %channel, "could not load messages around one");
                    describe_request(&err)
                });
                let _ = events.send(Event::Around { channel, message, result }).await;
            });
        }
        Command::JoinThread(thread) => {
            tokio::spawn(async move {
                if let Err(err) = api.join_thread(thread).await {
                    notify(&events, "join that thread", &err).await;
                }
            });
        }
        Command::LeaveThread(thread) => {
            tokio::spawn(async move {
                if let Err(err) = api.leave_thread(thread).await {
                    notify(&events, "leave that thread", &err).await;
                }
            });
        }
        Command::CreateThread { channel, message, name } => {
            tokio::spawn(async move {
                let result = api.create_thread(channel, message, &name).await.map_err(|err| {
                    tracing::warn!(?err, "could not create a thread");
                    describe_request(&err)
                });
                let _ = events.send(Event::ThreadCreated(result)).await;
            });
        }
        Command::LoadMentions => {
            tokio::spawn(async move {
                let result = api.mentions().await.map_err(|err| {
                    tracing::warn!(?err, "could not load mentions");
                    describe_request(&err)
                });
                let _ = events.send(Event::Mentions(result)).await;
            });
        }
        Command::LoadPins(channel) => {
            tokio::spawn(async move { send_pins(&api, channel, &events).await });
        }
        Command::SetPinned { channel, message, pinned } => {
            tokio::spawn(async move {
                if let Err(err) = api.set_pinned(channel, message, pinned).await {
                    notify(&events, "change that pin", &err).await;
                }
                send_pins(&api, channel, &events).await;
            });
        }
        Command::Search { guild, channel, raw, query, offset } => {
            tokio::spawn(async move {
                let scope = guild.map_or(SearchScope::Channel(channel), SearchScope::Guild);
                let result = run_search(&api, scope, &query, offset).await;
                let _ = events.send(Event::Search { raw, offset, result }).await;
            });
        }
        Command::Admin(request) => {
            tokio::spawn(async move { admin_request(&api, request, &events).await });
        }
        Command::Account(request) => {
            tokio::spawn(async move { account_request(&api, request, &events).await });
        }
        Command::OpenDm(user) => {
            tokio::spawn(async move {
                let result = api.open_dm(user).await.map_err(|err| {
                    tracing::warn!(?err, "could not open a conversation");
                    describe_request(&err)
                });
                let _ = events.send(Event::DmOpened(result)).await;
            });
        }
        Command::OpenGroupDm(users) => {
            tokio::spawn(async move {
                let result = api.open_conversation(&users).await.map_err(|err| {
                    tracing::warn!(?err, "could not start a group conversation");
                    describe_request(&err)
                });
                let _ = events.send(Event::DmOpened(result)).await;
            });
        }
        Command::AddRecipients { channel, users } => {
            tokio::spawn(async move {
                for user in users {
                    if let Err(err) = api.add_recipient(channel, user).await {
                        notify(&events, "add that person", &err).await;
                        break;
                    }
                }
            });
        }
        Command::CloseDm(channel) => {
            tokio::spawn(async move {
                if let Err(err) = api.close_channel(channel).await {
                    notify(&events, "close that conversation", &err).await;
                }
            });
        }
        Command::LoadProfile { user, guild } => {
            tokio::spawn(async move {
                let mut result = api.profile(user, guild).await.map_err(|err| {
                    tracing::debug!(?err, "could not load a profile");
                    describe_request(&err)
                });
                if let Ok(profile) = &mut result {
                    if let Ok(note) = api.user_note(user).await {
                        profile.note = note;
                    }
                }
                let _ = events.send(Event::Profile { user, result }).await;
            });
        }
        Command::BlockUser(user) => {
            tokio::spawn(async move {
                if let Err(err) = api.block_user(user).await {
                    notify(&events, "block that person", &err).await;
                }
            });
        }
        Command::AddFriendByName(username) => {
            tokio::spawn(async move {
                let result = api.add_friend_by_name(&username).await.map_err(|err| {
                    tracing::warn!(?err, "friend request failed");
                    friend_request_failure(&err)
                });
                let _ = events.send(Event::FriendRequest { username, result }).await;
            });
        }
        Command::AcceptFriend(user) => {
            tokio::spawn(async move {
                if let Err(err) = api.accept_friend(user).await {
                    notify(&events, "accept that request", &err).await;
                }
            });
        }
        Command::RemoveRelationship(user) => {
            tokio::spawn(async move {
                if let Err(err) = api.remove_relationship(user).await {
                    notify(&events, "remove that person", &err).await;
                }
            });
        }
        Command::SubscribeMembers { guild, channel } => {
            live.gateway.subscribe_members(guild, channel);
        }
        Command::RequestMembers { guild, ids } => live.gateway.request_members(guild, ids),
        Command::SetStatus(status) => live.gateway.set_status(status),
        // Handled before it gets here.
        Command::Login(_)
        | Command::BrowserLogin
        | Command::Logout
        | Command::FetchImage(_)
        | Command::Notify { .. }
        | Command::SaveSettings(_) => {}
    }
}

/// Loads a channel's pins and reports them, errors included.
async fn send_pins(api: &ApiClient, channel: Snowflake, events: &Sender<Event>) {
    let result = api.pins(channel).await.map_err(|err| {
        tracing::warn!(?err, "could not load pins");
        describe_request(&err)
    });
    let _ = events.send(Event::Pins { channel, result }).await;
}

/// A fresh server may still be building its search index. Wait out what
/// Discord asks for a few times before giving up.
async fn run_search(
    api: &ApiClient,
    scope: SearchScope,
    query: &oxicord_model::SearchQuery,
    offset: u32,
) -> Result<SearchPage, String> {
    for _ in 0..4 {
        match api.search_messages(scope, query, offset).await {
            Ok(SearchOutcome::Ready(page)) => return Ok(page),
            Ok(SearchOutcome::Indexing(wait)) => tokio::time::sleep(wait).await,
            Err(err) => {
                tracing::warn!(?err, "search failed");
                return Err(describe_request(&err));
            }
        }
    }
    Err("Discord is still indexing this server. Try again in a moment.".into())
}


/// Runs a server management request. Changes are reported by the gateway,
/// so only the lists are answered here, and a list is read again after a
/// change to it.
async fn admin_request(api: &ApiClient, request: AdminRequest, events: &Sender<Event>) {
    let say = |event| async move {
        let _ = events.send(Event::Admin(event)).await;
    };
    let text = |err: ApiError| {
        tracing::warn!(?err, "server management request failed");
        describe_request(&err)
    };
    // Reports a failed change in the usual notice line.
    let check = |result: Result<(), ApiError>, what: &'static str| async move {
        if let Err(err) = result {
            notify(events, what, &err).await;
        }
    };

    match request {
        AdminRequest::LoadInvites(guild) => say(AdminEvent::Invites(guild, api.guild_invites(guild).await.map_err(text))).await,
        AdminRequest::RevokeInvite { guild, code } => {
            check(api.revoke_invite(&code).await, "revoke that invite").await;
            say(AdminEvent::Invites(guild, api.guild_invites(guild).await.map_err(text))).await;
        }
        AdminRequest::LoadBans(guild) => say(AdminEvent::Bans(guild, api.guild_bans(guild).await.map_err(text))).await,
        AdminRequest::Unban { guild, user } => {
            check(api.unban(guild, user).await, "unban that person").await;
            say(AdminEvent::Bans(guild, api.guild_bans(guild).await.map_err(text))).await;
        }
        AdminRequest::LoadAudit(guild) => say(AdminEvent::Audit(guild, api.audit_log(guild).await.map_err(text))).await,
        AdminRequest::EditGuild { guild, fields } => check(api.edit_guild(guild, &fields).await, "save the server").await,
        AdminRequest::CreateRole { guild, name } => check(api.create_role(guild, &name).await, "create that role").await,
        AdminRequest::EditRole { guild, role, fields } => check(api.edit_role(guild, role, &fields).await, "save that role").await,
        AdminRequest::DeleteRole { guild, role } => check(api.delete_role(guild, role).await, "delete that role").await,
        AdminRequest::RenameEmoji { guild, emoji, name } => {
            check(api.rename_emoji(guild, emoji, &name).await, "rename that emoji").await;
        }
        AdminRequest::DeleteEmoji { guild, emoji } => check(api.delete_emoji(guild, emoji).await, "delete that emoji").await,
        AdminRequest::CreateChannel { guild, name, kind, parent } => {
            check(api.create_channel(guild, &name, kind, parent).await, "create that channel").await;
        }
        AdminRequest::EditChannel { channel, fields } => check(api.edit_channel(channel, &fields).await, "save that channel").await,
        AdminRequest::DeleteChannel(channel) => check(api.close_channel(channel).await, "delete that channel").await,
    }
}
/// Runs one account request and reports what the screens need to refresh.
async fn account_request(api: &ApiClient, request: AccountRequest, events: &Sender<Event>) {
    let say = |event| async move {
        let _ = events.send(Event::Account(event)).await;
    };
    let text = |err: ApiError| {
        tracing::warn!(?err, "account request failed");
        describe_request(&err)
    };

    match request {
        AccountRequest::LoadDetails => say(AccountEvent::Details(api.account_details().await.map_err(text))).await,
        AccountRequest::LoadSettings => say(AccountEvent::Settings(api.account_settings().await.map_err(text))).await,
        AccountRequest::SaveSettings(patch) => {
            if let Err(err) = api.update_account_settings(&patch).await {
                notify(events, "save that setting", &err).await;
            }
            // Read back what the server holds, which also gives us the new
            // data version the next change must be built against.
            say(AccountEvent::Settings(api.account_settings().await.map_err(text))).await;
        }
        AccountRequest::LoadApps => say(AccountEvent::Apps(api.authorized_apps().await.map_err(text))).await,
        AccountRequest::RevokeApp(token) => {
            if let Err(err) = api.revoke_app(token).await {
                notify(events, "remove that app", &err).await;
            }
            say(AccountEvent::Apps(api.authorized_apps().await.map_err(text))).await;
        }
        AccountRequest::LoadConnections => say(AccountEvent::Connections(api.connections().await.map_err(text))).await,
        AccountRequest::LoadSessions => say(AccountEvent::Sessions(api.login_sessions().await.map_err(text))).await,
        AccountRequest::SetDisplayName(name) => {
            if let Err(err) = api.set_display_name(&name).await {
                notify(events, "change your display name", &err).await;
            }
            say(AccountEvent::Details(api.account_details().await.map_err(text))).await;
        }
        AccountRequest::EditProfile { bio, pronouns } => {
            if let Err(err) = api.edit_profile(&bio, &pronouns).await {
                notify(events, "save your profile", &err).await;
            }
            say(AccountEvent::Details(api.account_details().await.map_err(text))).await;
        }
    }
}

/// Toasts block while the system call runs, so keep them off the runtime.
fn show_notification(title: String, body: String) {
    tokio::task::spawn_blocking(move || {
        if let Err(err) = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .show()
        {
            tracing::debug!(%err, "could not show a notification");
        }
    });
}

/// Largest file we will read into memory. Discord's own limit is lower for
/// most accounts and it reports that itself, this only protects our memory.
const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;

async fn read_files(paths: &[PathBuf]) -> Result<Vec<Upload>, String> {
    let mut uploads = Vec::new();
    for path in paths {
        let name = path
            .file_name()
            .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
        let size = tokio::fs::metadata(path)
            .await
            .map_err(|err| format!("Couldn't read {name}: {err}"))?
            .len();
        if size > MAX_UPLOAD_BYTES {
            return Err(format!("{name} is too large to upload from here."));
        }
        let data = tokio::fs::read(path)
            .await
            .map_err(|err| format!("Couldn't read {name}: {err}"))?;
        uploads.push(Upload { filename: name, data: std::sync::Arc::new(data) });
    }
    Ok(uploads)
}

/// Tells the UI that an action failed, in words a person can act on.
async fn notify(events: &Sender<Event>, action: &str, err: &ApiError) {
    tracing::warn!(?err, action, "request failed");
    let _ = events
        .send(Event::Notice(format!("Couldn't {action}. {}", describe_request(err))))
        .await;
}

/// A snowflake-shaped nonce, like the official client sends. Discord uses it
/// to drop a send that was repeated.
fn nonce() -> String {
    const DISCORD_EPOCH_MS: u128 = 1_420_070_400_000;
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let elapsed = u64::try_from(now_ms.saturating_sub(DISCORD_EPOCH_MS)).unwrap_or(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) & 0x3F_FFFF;
    ((elapsed << 22) | sequence).to_string()
}

async fn load_history(
    api: &ApiClient,
    channel: Snowflake,
    before: Option<Snowflake>,
) -> Result<Vec<oxicord_model::Message>, String> {
    let page = MessagePage {
        before,
        limit: HISTORY_PAGE,
        ..MessagePage::default()
    };
    api.messages(channel, page).await.map_err(|err| {
        tracing::warn!(?err, %channel, "could not load history");
        describe_request(&err)
    })
}

/// Loads the commands of a channel. Discord answers 202 while it builds the
/// list, so wait and ask again a few times.
async fn load_commands(
    api: &ApiClient,
    channel: Snowflake,
) -> Result<oxicord_model::CommandIndex, String> {
    for _ in 0..4 {
        match api.command_index(channel).await {
            Ok(IndexOutcome::Ready { commands, apps }) => {
                return Ok(oxicord_model::CommandIndex { commands, apps });
            }
            Ok(IndexOutcome::Pending) => tokio::time::sleep(std::time::Duration::from_secs(3)).await,
            Err(err) => {
                tracing::debug!(?err, "could not load commands");
                return Err(describe_request(&err));
            }
        }
    }
    Err("Discord is still preparing the commands.".into())
}

/// Says why an invite lookup or join failed, in words a person can act on.
fn invite_failure(err: &ApiError) -> String {
    match err {
        ApiError::Status { status: 404, .. } => "That invite is invalid or has expired.".into(),
        ApiError::Status { message, status: 400, .. } if message.to_lowercase().contains("captcha") => {
            "Discord wants a captcha to join this server, which this app can't show. Join from Discord's own app."
                .into()
        }
        ApiError::Status { message, .. } if !message.is_empty() => message.clone(),
        other => describe_request(other),
    }
}

/// Says why a friend request by name didn't go through.
fn friend_request_failure(err: &ApiError) -> String {
    match err {
        ApiError::Status { status: 400, message, .. } if message.to_lowercase().contains("captcha") => {
            "Discord wants a captcha for this request, which this app can't show. Send it from Discord's own app."
                .into()
        }
        ApiError::Status { message, .. } if !message.is_empty() => message.clone(),
        other => describe_request(other),
    }
}

fn describe_request(err: &ApiError) -> String {
    match err {
        ApiError::Status { status: 403, .. } => "You don't have access to that.".into(),
        ApiError::Status { status: 404, .. } => "That no longer exists.".into(),
        other => format!("Request failed: {other}"),
    }
}

/// Opens Discord's login page on a blocking thread, since the window owns
/// that thread's message loop. What it captured goes back to the main loop
/// to be logged in, anything else is reported as a failure.
fn open_browser(signins: Sender<Capture>, events: Sender<Event>) {
    tokio::task::spawn_blocking(move || match oxicord_browser_login::sign_in() {
        Outcome::SignedIn(capture) => {
            tracing::info!(
                shape = %token_shape(&capture.token),
                identity_captured = capture.identity.is_some(),
                "sign-in window produced a login"
            );
            let _ = signins.send_blocking(capture);
        }
        other => {
            tracing::info!(?other, "sign-in window ended without a token");
            let _ = events.send_blocking(Event::LoginFailed(browser_failure(other)));
        }
    });
}

fn browser_failure(outcome: Outcome) -> String {
    match outcome {
        Outcome::Closed => "Sign-in was cancelled.".into(),
        Outcome::TimedOut => "Sign-in timed out after ten minutes.".into(),
        Outcome::Failed(message) => message,
        Outcome::Unsupported => {
            "Signing in through Discord's page isn't available on this platform yet. \
             Paste a token instead."
                .into()
        }
        Outcome::SignedIn(_) => unreachable!("a sign-in is not a failure"),
    }
}

/// Checks the token over REST first, then opens the gateway. The login only
/// counts once READY arrives, which is where the token gets saved.
async fn begin_login(
    token: String,
    identity: Option<Identity>,
    events: &Sender<Event>,
) -> Option<Live> {
    let shape = token_shape(&token);
    let (props, identity) = resolve_identity(&token, identity);
    tracing::info!(
        %shape,
        user_agent = props.user_agent(),
        build_number = props.build_number(),
        from_browser = identity.is_some(),
        "checking token over REST"
    );

    let (api, user) = match validate(&token, &props).await {
        Ok((_, user)) if user.bot => {
            tracing::warn!("REST accepted the token but it belongs to a bot");
            let message = "That is a bot token. Only user account tokens work here.";
            let _ = events.send(Event::LoginFailed(message.into())).await;
            return None;
        }
        Ok(pair) => {
            tracing::info!("REST accepted the token");
            pair
        }
        Err(err) => {
            tracing::warn!(?err, %shape, "REST rejected the token");
            if err.is_unauthorized() {
                forget_if_saved(&token);
            }
            let _ = events.send(Event::LoginFailed(describe(&err))).await;
            return None;
        }
    };
    let gateway = connect(token, props, identity, user, events.clone());
    Some(Live { gateway, api })
}

/// Picks the identity to present with this token: the one the sign-in window
/// reported, else the one we saved for this very token, else the default.
/// Returns the identity actually used, so it can be saved with the token.
fn resolve_identity(token: &str, given: Option<Identity>) -> (SuperProperties, Option<Identity>) {
    let candidate = given.or_else(|| {
        credentials::load()
            .filter(|saved| saved.token == token)
            .and_then(|saved| saved.identity)
    });

    let Some(identity) = candidate else {
        return (SuperProperties::default(), None);
    };
    match SuperProperties::from_browser(
        &identity.user_agent,
        &identity.super_properties,
        identity.locale.as_deref(),
        identity.timezone.as_deref(),
    ) {
        Ok(props) => (props, Some(identity)),
        Err(err) => {
            tracing::warn!(%err, "the captured identity was unusable, using the default");
            (SuperProperties::default(), None)
        }
    }
}

/// Length and layout of a token, for the log. Enough to spot a truncated or
/// wrong value (a stray quote, the wrong header) without revealing any of it.
fn token_shape(token: &str) -> String {
    let parts: Vec<String> = token.split('.').map(|p| p.len().to_string()).collect();
    format!("{} chars, segments [{}]", token.len(), parts.join(", "))
}

async fn validate(
    token: &str,
    props: &SuperProperties,
) -> Result<(ApiClient, User), ApiError> {
    let client = ApiClient::with_props(&Credentials::Token(token.to_owned()), props)?;
    let user = client.current_user().await?;
    Ok((client, user))
}

fn describe(err: &ApiError) -> String {
    if err.is_unauthorized() {
        "Discord rejected that token. Make sure you copied all of it.".into()
    } else if matches!(err, ApiError::BadToken) {
        "That doesn't look like a token. It should be a single line with no spaces.".into()
    } else {
        format!("Couldn't reach Discord: {err}")
    }
}

/// A dead token shouldn't be retried at every launch.
fn forget_if_saved(token: &str) {
    if credentials::load().is_some_and(|saved| saved.token == token) {
        credentials::clear();
    }
}

/// What to do with one gateway event while a login is still waiting on READY.
#[derive(Debug, PartialEq)]
struct Routed {
    send: Vec<RoutedEvent>,
    save_token: bool,
    forget_token: bool,
    stop: bool,
}

#[derive(Debug, PartialEq)]
enum RoutedEvent {
    Authenticated,
    LoginFailed(String),
    Forward,
}

/// The decision logic, separated from the channels so it can be tested.
/// `waiting` is true until READY has been seen.
fn route(waiting: bool, event: &GatewayEvent) -> Routed {
    let plain = |send| Routed {
        send,
        save_token: false,
        forget_token: false,
        stop: false,
    };

    if !waiting {
        return plain(vec![RoutedEvent::Forward]);
    }

    match event {
        GatewayEvent::Dispatch(DispatchEvent::Ready(_)) => Routed {
            send: vec![RoutedEvent::Authenticated, RoutedEvent::Forward],
            save_token: true,
            ..plain(vec![])
        },
        GatewayEvent::State(ConnectionState::AuthFailed) => Routed {
            send: vec![RoutedEvent::LoginFailed(
                "Discord rejected the token while signing in.".into(),
            )],
            forget_token: true,
            stop: true,
            ..plain(vec![])
        },
        GatewayEvent::State(ConnectionState::Closed(reason)) => Routed {
            send: vec![RoutedEvent::LoginFailed(reason.clone())],
            stop: true,
            ..plain(vec![])
        },
        _ => plain(vec![RoutedEvent::Forward]),
    }
}

/// Starts the gateway and forwards what it produces to the UI. Until READY
/// arrives the login is provisional: success saves the token and tells the
/// UI, failure reports back to the login screen.
fn connect(
    token: String,
    props: SuperProperties,
    identity: Option<Identity>,
    user: User,
    events: Sender<Event>,
) -> GatewayHandle {
    let config = GatewayConfig {
        props,
        ..GatewayConfig::new(token.clone())
    };
    let (handle, mut gateway_events) = oxicord_gateway::spawn(config);

    tokio::spawn(async move {
        let mut waiting = true;
        let mut user = Some(user);

        while let Some(event) = gateway_events.recv().await {
            if let GatewayEvent::State(state) = &event {
                tracing::info!(?state, waiting_for_ready = waiting, "gateway state");
            }
            let routed = route(waiting, &event);

            if routed.save_token {
                credentials::save(&token, identity.as_ref());
            }
            if routed.forget_token {
                forget_if_saved(&token);
            }

            let mut event = Some(event);
            for item in routed.send {
                let out = match item {
                    RoutedEvent::Authenticated => user.take().map(Event::Authenticated),
                    RoutedEvent::LoginFailed(message) => Some(Event::LoginFailed(message)),
                    RoutedEvent::Forward => event.take().map(Event::Gateway),
                };
                if let Some(out) = out
                    && events.send(out).await.is_err()
                {
                    return;
                }
            }

            if routed.save_token {
                waiting = false;
            }
            if routed.stop {
                return;
            }
        }
    });
    handle
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ready() -> GatewayEvent {
        let ready = DispatchEvent::parse(
            "READY",
            json!({ "user": { "id": "1", "username": "me" }, "session_id": "s" }),
        )
        .unwrap();
        GatewayEvent::Dispatch(ready)
    }

    #[tokio::test]
    async fn files_are_read_under_their_own_names() {
        let path = std::env::temp_dir().join(format!("oxicord-upload-{}.txt", std::process::id()));
        tokio::fs::write(&path, b"hello").await.unwrap();

        let uploads = read_files(std::slice::from_ref(&path)).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();

        assert_eq!(uploads.len(), 1);
        assert!(uploads[0].filename.starts_with("oxicord-upload-"));
        assert_eq!(uploads[0].data.as_slice(), b"hello");
    }

    #[tokio::test]
    async fn a_missing_file_is_reported_by_name() {
        let err = read_files(&[PathBuf::from("no-such-dir/ghost.bin")]).await.unwrap_err();
        assert!(err.contains("ghost.bin"), "{err}");
    }

    #[test]
    fn nonces_are_unique_and_snowflake_sized() {
        let (a, b) = (nonce(), nonce());
        assert_ne!(a, b);
        // A real snowflake is 18 or 19 digits.
        assert!((17..=19).contains(&a.len()), "{a}");
    }

    #[test]
    fn token_shape_reveals_layout_but_never_content() {
        let shape = token_shape("abcd.efg.hijklm");
        assert_eq!(shape, "15 chars, segments [4, 3, 6]");
        assert!(!shape.contains("abcd") && !shape.contains("hijklm"));
    }

    #[test]
    fn ready_completes_the_login_and_saves_the_token() {
        let routed = route(true, &ready());
        assert!(routed.save_token);
        assert_eq!(routed.send, [RoutedEvent::Authenticated, RoutedEvent::Forward]);
        assert!(!routed.stop);
    }

    #[test]
    fn a_rejected_token_before_ready_fails_the_login_and_is_forgotten() {
        let routed = route(true, &GatewayEvent::State(ConnectionState::AuthFailed));
        assert!(routed.forget_token && routed.stop);
        assert!(matches!(routed.send[0], RoutedEvent::LoginFailed(_)));
        assert!(!routed.save_token);
    }

    #[test]
    fn connecting_states_are_forwarded_while_waiting() {
        let routed = route(true, &GatewayEvent::State(ConnectionState::Connecting));
        assert_eq!(routed.send, [RoutedEvent::Forward]);
        assert!(!routed.save_token && !routed.stop);
    }

    #[test]
    fn a_giving_up_gateway_fails_the_login_with_its_reason() {
        let event = GatewayEvent::State(ConnectionState::Closed("could not read READY".into()));
        let routed = route(true, &event);
        assert_eq!(routed.send, [RoutedEvent::LoginFailed("could not read READY".into())]);
        assert!(routed.stop);
    }

    #[test]
    fn after_login_everything_is_forwarded_untouched() {
        for event in [ready(), GatewayEvent::State(ConnectionState::AuthFailed)] {
            let routed = route(false, &event);
            assert_eq!(routed.send, [RoutedEvent::Forward]);
            assert!(!routed.save_token && !routed.forget_token && !routed.stop);
        }
    }
}
