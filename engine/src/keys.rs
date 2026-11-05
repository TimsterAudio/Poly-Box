//! Playing from a window: the keys a window holds, the wheels, and what the
//! voices are doing, shared between a window and the audio thread.
//!
//! The window side ([`KeyPresses`]) turns computer keys and mouse presses
//! into held notes in a [`Live`]; the audio side ([`KeyBridge`]) picks those
//! up once a block, plays them, and writes back what's held and what each
//! voice is playing for the lights. [`Live`] is all atomics, so the audio
//! thread never waits on the window. Nothing here knows about any GUI
//! toolkit: windows pass in letters.

use crate::synth::{Synth, VOICES};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU8, Ordering::Relaxed};

/// The computer keys that play notes, as semitones above the keyboard's
/// octave: the home row is the white keys, the row above the black keys.
pub const NOTE_LETTERS: [(char, u8); 16] = [
    ('a', 0),
    ('w', 1),
    ('s', 2),
    ('e', 3),
    ('d', 4),
    ('f', 5),
    ('t', 6),
    ('g', 7),
    ('y', 8),
    ('h', 9),
    ('u', 10),
    ('j', 11),
    ('k', 12),
    ('o', 13),
    ('l', 14),
    ('p', 15),
];

/// The lowest and highest notes the computer keys' octave can start from.
pub const LOWEST_BASE: u8 = 24;
pub const HIGHEST_BASE: u8 = 84;

/// What a window and the audio thread share.
pub struct Live {
    /// Keys the window is holding down: velocity 1..=127, 0 when up.
    window_keys: [AtomicU8; 128],
    /// Pitch bend from the window, -1..1 (f32 bits).
    bend: AtomicU32,
    /// Mod wheel from the window, 0..1 (f32 bits).
    mod_wheel: AtomicU32,
    /// Keys held from anywhere (window or MIDI), as the audio thread last saw them.
    held: [AtomicBool; 128],
    /// Each voice: 0 when silent, otherwise note + 1, plus 0x100 while its key is down.
    voices: [AtomicU16; VOICES],
}

impl Default for Live {
    fn default() -> Self {
        Self {
            window_keys: std::array::from_fn(|_| AtomicU8::new(0)),
            bend: AtomicU32::new(0f32.to_bits()),
            mod_wheel: AtomicU32::new(0f32.to_bits()),
            held: std::array::from_fn(|_| AtomicBool::new(false)),
            voices: std::array::from_fn(|_| AtomicU16::new(0)),
        }
    }
}

/// What a voice light shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceLight {
    Off,
    /// Its key is down.
    Held(u8),
    /// Still sounding after its key was let go.
    Releasing(u8),
}

impl Live {
    pub fn voice(&self, i: usize) -> VoiceLight {
        let v = self.voices[i].load(Relaxed);
        match (v & 0xFF, v & 0x100 != 0) {
            (0, _) => VoiceLight::Off,
            (n, true) => VoiceLight::Held(n as u8 - 1),
            (n, false) => VoiceLight::Releasing(n as u8 - 1),
        }
    }

    pub fn is_held(&self, note: u8) -> bool {
        self.held[note as usize & 127].load(Relaxed)
    }

    /// -1..1, springing back to 0 is the window's job.
    pub fn set_bend(&self, bend: f32) {
        self.bend.store(bend.clamp(-1.0, 1.0).to_bits(), Relaxed);
    }

    pub fn bend(&self) -> f32 {
        f32::from_bits(self.bend.load(Relaxed))
    }

    pub fn set_mod_wheel(&self, v: f32) {
        self.mod_wheel.store(v.clamp(0.0, 1.0).to_bits(), Relaxed);
    }

    pub fn mod_wheel(&self) -> f32 {
        f32::from_bits(self.mod_wheel.load(Relaxed))
    }
}

/// The audio thread's side.
pub struct KeyBridge {
    window_prev: [u8; 128],
    held: [bool; 128],
    bend_prev: f32,
    wheel_prev: f32,
    /// Semitones a full bend moves.
    pub bend_range: f32,
}

impl Default for KeyBridge {
    fn default() -> Self {
        Self {
            window_prev: [0; 128],
            held: [false; 128],
            bend_prev: 0.0,
            wheel_prev: 0.0,
            bend_range: 2.0,
        }
    }
}

impl KeyBridge {
    /// A note from anywhere else (MIDI), so the lights show it too.
    pub fn note_on(&mut self, synth: &mut Synth, note: u8, velocity: f32) {
        self.held[note as usize & 127] = true;
        synth.note_on(note, velocity);
    }

    pub fn note_off(&mut self, synth: &mut Synth, note: u8) {
        self.held[note as usize & 127] = false;
        synth.note_off(note);
    }

    /// Play whatever the window pressed or let go since the last call, and
    /// follow its wheels when they move (so MIDI wheels aren't overridden
    /// by a window that's sitting still).
    pub fn take_window(&mut self, live: &Live, synth: &mut Synth) {
        for note in 0..128u8 {
            let v = live.window_keys[note as usize].load(Relaxed);
            let prev = self.window_prev[note as usize];
            if v != prev {
                self.window_prev[note as usize] = v;
                if v > 0 && prev == 0 {
                    self.note_on(synth, note, v as f32 / 127.0);
                } else if v == 0 {
                    self.note_off(synth, note);
                }
            }
        }
        let bend = live.bend();
        if bend != self.bend_prev {
            self.bend_prev = bend;
            synth.set_bend(bend * self.bend_range);
        }
        let wheel = live.mod_wheel();
        if wheel != self.wheel_prev {
            self.wheel_prev = wheel;
            synth.set_mod_wheel(wheel);
        }
    }

    /// Tell the window what's held and what each voice is doing.
    pub fn publish(&self, live: &Live, synth: &Synth) {
        for (shown, &held) in live.held.iter().zip(&self.held) {
            shown.store(held, Relaxed);
        }
        for (i, shown) in live.voices.iter().enumerate() {
            let v = match synth.voice_state(i) {
                None => 0,
                Some((note, held)) => (note as u16 + 1) | if held { 0x100 } else { 0 },
            };
            shown.store(v, Relaxed);
        }
    }

    pub fn reset(&mut self) {
        self.held = [false; 128];
    }
}

/// The window's side: which computer keys and which on-screen key are down,
/// the keys' octave and their velocity. Lives on the window's thread.
pub struct KeyPresses {
    /// The note the A key plays.
    pub base: u8,
    pub velocity: u8,
    /// The note each computer key started, by its place in [`NOTE_LETTERS`]
    /// (so changing octave while holding a key still lets go of the right note).
    letters_down: [Option<u8>; NOTE_LETTERS.len()],
    /// The on-screen key under the mouse while the button is down.
    mouse_note: Option<u8>,
    /// How many presses hold each note, so the mouse and a computer key on
    /// the same note don't cut each other off.
    presses: [u8; 128],
}

impl Default for KeyPresses {
    fn default() -> Self {
        Self {
            base: 60,
            velocity: 100,
            letters_down: [None; NOTE_LETTERS.len()],
            mouse_note: None,
            presses: [0; 128],
        }
    }
}

impl KeyPresses {
    pub fn press(&mut self, live: &Live, note: u8, velocity: u8) {
        let n = note as usize & 127;
        self.presses[n] = self.presses[n].saturating_add(1);
        if self.presses[n] == 1 {
            live.window_keys[n].store(velocity.clamp(1, 127), Relaxed);
        }
    }

    pub fn release(&mut self, live: &Live, note: u8) {
        let n = note as usize & 127;
        if self.presses[n] > 0 {
            self.presses[n] -= 1;
            if self.presses[n] == 0 {
                live.window_keys[n].store(0, Relaxed);
            }
        }
    }

    pub fn release_all(&mut self, live: &Live) {
        self.letters_down = [None; NOTE_LETTERS.len()];
        self.mouse_note = None;
        self.presses = [0; 128];
        for k in &live.window_keys {
            k.store(0, Relaxed);
        }
    }

    /// A letter key went down. Returns true if it was one of ours: a note,
    /// Z / X (octave) or C / V (velocity).
    pub fn letter_down(&mut self, live: &Live, letter: char) -> bool {
        match letter.to_ascii_lowercase() {
            'z' => self.octave_down(),
            'x' => self.octave_up(),
            'c' => self.velocity = self.velocity.saturating_sub(16).max(1),
            'v' => self.velocity = (self.velocity + 16).min(127),
            l => {
                let Some(i) = NOTE_LETTERS.iter().position(|(c, _)| *c == l) else {
                    return false;
                };
                if self.letters_down[i].is_none() {
                    let note = (self.base + NOTE_LETTERS[i].1).min(127);
                    self.letters_down[i] = Some(note);
                    self.press(live, note, self.velocity);
                }
            }
        }
        true
    }

    pub fn letter_up(&mut self, live: &Live, letter: char) {
        let l = letter.to_ascii_lowercase();
        if let Some(i) = NOTE_LETTERS.iter().position(|(c, _)| *c == l) {
            if let Some(note) = self.letters_down[i].take() {
                self.release(live, note);
            }
        }
    }

    pub fn octave_down(&mut self) {
        self.base = self.base.saturating_sub(12).max(LOWEST_BASE);
    }

    pub fn octave_up(&mut self) {
        self.base = (self.base + 12).min(HIGHEST_BASE);
    }

    /// The computer key that plays `note` at the current octave, if any.
    pub fn letter_for(&self, note: u8) -> Option<char> {
        let offset = note.checked_sub(self.base)?;
        NOTE_LETTERS
            .iter()
            .find(|(_, o)| *o == offset)
            .map(|(c, _)| *c)
    }

    /// The mouse is down on `note` at `velocity`, or not on any key.
    pub fn mouse(&mut self, live: &Live, under: Option<(u8, u8)>) {
        let note = under.map(|(n, _)| n);
        if note == self.mouse_note {
            return;
        }
        if let Some(old) = self.mouse_note.take() {
            self.release(live, old);
        }
        if let Some((n, v)) = under {
            self.press(live, n, v);
            self.mouse_note = Some(n);
        }
    }
}

pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[note as usize % 12], note as i32 / 12 - 1)
}

pub fn is_black(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::Patch;

    #[test]
    fn note_names() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C#4");
        assert_eq!(note_name(24), "C1");
    }

    #[test]
    fn window_keys_reach_the_synth_and_come_back_as_lights() {
        let live = Live::default();
        let mut kb = KeyPresses::default();
        let mut bridge = KeyBridge::default();
        let p = Patch::default();
        let mut synth = Synth::new(48_000.0);
        synth.set_block(&p);

        kb.letter_down(&live, 'a');
        kb.letter_down(&live, 'd');
        bridge.take_window(&live, &mut synth);
        for _ in 0..64 {
            synth.next(&p);
        }
        bridge.publish(&live, &synth);
        assert!(live.is_held(60) && live.is_held(64));
        let lights: Vec<VoiceLight> = (0..VOICES).map(|i| live.voice(i)).collect();
        assert!(lights.contains(&VoiceLight::Held(60)));
        assert!(lights.contains(&VoiceLight::Held(64)));

        kb.letter_up(&live, 'a');
        bridge.take_window(&live, &mut synth);
        for _ in 0..64 {
            synth.next(&p);
        }
        bridge.publish(&live, &synth);
        assert!(!live.is_held(60));
        let lights: Vec<VoiceLight> = (0..VOICES).map(|i| live.voice(i)).collect();
        assert!(lights.contains(&VoiceLight::Releasing(60)));
    }

    #[test]
    fn changing_octave_while_holding_lets_go_of_the_right_note() {
        let live = Live::default();
        let mut kb = KeyPresses::default();
        kb.letter_down(&live, 'a');
        kb.letter_down(&live, 'x');
        kb.letter_up(&live, 'a');
        assert_eq!(live.window_keys[60].load(Relaxed), 0);
        assert_eq!(kb.base, 72);
    }

    #[test]
    fn two_presses_on_one_note_need_two_releases() {
        let live = Live::default();
        let mut kb = KeyPresses::default();
        kb.press(&live, 60, 100);
        kb.press(&live, 60, 80);
        kb.release(&live, 60);
        assert_eq!(live.window_keys[60].load(Relaxed), 100);
        kb.release(&live, 60);
        assert_eq!(live.window_keys[60].load(Relaxed), 0);
    }

    #[test]
    fn the_wheels_reach_the_synth_only_when_they_move() {
        let live = Live::default();
        let mut bridge = KeyBridge::default();
        let mut synth = Synth::new(48_000.0);
        synth.set_bend(1.5); // as if from MIDI
        bridge.take_window(&live, &mut synth);
        assert_eq!(synth.bend(), 1.5);
        live.set_bend(-1.0);
        bridge.take_window(&live, &mut synth);
        assert_eq!(synth.bend(), -2.0);
    }
}
