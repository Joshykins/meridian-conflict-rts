//! Guard areas on the battlefield: the ring of the area guarded, and the circle
//! aircraft fly round it. An aircraft that orbits (`UnitBlueprint::orbit`, the Argus)
//! has its guard drawn as an Orbit: blue, the circle on the ring itself.

use super::Field;
use crate::hud;
use crate::ui::{self, Ui};
use glam::Vec2;
use mc_data::{BlueprintId, Blueprints, MoveLayer};
use mc_sim::mirror::UnitInstance;

/// The edge of a guard area: a dashed ring on the ground, finer the bigger it is,
/// with a faint second ring just inside so it reads as a zone, not a range.
pub(super) fn guard_ring(
    ui: &mut Ui,
    field: &Field,
    c: Vec2,
    radius: f32,
    strength: f32,
    tone: u32,
) {
    let segments = ((radius / 10.0) as usize).clamp(48, 192) & !1;
    let point = |i: usize, r: f32| {
        let p = c + Vec2::from_angle(i as f32 / segments as f32 * std::f32::consts::TAU) * r;
        field
            .camera
            .project(p.extend(field.renderer.surface_height(p) + 1.0))
    };
    let inner = (radius - 6.0).max(radius * 0.97);
    for i in (0..segments).step_by(2) {
        if let (Some(a), Some(b)) = (point(i, radius), point(i + 1, radius)) {
            ui.stroke(a / ui.s, b / ui.s, 1.8, ui::rgb(tone, 0.85 * strength));
        }
        if let (Some(a), Some(b)) = (point(i, inner), point(i + 2, inner)) {
            ui.stroke(a / ui.s, b / ui.s, 1.0, ui::rgb(tone, 0.22 * strength));
        }
    }
}

/// The blue of an Orbit.
const ORBIT: u32 = 0x4F8BFF;

/// How a unit's guard order is drawn.
#[derive(Clone, Copy)]
pub(super) struct GuardLook {
    /// Share of the area's radius the unit circles at (`mc_sim`'s `orbit.rs`): none on
    /// the ground or at sea, half for an aircraft, all of it for one that orbits.
    pub circle: f32,
    pub tone: u32,
}

impl GuardLook {
    /// An engineer's guard is an area assist: its ring is work, not a stance.
    pub(super) fn area_assist() -> GuardLook {
        GuardLook {
            circle: 0.0,
            tone: hud::style::Family::Engineering.tone(),
        }
    }

    fn stance(circle: f32) -> GuardLook {
        GuardLook {
            circle,
            tone: hud::style::Family::Stance.tone(),
        }
    }
}

const ORBITS: GuardLook = GuardLook {
    circle: 1.0,
    tone: ORBIT,
};

/// How `unit`'s guard order is drawn.
pub(super) fn guard_look(blueprints: &Blueprints, unit: &UnitInstance) -> GuardLook {
    let bp = blueprints.unit(BlueprintId(unit.blueprint as u16));
    if bp.orbit.is_some() {
        ORBITS
    } else if bp.motion.is_some_and(|m| m.layer == MoveLayer::Air) {
        GuardLook::stance(0.5)
    } else if bp.builder.is_some() && bp.weapons.is_empty() {
        GuardLook::area_assist()
    } else {
        GuardLook::stance(0.0)
    }
}

/// How a guard the selection is about to be given is drawn: as an Orbit when every
/// aircraft in it orbits, with the aircraft's circle when any flies.
pub(super) fn selection_guard_look(field: &Field) -> GuardLook {
    let view = field.view;
    let looks = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| guard_look(field.blueprints, &view.frame.units[i]))
        .filter(|l| l.circle > 0.0);
    looks.fold(GuardLook::stance(0.0), |seen, l| {
        if seen.circle == 0.0 {
            l
        } else if seen.circle == l.circle {
            seen
        } else {
            GuardLook::stance(0.5)
        }
    })
}

/// Aircraft circling on guard: the circle, `radius` metres round `c`, with a soft glow and,
/// while it is lively (`time`), chevrons flying round it the way the aircraft circle
/// (anticlockwise); a small mark at the middle. `project` takes world metres to points.
/// Returns the strokes it cost.
pub(super) fn orbit_ring(
    ui: &mut Ui,
    project: &dyn Fn(Vec2) -> Option<Vec2>,
    c: Vec2,
    radius: f32,
    strength: f32,
    tone: u32,
    time: Option<f32>,
    budget: usize,
) -> usize {
    use std::f32::consts::TAU;
    const SEGMENTS: usize = 72;
    let on = |a: f32| project(c + Vec2::from_angle(a) * radius);
    let view = ui.size + 40.0;
    let seen = |p: Vec2| p.cmpge(Vec2::splat(-40.0)).all() && p.cmple(view).all();
    let mut cost = 0;
    let mut prev = on(0.0);
    for i in 1..=SEGMENTS {
        let next = on(i as f32 / SEGMENTS as f32 * TAU);
        if let (Some(a), Some(b)) = (prev, next) {
            if (seen(a) || seen(b)) && cost + 2 <= budget {
                ui.stroke(a, b, 6.0, ui::rgb(tone, 0.14 * strength));
                ui.stroke(a, b, 1.5, ui::rgb(tone, 0.7 * strength));
                cost += 2;
            }
        }
        prev = next;
    }
    if let Some(m) = project(c).filter(|m| seen(*m)) {
        ui.disc(m, 5.0, ui::rgb(0x000000, 0.35 * strength));
        ui.arc(m, 4.5, 0.0, TAU, 1.4, ui::rgb(tone, 0.95 * strength));
        ui.disc(m, 1.8, ui::rgb(0xFFFFFF, strength));
        cost += 1;
    }
    let Some(time) = time else {
        return cost;
    };
    // Chevrons about 90 m apart, flying round at 60 m/s.
    let count = ((TAU * radius / 90.0) as usize).clamp(4, 16);
    let spin = time * 60.0 / radius.max(1.0);
    let bright = ui::rgb(super::mix_white(tone, 0.5), 0.9 * strength);
    for i in 0..count {
        if cost + 3 > budget {
            break;
        }
        let a = spin + i as f32 / count as f32 * TAU;
        let (Some(head), Some(tail)) = (on(a), on(a - 18.0 / radius.max(1.0))) else {
            continue;
        };
        if !seen(head) || head.distance(tail) < 2.0 {
            continue;
        }
        let dir = (head - tail).normalize();
        let side = dir.perp() * 4.0;
        let back = head - dir * 6.0;
        ui.stroke(tail, head, 2.0, bright);
        ui.stroke(head, back + side, 1.8, bright);
        ui.stroke(head, back - side, 1.8, bright);
        cost += 3;
    }
    cost
}
