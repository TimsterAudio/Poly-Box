//! ADSR envelope with analog-style curves: the attack is a capacitor charging
//! towards a target above full scale (so it rises fast and bends over at the
//! top), and the decay and release fall exponentially. A new note starts its
//! attack from wherever the envelope is, as analog envelopes do.

/// The attack aims past 1.0 so it reaches the top in a finite time.
const ATTACK_TARGET: f32 = 1.3;
/// ln(1.3 / 0.3): time constants from 0 to 1 when aiming at 1.3.
const ATTACK_SPAN: f32 = 1.466_337;
/// ln(1000): decay and release times are to -60 dB.
const FALL_SPAN: f32 = 6.907_755;
/// Below this the envelope counts as finished (-80 dB).
const SILENT: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    /// Falling to the sustain level, then holding there while the key is down.
    Decay,
    Release,
}

#[derive(Clone, Debug)]
pub struct Adsr {
    stage: Stage,
    value: f32,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self {
            stage: Stage::Idle,
            value: 0.0,
            attack: 0.0,
            decay: 0.0,
            sustain: 1.0,
            release: 0.0,
        }
    }
}

/// Per-sample pole for an exponential that covers `span` time constants in
/// `seconds`.
fn pole(seconds: f32, span: f32, sample_rate: f32) -> f32 {
    let samples = (seconds * sample_rate).max(1.0);
    (-span / samples).exp()
}

impl Adsr {
    /// Times in seconds, sustain 0..1.
    pub fn set(&mut self, attack: f32, decay: f32, sustain: f32, release: f32, sample_rate: f32) {
        self.attack = pole(attack, ATTACK_SPAN, sample_rate);
        self.decay = pole(decay, FALL_SPAN, sample_rate);
        self.sustain = sustain.clamp(0.0, 1.0);
        self.release = pole(release, FALL_SPAN, sample_rate);
    }

    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.value = 0.0;
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    #[inline]
    pub fn advance(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.value = ATTACK_TARGET + (self.value - ATTACK_TARGET) * self.attack;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.value = self.sustain + (self.value - self.sustain) * self.decay;
            }
            Stage::Release => {
                self.value *= self.release;
                if self.value < SILENT {
                    self.value = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn attack_time_is_honoured() {
        let mut e = Adsr::default();
        e.set(0.1, 0.5, 0.5, 0.5, SR);
        e.gate_on();
        let mut n = 0;
        while e.stage() == Stage::Attack {
            e.advance();
            n += 1;
        }
        let secs = n as f32 / SR;
        assert!((secs - 0.1).abs() < 0.002, "{secs}");
    }

    #[test]
    fn settles_on_sustain_then_releases_to_idle() {
        let mut e = Adsr::default();
        e.set(0.005, 0.1, 0.4, 0.2, SR);
        e.gate_on();
        for _ in 0..(SR as usize) {
            e.advance();
        }
        assert!((e.advance() - 0.4).abs() < 1e-3);
        e.gate_off();
        let mut n = 0;
        while e.is_active() {
            e.advance();
            n += 1;
        }
        // From 0.4 to -80 dB of full scale is a bit under the -60 dB release time plus a third.
        let secs = n as f32 / SR;
        assert!(secs > 0.2 && secs < 0.3, "{secs}");
    }

    #[test]
    fn retrigger_starts_from_where_it_is() {
        let mut e = Adsr::default();
        e.set(0.05, 0.1, 0.5, 1.0, SR);
        e.gate_on();
        for _ in 0..2_000 {
            e.advance();
        }
        e.gate_off();
        for _ in 0..1_000 {
            e.advance();
        }
        let before = e.advance();
        e.gate_on();
        let after = e.advance();
        assert!(after >= before && after - before < 0.05);
    }
}
