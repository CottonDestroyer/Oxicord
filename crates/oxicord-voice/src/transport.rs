//! The transport layer of a voice call: RTP framing and the AEAD that wraps
//! each packet. Everything here is pure, so it is covered by plain tests.

use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit, Payload},
};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

/// The only RTP payload type Discord uses for audio.
pub const OPUS_PAYLOAD_TYPE: u8 = 0x78;
const RTP_HEADER: usize = 12;
const NONCE_SUFFIX: usize = 4;
const TAG: usize = 16;

/// The two `*_rtpsize` modes. Discord requires the xchacha one and prefers
/// AES-GCM when we offer both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Aes256Gcm,
    XChaCha20Poly1305,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Aes256Gcm => "aead_aes256_gcm_rtpsize",
            Self::XChaCha20Poly1305 => "aead_xchacha20_poly1305_rtpsize",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aead_aes256_gcm_rtpsize" => Some(Self::Aes256Gcm),
            "aead_xchacha20_poly1305_rtpsize" => Some(Self::XChaCha20Poly1305),
            _ => None,
        }
    }

    /// The best mode out of what the server offers.
    pub fn pick(offered: &[String]) -> Option<Self> {
        let has = |mode: Self| offered.iter().any(|name| name == mode.name());
        [Self::Aes256Gcm, Self::XChaCha20Poly1305].into_iter().find(|&m| has(m))
    }
}

/// What the unencrypted header of a packet tells us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    /// Bytes that stay in the clear: the fixed header, any CSRCs and the
    /// extension preamble. Those are the associated data of the AEAD.
    pub clear_len: usize,
    /// Words in the header extension. Its body sits at the start of the
    /// encrypted payload and has to be skipped after decrypting.
    pub extension_words: usize,
    pub padded: bool,
}

pub fn parse_header(packet: &[u8]) -> Option<Header> {
    if packet.len() < RTP_HEADER || packet[0] >> 6 != 2 {
        return None;
    }
    let csrcs = usize::from(packet[0] & 0x0f);
    let extension = packet[0] & 0x10 != 0;
    let mut clear_len = RTP_HEADER + csrcs * 4;
    let mut extension_words = 0;
    if extension {
        let preamble = packet.get(clear_len..clear_len + 4)?;
        extension_words = usize::from(u16::from_be_bytes([preamble[2], preamble[3]]));
        clear_len += 4;
    }
    Some(Header {
        payload_type: packet[1] & 0x7f,
        sequence: u16::from_be_bytes([packet[2], packet[3]]),
        timestamp: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
        ssrc: u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]),
        clear_len,
        extension_words,
        padded: packet[0] & 0x20 != 0,
    })
}

pub struct Cipher {
    mode: Mode,
    key: [u8; 32],
}

impl Cipher {
    pub fn new(mode: Mode, key: &[u8]) -> Option<Self> {
        Some(Self { mode, key: key.try_into().ok()? })
    }

    /// Decrypts a received packet and returns its header with the Opus
    /// payload, once the extension body and any padding are gone.
    pub fn open(&self, packet: &[u8]) -> Option<(Header, Vec<u8>)> {
        let header = parse_header(packet)?;
        if packet.len() < header.clear_len + TAG + NONCE_SUFFIX {
            return None;
        }
        let (body, suffix) = packet.split_at(packet.len() - NONCE_SUFFIX);
        let (aad, sealed) = body.split_at(header.clear_len);
        let mut plain = self.decrypt(suffix, sealed, aad)?;

        let skip = header.extension_words * 4;
        if skip > plain.len() {
            return None;
        }
        plain.drain(..skip);
        if header.padded {
            let pad = usize::from(*plain.last()?);
            plain.truncate(plain.len().checked_sub(pad)?);
        }
        Some((header, plain))
    }

    /// Builds a packet to send: the 12 byte header in the clear, then the
    /// sealed payload and the 4 byte nonce counter.
    pub fn seal(&self, header: [u8; RTP_HEADER], payload: &[u8], counter: u32) -> Option<Vec<u8>> {
        let suffix = counter.to_be_bytes();
        let sealed = self.encrypt(&suffix, payload, &header)?;
        let mut packet = Vec::with_capacity(RTP_HEADER + sealed.len() + NONCE_SUFFIX);
        packet.extend_from_slice(&header);
        packet.extend_from_slice(&sealed);
        packet.extend_from_slice(&suffix);
        Some(packet)
    }

    // The 4 byte counter sits at the front of a nonce that is otherwise zero.
    fn decrypt(&self, suffix: &[u8], sealed: &[u8], aad: &[u8]) -> Option<Vec<u8>> {
        let payload = Payload { msg: sealed, aad };
        match self.mode {
            Mode::Aes256Gcm => {
                let mut nonce = [0u8; 12];
                nonce[..NONCE_SUFFIX].copy_from_slice(suffix);
                Aes256Gcm::new_from_slice(&self.key)
                    .ok()?
                    .decrypt(aes_gcm::Nonce::from_slice(&nonce), payload)
                    .ok()
            }
            Mode::XChaCha20Poly1305 => {
                let mut nonce = [0u8; 24];
                nonce[..NONCE_SUFFIX].copy_from_slice(suffix);
                XChaCha20Poly1305::new_from_slice(&self.key)
                    .ok()?
                    .decrypt(XNonce::from_slice(&nonce), payload)
                    .ok()
            }
        }
    }

    fn encrypt(&self, suffix: &[u8], plain: &[u8], aad: &[u8]) -> Option<Vec<u8>> {
        let payload = Payload { msg: plain, aad };
        match self.mode {
            Mode::Aes256Gcm => {
                let mut nonce = [0u8; 12];
                nonce[..NONCE_SUFFIX].copy_from_slice(suffix);
                Aes256Gcm::new_from_slice(&self.key)
                    .ok()?
                    .encrypt(aes_gcm::Nonce::from_slice(&nonce), payload)
                    .ok()
            }
            Mode::XChaCha20Poly1305 => {
                let mut nonce = [0u8; 24];
                nonce[..NONCE_SUFFIX].copy_from_slice(suffix);
                XChaCha20Poly1305::new_from_slice(&self.key)
                    .ok()?
                    .encrypt(XNonce::from_slice(&nonce), payload)
                    .ok()
            }
        }
    }
}

/// The 12 byte header of an outgoing audio packet.
pub fn audio_header(sequence: u16, timestamp: u32, ssrc: u32) -> [u8; RTP_HEADER] {
    let mut header = [0u8; RTP_HEADER];
    header[0] = 0x80;
    header[1] = OPUS_PAYLOAD_TYPE;
    header[2..4].copy_from_slice(&sequence.to_be_bytes());
    header[4..8].copy_from_slice(&timestamp.to_be_bytes());
    header[8..12].copy_from_slice(&ssrc.to_be_bytes());
    header
}

/// The probe sent to learn our external address, 74 bytes long.
pub fn discovery_request(ssrc: u32) -> [u8; 74] {
    let mut packet = [0u8; 74];
    packet[..2].copy_from_slice(&1u16.to_be_bytes());
    packet[2..4].copy_from_slice(&70u16.to_be_bytes());
    packet[4..8].copy_from_slice(&ssrc.to_be_bytes());
    packet
}

/// Our external address and port out of the discovery answer.
pub fn parse_discovery(packet: &[u8]) -> Option<(String, u16)> {
    if packet.len() < 74 || u16::from_be_bytes([packet[0], packet[1]]) != 2 {
        return None;
    }
    let address = &packet[8..72];
    let end = address.iter().position(|&b| b == 0).unwrap_or(address.len());
    let address = std::str::from_utf8(&address[..end]).ok()?.to_owned();
    let port = u16::from_be_bytes([packet[72], packet[73]]);
    Some((address, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cipher(mode: Mode) -> Cipher {
        Cipher::new(mode, &[7u8; 32]).unwrap()
    }

    #[test]
    fn both_modes_round_trip_a_packet() {
        for mode in [Mode::Aes256Gcm, Mode::XChaCha20Poly1305] {
            let c = cipher(mode);
            let packet = c.seal(audio_header(9, 960, 42), b"opus bytes", 5).unwrap();
            let (header, payload) = c.open(&packet).unwrap();
            assert_eq!((header.sequence, header.timestamp, header.ssrc), (9, 960, 42));
            assert_eq!(header.payload_type, OPUS_PAYLOAD_TYPE);
            assert_eq!(payload, b"opus bytes");
        }
    }

    #[test]
    fn a_tampered_header_fails_authentication() {
        let c = cipher(Mode::Aes256Gcm);
        let mut packet = c.seal(audio_header(1, 2, 3), b"hi", 0).unwrap();
        packet[3] ^= 1;
        assert!(c.open(&packet).is_none());
    }

    #[test]
    fn the_extension_body_is_skipped_after_decrypting() {
        let c = cipher(Mode::XChaCha20Poly1305);
        // Header with the extension bit set, one CSRC-free preamble of one word.
        let mut clear = audio_header(1, 2, 3).to_vec();
        clear[0] |= 0x10;
        clear.extend_from_slice(&[0xbe, 0xde, 0, 1]);
        let mut plain = vec![0xaa, 0xbb, 0xcc, 0xdd];
        plain.extend_from_slice(b"voice");
        let sealed = c.encrypt(&0u32.to_be_bytes(), &plain, &clear).unwrap();
        let mut packet = clear;
        packet.extend_from_slice(&sealed);
        packet.extend_from_slice(&0u32.to_be_bytes());

        let (header, payload) = c.open(&packet).unwrap();
        assert_eq!(header.extension_words, 1);
        assert_eq!(payload, b"voice");
    }

    #[test]
    fn short_or_foreign_packets_are_ignored() {
        let c = cipher(Mode::Aes256Gcm);
        assert!(c.open(&[0u8; 8]).is_none());
        assert!(c.open(&[0x40; 40]).is_none());
    }

    #[test]
    fn the_best_offered_mode_wins() {
        let offered = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            Mode::pick(&offered(&["aead_xchacha20_poly1305_rtpsize", "aead_aes256_gcm_rtpsize"])),
            Some(Mode::Aes256Gcm)
        );
        assert_eq!(
            Mode::pick(&offered(&["aead_xchacha20_poly1305_rtpsize"])),
            Some(Mode::XChaCha20Poly1305)
        );
        assert_eq!(Mode::pick(&offered(&["xsalsa20_poly1305"])), None);
    }

    #[test]
    fn discovery_packets_have_the_documented_layout() {
        let request = discovery_request(0x0102_0304);
        assert_eq!(&request[..8], &[0, 1, 0, 70, 1, 2, 3, 4]);

        let mut answer = [0u8; 74];
        answer[1] = 2;
        answer[8..19].copy_from_slice(b"203.0.113.9");
        answer[72..74].copy_from_slice(&50000u16.to_be_bytes());
        assert_eq!(parse_discovery(&answer), Some(("203.0.113.9".to_owned(), 50000)));
        assert_eq!(parse_discovery(&request), None);
    }
}
