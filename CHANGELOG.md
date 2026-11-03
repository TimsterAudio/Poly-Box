# Changelog

## Unreleased

### Added
- Step 1 of the roadmap: one voice played over MIDI.
  - Two VCOs, each with one shape control running saw → square → narrow pulse (band-limited with polyBLEP). Osc 1 and osc 2 each have an octave (32' to 2'); osc 2 also has semitones, fine tune and hard sync to osc 1. Noise.
  - The SEM-style filter: 12 dB/oct state-variable, the mode control sweeps low-pass → notch → high-pass, with a band-pass switch, plus resonance, drive, key tracking, and filter envelope amount and velocity.
  - Filter and amp ADSR envelopes with analog-style curves; amp velocity.
  - Last-note priority (a held note comes back), pitch bend ±2 semitones.
  - CLAP plug-in and a standalone app (JACK or ALSA on Linux, CoreAudio on a Mac, WASAPI on Windows, with MIDI input); VST3 behind the `vst3` feature.
  - A plain list of the controls as the window, until the Box panel (step 4).
  - `examples/render.rs`: renders a demo to a WAV without an audio device.
