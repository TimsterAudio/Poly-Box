//! The SEM-style filter: a 12 dB/octave state-variable filter whose mode
//! control sweeps low-pass, through notch, to high-pass, with a separate
//! band-pass switch, as on the Oberheim SEM.
//!
//! A zero-delay-feedback (topology-preserving) state-variable filter, so it
//! tracks the cutoff accurately up to near Nyquist and stays stable under fast
//! modulation. Two soft saturators stand in for the analog circuit: one at the
//! input (the drive control) and one on the band-pass integrator, which rounds
//! off the resonance peak on loud signals the way the SEM's does instead of
//! letting it ring without limit.

use std::f32::consts::PI;

/// The level at which the band-pass integrator starts to saturate.
const STATE_LIMIT: f32 = 2.5;

/// Resonance never quite reaches self-oscillation, like the SEM's.
const MIN_DAMPING: f32 = 0.04;

/// A quick, smooth tanh approximation, accurate to within a percent or so
/// and exactly ±1 past ±3.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

/// Where the filter is set, worked out once per sample and shared by
/// whatever voices use the same settings.
#[derive(Clone, Copy, Debug)]
pub struct Coeffs {
    g: f32,
    k: f32,
}

impl Coeffs {
    /// `resonance` runs 0..1; at 1 the filter rings hard but doesn't
    /// self-oscillate.
    pub fn new(cutoff_hz: f32, resonance: f32, sample_rate: f32) -> Self {
        let fc = cutoff_hz.clamp(5.0, 0.45 * sample_rate);
        let g = (PI * fc / sample_rate).tan();
        let k = 2.0 - (2.0 - MIN_DAMPING) * resonance.clamp(0.0, 1.0);
        Self { g, k }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SemFilter {
    ic1: f32,
    ic2: f32,
}

impl SemFilter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Filter one sample.
    ///
    /// `mode` runs 0 (low-pass) through 0.5 (notch) to 1 (high-pass);
    /// `bandpass` overrides it. `drive` is the input gain into the saturator,
    /// 1 for clean.
    #[inline]
    pub fn process(&mut self, x: f32, c: Coeffs, mode: f32, bandpass: bool, drive: f32) -> f32 {
        let x = soft_clip(x * drive) / drive.sqrt();

        let a1 = 1.0 / (1.0 + c.g * (c.g + c.k));
        let a2 = c.g * a1;
        let a3 = c.g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = STATE_LIMIT * soft_clip((2.0 * v1 - self.ic1) / STATE_LIMIT);
        self.ic2 = 2.0 * v2 - self.ic2;

        let lp = v2;
        let bp = v1;
        let hp = x - c.k * v1 - v2;

        if bandpass {
            // Scaled so the band-pass is about as loud as the other modes at
            // low resonance, and rises with it as on the SEM.
            return bp * 1.5;
        }
        let m = mode.clamp(0.0, 1.0);
        // Crossfade low- and high-pass; halfway between they cancel at the
        // cutoff, which is the notch. Rescale so the middle isn't quieter.
        (lp * (1.0 - m) + hp * m) / (1.0 - m).max(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Steady-state level of a sine at `freq` through the filter.
    fn gain_at(freq: f32, cutoff: f32, res: f32, mode: f32, bandpass: bool) -> f32 {
        let mut f = SemFilter::default();
        let c = Coeffs::new(cutoff, res, SR);
        let amp = 0.1; // quiet, so the saturators stay out of it
        let n = 48_000;
        let mut peak = 0.0f32;
        for i in 0..n {
            let x = amp * (2.0 * PI * freq * i as f32 / SR).sin();
            let y = f.process(x, c, mode, bandpass, 1.0);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak / amp
    }

    #[test]
    fn low_pass_passes_lows_and_cuts_highs_at_12_db() {
        assert!((gain_at(100.0, 1_000.0, 0.0, 0.0, false) - 1.0).abs() < 0.05);
        // One octave above, 12 dB/oct gives about a quarter; at ten times, a hundredth.
        let g10 = gain_at(10_000.0, 1_000.0, 0.0, 0.0, false);
        assert!(g10 < 0.015 && g10 > 0.005, "{g10}");
    }

    #[test]
    fn high_pass_is_the_mirror() {
        assert!((gain_at(10_000.0, 1_000.0, 0.0, 1.0, false) - 1.0).abs() < 0.05);
        assert!(gain_at(100.0, 1_000.0, 0.0, 1.0, false) < 0.015);
    }

    #[test]
    fn notch_in_the_middle() {
        let at_cutoff = gain_at(1_000.0, 1_000.0, 0.0, 0.5, false);
        let away = gain_at(100.0, 1_000.0, 0.0, 0.5, false);
        assert!(at_cutoff < 0.05, "{at_cutoff}");
        assert!(away > 0.9, "{away}");
    }

    #[test]
    fn resonance_lifts_the_cutoff() {
        let flat = gain_at(1_000.0, 1_000.0, 0.0, 0.0, false);
        let res = gain_at(1_000.0, 1_000.0, 0.9, 0.0, false);
        assert!(res > 4.0 * flat, "{flat} -> {res}");
    }

    #[test]
    fn band_pass_peaks_at_cutoff() {
        assert!(
            gain_at(1_000.0, 1_000.0, 0.0, 0.0, true)
                > 3.0 * gain_at(100.0, 1_000.0, 0.0, 0.0, true)
        );
    }

    #[test]
    fn stays_bounded_when_driven_hard_at_full_resonance() {
        let mut f = SemFilter::default();
        let mut peak = 0.0f32;
        for i in 0..96_000 {
            // A loud square swept through the range while the cutoff moves fast.
            let x = if (i / 37) % 2 == 0 { 3.0 } else { -3.0 };
            let cutoff = 50.0 * 2f32.powf(9.0 * ((i as f32 / 3_000.0).sin() * 0.5 + 0.5));
            let y = f.process(x, Coeffs::new(cutoff, 1.0, SR), 0.0, false, 4.0);
            assert!(y.is_finite());
            peak = peak.max(y.abs());
        }
        assert!(peak < 20.0, "{peak}");
    }
}
