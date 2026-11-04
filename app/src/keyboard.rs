//! The keyboard and the pitch and mod wheels, drawn as hardware.

use egui::epaint::Mesh;
use egui::{Align2, Color32, Id, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use poly_box_engine::keys::{is_black, note_name, KeyPresses, Live};
use taw_panel::theme::{font, F_LEGEND, F_LEGEND_BOLD, LED};

/// The keyboard runs from C2 to C6: 49 keys.
const LOWEST: u8 = 36;
const HIGHEST: u8 = 84;

/// A rectangle shaded from `top` to `bottom`.
fn gradient(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

/// The keys and where each sits, black keys after white ones (they're on top).
fn layout(rect: Rect) -> Vec<(u8, Rect)> {
    let whites = (LOWEST..=HIGHEST).filter(|&n| !is_black(n)).count() as f32;
    let w = rect.width() / whites;
    let mut out = Vec::new();
    let mut x = rect.left();
    for n in LOWEST..=HIGHEST {
        if !is_black(n) {
            out.push((
                n,
                Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height())),
            ));
            x += w;
        }
    }
    let mut x = rect.left();
    for n in LOWEST..=HIGHEST {
        if is_black(n) {
            let bw = w * 0.58;
            // Nudge the black keys the way real ones sit, off-centre within their groups.
            let nudge = match n % 12 {
                1 | 6 => -0.08,
                3 | 10 => 0.08,
                _ => 0.0,
            } * w;
            out.push((
                n,
                Rect::from_min_size(
                    Pos2::new(x - bw / 2.0 + nudge, rect.top()),
                    Vec2::new(bw, rect.height() * 0.62),
                ),
            ));
        } else {
            x += w;
        }
    }
    out
}

pub fn keys(ui: &mut Ui, rect: Rect, live: &Live, kb: &mut KeyPresses) {
    let resp = ui.interact(rect, Id::new("keyboard"), Sense::click_and_drag());
    let keys = layout(rect);

    // Mouse: press, drag across keys, let go. Lower on a key is louder.
    let under = if resp.is_pointer_button_down_on() {
        resp.interact_pointer_pos().and_then(|p| {
            keys.iter()
                .rev() // black keys first
                .find(|(_, r)| r.contains(p))
                .map(|&(n, r)| {
                    let depth = ((p.y - r.top()) / r.height()).clamp(0.0, 1.0);
                    (n, (40.0 + 87.0 * depth) as u8)
                })
        })
    } else {
        None
    };
    kb.mouse(live, under);

    let p = ui.painter();
    // The key bed and the red felt along the back.
    p.rect_filled(rect.expand(4.0), 3, Color32::from_rgb(8, 8, 8));
    p.rect_filled(
        Rect::from_min_size(rect.min - Vec2::new(0.0, 3.0), Vec2::new(rect.width(), 5.0)),
        0,
        Color32::from_rgb(0x7a, 0x14, 0x12),
    );

    for &(n, r) in &keys {
        let held = live.is_held(n);
        if is_black(n) {
            // Black keys: a lit top face and a darker front.
            let r = if held {
                r.translate(Vec2::new(0.0, 1.5))
            } else {
                r
            };
            p.rect_filled(
                r.expand(1.0).translate(Vec2::new(1.5, 2.0)),
                2,
                Color32::from_black_alpha(140),
            );
            p.rect_filled(r, 2, Color32::from_rgb(10, 10, 10));
            let top = Rect::from_min_max(
                r.min + Vec2::new(2.0, 0.0),
                Pos2::new(r.right() - 2.0, r.bottom() - 9.0),
            );
            let (a, b) = if held {
                (
                    Color32::from_rgb(0x30, 0x14, 0x12),
                    Color32::from_rgb(0x52, 0x1c, 0x16),
                )
            } else {
                (Color32::from_rgb(22, 22, 22), Color32::from_rgb(52, 52, 52))
            };
            gradient(p, top, a, b);
            if held {
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(top.left(), top.bottom() - 3.0),
                        Vec2::new(top.width(), 3.0),
                    ),
                    0,
                    LED.gamma_multiply(0.8),
                );
            }
            if let Some(c) = kb.letter_for(n) {
                p.text(
                    Pos2::new(r.center().x, r.bottom() - 18.0),
                    Align2::CENTER_CENTER,
                    c.to_ascii_uppercase(),
                    font(F_LEGEND_BOLD, 11.0),
                    Color32::from_gray(150),
                );
            }
        } else {
            // White keys: ivory, shaded down to a lip at the front.
            let lip = if held { 4.0 } else { 9.0 };
            let face = Rect::from_min_max(
                r.min + Vec2::new(1.0, 0.0),
                Pos2::new(r.right() - 1.0, r.bottom() - lip),
            );
            let (a, b) = if held {
                (
                    Color32::from_rgb(0xd8, 0xc8, 0xbc),
                    Color32::from_rgb(0xe8, 0xb8, 0xa8),
                )
            } else {
                (
                    Color32::from_rgb(0xe6, 0xe1, 0xd4),
                    Color32::from_rgb(0xf6, 0xf3, 0xea),
                )
            };
            gradient(p, face, a, b);
            let front = Rect::from_min_max(
                Pos2::new(face.left(), face.bottom()),
                Pos2::new(face.right(), r.bottom()),
            );
            gradient(
                p,
                front,
                Color32::from_rgb(0xc9, 0xc3, 0xb5),
                Color32::from_rgb(0x9a, 0x94, 0x88),
            );
            if held {
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(face.left(), face.bottom() - 3.0),
                        Vec2::new(face.width(), 3.0),
                    ),
                    0,
                    LED.gamma_multiply(0.8),
                );
            }
            p.line_segment(
                [
                    Pos2::new(r.right(), r.top()),
                    Pos2::new(r.right(), r.bottom()),
                ],
                Stroke::new(1.0, Color32::from_rgb(0x5a, 0x56, 0x4e)),
            );
            if let Some(c) = kb.letter_for(n) {
                p.text(
                    Pos2::new(r.center().x, r.bottom() - 24.0),
                    Align2::CENTER_CENTER,
                    c.to_ascii_uppercase(),
                    font(F_LEGEND_BOLD, 12.0),
                    Color32::from_rgb(0x8a, 0x84, 0x78),
                );
            }
            if n % 12 == 0 {
                p.text(
                    Pos2::new(r.center().x, r.bottom() - 42.0),
                    Align2::CENTER_CENTER,
                    note_name(n),
                    font(F_LEGEND, 10.0),
                    Color32::from_rgb(0xa8, 0xa2, 0x96),
                );
            }
        }
    }
    // Shadow from the panel's lip over the back of the keys.
    gradient(
        p,
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), 10.0)),
        Color32::from_black_alpha(150),
        Color32::TRANSPARENT,
    );
}

/// A wheel in a slot. `centred` wheels (pitch bend) run -1..1 and spring back
/// to 0 when let go; the others (mod) run 0..1 and stay put.
pub fn wheel(ui: &mut Ui, slot: Rect, id: &str, value: f32, centred: bool) -> f32 {
    let resp = ui.interact(slot, Id::new(id), Sense::click_and_drag());
    let travel = slot.height() * 0.8;
    let mut v = value;
    if resp.dragged() {
        let dy = resp.drag_delta().y;
        let span = if centred { 2.0 } else { 1.0 };
        v -= dy / travel * span;
    }
    if centred && !resp.dragged() {
        v = 0.0;
    }
    if !centred && resp.double_clicked() {
        v = 0.0;
    }
    let v = if centred {
        v.clamp(-1.0, 1.0)
    } else {
        v.clamp(0.0, 1.0)
    };

    let p = ui.painter();
    // The slot.
    p.rect_filled(slot.expand(3.0), 4, Color32::from_rgb(6, 6, 6));
    p.rect_stroke(
        slot.expand(3.0),
        4,
        Stroke::new(1.0, Color32::from_gray(60)),
        egui::StrokeKind::Inside,
    );
    // The wheel: a ribbed cylinder, dark at its ends, lit across the middle.
    let body = slot.shrink2(Vec2::new(4.0, 2.0));
    let mid = body.center().y;
    let half = body.height() / 2.0;
    let shade = |y: f32| -> Color32 {
        let t = ((y - mid) / half).clamp(-1.0, 1.0);
        let light = (1.0 - t * t).sqrt();
        Color32::from_gray((12.0 + 52.0 * light) as u8)
    };
    let steps = 24;
    for k in 0..steps {
        let y0 = body.top() + body.height() * k as f32 / steps as f32;
        let y1 = body.top() + body.height() * (k + 1) as f32 / steps as f32;
        let r = Rect::from_min_max(Pos2::new(body.left(), y0), Pos2::new(body.right(), y1));
        gradient(p, r, shade(y0), shade(y1));
    }
    // Ribs roll with the value.
    let roll = if centred { v } else { v * 2.0 - 1.0 } * travel / 2.0;
    let pitch = 7.0;
    let offset = (-roll).rem_euclid(pitch);
    let mut y = body.top() + offset;
    while y < body.bottom() {
        let t = ((y - mid) / half).clamp(-1.0, 1.0);
        let a = ((1.0 - t * t).sqrt() * 120.0) as u8;
        p.line_segment(
            [
                Pos2::new(body.left() + 1.0, y),
                Pos2::new(body.right() - 1.0, y),
            ],
            Stroke::new(1.0, Color32::from_black_alpha(a)),
        );
        y += pitch;
    }
    // The marker line: the wheel's position.
    let marker = mid - roll * 0.9;
    if (body.top()..body.bottom()).contains(&marker) {
        p.line_segment(
            [
                Pos2::new(body.left() + 2.0, marker),
                Pos2::new(body.right() - 2.0, marker),
            ],
            Stroke::new(2.0, Color32::from_gray(225)),
        );
    }
    v
}
