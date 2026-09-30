//! Which unit the pointer is on: its body, or its strategic icon.
//!
//! A unit is drawn twice over: its model, and its icon, which icons.wgsl puts at half
//! the model's height and draws a fixed number of pixels wide. A click on either picks
//! the unit. For most units the two nearly coincide; a titan's icon floats 240 m up and
//! is twice the size of any other, so aiming at the body alone missed it.

use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Blueprints, IconKind};
use mc_render::gpu_consts::icon;
use mc_render::Camera;
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
    let mut best: Option<(f32, usize)> = None;
    for (i, u) in units.iter().enumerate() {
        let bp = blueprints.unit(BlueprintId(u.blueprint as u16));
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
        // The body: a disc the unit's size round its middle.
        let body = pos + Vec3::Z * u.radius * 0.5;
        let body_reach = (u.radius * scale / (body - eye).length().max(1.0)).max(MIN_REACH);
        // The icon: where icons.wgsl centres it, half its drawn width round that.
        let icon_at = pos + Vec3::Z * bp.height.to_f32() * 0.5;
        let icon_reach = (icon_px(bp.visual.icon) * 0.5).max(MIN_REACH);
        let hit = [(body, body_reach), (icon_at, icon_reach)]
            .into_iter()
            .filter_map(|(at, reach)| {
                let d = camera.project(at)?.distance(pixel);
                (d <= reach).then_some(d)
            })
            .reduce(f32::min);
        if let Some(d) = hit {
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
        }
    }
    best.map(|(_, i)| i)
}

/// How wide icons.wgsl draws an icon, in pixels. Only the titan's is wider than the
/// click reach every unit gets anyway.
fn icon_px(kind: IconKind) -> f32 {
    if kind == IconKind::Titan {
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
        Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
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
}
