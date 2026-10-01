//! Timed self-destructs (`mc_sim::destruct`) on the battlefield: a red ring round each
//! unit counting down, draining as the seconds run out and beating faster toward the
//! end, and a tag over it with the seconds left. Delete again calls it off.

use crate::nuke_marks::{project, surface, tag};
use crate::orders::Field;
use crate::ui::{palette, rgb, Ui};
use glam::{Vec2, Vec3};
use mc_sim::mirror::UNIT_STORED;

/// Points the whole ring is drawn with.
const RING_POINTS: usize = 64;

pub fn draw(ui: &mut Ui, field: &Field, alpha: f32) {
    let view = field.view;
    for d in &view.frame.destructs {
        let Some(u) = view.index_of.get(&d.unit_id).map(|&i| &view.frame.units[i]) else {
            continue;
        };
        if u.status[0] & UNIT_STORED != 0 || u.in_warp() {
            continue;
        }
        // The count drops a tick at a time; between ticks it runs on smoothly.
        let left = ((d.ticks_left as f32 - alpha) / 10.0).max(0.0);
        let share = (left / (d.length as f32 / 10.0)).clamp(0.0, 1.0);
        let pos = Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), alpha);
        let radius = (u.radius * 1.3).max(6.0);
        // Faster as it runs out: from about one beat a second to five.
        let beat = 0.6 + 0.4 * (ui.time * (3.0 + 13.0 * (1.0 - share))).sin().abs();
        let point = |i: usize| {
            // From the top, draining anticlockwise as seen from above.
            let a =
                std::f32::consts::FRAC_PI_2 + i as f32 / RING_POINTS as f32 * std::f32::consts::TAU;
            let p = pos.truncate() + Vec2::from_angle(a) * radius;
            // Ground units ring the ground under them, aircraft their own height.
            project(
                ui,
                field,
                p.extend((surface(field, p) + 1.5).max(pos.z + 0.5)),
            )
        };
        let ring: Vec<Option<Vec2>> = (0..=RING_POINTS).map(point).collect();
        let lit = (share * RING_POINTS as f32).ceil() as usize;
        for (i, w) in ring.windows(2).enumerate() {
            if let [Some(a), Some(b)] = *w {
                if i < lit {
                    ui.stroke(a, b, 3.0, rgb(palette::BAD, beat));
                } else {
                    ui.stroke(a, b, 1.5, rgb(palette::BAD, 0.25));
                }
            }
        }
        let over = pos + Vec3::Z * (u.radius * 0.8 + 6.0);
        if let Some(p) = project(ui, field, over) {
            let text = format!("SELF-DESTRUCT  {:.1} s", left);
            tag(ui, p - Vec2::new(0.0, 16.0), &text, palette::BAD);
        }
    }
}
