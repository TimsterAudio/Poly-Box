# Poly Box

A five-voice analog-style polysynth from Timster's Audio Works, after the sound of the Oberheim SEM: two VCOs per voice with a continuous saw-to-pulse shape and hard sync, the SEM's 12 dB state-variable filter sweeping low-pass through notch to high-pass, and (soon) an arpeggiator and a 64-step polyphonic sequencer.

For Linux, Windows and macOS, as a standalone app and a CLAP or VST3 plug-in.

**Where it is:** step 2 of [ROADMAP.md](ROADMAP.md) is done: five voices (poly, mono, unison), Vintage, glide, two LFOs and a modulation matrix. The arpeggiator and sequencer are next.

## Try it

```sh
cargo run --release --example render -- demo.wav     # hear it without an audio device
cargo build --release && ./target/release/poly-box     # play it with the computer keys: A W S E D F T G Y H U J K O L P
cargo xtask bundle poly-box --release                # CLAP and VST3 plug-ins in target/bundled/
```

Poly Box isn't affiliated with Oberheim or Sequential; it's an original instrument in the spirit of the SEM.
