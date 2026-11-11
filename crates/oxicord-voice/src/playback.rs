//! Mixes everyone in the call into the default output device.
//!
//! The network task pushes decoded frames into a [`Mixer`] and the audio
//! callback pulls from it. The cpal stream is not `Send`, so it lives on a
//! thread of its own for as long as the call does.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use cpal::{
    FromSample, SampleFormat, SizedSample, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::opus::{CHANNELS, SAMPLE_RATE};

/// How much a speaker has to have buffered before we start playing them, so
/// a little network jitter does not turn into crackling. 60 ms.
const PRIME: usize = (SAMPLE_RATE as usize * CHANNELS) * 60 / 1000;
/// The most we keep per speaker. Past this the oldest audio is dropped so a
/// stalled device cannot build up seconds of delay. 400 ms.
const LIMIT: usize = (SAMPLE_RATE as usize * CHANNELS) * 400 / 1000;
/// A speaker who has sent nothing for this long is forgotten.
const FORGET: Duration = Duration::from_secs(10);

struct Source {
    samples: VecDeque<i16>,
    playing: bool,
    last_push: Instant,
}

#[derive(Default)]
pub struct Mixer {
    sources: HashMap<u32, Source>,
    deafened: bool,
}

impl Mixer {
    pub fn push(&mut self, ssrc: u32, pcm: &[i16]) {
        if self.deafened {
            return;
        }
        let now = Instant::now();
        self.sources.retain(|_, s| now.duration_since(s.last_push) < FORGET);
        let source = self.sources.entry(ssrc).or_insert_with(|| Source {
            samples: VecDeque::new(),
            playing: false,
            last_push: now,
        });
        source.last_push = now;
        source.samples.extend(pcm);
        let excess = source.samples.len().saturating_sub(LIMIT);
        source.samples.drain(..excess);
    }

    pub fn set_deafened(&mut self, deafened: bool) {
        self.deafened = deafened;
        if deafened {
            self.sources.clear();
        }
    }

    /// Fills `out` (interleaved stereo, 48 kHz) with everyone mixed together.
    fn pull(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        for source in self.sources.values_mut() {
            if !source.playing && source.samples.len() >= PRIME {
                source.playing = true;
            }
            if !source.playing {
                continue;
            }
            for slot in out.iter_mut() {
                match source.samples.pop_front() {
                    Some(sample) => *slot += f32::from(sample) / 32768.0,
                    None => {
                        // Ran dry: wait for it to refill before playing on.
                        source.playing = false;
                        break;
                    }
                }
            }
        }
        for slot in out.iter_mut() {
            *slot = slot.clamp(-1.0, 1.0);
        }
    }
}

/// Converts the mixer's 48 kHz to whatever rate the device runs at by linear
/// interpolation. Speech survives that fine.
struct Resampler {
    step: f64,
    position: f64,
    queue: VecDeque<[f32; 2]>,
}

impl Resampler {
    fn new(device_rate: u32) -> Self {
        Self {
            step: f64::from(SAMPLE_RATE) / f64::from(device_rate),
            position: 0.0,
            queue: VecDeque::new(),
        }
    }

    fn next(&mut self, mixer: &Mutex<Mixer>) -> [f32; 2] {
        if self.queue.len() < 2 {
            let mut chunk = [0.0f32; 480 * CHANNELS];
            if let Ok(mut mixer) = mixer.lock() {
                mixer.pull(&mut chunk);
            }
            self.queue.extend(chunk.chunks_exact(2).map(|f| [f[0], f[1]]));
        }
        let (a, b) = (self.queue[0], self.queue[1]);
        let t = self.position as f32;
        let frame = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];

        self.position += self.step;
        while self.position >= 1.0 {
            self.position -= 1.0;
            self.queue.pop_front();
            if self.queue.len() < 2 {
                break;
            }
        }
        frame
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mixer: Arc<Mutex<Mixer>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut resampler = Resampler::new(config.sample_rate);
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            for frame in data.chunks_mut(channels) {
                let [left, right] = resampler.next(&mixer);
                for (ix, slot) in frame.iter_mut().enumerate() {
                    *slot = T::from_sample(match (channels, ix) {
                        (1, _) => (left + right) / 2.0,
                        (_, 0) => left,
                        (_, 1) => right,
                        _ => 0.0,
                    });
                }
            }
        },
        |err| tracing::warn!(%err, "audio output error"),
        None,
    )
}

/// The running output. Dropping it stops the device.
pub struct Playback {
    mixer: Arc<Mutex<Mixer>>,
    _stop: mpsc::Sender<()>,
}

impl Playback {
    pub fn start(device: Option<String>) -> Result<Self, String> {
        let mixer = Arc::new(Mutex::new(Mixer::default()));
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let shared = mixer.clone();
        thread::Builder::new()
            .name("oxicord-audio-out".into())
            .spawn(move || {
                let stream = open(shared, device.as_deref());
                match stream {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        // Holds the stream alive until the sender is dropped.
                        let _ = stop_rx.recv();
                        drop(stream);
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                    }
                }
            })
            .map_err(|err| err.to_string())?;

        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "the audio device took too long to open".to_owned())??;
        Ok(Self { mixer, _stop: stop_tx })
    }

    pub fn push(&self, ssrc: u32, pcm: &[i16]) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.push(ssrc, pcm);
        }
    }

    pub fn set_deafened(&self, deafened: bool) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.set_deafened(deafened);
        }
    }
}

/// The loudest a person can be turned up to, like the official client's 200%.
pub const MAX_USER_VOLUME: u16 = 200;

/// Plays one person louder or quieter. 100 is as sent, past it the samples
/// are clipped rather than wrapped.
pub fn with_gain(pcm: &[i16], percent: u16) -> Vec<i16> {
    let gain = f32::from(percent.min(MAX_USER_VOLUME)) / 100.0;
    pcm.iter().map(|&s| (f32::from(s) * gain).round().clamp(-32768.0, 32767.0) as i16).collect()
}

/// Names of the speakers and headphones we can play on.
pub fn output_devices() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|devices| devices.filter_map(|d| d.description().ok().map(|d| d.name().to_owned())).collect())
        .unwrap_or_default()
}

/// The device called `name`, or the default one when it is not given or gone.
fn find_output(name: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let named = name.and_then(|name| {
        host.output_devices()
            .ok()?
            .find(|d| d.description().is_ok_and(|d| d.name() == name))
    });
    named
        .or_else(|| host.default_output_device())
        .ok_or_else(|| "no audio output device".to_owned())
}

fn open(mixer: Arc<Mutex<Mixer>>, device: Option<&str>) -> Result<cpal::Stream, String> {
    let device = find_output(device)?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, mixer),
        SampleFormat::I16 => build::<i16>(&device, config, mixer),
        SampleFormat::U16 => build::<u16>(&device, config, mixer),
        SampleFormat::I32 => build::<i32>(&device, config, mixer),
        other => return Err(format!("unsupported audio format {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(n: usize, value: i16) -> Vec<i16> {
        vec![value; n * CHANNELS]
    }

    #[test]
    fn a_speaker_stays_silent_until_primed_then_plays() {
        let mut mixer = Mixer::default();
        mixer.push(1, &frames(480, 16384));
        let mut out = [0.0f32; 8];
        mixer.pull(&mut out);
        assert!(out.iter().all(|&s| s == 0.0));

        mixer.push(1, &frames(2400, 16384));
        mixer.pull(&mut out);
        assert!(out.iter().all(|&s| (s - 0.5).abs() < 1e-3));
    }

    #[test]
    fn two_speakers_add_and_clip() {
        let mut mixer = Mixer::default();
        mixer.push(1, &frames(2880, 30000));
        mixer.push(2, &frames(2880, 30000));
        let mut out = [0.0f32; 4];
        mixer.pull(&mut out);
        assert!(out.iter().all(|&s| s == 1.0));
    }

    #[test]
    fn deafening_drops_what_was_queued() {
        let mut mixer = Mixer::default();
        mixer.push(1, &frames(2880, 1000));
        mixer.set_deafened(true);
        mixer.push(1, &frames(2880, 1000));
        let mut out = [1.0f32; 4];
        mixer.pull(&mut out);
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn a_person_can_be_turned_down_up_and_off() {
        let pcm = [1000, -1000];
        assert_eq!(with_gain(&pcm, 100), vec![1000, -1000]);
        assert_eq!(with_gain(&pcm, 50), vec![500, -500]);
        assert_eq!(with_gain(&pcm, 0), vec![0, 0]);
        assert_eq!(with_gain(&pcm, 200), vec![2000, -2000]);
        assert_eq!(with_gain(&pcm, 900), vec![2000, -2000], "capped at the maximum");
        assert_eq!(with_gain(&[30000, -30000], 200), vec![32767, -32768], "clipped, not wrapped");
    }

    #[test]
    fn the_buffer_is_capped() {
        let mut mixer = Mixer::default();
        mixer.push(1, &frames(48000, 1));
        assert_eq!(mixer.sources[&1].samples.len(), LIMIT);
    }

    #[test]
    fn resampling_to_the_same_rate_is_a_pass_through() {
        let mixer = Mutex::new(Mixer::default());
        mixer.lock().unwrap().push(1, &frames(2880, 16384));
        let mut resampler = Resampler::new(48_000);
        let frame = resampler.next(&mixer);
        assert!((frame[0] - 0.5).abs() < 1e-3);
    }
}
