//! Poly Box, the app: the instrument's panel in its own window, with its own
//! sound out of the computer's default audio device.

mod audio;
mod controls;
mod keyboard;
mod panel;
mod settings;

use controls::Values;
use poly_box_engine::keys::KeyPresses;
use std::sync::Arc;
use taw_panel::frame::fit_to_window;
use taw_panel::theme::install_fonts;

/// Save at most this often while things are changing, in seconds.
const SAVE_EVERY: f64 = 2.0;

pub struct App {
    values: Values,
    shared: Arc<audio::Shared>,
    audio: Result<audio::Output, String>,
    saved: settings::Saved,
    keys: KeyPresses,
    /// Pitch bend (-1..1) and mod wheel (0..1) as the window's wheels have them.
    bend: f32,
    mod_wheel: f32,
    last_touched: Option<usize>,
    last_touched_at: f64,
    /// Seconds, from egui's clock.
    now: f64,
    meter_db: [f32; 2],
    dirty: bool,
    last_save: f64,
}

impl App {
    fn new() -> Self {
        let saved = settings::load();
        let values = saved.values();
        let shared = Arc::new(audio::Shared::new(&values));
        let audio = audio::start(shared.clone());
        if let Err(e) = &audio {
            eprintln!("Poly Box: {e}; the panel works but makes no sound");
            audio::start_silent(shared.clone());
        }
        let mut keys = KeyPresses::default();
        if let Some(base) = saved.keys_base {
            keys.base = base.clamp(
                poly_box_engine::keys::LOWEST_BASE,
                poly_box_engine::keys::HIGHEST_BASE,
            );
        }
        Self {
            values,
            shared,
            audio,
            saved,
            keys,
            bend: 0.0,
            mod_wheel: 0.0,
            last_touched: None,
            last_touched_at: 0.0,
            now: 0.0,
            meter_db: [-90.0; 2],
            dirty: false,
            last_save: 0.0,
        }
    }

    fn save(&mut self) {
        self.saved.set_values(&self.values);
        self.saved.keys_base = Some(self.keys.base);
        settings::save(&self.saved);
        self.dirty = false;
        self.last_save = self.now;
    }

    /// The computer keyboard: letters play, Z / X octave, C / V velocity, Esc all off.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        let live = &self.shared.live;
        let typing = ctx.egui_wants_keyboard_input();
        for event in ctx.input(|i| i.events.clone()) {
            match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if !typing && !modifiers.ctrl && !modifiers.command && !modifiers.alt => {
                    if key == egui::Key::Escape {
                        self.keys.release_all(live);
                    } else if let Some(c) = letter(key) {
                        let base = self.keys.base;
                        self.keys.letter_down(live, c);
                        if self.keys.base != base {
                            self.dirty = true;
                        }
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: false,
                    ..
                } => {
                    if let Some(c) = letter(key) {
                        self.keys.letter_up(live, c);
                    }
                }
                // Keys let go while the window was in the background never
                // arrive, so let go of everything when it loses focus.
                egui::Event::WindowFocused(false) => self.keys.release_all(live),
                _ => {}
            }
        }
    }

    fn update_meter(&mut self, dt: f32) {
        let peaks = self.shared.take_peaks();
        for (shown, peak) in self.meter_db.iter_mut().zip(peaks) {
            let db = 20.0 * peak.max(1e-6).log10();
            // Rise at once, fall at 24 dB a second.
            *shown = db.max(*shown - 24.0 * dt);
        }
    }
}

fn letter(key: egui::Key) -> Option<char> {
    let name = key.name();
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        fit_to_window(&ctx, panel::DESIGN, 3.0);
        let (now, dt) = ctx.input(|i| (i.time, i.stable_dt));
        self.now = now;
        self.handle_keys(&ctx);
        self.update_meter(dt);
        self.draw(ui);
        if self.dirty && self.now - self.last_save > SAVE_EVERY {
            self.save();
        }
        // Keep the lights and meter moving.
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.keys.release_all(&self.shared.live);
        self.save();
    }
}

fn main() -> eframe::Result {
    let scale = 0.85;
    eframe::run_native(
        "Poly Box",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Poly Box")
                .with_inner_size(panel::DESIGN * scale)
                .with_min_inner_size(panel::DESIGN * 0.4),
            ..Default::default()
        },
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}
