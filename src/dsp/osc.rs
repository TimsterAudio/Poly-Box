//! The VCO: one continuous waveshape control that runs from sawtooth through
//! square to a narrow pulse, with hard sync to another oscillator.
//!
//! Band-limited with polyBLEP. Each discontinuity (the saw's reset, the
//! pulse's edges, a sync reset) is smoothed over the two samples either side
//! of it, so the oscillator runs one sample late: the sample before an edge is
//! held back until we know whether an edge follows it.

/// Most edges one sample can hold: a pulse edge and a wrap before a sync
/// reset, the reset itself, and a pulse edge and a wrap after it.
const MAX_EDGES: usize = 6;

/// The highest phase increment allowed (cycles per sample), just under
/// Nyquist, so one sample never spans more than one wrap.
const MAX_INC: f32 = 0.45;

#[derive(Clone, Copy, Default)]
struct Edge {
    /// Time from the edge to the current sample point, in samples (0..1).
    d: f32,
    /// How far the waveform jumps at the edge.
    h: f32,
}

#[derive(Default)]
struct Edges {
    list: [Edge; MAX_EDGES],
    len: usize,
}

impl Edges {
    fn push(&mut self, d: f32, h: f32) {
        if self.len < MAX_EDGES && h != 0.0 {
            self.list[self.len] = Edge {
                d: d.clamp(0.0, 1.0),
                h,
            };
            self.len += 1;
        }
    }
}

/// What the shape control means at a given setting: how much of the pulse is
/// mixed in against the saw, and the pulse's width.
///
/// 0 is a sawtooth, 0.5 a square, 1 a 5% pulse. From 0 to 0.5 the saw fades
/// into the square; from 0.5 to 1 the square narrows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub pulse: f32,
    pub width: f32,
}

impl Shape {
    pub fn from_control(shape: f32) -> Self {
        let s = shape.clamp(0.0, 1.0);
        if s < 0.5 {
            Shape {
                pulse: s * 2.0,
                width: 0.5,
            }
        } else {
            Shape {
                pulse: 1.0,
                width: 0.5 - (s - 0.5) * 0.9,
            }
        }
    }

    /// The waveform at phase `p` (0..1), before band-limiting.
    fn value(self, p: f32) -> f32 {
        let saw = 2.0 * p - 1.0;
        let pulse = if p < self.width { 1.0 } else { -1.0 };
        (1.0 - self.pulse) * saw + self.pulse * pulse
    }
}

#[derive(Clone, Debug, Default)]
pub struct Osc {
    phase: f32,
    /// The previous sample, still open to corrections from edges just after it.
    held: f32,
}

impl Osc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self, phase: f32) {
        self.phase = phase.rem_euclid(1.0);
        self.held = 0.0;
    }

    /// Advance one sample.
    ///
    /// `inc` is the frequency in cycles per sample. `sync` is the time, in
    /// samples before this sample point, at which the master oscillator
    /// wrapped (what this function returns as its second value), if it did.
    ///
    /// Returns the output and, if this oscillator wrapped during the sample,
    /// how long before the sample point it did, for syncing another to it.
    pub fn next(&mut self, inc: f32, shape: Shape, sync: Option<f32>) -> (f32, Option<f32>) {
        let inc = inc.clamp(1.0e-7, MAX_INC);
        let mut edges = Edges::default();
        let mut wrapped = None;

        let end = match sync {
            Some(ds) => {
                let ds = ds.clamp(0.0, 1.0);
                // Run up to the reset, jump back to phase 0, run on from there.
                let at_reset = run(
                    self.phase,
                    (1.0 - ds) * inc,
                    ds,
                    inc,
                    shape,
                    &mut edges,
                    &mut wrapped,
                );
                edges.push(ds, shape.value(0.0) - shape.value(at_reset));
                // A sync reset counts as a wrap for anything synced to this one.
                wrapped = Some(ds);
                run(0.0, ds * inc, 0.0, inc, shape, &mut edges, &mut wrapped)
            }
            None => run(self.phase, inc, 0.0, inc, shape, &mut edges, &mut wrapped),
        };
        self.phase = end;

        let mut current = shape.value(end);
        let mut out = self.held;
        for e in &edges.list[..edges.len] {
            let d = e.d;
            current += 0.5 * e.h * (2.0 * d - d * d - 1.0);
            out += 0.5 * e.h * d * d;
        }
        self.held = current;
        (out, wrapped)
    }
}

/// Move the phase from `p0` on by `len`, recording the edges crossed.
/// `end_d` is the time from the end of this stretch to the sample point.
/// Returns the phase at the end of the stretch.
fn run(
    p0: f32,
    len: f32,
    end_d: f32,
    inc: f32,
    shape: Shape,
    edges: &mut Edges,
    wrapped: &mut Option<f32>,
) -> f32 {
    let pe = p0 + len;
    let time_to = |x: f32| end_d + (pe - x) / inc;
    let fall = -2.0 * shape.pulse;
    if p0 < shape.width && pe >= shape.width {
        edges.push(time_to(shape.width), fall);
    }
    if pe >= 1.0 {
        let d = time_to(1.0);
        edges.push(d, shape.value(0.0) - shape.value(0.999_999));
        *wrapped = Some(d);
        let p = pe - 1.0;
        if p >= shape.width {
            edges.push(time_to(1.0 + shape.width), fall);
        }
        return p;
    }
    pe
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(freq: f32, sr: f32, shape: f32, n: usize) -> Vec<f32> {
        let mut o = Osc::new();
        let s = Shape::from_control(shape);
        (0..n).map(|_| o.next(freq / sr, s, None).0).collect()
    }

    #[test]
    fn shapes_run_saw_square_pulse() {
        assert_eq!(
            Shape::from_control(0.0),
            Shape {
                pulse: 0.0,
                width: 0.5
            }
        );
        assert_eq!(
            Shape::from_control(0.5),
            Shape {
                pulse: 1.0,
                width: 0.5
            }
        );
        let narrow = Shape::from_control(1.0);
        assert!((narrow.width - 0.05).abs() < 1e-6);
    }

    #[test]
    fn stays_in_range_and_has_no_dc_for_saw_and_square() {
        for shape in [0.0, 0.25, 0.5] {
            let x = render(220.0, 48_000.0, shape, 48_000);
            let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak < 1.2, "shape {shape}: peak {peak}");
            let mean = x.iter().sum::<f32>() / x.len() as f32;
            assert!(mean.abs() < 0.01, "shape {shape}: DC {mean}");
        }
    }

    /// The band-limited saw should alias far less than a naive one: compare the
    /// energy left after removing the ideal harmonics' fundamental period.
    #[test]
    fn polyblep_saw_is_smoother_than_naive() {
        let sr = 48_000.0;
        let f = 3_517.0;
        let x = render(f, sr, 0.0, 4_800);
        // Second difference measures the energy of sharp corners.
        let rough = |v: &[f32]| {
            v.windows(3)
                .map(|w| (w[0] - 2.0 * w[1] + w[2]).powi(2))
                .sum::<f32>()
        };
        let naive: Vec<f32> = (0..4_800)
            .map(|i| 2.0 * ((i as f32 * f / sr) % 1.0) - 1.0)
            .collect();
        assert!(rough(&x[10..]) < 0.5 * rough(&naive[10..]));
    }

    #[test]
    fn sync_locks_the_slave_to_the_master_period() {
        let sr = 48_000.0;
        let mut m = Osc::new();
        let mut s = Osc::new();
        let saw = Shape::from_control(0.0);
        let period = 480; // 100 Hz master
        let out: Vec<f32> = (0..period * 20)
            .map(|_| {
                let (_, w) = m.next(100.0 / sr, saw, None);
                s.next(337.0 / sr, saw, w).0
            })
            .collect();
        // Once running, each master period repeats (to within the rounding
        // of the master's own phase), although 337 Hz alone wouldn't.
        let a = &out[period * 10..period * 11];
        let b = &out[period * 11..period * 12];
        let diff = a
            .iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(diff < 0.02, "sync period mismatch {diff}");
    }
}
