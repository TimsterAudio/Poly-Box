//! Render a demo through the synth to a stereo WAV file, no audio device needed:
//!
//! ```sh
//! cargo run --release --example render -- demo.wav
//! ```
//!
//! Five sections: a mono bass with a little glide, poly chords with Vintage
//! and pulse-width modulation, a unison lead, a sync lead, and the SEM's
//! low-pass → notch → high-pass sweep on a chord.

use poly_box::dsp::{
    LfoSettings, LfoShape, ModDest, ModSlot, ModSource, Patch, Synth, VoiceMode, MOD_SLOTS,
};
use std::io::Write;

const SR: f32 = 48_000.0;

/// A note: key, start and length in seconds, velocity.
type Note = (u8, f32, f32, f32);

/// Play `notes` for `seconds` with a patch that may change over time.
fn play(notes: &[Note], seconds: f32, at: impl Fn(f32) -> Patch) -> Vec<(f32, f32)> {
    let mut synth = Synth::new(SR);
    let mut events: Vec<(f32, bool, u8, f32)> = notes
        .iter()
        .flat_map(|&(n, start, len, vel)| [(start, true, n, vel), (start + len, false, n, 0.0)])
        .collect();
    // Offs before ons at the same moment, so a repeated note retriggers.
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut next = 0;
    let n = (seconds * SR) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SR;
        let p = at(t);
        if i % 64 == 0 {
            synth.set_block(&p);
        }
        while next < events.len() && events[next].0 <= t {
            let (_, on, note, vel) = events[next];
            if on {
                synth.note_on(note, vel);
            } else {
                synth.note_off(note);
            }
            next += 1;
        }
        out.push(synth.next(&p));
    }
    out
}

fn seq(pattern: &[(u8, f32)], step: f32, gate: f32) -> Vec<Note> {
    pattern
        .iter()
        .enumerate()
        .map(|(i, &(note, vel))| (note, i as f32 * step, step * gate, vel))
        .collect()
}

fn chords(chords: &[&[u8]], length: f32) -> Vec<Note> {
    chords
        .iter()
        .enumerate()
        .flat_map(|(i, c)| {
            c.iter()
                .map(move |&n| (n, i as f32 * length, length * 0.95, 0.8))
        })
        .collect()
}

fn mods(slots: &[(ModSource, ModDest, f32)]) -> [ModSlot; MOD_SLOTS] {
    let mut m = [ModSlot::default(); MOD_SLOTS];
    for (slot, &(source, dest, amount)) in m.iter_mut().zip(slots) {
        *slot = ModSlot {
            source,
            dest,
            amount,
        };
    }
    m
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "poly-box-demo.wav".into());
    let mut audio = Vec::new();

    // 1. Mono bass: two saws an octave down, short filter envelope, a touch of glide.
    let bass = seq(
        &[
            (36, 1.0),
            (36, 0.6),
            (48, 0.8),
            (36, 0.6),
            (39, 1.0),
            (36, 0.6),
            (43, 0.8),
            (46, 0.7),
        ]
        .repeat(2),
        0.18,
        0.6,
    );
    audio.extend(play(&bass, 3.2, |_| Patch {
        voice_mode: VoiceMode::Mono,
        glide: 0.04,
        osc1_octave: -1,
        osc2_octave: -1,
        osc2_cents: -6.0,
        cutoff_hz: 180.0,
        resonance: 0.45,
        drive: 0.6,
        filter_env_amount: 0.55,
        filter_velocity: 0.6,
        filter_env: [0.002, 0.22, 0.05, 0.15],
        amp_env: [0.002, 0.4, 0.7, 0.08],
        ..Patch::default()
    }));

    // 2. Poly chords: a pulse and a saw, slow PWM from LFO 2, gentle vibrato,
    //    Vintage up and the voices spread wide.
    let pad = chords(
        &[
            &[45, 52, 57, 60, 64],
            &[41, 48, 53, 57, 64],
            &[43, 50, 55, 59, 62],
            &[40, 47, 52, 56, 59],
        ],
        1.6,
    );
    audio.extend(play(&pad, 7.2, |_| Patch {
        osc1_shape: 0.6,
        osc2_cents: 5.0,
        cutoff_hz: 900.0,
        resonance: 0.3,
        filter_env_amount: 0.25,
        filter_env: [0.4, 1.2, 0.4, 0.8],
        amp_env: [0.25, 1.0, 0.85, 0.9],
        vintage: 0.7,
        spread: 0.8,
        lfo1: LfoSettings {
            rate_hz: 5.5,
            shape: LfoShape::Triangle,
        },
        lfo2: LfoSettings {
            rate_hz: 0.4,
            shape: LfoShape::Triangle,
        },
        mods: mods(&[
            (ModSource::Lfo1, ModDest::Pitch, 0.008),
            (ModSource::Lfo2, ModDest::Osc1Shape, 0.3),
        ]),
        ..Patch::default()
    }));

    // 3. Unison lead with glide: all five voices on each note, detuned and spread.
    let lead = seq(
        &[
            (57, 0.9),
            (60, 0.9),
            (64, 0.9),
            (69, 1.0),
            (67, 0.8),
            (64, 0.8),
            (62, 0.8),
            (64, 1.0),
        ],
        0.3,
        1.02, // overlapping, so it glides legato
    );
    audio.extend(play(&lead, 3.0, |_| Patch {
        voice_mode: VoiceMode::Unison,
        unison_detune: 25.0,
        glide: 0.08,
        spread: 0.9,
        cutoff_hz: 1_800.0,
        resonance: 0.35,
        filter_env_amount: 0.3,
        amp_env: [0.01, 0.5, 0.9, 0.3],
        ..Patch::default()
    }));

    // 4. Sync lead: osc 2 synced to osc 1, its pitch swept by the aux envelope on every note.
    let sync = seq(
        &[
            (60, 0.9),
            (63, 0.9),
            (67, 0.9),
            (70, 0.9),
            (72, 1.0),
            (70, 0.8),
            (67, 0.8),
            (63, 0.8),
        ],
        0.25,
        0.9,
    );
    audio.extend(play(&sync, 2.4, |_| Patch {
        sync: true,
        osc1_level: 0.3,
        osc2_level: 0.9,
        osc2_semitones: 7,
        cutoff_hz: 2_500.0,
        resonance: 0.3,
        filter_env_amount: 0.3,
        amp_env: [0.005, 0.3, 0.8, 0.15],
        aux_env: [0.001, 0.25, 0.0, 0.1],
        mods: mods(&[(ModSource::AuxEnv, ModDest::Osc2Pitch, 0.9)]),
        ..Patch::default()
    }));

    // 5. The SEM mode sweep on a held chord: low-pass, through notch, to high-pass.
    let held = [
        (48, 0.0, 3.6, 0.8),
        (55, 0.0, 3.6, 0.8),
        (64, 0.0, 3.6, 0.8),
    ];
    audio.extend(play(&held, 4.0, |t| Patch {
        osc1_shape: 0.5,
        osc2_level: 0.5,
        noise_level: 0.1,
        cutoff_hz: 900.0,
        resonance: 0.6,
        filter_mode: (t / 3.6).min(1.0),
        filter_env_amount: 0.0,
        amp_env: [0.05, 1.0, 1.0, 0.3],
        spread: 0.6,
        ..Patch::default()
    }));

    let peak = audio
        .iter()
        .fold(0.0f32, |m, &(l, r)| m.max(l.abs()).max(r.abs()));
    let gain = 0.9 / peak.max(1e-6);
    write_wav(&path, &audio, gain).expect("couldn't write the WAV file");
    eprintln!(
        "wrote {path}: {:.1} s, peak {peak:.2} before normalising",
        audio.len() as f32 / SR
    );
}

/// 16-bit stereo WAV.
fn write_wav(path: &str, samples: &[(f32, f32)], gain: f32) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let data_len = (samples.len() * 4) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&2u16.to_le_bytes())?; // stereo
    f.write_all(&(SR as u32).to_le_bytes())?;
    f.write_all(&(SR as u32 * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for &(l, r) in samples {
        for s in [l, r] {
            let v = (s * gain).clamp(-1.0, 1.0) * i16::MAX as f32;
            f.write_all(&(v as i16).to_le_bytes())?;
        }
    }
    f.flush()
}
