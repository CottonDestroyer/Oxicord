//! One voice call: the voice websocket, the UDP socket beside it and the
//! decode path that turns packets into sound.

use std::{
    collections::HashMap,
    future::pending,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    net::UdpSocket,
    sync::mpsc,
    time::{Interval, MissedTickBehavior, interval, timeout},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    Command, VoiceEvent, VoiceParams,
    capture::{Capture, Gate},
    dave::{self, Dave, Out},
    opus::{Decoder, Encoder, FRAME},
    playback::Playback,
    transport::{self, Cipher, Mode},
};

const KEEPALIVE: Duration = Duration::from_secs(5);
/// What Discord sends in place of audio when someone stops talking.
const SILENCE: [u8; 3] = [0xf8, 0xff, 0xfe];

pub(crate) struct Closed {
    pub code: Option<u16>,
    pub reason: String,
}

impl Closed {
    fn new(reason: impl Into<String>) -> Self {
        Self { code: None, reason: reason.into() }
    }
}

pub(crate) async fn run(
    params: VoiceParams,
    mut commands: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<VoiceEvent>,
) {
    let closed = match serve(params, &mut commands, &events).await {
        Ok(()) => Closed::new("left the call"),
        Err(closed) => closed,
    };
    let _ = events.send(VoiceEvent::Closed { code: closed.code, reason: closed.reason });
}

struct Call {
    params: VoiceParams,
    events: mpsc::UnboundedSender<VoiceEvent>,
    dave: Dave,
    mode: Option<Mode>,
    cipher: Option<Cipher>,
    udp: Option<UdpSocket>,
    ssrc: u32,
    users: HashMap<u32, u64>,
    decoders: HashMap<u32, Decoder>,
    playback: Option<Playback>,
    deafened: bool,
    /// The newest sequence number the gateway sent, echoed in heartbeats.
    last_seq: i64,
    awaiting_ack: bool,
    keepalive_counter: u64,
    muted: bool,
    /// Where the microphone delivers its frames.
    frames_tx: mpsc::UnboundedSender<Vec<i16>>,
    capture: Option<Capture>,
    encoder: Option<Encoder>,
    gate: Gate,
    /// Whether we are currently sending speech.
    talking: bool,
    /// Every frame heard from the microphone, sent or not. It sets the RTP
    /// timestamp, so the gaps while we are quiet show up as time.
    heard: u64,
    sequence: u16,
    nonce: u32,
}

async fn serve(
    params: VoiceParams,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    events: &mpsc::UnboundedSender<VoiceEvent>,
) -> Result<(), Closed> {
    let endpoint = params.endpoint.trim_start_matches("wss://").trim_end_matches('/');
    let url = format!("wss://{endpoint}/?v=8");
    tracing::debug!(%url, "connecting to voice gateway");
    let (ws, _) = connect_async(url)
        .await
        .map_err(|err| Closed::new(format!("could not reach the voice server: {err}")))?;
    let (mut sink, mut stream) = ws.split();
    let (frames_tx, mut frames_rx) = mpsc::unbounded_channel();

    let mut call = Call {
        dave: Dave::new(params.user_id, params.channel_id),
        params,
        events: events.clone(),
        mode: None,
        cipher: None,
        udp: None,
        ssrc: 0,
        users: HashMap::new(),
        decoders: HashMap::new(),
        playback: None,
        deafened: false,
        last_seq: -1,
        awaiting_ack: false,
        keepalive_counter: 0,
        muted: false,
        frames_tx,
        capture: None,
        encoder: None,
        gate: Gate::default(),
        talking: false,
        heard: 0,
        sequence: 0,
        nonce: 0,
    };

    let mut beat: Option<Interval> = None;
    let mut keepalive = interval(KEEPALIVE);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut buf = vec![0u8; 2048];

    loop {
        let beat_tick = async {
            match beat.as_mut() {
                Some(beat) => {
                    beat.tick().await;
                }
                None => pending().await,
            }
        };
        let udp_recv = async {
            match call.udp.as_ref() {
                Some(udp) => udp.recv(&mut buf).await,
                None => pending().await,
            }
        };

        let mut out = Vec::new();
        tokio::select! {
            frame = stream.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                    if let Some(period) = call.on_json(&value, &mut out).await? {
                        let mut timer = interval(period);
                        timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
                        beat = Some(timer);
                    }
                }
                Some(Ok(Message::Binary(data))) => call.on_binary(&data, &mut out),
                Some(Ok(Message::Close(frame))) => {
                    let code = frame.as_ref().map(|f| u16::from(f.code));
                    let reason = frame.map(|f| f.reason.to_string()).unwrap_or_default();
                    return Err(Closed { code, reason });
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => return Err(Closed::new(format!("voice connection lost: {err}"))),
                None => return Err(Closed::new("voice connection closed")),
            },

            received = udp_recv => {
                if let Ok(len) = received {
                    call.on_packet(&buf[..len]);
                }
            }

            _ = beat_tick => {
                if call.awaiting_ack {
                    return Err(Closed::new("the voice server stopped answering"));
                }
                call.awaiting_ack = true;
                out.push(Out::Json(call.heartbeat()));
            }

            _ = keepalive.tick() => call.send_keepalive().await,

            Some(frame) = frames_rx.recv() => call.on_frame(&frame, &mut out),

            command = commands.recv() => match command {
                Some(Command::SetDeafened(deafened)) => call.set_deafened(deafened),
                Some(Command::SetMuted(muted)) => call.set_muted(muted, &mut out),
                Some(Command::Leave) | None => {
                    let _ = sink.send(Message::Close(None)).await;
                    return Ok(());
                }
            },
        }

        for message in out {
            let message = match message {
                Out::Json(value) => Message::text(value.to_string()),
                Out::Binary(bytes) => Message::binary(bytes),
            };
            if let Err(err) = sink.send(message).await {
                return Err(Closed::new(format!("voice connection lost: {err}")));
            }
        }
    }
}

impl Call {
    fn heartbeat(&self) -> Value {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        json!({ "op": 3, "d": { "t": nonce, "seq_ack": self.last_seq } })
    }

    fn set_deafened(&mut self, deafened: bool) {
        self.deafened = deafened;
        if let Some(playback) = &self.playback {
            playback.set_deafened(deafened);
        }
    }

    fn set_muted(&mut self, muted: bool, out: &mut Vec<Out>) {
        self.muted = muted;
        if muted {
            self.stop_talking(out);
        }
        self.sync_capture();
    }

    /// Holds the microphone only while we are unmuted in a live call, so a
    /// muted user is not shown as recording.
    fn sync_capture(&mut self) {
        let live = self.cipher.is_some();
        if self.muted || !live {
            self.capture = None;
        } else if self.capture.is_none() {
            match Capture::start(self.frames_tx.clone()) {
                Ok(capture) => self.capture = Some(capture),
                Err(reason) => {
                    let _ = self
                        .events
                        .send(VoiceEvent::Warning(format!("No one can hear you: {reason}.")));
                }
            }
        }
    }

    /// One 20 ms frame from the microphone.
    fn on_frame(&mut self, frame: &[i16], out: &mut Vec<Out>) {
        self.heard += 1;
        if self.muted || frame.len() != FRAME {
            return;
        }
        if self.gate.passes(frame) {
            if !self.talking {
                self.talking = true;
                out.push(self.speaking(true));
            }
            let Some(packet) = self.encoder.as_mut().and_then(|encoder| encoder.encode(frame)) else { return };
            self.send_opus(&packet);
        } else {
            self.stop_talking(out);
        }
    }

    /// Ends a stretch of speech the way Discord expects: five frames of
    /// silence, then the speaking flag off.
    fn stop_talking(&mut self, out: &mut Vec<Out>) {
        if !self.talking {
            return;
        }
        self.talking = false;
        for _ in 0..5 {
            self.send_opus(&SILENCE);
        }
        out.push(self.speaking(false));
    }

    fn speaking(&self, speaking: bool) -> Out {
        let _ = self.events.send(VoiceEvent::Speaking { user: self.params.user_id, speaking });
        Out::Json(json!({
            "op": 5,
            "d": { "speaking": u8::from(speaking), "delay": 0, "ssrc": self.ssrc }
        }))
    }

    fn send_opus(&mut self, opus: &[u8]) {
        let (Some(cipher), Some(udp)) = (&self.cipher, &self.udp) else { return };
        let Some(payload) = self.dave.seal(opus) else { return };
        let timestamp = (self.heard.wrapping_mul(FRAME as u64)) as u32;
        let header = transport::audio_header(self.sequence, timestamp, self.ssrc);
        let Some(packet) = cipher.seal(header, &payload, self.nonce) else { return };
        let _ = udp.try_send(&packet);
        self.sequence = self.sequence.wrapping_add(1);
        self.nonce = self.nonce.wrapping_add(1);
    }

    async fn send_keepalive(&mut self) {
        let Some(udp) = &self.udp else { return };
        self.keepalive_counter += 1;
        let _ = udp.send(&self.keepalive_counter.to_be_bytes()).await;
    }

    /// Handles one text frame. A returned duration starts the heartbeat.
    async fn on_json(&mut self, value: &Value, out: &mut Vec<Out>) -> Result<Option<Duration>, Closed> {
        if let Some(seq) = value["seq"].as_i64() {
            self.last_seq = self.last_seq.max(seq);
        }
        let data = &value["d"];
        match value["op"].as_u64() {
            // Hello
            Some(8) => {
                let millis = data["heartbeat_interval"].as_f64().unwrap_or(13_750.0);
                out.push(Out::Json(self.identify()));
                return Ok(Some(Duration::from_millis(millis as u64)));
            }
            // Ready
            Some(2) => self.on_ready(data, out).await?,
            // Session description
            Some(4) => self.on_description(data, out),
            // Heartbeat ack
            Some(6) => self.awaiting_ack = false,
            // Speaking
            Some(5) => {
                let user = id(&data["user_id"]);
                if let (Some(user), Some(ssrc)) = (user, data["ssrc"].as_u64()) {
                    self.users.insert(ssrc as u32, user);
                }
                if let Some(user) = user {
                    let speaking = data["speaking"].as_u64().unwrap_or(0) != 0;
                    let _ = self.events.send(VoiceEvent::Speaking { user, speaking });
                }
            }
            // Someone's streams: this is how we learn which ssrc is whose.
            Some(12) => {
                if let (Some(user), Some(ssrc)) = (id(&data["user_id"]), data["audio_ssrc"].as_u64()) {
                    self.users.insert(ssrc as u32, user);
                }
            }
            // Someone left.
            Some(13) => {
                if let Some(user) = id(&data["user_id"]) {
                    self.users.retain(|_, u| *u != user);
                    let _ = self.events.send(VoiceEvent::Speaking { user, speaking: false });
                }
            }
            Some(21) => {
                let transition = data["transition_id"].as_u64().unwrap_or(0) as u16;
                let version = data["protocol_version"].as_u64().unwrap_or(0) as u16;
                out.extend(self.dave.prepare_transition(transition, version));
            }
            Some(22) => {
                self.dave.execute_transition(data["transition_id"].as_u64().unwrap_or(0) as u16);
                self.report_encryption();
            }
            Some(24) => {
                let version = data["protocol_version"].as_u64().unwrap_or(0) as u16;
                let epoch = data["epoch"].as_u64().unwrap_or(0);
                out.extend(self.dave.prepare_epoch(version, epoch));
            }
            _ => {}
        }
        Ok(None)
    }

    fn on_binary(&mut self, data: &[u8], out: &mut Vec<Out>) {
        // Server messages: a big-endian sequence number, the opcode, the body.
        let Some((seq, rest)) = data.split_at_checked(2) else { return };
        self.last_seq = self.last_seq.max(i64::from(u16::from_be_bytes([seq[0], seq[1]])));
        let Some((&op, body)) = rest.split_first() else { return };
        match op {
            25 => self.dave.external_sender(body),
            27 => out.extend(self.dave.proposals(body)),
            29 => out.extend(self.dave.announce_commit(body)),
            30 => out.extend(self.dave.welcome(body)),
            _ => {}
        }
        self.report_encryption();
    }

    fn report_encryption(&self) {
        let _ = self.events.send(VoiceEvent::Encryption {
            active: self.dave.is_encrypted(),
            code: self.dave.privacy_code(),
        });
    }

    fn identify(&self) -> Value {
        json!({
            "op": 0,
            "d": {
                "server_id": self.params.server_id.to_string(),
                "user_id": self.params.user_id.to_string(),
                "session_id": self.params.session_id,
                "token": self.params.token,
                "max_dave_protocol_version": dave::MAX_VERSION,
            }
        })
    }

    async fn on_ready(&mut self, data: &Value, out: &mut Vec<Out>) -> Result<(), Closed> {
        let ssrc = data["ssrc"].as_u64().ok_or_else(|| Closed::new("the voice server sent no ssrc"))? as u32;
        let ip = data["ip"].as_str().unwrap_or_default();
        let port = data["port"].as_u64().unwrap_or(0) as u16;
        let modes: Vec<String> = data["modes"]
            .as_array()
            .map(|modes| modes.iter().filter_map(|m| m.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let mode = Mode::pick(&modes).ok_or_else(|| Closed::new("the voice server offers no encryption we support"))?;

        let target: SocketAddr = format!("{ip}:{port}")
            .parse()
            .map_err(|_| Closed::new("the voice server sent a bad address"))?;
        let udp = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|err| Closed::new(format!("could not open a voice socket: {err}")))?;
        udp.connect(target)
            .await
            .map_err(|err| Closed::new(format!("could not reach the voice server: {err}")))?;
        let (address, external_port) = discover(&udp, ssrc).await?;

        self.ssrc = ssrc;
        self.mode = Some(mode);
        self.udp = Some(udp);
        out.push(Out::Json(json!({
            "op": 1,
            "d": {
                "protocol": "udp",
                "data": { "address": address, "port": external_port, "mode": mode.name() },
            }
        })));
        Ok(())
    }

    fn on_description(&mut self, data: &Value, out: &mut Vec<Out>) {
        let key: Vec<u8> = data["secret_key"]
            .as_array()
            .map(|key| key.iter().filter_map(|b| b.as_u64().map(|b| b as u8)).collect())
            .unwrap_or_default();
        let mode = data["mode"].as_str().and_then(Mode::from_name).or(self.mode);
        let Some(cipher) = mode.and_then(|mode| Cipher::new(mode, &key)) else {
            tracing::warn!("the session description had no usable key");
            return;
        };
        self.cipher = Some(cipher);

        let version = data["dave_protocol_version"].as_u64().unwrap_or(0) as u16;
        out.extend(self.dave.start(version));

        match Playback::start() {
            Ok(playback) => {
                playback.set_deafened(self.deafened);
                self.playback = Some(playback);
            }
            Err(reason) => {
                let _ = self.events.send(VoiceEvent::Warning(format!("You can't hear anyone: {reason}.")));
            }
        }
        self.encoder = Encoder::new();
        self.sync_capture();
        let _ = self.events.send(VoiceEvent::Connected);
        self.report_encryption();
    }

    /// One datagram from the voice server.
    fn on_packet(&mut self, packet: &[u8]) {
        let (Some(cipher), Some(playback)) = (&self.cipher, &self.playback) else { return };
        let Some((header, payload)) = cipher.open(packet) else { return };
        if header.payload_type != transport::OPUS_PAYLOAD_TYPE || header.ssrc == self.ssrc || payload == SILENCE {
            return;
        }
        let sender = self.users.get(&header.ssrc).copied();
        let Some(opus) = self.dave.open(sender, payload) else { return };

        let decoder = match self.decoders.entry(header.ssrc) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => match Decoder::new() {
                Some(decoder) => entry.insert(decoder),
                None => return,
            },
        };
        if let Some(pcm) = decoder.decode(&opus) {
            playback.push(header.ssrc, &pcm);
        }
    }
}

/// Learns our address as the voice server sees it.
async fn discover(udp: &UdpSocket, ssrc: u32) -> Result<(String, u16), Closed> {
    let fail = |what: &str| Closed::new(format!("voice address discovery failed: {what}"));
    udp.send(&transport::discovery_request(ssrc))
        .await
        .map_err(|err| fail(&err.to_string()))?;
    let mut answer = [0u8; 128];
    let len = timeout(Duration::from_secs(5), udp.recv(&mut answer))
        .await
        .map_err(|_| fail("no answer"))?
        .map_err(|err| fail(&err.to_string()))?;
    transport::parse_discovery(&answer[..len]).ok_or_else(|| fail("unreadable answer"))
}

/// Discord sends ids as strings.
fn id(value: &Value) -> Option<u64> {
    value.as_str().and_then(|s| s.parse().ok()).or_else(|| value.as_u64())
}
