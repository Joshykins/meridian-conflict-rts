//! Height guides for the selected aircraft: a line from each down to the spot on the
//! ground it is over.

use super::{Field, MAX_AIR_GUIDES};
use crate::ui::Ui;
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use mc_sim::mirror::{KIND_GHOST, KIND_PROP, KIND_WRECK, STATE_RADAR};

/// The box round the selected aircraft, where they were and are this tick, kept for the
/// `Picked::stamp` it was found for: when it is all off one side of the screen, so is
/// every guide, and none of the units is looked at.
#[derive(Default)]
pub(super) struct AirBox {
    stamp: Option<u64>,
    /// Corners, world metres; `None` with no aircraft selected.
    corners: Option<(Vec3, Vec3)>,
}

/// Whether every one of `points`, projected, lies past the same edge of the screen.
fn all_off_one_side(points: &[Option<Vec2>], viewport: Vec2) -> bool {
    let Some(points) = points.iter().copied().collect::<Option<Vec<Vec2>>>() else {
        // Something behind the camera: no shortcut.
        return false;
    };
    let past = |edge: &dyn Fn(Vec2) -> bool| points.iter().all(|&p| edge(p));
    past(&|p| p.x < 0.0)
        || past(&|p| p.y < 0.0)
        || past(&|p| p.x > viewport.x)
        || past(&|p| p.y > viewport.y)
}

/// Height guides for the selected aircraft: a line down to the spot on the ground
/// each one is over. Radar-only contacts have no visible hull to anchor one to.
pub(super) fn aircraft_guides(ui: &mut Ui, field: &Field, alpha: f32, kept: &mut AirBox) {
    let view = field.view;
    let blueprints = field.blueprints;
    let air = |bp: BlueprintId| {
        (blueprints.unit(bp).motion).is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
    };
    let picked = view.picked();
    if kept.stamp != Some(picked.stamp()) {
        kept.stamp = Some(picked.stamp());
        kept.corners = None;
        for &i in picked.rows() {
            let unit = &view.frame.units[i as usize];
            if air(BlueprintId(unit.blueprint as u16)) {
                let (a, b) = (Vec3::from(unit.prev_pos), Vec3::from(unit.pos));
                let (lo, hi) = kept.corners.unwrap_or((a, a));
                kept.corners = Some((lo.min(a).min(b), hi.max(a).max(b)));
            }
        }
    }
    let Some((lo, hi)) = kept.corners else {
        return;
    };
    let viewport = field.camera.viewport;
    let on_screen = |p: Vec2| p.cmpge(Vec2::ZERO).all() && p.cmple(viewport).all();
    let projector = field.camera.projector();
    // A guide runs straight down to the surface, which is never under the sea's: when the
    // line from an aircraft down to that is wholly off one side of the screen, so is its
    // guide, and the ground under it need not be found.
    let lowest = field.map.info().water_level.to_f32() + 1.0;
    let corners: Vec<Option<Vec2>> = (0..8)
        .map(|k| {
            let z = if k & 4 == 0 { hi.z } else { lowest.min(lo.z) };
            let x = if k & 1 == 0 { lo.x } else { hi.x };
            let y = if k & 2 == 0 { lo.y } else { hi.y };
            projector.project(Vec3::new(x, y, z))
        })
        .collect();
    if all_off_one_side(&corners, viewport) {
        return;
    }
    let mut drawn = 0;
    for i in picked.in_frame_order() {
        if drawn >= MAX_AIR_GUIDES {
            return;
        }
        let unit = &view.frame.units[i];
        if unit.owner_flags & (KIND_GHOST | KIND_PROP | KIND_WRECK | STATE_RADAR) != 0
            || !air(BlueprintId(unit.blueprint as u16))
        {
            continue;
        }
        let position = Vec3::from(unit.prev_pos).lerp(Vec3::from(unit.pos), alpha);
        let bottom = position.truncate().extend(lowest.min(position.z));
        if all_off_one_side(
            &[projector.project(position), projector.project(bottom)],
            viewport,
        ) {
            continue;
        }
        let ground = position
            .truncate()
            .extend(field.renderer.surface_height(position.truncate()) + 1.0);
        if position.z <= ground.z {
            continue;
        }
        let (Some(top), Some(foot)) = (projector.project(position), projector.project(ground))
        else {
            continue;
        };
        if !(on_screen(top) || on_screen(foot))
            || top.abs().max_element() >= 20_000.0
            || foot.abs().max_element() >= 20_000.0
        {
            continue;
        }
        drawn += 1;
        let team = field.view.colors[(unit.owner_flags & 0xFF) as usize % mc_core::MAX_PLAYERS];
        let color = [team[0], team[1], team[2], 0.7];
        ui.stroke(top / ui.s, foot / ui.s, 1.0, color);
        if on_screen(foot) {
            let c = foot / ui.s;
            ui.stroke(c - Vec2::X * 3.0, c + Vec2::X * 3.0, 1.0, color);
            ui.stroke(c - Vec2::Y * 3.0, c + Vec2::Y * 3.0, 1.0, color);
        }
    }
}
