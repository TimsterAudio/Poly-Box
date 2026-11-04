//! One voice: two VCOs and noise into the SEM filter, with filter, amp and
//! aux envelopes, glide, the modulation matrix, and the small differences
//! between voices that Vintage brings out.

use super::env::{Adsr, Stage};
use super::filter::{Coeffs, SemFilter};
use super::lfo::LfoSettings;
use super::modulation::{
    route, ModDest, ModSlot, ModSource, CUTOFF_RANGE_OCTAVES, MOD_SLOTS, PITCH_RANGE_SEMITONES,
};
use super::osc::{Osc, Shape};

/// How far the filter envelope can move the cutoff, in octaves, at full amount.
pub const FILTER_ENV_OCTAVES: f32 = 7.0;

/// At full Vintage: each oscillator's fixed mistuning, in cents.
const VINTAGE_TUNE_CENTS: f32 = 10.0;
/// At full Vintage: how far each oscillator wanders over time, in cents.
const VINTAGE_DRIFT_CENTS: f32 = 6.0;
/// At full Vintage: each voice's cutoff offset, in octaves.
const VINTAGE_CUTOFF_OCTAVES: f32 = 0.3;
/// At full Vintage: each voice's envelope times stretched or shrunk by this fraction.
const VINTAGE_ENV_SPREAD: f32 = 0.2;
/// The drift picks a new target this often, in samples, and glides to it.
const DRIFT_STEP: u32 = 2_048;
/// The drift's time constant, in seconds.
const DRIFT_SECONDS: f32 = 0.8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoiceMode {
    /// Each key gets its own voice.
    #[default]
    Poly,
    /// One voice, last-note priority.
    Mono,
    /// All five voices on one note, detuned against each other.
    Unison,
}

/// The sound's settings in plain units. The plug-in fills this from its
/// parameters each sample; the offline renderer and tests fill it by hand.
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

    /// A third envelope, only heard through the modulation matrix.
    pub aux_env: [f32; 4],
    pub lfo1: LfoSettings,
    pub lfo2: LfoSettings,
    pub mods: [ModSlot; MOD_SLOTS],

    pub voice_mode: VoiceMode,
    /// Total detune across the unison stack, in cents.
    pub unison_detune: f32,
    /// Glide time in seconds; 0 is off.
    pub glide: f32,
    /// 0..1: how different the voices are from each other, and how much they drift.
    pub vintage: f32,
    /// 0..1: how widely the voices are spread across the stereo field.
    pub spread: f32,
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
            aux_env: [0.5, 1.0, 0.0, 0.5],
            lfo1: LfoSettings::default(),
            lfo2: LfoSettings {
                rate_hz: 0.3,
                ..LfoSettings::default()
            },
            mods: [ModSlot::default(); MOD_SLOTS],
            voice_mode: VoiceMode::Poly,
            unison_detune: 20.0,
            glide: 0.0,
            vintage: 0.3,
            spread: 0.3,
        }
    }
}

/// What every voice shares in a sample: the LFOs and the pitch bend.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shared {
    pub lfo1: f32,
    pub lfo2: f32,
    /// Semitones.
    pub bend: f32,
    /// 0..1.
    pub mod_wheel: f32,
}

fn note_to_hz(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

fn xorshift(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// A voice's own quirks, fixed for its life: each -1..1, scaled by Vintage.
#[derive(Clone, Copy, Debug)]
struct Character {
    tune1: f32,
    tune2: f32,
    cutoff: f32,
    envelopes: f32,
}

#[derive(Clone, Debug)]
pub struct Voice {
    sample_rate: f32,
    osc1: Osc,
    osc2: Osc,
    rng: u32,
    filter: SemFilter,
    filter_env: Adsr,
    amp_env: Adsr,
    aux_env: Adsr,
    character: Character,
    /// The oscillators' slow wander, -1..1, and where each is heading.
    drift: [f32; 2],
    drift_target: [f32; 2],
    drift_clock: u32,
    drift_rate: f32,
    /// The note asked for, and how far the sounding pitch is from it while
    /// gliding, in semitones. Kept apart so the glide can settle to exactly
    /// zero: added together in one f32 it would stall short near the end.
    note: f32,
    glide_offset: f32,
    glide: f32,
    played: bool,
    velocity: f32,
    random: f32,
    /// Fixed offset in cents, for unison.
    detune: f32,
    /// Where the voice sits in the stereo field, -1..1, before modulation.
    pan: f32,
}

impl Voice {
    /// `index` picks the voice's character, so voice 3 always sounds like voice 3.
    pub fn new(sample_rate: f32, index: usize) -> Self {
        let mut seed = 0x9E37_79B9u32 ^ (index as u32 + 1).wrapping_mul(0x85EB_CA6B);
        for _ in 0..4 {
            xorshift(&mut seed);
        }
        let character = Character {
            tune1: xorshift(&mut seed),
            tune2: xorshift(&mut seed),
            cutoff: xorshift(&mut seed),
            envelopes: xorshift(&mut seed),
        };
        Self {
            sample_rate,
            osc1: Osc::new(),
            osc2: Osc::new(),
            rng: seed,
            filter: SemFilter::default(),
            filter_env: Adsr::default(),
            amp_env: Adsr::default(),
            aux_env: Adsr::default(),
            character,
            drift: [0.0; 2],
            drift_target: [0.0; 2],
            drift_clock: index as u32 * (DRIFT_STEP / 5),
            drift_rate: 1.0 / (DRIFT_SECONDS * sample_rate),
            note: 60.0,
            glide_offset: 0.0,
            glide: 0.0,
            played: false,
            velocity: 1.0,
            random: 0.0,
            detune: 0.0,
            pan: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.osc1.reset(0.0);
        self.osc2.reset(0.0);
        self.filter.reset();
        self.filter_env.reset();
        self.amp_env.reset();
        self.aux_env.reset();
        self.played = false;
    }

    /// Start a note. With `retrigger` false the pitch moves but the envelopes
    /// carry on (legato). `detune` is in cents.
    pub fn note_on(&mut self, note: u8, velocity: f32, detune: f32, retrigger: bool) {
        let from = self.pitch();
        self.note = note as f32;
        self.detune = detune;
        // Glide from wherever the voice was, unless it has nothing to glide from yet.
        self.glide_offset = if self.played { from - self.note } else { 0.0 };
        self.played = true;
        if retrigger {
            self.velocity = velocity.clamp(0.0, 1.0);
            self.random = xorshift(&mut self.rng);
            self.filter_env.gate_on();
            self.amp_env.gate_on();
            self.aux_env.gate_on();
        }
    }

    pub fn note_off(&mut self) {
        self.filter_env.gate_off();
        self.amp_env.gate_off();
        self.aux_env.gate_off();
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    /// The note this voice was last given.
    pub fn note(&self) -> u8 {
        self.note as u8
    }

    /// Whether its key is still down (not yet in its release).
    pub fn is_gated(&self) -> bool {
        matches!(self.amp_env.stage(), Stage::Attack | Stage::Decay)
    }

    /// The pitch sounding now, in MIDI note numbers, glide included.
    pub fn pitch(&self) -> f32 {
        self.note + self.glide_offset
    }

    /// Cents, for unison.
    pub fn set_detune(&mut self, cents: f32) {
        self.detune = cents;
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan;
    }

    /// Take the settings that don't need updating every sample: envelope
    /// times (stretched by this voice's character) and glide.
    pub fn set_block(&mut self, p: &Patch) {
        let stretch = 1.0 + p.vintage * VINTAGE_ENV_SPREAD * self.character.envelopes;
        let sr = self.sample_rate;
        let set = |env: &mut Adsr, [a, d, s, r]: [f32; 4]| {
            env.set(a * stretch, d * stretch, s, r * stretch, sr)
        };
        set(&mut self.filter_env, p.filter_env);
        set(&mut self.amp_env, p.amp_env);
        set(&mut self.aux_env, p.aux_env);
        self.glide = if p.glide < 0.001 {
            0.0
        } else {
            // About 95% of the way there in the glide time.
            (-3.0 / (p.glide * sr)).exp()
        };
    }

    /// One sample: the signal and where it sits in the stereo field (-1..1).
    #[inline]
    pub fn next(&mut self, p: &Patch, shared: Shared) -> (f32, f32) {
        let fe = self.filter_env.advance();
        let ae = self.amp_env.advance();
        let xe = self.aux_env.advance();

        let mut src = [0.0; ModSource::COUNT];
        src[ModSource::Lfo1 as usize] = shared.lfo1;
        src[ModSource::Lfo2 as usize] = shared.lfo2;
        src[ModSource::FilterEnv as usize] = fe;
        src[ModSource::AmpEnv as usize] = ae;
        src[ModSource::AuxEnv as usize] = xe;
        src[ModSource::Velocity as usize] = self.velocity;
        src[ModSource::Key as usize] = (self.note - 60.0) / 60.0;
        src[ModSource::Random as usize] = self.random;
        src[ModSource::ModWheel as usize] = shared.mod_wheel;
        let m = route(&p.mods, &src);
        let md = |d: ModDest| m[d as usize];

        self.glide_offset *= self.glide;
        let pitch = self.pitch();
        self.drift_clock += 1;
        if self.drift_clock >= DRIFT_STEP {
            self.drift_clock = 0;
            self.drift_target = [xorshift(&mut self.rng), xorshift(&mut self.rng)];
        }
        for i in 0..2 {
            self.drift[i] += (self.drift_target[i] - self.drift[i]) * self.drift_rate;
        }

        let c = self.character;
        let v = p.vintage;
        let cents1 = v * (VINTAGE_TUNE_CENTS * c.tune1 + VINTAGE_DRIFT_CENTS * self.drift[0]);
        let cents2 = v * (VINTAGE_TUNE_CENTS * c.tune2 + VINTAGE_DRIFT_CENTS * self.drift[1]);
        let common =
            pitch + shared.bend + self.detune / 100.0 + PITCH_RANGE_SEMITONES * md(ModDest::Pitch);
        let n1 = common
            + 12.0 * p.osc1_octave as f32
            + cents1 / 100.0
            + PITCH_RANGE_SEMITONES * md(ModDest::Osc1Pitch);
        let n2 = common
            + 12.0 * p.osc2_octave as f32
            + p.osc2_semitones as f32
            + (p.osc2_cents + cents2) / 100.0
            + PITCH_RANGE_SEMITONES * md(ModDest::Osc2Pitch);

        let sr = self.sample_rate;
        let shape1 = Shape::from_control(p.osc1_shape + md(ModDest::Osc1Shape));
        let shape2 = Shape::from_control(p.osc2_shape + md(ModDest::Osc2Shape));
        let (o1, wrapped) = self.osc1.next(note_to_hz(n1) / sr, shape1, None);
        let sync = if p.sync { wrapped } else { None };
        let (o2, _) = self.osc2.next(note_to_hz(n2) / sr, shape2, sync);
        let noise = xorshift(&mut self.rng);

        let level = |base: f32, d: ModDest| (base + md(d)).clamp(0.0, 1.0);
        let mix = o1 * level(p.osc1_level, ModDest::Osc1Level)
            + o2 * level(p.osc2_level, ModDest::Osc2Level)
            + noise * level(p.noise_level, ModDest::NoiseLevel);

        let vel_f = 1.0 - p.filter_velocity + p.filter_velocity * self.velocity;
        let octaves = p.filter_env_amount * FILTER_ENV_OCTAVES * fe * vel_f
            + p.key_track * (pitch - 60.0) / 12.0
            + v * VINTAGE_CUTOFF_OCTAVES * c.cutoff
            + CUTOFF_RANGE_OCTAVES * md(ModDest::Cutoff);
        let cutoff = p.cutoff_hz * octaves.exp2();
        let resonance = (p.resonance + md(ModDest::Resonance)).clamp(0.0, 1.0);
        let coeffs = Coeffs::new(cutoff, resonance, sr);
        let mode = (p.filter_mode + md(ModDest::FilterMode)).clamp(0.0, 1.0);
        let drive = 1.0 + 3.0 * (p.drive + md(ModDest::Drive)).clamp(0.0, 1.0);
        let y = self.filter.process(mix, coeffs, mode, p.bandpass, drive);

        let vel_a = 1.0 - p.amp_velocity + p.amp_velocity * self.velocity;
        let amp = ae * vel_a * (1.0 + md(ModDest::Amp)).max(0.0);
        let pan = (self.pan + md(ModDest::Pan)).clamp(-1.0, 1.0);
        (y * amp, pan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn clean() -> Patch {
        Patch {
            osc2_level: 0.0,
            cutoff_hz: 20_000.0,
            filter_env_amount: 0.0,
            resonance: 0.0,
            amp_env: [0.001, 1.0, 1.0, 0.1],
            vintage: 0.0,
            ..Patch::default()
        }
    }

    /// Cycles per second of osc 1 alone: the rising saw crosses zero
    /// downwards only at its reset.
    fn measure_hz(v: &mut Voice, p: &Patch) -> usize {
        for _ in 0..4_800 {
            v.next(p, Shared::default());
        }
        let x: Vec<f32> = (0..48_000)
            .map(|_| v.next(p, Shared::default()).0)
            .collect();
        x.windows(2).filter(|w| w[0] >= 0.0 && w[1] < 0.0).count()
    }

    #[test]
    fn plays_and_stops() {
        let p = Patch::default();
        let mut v = Voice::new(SR, 0);
        v.set_block(&p);
        assert!(!v.is_active());
        v.note_on(48, 1.0, 0.0, true);
        let held: Vec<f32> = (0..24_000)
            .map(|_| v.next(&p, Shared::default()).0)
            .collect();
        let peak = held.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.2 && peak < 2.0, "{peak}");
        v.note_off();
        let mut n = 0;
        while v.is_active() {
            let (y, _) = v.next(&p, Shared::default());
            assert!(y.is_finite());
            n += 1;
        }
        assert!(n < 48_000);
    }

    #[test]
    fn pitch_is_right() {
        let p = clean();
        let mut v = Voice::new(SR, 0);
        v.set_block(&p);
        v.note_on(45, 1.0, 0.0, true);
        assert!((109..=111).contains(&measure_hz(&mut v, &p)));
    }

    #[test]
    fn glide_takes_its_time() {
        let p = Patch {
            glide: 0.5,
            ..clean()
        };
        let mut v = Voice::new(SR, 0);
        v.set_block(&p);
        v.note_on(48, 1.0, 0.0, true);
        v.note_on(60, 1.0, 0.0, false);
        for _ in 0..(SR as usize / 10) {
            v.next(&p, Shared::default());
        }
        let after_100ms = v.pitch();
        assert!(after_100ms > 49.0 && after_100ms < 59.0, "{after_100ms}");
        // It's an RC glide, so it settles exponentially: well there after four glide times.
        for _ in 0..(2 * SR as usize) {
            v.next(&p, Shared::default());
        }
        assert!((v.pitch() - 60.0).abs() < 0.01, "{}", v.pitch());
    }

    #[test]
    fn vintage_makes_voices_differ() {
        let render = |vintage: f32, index: usize| {
            let p = Patch { vintage, ..clean() };
            let mut v = Voice::new(SR, index);
            v.set_block(&p);
            v.note_on(81, 1.0, 0.0, true); // 880 Hz, so cents show up as cycles
            measure_hz(&mut v, &p)
        };
        assert_eq!(render(0.0, 0), render(0.0, 3));
        let spread: Vec<usize> = (0..5).map(|i| render(1.0, i)).collect();
        assert!(spread.iter().any(|&f| f != spread[0]), "{spread:?}");
        // But still in tune to within the stated range (about ±16 cents at 880 Hz is ±8 Hz).
        assert!(
            spread.iter().all(|&f| (871..=889).contains(&f)),
            "{spread:?}"
        );
    }

    #[test]
    fn lfo_to_pitch_vibrato_moves_the_pitch() {
        let mut p = clean();
        p.mods[0] = ModSlot {
            source: ModSource::Lfo1,
            dest: ModDest::Pitch,
            amount: 1.0,
        };
        let mut v = Voice::new(SR, 0);
        v.set_block(&p);
        v.note_on(57, 1.0, 0.0, true);
        let up = Shared {
            lfo1: 1.0,
            ..Shared::default()
        };
        for _ in 0..4_800 {
            v.next(&p, up);
        }
        let x: Vec<f32> = (0..48_000).map(|_| v.next(&p, up).0).collect();
        let hz = x.windows(2).filter(|w| w[0] >= 0.0 && w[1] < 0.0).count();
        // A full-scale LFO moves the pitch an octave: 220 Hz becomes 440.
        assert!((438..=442).contains(&hz), "{hz}");
    }
}
