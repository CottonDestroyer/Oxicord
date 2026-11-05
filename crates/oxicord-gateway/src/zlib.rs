use flate2::{Decompress, DecompressError, FlushDecompress};

/// Every complete zlib-stream message ends with an empty sync-flush block.
const SYNC_SUFFIX: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

/// Frames larger than this without a terminator are treated as corrupt
/// rather than buffered forever.
const MAX_PENDING: usize = 32 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ZlibError {
    #[error("inflate failed: {0}")]
    Inflate(#[from] DecompressError),
    #[error("inflater made no progress")]
    Stalled,
    #[error("compressed message exceeds {MAX_PENDING} bytes")]
    TooLarge,
}

/// Decoder for `compress=zlib-stream`.
///
/// The whole connection is one zlib stream, so the inflater has to live as
/// long as the socket does. A new connection needs a new `ZlibStream`.
pub struct ZlibStream {
    inflater: Decompress,
    pending: Vec<u8>,
}

impl Default for ZlibStream {
    fn default() -> Self {
        Self {
            inflater: Decompress::new(true),
            pending: Vec::new(),
        }
    }
}

impl ZlibStream {
    /// Feeds one websocket binary frame. A payload can span several frames,
    /// so this only yields output once the terminator has arrived.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Option<Vec<u8>>, ZlibError> {
        self.pending.extend_from_slice(chunk);
        if self.pending.len() > MAX_PENDING {
            return Err(ZlibError::TooLarge);
        }
        if !self.pending.ends_with(&SYNC_SUFFIX) {
            return Ok(None);
        }

        let input = std::mem::take(&mut self.pending);
        let start = self.inflater.total_in();
        let mut out = Vec::with_capacity(input.len() * 4);

        loop {
            let consumed = (self.inflater.total_in() - start) as usize;
            let produced_before = self.inflater.total_out();
            if out.len() == out.capacity() {
                out.reserve(out.capacity().max(4096));
            }

            self.inflater
                .decompress_vec(&input[consumed..], &mut out, FlushDecompress::Sync)?;

            let consumed_now = (self.inflater.total_in() - start) as usize;
            // Spare capacity left over means the inflater drained everything
            // it had. A full buffer might hide more output, so go around.
            if consumed_now >= input.len() && out.len() < out.capacity() {
                return Ok(Some(out));
            }
            if consumed_now == consumed && self.inflater.total_out() == produced_before {
                return Err(ZlibError::Stalled);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use flate2::{Compress, Compression, FlushCompress};

    use super::*;

    /// Compresses like the server does: one shared context, a sync flush
    /// after every message.
    pub(crate) struct TestCompressor(Compress);

    impl TestCompressor {
        pub(crate) fn new() -> Self {
            Self(Compress::new(Compression::default(), true))
        }

        pub(crate) fn message(&mut self, json: &str) -> Vec<u8> {
            let mut out = Vec::with_capacity(json.len() + 64);
            self.0
                .compress_vec(json.as_bytes(), &mut out, FlushCompress::Sync)
                .unwrap();
            out
        }
    }

    #[test]
    fn decodes_consecutive_messages_sharing_a_context() {
        let mut enc = TestCompressor::new();
        let mut dec = ZlibStream::default();

        for text in [r#"{"op":10,"d":{}}"#, r#"{"op":11}"#, r#"{"op":10,"d":{}}"#] {
            let frame = enc.message(text);
            let out = dec.push(&frame).unwrap().expect("complete message");
            assert_eq!(out, text.as_bytes());
        }
    }

    #[test]
    fn waits_for_the_terminator_across_split_frames() {
        let mut enc = TestCompressor::new();
        let mut dec = ZlibStream::default();
        let frame = enc.message(r#"{"op":0,"t":"READY"}"#);

        let (a, b) = frame.split_at(frame.len() - 6);
        assert!(dec.push(a).unwrap().is_none());
        let out = dec.push(b).unwrap().expect("complete message");
        assert_eq!(out, br#"{"op":0,"t":"READY"}"#);
    }

    #[test]
    fn handles_output_much_larger_than_input() {
        let mut enc = TestCompressor::new();
        let mut dec = ZlibStream::default();
        let big = format!(r#"{{"pad":"{}"}}"#, "a".repeat(500_000));

        let frame = enc.message(&big);
        assert!(frame.len() < big.len() / 50);
        let out = dec.push(&frame).unwrap().expect("complete message");
        assert_eq!(out.len(), big.len());
    }

    #[test]
    fn rejects_garbage_with_a_terminator() {
        let mut dec = ZlibStream::default();
        assert!(dec.push(&[1, 2, 3, 4, 0x00, 0x00, 0xff, 0xff]).is_err());
    }
}
