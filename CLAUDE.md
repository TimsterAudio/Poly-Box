# Poly Box: working conventions

A five-voice analog-style polysynth in the Timster's Audio Works **Box** family (with Drone Box, Drum Box and EQ Box). TAW-Panel's `FAMILY.md` is the family overview; `ROADMAP.md` here holds the pipeline and the arpeggiator and sequencer specs.

## Hard rules

- **Inspired by, never a copy.** The SEM sound and the TEO-5's playing are the reference. No Oberheim or Sequential names, logos, panel layouts or factory presets anywhere: in code, docs, parameter names or the UI.
- **Nothing allocates or locks on the audio thread.** `process()` and everything under `src/dsp` run per sample; fixed-size arrays, no `Vec` growth, no `String`.
- **Parameter ids are forever.** They're saved in hosts' projects: add parameters, never rename or reuse an id.
- **`src/dsp` stays free of nih-plug**, so it can be tested and rendered offline (`examples/render.rs`).
- **For the user's own use for now.** VST3 is built by default (its bindings are GPLv3); ROADMAP.md's back burner says what to do before sharing it.

## Checks before pushing

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --target x86_64-pc-windows-gnu -- -D warnings
```

For anything audible, render the demo and look at a spectrogram of it (aliasing shows as lines falling against the harmonics):

```sh
cargo run --release --example render -- demo.wav
```

Building the plug-in: `cargo xtask bundle poly-box --release` (CLAP and VST3 in `target/bundled/`). The standalone is `target/release/poly-box`; `--help` lists the audio and MIDI options.

On Linux the build needs `libasound2-dev`, `libjack-jackd2-dev`, `libx11-xcb-dev`, `libxcursor-dev` and `libgl-dev`. The plug-in window needs a real OpenGL display (under Xvfb, baseview finds no sRGB framebuffer config), so to look at the window in a cloud session, run the preview under Xvfb instead: it draws the same `editor::draw` through eframe, with the synth running silently behind it. `xdotool` (keydown, mousedown) plays it:

```sh
cargo run --release --example preview
```

The window and the audio thread share `editor::Live` (atomics only); the window never calls into the synth directly.

## Conventions

Like the rest of the family: feature branches, `main` always works, CHANGELOG.md with an Unreleased section.
