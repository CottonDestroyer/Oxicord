use std::{
    fmt::Display,
    future::pending,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{Sink, SinkExt, StreamExt};
use oxicord_model::gateway::{DispatchEvent, GatewayPayload, Hello, op};
use serde_json::Value;
use tokio::{
    sync::mpsc,
    time::{Instant, sleep_until},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    ConnectionState, GatewayConfig, GatewayEvent,
    protocol::{self, BeatAction, Backoff, ClientStatus, CloseAction, Heartbeat},
    zlib::ZlibStream,
};

pub(crate) enum Command {
    SetStatus(ClientStatus),
    Shutdown,
}

enum Exit {
    Shutdown,
    Fatal(ConnectionState),
    Reconnect { resumable: bool },
}

const RECONNECT: Exit = Exit::Reconnect { resumable: true };

/// What we need to remember between sockets to be able to RESUME.
#[derive(Default)]
struct Persist {
    session_id: Option<String>,
    resume_url: Option<String>,
    seq: Option<u64>,
}

/// State that only lives as long as one socket.
struct Conn<S> {
    sink: S,
    heartbeat: Heartbeat,
    interval: Option<Duration>,
    next_beat: Option<Instant>,
    /// True once READY or RESUMED arrived. Presence updates sent earlier
    /// would be rejected as unauthenticated.
    ready: bool,
}

impl<S> Conn<S>
where
    S: Sink<Message> + Unpin,
    S::Error: Display,
{
    async fn send(&mut self, value: &Value) -> bool {
        match self.sink.send(Message::text(value.to_string())).await {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(%err, "gateway send failed");
                false
            }
        }
    }

    async fn close(&mut self) {
        let _ = self.sink.send(Message::Close(None)).await;
    }
}

pub(crate) struct Runtime {
    config: GatewayConfig,
    cmds: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<GatewayEvent>,
    persist: Persist,
    backoff: Backoff,
    status: ClientStatus,
}

impl Runtime {
    pub(crate) fn new(
        config: GatewayConfig,
        cmds: mpsc::UnboundedReceiver<Command>,
        events: mpsc::UnboundedSender<GatewayEvent>,
    ) -> Self {
        Self {
            backoff: Backoff::new(config.reconnect_base),
            status: config.status,
            config,
            cmds,
            events,
            persist: Persist::default(),
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            let state = match self.backoff.attempt() {
                0 => ConnectionState::Connecting,
                attempt => ConnectionState::Reconnecting { attempt },
            };
            if !self.emit(GatewayEvent::State(state)) {
                return;
            }

            match self.serve().await {
                Exit::Shutdown => return,
                Exit::Fatal(state) => {
                    self.emit(GatewayEvent::State(state));
                    return;
                }
                Exit::Reconnect { resumable } => {
                    if !resumable {
                        self.persist = Persist::default();
                    }
                    let delay = self.backoff.next_delay();
                    if !self.wait(delay).await {
                        return;
                    }
                }
            }
        }
    }

    /// Returns false when the consumer went away and we should stop.
    fn emit(&self, event: GatewayEvent) -> bool {
        self.events.send(event).is_ok()
    }

    /// Sleeps between attempts but stays responsive to commands. False
    /// means shutdown was requested.
    async fn wait(&mut self, delay: Duration) -> bool {
        let deadline = Instant::now() + delay;
        loop {
            tokio::select! {
                _ = sleep_until(deadline) => return true,
                cmd = self.cmds.recv() => match cmd {
                    Some(Command::SetStatus(status)) => self.status = status,
                    Some(Command::Shutdown) | None => return false,
                },
            }
        }
    }

    async fn serve(&mut self) -> Exit {
        let base = self
            .persist
            .resume_url
            .as_deref()
            .unwrap_or(&self.config.url);
        let url = protocol::gateway_url(base);
        tracing::debug!(%url, "connecting to gateway");

        let ws = match connect_async(url).await {
            Ok((ws, _)) => ws,
            Err(err) => {
                tracing::warn!(%err, "gateway connect failed");
                return RECONNECT;
            }
        };
        tracing::info!(resuming = self.persist.session_id.is_some(), "gateway socket open");
        let (sink, mut stream) = ws.split();

        let mut conn = Conn {
            sink,
            heartbeat: Heartbeat::default(),
            interval: None,
            next_beat: None,
            ready: false,
        };
        let mut zlib = ZlibStream::default();

        loop {
            let next_beat = conn.next_beat;
            let beat = async {
                match next_beat {
                    Some(at) => sleep_until(at).await,
                    None => pending().await,
                }
            };

            tokio::select! {
                frame = stream.next() => {
                    let msg = match frame {
                        Some(Ok(msg)) => msg,
                        Some(Err(err)) => {
                            tracing::warn!(%err, "gateway read failed");
                            return RECONNECT;
                        }
                        None => return RECONNECT,
                    };

                    let bytes = match msg {
                        Message::Binary(chunk) => match zlib.push(&chunk) {
                            Ok(Some(bytes)) => bytes,
                            Ok(None) => continue,
                            Err(err) => {
                                // The inflate context is now out of step with
                                // the server, so this socket is useless.
                                tracing::warn!(%err, "bad zlib-stream data");
                                return RECONNECT;
                            }
                        },
                        Message::Text(text) => text.as_bytes().to_vec(),
                        Message::Close(frame) => {
                            let code = frame.as_ref().map(|f| u16::from(f.code));
                            let reason = frame.map(|f| f.reason.to_string()).unwrap_or_default();
                            return self.on_close(code, &reason);
                        }
                        _ => continue,
                    };

                    let payload = match serde_json::from_slice::<GatewayPayload>(&bytes) {
                        Ok(p) => p,
                        Err(err) => {
                            tracing::warn!(%err, "unreadable gateway payload");
                            continue;
                        }
                    };
                    if let Some(exit) = self.on_payload(&mut conn, payload).await {
                        return exit;
                    }
                }

                _ = beat => {
                    match conn.heartbeat.tick() {
                        BeatAction::Send => {
                            if !conn.send(&protocol::heartbeat(self.persist.seq)).await {
                                return RECONNECT;
                            }
                            conn.next_beat = conn.interval.map(|i| Instant::now() + i);
                        }
                        BeatAction::Zombie => {
                            tracing::warn!("heartbeat was never acknowledged, reconnecting");
                            conn.close().await;
                            return RECONNECT;
                        }
                    }
                }

                cmd = self.cmds.recv() => match cmd {
                    Some(Command::SetStatus(status)) => {
                        self.status = status;
                        if conn.ready && !conn.send(&protocol::presence_update(status)).await {
                            return RECONNECT;
                        }
                    }
                    Some(Command::Shutdown) | None => {
                        conn.close().await;
                        return Exit::Shutdown;
                    }
                },
            }
        }
    }

    fn on_close(&self, code: Option<u16>, reason: &str) -> Exit {
        let action = protocol::close_action(code);
        tracing::info!(?code, reason, ?action, "gateway closed the connection");
        match action {
            CloseAction::Resume => Exit::Reconnect { resumable: true },
            CloseAction::Reidentify => Exit::Reconnect { resumable: false },
            CloseAction::Fatal if code == Some(4004) => Exit::Fatal(ConnectionState::AuthFailed),
            CloseAction::Fatal => Exit::Fatal(ConnectionState::Closed(format!(
                "gateway refused the connection (code {})",
                code.unwrap_or_default()
            ))),
        }
    }

    async fn on_payload<S>(&mut self, conn: &mut Conn<S>, p: GatewayPayload) -> Option<Exit>
    where
        S: Sink<Message> + Unpin,
        S::Error: Display,
    {
        match p.op {
            op::HELLO => {
                let hello: Hello = serde_json::from_value(p.d).ok()?;
                let interval = Duration::from_millis(hello.heartbeat_interval.max(1));
                conn.interval = Some(interval);
                // Discord asks for the first beat after a random fraction of
                // the interval so reconnecting clients don't all line up.
                conn.next_beat = Some(Instant::now() + interval.mul_f64(jitter()));

                let reply = match &self.persist.session_id {
                    Some(id) => protocol::resume(&self.config.token, id, self.persist.seq),
                    None => {
                        protocol::identify(&self.config.token, &self.config.props, self.status)
                    }
                };
                if !conn.send(&reply).await {
                    return Some(RECONNECT);
                }
                None
            }
            op::HEARTBEAT_ACK => {
                conn.heartbeat.ack();
                None
            }
            op::HEARTBEAT => {
                conn.heartbeat.forced();
                if !conn.send(&protocol::heartbeat(self.persist.seq)).await {
                    return Some(RECONNECT);
                }
                None
            }
            op::RECONNECT => Some(RECONNECT),
            op::INVALID_SESSION => Some(Exit::Reconnect {
                resumable: p.d.as_bool().unwrap_or(false),
            }),
            op::DISPATCH => self.on_dispatch(conn, p),
            other => {
                tracing::debug!(op = other, "ignoring unknown opcode");
                None
            }
        }
    }

    fn on_dispatch<S>(&mut self, conn: &mut Conn<S>, p: GatewayPayload) -> Option<Exit> {
        if let Some(seq) = p.s {
            self.persist.seq = Some(seq);
        }
        let name = p.t?;

        let event = match DispatchEvent::parse(&name, p.d) {
            Ok(event) => event,
            // Losing READY would leave the UI waiting forever, so surface it.
            // Any other event is safe to skip.
            Err(err) if name == "READY" => {
                return Some(Exit::Fatal(ConnectionState::Closed(format!(
                    "could not read READY: {err}"
                ))));
            }
            Err(err) => {
                tracing::warn!(event = %name, %err, "skipping unreadable dispatch");
                return None;
            }
        };

        let connected = match &event {
            DispatchEvent::Ready(ready) => {
                tracing::info!(guilds = ready.guilds.len(), "READY received");
                self.persist.session_id = Some(ready.session_id.clone());
                self.persist.resume_url = ready.resume_gateway_url.clone();
                true
            }
            DispatchEvent::Resumed => true,
            _ => false,
        };

        if !self.emit(GatewayEvent::Dispatch(event)) {
            return Some(Exit::Shutdown);
        }
        if connected {
            conn.ready = true;
            self.backoff.reset();
            if !self.emit(GatewayEvent::State(ConnectionState::Connected)) {
                return Some(Exit::Shutdown);
            }
        }
        None
    }
}

fn jitter() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| f64::from(d.subsec_nanos()) / 1e9)
        .unwrap_or(0.5)
}
