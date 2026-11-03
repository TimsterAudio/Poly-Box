//! Render a short demo through the voice to a WAV file, no audio device needed:
//!
//! ```sh
//! cargo run --release --example render -- demo.wav
//! ```
//!
//! Four sounds in turn: a plucky bass, a resonant filter sweep, a sync lead
//! and the SEM notch/high-pass sweep.

use poly_box::dsp::{Patch, Voice};
use std::io::Write;

const SR: f32 = 48_000.0;

struct Note {
    note: u8,
    start: f32,
    length: f32,
    velocity: f32,
}

/// Play `notes` with a patch that may change over time (`at` gets the time in
/// seconds), returning the samples.
fn play(notes: &[Note], seconds: f32, at: impl Fn(f32) -> Patch) -> Vec<f32> {
    let mut v = Voice::new(SR);
    let n = (seconds * SR) as usize;
    let mut out = Vec::with_capacity(n);
    let mut idx = 0;
    let mut gate_off_at = None;
    for i in 0..n {
        let t = i as f32 / SR;
        let p = at(t);
        if i % 64 == 0 {
            v.set_envelopes(&p);
        }
        if idx < notes.len() && t >= notes[idx].start {
            let nt = &notes[idx];
            v.note_on(nt.note, nt.velocity, true);
            gate_off_at = Some(nt.start + nt.length);
            idx += 1;
        }
        if gate_off_at.is_some_and(|off| t >= off) {
            v.note_off();
            gate_off_at = None;
        }
        out.push(v.next(&p, 0.0));
    }
    out
}

fn seq(pattern: &[(u8, f32)], step: f32, gate: f32, start: f32) -> Vec<Note> {
    pattern
        .iter()
        .enumerate()
        .map(|(i, &(note, velocity))| Note {
            note,
            start: start + i as f32 * step,
            length: step * gate,
            velocity,
        })
        .collect()
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "poly-box-demo.wav".into());
    let mut audio = Vec::new();

    // 1. Bass: two saws an octave down, short filter envelope, a touch of drive.
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
        0.0,
    );
    audio.extend(play(&bass, 3.2, |_| Patch {
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

    // 2. One long note, cutoff swept up six octaves with the resonance high.
    let pad = [Note {
        note: 45,
        start: 0.0,
        length: 4.5,
        velocity: 0.8,
    }];
    audio.extend(play(&pad, 5.0, |t| Patch {
        osc1_shape: 0.65,
        osc2_shape: 0.0,
        osc2_cents: 9.0,
        cutoff_hz: 120.0 * (t / 4.5 * 6.0).min(6.0).exp2(),
        resonance: 0.85,
        filter_env_amount: 0.0,
        amp_env: [0.3, 1.0, 1.0, 0.5],
        ..Patch::default()
    }));

    // 3. Sync lead: osc 2 synced to osc 1 and stepped up a semitone at a time.
    let lead = seq(
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
        0.0,
    );
    audio.extend(play(&lead, 2.4, |t| Patch {
        sync: true,
        osc1_level: 0.3,
        osc2_level: 0.9,
        osc2_semitones: (t * 5.0) as i32 % 12 + 1,
        cutoff_hz: 2_500.0,
        resonance: 0.3,
        filter_env_amount: 0.3,
        amp_env: [0.005, 0.3, 0.8, 0.15],
        ..Patch::default()
    }));

    // 4. The SEM mode sweep: low-pass, through notch, to high-pass, on noise and a square.
    let held = [Note {
        note: 48,
        start: 0.0,
        length: 3.6,
        velocity: 0.8,
    }];
    audio.extend(play(&held, 4.0, |t| Patch {
        osc1_shape: 0.5,
        osc2_level: 0.0,
        noise_level: 0.25,
        cutoff_hz: 900.0,
        resonance: 0.6,
        filter_mode: (t / 3.6).min(1.0),
        filter_env_amount: 0.0,
        amp_env: [0.05, 1.0, 1.0, 0.3],
        ..Patch::default()
    }));

    let peak = audio.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let gain = 0.9 / peak.max(1e-6);
    write_wav(&path, &audio, gain).expect("couldn't write the WAV file");
    eprintln!(
        "wrote {path}: {:.1} s, peak {peak:.2} before normalising",
        audio.len() as f32 / SR
    );
}

/// 16-bit mono WAV.
fn write_wav(path: &str, samples: &[f32], gain: f32) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let data_len = (samples.len() * 2) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&(SR as u32).to_le_bytes())?;
    f.write_all(&(SR as u32 * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for &s in samples {
        let v = (s * gain).clamp(-1.0, 1.0) * i16::MAX as f32;
        f.write_all(&(v as i16).to_le_bytes())?;
    }
    f.flush()
}
