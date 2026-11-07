//! Animated PNGs, which is how Discord ships profile effects and animated
//! nameplates. They are decoded here, already scaled to the size they will be
//! drawn at, because a 450 x 880 effect has dozens of frames and keeping them
//! at full size would cost tens of megabytes per layer.

use std::io::Cursor;

use image::{
    AnimationDecoder, RgbaImage,
    codecs::png::PngDecoder,
    imageops::{self, FilterType},
};

/// More frames than any shop item has. Anything longer is refused instead of
/// held in memory.
const MAX_FRAMES: usize = 120;

/// What all frames of one animation may take together, once decoded.
const MAX_BYTES: usize = 96 * 1024 * 1024;

/// A frame is shown at least this long, some encoders write a delay of zero.
const MIN_DELAY_MS: u32 = 20;

pub struct Frame {
    pub rgba: RgbaImage,
    pub delay_ms: u32,
}

/// Every frame of `bytes`, scaled to `width` pixels across with the shape
/// kept. A plain PNG gives its one picture, so callers need not tell them
/// apart. `None` for anything that is not a PNG we can read in full.
pub fn decode(bytes: &[u8], width: u32) -> Option<Vec<Frame>> {
    let decoder = PngDecoder::new(Cursor::new(bytes)).ok()?;
    let frames = if decoder.is_apng().ok()? {
        decoder
            .apng()
            .ok()?
            .into_frames()
            .take(MAX_FRAMES + 1)
            .collect::<Result<Vec<_>, _>>()
            .ok()?
    } else {
        let picture = image::load_from_memory(bytes).ok()?.to_rgba8();
        vec![image::Frame::new(picture)]
    };
    if frames.is_empty() || frames.len() > MAX_FRAMES {
        return None;
    }

    let (source_width, source_height) = frames[0].buffer().dimensions();
    if source_width == 0 || source_height == 0 {
        return None;
    }
    let width = width.clamp(1, source_width);
    let height = ((u64::from(source_height) * u64::from(width)) / u64::from(source_width)).max(1) as u32;
    if frames.len() * width as usize * height as usize * 4 > MAX_BYTES {
        return None;
    }

    Some(
        frames
            .into_iter()
            .map(|frame| {
                let (numer, denom) = frame.delay().numer_denom_ms();
                let delay_ms = (numer / denom.max(1)).max(MIN_DELAY_MS);
                let rgba = if width == source_width {
                    frame.into_buffer()
                } else {
                    imageops::resize(frame.buffer(), width, height, FilterType::Triangle)
                };
                Frame { rgba, delay_ms }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two frame animation of flat colors, 8 x 4 pixels.
    fn animation(delays: [u16; 2]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, 8, 4);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 0).unwrap();
        let mut writer = encoder.write_header().unwrap();
        for (shade, delay) in [60u8, 200].into_iter().zip(delays) {
            writer.set_frame_delay(delay, 1000).unwrap();
            writer.write_image_data(&[shade; 8 * 4 * 4]).unwrap();
        }
        writer.finish().unwrap();
        out
    }

    #[test]
    fn every_frame_comes_back_with_its_delay() {
        let frames = decode(&animation([100, 250]), 8).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].delay_ms, 100);
        assert_eq!(frames[1].delay_ms, 250);
        assert_eq!(frames[0].rgba.get_pixel(0, 0).0, [60; 4]);
        assert_eq!(frames[1].rgba.get_pixel(7, 3).0, [200; 4]);
    }

    #[test]
    fn frames_are_scaled_to_the_width_asked_for() {
        let frames = decode(&animation([100, 100]), 4).unwrap();
        assert_eq!(frames[0].rgba.dimensions(), (4, 2));
        // Never blown up past the original.
        let frames = decode(&animation([100, 100]), 64).unwrap();
        assert_eq!(frames[0].rgba.dimensions(), (8, 4));
    }

    #[test]
    fn a_zero_delay_is_raised_so_the_animation_does_not_spin() {
        let frames = decode(&animation([0, 0]), 8).unwrap();
        assert_eq!(frames[0].delay_ms, MIN_DELAY_MS);
    }

    #[test]
    fn a_still_png_is_one_frame() {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&[9; 16]).unwrap();
        assert_eq!(decode(&out, 2).unwrap().len(), 1);
    }

    #[test]
    fn garbage_is_refused() {
        assert!(decode(b"not a picture", 8).is_none());
        assert!(decode(b"", 8).is_none());
    }
}
