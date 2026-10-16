//! Which unit the pointer is on: its body, or its strategic icon.
//!
//! A unit is drawn twice over: its model, and its icon, which icons.wgsl puts at half
//! the model's height and draws a fixed number of pixels wide. A click on either picks
//! the unit. For most units the two nearly coincide; a titan's icon floats 240 m up and
//! is twice the size of any other, so aiming at the body alone missed it.
//!
//! The body is the unit's standing column, from its feet to its full height, as wide as
//! the unit: a titan's torso stands 300 m and more over its feet, and a disc round its
//! shins (or the point at its feet, for a drag box) missed everything a player aims at.
//! A long hull (`UnitBlueprint::body`) is its spine instead, laid along its heading half
//! way up, as wide as its beam: a long hull's radius reaches far past its beam, and a
//! disc that wide picked it from open sky.

use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Blueprints, IconKind};
use mc_render::gpu_consts::icon;
use mc_render::{Camera, Projector};
use mc_sim::mirror::{self, UnitInstance, KIND_WRECK};
use mc_sim::tables::flag;

/// No target is smaller than this many pixels of reach, however far out the view.
const MIN_REACH: f32 = 11.0;

/// The unit (or wreck) drawn nearest to `pixel`, if any is within reach of it.
pub(crate) fn unit_at(
    units: &[UnitInstance],
    blueprints: &Blueprints,
    camera: &Camera,
    pixel: Vec2,
) -> Option<usize> {
    let eye = camera.eye();
    let scale = camera.projection_scale();
    let projector = camera.projector();
    let (_, ray) = camera.ray(pixel);
    let mut best: Option<(f32, usize)> = None;
    for (i, u) in units.iter().enumerate() {
        let bp = blueprints.unit(BlueprintId(u.blueprint as u16));
        // Most of an army is nowhere near the pointer: one far off its ray (further
        // from it than the unit's whole size and every reach a click has, at its
        // distance) is passed over before anything is drawn out. A long hull is
        // measured along its spine below.
        if bp.body.is_none() {
            let size = bp.height.to_f32() + u.radius;
            let to = Vec3::from(u.pos) + Vec3::Z * bp.height.to_f32() * 0.5 - eye;
            let along = to.dot(ray);
            let reach = MIN_REACH.max(icon_px(bp.visual.icon) * 0.5) * 2.0;
            let off = (to.length_squared() - along * along).max(0.0);
            let room = along.max(0.0) * reach / scale + size;
            if along > 0.0 && off > room * room {
                continue;
            }
        }
        if bp.carried_drone && u.owner_flags & KIND_WRECK == 0 {
            continue;
        }
        // Falling hull IDs belong to the former unit, never the wreck table.
        if u.owner_flags & KIND_WRECK != 0 && u.packed == mirror::WRECK_FALLING {
            continue;
        }
        if u.owner_flags & (flag::IN_FACTORY as u32) << 8 != 0 {
            continue;
        }
        let pos = Vec3::from(u.pos);
        let height = bp.height.to_f32();
        // The body: the column from its feet to its top, the unit's size across; or a long
        // hull's spine half way up, its beam across.
        let (ends, width) = match bp.body {
            Some(b) => {
                let nose = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
                let mid = pos + Vec3::Z * height * 0.5;
                let beam = b.beam.to_f32();
                let ends = (
                    mid + nose * (b.aft.to_f32() + beam),
                    mid + nose * (b.fore.to_f32() - beam),
                );
                (ends, beam.max(height * 0.5))
            }
            None => ((pos, pos + Vec3::Z * height), u.radius),
        };
        let body = segment(&projector, ends.0, ends.1).and_then(|(a, b)| {
            let (d, along) = to_segment(pixel, a, b);
            let at = ends.0.lerp(ends.1, along);
            let reach = (width * scale / (at - eye).length().max(1.0)).max(MIN_REACH);
            (d <= reach).then_some(d)
        });
        // The icon: where icons.wgsl centres it, half its drawn width round that.
        let icon = projector
            .project(pos + Vec3::Z * height * 0.5)
            .map(|at| at.distance(pixel))
            .filter(|&d| d <= (icon_px(bp.visual.icon) * 0.5).max(MIN_REACH));
        let hit = body.into_iter().chain(icon).reduce(f32::min);
        if let Some(d) = hit {
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
        }
    }
    best.map(|(_, i)| i)
}

/// Whether any of a unit's standing column (`pos` up `height`) is drawn inside the
/// screen rectangle `min..max`: a drag box over a titan's torso takes it.
pub(crate) fn column_in_box(
    projector: &Projector,
    pos: Vec3,
    height: f32,
    min: Vec2,
    max: Vec2,
) -> bool {
    let Some((feet, top)) = column(projector, pos, height) else {
        return false;
    };
    // Clip the drawn column to the box, one side at a time (Liang-Barsky).
    let d = top - feet;
    let (mut enter, mut leave) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, feet.x - min.x),
        (d.x, max.x - feet.x),
        (-d.y, feet.y - min.y),
        (d.y, max.y - feet.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else if p < 0.0 {
            enter = enter.max(q / p);
        } else {
            leave = leave.min(q / p);
        }
    }
    enter <= leave
}

/// Where a unit's feet (`pos`) and top (`height` over them) are drawn. A top behind the
/// camera falls back to the feet, and the other way round.
fn column(projector: &Projector, pos: Vec3, height: f32) -> Option<(Vec2, Vec2)> {
    segment(projector, pos, pos + Vec3::Z * height)
}

/// Where the segment `a..b` is drawn. An end behind the camera falls back to the other.
fn segment(projector: &Projector, a: Vec3, b: Vec3) -> Option<(Vec2, Vec2)> {
    match (projector.project(a), projector.project(b)) {
        (Some(f), Some(t)) => Some((f, t)),
        (Some(p), None) | (None, Some(p)) => Some((p, p)),
        (None, None) => None,
    }
}

/// How far `p` is from the segment `a..b`, and how far along it (0 at `a`, 1 at `b`)
/// the nearest point lies.
fn to_segment(p: Vec2, a: Vec2, b: Vec2) -> (f32, f32) {
    let d = b - a;
    let along = ((p - a).dot(d) / d.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
    (p.distance(a + d * along), along)
}

/// How wide icons.wgsl draws an icon, in pixels. Only the tier 5s' (a titan's, a space
/// elevator's) are wider than the click reach every unit gets anyway.
fn icon_px(kind: IconKind) -> f32 {
    if matches!(
        kind,
        IconKind::Titan | IconKind::WingedTitan | IconKind::Elevator | IconKind::GraviticEngine
    ) {
        icon::TITAN_PX
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;

    fn blueprints() -> Blueprints {
        Blueprints::load(
            &std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../../data"),
        )
        .expect("blueprints")
    }

    /// The strategic view: far out, looking down at an angle.
    fn far_camera(at: Vec3) -> Camera {
        let mut camera = Camera::new(Vec2::splat(24576.0), Vec2::new(2560.0, 1440.0));
        camera.focus = at;
        camera.distance = 16000.0;
        camera
    }

    fn titan_at(blueprints: &Blueprints, at: Vec3) -> UnitInstance {
        let bp = blueprints.unit_by_key("aster_t5_titan").expect("titan");
        UnitInstance {
            pos: at.into(),
            prev_pos: at.into(),
            blueprint: bp.id.0 as u32,
            radius: bp.radius.to_f32(),
            build: 1.0,
            health: 1.0,
            ..UnitInstance::zeroed()
        }
    }

    #[test]
    fn a_click_on_a_titans_icon_picks_it() {
        let blueprints = blueprints();
        let at = Vec3::new(12000.0, 12000.0, 0.0);
        let camera = far_camera(at);
        let titan = titan_at(&blueprints, at);
        let bp = blueprints.unit(BlueprintId(titan.blueprint as u16));
        let icon = camera
            .project(at + Vec3::Z * bp.height.to_f32() * 0.5)
            .expect("on screen");
        // Near the icon's top edge, where the body's reach does not come.
        let pixel = icon - Vec2::Y * icon::TITAN_PX * 0.45;
        assert_eq!(unit_at(&[titan], &blueprints, &camera, pixel), Some(0));
        // Clear of the icon, nothing.
        let pixel = icon - Vec2::Y * icon::TITAN_PX * 1.5;
        assert_eq!(unit_at(&[titan], &blueprints, &camera, pixel), None);
    }

    /// Close in, looking across at it, where its torso stands well clear of its icon.
    fn near_camera(at: Vec3) -> Camera {
        let mut camera = far_camera(at);
        camera.distance = 2500.0;
        camera
    }

    #[test]
    fn a_click_on_a_titans_torso_picks_it() {
        let blueprints = blueprints();
        let at = Vec3::new(12000.0, 12000.0, 0.0);
        let camera = near_camera(at);
        let titan = titan_at(&blueprints, at);
        let torso = camera.project(at + Vec3::Z * 420.0).expect("on screen");
        let icon = camera.project(at + Vec3::Z * 240.0).expect("on screen");
        assert!(
            torso.distance(icon) > icon::TITAN_PX,
            "the torso is clear of the icon"
        );
        assert_eq!(unit_at(&[titan], &blueprints, &camera, torso), Some(0));
        // Well to the side of it, nothing.
        let beside = torso + Vec2::X * 400.0;
        assert_eq!(unit_at(&[titan], &blueprints, &camera, beside), None);
    }

    #[test]
    fn a_drag_box_over_a_titans_torso_takes_it() {
        let blueprints = blueprints();
        let at = Vec3::new(12000.0, 12000.0, 0.0);
        let camera = near_camera(at);
        let height = blueprints
            .unit_by_key("aster_t5_titan")
            .expect("titan")
            .height
            .to_f32();
        let torso = camera.project(at + Vec3::Z * 420.0).expect("on screen");
        let feet = camera.project(at).expect("on screen");
        let half = Vec2::splat(20.0);
        assert!(
            !(feet.cmpge(torso - half).all() && feet.cmple(torso + half).all()),
            "the box leaves its feet out"
        );
        assert!(column_in_box(
            &camera.projector(),
            at,
            height,
            torso - half,
            torso + half
        ));
        let aside = torso + Vec2::X * 400.0;
        assert!(!column_in_box(
            &camera.projector(),
            at,
            height,
            aside - half,
            aside + half
        ));
    }

    /// A long hull is picked along its spine, not from a disc of its radius: a click over
    /// its nose takes it, one off its beam as far out as its nose is ahead does not.
    #[test]
    fn a_click_takes_a_long_hull_along_its_spine_only() {
        let blueprints = blueprints();
        let bp = blueprints
            .unit_by_key("regency_t4_space_destroyer")
            .expect("scourge");
        let at = Vec3::new(12000.0, 12000.0, 1500.0);
        let ship = UnitInstance {
            pos: at.into(),
            prev_pos: at.into(),
            blueprint: bp.id.0 as u32,
            radius: bp.radius.to_f32(),
            build: 1.0,
            health: 1.0,
            ..UnitInstance::zeroed()
        };
        let camera = far_camera(at);
        let mid = Vec3::Z * bp.height.to_f32() * 0.5;
        let nose = camera
            .project(at + mid + Vec3::X * 300.0)
            .expect("on screen");
        assert_eq!(unit_at(&[ship], &blueprints, &camera, nose), Some(0));
        let beam = camera
            .project(at + mid + Vec3::Y * 300.0)
            .expect("on screen");
        assert_eq!(unit_at(&[ship], &blueprints, &camera, beam), None);
    }
}
