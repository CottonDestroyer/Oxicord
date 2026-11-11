//! Reads the default microphone and cuts it into 20 ms mono frames at 48 kHz,
//! ready for the Opus encoder. Like the output, the cpal stream lives on a
//! thread of its own.

use std::{sync::mpsc, thread, time::Duration};

use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::opus::{FRAME, SAMPLE_RATE};

/// Loudness (RMS of a frame) above which we consider someone to be talking.
const THRESHOLD: f64 = 350.0;
/// Frames the gate stays open after the last loud one, so the ends of words
/// are not clipped. 200 ms.
const HANGOVER: u32 = 10;

/// Decides which frames are speech and which are room noise.
#[derive(Default)]
pub struct Gate {
    hangover: u32,
}

impl Gate {
    /// Whether this frame should be sent.
    pub fn passes(&mut self, frame: &[i16]) -> bool {
        let energy: f64 = frame.iter().map(|&s| f64::from(s).powi(2)).sum();
        if (energy / frame.len().max(1) as f64).sqrt() > THRESHOLD {
            self.hangover = HANGOVER;
        } else {
            self.hangover = self.hangover.saturating_sub(1);
            return self.hangover > 0;
        }
        true
    }
}

/// The input volume setting scaled onto a frame: 100 leaves it alone, 0
/// silences it. Like the official client's slider it only turns the mic down.
pub fn with_volume(frame: &[i16], percent: u8) -> Vec<i16> {
    let percent = i32::from(percent.min(100));
    frame.iter().map(|&s| (i32::from(s) * percent / 100) as i16).collect()
}

/// RNNoise, which takes the hiss, fans and keyboard out of the microphone.
/// It works on 10 ms at a time, so a 20 ms frame goes through in two halves.
/// The official client uses Krisp, which is proprietary, so this is the
/// closest thing that can ship in the open.
pub struct Denoiser {
    state: Box<nnnoiseless::DenoiseState<'static>>,
}

impl Default for Denoiser {
    fn default() -> Self {
        Self { state: nnnoiseless::DenoiseState::new() }
    }
}

impl Denoiser {
    /// The same frame with the noise taken out.
    pub fn clean(&mut self, frame: &[i16]) -> Vec<i16> {
        const HALF: usize = nnnoiseless::FRAME_SIZE;
        let mut out = Vec::with_capacity(frame.len());
        let mut input = [0.0f32; HALF];
        let mut cleaned = [0.0f32; HALF];
        for part in frame.chunks(HALF) {
            if part.len() < HALF {
                // Not a whole 10 ms: nothing to run it on.
                out.extend_from_slice(part);
                continue;
            }
            for (to, &from) in input.iter_mut().zip(part) {
                *to = f32::from(from);
            }
            self.state.process_frame(&mut cleaned, &input);
            out.extend(cleaned.iter().map(|&s| s.round().clamp(-32768.0, 32767.0) as i16));
        }
        out
    }
}

/// Turns whatever the device delivers into 20 ms mono frames at 48 kHz:
/// channels are averaged and the rate is converted by linear interpolation.
pub struct Framer {
    channels: usize,
    step: f64,
    position: f64,
    previous: f32,
    pending: Vec<i16>,
}

impl Framer {
    pub fn new(channels: u16, device_rate: u32) -> Self {
        Self {
            channels: usize::from(channels.max(1)),
            step: f64::from(device_rate) / f64::from(SAMPLE_RATE),
            position: 0.0,
            previous: 0.0,
            pending: Vec::with_capacity(FRAME),
        }
    }

    pub fn push(&mut self, interleaved: &[f32], mut emit: impl FnMut(Vec<i16>)) {
        for chunk in interleaved.chunks_exact(self.channels) {
            let sample = chunk.iter().sum::<f32>() / self.channels as f32;
            while self.position < 1.0 {
                let out = self.previous + (sample - self.previous) * self.position as f32;
                self.pending.push((out.clamp(-1.0, 1.0) * 32767.0) as i16);
                if self.pending.len() == FRAME {
                    emit(std::mem::replace(&mut self.pending, Vec::with_capacity(FRAME)));
                }
                self.position += self.step;
            }
            self.position -= 1.0;
            self.previous = sample;
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    frames: UnboundedSender<Vec<i16>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut framer = Framer::new(config.channels, config.sample_rate);
    let mut scratch: Vec<f32> = Vec::new();
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            scratch.clear();
            scratch.extend(data.iter().map(|&s| f32::from_sample(s)));
            framer.push(&scratch, |frame| {
                let _ = frames.send(frame);
            });
        },
        |err| tracing::warn!(%err, "audio input error"),
        None,
    )
}

/// Names of the microphones we can record from.
pub fn input_devices() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|devices| devices.filter_map(|d| d.description().ok().map(|d| d.name().to_owned())).collect())
        .unwrap_or_default()
}

/// The device called `name`, or the default one when it is not given or gone.
fn find_input(name: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let named = name.and_then(|name| {
        host.input_devices()
            .ok()?
            .find(|d| d.description().is_ok_and(|d| d.name() == name))
    });
    named
        .or_else(|| host.default_input_device())
        .ok_or_else(|| "no microphone found".to_owned())
}

fn open(frames: UnboundedSender<Vec<i16>>, device: Option<&str>) -> Result<cpal::Stream, String> {
    let device = find_input(device)?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, frames),
        SampleFormat::I16 => build::<i16>(&device, config, frames),
        SampleFormat::U16 => build::<u16>(&device, config, frames),
        SampleFormat::I32 => build::<i32>(&device, config, frames),
        other => return Err(format!("unsupported microphone format {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

/// The running microphone. Dropping it releases the device.
pub struct Capture {
    _stop: mpsc::Sender<()>,
}

impl Capture {
    pub fn start(frames: UnboundedSender<Vec<i16>>, device: Option<String>) -> Result<Self, String> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        thread::Builder::new()
            .name("oxicord-audio-in".into())
            .spawn(move || match open(frames, device.as_deref()) {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(err));
                }
            })
            .map_err(|err| err.to_string())?;

        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "the microphone took too long to open".to_owned())??;
        Ok(Self { _stop: stop_tx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(framer: &mut Framer, input: &[f32]) -> Vec<Vec<i16>> {
        let mut frames = Vec::new();
        framer.push(input, |frame| frames.push(frame));
        frames
    }

    #[test]
    fn stereo_at_48k_is_averaged_into_whole_frames() {
        let mut framer = Framer::new(2, 48_000);
        // 1920 interleaved samples is 960 frames: one 20 ms output frame
        // (less the one sample of interpolation delay).
        let input: Vec<f32> = (0..1920 * 2).map(|i| if i % 2 == 0 { 0.5 } else { 0.1 }).collect();
        let frames = collect(&mut framer, &input);
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|f| f.len() == FRAME));
        let settled = frames[1][500];
        assert!((i32::from(settled) - (0.3 * 32767.0) as i32).abs() < 4, "{settled}");
    }

    #[test]
    fn a_slower_device_is_stretched_to_48k() {
        let mut framer = Framer::new(1, 24_000);
        // 24 kHz in gives twice as many samples out: 480 in -> 960 out.
        let frames = collect(&mut framer, &vec![0.25; 480]);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn a_faster_device_is_squeezed_to_48k() {
        let mut framer = Framer::new(1, 96_000);
        let frames = collect(&mut framer, &vec![0.25; 1920]);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn the_input_volume_scales_the_frame() {
        let frame = vec![1000i16, -1000, 32767, -32768];
        assert_eq!(with_volume(&frame, 100), frame);
        assert_eq!(with_volume(&frame, 50), vec![500, -500, 16383, -16384]);
        assert_eq!(with_volume(&frame, 0), vec![0; 4]);
        assert_eq!(with_volume(&frame, 250), frame, "above 100 is capped");
    }


    #[test]
    fn the_denoiser_keeps_the_length_and_quiets_steady_noise() {
        let mut denoiser = Denoiser::default();
        let mut seed = 12345u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((seed >> 16) as i16 % 800) as i16
        };
        let rms = |frame: &[i16]| (frame.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / frame.len() as f64).sqrt();
        let (mut before, mut after) = (0.0, 0.0);
        for i in 0..100 {
            let frame: Vec<i16> = (0..FRAME).map(|_| noise()).collect();
            let cleaned = denoiser.clean(&frame);
            assert_eq!(cleaned.len(), FRAME);
            // The first frames are the model warming up.
            if i >= 50 {
                before += rms(&frame);
                after += rms(&cleaned);
            }
        }
        assert!(after < before * 0.5, "{after} vs {before}");
    }
    #[test]
    fn the_gate_opens_on_speech_and_closes_after_the_hangover() {
        let mut gate = Gate::default();
        let quiet = vec![10i16; FRAME];
        let loud = vec![5000i16; FRAME];
        assert!(!gate.passes(&quiet));
        assert!(gate.passes(&loud));
        // Stays open for the hangover, then shuts.
        let open = (0..HANGOVER).filter(|_| gate.passes(&quiet)).count();
        assert_eq!(open, HANGOVER as usize - 1);
        assert!(!gate.passes(&quiet));
    }
}
