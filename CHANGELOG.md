# Changelog

## Unreleased

### Added
- Play it without a MIDI keyboard. The window has a six-octave keyboard (C1 to C7) to play with the mouse (lower on a key is louder; drag across keys), and the computer keys play it too: A W S E D F T G Y H U J K O L P from the shown octave, Z / X octave down / up, C / V velocity down / up, Esc lets go of everything. Keys held from MIDI light up as well.
- Five voice lights across the top: lit while a voice's key is down, dim while it rings out, with the note it's playing, so you can watch notes rotate round the voices, steal, and stack in unison.
- `examples/preview.rs`: the window without a host or audio device (the synth runs silently behind it), which also works under Xvfb.
- Step 2 of the roadmap: five voices.
  - Voice modes: Poly (notes rotate round the voices; a free voice first, then the one released longest ago, then the one held longest), Mono (last-note priority) and Unison (all five on one note, detuned symmetrically up to 50 cents).
  - Glide, from each voice's last pitch (RC-style).
  - Vintage: each voice has its own fixed tuning, cutoff and envelope-time quirks, and its oscillators drift slowly; 0 makes every voice identical.
  - Stereo spread: voices placed alternately left and right.
  - An aux envelope, two LFOs (triangle, sine, saw up, saw down, square, sample & hold, 0.02 to 40 Hz) and a six-slot modulation matrix: sources LFO 1 and 2, the three envelopes, velocity, key and random-per-note; destinations pitch, each oscillator's pitch, shape and level, noise, cutoff, resonance, filter mode, drive, amp and pan. The first three slots come set up at zero for vibrato, PWM and filter wobble.
  - VST3 alongside CLAP, now built by default.
- Step 1 of the roadmap: one voice played over MIDI.
  - Two VCOs, each with one shape control running saw → square → narrow pulse (band-limited with polyBLEP). Osc 1 and osc 2 each have an octave (32' to 2'); osc 2 also has semitones, fine tune and hard sync to osc 1. Noise.
  - The SEM-style filter: 12 dB/oct state-variable, the mode control sweeps low-pass → notch → high-pass, with a band-pass switch, plus resonance, drive, key tracking, and filter envelope amount and velocity.
  - Filter and amp ADSR envelopes with analog-style curves; amp velocity.
  - Last-note priority (a held note comes back), pitch bend ±2 semitones.
  - CLAP plug-in and a standalone app (JACK or ALSA on Linux, CoreAudio on a Mac, WASAPI on Windows, with MIDI input); VST3 behind the `vst3` feature.
  - A plain list of the controls as the window, until the Box panel (step 4).
  - `examples/render.rs`: renders a demo to a WAV without an audio device.
