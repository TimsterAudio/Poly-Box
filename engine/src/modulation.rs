//! The modulation matrix: a handful of slots, each routing one source to one
//! destination by a signed amount.

/// How many slots the matrix has.
pub const MOD_SLOTS: usize = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModSource {
    #[default]
    Off,
    Lfo1,
    Lfo2,
    FilterEnv,
    AmpEnv,
    AuxEnv,
    /// The note's velocity, 0..1.
    Velocity,
    /// The note's position on the keyboard: 0 at middle C, ±1 five octaves away.
    Key,
    /// A random value picked when the note starts, -1..1.
    Random,
    /// The mod wheel, 0..1.
    ModWheel,
}

impl ModSource {
    pub const COUNT: usize = 10;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModDest {
    #[default]
    Off,
    /// Both oscillators.
    Pitch,
    Osc1Pitch,
    Osc2Pitch,
    Osc1Shape,
    Osc2Shape,
    Osc1Level,
    Osc2Level,
    NoiseLevel,
    Cutoff,
    Resonance,
    FilterMode,
    Drive,
    Amp,
    Pan,
}

impl ModDest {
    pub const COUNT: usize = 15;
}

/// How far a full-scale (±1) modulation moves each kind of destination.
pub const PITCH_RANGE_SEMITONES: f32 = 12.0;
pub const CUTOFF_RANGE_OCTAVES: f32 = 6.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModSlot {
    pub source: ModSource,
    pub dest: ModDest,
    /// -1..1.
    pub amount: f32,
}

/// Sum every slot's contribution per destination. `sources` is indexed by
/// [`ModSource`] as `usize`; the result by [`ModDest`].
#[inline]
pub fn route(
    slots: &[ModSlot; MOD_SLOTS],
    sources: &[f32; ModSource::COUNT],
) -> [f32; ModDest::COUNT] {
    let mut out = [0.0; ModDest::COUNT];
    for s in slots {
        if s.source != ModSource::Off && s.dest != ModDest::Off && s.amount != 0.0 {
            out[s.dest as usize] += sources[s.source as usize] * s.amount;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_add_up_and_off_does_nothing() {
        let mut slots = [ModSlot::default(); MOD_SLOTS];
        slots[0] = ModSlot {
            source: ModSource::Lfo1,
            dest: ModDest::Cutoff,
            amount: 0.5,
        };
        slots[1] = ModSlot {
            source: ModSource::Velocity,
            dest: ModDest::Cutoff,
            amount: -0.25,
        };
        slots[2] = ModSlot {
            source: ModSource::Off,
            dest: ModDest::Pitch,
            amount: 1.0,
        };
        let mut src = [0.0; ModSource::COUNT];
        src[ModSource::Lfo1 as usize] = 1.0;
        src[ModSource::Velocity as usize] = 1.0;
        src[ModSource::Off as usize] = 1.0;
        let out = route(&slots, &src);
        assert!((out[ModDest::Cutoff as usize] - 0.25).abs() < 1e-6);
        assert_eq!(out[ModDest::Pitch as usize], 0.0);
    }
}
