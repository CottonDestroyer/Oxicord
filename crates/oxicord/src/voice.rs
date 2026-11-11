//! Joining a voice call takes three parties: we ask the gateway to put us in
//! a channel, the gateway answers with the voice server to connect to, and
//! the voice crate runs the call. This keeps those in step.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, atomic::{AtomicBool, Ordering}},
    time::Duration,
};

use async_channel::Sender;
use oxicord_gateway::GatewayHandle;
use oxicord_model::{Snowflake, gateway::{DispatchEvent, VoiceServer}};
use oxicord_ui::{Event, VoiceNotice};
use oxicord_voice::{VoiceEvent, VoiceHandle, VoiceParams};

/// The channel we asked to be in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Want {
    guild: Option<Snowflake>,
    channel: Snowflake,
}

impl Want {
    /// Whether a server update is the answer to this join.
    fn is_answered_by(&self, server: &VoiceServer) -> bool {
        match self.guild {
            Some(guild) => server.guild_id == Some(guild),
            None => server.guild_id.is_none() && server.channel_id == Some(self.channel),
        }
    }

    /// A private call is identified by its channel.
    fn server_id(&self) -> Snowflake {
        self.guild.unwrap_or(self.channel)
    }
}

struct Inner {
    session_id: Option<String>,
    user: Option<Snowflake>,
    want: Option<Want>,
    call: Option<VoiceHandle>,
    /// Counts calls, so a call that already ended cannot report in again.
    generation: u64,
    mute: bool,
    deafen: bool,
    /// The microphone and speakers chosen, the system defaults when `None`.
    input_device: Option<String>,
    output_device: Option<String>,
    /// The Input Volume setting, 0 to 100.
    input_volume: u8,
    /// Whether the microphone goes through noise suppression.
    noise_suppression: bool,
    /// How loud each person is played, 0 to 200, by user id.
    user_volumes: HashMap<u64, u16>,
    /// The push to talk key, while push to talk is on.
    talk_key: Option<String>,
    /// Whether that key is held, for a call that starts while it is.
    talk_key_down: bool,
    /// Tells the thread watching the key to stop.
    watcher: Option<Arc<AtomicBool>>,
}

#[derive(Clone)]
pub struct Voice {
    inner: Arc<Mutex<Inner>>,
    gateway: GatewayHandle,
    events: Sender<Event>,
}

impl Voice {
    pub fn new(gateway: GatewayHandle, events: Sender<Event>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                session_id: None,
                user: None,
                want: None,
                call: None,
                generation: 0,
                mute: false,
                deafen: false,
                input_device: None,
                output_device: None,
                input_volume: 100,
                noise_suppression: false,
                user_volumes: HashMap::new(),
                talk_key: None,
                talk_key_down: false,
                watcher: None,
            })),
            gateway,
            events,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn tell(&self, notice: VoiceNotice) {
        let _ = self.events.try_send(Event::Voice(notice));
    }

    /// Watches the gateway for what a call depends on.
    pub fn observe(&self, event: &DispatchEvent) {
        match event {
            DispatchEvent::Ready(ready) => {
                let mut inner = self.lock();
                inner.session_id = Some(ready.session_id.clone());
                inner.user = Some(ready.user.id);
            }
            DispatchEvent::VoiceServerUpdate(server) => self.on_server(server),
            DispatchEvent::VoiceStateUpdate(state) => self.on_state(state.user_id, state.channel_id),
            _ => {}
        }
    }

    fn on_server(&self, server: &VoiceServer) {
        let mut inner = self.lock();
        let Some(want) = inner.want.filter(|want| want.is_answered_by(server)) else { return };
        // No endpoint yet: Discord is still choosing a server.
        let Some(endpoint) = server.endpoint.clone() else { return };
        let (Some(session_id), Some(user)) = (inner.session_id.clone(), inner.user) else { return };

        // A new server for the same channel replaces the call we had.
        if let Some(old) = inner.call.take() {
            old.leave();
        }
        inner.generation += 1;
        let generation = inner.generation;

        let (handle, mut events) = oxicord_voice::spawn(VoiceParams {
            endpoint,
            server_id: want.server_id().0,
            channel_id: want.channel.0,
            user_id: user.0,
            session_id,
            token: server.token.clone(),
            input_device: inner.input_device.clone(),
            output_device: inner.output_device.clone(),
        });
        handle.set_input_volume(inner.input_volume);
        handle.set_noise_suppression(inner.noise_suppression);
        for (user, percent) in &inner.user_volumes {
            handle.set_user_volume(*user, *percent);
        }
        handle.set_push_to_talk(inner.talk_key.is_some());
        handle.set_talk_key(inner.talk_key_down);
        handle.set_deafened(inner.deafen);
        handle.set_muted(inner.mute || inner.deafen);
        inner.call = Some(handle);
        drop(inner);

        let this = self.clone();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                let notice = match event {
                    VoiceEvent::Connected => VoiceNotice::Connected,
                    VoiceEvent::Warning(text) => VoiceNotice::Warning(text),
                    VoiceEvent::Speaking { user, speaking } => {
                        VoiceNotice::Speaking { user: Snowflake(user), speaking }
                    }
                    VoiceEvent::Encryption { active, code } => VoiceNotice::Encryption { active, code },
                    VoiceEvent::Closed { code, reason } => {
                        this.on_closed(generation, code, &reason);
                        return;
                    }
                };
                if this.lock().generation == generation {
                    this.tell(notice);
                }
            }
        });
    }

    /// Our own voice state changed on Discord's side.
    fn on_state(&self, user: Snowflake, channel: Option<Snowflake>) {
        let mut inner = self.lock();
        if inner.user != Some(user) {
            return;
        }
        match (channel, inner.want) {
            // Moved by someone else: follow, the server will name a new voice server.
            (Some(channel), Some(want)) if want.channel != channel => {
                inner.want = Some(Want { channel, ..want });
            }
            // Removed from the call while we thought we were in it.
            (None, Some(_)) if inner.call.is_some() => {
                inner.want = None;
                inner.generation += 1;
                if let Some(call) = inner.call.take() {
                    call.leave();
                }
                drop(inner);
                self.tell(VoiceNotice::Left(Some("You were disconnected from the call.".into())));
            }
            _ => {}
        }
    }

    fn on_closed(&self, generation: u64, code: Option<u16>, reason: &str) {
        let mut inner = self.lock();
        if inner.generation != generation {
            return;
        }
        inner.call = None;
        if inner.want.take().is_none() {
            return;
        }
        let (mute, deafen) = (inner.mute, inner.deafen);
        drop(inner);

        // Tell Discord we are gone too, or it would keep us in the channel.
        self.gateway.set_voice_state(None, None, mute, deafen);
        self.tell(VoiceNotice::Left(Some(describe_close(code, reason))));
    }

    pub fn join(&self, guild: Option<Snowflake>, channel: Snowflake) {
        let mut inner = self.lock();
        inner.generation += 1;
        if let Some(call) = inner.call.take() {
            call.leave();
        }
        inner.want = Some(Want { guild, channel });
        let (mute, deafen) = (inner.mute, inner.deafen);
        drop(inner);
        self.gateway.set_voice_state(guild, Some(channel), mute, deafen);
    }

    pub fn leave(&self) {
        let mut inner = self.lock();
        inner.generation += 1;
        if let Some(call) = inner.call.take() {
            call.leave();
        }
        let was_in = inner.want.take().is_some();
        let (mute, deafen) = (inner.mute, inner.deafen);
        drop(inner);
        if was_in {
            self.gateway.set_voice_state(None, None, mute, deafen);
        }
        self.tell(VoiceNotice::Left(None));
    }

    pub fn set_flags(&self, mute: bool, deafen: bool) {
        let mut inner = self.lock();
        inner.mute = mute;
        inner.deafen = deafen;
        if let Some(call) = &inner.call {
            call.set_deafened(deafen);
            call.set_muted(mute || deafen);
        }
        let want = inner.want;
        drop(inner);
        if let Some(want) = want {
            self.gateway.set_voice_state(want.guild, Some(want.channel), mute, deafen);
        }
    }

    /// Picks the microphone and speakers, for this call and the next ones.
    pub fn set_devices(&self, input: Option<String>, output: Option<String>) {
        let mut inner = self.lock();
        let (input_changed, output_changed) = (inner.input_device != input, inner.output_device != output);
        inner.input_device = input.clone();
        inner.output_device = output.clone();
        if let Some(call) = &inner.call {
            if input_changed {
                call.set_input_device(input);
            }
            if output_changed {
                call.set_output_device(output);
            }
        }
    }

    /// Sends the microphone at this percent of its level, for this call and
    /// the next ones.
    pub fn set_input_volume(&self, percent: u8) {
        let mut inner = self.lock();
        inner.input_volume = percent.min(100);
        if let Some(call) = &inner.call {
            call.set_input_volume(inner.input_volume);
        }
    }

    /// Cleans the microphone of background noise, for this call and the next
    /// ones.
    pub fn set_noise_suppression(&self, on: bool) {
        let mut inner = self.lock();
        inner.noise_suppression = on;
        if let Some(call) = &inner.call {
            call.set_noise_suppression(on);
        }
    }

    /// Plays one person at this percent of their level, for this call and
    /// the next ones.
    pub fn set_user_volume(&self, user: Snowflake, percent: u16) {
        let mut inner = self.lock();
        inner.user_volumes.insert(user.0, percent);
        if let Some(call) = &inner.call {
            call.set_user_volume(user.0, percent);
        }
    }

    /// Turns push to talk on with this key, or off for `None`. The key is
    /// watched even while another program has the focus.
    pub fn set_push_to_talk(&self, key: Option<String>) {
        let mut inner = self.lock();
        if inner.talk_key == key {
            return;
        }
        if let Some(stop) = inner.watcher.take() {
            stop.store(true, Ordering::Relaxed);
        }
        inner.talk_key = key.clone();
        inner.talk_key_down = false;
        if let Some(call) = &inner.call {
            call.set_push_to_talk(key.is_some());
            call.set_talk_key(false);
        }
        let Some(code) = key.as_deref().and_then(oxicord_ui::talk_key::virtual_key) else { return };
        let stop = Arc::new(AtomicBool::new(false));
        inner.watcher = Some(stop.clone());
        drop(inner);

        let this = self.clone();
        std::thread::spawn(move || {
            let mut held = false;
            while !stop.load(Ordering::Relaxed) {
                let down = crate::hotkey::is_down(code);
                if down != held {
                    held = down;
                    this.set_talk_key_down(down);
                }
                std::thread::sleep(Duration::from_millis(8));
            }
        });
    }

    fn set_talk_key_down(&self, down: bool) {
        let mut inner = self.lock();
        inner.talk_key_down = down;
        if let Some(call) = &inner.call {
            call.set_talk_key(down);
        }
    }

    /// Ends the call without telling anyone, for when the session goes away.
    pub fn shutdown(&self) {
        let mut inner = self.lock();
        if let Some(stop) = inner.watcher.take() {
            stop.store(true, Ordering::Relaxed);
        }
        inner.talk_key = None;
        inner.generation += 1;
        inner.want = None;
        if let Some(call) = inner.call.take() {
            call.leave();
        }
    }
}

/// What to tell the user when a call ends on its own.
fn describe_close(code: Option<u16>, reason: &str) -> String {
    match code {
        Some(4014) => "You were disconnected from the call.".into(),
        Some(4006) => "The voice session is no longer valid. Join the call again.".into(),
        Some(4015) => "The voice server went down. Join the call again.".into(),
        Some(4017) => "Discord requires end-to-end encryption for this call, which could not be set up.".into(),
        Some(code) if reason.is_empty() => format!("The call ended (code {code})."),
        _ if reason.is_empty() => "The call ended.".into(),
        _ => reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(guild: Option<u64>, channel: Option<u64>) -> VoiceServer {
        serde_json::from_value(serde_json::json!({
            "token": "t",
            "guild_id": guild.map(|g| g.to_string()),
            "channel_id": channel.map(|c| c.to_string()),
            "endpoint": "x.discord.media:443",
        }))
        .unwrap()
    }

    #[test]
    fn a_guild_join_is_answered_by_that_guilds_server() {
        let want = Want { guild: Some(Snowflake(1)), channel: Snowflake(2) };
        assert!(want.is_answered_by(&server(Some(1), None)));
        assert!(!want.is_answered_by(&server(Some(9), None)));
        assert_eq!(want.server_id(), Snowflake(1));
    }

    #[test]
    fn a_private_call_is_answered_by_its_channel() {
        let want = Want { guild: None, channel: Snowflake(2) };
        assert!(want.is_answered_by(&server(None, Some(2))));
        assert!(!want.is_answered_by(&server(None, Some(3))));
        assert!(!want.is_answered_by(&server(Some(1), Some(2))));
        assert_eq!(want.server_id(), Snowflake(2));
    }

    #[test]
    fn close_codes_become_sentences() {
        assert!(describe_close(Some(4014), "").contains("disconnected"));
        assert!(describe_close(Some(4017), "").contains("encryption"));
        assert_eq!(describe_close(Some(4000), ""), "The call ended (code 4000).");
        assert_eq!(describe_close(None, "boom"), "boom");
    }
}
