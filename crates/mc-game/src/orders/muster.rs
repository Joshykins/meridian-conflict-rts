//! A batching factory's muster block on the ground (`mc_sim::batch`): a pad for each
//! place its batch takes, lit as a unit stands in it, corner brackets round the block,
//! and how many are ready over it.

use super::Field;
use crate::ui::{self, type_scale, Ui};
use glam::Vec2;
use mc_sim::mirror::BatchView;

/// The colour of a batch, as on its switch.
const TONE: u32 = crate::hud::style::Family::Movement.tone();

/// Draws `batch`'s block, at `strength` (one for a selected factory).
pub(super) fn muster_block(ui: &mut Ui, field: &Field, batch: &BatchView, strength: f32) {
    if batch.places.is_empty() {
        return;
    }
    let ahead = Vec2::from(batch.facing);
    let across = ahead.perp();
    let scale = ui.s;
    let ground = |p: Vec2| {
        field
            .camera
            .project(p.extend(field.renderer.surface_height(p) + 1.0))
            .map(|q| q / scale)
    };
    let pad = batch.spacing * 0.32;
    let breathe = 0.75 + 0.25 * (ui.time * 2.2).sin();
    for &(p, here) in &batch.places {
        let c = Vec2::from(p);
        let corners = [
            c + (ahead + across) * pad,
            c + (ahead - across) * pad,
            c - (ahead + across) * pad,
            c - (ahead - across) * pad,
        ];
        let Some(q) = corners
            .iter()
            .map(|&k| ground(k))
            .collect::<Option<Vec<Vec2>>>()
        else {
            continue;
        };
        if here {
            ui.triangle(q[0], q[1], q[2], ui::rgb(TONE, 0.28 * strength));
            ui.triangle(q[0], q[2], q[3], ui::rgb(TONE, 0.28 * strength));
            ui.polyline(&q, 1.6, ui::rgb(TONE, 0.95 * strength), true);
        } else {
            // Still to fill: a faint outline that breathes while the batch forms.
            ui.polyline(&q, 1.1, ui::rgb(TONE, 0.45 * breathe * strength), true);
        }
    }

    // Corner brackets round the whole block, in its own frame.
    let local = |p: [f32; 2]| {
        let v = Vec2::from(p);
        Vec2::new(v.dot(ahead), v.dot(across))
    };
    let (lo, hi) = batch.places.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), &(p, _)| (lo.min(local(p)), hi.max(local(p))),
    );
    let margin = batch.spacing * 0.5 + 3.0;
    let (lo, hi) = (lo - Vec2::splat(margin), hi + Vec2::splat(margin));
    let world = |l: Vec2| ahead * l.x + across * l.y;
    let arm = (batch.spacing * 0.9).min((hi - lo).min_element() * 0.4);
    let color = ui::rgb(TONE, 0.8 * strength);
    for (corner, dx, dy) in [
        (Vec2::new(lo.x, lo.y), 1.0, 1.0),
        (Vec2::new(lo.x, hi.y), 1.0, -1.0),
        (Vec2::new(hi.x, lo.y), -1.0, 1.0),
        (Vec2::new(hi.x, hi.y), -1.0, -1.0),
    ] {
        let points: Option<Vec<Vec2>> = [
            corner + Vec2::new(dx * arm, 0.0),
            corner,
            corner + Vec2::new(0.0, dy * arm),
        ]
        .iter()
        .map(|&l| ground(world(l)))
        .collect();
        if let Some(points) = points {
            ui.polyline(&points, 1.8, color, false);
        }
    }

    // How many are ready, over the far edge of the block.
    let far = world(Vec2::new(hi.x, (lo.y + hi.y) * 0.5));
    if let Some(at) = ground(far) {
        ui.text_centred(
            at.x,
            at.y - 12.0,
            type_scale::MICRO,
            ui::rgb(TONE, strength),
            &format!("BATCH  {}/{}", batch.made, batch.size),
        );
    }
}
