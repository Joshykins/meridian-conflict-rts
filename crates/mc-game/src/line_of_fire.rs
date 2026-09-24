//! Line of fire, as the interface shows it: a selected unit whose target the ground
//! hides gets a dashed amber line to it and a "No line of fire" tag, and the attack
//! pointer over an enemy says when the ground hides it from the selection.
//!
//! The sim decides (`mc_sim::line_of_fire`) and reports it per unit in
//! `UnitOrders::hidden_target`. The pointer's preview is worked out here from the
//! rendered ground, before any order is given; it is only a hint.

use crate::orders::Field;
use crate::ui::{self, palette, type_scale, Ui};
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, MoveLayer, Trajectory, UnitBlueprint, Weapon};

/// Most dashed lines drawn in one frame.
const MAX_LINES: usize = 48;
/// Metres between samples of the ground along a line of fire.
const MARCH_STEP: f32 = 4.0;

/// Whether `weapon` on `unit` needs to see what it shoots: the sim's rule (`World::needs_line`).
fn needs_line(unit: &UnitBlueprint, weapon: &Weapon) -> bool {
    weapon.trajectory == Trajectory::Direct
        && !weapon.guided
        && !weapon.torpedo
        && unit.motion.is_none_or(|m| m.layer != MoveLayer::Air)
}

/// The selection's hidden targets: a dashed amber line from each unit to what it is laid
/// on, and the tag once by each target.
pub fn draw_hidden(ui: &mut Ui, field: &Field, alpha: f32) {
    let view = field.view;
    let camera = field.camera;
    let mut tags: Vec<Vec2> = Vec::new();
    let mut lines = 0;
    for queue in &view.status.queues {
        let Some(target) = queue.hidden_target else {
            continue;
        };
        if !view.selection.contains(&queue.unit_id) || lines >= MAX_LINES {
            continue;
        }
        let Some(unit) = view.index_of.get(&queue.unit_id).map(|&i| &view.frame.units[i]) else {
            continue;
        };
        let from = Vec3::from(unit.prev_pos).lerp(Vec3::from(unit.pos), alpha) + Vec3::Z * unit.radius * 0.5;
        let to = Vec3::from(target);
        let (Some(a), Some(b)) = (camera.project(from), camera.project(to)) else {
            continue;
        };
        let (a, b) = (a / ui.s, b / ui.s);
        dashed(ui, a, b, ui.time);
        lines += 1;
        if tags.iter().all(|t| t.distance(b) > 24.0) {
            tags.push(b);
        }
    }
    for at in tags {
        tag(ui, at + Vec2::new(0.0, 18.0), "No line of fire");
    }
}

/// With the attack pointer on `target` (an index into the frame's units): how many of the
/// selection would shoot at it from where they stand but for the ground in between. Shown
/// under the pointer; they close in when told to attack.
pub fn draw_hover(ui: &mut Ui, field: &Field, target: usize) {
    let view = field.view;
    let blueprints = field.blueprints;
    let Some(t) = view.frame.units.get(target) else {
        return;
    };
    let tb = blueprints.unit(BlueprintId(t.blueprint as u16));
    let aim = Vec3::from(t.pos);
    let (mut in_range, mut hidden) = (0, 0);
    for id in &view.selection {
        let Some(u) = view.index_of.get(id).map(|&i| &view.frame.units[i]) else {
            continue;
        };
        let ub = blueprints.unit(BlueprintId(u.blueprint as u16));
        let gap = Vec2::from([u.pos[0], u.pos[1]]).distance(aim.truncate()) - t.radius;
        let guns: Vec<&Weapon> = ub
            .weapons
            .iter()
            .filter(|w| needs_line(ub, w) && gap <= w.range_max.to_f32())
            .collect();
        // Only units that could shoot it from here with a flat gun, and would not already.
        if guns.is_empty() || ub.weapons.iter().any(|w| !needs_line(ub, w) && gap <= w.range_max.to_f32()) {
            continue;
        }
        in_range += 1;
        let muzzle = guns[0].pivot.unwrap_or(guns[0].muzzle).z.to_f32();
        let from = Vec3::from(u.pos) + Vec3::Z * muzzle;
        let height = tb.height.to_f32();
        let seen = |z: f32| clear(field, from, aim.truncate().extend(aim.z + z), t.radius);
        if !seen(height * 0.5) && !seen(height) {
            hidden += 1;
        }
    }
    if hidden == 0 {
        return;
    }
    let text = if hidden == in_range {
        "No line of fire: will close in".to_owned()
    } else {
        format!("{hidden} of {in_range} have no line of fire")
    };
    tag(ui, ui.cursor + Vec2::new(0.0, 26.0), &text);
}

/// Whether the ground stays below the line from `from` to `to`, stopping `short` metres
/// before `to` as the sim does.
fn clear(field: &Field, from: Vec3, to: Vec3, short: f32) -> bool {
    let across = (to - from).truncate().length();
    if across <= short * 2.0 {
        return true;
    }
    let end = from.lerp(to, (across - short) / across);
    let steps = ((across - short) / MARCH_STEP).ceil().max(1.0) as usize;
    (1..=steps).all(|i| {
        let p = from.lerp(end, i as f32 / steps as f32);
        field.renderer.ground_height(p.truncate()) < p.z
    })
}

/// A dashed amber line, its dashes crawling toward the target.
fn dashed(ui: &mut Ui, a: Vec2, b: Vec2, time: f32) {
    let len = a.distance(b);
    if len < 1.0 {
        return;
    }
    let dir = (b - a) / len;
    let (dash, gap) = (6.0, 5.0);
    let mut d = (time * 18.0).rem_euclid(dash + gap) - (dash + gap);
    ui.stroke(a, b, 5.0, ui::rgb(palette::WARN, 0.10));
    while d < len {
        let (s, e) = (d.max(0.0), (d + dash).min(len));
        if e > s {
            ui.stroke(a + dir * s, a + dir * e, 1.5, ui::rgb(palette::WARN, 0.85));
        }
        d += dash + gap;
    }
    ui.disc(b, 2.4, ui::rgb(palette::WARN, 0.9));
}

/// A small glass plate with an amber bar and the words, centred on `at`.
fn tag(ui: &mut Ui, at: Vec2, text: &str) {
    let st = type_scale::CAPTION;
    let w = ui.text_width(st, text) + 16.0;
    let h = 20.0;
    let r = ui::Rect::new(at.x - w * 0.5, at.y - h * 0.5, w, h);
    ui.fill_cut(r, 4.0, ui::rgb(palette::INK, 0.62));
    ui.fill(ui::Rect::new(r.x, r.y, 2.0, h), ui::rgb(palette::WARN, 0.95));
    ui.text(r.x + 9.0, at.y, st, ui::rgb(palette::WARN, 1.0), text);
}
