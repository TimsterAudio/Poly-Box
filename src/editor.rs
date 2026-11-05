//! The plug-in's window: five voice lights, the controls as a plain list, and
//! a keyboard to play with the mouse or the computer keys. The app (`app/`)
//! is where the panel lives; in a host this is enough to get by.
//!
//! Playing goes through the engine's [`keys`](crate::dsp::keys): this window
//! only turns egui's key events into letters.

use crate::dsp::keys::{is_black, note_name, KeyPresses, Live, VoiceLight};
use crate::dsp::VOICES;
use nih_plug::prelude::{ParamSetter, Params};
use nih_plug_egui::egui::{
    self, Align2, Color32, CornerRadius, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use nih_plug_egui::widgets::generic_ui;
use std::sync::Arc;
use std::time::Duration;

const LOWEST: u8 = 24;
const HIGHEST: u8 = 96;

const LIT: Color32 = Color32::from_rgb(0xE0, 0x44, 0x2E);
const DIM: Color32 = Color32::from_rgb(0x6A, 0x26, 0x1E);
const OFF: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);

fn letter(key: Key) -> Option<char> {
    let name = key.name();
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

fn handle_keys(ctx: &egui::Context, live: &Live, kb: &mut KeyPresses) {
    // While a value is being typed into a control, the letters are for that.
    // Only a text field counts: a focused slider takes no letters, and
    // `wants_keyboard_input` (true for any focus) silenced the keys after
    // every click on one.
    let typing = ctx
        .memory(|m| m.focused())
        .is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some());
    for event in ctx.input(|i| i.events.clone()) {
        match event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                ..
            } if !typing => {
                if key == Key::Escape {
                    kb.release_all(live);
                } else if let Some(c) = letter(key) {
                    kb.letter_down(live, c);
                }
            }
            egui::Event::Key {
                key,
                pressed: false,
                ..
            } => {
                if let Some(c) = letter(key) {
                    kb.letter_up(live, c);
                }
            }
            // Keys let go while the window was in the background never
            // arrive, so let go of everything when it loses focus.
            egui::Event::WindowFocused(false) => kb.release_all(live),
            _ => {}
        }
    }
}

/// Draw the whole window.
pub fn draw<P: Params>(
    ctx: &egui::Context,
    params: Arc<P>,
    setter: &ParamSetter,
    live: &Live,
    kb: &mut KeyPresses,
) {
    handle_keys(ctx, live, kb);

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

    ctx.request_repaint_after(Duration::from_millis(30));
}

fn voice_lights(ui: &mut egui::Ui, live: &Live) {
    for i in 0..VOICES {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(58.0, 40.0), Sense::hover());
        let p = ui.painter();
        let centre = Pos2::new(rect.center().x, rect.top() + 11.0);
        let (fill, label) = match live.voice(i) {
            VoiceLight::Off => (OFF, format!("{}", i + 1)),
            VoiceLight::Held(n) => (LIT, note_name(n)),
            VoiceLight::Releasing(n) => (DIM, note_name(n)),
        };
        p.circle(
            centre,
            8.0,
            fill,
            Stroke::new(1.0_f32, Color32::from_gray(90)),
        );
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 6.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            ui.visuals().text_color(),
        );
    }
}

fn piano(ui: &mut egui::Ui, live: &Live, kb: &mut KeyPresses) {
    let whites = (LOWEST..=HIGHEST).filter(|&n| !is_black(n)).count() as f32;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 96.0), Sense::click_and_drag());
    let white_w = width / whites;

    let mut keys: Vec<(u8, Rect)> = Vec::new();
    let mut x = rect.left();
    for n in LOWEST..=HIGHEST {
        let r = if is_black(n) {
            let w = white_w * 0.62;
            Rect::from_min_size(
                Pos2::new(x - w / 2.0, rect.top()),
                Vec2::new(w, rect.height() * 0.6),
            )
        } else {
            x += white_w;
            Rect::from_min_size(
                Pos2::new(x - white_w, rect.top()),
                Vec2::new(white_w, rect.height()),
            )
        };
        keys.push((n, r));
    }

    // Mouse: press, drag across keys, let go. Lower on a key is louder.
    let pointer = response.interact_pointer_pos();
    let under = if response.is_pointer_button_down_on() {
        pointer.and_then(|p| {
            keys.iter()
                .filter(|(n, _)| is_black(*n))
                .chain(keys.iter().filter(|(n, _)| !is_black(*n)))
                .find(|(_, r)| r.contains(p))
                .map(|&(n, r)| {
                    let depth = ((p.y - r.top()) / r.height()).clamp(0.0, 1.0);
                    (n, (30.0 + 97.0 * depth) as u8)
                })
        })
    } else {
        None
    };
    kb.mouse(live, under);

    let painter = ui.painter_at(rect);
    for black in [false, true] {
        for &(n, r) in keys.iter().filter(|(n, _)| is_black(*n) == black) {
            let held = live.is_held(n);
            let fill = match (held, black) {
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
            if let Some(c) = kb.letter_for(n) {
                let colour = if black || held {
                    Color32::from_gray(220)
                } else {
                    Color32::from_gray(60)
                };
                painter.text(
                    Pos2::new(r.center().x, r.bottom() - 9.0),
                    Align2::CENTER_CENTER,
                    c.to_ascii_uppercase(),
                    FontId::proportional(10.0),
                    colour,
                );
            }
        }
    }
}
