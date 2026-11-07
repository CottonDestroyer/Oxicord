//! A small safe wrapper over the pure-Rust libopus transpile: decoding the
//! stereo 48 kHz frames Discord sends and encoding our microphone.

use unsafe_libopus::{
    OPUS_APPLICATION_VOIP, OpusDecoder, OpusEncoder, opus_decode, opus_decoder_create, opus_decoder_destroy,
    opus_encode, opus_encoder_create, opus_encoder_destroy,
};

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
/// Samples in one 20 ms frame, per channel.
pub const FRAME: usize = 960;
/// The longest frame Opus can produce, per channel.
const MAX_FRAME: usize = 5760;
/// Room for the largest packet we will send.
const MAX_PACKET: usize = 1275;

/// Encodes mono speech, one 20 ms frame at a time.
pub struct Encoder {
    raw: *mut OpusEncoder,
}

// The encoder is only ever touched from the task that owns it.
unsafe impl Send for Encoder {}

impl Encoder {
    pub fn new() -> Option<Self> {
        let mut error = 0;
        // SAFETY: valid arguments, and a null result is checked below.
        let raw = unsafe { opus_encoder_create(SAMPLE_RATE as i32, 1, OPUS_APPLICATION_VOIP, &mut error) };
        (!raw.is_null() && error == 0).then_some(Self { raw })
    }

    /// `pcm` must hold exactly one frame of mono samples.
    pub fn encode(&mut self, pcm: &[i16]) -> Option<Vec<u8>> {
        if pcm.len() != FRAME {
            return None;
        }
        let mut packet = vec![0u8; MAX_PACKET];
        // SAFETY: `pcm` is one full frame and `packet` has the room we claim.
        let len = unsafe { opus_encode(self.raw, pcm.as_ptr(), FRAME as i32, packet.as_mut_ptr(), MAX_PACKET as i32) };
        let len = usize::try_from(len).ok().filter(|&n| n > 0)?;
        packet.truncate(len);
        Some(packet)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `opus_encoder_create` and is freed once.
        unsafe { opus_encoder_destroy(self.raw) };
    }
}

pub struct Decoder {
    raw: *mut OpusDecoder,
}

// The decoder is only ever touched from the task that owns it.
unsafe impl Send for Decoder {}

impl Decoder {
    pub fn new() -> Option<Self> {
        let mut error = 0;
        // SAFETY: valid arguments, and a null result is checked below.
        let raw = unsafe { opus_decoder_create(SAMPLE_RATE as i32, CHANNELS as i32, &mut error) };
        (!raw.is_null() && error == 0).then_some(Self { raw })
    }

    /// Decodes one packet to interleaved stereo samples. `None` for a packet
    /// Opus rejects.
    pub fn decode(&mut self, packet: &[u8]) -> Option<Vec<i16>> {
        // An empty packet asks Opus to conceal a loss, which we do not want.
        if packet.is_empty() {
            return None;
        }
        let mut pcm = vec![0i16; MAX_FRAME * CHANNELS];
        // SAFETY: `pcm` holds MAX_FRAME frames and `packet` is a live slice.
        let frames = unsafe {
            opus_decode(
                self.raw,
                packet.as_ptr(),
                packet.len() as i32,
                pcm.as_mut_ptr(),
                MAX_FRAME as i32,
                0,
            )
        };
        let frames = usize::try_from(frames).ok().filter(|&f| f > 0)?;
        pcm.truncate(frames * CHANNELS);
        Some(pcm)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `opus_decoder_create` and is freed once.
        unsafe { opus_decoder_destroy(self.raw) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silence_frame_decodes_to_twenty_milliseconds() {
        let mut decoder = Decoder::new().unwrap();
        // The three byte silence packet Discord sends when someone stops talking.
        let pcm = decoder.decode(&[0xf8, 0xff, 0xfe]).unwrap();
        assert_eq!(pcm.len(), 960 * CHANNELS);
        assert!(pcm.iter().all(|&s| s == 0));
    }

    #[test]
    fn a_tone_survives_an_encode_decode_round_trip() {
        let mut encoder = Encoder::new().unwrap();
        let mut decoder = Decoder::new().unwrap();
        let tone = |ix: usize| ((ix as f32 * 0.1).sin() * 8000.0) as i16;

        // The first frames are the codec warming up, so judge the tenth.
        let mut decoded = Vec::new();
        for frame in 0..10 {
            let pcm: Vec<i16> = (0..FRAME).map(|ix| tone(frame * FRAME + ix)).collect();
            let packet = encoder.encode(&pcm).unwrap();
            assert!(packet.len() < 400, "{} bytes is not speech sized", packet.len());
            decoded = decoder.decode(&packet).unwrap();
        }
        assert_eq!(decoded.len(), FRAME * CHANNELS);
        let energy: i64 = decoded.iter().map(|&s| i64::from(s).pow(2)).sum();
        assert!(energy > 0, "the decoded tone is silent");
    }

    #[test]
    fn only_whole_frames_are_encoded() {
        assert!(Encoder::new().unwrap().encode(&[0; 100]).is_none());
    }

    #[test]
    fn garbage_is_rejected() {
        let mut decoder = Decoder::new().unwrap();
        assert!(decoder.decode(&[]).is_none());
    }
}
