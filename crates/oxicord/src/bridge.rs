//! Runs the network side on its own thread with a tokio runtime, because
//! gpui has its own executor and reqwest and the gateway need tokio.

use async_channel::{Receiver, Sender};
use oxicord_api::{ApiClient, ApiError, Credentials};
use oxicord_gateway::{GatewayConfig, GatewayHandle};
use oxicord_model::User;
use oxicord_ui::{Backend, Command, Event};

use crate::credentials;

pub fn start() -> Backend {
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();

    std::thread::Builder::new()
        .name("oxicord-net".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("failed to start the network runtime");
            runtime.block_on(run(command_rx, event_tx));
        })
        .expect("failed to spawn the network thread");

    Backend {
        commands: command_tx,
        events: event_rx,
    }
}

async fn run(commands: Receiver<Command>, events: Sender<Event>) {
    let mut gateway: Option<GatewayHandle> = None;

    while let Ok(command) = commands.recv().await {
        match command {
            Command::Login(token) => {
                if let Some(old) = gateway.take() {
                    old.shutdown();
                }
                match validate(&token).await {
                    Ok(user) => {
                        credentials::save(&token);
                        gateway = Some(connect(token, events.clone()));
                        let _ = events.send(Event::Authenticated(user)).await;
                    }
                    Err(message) => {
                        let _ = events.send(Event::LoginFailed(message)).await;
                    }
                }
            }
            Command::Logout => {
                if let Some(old) = gateway.take() {
                    old.shutdown();
                }
                credentials::clear();
                let _ = events.send(Event::LoggedOut).await;
            }
        }
    }
}

/// Checks the token with a cheap authenticated call before we commit to
/// opening a gateway connection with it.
async fn validate(token: &str) -> Result<User, String> {
    let client = ApiClient::new(&Credentials::Token(token.to_owned())).map_err(describe)?;
    client.current_user().await.map_err(describe)
}

fn describe(err: ApiError) -> String {
    if err.is_unauthorized() {
        "Discord rejected that token. Make sure you copied all of it.".into()
    } else if matches!(err, ApiError::BadToken) {
        "That doesn't look like a token. It should be a single line with no spaces.".into()
    } else {
        format!("Couldn't reach Discord: {err}")
    }
}

/// Starts the gateway and forwards everything it produces to the UI.
fn connect(token: String, events: Sender<Event>) -> GatewayHandle {
    let (handle, mut gateway_events) = oxicord_gateway::spawn(GatewayConfig::new(token));

    tokio::spawn(async move {
        while let Some(event) = gateway_events.recv().await {
            if events.send(Event::Gateway(event)).await.is_err() {
                break;
            }
        }
    });
    handle
}
