//! Runs the network side on its own thread with a tokio runtime, because
//! gpui has its own executor and reqwest and the gateway need tokio.

use async_channel::{Receiver, Sender};
use oxicord_api::{
    ApiClient, ApiError, Credentials,
    routes::{MessagePage, ReplyTarget, SendMessage},
    super_props::SuperProperties,
};
use oxicord_browser_login::{Capture, Identity, Outcome};
use oxicord_media::Fetcher;
use oxicord_gateway::{ConnectionState, GatewayConfig, GatewayEvent, GatewayHandle};
use oxicord_model::{Snowflake, User, gateway::DispatchEvent};
use oxicord_ui::{Backend, Command, Event};

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
                tracing::debug!(%err, %url, "image fetch failed");
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
        } => {
            tokio::spawn(async move {
                let message = SendMessage {
                    content,
                    nonce: Some(nonce()),
                    message_reference: reply_to.map(|message_id| ReplyTarget {
                        message_id,
                        channel_id: channel,
                        guild_id: guild,
                    }),
                };
                if let Err(err) = api.send_message(channel, &message).await {
                    notify(&events, "send your message", &err).await;
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
        Command::OpenDm(user) => {
            tokio::spawn(async move {
                let result = api.open_dm(user).await.map_err(|err| {
                    tracing::warn!(?err, "could not open a conversation");
                    describe_request(&err)
                });
                let _ = events.send(Event::DmOpened(result)).await;
            });
        }
        Command::LoadProfile { user, guild } => {
            tokio::spawn(async move {
                let result = api.profile(user, guild).await.map_err(|err| {
                    tracing::debug!(?err, "could not load a profile");
                    describe_request(&err)
                });
                let _ = events.send(Event::Profile { user, result }).await;
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
        Command::SetStatus(status) => live.gateway.set_status(status),
        // Handled before it gets here.
        Command::Login(_) | Command::BrowserLogin | Command::Logout | Command::FetchImage(_) => {}
    }
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
