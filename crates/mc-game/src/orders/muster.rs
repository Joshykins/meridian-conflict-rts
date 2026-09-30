//! Factory batches on the ground (`mc_sim::batch`). The factories of a batch wear blue
//! corner brackets round their lots, joined by blue lines along the ground, so a linked
//! batch reads as one thing. The units waiting at each factory are a group like any other,
//! under the same count badge (`orders.rs`), with a ring round it filling as the batch
//! does. A batch has one set of orders, and its order line leaves once, from the waiting
//! group nearest where it goes. With a factory selected, a mark shows where its next unit
//! will stand.

use super::{half_footprint, Field, Filling, Group};
use crate::ui::{self, type_scale, Ui};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use mc_sim::mirror::BatchView;
use std::f32::consts::{FRAC_PI_2, TAU};

/// The colour of a batch, as on its switch.
const TONE: u32 = crate::hud::style::Family::Movement.tone();

/// Most factories of one batch joined by lines: a deliberate cap on drawing, far past any
/// batch a player links by hand.
const MAX_LINKED: usize = 32;

/// A set of units' middle, and those of them seen.
type Seen = (Vec3, Vec<u32>);

/// Each batch forming at a factory, as a group: its waiting units and how full the batch
/// is. With none waiting yet, an empty badge stands where the first will, while the
/// factory is selected. `middle_of` finds a set of units' middle and those of them seen.
pub(super) fn batch_groups(field: &Field, middle_of: &dyn Fn(&[u32]) -> Seen) -> Vec<Group> {
    let view = field.view;
    view.status
        .queues
        .iter()
        .filter_map(|q| {
            let b = q.batch.as_ref()?;
            let (middle, members) = middle_of(&b.units);
            let factory_lit = view.shift || view.selection.contains(&q.unit_id);
            let middle = if members.is_empty() {
                let at = Vec2::from(b.next.filter(|_| factory_lit)?);
                at.extend(field.renderer.surface_height(at))
            } else {
                middle
            };
            Some(Group {
                formation: 0,
                middle,
                selected: factory_lit || members.iter().any(|id| view.selection.contains(id)),
                members,
                batch: Some(Filling {
                    group: b.group,
                    count: b.count,
                    size: b.size,
                }),
            })
        })
        .collect()
}

/// Where batch `group`'s order line leaves from, going first to `to`: the middle of the
/// waiting units nearest it, or with none waiting, the factory of the batch nearest it.
pub(super) fn exit(field: &Field, groups: &[Group], batch: &BatchView, to: Vec2) -> Option<Vec3> {
    let far = |p: &Vec3| (p.truncate() - to).length_squared();
    let waiting = groups
        .iter()
        .filter(|g| !g.members.is_empty() && g.batch.is_some_and(|b| b.group == batch.group))
        .map(|g| g.middle)
        .min_by(|a, b| far(a).total_cmp(&far(b)));
    waiting.or_else(|| {
        let view = field.view;
        batch
            .linked
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .map(|&i| Vec3::from(view.frame.units[i].pos))
            .min_by(|a, b| far(a).total_cmp(&far(b)))
    })
}

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
        let text = format!("{}/{}", filling.count, filling.size);
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

/// Every batch of the side on the ground: brackets round its factories' lots and lines
/// joining them, faint, lit while any of it is selected (or Shift is held); and the
/// selected factories' next places.
pub(super) fn links(ui: &mut Ui, field: &Field) {
    let view = field.view;
    let mut drawn: Vec<u32> = Vec::new();
    for q in &view.status.queues {
        let Some(b) = q.batch.as_ref() else {
            continue;
        };
        if view.selection.contains(&q.unit_id) {
            next_place(ui, field, b);
        }
        if drawn.contains(&b.group) {
            continue;
        }
        drawn.push(b.group);
        // Each factory's lot: its middle and half its size.
        let lots: Vec<(Vec2, Vec2)> = b
            .linked
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .map(|&i| &view.frame.units[i])
            .take(MAX_LINKED)
            .map(|u| {
                let half = half_footprint(field.blueprints, BlueprintId(u.blueprint as u16));
                (Vec2::new(u.pos[0], u.pos[1]), half)
            })
            .collect();
        let lit = view.shift
            || b.linked.iter().any(|id| view.selection.contains(id))
            || view
                .status
                .queues
                .iter()
                .filter_map(|o| o.batch.as_ref().filter(|o| o.group == b.group))
                .any(|o| o.units.iter().any(|u| view.selection.contains(u)));
        let strength = if lit { 1.0 } else { 0.35 };
        for &(c, half) in &lots {
            brackets(ui, field, c, half, strength);
        }
        for (a, z) in spanning(&lots) {
            let (a, z) = (lots[a], lots[z]);
            let d = z.0 - a.0;
            let (from, to) = (a.0 + d * lot_exit(a.1, d), z.0 - d * lot_exit(z.1, -d));
            ground_line(ui, field, &[from, to], strength);
        }
    }
}

/// How far along `d` from a lot's middle its edge (half size `half`) is, as a share of `d`,
/// with a little room so the line starts clear of the brackets.
fn lot_exit(half: Vec2, d: Vec2) -> f32 {
    let t = (half.x / d.x.abs().max(1e-3)).min(half.y / d.y.abs().max(1e-3));
    (t + 6.0 / d.length().max(1.0)).min(0.5)
}

/// The shortest set of lines joining every lot (Prim's, on distance): pairs of indices.
fn spanning(lots: &[(Vec2, Vec2)]) -> Vec<(usize, usize)> {
    let mut joined = vec![false; lots.len()];
    let mut out = Vec::new();
    if lots.is_empty() {
        return out;
    }
    joined[0] = true;
    for _ in 1..lots.len() {
        let best = (0..lots.len())
            .filter(|&i| joined[i])
            .flat_map(|i| (0..lots.len()).filter(|&j| !joined[j]).map(move |j| (i, j)))
            .min_by(|&(a, b), &(c, d)| {
                lots[a]
                    .0
                    .distance_squared(lots[b].0)
                    .total_cmp(&lots[c].0.distance_squared(lots[d].0))
            });
        let Some((i, j)) = best else {
            break;
        };
        joined[j] = true;
        out.push((i, j));
    }
    out
}

/// Corner brackets round a lot (middle `c`, half size `half`, metres), following the ground.
fn brackets(ui: &mut Ui, field: &Field, c: Vec2, half: Vec2, strength: f32) {
    let half = half + Vec2::splat(3.0);
    let arm = half.min_element() * 0.45;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = c + Vec2::new(sx * half.x, sy * half.y);
        ground_line(
            ui,
            field,
            &[
                corner - Vec2::new(sx * arm, 0.0),
                corner,
                corner - Vec2::new(0.0, sy * arm),
            ],
            strength,
        );
    }
}

/// A blue line laid along the ground through `points` (metres): a soft glow under a fine
/// core, like a selection's.
fn ground_line(ui: &mut Ui, field: &Field, points: &[Vec2], strength: f32) {
    let scale = ui.s;
    let project = |p: Vec2| {
        field
            .camera
            .project(p.extend(field.renderer.surface_height(p) + 1.0))
            .filter(|q| q.abs().max_element() < 20_000.0)
            .map(|q| q / scale)
    };
    // Sampled every few metres, so the line lies on the ground over rises and dips.
    let mut run: Vec<Vec2> = Vec::new();
    let mut runs: Vec<Vec<Vec2>> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = ((a.distance(b) / 6.0) as usize).clamp(1, 600);
        for i in 0..=steps {
            if i == 0 && !run.is_empty() {
                continue;
            }
            match project(a.lerp(b, i as f32 / steps as f32)) {
                Some(p) => run.push(p),
                None if run.len() > 1 => runs.push(std::mem::take(&mut run)),
                None => run.clear(),
            }
        }
    }
    if run.len() > 1 {
        runs.push(run);
    }
    for run in runs {
        ui.polyline(&run, 5.0, ui::rgb(TONE, 0.14 * strength), false);
        ui.polyline(&run, 1.6, ui::rgb(TONE, 0.85 * strength), false);
    }
}

/// Where the selected factory's next unit out will stand, once some wait (with none, the
/// empty badge stands on it): a small ring that breathes.
fn next_place(ui: &mut Ui, field: &Field, batch: &BatchView) {
    let Some(next) = batch.next.filter(|_| !batch.units.is_empty()) else {
        return;
    };
    let p = Vec2::from(next);
    let Some(p) = field
        .camera
        .project(p.extend(field.renderer.surface_height(p) + 1.0))
        .map(|q| q / ui.s)
    else {
        return;
    };
    let breathe = 0.6 + 0.4 * (ui.time * 2.4).sin();
    ui.arc(p, 5.0, 0.0, TAU, 1.3, ui::rgb(TONE, 0.6 * breathe));
    ui.disc(p, 1.5, ui::rgb(TONE, 0.8));
}
