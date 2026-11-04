//! Every control on the panel: its saved name, label, range and readout, and
//! how the set of values becomes the engine's [`Patch`].

use poly_box_engine::{
    LfoSettings, LfoShape, ModDest, ModSlot, ModSource, Patch, VoiceMode, MOD_SLOTS,
};
use taw_panel::widgets::Spec;

/// How a control's value reads in its display.
#[derive(Clone, Copy)]
pub enum Fmt {
    Percent,
    /// -100%..+100%.
    Bipolar,
    Hz,
    Seconds,
    /// Off below 2 ms.
    Glide,
    Cents,
    Semitones,
    /// Octave as organ footage: 32' to 2'.
    Footage,
    Shape,
    FilterMode,
    Db,
    OnOff,
    Choice(&'static [&'static str]),
}

pub struct Def {
    /// Saved in the settings file: never change one.
    pub id: &'static str,
    pub label: &'static str,
    pub spec: Spec,
    pub fmt: Fmt,
}

impl Def {
    /// Discrete controls (choices, octaves, switches) jump; the rest glide
    /// between values in the audio thread so nothing clicks.
    pub fn smoothed(&self) -> bool {
        self.spec.step < 1.0
    }

    pub fn show(&self, v: f32) -> String {
        match self.fmt {
            Fmt::Percent => format!("{:.0}%", v * 100.0),
            Fmt::Bipolar => format!("{:+.0}%", v * 100.0),
            Fmt::Hz => {
                if v >= 1000.0 {
                    format!("{:.2} kHz", v / 1000.0)
                } else if v >= 10.0 {
                    format!("{v:.0} Hz")
                } else {
                    format!("{v:.2} Hz")
                }
            }
            Fmt::Seconds => {
                if v < 1.0 {
                    format!("{:.0} ms", v * 1000.0)
                } else {
                    format!("{v:.2} s")
                }
            }
            Fmt::Glide => {
                if v < GLIDE_OFF {
                    "Off".into()
                } else if v < 1.0 {
                    format!("{:.0} ms", v * 1000.0)
                } else {
                    format!("{v:.2} s")
                }
            }
            Fmt::Cents => format!("{v:+.1}¢"),
            Fmt::Semitones => format!("{:+.0}", v),
            Fmt::Footage => match v.round() as i32 {
                -2 => "32'".into(),
                -1 => "16'".into(),
                0 => "8'".into(),
                1 => "4'".into(),
                _ => "2'".into(),
            },
            Fmt::Shape => {
                if v < 0.01 {
                    "Saw".into()
                } else if (v - 0.5).abs() < 0.01 {
                    "Square".into()
                } else if v < 0.5 {
                    format!("Saw-Sq {:.0}", v * 200.0)
                } else {
                    format!("Pulse {:.0}%", (0.5 - (v - 0.5) * 0.9) * 100.0)
                }
            }
            Fmt::FilterMode => {
                if v < 0.02 {
                    "LP".into()
                } else if (v - 0.5).abs() < 0.02 {
                    "Notch".into()
                } else if v > 0.98 {
                    "HP".into()
                } else if v < 0.5 {
                    format!("LP-N {:.0}", v * 200.0)
                } else {
                    format!("N-HP {:.0}", (v - 0.5) * 200.0)
                }
            }
            Fmt::Db => format!("{v:+.1} dB"),
            Fmt::OnOff => if v > 0.5 { "On" } else { "Off" }.into(),
            Fmt::Choice(names) => names
                .get(v.round().max(0.0) as usize)
                .copied()
                .unwrap_or("?")
                .into(),
        }
    }
}

/// Glide times below this are off.
pub const GLIDE_OFF: f32 = 0.002;

pub const VOICE_MODES: [&str; 3] = ["Poly", "Mono", "Unison"];
const VOICE_MODE_VALUES: [VoiceMode; 3] = [VoiceMode::Poly, VoiceMode::Mono, VoiceMode::Unison];

pub const LFO_SHAPES: [&str; 6] = ["Triangle", "Sine", "Saw Up", "Saw Down", "Square", "S & H"];
const LFO_SHAPE_VALUES: [LfoShape; 6] = [
    LfoShape::Triangle,
    LfoShape::Sine,
    LfoShape::SawUp,
    LfoShape::SawDown,
    LfoShape::Square,
    LfoShape::SampleHold,
];

pub const SOURCES: [&str; ModSource::COUNT] = [
    "Off",
    "LFO 1",
    "LFO 2",
    "Filter Env",
    "Amp Env",
    "Aux Env",
    "Velocity",
    "Key",
    "Random",
    "Mod Wheel",
];
const SOURCE_VALUES: [ModSource; ModSource::COUNT] = [
    ModSource::Off,
    ModSource::Lfo1,
    ModSource::Lfo2,
    ModSource::FilterEnv,
    ModSource::AmpEnv,
    ModSource::AuxEnv,
    ModSource::Velocity,
    ModSource::Key,
    ModSource::Random,
    ModSource::ModWheel,
];

pub const DESTS: [&str; ModDest::COUNT] = [
    "Off",
    "Pitch",
    "Osc 1 Pitch",
    "Osc 2 Pitch",
    "Osc 1 Shape",
    "Osc 2 Shape",
    "Osc 1 Level",
    "Osc 2 Level",
    "Noise",
    "Cutoff",
    "Resonance",
    "Filter Mode",
    "Drive",
    "Amp",
    "Pan",
];
const DEST_VALUES: [ModDest; ModDest::COUNT] = [
    ModDest::Off,
    ModDest::Pitch,
    ModDest::Osc1Pitch,
    ModDest::Osc2Pitch,
    ModDest::Osc1Shape,
    ModDest::Osc2Shape,
    ModDest::Osc1Level,
    ModDest::Osc2Level,
    ModDest::NoiseLevel,
    ModDest::Cutoff,
    ModDest::Resonance,
    ModDest::FilterMode,
    ModDest::Drive,
    ModDest::Amp,
    ModDest::Pan,
];

const fn lin(min: f32, max: f32, default: f32, step: f32) -> Spec {
    Spec {
        min,
        max,
        default,
        log: false,
        step,
    }
}

const fn log(min: f32, max: f32, default: f32) -> Spec {
    Spec {
        min,
        max,
        default,
        log: true,
        step: 0.0,
    }
}

const fn pct(default: f32) -> Spec {
    lin(0.0, 1.0, default, 0.01)
}

const fn choice(n: usize, default: f32) -> Spec {
    lin(0.0, (n - 1) as f32, default, 1.0)
}

const fn def(id: &'static str, label: &'static str, spec: Spec, fmt: Fmt) -> Def {
    Def {
        id,
        label,
        spec,
        fmt,
    }
}

macro_rules! controls {
    ($($name:ident = $def:expr),* $(,)?) => {
        /// Every control, in panel order.
        #[allow(dead_code)] // some are only reached by index (the mod slots)
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(usize)]
        pub enum Ctl { $($name),* }

        pub const DEFS: &[Def] = &[$($def),*];
    };
}

const MOD_AMOUNT: Spec = lin(-1.0, 1.0, 0.0, 0.005);

controls! {
    VoiceMode = def("voice_mode", "Mode", choice(3, 0.0), Fmt::Choice(&VOICE_MODES)),
    UnisonDetune = def("unison_detune", "Detune", lin(0.0, 50.0, 20.0, 0.1), Fmt::Cents),
    Glide = def("glide", "Glide", log(0.001, 5.0, 0.001), Fmt::Glide),
    Vintage = def("vintage", "Vintage", pct(0.3), Fmt::Percent),
    Spread = def("spread", "Spread", pct(0.3), Fmt::Percent),

    Osc1Octave = def("osc1_octave", "Octave", lin(-2.0, 2.0, 0.0, 1.0), Fmt::Footage),
    Osc1Shape = def("osc1_shape", "Shape", pct(0.0), Fmt::Shape),
    Osc2Octave = def("osc2_octave", "Octave", lin(-2.0, 2.0, 0.0, 1.0), Fmt::Footage),
    Osc2Semitones = def("osc2_semitones", "Semi", lin(-12.0, 12.0, 0.0, 1.0), Fmt::Semitones),
    Osc2Fine = def("osc2_fine", "Fine", lin(-50.0, 50.0, 7.0, 0.1), Fmt::Cents),
    Osc2Shape = def("osc2_shape", "Shape", pct(0.0), Fmt::Shape),
    Sync = def("sync", "Sync", lin(0.0, 1.0, 0.0, 1.0), Fmt::OnOff),

    Osc1Level = def("osc1_level", "Osc 1", pct(0.8), Fmt::Percent),
    Osc2Level = def("osc2_level", "Osc 2", pct(0.8), Fmt::Percent),
    Noise = def("noise", "Noise", pct(0.0), Fmt::Percent),

    Cutoff = def("cutoff", "Cutoff", log(20.0, 20_000.0, 1_500.0), Fmt::Hz),
    Resonance = def("resonance", "Reso", pct(0.2), Fmt::Percent),
    FilterMode = def("filter_mode", "Mode", lin(0.0, 1.0, 0.0, 0.01), Fmt::FilterMode),
    Bandpass = def("bandpass", "BP", lin(0.0, 1.0, 0.0, 1.0), Fmt::OnOff),
    Drive = def("drive", "Drive", pct(0.2), Fmt::Percent),
    KeyTrack = def("key_track", "Key Trk", pct(0.5), Fmt::Percent),
    FilterEnvAmount = def("filter_env_amount", "Env Amt", lin(-1.0, 1.0, 0.4, 0.01), Fmt::Bipolar),
    FilterVelocity = def("filter_velocity", "Velocity", pct(0.3), Fmt::Percent),

    FilterAttack = def("filter_attack", "A", log(0.001, 10.0, 0.005), Fmt::Seconds),
    FilterDecay = def("filter_decay", "D", log(0.001, 20.0, 0.4), Fmt::Seconds),
    FilterSustain = def("filter_sustain", "S", pct(0.3), Fmt::Percent),
    FilterRelease = def("filter_release", "R", log(0.001, 20.0, 0.3), Fmt::Seconds),

    AmpAttack = def("amp_attack", "A", log(0.001, 10.0, 0.002), Fmt::Seconds),
    AmpDecay = def("amp_decay", "D", log(0.001, 20.0, 0.5), Fmt::Seconds),
    AmpSustain = def("amp_sustain", "S", pct(0.8), Fmt::Percent),
    AmpRelease = def("amp_release", "R", log(0.001, 20.0, 0.25), Fmt::Seconds),
    AmpVelocity = def("amp_velocity", "Velocity", pct(0.5), Fmt::Percent),

    AuxAttack = def("aux_attack", "A", log(0.001, 10.0, 0.5), Fmt::Seconds),
    AuxDecay = def("aux_decay", "D", log(0.001, 20.0, 1.0), Fmt::Seconds),
    AuxSustain = def("aux_sustain", "S", pct(0.0), Fmt::Percent),
    AuxRelease = def("aux_release", "R", log(0.001, 20.0, 0.5), Fmt::Seconds),

    Lfo1Rate = def("lfo1_rate", "Rate", log(0.02, 40.0, 5.0), Fmt::Hz),
    Lfo1Shape = def("lfo1_shape", "Wave", choice(6, 0.0), Fmt::Choice(&LFO_SHAPES)),
    Lfo2Rate = def("lfo2_rate", "Rate", log(0.02, 40.0, 0.3), Fmt::Hz),
    Lfo2Shape = def("lfo2_shape", "Wave", choice(6, 0.0), Fmt::Choice(&LFO_SHAPES)),

    Mod1Source = def("mod1_source", "Source", choice(ModSource::COUNT, 1.0), Fmt::Choice(&SOURCES)),
    Mod1Dest = def("mod1_dest", "Dest", choice(ModDest::COUNT, 1.0), Fmt::Choice(&DESTS)),
    Mod1Amount = def("mod1_amount", "Mod 1", MOD_AMOUNT, Fmt::Bipolar),
    Mod2Source = def("mod2_source", "Source", choice(ModSource::COUNT, 2.0), Fmt::Choice(&SOURCES)),
    Mod2Dest = def("mod2_dest", "Dest", choice(ModDest::COUNT, 4.0), Fmt::Choice(&DESTS)),
    Mod2Amount = def("mod2_amount", "Mod 2", MOD_AMOUNT, Fmt::Bipolar),
    Mod3Source = def("mod3_source", "Source", choice(ModSource::COUNT, 1.0), Fmt::Choice(&SOURCES)),
    Mod3Dest = def("mod3_dest", "Dest", choice(ModDest::COUNT, 9.0), Fmt::Choice(&DESTS)),
    Mod3Amount = def("mod3_amount", "Mod 3", MOD_AMOUNT, Fmt::Bipolar),
    // So the on-screen mod wheel does something from the start: it opens the filter.
    Mod4Source = def("mod4_source", "Source", choice(ModSource::COUNT, 9.0), Fmt::Choice(&SOURCES)),
    Mod4Dest = def("mod4_dest", "Dest", choice(ModDest::COUNT, 9.0), Fmt::Choice(&DESTS)),
    Mod4Amount = def("mod4_amount", "Mod 4", lin(-1.0, 1.0, 0.4, 0.005), Fmt::Bipolar),
    Mod5Source = def("mod5_source", "Source", choice(ModSource::COUNT, 0.0), Fmt::Choice(&SOURCES)),
    Mod5Dest = def("mod5_dest", "Dest", choice(ModDest::COUNT, 0.0), Fmt::Choice(&DESTS)),
    Mod5Amount = def("mod5_amount", "Mod 5", MOD_AMOUNT, Fmt::Bipolar),
    Mod6Source = def("mod6_source", "Source", choice(ModSource::COUNT, 0.0), Fmt::Choice(&SOURCES)),
    Mod6Dest = def("mod6_dest", "Dest", choice(ModDest::COUNT, 0.0), Fmt::Choice(&DESTS)),
    Mod6Amount = def("mod6_amount", "Mod 6", MOD_AMOUNT, Fmt::Bipolar),

    Volume = def("volume", "Volume", lin(-48.0, 6.0, -6.0, 0.1), Fmt::Db),
}

pub const COUNT: usize = DEFS.len();

impl Ctl {
    #[cfg(test)]
    pub fn def(self) -> &'static Def {
        &DEFS[self as usize]
    }

    /// The name the main display shows: the section and the control.
    pub fn long_label(i: usize) -> String {
        use Ctl as C;
        let at = |c: C| c as usize;
        let group = if i <= at(C::Spread) {
            "Voice"
        } else if i <= at(C::Osc1Shape) {
            "Osc 1"
        } else if i <= at(C::Sync) {
            "Osc 2"
        } else if i <= at(C::Noise) {
            "Mixer"
        } else if i <= at(C::FilterVelocity) {
            "Filter"
        } else if i <= at(C::FilterRelease) {
            "Filter Env"
        } else if i <= at(C::AmpVelocity) {
            "Amp Env"
        } else if i <= at(C::AuxRelease) {
            "Aux Env"
        } else if i <= at(C::Lfo1Shape) {
            "LFO 1"
        } else if i <= at(C::Lfo2Shape) {
            "LFO 2"
        } else if i <= at(C::Mod6Amount) {
            return format!(
                "Mod {} {}",
                (i - at(C::Mod1Source)) / 3 + 1,
                match (i - at(C::Mod1Source)) % 3 {
                    0 => "Source",
                    1 => "Dest",
                    _ => "Amount",
                }
            );
        } else {
            return DEFS[i].label.into();
        };
        let label = match DEFS[i].label {
            "A" => "Attack",
            "D" => "Decay",
            "S" => "Sustain",
            "R" => "Release",
            l => l,
        };
        format!("{group} {label}")
    }

    /// The source, destination and amount controls of mod slot `i` (0-based).
    pub fn mod_slot(i: usize) -> [usize; 3] {
        let first = Ctl::Mod1Source as usize + 3 * i;
        [first, first + 1, first + 2]
    }
}

/// Every control's value, indexed by [`Ctl`].
pub type Values = [f32; COUNT];

pub fn defaults() -> Values {
    std::array::from_fn(|i| DEFS[i].spec.default)
}

fn pick<T: Copy>(table: &[T], v: f32) -> T {
    table[(v.round().max(0.0) as usize).min(table.len() - 1)]
}

/// The engine's patch for a set of values.
pub fn to_patch(v: &Values) -> Patch {
    let g = |c: Ctl| v[c as usize];
    let env = |a: Ctl, d: Ctl, s: Ctl, r: Ctl| [g(a), g(d), g(s), g(r)];
    Patch {
        osc1_octave: g(Ctl::Osc1Octave).round() as i32,
        osc1_shape: g(Ctl::Osc1Shape),
        osc2_octave: g(Ctl::Osc2Octave).round() as i32,
        osc2_semitones: g(Ctl::Osc2Semitones).round() as i32,
        osc2_cents: g(Ctl::Osc2Fine),
        osc2_shape: g(Ctl::Osc2Shape),
        sync: g(Ctl::Sync) > 0.5,
        osc1_level: g(Ctl::Osc1Level),
        osc2_level: g(Ctl::Osc2Level),
        noise_level: g(Ctl::Noise),
        cutoff_hz: g(Ctl::Cutoff),
        resonance: g(Ctl::Resonance),
        filter_mode: g(Ctl::FilterMode),
        bandpass: g(Ctl::Bandpass) > 0.5,
        drive: g(Ctl::Drive),
        key_track: g(Ctl::KeyTrack),
        filter_env_amount: g(Ctl::FilterEnvAmount),
        filter_velocity: g(Ctl::FilterVelocity),
        filter_env: env(
            Ctl::FilterAttack,
            Ctl::FilterDecay,
            Ctl::FilterSustain,
            Ctl::FilterRelease,
        ),
        amp_env: env(
            Ctl::AmpAttack,
            Ctl::AmpDecay,
            Ctl::AmpSustain,
            Ctl::AmpRelease,
        ),
        amp_velocity: g(Ctl::AmpVelocity),
        aux_env: env(
            Ctl::AuxAttack,
            Ctl::AuxDecay,
            Ctl::AuxSustain,
            Ctl::AuxRelease,
        ),
        lfo1: LfoSettings {
            rate_hz: g(Ctl::Lfo1Rate),
            shape: pick(&LFO_SHAPE_VALUES, g(Ctl::Lfo1Shape)),
        },
        lfo2: LfoSettings {
            rate_hz: g(Ctl::Lfo2Rate),
            shape: pick(&LFO_SHAPE_VALUES, g(Ctl::Lfo2Shape)),
        },
        mods: std::array::from_fn(|i| {
            let [s, d, a] = Ctl::mod_slot(i);
            ModSlot {
                source: pick(&SOURCE_VALUES, v[s]),
                dest: pick(&DEST_VALUES, v[d]),
                amount: v[a],
            }
        }),
        voice_mode: pick(&VOICE_MODE_VALUES, g(Ctl::VoiceMode)),
        unison_detune: g(Ctl::UnisonDetune),
        glide: if g(Ctl::Glide) < GLIDE_OFF {
            0.0
        } else {
            g(Ctl::Glide)
        },
        vintage: g(Ctl::Vintage),
        spread: g(Ctl::Spread),
    }
}

const _: () = assert!(MOD_SLOTS == 6, "the panel has six mod rows");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = DEFS.iter().map(|d| d.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), COUNT);
    }

    #[test]
    fn defaults_are_the_engines_init_patch() {
        let p = to_patch(&defaults());
        let init = Patch::default();
        assert_eq!(p.cutoff_hz, init.cutoff_hz);
        assert_eq!(p.filter_env, init.filter_env);
        assert_eq!(p.amp_env, init.amp_env);
        assert_eq!(p.aux_env, init.aux_env);
        assert_eq!(p.vintage, init.vintage);
        assert_eq!(p.glide, 0.0);
        assert_eq!(p.voice_mode, VoiceMode::Poly);
        // The first three mod slots come routed but at zero; the fourth
        // gives the mod wheel the filter.
        assert_eq!(p.mods[0].source, ModSource::Lfo1);
        assert_eq!(p.mods[0].dest, ModDest::Pitch);
        assert_eq!(p.mods[2].dest, ModDest::Cutoff);
        assert!(p.mods[..3].iter().all(|m| m.amount == 0.0));
        assert_eq!(p.mods[3].source, ModSource::ModWheel);
        assert_eq!(p.mods[3].dest, ModDest::Cutoff);
    }

    #[test]
    fn every_default_is_inside_its_range() {
        for d in DEFS {
            assert!(
                d.spec.default >= d.spec.min && d.spec.default <= d.spec.max,
                "{}",
                d.id
            );
        }
    }

    #[test]
    fn readouts() {
        assert_eq!(Ctl::Osc1Octave.def().show(-1.0), "16'");
        assert_eq!(Ctl::Glide.def().show(0.001), "Off");
        assert_eq!(Ctl::Cutoff.def().show(1_500.0), "1.50 kHz");
        assert_eq!(Ctl::Lfo1Shape.def().show(5.0), "S & H");
        assert_eq!(
            Ctl::long_label(Ctl::FilterAttack as usize),
            "Filter Env Attack"
        );
        assert_eq!(Ctl::long_label(Ctl::Mod2Amount as usize), "Mod 2 Amount");
        assert_eq!(Ctl::long_label(Ctl::Volume as usize), "Volume");
    }
}
