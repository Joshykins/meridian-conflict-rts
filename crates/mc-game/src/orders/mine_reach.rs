//! A core mine being placed against the others. The sim refuses one inside any
//! mine's reach, finished or begun, anyone's (`World::mine_in_the_way`), so the
//! preview does too, from the mines in sight. One inside the reach of a mine the
//! side or an ally has only planned may go, with a warning: whichever of the two
//! is begun second will be refused.

use super::{to_fx, Field};
use crate::game::View;
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Blueprints};
use mc_sim::mirror::{KIND_GHOST, KIND_WRECK, STATE_UNIDENTIFIED};
use std::f32::consts::TAU;

/// Metres past a mine's reach the nearest open site keeps, so the lot's snap to the
/// build grid cannot carry it back in.
const CLEAR_MARGIN: f32 = 14.0;
/// Points round each mine's reach tried for the nearest open site.
const RIM_PROBES: usize = 96;
/// Most mines whose rims are searched for it: the nearest ones (presentation only).
const RIM_MINES: usize = 8;

/// A mine the one being placed would crowd.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Crowding {
    pub(crate) at: Vec2,
    /// The circle the new mine must keep out of: the larger of the two reaches.
    pub(crate) keep: f32,
    /// The other mine's own reach, for its territory.
    pub(crate) reach: f32,
    pub(crate) owner: u8,
    pub(crate) blueprint: BlueprintId,
    /// Metres the new mine is inside `keep`.
    pub(crate) short: f32,
}

fn crowding(at: Vec2, reach: f32, other: Vec2, other_reach: f32) -> Option<f32> {
    let keep = reach.max(other_reach);
    let d = at.distance(other);
    (d < keep).then_some(keep - d)
}

/// The mines in sight, standing or begun, anyone's: where, their reach, whose, what.
fn standing_mines<'a>(
    view: &'a View,
    blueprints: &'a Blueprints,
) -> impl Iterator<Item = (Vec2, f32, u8, BlueprintId)> + 'a {
    view.frame.units.iter().filter_map(move |u| {
        if u.owner_flags & (KIND_WRECK | KIND_GHOST | STATE_UNIDENTIFIED) != 0 {
            return None;
        }
        let blueprint = BlueprintId(u.blueprint as u16);
        let m = blueprints.unit(blueprint).mine?;
        Some((
            Vec2::new(u.pos[0], u.pos[1]),
            m.reach.to_f32(),
            (u.owner_flags & 0xFF) as u8,
            blueprint,
        ))
    })
}

/// The nearest mine in sight whose reach a new mine of `reach` at `at` would stand
/// in (or that would stand in its): the sim will not build it there.
pub(crate) fn standing(
    view: &View,
    blueprints: &Blueprints,
    reach: f32,
    at: Vec2,
) -> Option<Crowding> {
    standing_mines(view, blueprints)
        .filter_map(|(p, r, owner, blueprint)| {
            let short = crowding(at, reach, p, r)?;
            Some(Crowding {
                at: p,
                keep: reach.max(r),
                reach: r,
                owner,
                blueprint,
                short,
            })
        })
        .max_by(|a, b| a.short.total_cmp(&b.short))
}

/// The nearest mine the side or an ally has planned and not begun whose reach a new
/// mine of `blueprint` at `at` would stand in. Not the plan in hand (`moving`), not
/// the very same plan (the builders join it), and not the selection's own plans an
/// order without shift is about to replace.
pub(crate) fn planned(
    view: &View,
    blueprints: &Blueprints,
    blueprint: BlueprintId,
    at: Vec2,
    moving: Option<Vec2>,
) -> Option<Crowding> {
    let reach = blueprints.unit(blueprint).mine?.reach.to_f32();
    let pos = to_fx(at);
    let own = view.status.plans.iter().filter_map(|plan| {
        let replaced = view.selection.contains(&plan.unit_id) && !view.shift;
        (!replaced).then_some((view.local, plan))
    });
    let allies = view.status.ally_mine_plans.iter().map(|(o, p)| (*o, p));
    let mut seen: Vec<Vec2> = Vec::new();
    own.chain(allies)
        .filter_map(|(owner, plan)| {
            let m = blueprints.unit(plan.blueprint).mine?;
            let p = Vec2::from(plan.pos);
            let same = plan.blueprint == blueprint && plan.at == pos;
            // A group's builders each carry the plan: count it once.
            if same || moving.is_some_and(|from| from.distance(p) < 1.0) || seen.contains(&p) {
                return None;
            }
            seen.push(p);
            let r = m.reach.to_f32();
            let short = crowding(at, reach, p, r)?;
            Some(Crowding {
                at: p,
                keep: reach.max(r),
                reach: r,
                owner,
                blueprint: plan.blueprint,
                short,
            })
        })
        .max_by(|a, b| a.short.total_cmp(&b.short))
}

/// While a mine of `blueprint` placed at `at` would stand in the reach of one in
/// sight: the nearest site where it would not (`nearest_open`).
pub(crate) fn open_instead(field: &Field, blueprint: BlueprintId, at: Vec2) -> Option<Vec2> {
    let reach = field.blueprints.unit(blueprint).mine?.reach.to_f32();
    standing(field.view, field.blueprints, reach, at)?;
    nearest_open(field, blueprint, at)
}

/// The nearest site to `at` where a mine of `blueprint` would go: out of every
/// mine's reach in sight, and on a lot the placement check passes. Searched round
/// the rims of the nearest mines' reaches, a little outside each.
fn nearest_open(field: &Field, blueprint: BlueprintId, at: Vec2) -> Option<Vec2> {
    let reach = field.blueprints.unit(blueprint).mine?.reach.to_f32();
    let mut mines: Vec<(Vec2, f32)> = standing_mines(field.view, field.blueprints)
        .map(|(p, r, ..)| (p, reach.max(r)))
        .filter(|(p, keep)| p.distance(at) < keep * 3.0)
        .collect();
    mines.sort_by(|a, b| a.0.distance(at).total_cmp(&b.0.distance(at)));
    let clear = |q: Vec2| mines.iter().all(|&(p, keep)| q.distance(p) >= keep);
    let mut candidates: Vec<Vec2> = mines
        .iter()
        .take(RIM_MINES)
        .flat_map(|&(p, keep)| {
            (0..RIM_PROBES).map(move |k| {
                let a = k as f32 / RIM_PROBES as f32 * TAU;
                p + Vec2::from_angle(a) * (keep + CLEAR_MARGIN)
            })
        })
        .filter(|&q| clear(q))
        .collect();
    candidates.sort_by(|a, b| a.distance(at).total_cmp(&b.distance(at)));
    let size = Vec2::from(field.map.info().size_metres().to_f32());
    candidates
        .into_iter()
        .filter(|q| q.x > 0.0 && q.y > 0.0 && q.x < size.x && q.y < size.y)
        .find_map(|q| {
            let (pos, fit) =
                super::site_verdict(field, blueprint, Vec3::new(q.x, q.y, 0.0), None, &[])?;
            fit.is_ok().then(|| Vec2::from(pos.to_f32()))
        })
}
