//! Connects to the live gateway and prints what arrives.
//!
//!     DISCORD_TOKEN=... cargo run -p oxicord-gateway --example gateway_dump
//!
//! Without a token it still proves TLS, zlib-stream and the auth rejection
//! path work, because Discord answers an invalid token with close code 4004.

use oxicord_gateway::{ConnectionState, GatewayConfig, GatewayEvent, spawn};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let token = std::env::var("DISCORD_TOKEN").unwrap_or_else(|_| "invalid-token".into());
    let (handle, mut events) = spawn(GatewayConfig::new(token));

    while let Some(event) = events.recv().await {
        match event {
            GatewayEvent::State(state) => {
                println!("state: {state:?}");
                if matches!(state, ConnectionState::AuthFailed | ConnectionState::Closed(_)) {
                    break;
                }
            }
            GatewayEvent::Dispatch(event) => {
                let text = format!("{event:?}");
                println!("event: {}", text.chars().take(160).collect::<String>());
            }
        }
    }
    handle.shutdown();
}
