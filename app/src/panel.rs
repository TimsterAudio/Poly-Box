//! The front panel, laid out like the hardware: wooden end cheeks, the top
//! strip (badge, display, voice lights, meter, volume, switches), two rows of
//! sections, and the keyboard with its wheels.
//!
//! Everything is placed at fixed design coordinates and the whole panel is
//! scaled to fit the window, as on Drone Box: it never re-wraps.

use crate::controls::{Ctl, DEFS, DESTS, SOURCES, VOICE_MODES};
use crate::keyboard;
use crate::App;
use egui::{Align2, Color32, Id, Pos2, Rect, Stroke, Ui, Vec2};
use poly_box_engine::keys::{note_name, VoiceLight};
use poly_box_engine::VOICES;
use taw_panel::frame::draw_panel_bg;
use taw_panel::theme::*;
use taw_panel::widgets::*;

/// The panel's size before scaling to the window.
pub const DESIGN: Vec2 = Vec2::new(1720.0, 960.0);
/// The wooden end cheeks.
const CHEEK: f32 = 28.0;
/// The left and right edges of the sections.
const LEFT: f32 = 52.0;
const RIGHT: f32 = DESIGN.x - LEFT;
/// Rows of sections.
const ROW1: f32 = 150.0;
const ROW2: f32 = 410.0;
const ROW_H: f32 = 244.0;
const KEYS_TOP: f32 = 672.0;
const KEYS_H: f32 = 248.0;

/// Where a control's legend and readout go under it.
const FADER_H: f32 = 150.0;

pub struct Panel<'a> {
    pub ui: &'a mut Ui,
    pub th: Theme,
    pub origin: Pos2,
}

impl Panel<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x, y)
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.at(x, y), Vec2::new(w, h))
    }
}

/// Knob colour by what a control does (Drone Box's families).
fn family(c: Ctl) -> Option<Color32> {
    use Ctl::*;
    match c {
        Osc1Octave | Osc2Octave | Osc2Semitones | Osc2Fine | UnisonDetune | Glide | Vintage => {
            Some(FAM_PITCH)
        }
        Osc1Shape | Osc2Shape | Cutoff | Resonance | FilterMode | Drive | KeyTrack
        | FilterEnvAmount => Some(FAM_TONE),
        Lfo1Rate | Lfo1Shape | Lfo2Rate | Lfo2Shape | Mod1Amount | Mod2Amount | Mod3Amount
        | Mod4Amount | Mod5Amount | Mod6Amount => Some(FAM_MOD),
        _ => None,
    }
}

impl App {
    fn touched(&mut self, c: usize) {
        self.last_touched = Some(c);
        self.last_touched_at = self.now;
    }

    fn set(&mut self, c: usize, v: f32) {
        if self.values[c] != v {
            self.values[c] = v;
            self.shared.set(c, v);
            self.dirty = true;
        }
        self.touched(c);
    }

    /// A knob with its legend and readout under it.
    fn knob(&mut self, p: &mut Panel, centre: Pos2, c: Ctl, r: f32) {
        let i = c as usize;
        let d = &DEFS[i];
        let mut v = self.values[i];
        let resp = knob(
            p.ui,
            centre,
            r,
            Id::new(d.id),
            &mut v,
            &d.spec,
            family(c),
            &p.th,
        );
        if resp.changed() || resp.drag_started() {
            self.set(i, v);
        }
        legend(
            p.ui.painter(),
            centre + Vec2::new(0.0, r + 10.0),
            Align2::CENTER_TOP,
            d.label,
            &p.th,
        );
        readout(
            p.ui.painter(),
            centre + Vec2::new(0.0, r + 25.0),
            &d.show(self.values[i]),
            58.0,
        );
    }

    /// A fader whose top is at `top`, centred on `cx`, with legend and readout under it.
    fn fader(&mut self, p: &mut Panel, cx: f32, top: f32, c: Ctl) {
        let i = c as usize;
        let d = &DEFS[i];
        let mut v = self.values[i];
        let rect = Rect::from_min_size(Pos2::new(cx - 18.0, top), Vec2::new(36.0, FADER_H));
        let resp = fader(p.ui, rect, Id::new(d.id), &mut v, &d.spec, family(c), &p.th);
        if resp.changed() || resp.drag_started() {
            self.set(i, v);
        }
        legend(
            p.ui.painter(),
            Pos2::new(cx, top + FADER_H + 6.0),
            Align2::CENTER_TOP,
            d.label,
            &p.th,
        );
        readout(
            p.ui.painter(),
            Pos2::new(cx, top + FADER_H + 21.0),
            &d.show(self.values[i]),
            50.0,
        );
    }

    /// A single LED button for an on/off control.
    fn switch(&mut self, p: &mut Panel, centre: Pos2, c: Ctl, colour: KeyColour, label: &str) {
        let i = c as usize;
        let on = self.values[i] > 0.5;
        let rect = Rect::from_center_size(centre, Vec2::new(46.0, 22.0));
        if key(
            p.ui,
            rect,
            Id::new(DEFS[i].id),
            colour,
            Some(on),
            label,
            &p.th,
        )
        .clicked()
        {
            self.set(i, if on { 0.0 } else { 1.0 });
        }
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        let th = Theme::new(self.saved.panel, self.saved.knobs);
        let origin = ui.max_rect().min;
        draw_panel_bg(ui.painter(), ui.max_rect(), &th);
        let mut p = Panel { ui, th, origin };
        cheeks(&p);
        self.top_strip(&mut p);
        stripe(p.ui.painter(), p.rect(LEFT, 130.0, RIGHT - LEFT, 6.0));
        self.row_one(&mut p);
        self.row_two(&mut p);
        self.keyboard_section(&mut p);
        self.footer(&mut p);
    }

    fn top_strip(&mut self, p: &mut Panel) {
        let th = p.th;
        badge(p, p.at(LEFT, 16.0));
        slanted_text(
            p.ui,
            p.at(LEFT + 2.0, 46.0),
            "POLY BOX",
            font(F_LOGO, 32.0),
            th.legend,
            3.0,
            12.0,
        );
        text(
            p.ui.painter(),
            p.at(LEFT, 98.0),
            Align2::LEFT_TOP,
            &format!(
                "PB-5 · FIVE VOICE ANALOG-STYLE POLYSYNTH · V{}",
                env!("CARGO_PKG_VERSION")
            ),
            font(F_LEGEND, 10.0),
            th.legend_dim,
            2.4,
        );

        // The display: the last control touched, then the voice mode and the keys' octave.
        let mode = VOICE_MODES[self.values[Ctl::VoiceMode as usize].round() as usize];
        let first = match self.last_touched {
            Some(c) if self.now - self.last_touched_at < 4.0 => {
                (Ctl::long_label(c), DEFS[c].show(self.values[c]))
            }
            _ => ("Timster's Audio Works".into(), "Poly Box".into()),
        };
        let second = match &self.audio {
            Ok(_) => (
                format!("{mode} · 5 voices"),
                format!("Keys {}", note_name(self.keys.base)),
            ),
            Err(_) => ("No audio output".into(), "Check device".into()),
        };
        lcd(
            p.ui.painter(),
            p.rect(470.0, 26.0, 350.0, 52.0),
            &[first, second],
        );

        // Voice lights.
        let vx = 862.0;
        text(
            p.ui.painter(),
            p.at(vx + 80.0, 18.0),
            Align2::CENTER_TOP,
            "VOICES",
            font(F_LEGEND_BOLD, 10.5),
            th.legend,
            2.0,
        );
        for i in 0..VOICES {
            let c = p.at(vx + i as f32 * 40.0, 44.0);
            match self.shared.live.voice(i) {
                VoiceLight::Off => led(p.ui.painter(), c, false),
                VoiceLight::Held(_) => led(p.ui.painter(), c, true),
                VoiceLight::Releasing(_) => {
                    led(p.ui.painter(), c, false);
                    p.ui.painter()
                        .circle_filled(c, 3.0, LED.gamma_multiply(0.45));
                }
            }
            let label = match self.shared.live.voice(i) {
                VoiceLight::Off => format!("{}", i + 1),
                VoiceLight::Held(n) | VoiceLight::Releasing(n) => note_name(n),
            };
            readout(p.ui.painter(), c + Vec2::new(0.0, 10.0), &label, 34.0);
        }

        // Output meter.
        meter(
            p.ui.painter(),
            p.rect(1090.0, 34.0, 180.0, 22.0),
            self.meter_db,
        );
        text(
            p.ui.painter(),
            p.at(1180.0, 64.0),
            Align2::CENTER_TOP,
            "OUTPUT L / R",
            font(F_LEGEND, 10.0),
            th.legend,
            1.6,
        );

        self.knob(p, p.at(1322.0, 46.0), Ctl::Volume, 22.0);

        // Panel and knob switches.
        let switch = |p: &mut Panel, x: f32, label: &str, on: bool| -> bool {
            key(
                p.ui,
                p.rect(x, 40.0, 44.0, 22.0),
                Id::new(("switch", label)),
                K_GREY,
                Some(on),
                label,
                &p.th,
            )
            .clicked()
        };
        for (x, title) in [(1426.0, "PANEL"), (1552.0, "KNOBS")] {
            text(
                p.ui.painter(),
                p.at(x, 16.0),
                Align2::CENTER_TOP,
                title,
                font(F_LEGEND_BOLD, 10.0),
                th.legend_dim,
                2.0,
            );
        }
        if switch(p, 1376.0, "Black", self.saved.panel == PanelKind::Black) {
            self.saved.panel = PanelKind::Black;
            self.dirty = true;
        }
        if switch(p, 1428.0, "Silver", self.saved.panel == PanelKind::Silver) {
            self.saved.panel = PanelKind::Silver;
            self.dirty = true;
        }
        if switch(p, 1502.0, "Colour", self.saved.knobs == KnobStyle::Colour) {
            self.saved.knobs = KnobStyle::Colour;
            self.dirty = true;
        }
        if switch(p, 1554.0, "Pointer", self.saved.knobs == KnobStyle::Pointer) {
            self.saved.knobs = KnobStyle::Pointer;
            self.dirty = true;
        }
        if key(
            p.ui,
            p.rect(1614.0, 38.0, 54.0, 26.0),
            Id::new("all-off"),
            K_RED,
            Some(false),
            "All off",
            &p.th,
        )
        .clicked()
        {
            self.keys.release_all(&self.shared.live);
        }
    }

    fn row_one(&mut self, p: &mut Panel) {
        let th = p.th;
        let y = ROW1;
        let k1 = y + 64.0;
        let k2 = y + 166.0;
        let ftop = y + 30.0;

        // Oscillator 1.
        section(
            p.ui.painter(),
            p.rect(LEFT, y, 120.0, ROW_H),
            "Osc 1",
            &th,
            false,
        );
        self.knob(p, p.at(LEFT + 60.0, k1), Ctl::Osc1Octave, 20.0);
        self.knob(p, p.at(LEFT + 60.0, k2), Ctl::Osc1Shape, 20.0);

        // Oscillator 2.
        let x = 188.0;
        section(
            p.ui.painter(),
            p.rect(x, y, 250.0, ROW_H),
            "Osc 2",
            &th,
            false,
        );
        self.knob(p, p.at(x + 44.0, k1), Ctl::Osc2Octave, 20.0);
        self.knob(p, p.at(x + 125.0, k1), Ctl::Osc2Semitones, 20.0);
        self.knob(p, p.at(x + 206.0, k1), Ctl::Osc2Fine, 20.0);
        self.knob(p, p.at(x + 44.0, k2), Ctl::Osc2Shape, 20.0);
        self.switch(p, p.at(x + 125.0, k2 - 4.0), Ctl::Sync, K_ORANGE, "Sync");

        // Mixer.
        let x = 454.0;
        section(
            p.ui.painter(),
            p.rect(x, y, 186.0, ROW_H),
            "Mixer",
            &th,
            false,
        );
        self.fader(p, p.at(x + 37.0, 0.0).x, p.at(0.0, ftop).y, Ctl::Osc1Level);
        self.fader(p, p.at(x + 93.0, 0.0).x, p.at(0.0, ftop).y, Ctl::Osc2Level);
        self.fader(p, p.at(x + 149.0, 0.0).x, p.at(0.0, ftop).y, Ctl::Noise);

        // Filter.
        let x = 656.0;
        section(
            p.ui.painter(),
            p.rect(x, y, 380.0, ROW_H),
            "Filter",
            &th,
            false,
        );
        self.fader(p, p.at(x + 40.0, 0.0).x, p.at(0.0, ftop).y, Ctl::Cutoff);
        self.fader(p, p.at(x + 98.0, 0.0).x, p.at(0.0, ftop).y, Ctl::Resonance);
        self.knob(p, p.at(x + 170.0, k1), Ctl::FilterMode, 20.0);
        self.knob(p, p.at(x + 248.0, k1), Ctl::Drive, 20.0);
        self.knob(p, p.at(x + 326.0, k1), Ctl::KeyTrack, 20.0);
        self.knob(p, p.at(x + 170.0, k2), Ctl::FilterEnvAmount, 20.0);
        self.knob(p, p.at(x + 248.0, k2), Ctl::FilterVelocity, 20.0);
        self.switch(
            p,
            p.at(x + 326.0, k2 - 4.0),
            Ctl::Bandpass,
            K_ORANGE,
            "Band-pass",
        );

        // Envelopes.
        let x = 1052.0;
        section(
            p.ui.painter(),
            p.rect(x, y, 240.0, ROW_H),
            "Filter Env",
            &th,
            false,
        );
        for (i, c) in [
            Ctl::FilterAttack,
            Ctl::FilterDecay,
            Ctl::FilterSustain,
            Ctl::FilterRelease,
        ]
        .into_iter()
        .enumerate()
        {
            self.fader(
                p,
                p.at(x + 36.0 + 56.0 * i as f32, 0.0).x,
                p.at(0.0, ftop).y,
                c,
            );
        }
        let x = 1308.0;
        section(
            p.ui.painter(),
            p.rect(x, y, RIGHT - x, ROW_H),
            "Amp Env",
            &th,
            false,
        );
        for (i, c) in [
            Ctl::AmpAttack,
            Ctl::AmpDecay,
            Ctl::AmpSustain,
            Ctl::AmpRelease,
        ]
        .into_iter()
        .enumerate()
        {
            self.fader(
                p,
                p.at(x + 36.0 + 56.0 * i as f32, 0.0).x,
                p.at(0.0, ftop).y,
                c,
            );
        }
        self.knob(p, p.at(x + 300.0, k1), Ctl::AmpVelocity, 20.0);
    }

    fn row_two(&mut self, p: &mut Panel) {
        let th = p.th;
        let y = ROW2;
        let k1 = y + 64.0;
        let k2 = y + 166.0;
        let ftop = y + 30.0;

        // Voice: assignment keys over four knobs.
        section(
            p.ui.painter(),
            p.rect(LEFT, y, 380.0, ROW_H),
            "Voice",
            &th,
            false,
        );
        let mode = self.values[Ctl::VoiceMode as usize].round() as usize;
        for (i, name) in VOICE_MODES.iter().enumerate() {
            let r = p.rect(LEFT + 28.0 + 66.0 * i as f32, y + 44.0, 54.0, 24.0);
            if key(
                p.ui,
                r,
                Id::new(("mode", i)),
                K_YELLOW,
                Some(mode == i),
                name,
                &th,
            )
            .clicked()
            {
                self.set(Ctl::VoiceMode as usize, i as f32);
            }
        }
        for (i, c) in [Ctl::UnisonDetune, Ctl::Glide, Ctl::Vintage, Ctl::Spread]
            .into_iter()
            .enumerate()
        {
            self.knob(p, p.at(LEFT + 50.0 + 92.0 * i as f32, k2), c, 20.0);
        }

        // LFOs: a rate fader, a stepped wave knob and a light that blinks at the rate.
        for (n, x, rate, wave) in [
            (1, 448.0, Ctl::Lfo1Rate, Ctl::Lfo1Shape),
            (2, 614.0, Ctl::Lfo2Rate, Ctl::Lfo2Shape),
        ] {
            section(
                p.ui.painter(),
                p.rect(x, y, 150.0, ROW_H),
                &format!("LFO {n}"),
                &th,
                false,
            );
            self.fader(p, p.at(x + 40.0, 0.0).x, p.at(0.0, ftop).y, rate);
            self.knob(p, p.at(x + 106.0, k1), wave, 20.0);
            let hz = self.values[rate as usize];
            let phase = (self.now * hz as f64).fract();
            let c = p.at(x + 106.0, k2 - 10.0);
            led(p.ui.painter(), c, phase < 0.5);
        }

        // Aux envelope.
        let x = 780.0;
        section(
            p.ui.painter(),
            p.rect(x, y, 240.0, ROW_H),
            "Aux Env",
            &th,
            false,
        );
        for (i, c) in [
            Ctl::AuxAttack,
            Ctl::AuxDecay,
            Ctl::AuxSustain,
            Ctl::AuxRelease,
        ]
        .into_iter()
        .enumerate()
        {
            self.fader(
                p,
                p.at(x + 36.0 + 56.0 * i as f32, 0.0).x,
                p.at(0.0, ftop).y,
                c,
            );
        }

        self.mod_matrix(p, 1036.0, y);
    }

    fn mod_matrix(&mut self, p: &mut Panel, x: f32, y: f32) {
        let th = p.th;
        section(
            p.ui.painter(),
            p.rect(x, y, RIGHT - x, ROW_H),
            "Mod Matrix",
            &th,
            false,
        );
        let cols = [
            (x + 52.0, "Source"),
            (x + 262.0, "Destination"),
            (x + 492.0, "Amount"),
        ];
        for (cx, label) in cols {
            text(
                p.ui.painter(),
                p.at(cx, y + 20.0),
                Align2::LEFT_TOP,
                &label.to_uppercase(),
                font(F_LEGEND, 10.0),
                th.legend_dim,
                1.8,
            );
        }
        let sources: Vec<String> = SOURCES.iter().map(|s| s.to_string()).collect();
        let dests: Vec<String> = DESTS.iter().map(|s| s.to_string()).collect();
        for slot in 0..6 {
            let [s, d, a] = Ctl::mod_slot(slot);
            let ry = y + 50.0 + 32.0 * slot as f32;
            let active = self.values[s] >= 1.0 && self.values[d] >= 1.0 && self.values[a] != 0.0;
            led(p.ui.painter(), p.at(x + 20.0, ry), active);
            text(
                p.ui.painter(),
                p.at(x + 34.0, ry),
                Align2::LEFT_CENTER,
                &format!("{}", slot + 1),
                font(F_LEGEND_BOLD, 12.0),
                th.legend,
                0.0,
            );
            let values = self.values;
            let combo = |p: &mut Panel, cx: f32, w: f32, i: usize, items: &[String]| {
                lcd_combo(
                    p.ui,
                    p.rect(cx, ry - 11.0, w, 22.0),
                    DEFS[i].id,
                    items,
                    values[i].round() as usize,
                )
            };
            if let Some(v) = combo(p, x + 52.0, 190.0, s, &sources) {
                self.set(s, v as f32);
            }
            arrow(p.ui.painter(), p.at(x + 251.0, ry), th.legend_dim);
            if let Some(v) = combo(p, x + 262.0, 200.0, d, &dests) {
                self.set(d, v as f32);
            }
            // A small amount knob with its readout beside it.
            let c = Ctl::Mod1Amount as usize + 3 * slot;
            let def = &DEFS[c];
            let mut v = self.values[c];
            let kc = p.at(x + 506.0, ry);
            let resp = knob(
                p.ui,
                kc,
                11.0,
                Id::new(def.id),
                &mut v,
                &def.spec,
                Some(FAM_MOD),
                &th,
            );
            if resp.changed() || resp.drag_started() {
                self.set(c, v);
            }
            readout(
                p.ui.painter(),
                kc + Vec2::new(66.0, -8.0),
                &def.show(self.values[c]),
                62.0,
            );
        }
    }

    fn keyboard_section(&mut self, p: &mut Panel) {
        let th = p.th;
        let y = KEYS_TOP;
        section(
            p.ui.painter(),
            p.rect(LEFT, y, RIGHT - LEFT, KEYS_H),
            "Keyboard",
            &th,
            false,
        );

        // The wheels.
        let slot_h = 160.0;
        let wy = y + 30.0;
        let bend = keyboard::wheel(
            p.ui,
            p.rect(LEFT + 26.0, wy, 34.0, slot_h),
            "pitch-wheel",
            self.bend,
            true,
        );
        if bend != self.bend {
            self.bend = bend;
            self.shared.live.set_bend(bend);
        }
        let wheel = keyboard::wheel(
            p.ui,
            p.rect(LEFT + 82.0, wy, 34.0, slot_h),
            "mod-wheel",
            self.mod_wheel,
            false,
        );
        if wheel != self.mod_wheel {
            self.mod_wheel = wheel;
            self.shared.live.set_mod_wheel(wheel);
        }
        for (cx, label) in [(LEFT + 43.0, "Pitch"), (LEFT + 99.0, "Mod")] {
            legend(
                p.ui.painter(),
                p.at(cx, wy + slot_h + 8.0),
                Align2::CENTER_TOP,
                label,
                &th,
            );
        }

        // Octave keys for the computer keyboard.
        let ox = LEFT + 140.0;
        if key(
            p.ui,
            p.rect(ox, wy + 8.0, 50.0, 24.0),
            Id::new("oct-down"),
            K_CREAM,
            None,
            "Oct −",
            &th,
        )
        .clicked()
        {
            self.keys.octave_down();
            self.dirty = true;
        }
        if key(
            p.ui,
            p.rect(ox, wy + 62.0, 50.0, 24.0),
            Id::new("oct-up"),
            K_CREAM,
            None,
            "Oct +",
            &th,
        )
        .clicked()
        {
            self.keys.octave_up();
            self.dirty = true;
        }
        legend(
            p.ui.painter(),
            p.at(ox + 25.0, wy + 118.0),
            Align2::CENTER_TOP,
            "Keys",
            &th,
        );
        readout(
            p.ui.painter(),
            p.at(ox + 25.0, wy + 134.0),
            &note_name(self.keys.base),
            50.0,
        );

        let keys = p.rect(LEFT + 214.0, y + 22.0, RIGHT - LEFT - 232.0, KEYS_H - 36.0);
        keyboard::keys(p.ui, keys, &self.shared.live, &mut self.keys);
    }

    fn footer(&mut self, p: &mut Panel) {
        let th = p.th;
        let y = KEYS_TOP + KEYS_H + 14.0;
        text(
            p.ui.painter(),
            p.at(LEFT, y),
            Align2::LEFT_TOP,
            "Play with A W S E D F T G Y H U J K O L P  ·  Z / X octave  ·  C / V velocity  ·  Esc all off  ·  drag knobs and faders (Shift for fine)  ·  double-click resets",
            font(F_LEGEND, 11.0),
            th.legend_dim,
            0.6,
        );
        let status = match &self.audio {
            Ok(o) => format!(
                "Audio: {}  ·  velocity {}",
                o.description, self.keys.velocity
            ),
            Err(_) => "Audio: no output device, so no sound (the panel still plays)".into(),
        };
        text(
            p.ui.painter(),
            p.at(RIGHT, y),
            Align2::RIGHT_TOP,
            &status,
            font(F_LEGEND, 11.0),
            th.legend_dim,
            0.6,
        );
    }
}

/// A small right-pointing arrow: source to destination.
fn arrow(p: &egui::Painter, c: Pos2, colour: Color32) {
    let s = Stroke::new(1.4, colour);
    p.line_segment([c - Vec2::new(6.0, 0.0), c + Vec2::new(5.0, 0.0)], s);
    p.line_segment([c + Vec2::new(5.0, 0.0), c + Vec2::new(1.0, -4.0)], s);
    p.line_segment([c + Vec2::new(5.0, 0.0), c + Vec2::new(1.0, 4.0)], s);
}

/// The maker's badge: a thin frame round "Timster's Audio Works - Poly Box".
fn badge(p: &Panel, at: Pos2) {
    let painter = p.ui.painter();
    let maker = "Timster's Audio Works - ";
    let name = "Poly Box";
    let f1 = font(F_MAKER, 13.0);
    let f2 = font(F_MAKER_BOLD, 13.0);
    let w1 = painter
        .layout_no_wrap(maker.into(), f1.clone(), Color32::WHITE)
        .size()
        .x;
    let w2 = painter
        .layout_no_wrap(name.into(), f2.clone(), Color32::WHITE)
        .size()
        .x;
    let r = Rect::from_min_size(at, Vec2::new(w1 + w2 + 22.0, 24.0));
    painter.rect_stroke(r, 3, Stroke::new(1.0, p.th.frame), egui::StrokeKind::Inside);
    painter.text(
        at + Vec2::new(11.0, 12.0),
        Align2::LEFT_CENTER,
        maker,
        f1,
        p.th.legend_dim,
    );
    painter.text(
        at + Vec2::new(11.0 + w1, 12.0),
        Align2::LEFT_CENTER,
        name,
        f2,
        p.th.legend,
    );
}

/// Wooden end cheeks, as on a hardware synth.
fn cheeks(p: &Panel) {
    let painter = p.ui.painter();
    for x in [0.0, DESIGN.x - CHEEK] {
        let r = p.rect(x, 0.0, CHEEK, DESIGN.y);
        painter.rect_filled(r, 0, Color32::from_rgb(0x4a, 0x2a, 0x14));
        // Grain: long, gently wandering lines in lighter and darker browns.
        let mut seed: u32 = if x == 0.0 { 0x2545_f491 } else { 0x9e37_79b9 };
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        for _ in 0..26 {
            let gx = r.left() + 1.0 + rnd() * (CHEEK - 2.0);
            let light = rnd() > 0.5;
            let colour = if light {
                Color32::from_rgba_unmultiplied(0xa0, 0x6a, 0x3a, 40 + (rnd() * 50.0) as u8)
            } else {
                Color32::from_rgba_unmultiplied(0x1e, 0x0e, 0x05, 50 + (rnd() * 60.0) as u8)
            };
            let wobble = 0.6 + rnd() * 1.4;
            let freq = 0.004 + rnd() * 0.01;
            let phase = rnd() * std::f32::consts::TAU;
            let pts: Vec<Pos2> = (0..=48)
                .map(|k| {
                    let yy = r.top() + r.height() * k as f32 / 48.0;
                    Pos2::new(
                        (gx + wobble * (yy * freq + phase).sin()).clamp(r.left(), r.right()),
                        yy,
                    )
                })
                .collect();
            painter.add(egui::Shape::line(pts, Stroke::new(0.5 + rnd(), colour)));
        }
        // Rounded light on the outer edge, shadow where it meets the panel.
        let (outer, inner) = if x == 0.0 {
            (r.left(), r.right())
        } else {
            (r.right(), r.left())
        };
        painter.line_segment(
            [Pos2::new(outer, r.top()), Pos2::new(outer, r.bottom())],
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(0xff, 0xd0, 0xa0, 28)),
        );
        painter.line_segment(
            [Pos2::new(inner, r.top()), Pos2::new(inner, r.bottom())],
            Stroke::new(2.0, Color32::from_black_alpha(160)),
        );
    }
}
