//! Poly Box: a five-voice analog-style polysynth after the Oberheim SEM sound.
//!
//! The sound engine lives in [`dsp`]; this file is the plug-in around it: the
//! parameters, note events and the (for now plain) window.

// nih-plug's `nih_export_vst3!` expands to a macro with a trailing semicolon
// that newer compilers warn about (rust-lang/rust#79813); it's in their code,
// not ours, so CI's -D warnings mustn't fail on it. Older compilers don't
// know the lint's name, hence `unknown_lints` too.
#![allow(unknown_lints)]
#![allow(semicolon_in_expressions_from_non_local_macros)]

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

/// The engine, under the name the plug-in has always used for it.
pub use poly_box_engine as dsp;

pub mod editor;

use dsp::keys::{KeyBridge, KeyPresses, Live};
use dsp::{LfoSettings, LfoShape, ModDest, ModSlot, ModSource, Patch, Synth, VoiceMode, MOD_SLOTS};

/// Pitch bend range, in semitones each way.
const BEND_RANGE: f32 = 2.0;

pub struct PolyBox {
    params: Arc<PolyBoxParams>,
    synth: Synth,
    /// Shared with the window: the keys it plays and the lights it shows.
    live: Arc<Live>,
    keys: KeyBridge,
}

fn footage(octave: i32) -> String {
    match octave {
        -2 => "32'".into(),
        -1 => "16'".into(),
        0 => "8'".into(),
        1 => "4'".into(),
        2 => "2'".into(),
        n => n.to_string(),
    }
}

fn shape_name(v: f32) -> String {
    if v < 0.01 {
        "Saw".into()
    } else if (v - 0.5).abs() < 0.01 {
        "Square".into()
    } else if v < 0.5 {
        format!("Saw/Sq {:.0}%", v * 200.0)
    } else {
        format!("Pulse {:.0}%", (0.5 - (v - 0.5) * 0.9) * 100.0)
    }
}

fn mode_name(v: f32) -> String {
    if v < 0.01 {
        "Low-pass".into()
    } else if (v - 0.5).abs() < 0.01 {
        "Notch".into()
    } else if v > 0.99 {
        "High-pass".into()
    } else if v < 0.5 {
        format!("LP→Notch {:.0}%", v * 200.0)
    } else {
        format!("Notch→HP {:.0}%", (v - 0.5) * 200.0)
    }
}

fn time_string(v: f32) -> String {
    if v < 1.0 {
        format!("{:.0} ms", v * 1000.0)
    } else {
        format!("{v:.2} s")
    }
}

fn seconds(name: &str, max: f32, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: 0.001,
            max,
            factor: FloatRange::skew_factor(-2.5),
        },
    )
    .with_value_to_string(Arc::new(time_string))
}

fn percent(name: &str, default: f32, min: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(10.0))
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

fn amount(name: &str, default: f32) -> FloatParam {
    percent(name, default, 0.0)
}

fn octave_param(name: &str) -> IntParam {
    IntParam::new(name, 0, IntRange::Linear { min: -2, max: 2 })
        .with_value_to_string(Arc::new(footage))
}

fn shape_param(name: &str) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(10.0))
        .with_value_to_string(Arc::new(shape_name))
}

fn lfo_rate(name: &str, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: 0.02,
            max: 40.0,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_smoother(SmoothingStyle::Logarithmic(20.0))
    .with_value_to_string(Arc::new(|v| {
        if v < 1.0 {
            format!("{v:.2} Hz")
        } else {
            format!("{v:.1} Hz")
        }
    }))
}

// The parameter enums mirror the engine's own, which stay free of nih-plug.
// Their ids are saved in hosts' projects like parameter ids: never change one.

#[derive(Enum, Debug, PartialEq, Clone, Copy)]
enum VoiceModeParam {
    #[id = "poly"]
    Poly,
    #[id = "mono"]
    Mono,
    #[id = "unison"]
    Unison,
}

impl From<VoiceModeParam> for VoiceMode {
    fn from(v: VoiceModeParam) -> Self {
        match v {
            VoiceModeParam::Poly => VoiceMode::Poly,
            VoiceModeParam::Mono => VoiceMode::Mono,
            VoiceModeParam::Unison => VoiceMode::Unison,
        }
    }
}

#[derive(Enum, Debug, PartialEq, Clone, Copy)]
enum LfoShapeParam {
    #[id = "tri"]
    Triangle,
    #[id = "sine"]
    Sine,
    #[id = "sawup"]
    #[name = "Saw Up"]
    SawUp,
    #[id = "sawdn"]
    #[name = "Saw Down"]
    SawDown,
    #[id = "square"]
    Square,
    #[id = "sh"]
    #[name = "Sample & Hold"]
    SampleHold,
}

impl From<LfoShapeParam> for LfoShape {
    fn from(v: LfoShapeParam) -> Self {
        match v {
            LfoShapeParam::Triangle => LfoShape::Triangle,
            LfoShapeParam::Sine => LfoShape::Sine,
            LfoShapeParam::SawUp => LfoShape::SawUp,
            LfoShapeParam::SawDown => LfoShape::SawDown,
            LfoShapeParam::Square => LfoShape::Square,
            LfoShapeParam::SampleHold => LfoShape::SampleHold,
        }
    }
}

#[derive(Enum, Debug, PartialEq, Clone, Copy)]
enum SourceParam {
    #[id = "off"]
    Off,
    #[id = "lfo1"]
    #[name = "LFO 1"]
    Lfo1,
    #[id = "lfo2"]
    #[name = "LFO 2"]
    Lfo2,
    #[id = "fenv"]
    #[name = "Filter Env"]
    FilterEnv,
    #[id = "aenv"]
    #[name = "Amp Env"]
    AmpEnv,
    #[id = "xenv"]
    #[name = "Aux Env"]
    AuxEnv,
    #[id = "vel"]
    Velocity,
    #[id = "key"]
    Key,
    #[id = "rand"]
    #[name = "Random per Note"]
    Random,
    #[id = "mwheel"]
    #[name = "Mod Wheel"]
    ModWheel,
}

impl From<SourceParam> for ModSource {
    fn from(v: SourceParam) -> Self {
        match v {
            SourceParam::Off => ModSource::Off,
            SourceParam::Lfo1 => ModSource::Lfo1,
            SourceParam::Lfo2 => ModSource::Lfo2,
            SourceParam::FilterEnv => ModSource::FilterEnv,
            SourceParam::AmpEnv => ModSource::AmpEnv,
            SourceParam::AuxEnv => ModSource::AuxEnv,
            SourceParam::Velocity => ModSource::Velocity,
            SourceParam::Key => ModSource::Key,
            SourceParam::Random => ModSource::Random,
            SourceParam::ModWheel => ModSource::ModWheel,
        }
    }
}

#[derive(Enum, Debug, PartialEq, Clone, Copy)]
enum DestParam {
    #[id = "off"]
    Off,
    #[id = "pitch"]
    Pitch,
    #[id = "o1pitch"]
    #[name = "Osc 1 Pitch"]
    Osc1Pitch,
    #[id = "o2pitch"]
    #[name = "Osc 2 Pitch"]
    Osc2Pitch,
    #[id = "o1shape"]
    #[name = "Osc 1 Shape"]
    Osc1Shape,
    #[id = "o2shape"]
    #[name = "Osc 2 Shape"]
    Osc2Shape,
    #[id = "o1lvl"]
    #[name = "Osc 1 Level"]
    Osc1Level,
    #[id = "o2lvl"]
    #[name = "Osc 2 Level"]
    Osc2Level,
    #[id = "noise"]
    #[name = "Noise Level"]
    NoiseLevel,
    #[id = "cutoff"]
    Cutoff,
    #[id = "res"]
    Resonance,
    #[id = "fmode"]
    #[name = "Filter Mode"]
    FilterMode,
    #[id = "drive"]
    Drive,
    #[id = "amp"]
    Amp,
    #[id = "pan"]
    Pan,
}

impl From<DestParam> for ModDest {
    fn from(v: DestParam) -> Self {
        match v {
            DestParam::Off => ModDest::Off,
            DestParam::Pitch => ModDest::Pitch,
            DestParam::Osc1Pitch => ModDest::Osc1Pitch,
            DestParam::Osc2Pitch => ModDest::Osc2Pitch,
            DestParam::Osc1Shape => ModDest::Osc1Shape,
            DestParam::Osc2Shape => ModDest::Osc2Shape,
            DestParam::Osc1Level => ModDest::Osc1Level,
            DestParam::Osc2Level => ModDest::Osc2Level,
            DestParam::NoiseLevel => ModDest::NoiseLevel,
            DestParam::Cutoff => ModDest::Cutoff,
            DestParam::Resonance => ModDest::Resonance,
            DestParam::FilterMode => ModDest::FilterMode,
            DestParam::Drive => ModDest::Drive,
            DestParam::Amp => ModDest::Amp,
            DestParam::Pan => ModDest::Pan,
        }
    }
}

/// One slot of the modulation matrix. In an array its ids get `_1`, `_2`…
#[derive(Params)]
pub struct ModSlotParams {
    #[id = "src"]
    source: EnumParam<SourceParam>,
    #[id = "dst"]
    dest: EnumParam<DestParam>,
    #[id = "amt"]
    amount: FloatParam,
}

impl ModSlotParams {
    fn new(n: usize, source: SourceParam, dest: DestParam) -> Self {
        Self {
            source: EnumParam::new(format!("Mod {n} Source"), source),
            dest: EnumParam::new(format!("Mod {n} Destination"), dest),
            amount: FloatParam::new(
                format!("Mod {n} Amount"),
                0.0,
                FloatRange::SymmetricalSkewed {
                    min: -1.0,
                    max: 1.0,
                    factor: FloatRange::skew_factor(-1.5),
                    center: 0.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }

    fn slot(&self) -> ModSlot {
        ModSlot {
            source: self.source.value().into(),
            dest: self.dest.value().into(),
            amount: self.amount.smoothed.next(),
        }
    }
}

/// Parameter ids are saved in hosts' projects: never change one.
#[derive(Params)]
pub struct PolyBoxParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "vmode"]
    voice_mode: EnumParam<VoiceModeParam>,
    #[id = "unidet"]
    unison_detune: FloatParam,
    #[id = "glide"]
    glide: FloatParam,
    #[id = "vintage"]
    vintage: FloatParam,
    #[id = "spread"]
    spread: FloatParam,

    #[id = "o1oct"]
    osc1_octave: IntParam,
    #[id = "o1shp"]
    osc1_shape: FloatParam,
    #[id = "o2oct"]
    osc2_octave: IntParam,
    #[id = "o2semi"]
    osc2_semitones: IntParam,
    #[id = "o2fine"]
    osc2_fine: FloatParam,
    #[id = "o2shp"]
    osc2_shape: FloatParam,
    #[id = "sync"]
    sync: BoolParam,

    #[id = "o1lvl"]
    osc1_level: FloatParam,
    #[id = "o2lvl"]
    osc2_level: FloatParam,
    #[id = "noise"]
    noise_level: FloatParam,

    #[id = "cutoff"]
    cutoff: FloatParam,
    #[id = "res"]
    resonance: FloatParam,
    #[id = "mode"]
    filter_mode: FloatParam,
    #[id = "bp"]
    bandpass: BoolParam,
    #[id = "drive"]
    drive: FloatParam,
    #[id = "ktrk"]
    key_track: FloatParam,
    #[id = "fenvamt"]
    filter_env_amount: FloatParam,
    #[id = "fvel"]
    filter_velocity: FloatParam,

    #[id = "fatk"]
    filter_attack: FloatParam,
    #[id = "fdec"]
    filter_decay: FloatParam,
    #[id = "fsus"]
    filter_sustain: FloatParam,
    #[id = "frel"]
    filter_release: FloatParam,

    #[id = "aatk"]
    amp_attack: FloatParam,
    #[id = "adec"]
    amp_decay: FloatParam,
    #[id = "asus"]
    amp_sustain: FloatParam,
    #[id = "arel"]
    amp_release: FloatParam,
    #[id = "avel"]
    amp_velocity: FloatParam,

    #[id = "xatk"]
    aux_attack: FloatParam,
    #[id = "xdec"]
    aux_decay: FloatParam,
    #[id = "xsus"]
    aux_sustain: FloatParam,
    #[id = "xrel"]
    aux_release: FloatParam,

    #[id = "l1rate"]
    lfo1_rate: FloatParam,
    #[id = "l1shp"]
    lfo1_shape: EnumParam<LfoShapeParam>,
    #[id = "l2rate"]
    lfo2_rate: FloatParam,
    #[id = "l2shp"]
    lfo2_shape: EnumParam<LfoShapeParam>,

    #[nested(array, group = "Mod")]
    mods: [ModSlotParams; MOD_SLOTS],

    #[id = "vol"]
    volume: FloatParam,
}

impl Default for PolyBoxParams {
    fn default() -> Self {
        let init = Patch::default();
        let [fa, fd, fs, fr] = init.filter_env;
        let [aa, ad, asus, ar] = init.amp_env;
        let [xa, xd, xs, xr] = init.aux_env;
        // The first slots come set up for the usual jobs, at zero, so they're
        // a turn of the amount away: vibrato, pulse-width modulation, filter wobble.
        let suggested = [
            (SourceParam::Lfo1, DestParam::Pitch),
            (SourceParam::Lfo2, DestParam::Osc1Shape),
            (SourceParam::Lfo1, DestParam::Cutoff),
            (SourceParam::Off, DestParam::Off),
            (SourceParam::Off, DestParam::Off),
            (SourceParam::Off, DestParam::Off),
        ];
        Self {
            editor_state: EguiState::from_size(820, 760),

            voice_mode: EnumParam::new("Voice Mode", VoiceModeParam::Poly),
            unison_detune: FloatParam::new(
                "Unison Detune",
                init.unison_detune,
                FloatRange::Linear {
                    min: 0.0,
                    max: 50.0,
                },
            )
            .with_unit(" ct")
            .with_step_size(0.1),
            glide: FloatParam::new(
                "Glide",
                init.glide,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 5.0,
                    factor: FloatRange::skew_factor(-2.5),
                },
            )
            .with_value_to_string(Arc::new(|v| {
                if v < 0.001 {
                    "Off".into()
                } else {
                    time_string(v)
                }
            })),
            vintage: amount("Vintage", init.vintage),
            spread: amount("Stereo Spread", init.spread),

            osc1_octave: octave_param("Osc 1 Octave"),
            osc1_shape: shape_param("Osc 1 Shape"),
            osc2_octave: octave_param("Osc 2 Octave"),
            osc2_semitones: IntParam::new(
                "Osc 2 Semitones",
                0,
                IntRange::Linear { min: -12, max: 12 },
            ),
            osc2_fine: FloatParam::new(
                "Osc 2 Fine",
                init.osc2_cents,
                FloatRange::Linear {
                    min: -50.0,
                    max: 50.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" ct")
            .with_step_size(0.1),
            osc2_shape: shape_param("Osc 2 Shape"),
            sync: BoolParam::new("Osc 2 Sync", false),

            osc1_level: amount("Osc 1 Level", init.osc1_level),
            osc2_level: amount("Osc 2 Level", init.osc2_level),
            noise_level: amount("Noise Level", init.noise_level),

            cutoff: FloatParam::new(
                "Cutoff",
                init.cutoff_hz,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            resonance: amount("Resonance", init.resonance),
            filter_mode: FloatParam::new(
                "Filter Mode",
                init.filter_mode,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(Arc::new(mode_name)),
            bandpass: BoolParam::new("Band-pass", false),
            drive: amount("Drive", init.drive),
            key_track: amount("Key Track", init.key_track),
            filter_env_amount: percent("Filter Env Amount", init.filter_env_amount, -1.0),
            filter_velocity: amount("Filter Velocity", init.filter_velocity),

            filter_attack: seconds("Filter Attack", 10.0, fa),
            filter_decay: seconds("Filter Decay", 20.0, fd),
            filter_sustain: amount("Filter Sustain", fs),
            filter_release: seconds("Filter Release", 20.0, fr),

            amp_attack: seconds("Amp Attack", 10.0, aa),
            amp_decay: seconds("Amp Decay", 20.0, ad),
            amp_sustain: amount("Amp Sustain", asus),
            amp_release: seconds("Amp Release", 20.0, ar),
            amp_velocity: amount("Amp Velocity", init.amp_velocity),

            aux_attack: seconds("Aux Attack", 10.0, xa),
            aux_decay: seconds("Aux Decay", 20.0, xd),
            aux_sustain: amount("Aux Sustain", xs),
            aux_release: seconds("Aux Release", 20.0, xr),

            lfo1_rate: lfo_rate("LFO 1 Rate", init.lfo1.rate_hz),
            lfo1_shape: EnumParam::new("LFO 1 Shape", LfoShapeParam::Triangle),
            lfo2_rate: lfo_rate("LFO 2 Rate", init.lfo2.rate_hz),
            lfo2_shape: EnumParam::new("LFO 2 Shape", LfoShapeParam::Triangle),

            mods: std::array::from_fn(|i| {
                let (s, d) = suggested[i];
                ModSlotParams::new(i + 1, s, d)
            }),

            volume: FloatParam::new(
                "Volume",
                util::db_to_gain(-6.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-48.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-48.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}

impl Default for PolyBox {
    fn default() -> Self {
        Self {
            params: Arc::new(PolyBoxParams::default()),
            synth: Synth::new(48_000.0),
            live: Arc::new(Live::default()),
            keys: KeyBridge::default(),
        }
    }
}

impl PolyBox {
    /// The settings read once a block: envelope times, voice mode, glide,
    /// Vintage, spread.
    fn block_patch(&self) -> Patch {
        let p = &self.params;
        Patch {
            filter_env: [
                p.filter_attack.value(),
                p.filter_decay.value(),
                p.filter_sustain.value(),
                p.filter_release.value(),
            ],
            amp_env: [
                p.amp_attack.value(),
                p.amp_decay.value(),
                p.amp_sustain.value(),
                p.amp_release.value(),
            ],
            aux_env: [
                p.aux_attack.value(),
                p.aux_decay.value(),
                p.aux_sustain.value(),
                p.aux_release.value(),
            ],
            voice_mode: p.voice_mode.value().into(),
            unison_detune: p.unison_detune.value(),
            glide: p.glide.value(),
            vintage: p.vintage.value(),
            spread: p.spread.value(),
            ..Patch::default()
        }
    }

    /// Everything, advancing the smoothers by one sample; `block` is this
    /// block's [`Self::block_patch`].
    fn patch(&self, block: &Patch) -> Patch {
        let p = &self.params;
        Patch {
            osc1_octave: p.osc1_octave.value(),
            osc1_shape: p.osc1_shape.smoothed.next(),
            osc2_octave: p.osc2_octave.value(),
            osc2_semitones: p.osc2_semitones.value(),
            osc2_cents: p.osc2_fine.smoothed.next(),
            osc2_shape: p.osc2_shape.smoothed.next(),
            sync: p.sync.value(),
            osc1_level: p.osc1_level.smoothed.next(),
            osc2_level: p.osc2_level.smoothed.next(),
            noise_level: p.noise_level.smoothed.next(),
            cutoff_hz: p.cutoff.smoothed.next(),
            resonance: p.resonance.smoothed.next(),
            filter_mode: p.filter_mode.smoothed.next(),
            bandpass: p.bandpass.value(),
            drive: p.drive.smoothed.next(),
            key_track: p.key_track.smoothed.next(),
            filter_env_amount: p.filter_env_amount.smoothed.next(),
            filter_velocity: p.filter_velocity.smoothed.next(),
            amp_velocity: p.amp_velocity.smoothed.next(),
            vintage: p.vintage.smoothed.next(),
            lfo1: LfoSettings {
                rate_hz: p.lfo1_rate.smoothed.next(),
                shape: p.lfo1_shape.value().into(),
            },
            lfo2: LfoSettings {
                rate_hz: p.lfo2_rate.smoothed.next(),
                shape: p.lfo2_shape.value().into(),
            },
            mods: std::array::from_fn(|i| p.mods[i].slot()),
            ..*block
        }
    }
}

impl Plugin for PolyBox {
    const NAME: &'static str = "Poly Box";
    const VENDOR: &'static str = "Timster's Audio Works";
    const URL: &'static str = "https://github.com/TimsterAudio/poly-box";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    // CCs as well as notes, for pitch bend.
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let live = self.live.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            KeyPresses::default(),
            |_, _| {},
            move |ctx, setter, keyboard| {
                editor::draw(ctx, params.clone(), setter, &live, keyboard);
            },
        )
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.synth = Synth::new(config.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.synth.reset();
        self.keys.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let block = self.block_patch();
        self.synth.set_block(&block);
        self.keys.take_window(&self.live, &mut self.synth);

        let mut next_event = context.next_event();
        for (i, mut frame) in buffer.iter_samples().enumerate() {
            while let Some(event) = next_event {
                if event.timing() as usize > i {
                    break;
                }
                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => {
                        self.keys.note_on(&mut self.synth, note, velocity)
                    }
                    NoteEvent::NoteOff { note, .. } => self.keys.note_off(&mut self.synth, note),
                    NoteEvent::MidiPitchBend { value, .. } => {
                        self.synth.set_bend((value - 0.5) * 2.0 * BEND_RANGE);
                    }
                    NoteEvent::MidiCC { cc: 1, value, .. } => self.synth.set_mod_wheel(value),
                    _ => {}
                }
                next_event = context.next_event();
            }

            let patch = self.patch(&block);
            let volume = self.params.volume.smoothed.next();
            let (l, r) = self.synth.next(&patch);
            let mut channels = frame.iter_mut();
            if let Some(s) = channels.next() {
                *s = l * volume;
            }
            if let Some(s) = channels.next() {
                *s = r * volume;
            }
        }
        self.keys.publish(&self.live, &self.synth);

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for PolyBox {
    const CLAP_ID: &'static str = "com.timsters-audio-works.poly-box";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Five-voice analog-style polysynth after the SEM sound");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

nih_export_clap!(PolyBox);

impl Vst3Plugin for PolyBox {
    const VST3_CLASS_ID: [u8; 16] = *b"TAWPolyBoxSynth!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_vst3!(PolyBox);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_read_well() {
        assert_eq!(footage(-1), "16'");
        assert_eq!(shape_name(0.0), "Saw");
        assert_eq!(shape_name(0.5), "Square");
        assert_eq!(shape_name(1.0), "Pulse 5%");
        assert_eq!(mode_name(0.5), "Notch");
    }

    #[test]
    fn parameter_ids_are_unique_and_the_mod_slots_are_numbered() {
        let params = PolyBoxParams::default();
        let ids: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
        for n in 1..=MOD_SLOTS {
            assert!(ids.contains(&format!("src_{n}")));
            assert!(ids.contains(&format!("amt_{n}")));
        }
    }
}
