# Changelog

## Unreleased

## 0.1.0 – 2026-11-04

Commit `617aefb22859f75f8cddcae0e4f52541be3d7b9a`, release [v0.1.0](https://github.com/TimsterAudio/poly-box/releases/tag/v0.1.0).

The first release: a five-voice analog-style polysynth after the SEM sound, as an app with a hardware front panel, plus CLAP and VST3 plug-ins. For Linux, Windows and macOS.

### The panel
- A front panel in the Box family's Jupiter look (TAW-Panel 0.5.0), after Drone Box: wooden end cheeks; a top strip with the badge and logo, a red display (the last control touched and its value, the voice mode, the keys' octave), five voice lights with the note each plays, an output meter, Volume, the Black / Silver and Colour / Pointer switches and All Off.
- Sections: Osc 1, Osc 2, Mixer, Filter, Filter Env, Amp Env; Voice, LFO 1, LFO 2, Aux Env and the Mod Matrix. Faders for levels, envelope times, cutoff, resonance and LFO rates; knobs coloured by what they do (blue pitch, green tone, red modulation); a red readout under every control; double-click resets; Shift for fine.
- A 49-key keyboard (C2 to C6) to play with the mouse (lower on a key is louder) or the computer keys: A W S E D F T G Y H U J K O L P, Z / X octave, C / V velocity, Esc all off. A pitch wheel that springs back and a mod wheel that stays.
- Remembers the sound, the panel look and the keys' octave between runs.

### The sound
- Five voices with Poly (notes rotate round the voices), Mono (last-note priority) and Unison (all five, detuned up to 50 cents) modes; glide; Vintage (each voice's own tuning, cutoff and envelope quirks, and slow drift); stereo spread.
- Two VCOs per voice, each with one shape control from saw through square to a narrow pulse (band-limited), octaves from 32' to 2'; osc 2 with semitones, fine tune and hard sync; noise.
- The SEM-style 12 dB state-variable filter: the mode control sweeps low-pass, notch, high-pass, with a band-pass switch, resonance, drive, key tracking, and envelope amount and velocity.
- Filter, amp and aux envelopes with analog-style curves.
- Two LFOs (triangle, sine, saw up, saw down, square, sample & hold, 0.02 to 40 Hz) and a six-slot modulation matrix: LFOs, envelopes, velocity, key, random-per-note and the mod wheel, to pitch, each oscillator's pitch, shape and level, noise, cutoff, resonance, filter mode, drive, amp and pan.

### The plug-ins
- CLAP and VST3, with a plain window (every control as a slider, voice lights, and a keyboard for the mouse and computer keys). MIDI notes, pitch bend (±2 semitones) and the mod wheel (CC 1).
