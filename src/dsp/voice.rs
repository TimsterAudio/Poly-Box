//! One voice: two VCOs and noise into the SEM filter, with a filter envelope
//! and an amp envelope.

use super::env::Adsr;
use super::filter::{Coeffs, SemFilter};
use super::osc::{Osc, Shape};

/// How far the filter envelope can move the cutoff, in octaves, at full amount.
pub const FILTER_ENV_OCTAVES: f32 = 7.0;

/// The sound's settings as one voice needs them, in plain units. The plug-in
/// fills this from its parameters each sample; the offline renderer and tests
/// fill it by hand.
#[derive(Clone, Copy, Debug)]
pub struct Patch {
    /// Octave shift, -2..2 (32' to 2').
    pub osc1_octave: i32,
    /// 0 saw, 0.5 square, 1 narrow pulse.
    pub osc1_shape: f32,
    pub osc2_octave: i32,
    pub osc2_semitones: i32,
    pub osc2_cents: f32,
    pub osc2_shape: f32,
    /// Hard-sync osc 2 to osc 1.
    pub sync: bool,

    pub osc1_level: f32,
    pub osc2_level: f32,
    pub noise_level: f32,

    pub cutoff_hz: f32,
    /// 0..1.
    pub resonance: f32,
    /// 0 low-pass, 0.5 notch, 1 high-pass.
    pub filter_mode: f32,
    pub bandpass: bool,
    /// 0..1, mapped to 1x..4x into the filter.
    pub drive: f32,
    /// 0..1: how far the cutoff follows the keyboard (1 = one octave per octave).
    pub key_track: f32,
    /// -1..1 of [`FILTER_ENV_OCTAVES`].
    pub filter_env_amount: f32,
    /// 0..1: how much velocity scales the filter envelope.
    pub filter_velocity: f32,
    /// Attack, decay, sustain, release (seconds, seconds, 0..1, seconds).
    pub filter_env: [f32; 4],

    pub amp_env: [f32; 4],
    /// 0..1: how much velocity scales the level.
    pub amp_velocity: f32,
}

impl Default for Patch {
    /// The init patch: a warm, slightly detuned two-saw sound with a little
    /// filter movement.
    fn default() -> Self {
        Self {
            osc1_octave: 0,
            osc1_shape: 0.0,
            osc2_octave: 0,
            osc2_semitones: 0,
            osc2_cents: 7.0,
            osc2_shape: 0.0,
            sync: false,
            osc1_level: 0.8,
            osc2_level: 0.8,
            noise_level: 0.0,
            cutoff_hz: 1_500.0,
            resonance: 0.2,
            filter_mode: 0.0,
            bandpass: false,
            drive: 0.2,
            key_track: 0.5,
            filter_env_amount: 0.4,
            filter_velocity: 0.3,
            filter_env: [0.005, 0.4, 0.3, 0.3],
            amp_env: [0.002, 0.5, 0.8, 0.25],
            amp_velocity: 0.5,
        }
    }
}

fn note_to_hz(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

#[derive(Clone, Debug)]
pub struct Voice {
    sample_rate: f32,
    osc1: Osc,
    osc2: Osc,
    noise: u32,
    filter: SemFilter,
    filter_env: Adsr,
    amp_env: Adsr,
    /// The note playing, in MIDI note numbers (fractional, for glide later).
    note: f32,
    velocity: f32,
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            osc1: Osc::new(),
            osc2: Osc::new(),
            noise: 0x9E37_79B9,
            filter: SemFilter::default(),
            filter_env: Adsr::default(),
            amp_env: Adsr::default(),
            note: 60.0,
            velocity: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.osc1.reset(0.0);
        self.osc2.reset(0.0);
        self.filter.reset();
        self.filter_env.reset();
        self.amp_env.reset();
    }

    /// Start a note. With `retrigger` false the pitch moves but the envelopes
    /// carry on (legato).
    pub fn note_on(&mut self, note: u8, velocity: f32, retrigger: bool) {
        self.note = note as f32;
        if retrigger {
            self.velocity = velocity.clamp(0.0, 1.0);
            self.filter_env.gate_on();
            self.amp_env.gate_on();
        }
    }

    pub fn note_off(&mut self) {
        self.filter_env.gate_off();
        self.amp_env.gate_off();
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    /// Take the patch's envelope times. Cheap enough per block, not needed per sample.
    pub fn set_envelopes(&mut self, p: &Patch) {
        let [a, d, s, r] = p.filter_env;
        self.filter_env.set(a, d, s, r, self.sample_rate);
        let [a, d, s, r] = p.amp_env;
        self.amp_env.set(a, d, s, r, self.sample_rate);
    }

    /// One sample. `bend` is in semitones.
    #[inline]
    pub fn next(&mut self, p: &Patch, bend: f32) -> f32 {
        let note = self.note + bend;
        let f1 = note_to_hz(note + 12.0 * p.osc1_octave as f32);
        let f2 = note_to_hz(
            note + 12.0 * p.osc2_octave as f32 + p.osc2_semitones as f32 + p.osc2_cents / 100.0,
        );
        let (o1, wrapped) = self.osc1.next(
            f1 / self.sample_rate,
            Shape::from_control(p.osc1_shape),
            None,
        );
        let sync = if p.sync { wrapped } else { None };
        let (o2, _) = self.osc2.next(
            f2 / self.sample_rate,
            Shape::from_control(p.osc2_shape),
            sync,
        );

        // xorshift32 white noise.
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let noise = (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0;

        let mix = o1 * p.osc1_level + o2 * p.osc2_level + noise * p.noise_level;

        let fe = self.filter_env.advance();
        let ae = self.amp_env.advance();

        let vel_f = 1.0 - p.filter_velocity + p.filter_velocity * self.velocity;
        let octaves = p.filter_env_amount * FILTER_ENV_OCTAVES * fe * vel_f
            + p.key_track * (self.note - 60.0) / 12.0;
        let cutoff = p.cutoff_hz * octaves.exp2();
        let coeffs = Coeffs::new(cutoff, p.resonance, self.sample_rate);
        let drive = 1.0 + 3.0 * p.drive;
        let y = self
            .filter
            .process(mix, coeffs, p.filter_mode, p.bandpass, drive);

        let vel_a = 1.0 - p.amp_velocity + p.amp_velocity * self.velocity;
        y * ae * vel_a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_and_stops() {
        let sr = 48_000.0;
        let p = Patch::default();
        let mut v = Voice::new(sr);
        v.set_envelopes(&p);
        assert!(!v.is_active());
        v.note_on(48, 1.0, true);
        let held: Vec<f32> = (0..24_000).map(|_| v.next(&p, 0.0)).collect();
        let peak = held.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.2 && peak < 2.0, "{peak}");
        v.note_off();
        let mut n = 0;
        while v.is_active() {
            let y = v.next(&p, 0.0);
            assert!(y.is_finite());
            n += 1;
        }
        assert!(n < 48_000);
    }

    #[test]
    fn pitch_is_right() {
        // A1 (110 Hz) through a wide-open filter: count rising zero crossings
        // of osc 1 alone over one second.
        let sr = 48_000.0;
        let p = Patch {
            osc2_level: 0.0,
            cutoff_hz: 20_000.0,
            filter_env_amount: 0.0,
            resonance: 0.0,
            amp_env: [0.001, 1.0, 1.0, 0.1],
            ..Patch::default()
        };
        let mut v = Voice::new(sr);
        v.set_envelopes(&p);
        v.note_on(45, 1.0, true);
        for _ in 0..4_800 {
            v.next(&p, 0.0);
        }
        let x: Vec<f32> = (0..48_000).map(|_| v.next(&p, 0.0)).collect();
        // The rising saw crosses zero downwards only at its reset, once a cycle.
        let falls = x.windows(2).filter(|w| w[0] >= 0.0 && w[1] < 0.0).count();
        assert!((109..=111).contains(&falls), "{falls}");
    }
}
