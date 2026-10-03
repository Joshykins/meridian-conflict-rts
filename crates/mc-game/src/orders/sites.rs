//! Where a structure being placed would stand, and why it cannot when it cannot:
//! the map's ground by the sim's own rules, the structures standing and planned,
//! and for a core mine the reach of the mines in sight (`mine_reach`).

use super::{half_footprint, mine_reach, to_fx, Field};
use glam::{Vec2, Vec3};
use mc_core::FxVec2;
use mc_data::BlueprintId;
use mc_sim::mirror::KIND_WRECK;
use mc_sim::placement::Unfit;

/// Where `blueprint` would stand with the pointer on `ground`, and whether it looks
/// buildable there. `moving` is the site of the plan being dragged, when it is one.
/// The sim has the final say; this only drives the preview colour.
pub fn site(
    field: &Field,
    blueprint: BlueprintId,
    ground: Vec3,
    moving: Option<FxVec2>,
) -> Option<(FxVec2, bool)> {
    site_verdict(field, blueprint, ground, moving, &[]).map(|(pos, fit)| (pos, fit.is_ok()))
}

/// `site`, with why it will not do when it will not.
pub fn site_verdict(
    field: &Field,
    blueprint: BlueprintId,
    ground: Vec3,
    moving: Option<FxVec2>,
    taken: &[FxVec2],
) -> Option<(FxVec2, Result<(), Unfit>)> {
    let bp = field.blueprints.unit(blueprint);
    let pos = mc_sim::world::snap_to_build_grid(bp, to_fx(ground.truncate()));
    // The ground and the map's cities first, by the sim's own rules.
    if let Some(sites) = field.view.sites.get() {
        if let Err(why) = sites.check(bp, pos) {
            return Some((pos, Err(why)));
        }
    }
    // A core mine keeps out of every mine's reach, as the sim has it: only the
    // ones in sight can say so here.
    if let Some(m) = bp.mine {
        let at = Vec2::from(pos.to_f32());
        if moving != Some(pos)
            && mine_reach::standing(field.view, field.blueprints, m.reach.to_f32(), at).is_some()
        {
            return Some((pos, Err(Unfit::MineReach)));
        }
    }
    let (pos, valid) = site_among(field, blueprint, ground, moving, taken)?;
    let why = if field.view.sites.get().is_some() {
        Unfit::Taken
    } else {
        Unfit::Water
    };
    Some((pos, if valid { Ok(()) } else { Err(why) }))
}

/// `site`, treating `taken` as structures already spoken for (the earlier
/// buildings of a place-drag). The ground itself is `site_verdict`'s, when
/// the map's sites are known.
fn site_among(
    field: &Field,
    blueprint: BlueprintId,
    ground: Vec3,
    moving: Option<FxVec2>,
    taken: &[FxVec2],
) -> Option<(FxVec2, bool)> {
    let Field {
        view,
        blueprints,
        map,
        ..
    } = field;
    let bp = blueprints.unit(blueprint);
    let mut valid = true;
    let pos = mc_sim::world::snap_to_build_grid(bp, to_fx(ground.truncate()));
    // The plan in hand put back where it was.
    if moving == Some(pos) {
        return Some((pos, true));
    }
    let half = half_footprint(blueprints, blueprint);
    let p = Vec2::from(pos.to_f32());
    let overlaps = |centre: Vec2, other: BlueprintId| {
        let d = (centre - p).abs();
        let reach = half + half_footprint(blueprints, other);
        d.x < reach.x && d.y < reach.y
    };
    for u in &view.frame.units {
        let other = BlueprintId(u.blueprint as u16);
        if u.owner_flags & KIND_WRECK == 0
            && blueprints.unit(other).is_structure()
            && overlaps(Vec2::new(u.pos[0], u.pos[1]), other)
        {
            valid = false;
        }
    }
    for plan in &view.status.plans {
        let same = plan.blueprint == blueprint && plan.at == pos;
        let selected = view.selection.contains(&plan.unit_id);
        let out_of_the_way = match moving {
            // The plan in hand; and the same plan in another queue, which it may join.
            Some(from) => same || (plan.at == from && plan.blueprint == blueprint),
            // An order that is not queued replaces the selection's plans; the same plan elsewhere can be joined.
            None => (selected && !view.shift) || (same && !selected),
        };
        if !out_of_the_way && overlaps(Vec2::from(plan.pos), plan.blueprint) {
            valid = false;
        }
    }
    for &centre in taken {
        if overlaps(Vec2::from(centre.to_f32()), blueprint) {
            valid = false;
        }
    }
    let water = map.info().water_level.to_f32();
    if view.sites.get().is_some() {
        // The ground was judged against the map, above.
    } else if !bp.water_build && field.renderer.ground_height(ground.truncate()) < water {
        valid = false;
    }
    // A naval yard wants open water under its whole lot: the middle and the corners.
    if bp.water_only() && view.sites.get().is_none() {
        let half = Vec2::new(bp.footprint.0 as f32, bp.footprint.1 as f32)
            * (mc_map::BUILD_CELL_M as f32 * 0.5);
        let at = Vec2::from(pos.to_f32());
        for corner in [
            Vec2::ZERO,
            half,
            -half,
            Vec2::new(half.x, -half.y),
            Vec2::new(-half.x, half.y),
        ] {
            if field.renderer.ground_height(at + corner) >= water {
                valid = false;
            }
        }
    }
    Some((pos, valid))
}
