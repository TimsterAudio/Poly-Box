//! The standalone app. Run with `--help` for the audio and MIDI options,
//! e.g. `poly-box --midi-input <device>`.

fn main() {
    nih_plug::nih_export_standalone::<poly_box::PolyBox>();
}
