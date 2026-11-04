# Poly Box: working conventions

A five-voice analog-style polysynth in the Timster's Audio Works **Box** family (with Drone Box, Drum Box and EQ Box). TAW-Panel's `FAMILY.md` is the family overview; `ROADMAP.md` here holds the pipeline and the arpeggiator and sequencer specs.

## Hard rules

- **Inspired by, never a copy.** The SEM sound and the TEO-5's playing are the reference. No Oberheim or Sequential names, logos, panel layouts or factory presets anywhere: in code, docs, parameter names or the UI.
- **Nothing allocates or locks on the audio thread.** `process()`, the app's audio callback and everything under `engine/` run per sample, except `engine::keys::KeyPresses`, which lives on the window's thread; fixed-size arrays, no `Vec` growth, no `String`.
- **Parameter and control ids are forever.** The plug-in's are saved in hosts' projects, the app's (`app/src/controls.rs`) in its settings file: add parameters, never rename or reuse an id.
- **`engine/` stays free of any plug-in framework or GUI toolkit.** The app, the plug-ins and the offline renderer all share it.
- **The app is the instrument.** The user plays the app (`app/`), not a DAW: the panel is where the look goes. The plug-ins (root crate) keep a plain window.
- **Apps pin a TAW-Panel commit** (`app/Cargo.toml`), as in the rest of the family; the panel follows Drone Box's `docs/DESIGN.md` (faders for levels, times, cutoff, resonance and rates; knobs coloured by family; a readout under every control).
- **For the user's own use for now.** VST3 is built by default (its bindings are GPLv3); ROADMAP.md's back burner says what to do before sharing it.

## Checks before pushing

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy --workspace --target x86_64-pc-windows-gnu -- -D warnings
```

For anything audible, render the demo and look at a spectrogram of it (aliasing shows as lines falling against the harmonics):

```sh
cargo run --release --example render -- demo.wav
```

The app: `cargo run --release -p poly-box-app` (the program is `poly-box`). It runs under Xvfb in a cloud session (no audio device there, so it plays silently but the lights and meter work); look at it at full size (1720 × 960) and small (1280 × 720), and play it with `xdotool` (keydown, mousedown, mousemove).

The plug-ins: `cargo xtask bundle poly-box --release` (CLAP and VST3 in `target/bundled/`). Their window needs a real OpenGL display (under Xvfb, baseview finds no sRGB framebuffer config).

On Linux the build needs `libasound2-dev`, `libjack-jackd2-dev`, `libx11-xcb-dev`, `libxcursor-dev` and `libgl-dev`.

## Releasing a version

1. Move Unreleased in CHANGELOG.md into `## X.Y.Z – YYYY-MM-DD`, and bump `version` in `app/Cargo.toml` (and the root and `engine/` Cargo.toml, which move together).
2. Merge to `main`. The Release workflow (`.github/workflows/release.yml`) sees a version with no GitHub release yet, builds Linux (x86_64), Windows (x86_64) and macOS (universal) packages with `scripts/package.sh`, and publishes release `vX.Y.Z` with that CHANGELOG section as its notes. Each package holds the app, the CLAP and VST3 plug-ins, and a README.
3. Test packages of a branch: Actions > Release > Run workflow on that branch (artifacts only, no release).

The builds aren't signed: Windows warns about an unknown publisher and macOS needs right-click > Open the first time (the README in each package says so).

## Conventions

Like the rest of the family: feature branches, `main` always works, CHANGELOG.md with an Unreleased section.
