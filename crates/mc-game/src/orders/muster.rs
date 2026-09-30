//! A batching factory's forming units on the ground (`mc_sim::batch`). They are a group
//! like any other, under the same count badge (`orders.rs`); a ring round the badge fills
//! as the batch does. With the factory selected, a mark shows where the next one will
//! stand, and linked factories' batches are joined by a dashed line.

use super::{Field, Filling};
use crate::ui::{self, type_scale, Ui};
use glam::Vec2;
use mc_sim::mirror::BatchView;
use std::f32::consts::{FRAC_PI_2, TAU};

/// The colour of a batch, as on its switch.
const TONE: u32 = crate::hud::style::Family::Movement.tone();

/// Round a batch's badge (centre `c`, radius `r`, points): a ring that fills as the batch
/// does, and while it is lit or hovered, how full it is under it.
pub(super) fn filling_ring(
    ui: &mut Ui,
    c: Vec2,
    r: f32,
    filling: Filling,
    lit: bool,
    hovered: bool,
    time: f32,
) {
    let ring = r + if hovered { 5.5 } else { 3.5 };
    let strength = if hovered || lit { 1.0 } else { 0.7 };
    let full = (f32::from(filling.count) / f32::from(filling.size.max(1))).clamp(0.0, 1.0);
    ui.arc(c, ring, 0.0, TAU, 2.0, ui::rgb(TONE, 0.18 * strength));
    if full > 0.0 {
        let from = -FRAC_PI_2;
        ui.arc(
            c,
            ring,
            from,
            from + TAU * full,
            2.0,
            ui::rgb(TONE, 0.9 * strength),
        );
    }
    // Where the ring has got to breathes: the batch is still forming.
    if full < 1.0 {
        let a = -FRAC_PI_2 + TAU * full;
        let glow = 0.5 + 0.5 * (time * 2.4).sin();
        ui.disc(
            c + Vec2::from_angle(a) * ring,
            1.6 + glow,
            ui::rgb(TONE, (0.5 + 0.4 * glow) * strength),
        );
    }
    if lit || hovered {
        let text = if filling.fixed {
            format!("{}/{}", filling.count, filling.size)
        } else {
            format!("lap {}/{}", filling.count, filling.size)
        };
        let y = c.y + ring + 9.0;
        for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            ui.text_centred(
                c.x + d.x,
                y + d.y,
                type_scale::MICRO,
                ui::rgb(0x000000, 0.7),
                &text,
            );
        }
        ui.text_centred(c.x, y, type_scale::MICRO, ui::rgb(TONE, 1.0), &text);
    }
}

/// The selected factory's batch: where the next one out will stand, and a dashed line on
/// to the next linked factory's, so a linked batch reads as one.
pub(super) fn muster_marks(ui: &mut Ui, field: &Field, batch: &BatchView, strength: f32) {
    let scale = ui.s;
    let ground = |p: Vec2| {
        field
            .camera
            .project(p.extend(field.renderer.surface_height(p) + 1.0))
            .map(|q| q / scale)
    };
    // The next place, once some wait (with none, the badge stands on it): a small ring
    // that breathes.
    if let Some(next) = batch.next.filter(|_| !batch.units.is_empty()) {
        if let Some(p) = ground(Vec2::from(next)) {
            let breathe = 0.6 + 0.4 * (ui.time * 2.4).sin();
            ui.arc(
                p,
                5.0,
                0.0,
                TAU,
                1.3,
                ui::rgb(TONE, 0.6 * breathe * strength),
            );
            ui.disc(p, 1.5, ui::rgb(TONE, 0.8 * strength));
        }
    }
    if let (Some(&a), Some(&b)) = (
        batch.linked.get(batch.index),
        batch.linked.get(batch.index + 1),
    ) {
        let (a, b) = (Vec2::from(a), Vec2::from(b));
        let steps = ((a.distance(b) / 8.0) as usize).clamp(2, 400);
        for i in (0..steps).step_by(2) {
            let p = |k: usize| a.lerp(b, k as f32 / steps as f32);
            if let (Some(x), Some(y)) = (ground(p(i)), ground(p(i + 1))) {
                ui.stroke(x, y, 1.4, ui::rgb(TONE, 0.5 * strength));
            }
        }
    }
}
