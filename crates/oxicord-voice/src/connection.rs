//! One voice call: the voice websocket, the UDP socket beside it and the
//! decode path that turns packets into sound.

use std::{
    collections::HashMap,
    future::pending,
    net::SocketAddr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
    capture::{Capture, Gate, with_volume},
    dave::{self, Dave, Out},
    opus::{Decoder, Encoder, FRAME},
    playback::{MAX_USER_VOLUME, Playback, with_gain},
    transport::{self, Cipher, Mode},
};

const KEEPALIVE: Duration = Duration::from_secs(5);
/// How long after someone's last audio they still count as talking.
const TALK_HANGOVER: Duration = Duration::from_millis(300);
/// What Discord sends in place of audio when someone stops talking.
const SILENCE: [u8; 3] = [0xf8, 0xff, 0xfe];

/// Who is talking, worked out from the audio that arrives. The gateway only
/// says which stream belongs to whom, and never says when someone stops.
#[derive(Default)]
struct Talkers {
    last_heard: HashMap<u64, Instant>,
}

impl Talkers {
    /// Notes audio from `user`. True when this starts a stretch of talking.
    fn heard(&mut self, user: u64, now: Instant) -> bool {
        self.last_heard.insert(user, now).is_none()
    }

    /// People who have been quiet for longer than the hangover.
    fn expire(&mut self, now: Instant) -> Vec<u64> {
        let quiet: Vec<u64> = self
            .last_heard
            .iter()
            .filter(|(_, at)| now.duration_since(**at) > TALK_HANGOVER)
            .map(|(&user, _)| user)
            .collect();
        for user in &quiet {
            self.last_heard.remove(user);
        }
        quiet
    }

    /// Someone left or was told to be silent. True if they were talking.
    fn stop(&mut self, user: u64) -> bool {
        self.last_heard.remove(&user).is_some()
    }
}

pub(crate) struct Closed {
    pub code: Option<u16>,
    pub reason: String,
    /// The session is still good on Discord's side, so reconnecting and
    /// resuming can bring the call back.
    pub resumable: bool,
}

impl Closed {
    /// The call is over.
    fn new(reason: impl Into<String>) -> Self {
        Self { code: None, reason: reason.into(), resumable: false }
    }

    /// The websocket went away but the call may carry on.
    fn dropped(reason: impl Into<String>) -> Self {
        Self { code: None, reason: reason.into(), resumable: true }
    }
}

/// Whether a close code leaves the session alive. No code means the socket
/// broke without a goodbye, and 4015 is a voice server that crashed. The
/// rest (kicked, session expired, bad token) are final.
fn can_resume(code: Option<u16>) -> bool {
    matches!(code, None | Some(1001 | 1006 | 4015))
}

/// What to wait before each attempt to resume. When they run out the call ends.
const RESUME_DELAYS: [Duration; 5] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
];
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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
    input_device: Option<String>,
    output_device: Option<String>,
    encoder: Option<Encoder>,
    gate: Gate,
    /// The input volume slider, 0 to 100.
    input_volume: u8,
    /// How loud each person is played, by user id. Missing means 100.
    user_volumes: HashMap<u64, u16>,
    /// Push to talk: the microphone is open only while the key is down.
    push_to_talk: bool,
    key_down: bool,
    /// Whether we are currently sending speech.
    talking: bool,
    /// Every frame heard from the microphone, sent or not. It sets the RTP
    /// timestamp, so the gaps while we are quiet show up as time.
    heard: u64,
    sequence: u16,
    nonce: u32,
    talkers: Talkers,
    /// The next websocket should resume the session instead of starting one.
    resuming: bool,
    /// Resumes tried since the last one that worked.
    retries: usize,
}

async fn serve(
    params: VoiceParams,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    events: &mpsc::UnboundedSender<VoiceEvent>,
) -> Result<(), Closed> {
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
        input_device: None,
        output_device: None,
        encoder: None,
        gate: Gate::default(),
        input_volume: 100,
        user_volumes: HashMap::new(),
        push_to_talk: false,
        key_down: false,
        talking: false,
        heard: 0,
        sequence: 0,
        nonce: 0,
        talkers: Talkers::default(),
        resuming: false,
        retries: 0,
    };

    call.input_device = call.params.input_device.clone();
    call.output_device = call.params.output_device.clone();

    // A dropped websocket does not end the call: the session, the UDP socket
    // and the keys all survive, so we reconnect and ask to pick up where we
    // were. Audio keeps flowing over UDP meanwhile.
    loop {
        let closed = match session(&mut call, commands, &mut frames_rx).await {
            Ok(()) => return Ok(()),
            Err(closed) => closed,
        };
        let Some(&delay) = RESUME_DELAYS.get(call.retries) else { return Err(closed) };
        if !closed.resumable || call.cipher.is_none() {
            return Err(closed);
        }
        tracing::info!(attempt = call.retries + 1, reason = %closed.reason, "voice websocket dropped, resuming");
        call.retries += 1;
        call.resuming = true;
        if call.pause(delay, commands, &mut frames_rx).await {
            return Ok(());
        }
    }
}

/// One voice websocket, from connecting to it closing. A new one starts with
/// identify, or with resume when the call is already running.
async fn session(
    call: &mut Call,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    frames_rx: &mut mpsc::UnboundedReceiver<Vec<i16>>,
) -> Result<(), Closed> {
    let endpoint = call.params.endpoint.trim_start_matches("wss://").trim_end_matches('/');
    let url = format!("wss://{endpoint}/?v=8");
    tracing::debug!(%url, "connecting to voice gateway");
    let resuming = call.resuming;
    let lost = |err: String| {
        let reason = format!("could not reach the voice server: {err}");
        if resuming { Closed::dropped(reason) } else { Closed::new(reason) }
    };
    let (ws, _) = timeout(CONNECT_TIMEOUT, connect_async(url))
        .await
        .map_err(|_| lost("it took too long".into()))?
        .map_err(|err| lost(err.to_string()))?;
    let (mut sink, mut stream) = ws.split();
    call.awaiting_ack = false;

    let mut beat: Option<Interval> = None;
    let mut keepalive = interval(KEEPALIVE);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut talk_check = interval(Duration::from_millis(100));
    talk_check.set_missed_tick_behavior(MissedTickBehavior::Delay);
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
                    return Err(Closed { code, reason, resumable: can_resume(code) });
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => return Err(Closed::dropped(format!("voice connection lost: {err}"))),
                None => return Err(Closed::dropped("voice connection closed")),
            },

            received = udp_recv => {
                if let Ok(len) = received {
                    call.on_packet(&buf[..len]);
                }
            }

            _ = beat_tick => {
                if call.awaiting_ack {
                    return Err(Closed::dropped("the voice server stopped answering"));
                }
                call.awaiting_ack = true;
                out.push(Out::Json(call.heartbeat()));
            }

            _ = keepalive.tick() => call.send_keepalive().await,

            _ = talk_check.tick() => call.expire_talkers(),

            Some(frame) = frames_rx.recv() => call.on_frame(&frame, &mut out),

            command = commands.recv() => {
                if call.on_command(command, &mut out) {
                    let _ = sink.send(Message::Close(None)).await;
                    return Ok(());
                }
            }
        }

        for message in out {
            let message = match message {
                Out::Json(value) => Message::text(value.to_string()),
                Out::Binary(bytes) => Message::binary(bytes),
            };
            if let Err(err) = sink.send(message).await {
                return Err(Closed::dropped(format!("voice connection lost: {err}")));
            }
        }
    }
}

impl Call {
    /// Applies one command from the app. True when it is time to leave.
    fn on_command(&mut self, command: Option<Command>, out: &mut Vec<Out>) -> bool {
        match command {
            Some(Command::SetDeafened(deafened)) => self.set_deafened(deafened),
            Some(Command::SetMuted(muted)) => self.set_muted(muted, out),
            Some(Command::SetInputDevice(device)) => self.set_input_device(device),
            Some(Command::SetOutputDevice(device)) => self.set_output_device(device),
            Some(Command::SetInputVolume(percent)) => self.set_input_volume(percent),
            Some(Command::SetPushToTalk(on)) => self.set_push_to_talk(on, out),
            Some(Command::SetTalkKey(down)) => self.set_talk_key(down, out),
            Some(Command::SetUserVolume { user, percent }) => {
                self.user_volumes.insert(user, percent.min(MAX_USER_VOLUME));
            }
            Some(Command::Leave) | None => return true,
        }
        false
    }

    /// Waits out the gap before a reconnect. The app can still change
    /// settings meanwhile, and the microphone clock keeps counting so the
    /// audio lines up again. True if we were told to leave.
    async fn pause(
        &mut self,
        delay: Duration,
        commands: &mut mpsc::UnboundedReceiver<Command>,
        frames_rx: &mut mpsc::UnboundedReceiver<Vec<i16>>,
    ) -> bool {
        let wake = tokio::time::sleep(delay);
        tokio::pin!(wake);
        loop {
            tokio::select! {
                _ = &mut wake => return false,
                Some(_) = frames_rx.recv() => self.heard += 1,
                command = commands.recv() => {
                    // Anything it would say to Discord has no socket to go to.
                    if self.on_command(command, &mut Vec::new()) {
                        return true;
                    }
                }
            }
        }
    }

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

    /// Switches microphone: the old one is let go and the new one opened.
    fn set_input_device(&mut self, device: Option<String>) {
        self.input_device = device;
        self.capture = None;
        self.sync_capture();
    }

    /// How loud the microphone is sent, 0 to 100.
    fn set_input_volume(&mut self, percent: u8) {
        self.input_volume = percent.min(100);
    }

    fn set_push_to_talk(&mut self, on: bool, out: &mut Vec<Out>) {
        self.push_to_talk = on;
        // Turning it on with the key up closes the microphone at once.
        if !self.mic_open() {
            self.stop_talking(out);
        }
    }

    fn set_talk_key(&mut self, down: bool, out: &mut Vec<Out>) {
        self.key_down = down;
        if !self.mic_open() {
            self.stop_talking(out);
        }
    }

    /// Whether push to talk lets the microphone through right now.
    fn mic_open(&self) -> bool {
        sends(self.push_to_talk, self.key_down, || true)
    }

    /// Switches speakers, keeping whatever was deafened.
    fn set_output_device(&mut self, device: Option<String>) {
        self.output_device = device;
        if self.cipher.is_none() {
            return;
        }
        self.playback = None;
        match Playback::start(self.output_device.clone()) {
            Ok(playback) => {
                playback.set_deafened(self.deafened);
                self.playback = Some(playback);
            }
            Err(reason) => {
                let _ = self.events.send(VoiceEvent::Warning(format!("You can't hear anyone: {reason}.")));
            }
        }
    }

    fn expire_talkers(&mut self) {
        for user in self.talkers.expire(Instant::now()) {
            let _ = self.events.send(VoiceEvent::Speaking { user, speaking: false });
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
            match Capture::start(self.frames_tx.clone(), self.input_device.clone()) {
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
        // Holding the key sends everything, so the gate is not asked.
        let sending = sends(self.push_to_talk, self.key_down, || self.gate.passes(frame));
        if sending {
            if !self.talking {
                self.talking = true;
                out.push(self.speaking(true));
            }
            let scaled = with_volume(frame, self.input_volume);
            let Some(packet) = self.encoder.as_mut().and_then(|encoder| encoder.encode(&scaled)) else { return };
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
                let hello = if self.resuming {
                    resume_payload(&self.params, self.last_seq)
                } else {
                    self.identify()
                };
                out.push(Out::Json(hello));
                return Ok(Some(Duration::from_millis(millis as u64)));
            }
            // Resumed: the session is back, nothing was missed.
            Some(9) => {
                tracing::info!("voice session resumed");
                self.resuming = false;
                self.retries = 0;
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
                // Only the stop is taken from here. Starting to talk is seen in
                // the audio itself, so a missed stop can never leave someone
                // marked as talking.
                if let Some(user) = user
                    && data["speaking"].as_u64().unwrap_or(0) == 0
                    && self.talkers.stop(user)
                {
                    let _ = self.events.send(VoiceEvent::Speaking { user, speaking: false });
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
                    self.talkers.stop(user);
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

        match Playback::start(self.output_device.clone()) {
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
        // In an encrypted call the silence marker only shows after decrypting.
        if opus == SILENCE {
            return;
        }
        if let Some(user) = sender
            && self.talkers.heard(user, Instant::now())
        {
            let _ = self.events.send(VoiceEvent::Speaking { user, speaking: true });
        }

        let decoder = match self.decoders.entry(header.ssrc) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => match Decoder::new() {
                Some(decoder) => entry.insert(decoder),
                None => return,
            },
        };
        if let Some(pcm) = decoder.decode(&opus) {
            match sender.and_then(|user| self.user_volumes.get(&user)).copied() {
                Some(percent) if percent != 100 => playback.push(header.ssrc, &with_gain(&pcm, percent)),
                _ => playback.push(header.ssrc, &pcm),
            }
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

/// Op 7: asks the voice server to pick up our session where it left off.
fn resume_payload(params: &VoiceParams, seq_ack: i64) -> Value {
    json!({
        "op": 7,
        "d": {
            "server_id": params.server_id.to_string(),
            "session_id": params.session_id,
            "token": params.token,
            "seq_ack": seq_ack,
        }
    })
}

/// Whether a frame from the microphone goes out. Push to talk sends while the
/// key is down and ignores the gate, which is only asked otherwise.
fn sends(push_to_talk: bool, key_down: bool, gate: impl FnOnce() -> bool) -> bool {
    if push_to_talk { key_down } else { gate() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_broken_socket_or_a_crashed_server_can_be_resumed() {
        assert!(can_resume(None), "no goodbye at all");
        assert!(can_resume(Some(1006)));
        assert!(can_resume(Some(4015)), "voice server crashed");
        assert!(!can_resume(Some(4014)), "we were disconnected on purpose");
        assert!(!can_resume(Some(4006)), "session no longer valid");
        assert!(!can_resume(Some(4009)), "session timed out");
        assert!(!can_resume(Some(4004)), "bad token");
        assert!(!Closed::new("no encryption we support").resumable);
        assert!(Closed::dropped("lost").resumable);
    }

    #[test]
    fn resume_names_the_session_and_the_last_message_seen() {
        let params = VoiceParams {
            endpoint: "x.discord.media:443".into(),
            server_id: 11,
            channel_id: 22,
            user_id: 33,
            session_id: "sess".into(),
            token: "tok".into(),
            input_device: None,
            output_device: None,
        };
        let payload = resume_payload(&params, 41);
        assert_eq!(payload["op"], 7);
        assert_eq!(payload["d"]["server_id"], "11");
        assert_eq!(payload["d"]["session_id"], "sess");
        assert_eq!(payload["d"]["token"], "tok");
        assert_eq!(payload["d"]["seq_ack"], 41);
    }

    #[test]
    fn push_to_talk_sends_only_while_the_key_is_down() {
        assert!(sends(true, true, || false), "held key beats a quiet gate");
        assert!(!sends(true, false, || true), "loud room, key up: nothing");
        assert!(sends(false, false, || true), "off: the gate decides");
        assert!(!sends(false, true, || false));
    }

    #[test]
    fn someone_is_talking_from_their_first_packet_until_the_hangover_passes() {
        let mut talkers = Talkers::default();
        let start = Instant::now();
        assert!(talkers.heard(7, start), "the first packet starts the talking");
        assert!(!talkers.heard(7, start + Duration::from_millis(20)), "later ones do not");

        assert!(talkers.expire(start + Duration::from_millis(200)).is_empty());
        assert_eq!(talkers.expire(start + Duration::from_millis(400)), vec![7]);
        // Talking again after a pause counts as a new start.
        assert!(talkers.heard(7, start + Duration::from_secs(1)));
    }

    #[test]
    fn a_silence_flag_from_the_gateway_ends_the_talking_at_once() {
        let mut talkers = Talkers::default();
        talkers.heard(7, Instant::now());
        assert!(talkers.stop(7));
        assert!(!talkers.stop(7));
    }
}
