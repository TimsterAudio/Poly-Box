#!/usr/bin/env bash
# Package a built Poly Box for one platform into dist/:
#
#   scripts/package.sh linux-x86_64 | windows-x86_64 | macos-universal
#
# Expects `cargo build --release -p poly-box-app` and
# `cargo xtask bundle poly-box --release` to have run already.
set -euo pipefail

platform="$1"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' app/Cargo.toml | head -1)
name="poly-box-$version-$platform"
out="dist/$name"
rm -rf dist
mkdir -p "$out/plugins"

case "$platform" in
  linux-*)
    cp target/release/poly-box "$out/"
    ;;
  windows-*)
    cp target/release/poly-box.exe "$out/Poly Box.exe"
    ;;
  macos-*)
    app="$out/Poly Box.app"
    mkdir -p "$app/Contents/MacOS"
    cp target/release/poly-box "$app/Contents/MacOS/poly-box"
    cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Poly Box</string>
  <key>CFBundleDisplayName</key><string>Poly Box</string>
  <key>CFBundleIdentifier</key><string>com.timsters-audio-works.poly-box</string>
  <key>CFBundleExecutable</key><string>poly-box</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
    # Unsigned: an ad-hoc signature keeps Apple silicon willing to run it.
    codesign --force --deep --sign - "$app" 2>/dev/null || true
    ;;
  *)
    echo "unknown platform $platform" >&2
    exit 1
    ;;
esac

cp -R target/bundled/poly-box.clap target/bundled/poly-box.vst3 "$out/plugins/"

cat > "$out/README.txt" <<TXT
Poly Box $version
Timster's Audio Works

A five-voice analog-style polysynth with a hardware front panel.

RUNNING IT
  Windows: double-click "Poly Box.exe". Windows may warn that it's from an
           unknown publisher: choose "More info", then "Run anyway".
  macOS:   drag "Poly Box.app" to Applications. The first time, right-click
           it and choose Open (it isn't signed by an Apple developer ID).
  Linux:   run ./poly-box (it needs ALSA, which every desktop has; PipeWire
           and PulseAudio provide it).

  It plays through the computer's default audio output.

PLAYING IT
  Click the window, then play with the computer keys:
    A W S E D F T G Y H U J K O L P   the notes
    Z / X                             octave down / up
    C / V                             softer / louder
    Esc                               all notes off
  Or click and drag on the keyboard. Drag knobs and faders up or down
  (hold Shift for fine control); double-click one to reset it.

THE PLUG-INS (optional, for a DAW)
  The plugins folder has CLAP and VST3 versions. Copy them to:
    Windows: C:\\Program Files\\Common Files\\CLAP and ...\\VST3
    macOS:   ~/Library/Audio/Plug-Ins/CLAP and .../VST3
    Linux:   ~/.clap and ~/.vst3

https://github.com/TimsterAudio/poly-box
TXT

cd dist
case "$platform" in
  linux-*) tar czf "$name.tar.gz" "$name" ;;
  windows-*) 7z a -tzip -bso0 "$name.zip" "$name" ;;
  macos-*) ditto -c -k --keepParent "$name" "$name.zip" ;;
esac
ls -l "$(pwd)"/"$name".*
