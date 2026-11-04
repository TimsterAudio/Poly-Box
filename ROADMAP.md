# Roadmap

Poly Box aims at the sound and the playing of a modern five-voice SEM-style poly (the Oberheim TEO-5 is the reference) without copying anyone's name, panel or presets. There's no reference unit, so it's voiced by ear against the SEM's well-known character.

## Pipeline (proposed)

| Step | What | State |
|---|---|---|
| 1 | One voice over MIDI: oscillators, the SEM filter, envelopes; standalone and CLAP | Done |
| 2 | Five voices: Poly/Mono/Unison voice assignment, Vintage, glide, stereo spread, aux envelope, two LFOs, a six-slot modulation matrix | Done |
| 3 | The arpeggiator and the 64-step sequencer (below) | Next |
| 4 | The Box panel, in TAW-Panel's Jupiter look, as an app with its own sound | Done (2026-11-04, before step 3, at the user's request) |
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

## Back burner

- **MIDI beyond notes** (the user's call, 2026-11-03: not needed yet). Notes and pitch bend already work; still to come: mod wheel and aftertouch as modulation sources, the sustain pedal, velocity curves, MIDI learn for the knobs.
- **Licensing.** Poly Box is for the user's own use for now, so VST3 is built by default. nih-plug's VST3 bindings are GPLv3: if it's ever shared, either publish the source under the GPL or drop to CLAP + AU + standalone (or clap-wrapper's VST3).

## Things to settle on the way

- **The panel's egui.** nih-plug's egui adapter (`nih_plug_egui`) is on egui 0.31; TAW-Panel is on 0.36. Step 4 needs one of: our own small baseview + egui 0.36 glue (preferred, keeps the panel crate as it is), or a TAW-Panel build on an older egui.
- **Oversampling.** The filter's saturators run at the host rate. If drive at high resonance sounds gritty in the top octave, run the voices at 2x.
- **Tempo-synced LFOs** come with the arpeggiator's clock in step 3.
