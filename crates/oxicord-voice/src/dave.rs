//! Discord's end-to-end encryption for calls (DAVE). The MLS group and the
//! frame cipher come from the `davey` crate, this adapts it to the voice
//! gateway's messages.

use std::num::NonZeroU16;

use davey::{DaveSession, MediaType, ProposalsOperationType};
use serde_json::{Value, json};

/// The highest DAVE version we speak.
pub const MAX_VERSION: u16 = 1;

const OP_READY_FOR_TRANSITION: u8 = 23;
const OP_KEY_PACKAGE: u8 = 26;
const OP_COMMIT_WELCOME: u8 = 28;
const OP_INVALID_COMMIT: u8 = 31;

/// A message to send back to the voice gateway.
#[derive(Debug, PartialEq)]
pub enum Out {
    Json(Value),
    /// An opcode byte followed by its payload.
    Binary(Vec<u8>),
}

pub struct Dave {
    user_id: u64,
    channel_id: u64,
    session: Option<DaveSession>,
}

impl Dave {
    pub fn new(user_id: u64, channel_id: u64) -> Self {
        Self { user_id, channel_id, session: None }
    }

    /// Whether this call is end-to-end encrypted at the moment.
    pub fn is_encrypted(&self) -> bool {
        self.session.as_ref().is_some_and(DaveSession::is_ready)
    }

    /// The code two people compare out of band to be sure nobody is in the
    /// middle.
    pub fn privacy_code(&self) -> Option<String> {
        self.session.as_ref()?.voice_privacy_code().map(str::to_owned)
    }

    /// The session description named the protocol version of this call.
    pub fn start(&mut self, version: u16) -> Vec<Out> {
        let Some(version) = NonZeroU16::new(version) else {
            self.session = None;
            return Vec::new();
        };
        match DaveSession::new(version, self.user_id, self.channel_id, None) {
            Ok(mut session) => {
                // Until keys exist, frames from peers who have not switched
                // yet must still get through.
                session.set_passthrough_mode(true, None);
                self.session = Some(session);
                self.key_package()
            }
            Err(err) => {
                tracing::warn!(%err, "could not start a DAVE session");
                Vec::new()
            }
        }
    }

    fn key_package(&mut self) -> Vec<Out> {
        let Some(session) = self.session.as_mut() else { return Vec::new() };
        match session.create_key_package() {
            Ok(package) => vec![binary(OP_KEY_PACKAGE, &[&package])],
            Err(err) => {
                tracing::warn!(%err, "could not build a key package");
                Vec::new()
            }
        }
    }

    /// Op 24: a new group is coming when the epoch is 1.
    pub fn prepare_epoch(&mut self, version: u16, epoch: u64) -> Vec<Out> {
        if epoch != 1 {
            return Vec::new();
        }
        let (user, channel) = (self.user_id, self.channel_id);
        let Some(version) = NonZeroU16::new(version) else { return Vec::new() };
        match self.session.as_mut() {
            Some(session) => {
                if let Err(err) = session.reinit(version, user, channel, None) {
                    tracing::warn!(%err, "could not restart the DAVE session");
                    return Vec::new();
                }
                self.key_package()
            }
            None => self.start(version.get()),
        }
    }

    /// Op 21.
    pub fn prepare_transition(&mut self, transition: u16, version: u16) -> Vec<Out> {
        let Some(session) = self.session.as_mut() else { return Vec::new() };
        if let Err(err) = session.prepare_transition(transition, version) {
            tracing::warn!(%err, "could not prepare a DAVE transition");
            return Vec::new();
        }
        if version == 0 {
            session.set_passthrough_mode(true, None);
        }
        ready(transition)
    }

    /// Op 22.
    pub fn execute_transition(&mut self, transition: u16) {
        let Some(session) = self.session.as_mut() else { return };
        match session.execute_transition(transition) {
            Ok(true) if session.is_ready() => session.set_passthrough_mode(false, None),
            Ok(_) => {}
            Err(err) => tracing::warn!(%err, "could not execute a DAVE transition"),
        }
    }

    /// Op 25: who the group's external sender is.
    pub fn external_sender(&mut self, payload: &[u8]) {
        let Some(session) = self.session.as_mut() else { return };
        if let Err(err) = session.set_external_sender(payload) {
            tracing::warn!(%err, "bad external sender");
        }
    }

    /// Op 27: proposals to add or remove members. We answer with a commit.
    pub fn proposals(&mut self, payload: &[u8]) -> Vec<Out> {
        let Some((&kind, body)) = payload.split_first() else { return Vec::new() };
        let kind = if kind == 0 { ProposalsOperationType::APPEND } else { ProposalsOperationType::REVOKE };
        let Some(session) = self.session.as_mut() else { return Vec::new() };
        match session.process_proposals(kind, body, None) {
            Ok(Some(commit)) => {
                let welcome = commit.welcome.unwrap_or_default();
                vec![binary(OP_COMMIT_WELCOME, &[&commit.commit, &welcome])]
            }
            Ok(None) => Vec::new(),
            Err(err) => {
                tracing::warn!(%err, "could not process proposals");
                self.recover(0)
            }
        }
    }

    /// Op 29: the commit the gateway picked for the next epoch.
    pub fn announce_commit(&mut self, payload: &[u8]) -> Vec<Out> {
        let Some((transition, commit)) = split_transition(payload) else { return Vec::new() };
        let Some(session) = self.session.as_mut() else { return Vec::new() };
        match session.process_commit(commit) {
            Ok(true) => {
                let version = session.protocol_version().get();
                self.prepare_transition(transition, version)
            }
            Ok(false) => Vec::new(),
            Err(err) => {
                tracing::warn!(%err, "could not process a commit");
                self.recover(transition)
            }
        }
    }

    /// Op 30: we were added to a group and got its welcome.
    pub fn welcome(&mut self, payload: &[u8]) -> Vec<Out> {
        let Some((transition, welcome)) = split_transition(payload) else { return Vec::new() };
        let Some(session) = self.session.as_mut() else { return Vec::new() };
        match session.process_welcome(welcome, None) {
            Ok(()) => {
                let version = session.protocol_version().get();
                self.prepare_transition(transition, version)
            }
            Err(err) => {
                tracing::warn!(%err, "could not process a welcome");
                self.recover(transition)
            }
        }
    }

    /// Tells the gateway the commit or welcome was no good, drops our group
    /// and offers a fresh key package so we get added again.
    fn recover(&mut self, transition: u16) -> Vec<Out> {
        let (user, channel) = (self.user_id, self.channel_id);
        let mut out = vec![Out::Json(json!({ "op": OP_INVALID_COMMIT, "d": { "transition_id": transition } }))];
        if let Some(session) = self.session.as_mut() {
            let version = session.protocol_version();
            if session.reinit(version, user, channel, None).is_ok() {
                out.extend(self.key_package());
            }
        }
        out
    }

    /// Wraps an Opus packet of ours for sending. `None` means it cannot go
    /// out yet: in an encrypted call we have no key until the group is set up,
    /// and plain audio must never be sent in its place.
    pub fn seal(&mut self, opus: &[u8]) -> Option<Vec<u8>> {
        let Some(session) = self.session.as_mut() else {
            return Some(opus.to_vec());
        };
        if !session.can_encrypt() {
            return None;
        }
        session.encrypt_opus(opus).ok().map(std::borrow::Cow::into_owned)
    }

    /// Turns a received media payload into plain Opus. `None` means the
    /// frame cannot be read and is dropped.
    pub fn open(&mut self, sender: Option<u64>, payload: Vec<u8>) -> Option<Vec<u8>> {
        let Some(session) = self.session.as_mut() else {
            return Some(payload);
        };
        // Still negotiating: what arrives is encrypted for a group we have
        // not joined, and decoding it as Opus would only make noise.
        if !session.is_ready() {
            return None;
        }
        session.decrypt(sender?, MediaType::AUDIO, &payload).ok()
    }
}

fn ready(transition: u16) -> Vec<Out> {
    if transition == 0 {
        return Vec::new();
    }
    vec![Out::Json(json!({ "op": OP_READY_FOR_TRANSITION, "d": { "transition_id": transition } }))]
}

fn binary(op: u8, parts: &[&[u8]]) -> Out {
    let mut bytes = vec![op];
    for part in parts {
        bytes.extend_from_slice(part);
    }
    Out::Binary(bytes)
}

fn split_transition(payload: &[u8]) -> Option<(u16, &[u8])> {
    let (id, rest) = payload.split_at_checked(2)?;
    Some((u16::from_be_bytes([id[0], id[1]]), rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_zero_means_no_encryption_and_frames_pass_untouched() {
        let mut dave = Dave::new(1, 2);
        assert!(dave.start(0).is_empty());
        assert!(!dave.is_encrypted());
        assert_eq!(dave.open(None, vec![1, 2, 3]), Some(vec![1, 2, 3]));
    }

    #[test]
    fn starting_a_session_offers_a_key_package() {
        let mut dave = Dave::new(1, 2);
        let out = dave.start(1);
        assert_eq!(out.len(), 1);
        match &out[0] {
            Out::Binary(bytes) => assert_eq!(bytes[0], 26),
            other => panic!("expected a binary key package, got {other:?}"),
        }
        // No group yet, so nothing can be read and nothing is decoded.
        assert!(!dave.is_encrypted());
        assert_eq!(dave.open(Some(7), vec![1, 2, 3]), None);
    }

    #[test]
    fn audio_is_never_sent_in_the_clear_while_keys_are_pending() {
        let mut plain = Dave::new(1, 2);
        plain.start(0);
        assert_eq!(plain.seal(&[1, 2, 3]), Some(vec![1, 2, 3]));

        let mut waiting = Dave::new(1, 2);
        waiting.start(1);
        assert_eq!(waiting.seal(&[1, 2, 3]), None);
    }

    #[test]
    fn ready_is_not_sent_for_the_immediate_transition() {
        assert!(ready(0).is_empty());
        assert_eq!(ready(4), vec![Out::Json(json!({ "op": 23, "d": { "transition_id": 4 } }))]);
    }

    #[test]
    fn transition_ids_are_read_big_endian() {
        assert_eq!(split_transition(&[1, 2, 9, 9]), Some((258, &[9, 9][..])));
        assert_eq!(split_transition(&[1]), None);
    }

    #[test]
    fn a_bad_welcome_reports_the_transition_and_offers_a_new_key_package() {
        let mut dave = Dave::new(1, 2);
        dave.start(1);
        let out = dave.welcome(&[0, 5, 0xff, 0xff]);
        assert_eq!(out[0], Out::Json(json!({ "op": 31, "d": { "transition_id": 5 } })));
        assert!(matches!(&out[1], Out::Binary(b) if b[0] == 26));
    }
}
