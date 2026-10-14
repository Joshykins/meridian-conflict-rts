//! Static prop instances and authored surface appearance.
use super::*;
pub(super) fn instances(
    scene: &SceneDesc,
    tile_cache: &TileCache,
    staged: &[Prop],
    prop_base: u32,
) -> Vec<UnitInstance> {
    let kinds: Vec<u16> = PropKind::ALL.iter().map(|k| k.raw()).collect();
    scene
        .map
        .props()
        .iter()
        .chain(staged)
        .enumerate()
        .map(|(i, p)| {
            let xy = p.pos.to_f32();
            let pos = [
                xy[0],
                xy[1],
                tile_cache.overview_height(glam::Vec2::from(xy)),
            ];
            let heading = p.heading.to_radians_f32();
            let kind = kinds.iter().position(|k| *k == p.kind.raw()).unwrap_or(0) as u32;
            // A span of wires is raised to its pivot and pitched to meet the
            // ground at the next tower, stretched so it still reaches it.
            let (arm_pitch, packed) = match p.kind.span() {
                Some((length, pivot)) => {
                    let scale = p.scale_milli as f32 / 1000.0;
                    let reach = length as f32 * scale;
                    let next = glam::Vec2::from(xy) + glam::Vec2::from_angle(heading) * reach;
                    let pitch = (tile_cache.overview_height(next) - pos[2]).atan2(reach);
                    let stretched = p.scale_milli as f32 / pitch.cos();
                    let raise = pivot as f32 * scale;
                    ([pitch, pitch, -raise, 0.0], stretched.round() as u32)
                }
                // A tree stands at a height of its own (`fallen_trees::height_stretch`).
                None if kind < fallen_trees::TREE_KINDS => (
                    [0.0, 0.0, 0.0, fallen_trees::height_stretch(i as u32)],
                    p.scale_milli as u32,
                ),
                None => ([0.0; 4], p.scale_milli as u32),
            };
            // City dressing that nothing hits (cars, lights) shows its wear
            // as health: a burnt-out car. Structures' damage is `city_look`.
            let dressing = mc_map::city::structure(p.kind).is_some_and(|s| s.health == 0);
            let health = if dressing {
                1.0 - p.wear_milli as f32 / 1000.0
            } else {
                1.0
            };
            UnitInstance {
                prev_pos: pos,
                prev_heading: heading,
                pos,
                heading,
                blueprint: prop_base + kind,
                owner_flags: KIND_PROP,
                health,
                build: 1.0,
                turret_yaw: 0.0,
                radius: 4.0,
                unit_id: i as u32,
                packed,
                gait: [0.0; 3],
                upgrade: 0.0,
                arm_pitch,
                prev_turret_yaw: 0.0,
                weld: [0.0; 3],
                recoil: 0.0,
                prev_recoil: 0.0,
                weld_first: 0,
                weld_count: 0,
                deploy: 0.0,
                prev_deploy: 0.0,
                _pad2: [0.0; 2],
                refit_modules: 0,
                status: [0; 3],
                mount: [0.0; 4],
                spin_recoil: [0.0; 4],
                // Static props reserve fx for RGB tint and presence; sim units use it for their effects.
                fx: scene.map.prop_tints().get(i).map_or([0.0; 4], |t| {
                    [
                        t[0] as f32 / 255.0,
                        t[1] as f32 / 255.0,
                        t[2] as f32 / 255.0,
                        1.0,
                    ]
                }),
                drive_swing: [0.0; 2],
                twin_spin: [0.0; 2],
            }
        })
        .collect()
}
pub(super) fn skin_for(bps: &Blueprints, index: usize) -> u32 {
    let unit = bps.units.get(index);
    if let Some(key) = unit.and_then(|u| u.visual.skin.as_deref()) {
        if let Some(i) = bps.skins.iter().position(|s| s.key == key) {
            return i as u32;
        }
    }
    unit.map_or(bps.factions.first(), |u| {
        bps.factions.get(u.faction.0 as usize)
    })
    .map_or(0, |f| u32::from(f.skin.0))
}
