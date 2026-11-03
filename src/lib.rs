//! Poly Box: a five-voice analog-style polysynth after the Oberheim SEM sound.
//!
//! Step 1 of the plan: one voice, two VCOs and noise into the SEM filter, two
//! envelopes, played over MIDI. Notes are handled last-note-priority; a held
//! note comes back when the newer one is let go.

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::Arc;

pub mod dsp;

use dsp::{Patch, Voice};

/// Pitch bend range, in semitones each way.
const BEND_RANGE: f32 = 2.0;
/// Notes remembered for last-note priority.
const NOTE_STACK: usize = 16;

pub struct PolyBox {
    params: Arc<PolyBoxParams>,
    voice: Voice,
    held: NoteStack,
    bend: f32,
    /// One-pole DC blocker state (last input, last output), standing in for a
    /// hardware output's coupling capacitor.
    dc: (f32, f32),
    dc_pole: f32,
}

/// Keys held down, oldest first, without allocating on the audio thread.
#[derive(Default)]
struct NoteStack {
    notes: [u8; NOTE_STACK],
    len: usize,
}

impl NoteStack {
    fn push(&mut self, note: u8) {
        self.remove(note);
        if self.len == NOTE_STACK {
            self.notes.copy_within(1.., 0);
            self.len -= 1;
        }
        self.notes[self.len] = note;
        self.len += 1;
    }

    fn remove(&mut self, note: u8) {
        if let Some(i) = self.notes[..self.len].iter().position(|&n| n == note) {
            self.notes.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
    }

    fn top(&self) -> Option<u8> {
        self.len.checked_sub(1).map(|i| self.notes[i])
    }

    fn clear(&mut self) {
        self.len = 0;
    }
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
    .with_value_to_string(Arc::new(|v| {
        if v < 1.0 {
            format!("{:.0} ms", v * 1000.0)
        } else {
            format!("{v:.2} s")
        }
    }))
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

/// Parameter ids are saved in hosts' projects: never change one.
#[derive(Params)]
pub struct PolyBoxParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

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

    #[id = "vol"]
    volume: FloatParam,
}

impl Default for PolyBoxParams {
    fn default() -> Self {
        let init = Patch::default();
        let [fa, fd, fs, fr] = init.filter_env;
        let [aa, ad, asus, ar] = init.amp_env;
        Self {
            editor_state: EguiState::from_size(460, 720),

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
            voice: Voice::new(48_000.0),
            held: NoteStack::default(),
            bend: 0.0,
            dc: (0.0, 0.0),
            dc_pole: 0.999,
        }
    }
}

impl PolyBox {
    fn note_on(&mut self, note: u8, velocity: f32) {
        self.held.push(note);
        self.voice.note_on(note, velocity, true);
    }

    fn note_off(&mut self, note: u8) {
        let was_top = self.held.top() == Some(note);
        self.held.remove(note);
        if was_top {
            match self.held.top() {
                // Fall back to the note still held, without restarting the envelopes.
                Some(prev) => self.voice.note_on(prev, 0.0, false),
                None => self.voice.note_off(),
            }
        }
    }

    /// The envelope times, which are read once a block.
    fn envelopes(&self) -> Patch {
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
            ..Patch::default()
        }
    }

    /// Everything else, advancing the smoothers by one sample.
    fn patch(&self) -> Patch {
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
            ..self.envelopes()
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
        // A plain list of every control until the Box panel arrives (step 4).
        let params = self.params.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |ctx, setter, _| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Poly Box");
                    widgets::generic_ui::create(
                        ui,
                        params.clone(),
                        setter,
                        widgets::generic_ui::GenericSlider,
                    );
                });
            },
        )
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.voice = Voice::new(config.sample_rate);
        // About 5 Hz.
        self.dc_pole = (-2.0 * std::f32::consts::PI * 5.0 / config.sample_rate).exp();
        true
    }

    fn reset(&mut self) {
        self.voice.reset();
        self.held.clear();
        self.bend = 0.0;
        self.dc = (0.0, 0.0);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.voice.set_envelopes(&self.envelopes());

        let mut next_event = context.next_event();
        for (i, mut frame) in buffer.iter_samples().enumerate() {
            while let Some(event) = next_event {
                if event.timing() as usize > i {
                    break;
                }
                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => self.note_on(note, velocity),
                    NoteEvent::NoteOff { note, .. } => self.note_off(note),
                    NoteEvent::MidiPitchBend { value, .. } => {
                        self.bend = (value - 0.5) * 2.0 * BEND_RANGE;
                    }
                    _ => {}
                }
                next_event = context.next_event();
            }

            let patch = self.patch();
            let volume = self.params.volume.smoothed.next();
            let y = if self.voice.is_active() {
                self.voice.next(&patch, self.bend)
            } else {
                0.0
            };
            let (x1, y1) = self.dc;
            let out = y - x1 + self.dc_pole * y1;
            self.dc = (y, out);

            let out = out * volume;
            for s in frame.iter_mut() {
                *s = out;
            }
        }

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

#[cfg(feature = "vst3")]
impl Vst3Plugin for PolyBox {
    const VST3_CLASS_ID: [u8; 16] = *b"TAWPolyBoxSynth!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
        Vst3SubCategory::Stereo,
    ];
}

#[cfg(feature = "vst3")]
nih_export_vst3!(PolyBox);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_stack_returns_to_held_note() {
        let mut s = NoteStack::default();
        s.push(60);
        s.push(64);
        s.push(67);
        s.remove(67);
        assert_eq!(s.top(), Some(64));
        s.remove(60);
        assert_eq!(s.top(), Some(64));
        s.remove(64);
        assert_eq!(s.top(), None);
    }

    #[test]
    fn note_stack_drops_oldest_when_full() {
        let mut s = NoteStack::default();
        for n in 0..(NOTE_STACK as u8 + 4) {
            s.push(n);
        }
        assert_eq!(s.len, NOTE_STACK);
        assert_eq!(s.top(), Some(NOTE_STACK as u8 + 3));
        assert_eq!(s.notes[0], 4);
    }

    #[test]
    fn labels_read_well() {
        assert_eq!(footage(-1), "16'");
        assert_eq!(shape_name(0.0), "Saw");
        assert_eq!(shape_name(0.5), "Square");
        assert_eq!(shape_name(1.0), "Pulse 5%");
        assert_eq!(mode_name(0.5), "Notch");
    }
}
