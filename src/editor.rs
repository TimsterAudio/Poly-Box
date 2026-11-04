//! The window: five voice lights, the controls (a plain list until the Box
//! panel, step 4) and a keyboard you can play with the mouse or the computer's
//! keys.
//!
//! The window and the audio thread share a [`Live`]: the window writes the
//! keys it's holding, the audio thread picks them up each block through a
//! [`KeyBridge`] and writes back what's held and what each voice is playing.
//! All of it is atomics, so the audio thread never waits on the window.

use crate::dsp::{Synth, VOICES};
use nih_plug::prelude::{ParamSetter, Params};
use nih_plug_egui::egui::{
    self, Align2, Color32, CornerRadius, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use nih_plug_egui::widgets::generic_ui;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Duration;

/// The computer keys that play notes, as semitones above the keyboard's
/// octave: the home row is the white keys, the row above the black keys.
const NOTE_KEYS: [(Key, u8); 16] = [
    (Key::A, 0),
    (Key::W, 1),
    (Key::S, 2),
    (Key::E, 3),
    (Key::D, 4),
    (Key::F, 5),
    (Key::T, 6),
    (Key::G, 7),
    (Key::Y, 8),
    (Key::H, 9),
    (Key::U, 10),
    (Key::J, 11),
    (Key::K, 12),
    (Key::O, 13),
    (Key::L, 14),
    (Key::P, 15),
];

/// The on-screen keyboard runs from C1 to C7.
const LOWEST: u8 = 24;
const HIGHEST: u8 = 96;

const LIT: Color32 = Color32::from_rgb(0xE0, 0x44, 0x2E);
const DIM: Color32 = Color32::from_rgb(0x6A, 0x26, 0x1E);
const OFF: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);

/// What the window and the audio thread share.
pub struct Live {
    /// Keys the window is holding down: velocity 1..=127, 0 when up.
    gui_keys: [AtomicU8; 128],
    /// Keys held from anywhere (window or MIDI), as the audio thread last saw them.
    held: [AtomicBool; 128],
    /// Each voice: 0 when silent, otherwise note + 1, plus 0x100 while its key is down.
    voices: [AtomicU16; VOICES],
}

impl Default for Live {
    fn default() -> Self {
        Self {
            gui_keys: std::array::from_fn(|_| AtomicU8::new(0)),
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
}

/// The audio thread's side: turns the window's keys into notes and keeps
/// track of what's held, for the lights.
pub struct KeyBridge {
    gui_prev: [u8; 128],
    held: [bool; 128],
}

impl Default for KeyBridge {
    fn default() -> Self {
        Self {
            gui_prev: [0; 128],
            held: [false; 128],
        }
    }
}

impl KeyBridge {
    pub fn note_on(&mut self, synth: &mut Synth, note: u8, velocity: f32) {
        self.held[note as usize & 127] = true;
        synth.note_on(note, velocity);
    }

    pub fn note_off(&mut self, synth: &mut Synth, note: u8) {
        self.held[note as usize & 127] = false;
        synth.note_off(note);
    }

    /// Play whatever the window pressed or let go since the last call.
    pub fn take_window_keys(&mut self, live: &Live, synth: &mut Synth) {
        for note in 0..128u8 {
            let v = live.gui_keys[note as usize].load(Relaxed);
            let prev = self.gui_prev[note as usize];
            if v != prev {
                self.gui_prev[note as usize] = v;
                if v > 0 && prev == 0 {
                    self.note_on(synth, note, v as f32 / 127.0);
                } else if v == 0 {
                    self.note_off(synth, note);
                }
            }
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

/// The window's side of the keyboard: which computer keys and which on-screen
/// key are down, the octave and the velocity.
pub struct Keyboard {
    /// The note the A key plays.
    base: u8,
    velocity: u8,
    /// Computer keys down, and the note each started (so changing octave
    /// while holding a key still lets go of the right note).
    keys_down: HashMap<Key, u8>,
    /// The on-screen key under the mouse while the button is down.
    mouse_note: Option<u8>,
    /// How many presses hold each note, so the mouse and a computer key on
    /// the same note don't cut each other off.
    presses: [u8; 128],
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            base: 60,
            velocity: 100,
            keys_down: HashMap::new(),
            mouse_note: None,
            presses: [0; 128],
        }
    }
}

impl Keyboard {
    fn press(&mut self, live: &Live, note: u8, velocity: u8) {
        let n = note as usize & 127;
        self.presses[n] = self.presses[n].saturating_add(1);
        if self.presses[n] == 1 {
            live.gui_keys[n].store(velocity.clamp(1, 127), Relaxed);
        }
    }

    fn release(&mut self, live: &Live, note: u8) {
        let n = note as usize & 127;
        if self.presses[n] > 0 {
            self.presses[n] -= 1;
            if self.presses[n] == 0 {
                live.gui_keys[n].store(0, Relaxed);
            }
        }
    }

    fn release_all(&mut self, live: &Live) {
        self.keys_down.clear();
        self.mouse_note = None;
        self.presses = [0; 128];
        for k in &live.gui_keys {
            k.store(0, Relaxed);
        }
    }

    /// Handle the computer keys.
    fn handle_keys(&mut self, ctx: &egui::Context, live: &Live) {
        // While a value is being typed into a control, the letters are for that.
        let typing = ctx.wants_keyboard_input();
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } if !typing => match key {
                    Key::Z => self.base = self.base.saturating_sub(12).max(LOWEST),
                    Key::X => self.base = (self.base + 12).min(HIGHEST - 12),
                    Key::C => self.velocity = self.velocity.saturating_sub(16).max(1),
                    Key::V => self.velocity = (self.velocity + 16).min(127),
                    Key::Escape => self.release_all(live),
                    _ => {
                        if let Some(&(_, offset)) = NOTE_KEYS.iter().find(|(k, _)| *k == key) {
                            if !self.keys_down.contains_key(&key) {
                                let note = (self.base + offset).min(127);
                                self.keys_down.insert(key, note);
                                self.press(live, note, self.velocity);
                            }
                        }
                    }
                },
                egui::Event::Key {
                    key,
                    pressed: false,
                    ..
                } => {
                    if let Some(note) = self.keys_down.remove(&key) {
                        self.release(live, note);
                    }
                }
                // Keys let go while the window was in the background never
                // arrive, so let go of everything when it loses focus.
                egui::Event::WindowFocused(false) => self.release_all(live),
                _ => {}
            }
        }
    }
}

pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!("{}{}", NAMES[note as usize % 12], note as i32 / 12 - 1)
}

fn is_black(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
}

/// Draw the whole window.
pub fn draw<P: Params>(
    ctx: &egui::Context,
    params: Arc<P>,
    setter: &ParamSetter,
    live: &Live,
    kb: &mut Keyboard,
) {
    kb.handle_keys(ctx, live);

    egui::TopBottomPanel::top("voices").show(ctx, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("Poly Box");
            ui.add_space(24.0);
            voice_lights(ui, live);
        });
        ui.add_space(4.0);
    });

    egui::TopBottomPanel::bottom("keyboard").show(ctx, |ui| {
        ui.add_space(6.0);
        ui.label(format!(
            "Keys A W S E D F T G Y H U J K O L P play from {}  ·  Z / X octave  ·  C / V velocity ({})  ·  Esc lets go of everything",
            note_name(kb.base),
            kb.velocity
        ));
        ui.add_space(4.0);
        piano(ui, live, kb);
        ui.add_space(6.0);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        generic_ui::create(ui, params, setter, generic_ui::GenericSlider);
    });

    // Keep the lights moving.
    ctx.request_repaint_after(Duration::from_millis(30));
}

fn voice_lights(ui: &mut egui::Ui, live: &Live) {
    for i in 0..VOICES {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(58.0, 40.0), Sense::hover());
        let p = ui.painter();
        let centre = Pos2::new(rect.center().x, rect.top() + 11.0);
        let (fill, label) = match live.voice(i) {
            VoiceLight::Off => (OFF, String::new()),
            VoiceLight::Held(n) => (LIT, note_name(n)),
            VoiceLight::Releasing(n) => (DIM, note_name(n)),
        };
        p.circle(
            centre,
            8.0,
            fill,
            Stroke::new(1.0_f32, Color32::from_gray(90)),
        );
        let text = if label.is_empty() {
            format!("{}", i + 1)
        } else {
            label
        };
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 6.0),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(12.0),
            ui.visuals().text_color(),
        );
    }
}

/// The key under `pos`, black keys first since they sit on top.
fn key_at(pos: Pos2, keys: &[(u8, Rect)]) -> Option<u8> {
    keys.iter()
        .filter(|(n, _)| is_black(*n))
        .chain(keys.iter().filter(|(n, _)| !is_black(*n)))
        .find(|(_, r)| r.contains(pos))
        .map(|(n, _)| *n)
}

fn piano(ui: &mut egui::Ui, live: &Live, kb: &mut Keyboard) {
    let whites = (LOWEST..=HIGHEST).filter(|&n| !is_black(n)).count() as f32;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 96.0), Sense::click_and_drag());
    let white_w = width / whites;

    let mut keys: Vec<(u8, Rect)> = Vec::new();
    let mut x = rect.left();
    for n in LOWEST..=HIGHEST {
        if is_black(n) {
            let w = white_w * 0.62;
            keys.push((
                n,
                Rect::from_min_size(
                    Pos2::new(x - w / 2.0, rect.top()),
                    Vec2::new(w, rect.height() * 0.6),
                ),
            ));
        } else {
            keys.push((
                n,
                Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(white_w, rect.height())),
            ));
            x += white_w;
        }
    }

    // Mouse: press, drag across keys, let go. Lower on a key is louder.
    let pointer = response.interact_pointer_pos();
    let down = response.is_pointer_button_down_on();
    let under = if down {
        pointer.and_then(|p| key_at(p, &keys))
    } else {
        None
    };
    if under != kb.mouse_note {
        if let Some(old) = kb.mouse_note.take() {
            kb.release(live, old);
        }
        if let (Some(n), Some(p)) = (under, pointer) {
            let depth = ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
            kb.press(live, n, (30.0 + 97.0 * depth) as u8);
            kb.mouse_note = Some(n);
        }
    }

    let painter = ui.painter_at(rect);
    let mapped = kb.base..=kb.base + NOTE_KEYS[NOTE_KEYS.len() - 1].1;
    for pass_black in [false, true] {
        for &(n, r) in keys.iter().filter(|(n, _)| is_black(*n) == pass_black) {
            let held = live.is_held(n);
            let fill = match (held, pass_black) {
                (true, _) => LIT,
                (false, false) => Color32::from_gray(232),
                (false, true) => Color32::from_gray(24),
            };
            painter.rect(
                r,
                CornerRadius::same(2),
                fill,
                Stroke::new(1.0_f32, Color32::from_gray(60)),
                StrokeKind::Inside,
            );
            // Label the keys the computer keyboard reaches.
            if mapped.contains(&n) {
                if let Some((key, _)) = NOTE_KEYS.iter().find(|(_, o)| kb.base + o == n) {
                    let colour = if pass_black || held {
                        Color32::from_gray(220)
                    } else {
                        Color32::from_gray(60)
                    };
                    painter.text(
                        Pos2::new(r.center().x, r.bottom() - 9.0),
                        Align2::CENTER_CENTER,
                        key.name(),
                        FontId::proportional(10.0),
                        colour,
                    );
                }
            }
            if n % 12 == 0 && !held {
                painter.text(
                    Pos2::new(r.center().x, r.bottom() - 22.0),
                    Align2::CENTER_CENTER,
                    note_name(n),
                    FontId::proportional(9.0),
                    Color32::from_gray(120),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Patch;

    #[test]
    fn note_names() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C♯4");
        assert_eq!(note_name(24), "C1");
    }

    #[test]
    fn window_keys_reach_the_synth_and_come_back_as_lights() {
        let live = Live::default();
        let mut kb = Keyboard::default();
        let mut bridge = KeyBridge::default();
        let p = Patch::default();
        let mut synth = Synth::new(48_000.0);
        synth.set_block(&p);

        kb.press(&live, 60, 100);
        kb.press(&live, 64, 100);
        bridge.take_window_keys(&live, &mut synth);
        for _ in 0..64 {
            synth.next(&p);
        }
        bridge.publish(&live, &synth);
        assert!(live.is_held(60) && live.is_held(64));
        let lights: Vec<VoiceLight> = (0..VOICES).map(|i| live.voice(i)).collect();
        assert!(lights.contains(&VoiceLight::Held(60)));
        assert!(lights.contains(&VoiceLight::Held(64)));

        kb.release(&live, 60);
        bridge.take_window_keys(&live, &mut synth);
        for _ in 0..64 {
            synth.next(&p);
        }
        bridge.publish(&live, &synth);
        assert!(!live.is_held(60));
        let lights: Vec<VoiceLight> = (0..VOICES).map(|i| live.voice(i)).collect();
        assert!(lights.contains(&VoiceLight::Releasing(60)));
    }

    #[test]
    fn two_presses_on_one_note_need_two_releases() {
        let live = Live::default();
        let mut kb = Keyboard::default();
        kb.press(&live, 60, 100);
        kb.press(&live, 60, 80);
        kb.release(&live, 60);
        assert_eq!(live.gui_keys[60].load(Relaxed), 100);
        kb.release(&live, 60);
        assert_eq!(live.gui_keys[60].load(Relaxed), 0);
    }
}
