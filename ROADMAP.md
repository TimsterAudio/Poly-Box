# Roadmap

Poly Box aims at the sound and the playing of a modern five-voice SEM-style poly (the Oberheim TEO-5 is the reference) without copying anyone's name, panel or presets. There's no reference unit, so it's voiced by ear against the SEM's well-known character.

## Pipeline (proposed)

| Step | What | State |
|---|---|---|
| 1 | One voice over MIDI: oscillators, the SEM filter, envelopes; standalone and CLAP | Done, not yet released |
| 2 | Five voices: voice assignment (reuse the oldest released voice, unison), Vintage (per-voice drift and slop), glide, LFOs and the modulation routings, aftertouch and mod wheel | Next |
| 3 | The arpeggiator and the 64-step sequencer (below) | |
| 4 | The Box panel, in TAW-Panel's Jupiter look, with an on-screen keyboard and the computer keyboard as keys | |
| 5 | Effects (chorus, delay, reverb in the spirit of the hardware's), presets, AU through clap-wrapper for Logic, release builds for Linux, Windows and macOS | |

## Arpeggiator (step 3)

- Modes: up, down, up/down, random, as played.
- Range 1 to 4 octaves.
- Clock: the host's tempo in a plug-in; its own tempo with tap in the standalone. Divisions 1/32 to 1/2, with triplets and dotted.
- Gate length and swing.
- Latch: tap a chord and it keeps going.
- Repeats: each note 1 to 4 times.

## Sequencer (step 3)

- 64 steps, polyphonic: up to five notes a step.
- Step entry (hold notes, press next), rests and ties, velocity per step.
- Live recording over the running loop.
- Length 1 to 64, with its own rate.
- Transpose from the keys.
- Saved with the patch.

Both sit in front of the voices as note processors, so they play the synth exactly as hands would.

## Things to settle on the way

- **The panel's egui.** nih-plug's egui adapter (`nih_plug_egui`) is on egui 0.31; TAW-Panel is on 0.36. Step 4 needs one of: our own small baseview + egui 0.36 glue (preferred, keeps the panel crate as it is), or a TAW-Panel build on an older egui.
- **VST3 is GPL.** nih-plug's VST3 bindings are GPLv3, so a VST3 build is GPLv3; that's why it's behind the `vst3` feature. CLAP and the standalone aren't affected. Decide before the first release whether to ship VST3 (and publish the source), or ship CLAP + AU + standalone only, or wrap the CLAP with clap-wrapper's VST3 (MIT, built against Steinberg's now-MIT SDK).
- **Oversampling.** The filter's saturators run at the host rate. If drive at high resonance sounds gritty in the top octave, run the voice at 2x.
