//! Sound out: the synth running in the audio device's callback, reading the
//! panel's values and keys through atomics so it never waits on the window.

use crate::controls::{to_patch, Values, COUNT, DEFS};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use poly_box_engine::keys::{KeyBridge, Live};
use poly_box_engine::Synth;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::Arc;

/// Samples per control update: the panel's values are read, smoothed and
/// turned into a patch this often.
const BLOCK: usize = 32;
/// How long smoothed controls take to follow the panel.
const SMOOTH_SECONDS: f32 = 0.012;

/// What the window and the audio callback share.
pub struct Shared {
    values: [AtomicU32; COUNT],
    pub live: Live,
    /// Peak level per channel since the window last looked (f32 bits).
    peak: [AtomicU32; 2],
}

impl Shared {
    pub fn new(values: &Values) -> Self {
        Self {
            values: std::array::from_fn(|i| AtomicU32::new(values[i].to_bits())),
            live: Live::default(),
            peak: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    pub fn set(&self, i: usize, v: f32) {
        self.values[i].store(v.to_bits(), Relaxed);
    }

    fn get(&self, i: usize) -> f32 {
        f32::from_bits(self.values[i].load(Relaxed))
    }

    /// The peak level on each channel since the last call, then starts afresh.
    pub fn take_peaks(&self) -> [f32; 2] {
        std::array::from_fn(|c| f32::from_bits(self.peak[c].swap(0, Relaxed)))
    }
}

/// The audio thread's state.
pub struct Engine {
    shared: Arc<Shared>,
    synth: Synth,
    keys: KeyBridge,
    current: Values,
    smoothing: f32,
}

impl Engine {
    pub fn new(shared: Arc<Shared>, sample_rate: f32) -> Self {
        let current = std::array::from_fn(|i| shared.get(i));
        Self {
            shared,
            synth: Synth::new(sample_rate),
            keys: KeyBridge::default(),
            current,
            smoothing: 1.0 - (-(BLOCK as f32) / (SMOOTH_SECONDS * sample_rate)).exp(),
        }
    }

    /// Fill interleaved `out` with `channels` channels.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let mut peak = [0.0f32; 2];
        for chunk in out.chunks_mut(BLOCK * channels) {
            for (i, cur) in self.current.iter_mut().enumerate() {
                let target = self.shared.get(i);
                if DEFS[i].smoothed() {
                    *cur += (target - *cur) * self.smoothing;
                } else {
                    *cur = target;
                }
            }
            let patch = to_patch(&self.current);
            let gain = 10f32.powf(self.current[crate::controls::Ctl::Volume as usize] / 20.0);
            self.synth.set_block(&patch);
            self.keys.take_window(&self.shared.live, &mut self.synth);
            for frame in chunk.chunks_mut(channels) {
                let (l, r) = self.synth.next(&patch);
                let (l, r) = (l * gain, r * gain);
                peak[0] = peak[0].max(l.abs());
                peak[1] = peak[1].max(r.abs());
                match frame {
                    [only] => *only = 0.5 * (l + r),
                    [a, b, rest @ ..] => {
                        *a = l;
                        *b = r;
                        rest.fill(0.0);
                    }
                    [] => {}
                }
            }
        }
        self.keys.publish(&self.shared.live, &self.synth);
        for (c, p) in peak.iter().enumerate() {
            self.shared.peak[c].fetch_max(p.to_bits(), Relaxed);
        }
    }
}

/// An open output: keep it alive for as long as there should be sound.
pub struct Output {
    _stream: cpal::Stream,
    pub description: String,
}

/// With no audio device, still run the synth (silently, at about real
/// time) so the keys and voice lights work and the panel can be tried.
pub fn start_silent(shared: Arc<Shared>) {
    std::thread::spawn(move || {
        let rate = 48_000.0;
        let mut engine = Engine::new(shared, rate);
        let mut buf = vec![0.0f32; 256 * 2];
        let period = std::time::Duration::from_secs_f32(256.0 / rate);
        loop {
            engine.render(&mut buf, 2);
            std::thread::sleep(period);
        }
    });
}

/// Open the default output device and start the synth on it.
pub fn start(shared: Arc<Shared>) -> Result<Output, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no audio output device")?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("audio device: {e}"))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate.0;
    let description = format!(
        "{} · {} Hz",
        device.name().unwrap_or_else(|_| "Audio".into()),
        rate
    );
    let engine = Engine::new(shared, rate as f32);
    let stream = match format {
        SampleFormat::F32 => open::<f32>(&device, &config, engine),
        SampleFormat::I16 => open::<i16>(&device, &config, engine),
        SampleFormat::I32 => open::<i32>(&device, &config, engine),
        SampleFormat::U16 => open::<u16>(&device, &config, engine),
        other => return Err(format!("unsupported sample format {other}")),
    }?;
    stream.play().map_err(|e| format!("audio: {e}"))?;
    Ok(Output {
        _stream: stream,
        description,
    })
}

fn open<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut engine: Engine,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    // Render into a float scratch buffer, then convert: made once, here,
    // so the callback never allocates.
    let mut scratch = vec![0.0f32; 4096 * channels.max(1)];
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                for piece in data.chunks_mut(scratch.len()) {
                    let buf = &mut scratch[..piece.len()];
                    engine.render(buf, channels);
                    for (o, &s) in piece.iter_mut().zip(buf.iter()) {
                        *o = T::from_sample(s);
                    }
                }
            },
            |e| eprintln!("audio error: {e}"),
            None,
        )
        .map_err(|e| format!("audio stream: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{defaults, Ctl};

    #[test]
    fn a_key_from_the_window_makes_sound_and_lights_a_voice() {
        let shared = Arc::new(Shared::new(&defaults()));
        let mut engine = Engine::new(shared.clone(), 48_000.0);
        let mut keys = poly_box_engine::keys::KeyPresses::default();
        keys.letter_down(&shared.live, 'a');
        let mut out = vec![0.0f32; 4800 * 2];
        engine.render(&mut out, 2);
        let peaks = shared.take_peaks();
        assert!(peaks[0] > 0.05 && peaks[1] > 0.05, "{peaks:?}");
        assert!(matches!(
            shared.live.voice(0),
            poly_box_engine::keys::VoiceLight::Held(60)
        ));
        // And the peak starts afresh once read.
        assert_eq!(shared.take_peaks(), [0.0, 0.0]);
    }

    #[test]
    fn turning_the_volume_down_turns_it_down() {
        let shared = Arc::new(Shared::new(&defaults()));
        let mut engine = Engine::new(shared.clone(), 48_000.0);
        let mut keys = poly_box_engine::keys::KeyPresses::default();
        keys.letter_down(&shared.live, 'a');
        let mut out = vec![0.0f32; 4800 * 2];
        engine.render(&mut out, 2);
        let loud = shared.take_peaks()[0];
        shared.set(Ctl::Volume as usize, -40.0);
        for _ in 0..10 {
            engine.render(&mut out, 2);
        }
        shared.take_peaks();
        engine.render(&mut out, 2);
        assert!(shared.take_peaks()[0] < loud * 0.05);
    }

    #[test]
    fn mono_devices_get_both_channels() {
        let shared = Arc::new(Shared::new(&defaults()));
        let mut engine = Engine::new(shared.clone(), 48_000.0);
        let mut keys = poly_box_engine::keys::KeyPresses::default();
        keys.letter_down(&shared.live, 'a');
        let mut out = vec![0.0f32; 4800];
        engine.render(&mut out, 1);
        assert!(out.iter().any(|s| s.abs() > 0.01));
    }
}
