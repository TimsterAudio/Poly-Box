//! The whole instrument: five voices, the key assignment that hands notes to
//! them, the shared LFOs and the stereo mix.

use super::lfo::Lfo;
use super::voice::{Patch, Shared, Voice, VoiceMode};
use std::f32::consts::FRAC_PI_4;

pub const VOICES: usize = 5;

/// Where each voice sits at full spread, alternating sides so a chord spreads out.
const PAN_POSITIONS: [f32; VOICES] = [-1.0, 1.0, -0.5, 0.5, 0.0];
/// Each unison voice's share of the detune, so the stack is symmetric about the note.
const UNISON_POSITIONS: [f32; VOICES] = [-1.0, 1.0, -0.5, 0.5, 0.0];
/// Mix level, so a five-note chord has some headroom.
const POLY_GAIN: f32 = 0.7;
/// Five voices on one note add up; bring the stack down to sit with poly chords.
const UNISON_GAIN: f32 = 0.45;
/// Notes remembered for last-note priority in Mono and Unison.
const NOTE_STACK: usize = 16;

/// Keys held down, oldest first, without allocating on the audio thread.
#[derive(Default)]
struct NoteStack {
    notes: [u8; NOTE_STACK],
    len: usize,
}

impl NoteStack {
    fn push(&mut self, note: u8) {
        self.remove(note);
        if self.len == NOTE_STACK {
            self.notes.copy_within(1.., 0);
            self.len -= 1;
        }
        self.notes[self.len] = note;
        self.len += 1;
    }

    fn remove(&mut self, note: u8) {
        if let Some(i) = self.notes[..self.len].iter().position(|&n| n == note) {
            self.notes.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
    }

    fn top(&self) -> Option<u8> {
        self.len.checked_sub(1).map(|i| self.notes[i])
    }

    fn clear(&mut self) {
        self.len = 0;
    }
}

/// Voice `voice`'s share of a unison stack spread `detune` cents wide.
fn unison_cents(detune: f32, voice: usize) -> f32 {
    UNISON_POSITIONS[voice] * detune / 2.0
}

/// What the key assigner knows about each voice.
#[derive(Clone, Copy, Default)]
struct Assignment {
    /// The key holding this voice down, if any.
    key: Option<u8>,
    /// When it was last started and last let go, in note events.
    started: u64,
    released: u64,
}

pub struct Synth {
    sample_rate: f32,
    voices: [Voice; VOICES],
    assigned: [Assignment; VOICES],
    held: NoteStack,
    /// Poly mode starts looking for a voice here, so notes rotate round the
    /// voices as on a hardware poly instead of always landing on voice 1.
    next_voice: usize,
    events: u64,
    mode: VoiceMode,
    unison_detune: f32,
    lfo1: Lfo,
    lfo2: Lfo,
    bend: f32,
    mod_wheel: f32,
    /// DC blocker per channel (last input, last output), as a hardware
    /// output's coupling capacitor.
    dc: [(f32, f32); 2],
    dc_pole: f32,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            voices: std::array::from_fn(|i| Voice::new(sample_rate, i)),
            assigned: [Assignment::default(); VOICES],
            held: NoteStack::default(),
            next_voice: 0,
            events: 0,
            mode: VoiceMode::Poly,
            unison_detune: 0.0,
            lfo1: Lfo::new(0x1234_5678),
            lfo2: Lfo::new(0x8765_4321),
            bend: 0.0,
            mod_wheel: 0.0,
            dc: [(0.0, 0.0); 2],
            // About 5 Hz.
            dc_pole: (-2.0 * std::f32::consts::PI * 5.0 / sample_rate).exp(),
        }
    }

    pub fn reset(&mut self) {
        for v in &mut self.voices {
            v.reset();
        }
        self.assigned = [Assignment::default(); VOICES];
        self.held.clear();
        self.next_voice = 0;
        self.lfo1.reset();
        self.lfo2.reset();
        self.bend = 0.0;
        self.dc = [(0.0, 0.0); 2];
    }

    /// Let go of everything (the voices finish their release).
    pub fn all_notes_off(&mut self) {
        for (v, a) in self.voices.iter_mut().zip(&mut self.assigned) {
            v.note_off();
            a.key = None;
        }
        self.held.clear();
    }

    /// Semitones.
    pub fn set_bend(&mut self, semitones: f32) {
        self.bend = semitones;
    }

    pub fn bend(&self) -> f32 {
        self.bend
    }

    /// 0..1.
    pub fn set_mod_wheel(&mut self, v: f32) {
        self.mod_wheel = v.clamp(0.0, 1.0);
    }

    /// Take the settings that change at most once a block. A change of voice
    /// mode lets go of every note first.
    pub fn set_block(&mut self, p: &Patch) {
        if p.voice_mode != self.mode {
            self.all_notes_off();
            self.mode = p.voice_mode;
        }
        self.unison_detune = p.unison_detune;
        for (i, v) in self.voices.iter_mut().enumerate() {
            v.set_block(p);
            let pan = if self.mode == VoiceMode::Mono {
                0.0
            } else {
                PAN_POSITIONS[i] * p.spread
            };
            v.set_pan(pan);
            if self.mode == VoiceMode::Unison {
                v.set_detune(unison_cents(p.unison_detune, i));
            }
        }
    }

    fn unison_cents(&self, voice: usize) -> f32 {
        unison_cents(self.unison_detune, voice)
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) {
        self.events += 1;
        match self.mode {
            VoiceMode::Poly => {
                let i = self
                    .assigned
                    .iter()
                    .position(|a| a.key == Some(note))
                    .unwrap_or_else(|| self.pick_voice());
                self.voices[i].note_on(note, velocity, 0.0, true);
                self.assigned[i] = Assignment {
                    key: Some(note),
                    started: self.events,
                    ..self.assigned[i]
                };
                self.next_voice = (i + 1) % VOICES;
            }
            VoiceMode::Mono => {
                self.held.push(note);
                self.voices[0].note_on(note, velocity, 0.0, true);
            }
            VoiceMode::Unison => {
                self.held.push(note);
                for i in 0..VOICES {
                    let cents = self.unison_cents(i);
                    self.voices[i].note_on(note, velocity, cents, true);
                }
            }
        }
    }

    pub fn note_off(&mut self, note: u8) {
        self.events += 1;
        match self.mode {
            VoiceMode::Poly => {
                for (v, a) in self.voices.iter_mut().zip(&mut self.assigned) {
                    if a.key == Some(note) {
                        v.note_off();
                        a.key = None;
                        a.released = self.events;
                    }
                }
            }
            VoiceMode::Mono | VoiceMode::Unison => {
                let was_top = self.held.top() == Some(note);
                self.held.remove(note);
                if !was_top {
                    return;
                }
                let count = if self.mode == VoiceMode::Mono {
                    1
                } else {
                    VOICES
                };
                match self.held.top() {
                    // Fall back to the key still held, without restarting the envelopes.
                    Some(prev) => {
                        for i in 0..count {
                            let cents = if count == 1 {
                                0.0
                            } else {
                                self.unison_cents(i)
                            };
                            self.voices[i].note_on(prev, 0.0, cents, false);
                        }
                    }
                    None => {
                        for v in &mut self.voices[..count] {
                            v.note_off();
                        }
                    }
                }
            }
        }
    }

    /// A voice for a new poly note: a silent one if there is one (taking them
    /// in turn), otherwise the one released longest ago, otherwise the one
    /// held longest.
    fn pick_voice(&self) -> usize {
        let order = (0..VOICES).map(|k| (self.next_voice + k) % VOICES);
        if let Some(i) = order
            .clone()
            .find(|&i| self.assigned[i].key.is_none() && !self.voices[i].is_active())
        {
            return i;
        }
        let released = order.clone().filter(|&i| self.assigned[i].key.is_none());
        if let Some(i) = released.min_by_key(|&i| self.assigned[i].released) {
            return i;
        }
        order.min_by_key(|&i| self.assigned[i].started).unwrap_or(0)
    }

    /// How many voices are sounding.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    /// What voice `i` is doing: `None` when silent, otherwise its note and
    /// whether its key is still down.
    pub fn voice_state(&self, i: usize) -> Option<(u8, bool)> {
        let v = self.voices.get(i)?;
        v.is_active().then(|| (v.note(), v.is_gated()))
    }

    /// One stereo sample.
    #[inline]
    pub fn next(&mut self, p: &Patch) -> (f32, f32) {
        let shared = Shared {
            lfo1: self.lfo1.next(p.lfo1, self.sample_rate),
            lfo2: self.lfo2.next(p.lfo2, self.sample_rate),
            bend: self.bend,
            mod_wheel: self.mod_wheel,
        };
        let (mut l, mut r) = (0.0, 0.0);
        for v in &mut self.voices {
            if v.is_active() {
                let (y, pan) = v.next(p, shared);
                // Constant-power pan.
                let angle = (pan + 1.0) * FRAC_PI_4;
                l += y * angle.cos();
                r += y * angle.sin();
            }
        }
        let gain = if self.mode == VoiceMode::Unison {
            UNISON_GAIN
        } else {
            POLY_GAIN
        };
        let mut out = [l * gain, r * gain];
        for (x, (x1, y1)) in out.iter_mut().zip(&mut self.dc) {
            let y = *x - *x1 + self.dc_pole * *y1;
            *x1 = *x;
            *y1 = y;
            *x = y;
        }
        (out[0], out[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn synth(p: &Patch) -> Synth {
        let mut s = Synth::new(SR);
        s.set_block(p);
        s
    }

    fn run(s: &mut Synth, p: &Patch, samples: usize) {
        for _ in 0..samples {
            s.next(p);
        }
    }

    fn voice_of(s: &Synth, note: u8) -> Option<usize> {
        s.assigned.iter().position(|a| a.key == Some(note))
    }

    #[test]
    fn note_stack_returns_to_held_note() {
        let mut s = NoteStack::default();
        s.push(60);
        s.push(64);
        s.push(67);
        s.remove(67);
        assert_eq!(s.top(), Some(64));
        s.remove(60);
        assert_eq!(s.top(), Some(64));
        s.remove(64);
        assert_eq!(s.top(), None);
    }

    #[test]
    fn note_stack_drops_oldest_when_full() {
        let mut s = NoteStack::default();
        for n in 0..(NOTE_STACK as u8 + 4) {
            s.push(n);
        }
        assert_eq!(s.len, NOTE_STACK);
        assert_eq!(s.top(), Some(NOTE_STACK as u8 + 3));
        assert_eq!(s.notes[0], 4);
    }

    #[test]
    fn a_five_note_chord_takes_five_voices_and_a_sixth_steals_the_oldest() {
        let p = Patch::default();
        let mut s = synth(&p);
        for n in [60, 64, 67, 71, 74] {
            s.note_on(n, 1.0);
        }
        run(&mut s, &p, 100);
        assert_eq!(s.active_voices(), 5);
        let first = voice_of(&s, 60).unwrap();
        s.note_on(77, 1.0);
        assert_eq!(voice_of(&s, 77), Some(first));
        assert_eq!(voice_of(&s, 60), None);
    }

    #[test]
    fn notes_rotate_round_the_voices() {
        let p = Patch::default();
        let mut s = synth(&p);
        let mut used = Vec::new();
        for n in [60, 62, 64, 65, 67, 69] {
            s.note_on(n, 1.0);
            used.push(voice_of(&s, n).unwrap());
            s.note_off(n);
        }
        assert_eq!(used, vec![0, 1, 2, 3, 4, 0]);
    }

    #[test]
    fn a_released_voice_is_reused_before_a_held_one() {
        let p = Patch::default();
        let mut s = synth(&p);
        for n in [60, 62, 64, 65, 67] {
            s.note_on(n, 1.0);
        }
        let freed = voice_of(&s, 64).unwrap();
        s.note_off(64);
        run(&mut s, &p, 10); // still in its release
        s.note_on(72, 1.0);
        assert_eq!(voice_of(&s, 72), Some(freed));
        assert!(voice_of(&s, 60).is_some());
    }

    #[test]
    fn repeating_a_held_note_keeps_its_voice() {
        let p = Patch::default();
        let mut s = synth(&p);
        s.note_on(60, 1.0);
        let v = voice_of(&s, 60);
        s.note_on(60, 1.0);
        assert_eq!(voice_of(&s, 60), v);
        assert_eq!(s.assigned.iter().filter(|a| a.key.is_some()).count(), 1);
    }

    #[test]
    fn mono_plays_one_voice_and_falls_back() {
        let p = Patch {
            voice_mode: VoiceMode::Mono,
            ..Patch::default()
        };
        let mut s = synth(&p);
        s.note_on(60, 1.0);
        s.note_on(64, 1.0);
        run(&mut s, &p, 100);
        assert_eq!(s.active_voices(), 1);
        s.note_off(64);
        run(&mut s, &p, 10);
        assert!((s.voices[0].pitch() - 60.0).abs() < 0.01);
        s.note_off(60);
        run(&mut s, &p, 48_000);
        assert_eq!(s.active_voices(), 0);
    }

    #[test]
    fn unison_stacks_every_voice() {
        let p = Patch {
            voice_mode: VoiceMode::Unison,
            ..Patch::default()
        };
        let mut s = synth(&p);
        s.note_on(48, 1.0);
        run(&mut s, &p, 100);
        assert_eq!(s.active_voices(), 5);
    }

    #[test]
    fn changing_mode_lets_go_of_held_notes() {
        let p = Patch::default();
        let mut s = synth(&p);
        s.note_on(60, 1.0);
        let mono = Patch {
            voice_mode: VoiceMode::Mono,
            ..p
        };
        s.set_block(&mono);
        run(&mut s, &mono, 48_000);
        assert_eq!(s.active_voices(), 0);
    }

    #[test]
    fn spread_places_voices_in_the_stereo_field() {
        let p = Patch {
            spread: 1.0,
            ..Patch::default()
        };
        let mut s = synth(&p);
        s.note_on(60, 1.0); // voice 1, hard left
        let (mut l, mut r) = (0.0, 0.0);
        for _ in 0..24_000 {
            let (a, b) = s.next(&p);
            l += a * a;
            r += b * b;
        }
        assert!(l > 100.0 * r, "{l} {r}");
    }
}
