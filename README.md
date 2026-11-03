# Poly Box

A five-voice analog-style polysynth from Timster's Audio Works, after the sound of the Oberheim SEM: two VCOs per voice with a continuous saw-to-pulse shape and hard sync, the SEM's 12 dB state-variable filter sweeping low-pass through notch to high-pass, and (soon) an arpeggiator and a 64-step polyphonic sequencer.

For Linux, Windows and macOS, as a standalone app and a CLAP plug-in.

**Where it is:** step 1 of [ROADMAP.md](ROADMAP.md): one voice over MIDI. Five voices are next.

## Try it

```sh
cargo run --release --example render -- demo.wav     # hear it without an audio device
cargo build --release && ./target/release/poly-box --help
cargo xtask bundle poly-box --release                # CLAP plug-in in target/bundled/
```

Poly Box isn't affiliated with Oberheim or Sequential; it's an original instrument in the spirit of the SEM.
