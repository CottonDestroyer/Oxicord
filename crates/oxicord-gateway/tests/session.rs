//! Drives the real client against a local fake gateway that speaks
//! zlib-stream, so reconnect behaviour is covered without Discord.

use std::time::Duration;

use flate2::{Compress, Compression, FlushCompress};
use futures_util::{SinkExt, StreamExt};
use oxicord_gateway::{ConnectionState, GatewayConfig, GatewayEvent, spawn};
use oxicord_model::{Snowflake, gateway::DispatchEvent};
use serde_json::{Value, json};
use tokio::{net::{TcpListener, TcpStream}, sync::mpsc::UnboundedReceiver, time::timeout};
use tokio_tungstenite::{
    WebSocketStream, accept_async,
    tungstenite::{
        Message,
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};

const WAIT: Duration = Duration::from_secs(10);

struct FakeSocket {
    ws: WebSocketStream<TcpStream>,
    enc: Compress,
}

impl FakeSocket {
    async fn accept(listener: &TcpListener) -> Self {
        let (tcp, _) = listener.accept().await.unwrap();
        Self {
            ws: accept_async(tcp).await.unwrap(),
            enc: Compress::new(Compression::default(), true),
        }
    }

    async fn send(&mut self, value: Value) {
        let text = value.to_string();
        let mut out = Vec::with_capacity(text.len() + 64);
        self.enc
            .compress_vec(text.as_bytes(), &mut out, FlushCompress::Sync)
            .unwrap();
        self.ws.send(Message::Binary(out.into())).await.unwrap();
    }

    /// Clients send plain JSON text, only the server compresses.
    async fn recv(&mut self) -> Value {
        loop {
            match self.ws.next().await.unwrap().unwrap() {
                Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
                _ => continue,
            }
        }
    }

    async fn hello(&mut self) {
        self.send(json!({ "op": 10, "d": { "heartbeat_interval": 60_000 } }))
            .await;
    }

    async fn ready(&mut self, session: &str, url: &str) {
        self.send(json!({
            "op": 0, "s": 1, "t": "READY",
            "d": {
                "user": { "id": "1", "username": "tester" },
                "session_id": session,
                "resume_gateway_url": url,
                "guilds": [],
                "private_channels": []
            }
        }))
        .await;
    }

    async fn message(&mut self, seq: u64, content: &str) {
        self.send(json!({
            "op": 0, "s": seq, "t": "MESSAGE_CREATE",
            "d": {
                "id": seq.to_string(), "channel_id": "20",
                "author": { "id": "2", "username": "other" },
                "content": content,
                "timestamp": "2024-05-01T12:00:00+00:00"
            }
        }))
        .await;
    }
}

async fn bind() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (listener, url)
}

fn config(url: &str) -> GatewayConfig {
    let mut cfg = GatewayConfig::new("test-token");
    cfg.url = url.to_owned();
    cfg.reconnect_base = Duration::from_millis(10);
    cfg
}

async fn next_event(events: &mut UnboundedReceiver<GatewayEvent>) -> Option<GatewayEvent> {
    timeout(WAIT, events.recv()).await.expect("timed out waiting for an event")
}

/// Skips connection state changes, remembering them.
async fn next_dispatch(
    events: &mut UnboundedReceiver<GatewayEvent>,
    states: &mut Vec<ConnectionState>,
) -> DispatchEvent {
    loop {
        match next_event(events).await.expect("event stream ended") {
            GatewayEvent::State(s) => states.push(s),
            GatewayEvent::Dispatch(d) => return d,
        }
    }
}

#[tokio::test]
async fn resumes_with_session_and_sequence_after_a_dropped_socket() {
    let (listener, url) = bind().await;
    let resume_url = url.clone();

    let server = tokio::spawn(async move {
        let mut first = FakeSocket::accept(&listener).await;
        first.hello().await;
        let identify = first.recv().await;
        first.ready("sess-1", &resume_url).await;
        first.message(2, "before").await;
        drop(first); // no close frame, like a network failure

        let mut second = FakeSocket::accept(&listener).await;
        second.hello().await;
        let resume = second.recv().await;
        second.send(json!({ "op": 0, "s": 3, "t": "RESUMED", "d": {} })).await;
        second.message(4, "after").await;

        // Hold the socket open until the client hangs up.
        while second.ws.next().await.is_some_and(|m| m.is_ok()) {}
        (identify, resume)
    });

    let (handle, mut events) = spawn(config(&url));
    let mut states = Vec::new();

    let DispatchEvent::Ready(ready) = next_dispatch(&mut events, &mut states).await else {
        panic!("expected READY first");
    };
    assert_eq!(ready.session_id, "sess-1");

    let DispatchEvent::MessageCreate(m) = next_dispatch(&mut events, &mut states).await else {
        panic!("expected the first message");
    };
    assert_eq!(m.content, "before");

    assert!(matches!(
        next_dispatch(&mut events, &mut states).await,
        DispatchEvent::Resumed
    ));
    let DispatchEvent::MessageCreate(m) = next_dispatch(&mut events, &mut states).await else {
        panic!("expected the message after resume");
    };
    assert_eq!(m.content, "after");

    assert!(states.contains(&ConnectionState::Connecting));
    assert!(states.contains(&ConnectionState::Connected));
    assert!(states.contains(&ConnectionState::Reconnecting { attempt: 1 }));

    handle.shutdown();
    let (identify, resume) = timeout(WAIT, server).await.unwrap().unwrap();
    assert_eq!(identify["op"], 2);
    assert_eq!(identify["d"]["token"], "test-token");
    assert_eq!(resume["op"], 6);
    assert_eq!(resume["d"]["session_id"], "sess-1");
    assert_eq!(resume["d"]["seq"], 2);
}

#[tokio::test]
async fn asks_for_the_member_list_once_ready_and_again_after_a_reconnect() {
    let (listener, url) = bind().await;
    let resume_url = url.clone();

    let server = tokio::spawn(async move {
        let mut first = FakeSocket::accept(&listener).await;
        first.hello().await;
        first.recv().await; // identify
        first.ready("sess-1", &resume_url).await;
        let subscribe = first.recv().await;
        let ranges = first.recv().await;
        drop(first);

        let mut second = FakeSocket::accept(&listener).await;
        second.hello().await;
        second.recv().await; // resume
        second.send(json!({ "op": 0, "s": 3, "t": "RESUMED", "d": {} })).await;
        let resubscribe = second.recv().await;
        let reranges = second.recv().await;
        while second.ws.next().await.is_some_and(|m| m.is_ok()) {}
        (subscribe, ranges, resubscribe, reranges)
    });

    let (handle, mut events) = spawn(config(&url));
    // Asked for before the session exists. It must wait for READY.
    handle.subscribe_members(Snowflake(5), Snowflake(9));

    let mut states = Vec::new();
    loop {
        if matches!(next_dispatch(&mut events, &mut states).await, DispatchEvent::Resumed) {
            break;
        }
    }
    handle.shutdown();

    let (subscribe, ranges, resubscribe, reranges) =
        timeout(WAIT, server).await.unwrap().unwrap();
    for frame in [&subscribe, &ranges, &resubscribe, &reranges] {
        assert_eq!(frame["op"], 37);
    }
    // Subscribe first (no channels), then the channel ranges.
    assert_eq!(subscribe["d"]["subscriptions"]["5"]["channels"], json!({}));
    assert_eq!(ranges["d"]["subscriptions"]["5"]["channels"]["9"], json!([[0, 99]]));
    // A new socket starts from nothing, so it does both steps again.
    assert_eq!(resubscribe["d"]["subscriptions"]["5"]["channels"], json!({}));
    assert_eq!(reranges["d"]["subscriptions"]["5"]["channels"]["9"], json!([[0, 99]]));
}

#[tokio::test]
async fn asks_for_the_member_ranges_again_after_the_server_invalidates_them() {
    let (listener, url) = bind().await;
    let resume_url = url.clone();

    let server = tokio::spawn(async move {
        let mut socket = FakeSocket::accept(&listener).await;
        socket.hello().await;
        socket.recv().await; // identify
        socket.ready("sess-1", &resume_url).await;
        socket.recv().await; // subscribe
        socket.recv().await; // first ranges
        socket
            .send(json!({
                "op": 0, "s": 3, "t": "GUILD_MEMBER_LIST_UPDATE",
                "d": { "guild_id": "5", "id": "everyone", "ops": [{ "op": "INVALIDATE", "range": [0, 99] }] }
            }))
            .await;
        let again = socket.recv().await;
        while socket.ws.next().await.is_some_and(|m| m.is_ok()) {}
        again
    });

    let (handle, mut events) = spawn(config(&url));
    handle.subscribe_members(Snowflake(5), Snowflake(9));

    let mut states = Vec::new();
    loop {
        if matches!(next_dispatch(&mut events, &mut states).await, DispatchEvent::GuildMemberListUpdate(_)) {
            break;
        }
    }
    handle.shutdown();
    let again = timeout(WAIT, server).await.unwrap().unwrap();

    assert_eq!(again["op"], 37);
    assert_eq!(again["d"]["subscriptions"]["5"]["channels"]["9"], json!([[0, 99]]));
}

#[tokio::test]
async fn identifies_again_after_a_non_resumable_invalid_session() {
    let (listener, url) = bind().await;
    let resume_url = url.clone();

    let server = tokio::spawn(async move {
        let mut first = FakeSocket::accept(&listener).await;
        first.hello().await;
        first.recv().await;
        first.ready("sess-1", &resume_url).await;
        first.send(json!({ "op": 9, "d": false })).await;

        let mut second = FakeSocket::accept(&listener).await;
        second.hello().await;
        let reply = second.recv().await;
        while second.ws.next().await.is_some_and(|m| m.is_ok()) {}
        reply
    });

    let (handle, mut events) = spawn(config(&url));
    let mut states = Vec::new();
    assert!(matches!(
        next_dispatch(&mut events, &mut states).await,
        DispatchEvent::Ready(_)
    ));

    // Give the client time to reconnect and answer HELLO.
    tokio::time::sleep(Duration::from_millis(300)).await;
    handle.shutdown();

    let reply = timeout(WAIT, server).await.unwrap().unwrap();
    assert_eq!(reply["op"], 2, "a dead session must not be resumed");
}

#[tokio::test]
async fn rejected_token_stops_without_retrying() {
    let (listener, url) = bind().await;

    let server = tokio::spawn(async move {
        let mut socket = FakeSocket::accept(&listener).await;
        socket.hello().await;
        socket.recv().await;
        socket
            .ws
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::from(4004),
                reason: "Authentication failed.".into(),
            })))
            .await
            .unwrap();
        // If the client wrongly retries, this second accept would succeed.
        timeout(Duration::from_millis(500), listener.accept()).await.is_err()
    });

    let (_handle, mut events) = spawn(config(&url));

    let mut saw_auth_failed = false;
    while let Some(event) = next_event(&mut events).await {
        if matches!(event, GatewayEvent::State(ConnectionState::AuthFailed)) {
            saw_auth_failed = true;
        }
    }
    assert!(saw_auth_failed);
    assert!(server.await.unwrap(), "client reconnected after a fatal close");
}
