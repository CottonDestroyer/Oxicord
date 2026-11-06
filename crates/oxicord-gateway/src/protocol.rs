//! The parts of the gateway protocol that need no socket: payload shapes,
//! close code policy, heartbeat bookkeeping and reconnect timing.

use std::time::Duration;

use oxicord_api::super_props::SuperProperties;
use serde_json::{Value, json};

use oxicord_model::{Snowflake, gateway::op};

pub const DEFAULT_GATEWAY: &str = "wss://gateway.discord.gg";

pub fn gateway_url(base: &str) -> String {
    format!(
        "{}/?v=10&encoding=json&compress=zlib-stream",
        base.trim_end_matches('/')
    )
}

/// The status a user can pick for themselves. Differs from the status we
/// *receive* for others because invisible is only ever outgoing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClientStatus {
    #[default]
    Online,
    Idle,
    Dnd,
    Invisible,
}

impl ClientStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Idle => "idle",
            Self::Dnd => "dnd",
            Self::Invisible => "invisible",
        }
    }
}

/// The IDENTIFY a normal user session sends.
///
/// This deliberately has no `capabilities` and no `client_state`, even though
/// the official web client sends both. Live testing showed Discord revoking
/// the token of a session that identified the way the web client does, and
/// not one that identified like this. Keep it this plain.
pub fn identify(token: &str, props: &SuperProperties, status: ClientStatus) -> Value {
    json!({
        "op": op::IDENTIFY,
        "d": {
            "token": token,
            "compress": false,
            "properties": props.to_value(),
            "presence": presence_body(status),
        }
    })
}

pub fn resume(token: &str, session_id: &str, seq: Option<u64>) -> Value {
    json!({
        "op": op::RESUME,
        "d": { "token": token, "session_id": session_id, "seq": seq }
    })
}

pub fn heartbeat(seq: Option<u64>) -> Value {
    json!({ "op": op::HEARTBEAT, "d": seq })
}

/// Asks for a guild's member list. Discord ignores channel ranges until the
/// guild has been subscribed by an earlier frame, so this is sent twice: once
/// with `channel` as `None` to subscribe, then again with the channel to ask
/// for its first hundred members.
pub fn member_subscription(guild: Snowflake, channel: Option<Snowflake>) -> Value {
    let channels = match channel {
        Some(channel) => json!({ channel.to_string(): [[0, 99]] }),
        None => json!({}),
    };
    json!({
        "op": op::GUILD_SUBSCRIPTIONS_BULK,
        "d": { "subscriptions": { guild.to_string(): {
            "typing": true,
            "threads": false,
            "activities": true,
            "member_updates": false,
            "members": [],
            "channels": channels,
            "thread_member_lists": [],
        } } }
    })
}

/// Asks for specific people of a guild. The official client sends the guild
/// as a list and no more than a hundred ids per frame.
pub fn request_members(guild: Snowflake, user_ids: &[Snowflake]) -> Value {
    json!({
        "op": op::REQUEST_GUILD_MEMBERS,
        "d": {
            "guild_id": [guild.to_string()],
            "user_ids": user_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "presences": false,
        }
    })
}

pub fn presence_update(status: ClientStatus) -> Value {
    json!({ "op": op::PRESENCE_UPDATE, "d": presence_body(status) })
}

fn presence_body(status: ClientStatus) -> Value {
    json!({
        "status": status.as_str(),
        "since": 0,
        "activities": [],
        "afk": false,
    })
}

/// What to do after the server closes the socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAction {
    /// Reconnect and RESUME, keeping the session.
    Resume,
    /// Reconnect and IDENTIFY from scratch.
    Reidentify,
    /// Stop. Retrying cannot help.
    Fatal,
}

/// `None` means the connection dropped without a close frame, which is a
/// network problem and worth resuming from.
pub fn close_action(code: Option<u16>) -> CloseAction {
    match code {
        // Auth failed, bad shard, bad version, bad or disallowed intents.
        Some(4004 | 4010..=4014) => CloseAction::Fatal,
        // A clean close ends the session on Discord's side. Invalid seq and
        // session timeout mean the same.
        Some(1000 | 1001 | 4007 | 4009) => CloseAction::Reidentify,
        _ => CloseAction::Resume,
    }
}

/// Tracks whether the previous heartbeat was acknowledged. Two beats in a
/// row without an ack means the connection is a zombie.
#[derive(Debug, Default)]
pub struct Heartbeat {
    awaiting_ack: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BeatAction {
    Send,
    Zombie,
}

impl Heartbeat {
    pub fn tick(&mut self) -> BeatAction {
        if self.awaiting_ack {
            return BeatAction::Zombie;
        }
        self.awaiting_ack = true;
        BeatAction::Send
    }

    /// The server asked for a beat out of schedule (op 1).
    pub fn forced(&mut self) {
        self.awaiting_ack = true;
    }

    pub fn ack(&mut self) {
        self.awaiting_ack = false;
    }
}

/// Doubling delay between reconnect attempts, capped at a minute.
#[derive(Clone, Debug)]
pub struct Backoff {
    base: Duration,
    attempt: u32,
}

const MAX_BACKOFF: Duration = Duration::from_secs(60);

impl Backoff {
    pub fn new(base: Duration) -> Self {
        Self { base, attempt: 0 }
    }

    pub fn next_delay(&mut self) -> Duration {
        let factor = 1u32 << self.attempt.min(16);
        self.attempt = self.attempt.saturating_add(1);
        self.base.saturating_mul(factor).min(MAX_BACKOFF)
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_request_names_the_guild_in_a_list_and_ids_as_text() {
        let frame = request_members(Snowflake(7), &[Snowflake(1), Snowflake(2)]);
        assert_eq!(frame["op"], 8);
        assert_eq!(frame["d"]["guild_id"], json!(["7"]));
        assert_eq!(frame["d"]["user_ids"], json!(["1", "2"]));
        assert_eq!(frame["d"]["presences"], false);
    }

    #[test]
    fn url_carries_version_encoding_and_compression() {
        assert_eq!(
            gateway_url("wss://gateway.discord.gg/"),
            "wss://gateway.discord.gg/?v=10&encoding=json&compress=zlib-stream"
        );
    }

    #[test]
    fn identify_is_exactly_the_plain_user_shape() {
        let v = identify("tok", &SuperProperties::default(), ClientStatus::Idle);
        assert_eq!(v["op"], 2);

        let mut keys: Vec<_> = v["d"].as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["compress", "presence", "properties", "token"]);

        assert_eq!(v["d"]["token"], "tok");
        assert_eq!(v["d"]["compress"], false);
        assert_eq!(v["d"]["presence"]["status"], "idle");
        assert!(v["d"]["properties"]["client_build_number"].is_number());
    }

    #[test]
    fn member_subscription_has_the_shape_discord_expects() {
        let subscribe = member_subscription(Snowflake(5), None);
        assert_eq!(subscribe["op"], 37);
        let guild = &subscribe["d"]["subscriptions"]["5"];
        assert_eq!(guild["typing"], true);
        assert_eq!(guild["channels"], serde_json::json!({}));

        let ranged = member_subscription(Snowflake(5), Some(Snowflake(9)));
        assert_eq!(ranged["d"]["subscriptions"]["5"]["channels"]["9"], serde_json::json!([[0, 99]]));
    }

    #[test]
    fn resume_carries_session_and_sequence() {
        let v = resume("tok", "sess", Some(41));
        assert_eq!(v["op"], 6);
        assert_eq!(v["d"]["session_id"], "sess");
        assert_eq!(v["d"]["seq"], 41);
    }

    #[test]
    fn first_heartbeat_has_null_sequence() {
        assert!(heartbeat(None)["d"].is_null());
        assert_eq!(heartbeat(Some(7))["d"], 7);
    }

    #[test]
    fn close_codes_map_to_the_right_policy() {
        assert_eq!(close_action(Some(4004)), CloseAction::Fatal);
        assert_eq!(close_action(Some(4014)), CloseAction::Fatal);
        assert_eq!(close_action(Some(4007)), CloseAction::Reidentify);
        assert_eq!(close_action(Some(4009)), CloseAction::Reidentify);
        assert_eq!(close_action(Some(1000)), CloseAction::Reidentify);
        assert_eq!(close_action(Some(4000)), CloseAction::Resume);
        assert_eq!(close_action(Some(4008)), CloseAction::Resume);
        assert_eq!(close_action(None), CloseAction::Resume);
    }

    #[test]
    fn missed_ack_is_a_zombie() {
        let mut hb = Heartbeat::default();
        assert_eq!(hb.tick(), BeatAction::Send);
        assert_eq!(hb.tick(), BeatAction::Zombie);
    }

    #[test]
    fn ack_clears_the_wait() {
        let mut hb = Heartbeat::default();
        hb.tick();
        hb.ack();
        assert_eq!(hb.tick(), BeatAction::Send);
    }

    #[test]
    fn backoff_doubles_then_caps_and_resets() {
        let mut b = Backoff::new(Duration::from_secs(1));
        let delays: Vec<u64> = (0..8).map(|_| b.next_delay().as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 32, 60, 60]);

        b.reset();
        assert_eq!(b.attempt(), 0);
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }
}
