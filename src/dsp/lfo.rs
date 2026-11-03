//! Free-running LFOs, shared by all voices as on most analog polys.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LfoShape {
    #[default]
    Triangle,
    Sine,
    SawUp,
    SawDown,
    Square,
    /// A new random level each cycle.
    SampleHold,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LfoSettings {
    pub rate_hz: f32,
    pub shape: LfoShape,
}

impl Default for LfoSettings {
    fn default() -> Self {
        Self {
            rate_hz: 5.0,
            shape: LfoShape::Triangle,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Lfo {
    phase: f32,
    held: f32,
    rng: u32,
}

impl Lfo {
    pub fn new(seed: u32) -> Self {
        Self {
            phase: 0.0,
            held: 0.0,
            rng: seed | 1,
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Advance one sample; the output is bipolar, -1..1.
    #[inline]
    pub fn next(&mut self, s: LfoSettings, sample_rate: f32) -> f32 {
        self.phase += s.rate_hz.max(0.0) / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 17;
            self.rng ^= self.rng << 5;
            self.held = (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
        let p = self.phase;
        match s.shape {
            LfoShape::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
            LfoShape::Sine => (std::f32::consts::TAU * p).sin(),
            LfoShape::SawUp => 2.0 * p - 1.0,
            LfoShape::SawDown => 1.0 - 2.0 * p,
            LfoShape::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoShape::SampleHold => self.held,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_is_bipolar_and_in_range() {
        for shape in [
            LfoShape::Triangle,
            LfoShape::Sine,
            LfoShape::SawUp,
            LfoShape::SawDown,
            LfoShape::Square,
            LfoShape::SampleHold,
        ] {
            let mut l = Lfo::new(7);
            let s = LfoSettings {
                rate_hz: 3.0,
                shape,
            };
            let x: Vec<f32> = (0..48_000).map(|_| l.next(s, 48_000.0)).collect();
            let (lo, hi) = x
                .iter()
                .fold((1.0f32, -1.0f32), |(a, b), &v| (a.min(v), b.max(v)));
            assert!(lo >= -1.0 && hi <= 1.0, "{shape:?}");
            assert!(hi - lo > 0.5, "{shape:?} barely moves");
        }
    }

    #[test]
    fn rate_is_right() {
        let mut l = Lfo::new(1);
        let s = LfoSettings {
            rate_hz: 4.0,
            shape: LfoShape::Square,
        };
        let x: Vec<f32> = (0..48_000).map(|_| l.next(s, 48_000.0)).collect();
        let rises = x.windows(2).filter(|w| w[0] < 0.0 && w[1] > 0.0).count();
        assert!((3..=4).contains(&rises), "{rises}");
    }
}
