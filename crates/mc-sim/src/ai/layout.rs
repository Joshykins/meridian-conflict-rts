//! The base's layout: power in compact farms, shields over what is worth
//! covering.
//!
//! Power used to go wherever a builder happened to stand once it was far from
//! home, and every new plant was then taken as the seed of a farm, so home
//! builders grew little farms around those strays too. At home, a ring search
//! from a fixed spot behind the yard put each plant in a random slot, and when
//! that spot was water the plants strung out along the shore. Shields went a
//! ring or two out from the first factory, whatever stood there. Now power
//! packs, nearest first, into a few farms on the widest stretch of home ground,
//! and a shield goes where it covers the most that is not covered yet, or
//! nowhere.
use super::*;
use mc_data::MoveLayer;

/// A farm holds plants within this of its centre; then the next farm opens.
const FARM_RADIUS: i32 = 110;
/// Farms stand at least this far apart, so they read as separate blocks.
const FARM_SPACING: i32 = 230;
/// Farm centres are tried on rings this far from the start.
const FARM_RINGS: [i32; 3] = [240, 330, 420];
/// Ground sampled around a farm centre to judge it, on this pitch.
const FARM_SAMPLE_M: i32 = 24;
/// A shield pays for itself only over at least this much of its own mass cost
/// in buildings not yet under another shield, in percent.
const SHIELD_WORTH: i64 = 150;
/// Buildings this far from the start are not the base a shield is for.
const SHIELDED_BASE: i32 = 700;
/// A shield's lot is looked for this far around the spot it should cover:
/// the lanes between factories are too narrow for one.
const SHIELD_SLACK: i32 = 96;

/// Where a structure's lot is chosen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Place {
    /// The ring search around `Job::near` (`find_site`).
    Around,
    /// The free lot nearest `Job::near`, in a square this many metres out.
    Packed(i32),
    /// A lot in the base's power farms (`farm_site`).
    Farm,
}

impl World {
    /// Whether `bp` could go at `site`: placeable, clear of this think's
    /// claims, off the mine pads when asked, on home ground, keeping its lanes.
    pub(super) fn lot_free(
        &self,
        bp: &UnitBlueprint,
        site: FxVec2,
        claimed: &[Claim],
        ore: &[FxVec2],
        home: Option<&staging::HomeGround>,
    ) -> bool {
        let foot = bp.footprint.0.max(bp.footprint.1) as i32;
        let extra = if bp.has(cat::FACTORY) { 1 } else { 0 };
        let cell = mc_map::BUILD_CELL_M;
        // Stay off the mex pad, not a factory-width away from every deposit.
        let deposit_pad = Fx::from_int(foot * cell / 2 + 16);
        self.can_place(bp, site)
            && !claimed.iter().any(|c| {
                let need = Fx::from_int((foot + c.foot + extra) * cell / 2);
                c.pos.distance(site) < need
            })
            && !ore.iter().any(|d| d.distance(site) < deposit_pad)
            // Not up on a shelf or across a river from home: what a factory
            // there makes cannot join the army, and builders walk around
            // the cliff for every job. Mines go where the ground pays, and
            // a site on the water is judged by the water.
            && (bp.mine.is_some()
                || !self.nav.passable(MoveLayer::Land, 0, site)
                || home.is_none_or(|g| g.reaches(site)))
            && self.keeps_lanes(bp, site, claimed)
    }

    /// The free lot nearest `centre` in a square `radius` metres out, on a
    /// grid `step` build cells apart. Filling nearest first grows one tight
    /// block instead of a scatter.
    pub(super) fn pack_site(
        &self,
        bp: &UnitBlueprint,
        centre: FxVec2,
        radius: i32,
        step: i32,
        claimed: &[Claim],
        keep_off_deposits: bool,
        home: Option<&staging::HomeGround>,
    ) -> Option<FxVec2> {
        let cell = mc_map::BUILD_CELL_M * step.max(1);
        let origin = snap_to_build_grid(bp, centre);
        let n = radius / cell;
        let mut spots: Vec<(i32, i32, FxVec2)> = Vec::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let site = origin + FxVec2::from_ints(dx * cell, dy * cell);
                if self.terrain.in_bounds(site) {
                    spots.push((dx.abs().max(dy.abs()), dx * dx + dy * dy, site));
                }
            }
        }
        // Square rings outward, so a farm fills in as a block, not a diamond.
        spots.sort_by_key(|&(ring, d2, p)| (ring, d2, p.x, p.y));
        let ore = if keep_off_deposits {
            self.ore_centres()
        } else {
            Vec::new()
        };
        spots
            .into_iter()
            .map(|(_, _, p)| p)
            .find(|&p| self.lot_free(bp, p, claimed, &ore, home))
    }

    /// A lot for a power plant (or storage): the first farm, in order of
    /// preference, with room left, else anywhere near home as a last resort.
    pub(super) fn farm_site(
        &self,
        bp: &UnitBlueprint,
        start: FxVec2,
        facing: Angle,
        claimed: &[Claim],
        home: Option<&staging::HomeGround>,
    ) -> Option<FxVec2> {
        let cells = bp.footprint.0.max(bp.footprint.1) as i32;
        // A plant bigger than a farm's middle still fits at its edge.
        let reach = FARM_RADIUS + cells * mc_map::BUILD_CELL_M / 2;
        // Rows and columns a lot apart, big plants a lane apart as well: the
        // farm comes out a grid, not rows that slip half a plant.
        let step = if cells > lots::SMALL_FOOT {
            cells + lots::LANE_CELLS
        } else {
            cells
        };
        let mut farms = self.power_farms(start, facing, home);
        // Small plants stand flush in a block, big ones keep lanes: each size
        // starts on a farm of its own, or the lanes break the block up.
        if cells > lots::SMALL_FOOT && farms.len() > 1 {
            farms.rotate_left(1);
        }
        farms
            .into_iter()
            .find_map(|c| self.pack_site(bp, c, reach, step, claimed, true, home))
            .or_else(|| self.pack_site(bp, start, HOME_RADIUS.floor_int(), 1, claimed, true, home))
    }

    /// Farm centres, best first. They depend on the ground alone, never on
    /// what stands there, so a farm keeps its spot for the whole match: the
    /// widest open home ground wins, behind the base before its flanks, clear
    /// of the factory yard.
    fn power_farms(
        &self,
        start: FxVec2,
        facing: Angle,
        home: Option<&staging::HomeGround>,
    ) -> Vec<FxVec2> {
        let back = facing + Angle::HALF_TURN;
        // (open ground, preference, centre)
        let mut spots: Vec<(i32, i32, FxVec2)> = Vec::new();
        for (ri, &r) in FARM_RINGS.iter().enumerate() {
            // Never on the side facing the enemy.
            for k in 0..7i32 {
                // 0, +30, -30, ... +-90 degrees off straight back.
                let off = (k + 1) / 2 * if k % 2 == 0 { -1 } else { 1 };
                let at = start
                    + FxVec2::from_angle(back + Angle((off * 65536 / 12) as u16)) * Fx::from_int(r);
                if !self.terrain.in_bounds(at)
                    || (0..6).any(|i| {
                        self.yard_anchor(start, facing, i).distance(at) < Fx::from_int(120)
                    })
                {
                    continue;
                }
                let open = self.open_ground(at, home);
                spots.push((open, off.abs() * 2 + ri as i32, at));
            }
        }
        let best = spots.iter().map(|s| s.0).max().unwrap_or(0);
        // Ground nearly as open as the best counts as good as it: then the
        // spot behind the base wins over one out on a flank.
        spots.sort_by_key(|&(open, pref, p)| (-(open * 5 / best.max(1)).min(4), pref, p.x, p.y));
        let mut farms: Vec<FxVec2> = Vec::new();
        for (open, _, p) in spots {
            if open * 2 < best {
                continue;
            }
            if farms
                .iter()
                .all(|f| f.distance(p) >= Fx::from_int(FARM_SPACING))
            {
                farms.push(p);
            }
        }
        farms
    }

    /// Flat, walkable home ground around `at`, in samples.
    fn open_ground(&self, at: FxVec2, home: Option<&staging::HomeGround>) -> i32 {
        let n = FARM_RADIUS / FARM_SAMPLE_M;
        let mut open = 0;
        for dy in -n..=n {
            for dx in -n..=n {
                if dx * dx + dy * dy > n * n {
                    continue;
                }
                let p = at + FxVec2::from_ints(dx * FARM_SAMPLE_M, dy * FARM_SAMPLE_M);
                // A medium hull's clearance stands in for flat enough to build on.
                open += (self.terrain.in_bounds(p)
                    && self.nav.passable(MoveLayer::Land, 1, p)
                    && home.is_none_or(|g| g.reaches(p))) as i32;
            }
        }
        open
    }

    /// Where a new shield of `bp` would cover the most of the base that no
    /// shield covers yet, if that is worth the shield. `claimed` shields of
    /// this think count as covering already.
    pub(super) fn shield_spot(
        &self,
        player: u8,
        bp: &UnitBlueprint,
        start: FxVec2,
        claimed: &[Claim],
    ) -> Option<FxVec2> {
        let reach = bp.shield.as_ref()?.radius;
        let worth = self.unshielded(player, start, claimed);
        // Try each building's own spot, and the midpoint of each pair of big
        // ones: two factories side by side are best covered from between them.
        let mut tries: Vec<FxVec2> = worth.iter().map(|w| w.0).collect();
        let big: Vec<_> = worth
            .iter()
            .filter(|w| w.1 * 3 >= shield_need(bp))
            .collect();
        for (i, a) in big.iter().enumerate() {
            for b in &big[i + 1..] {
                if a.0.distance(b.0) < reach * 2 {
                    tries.push(a.0.lerp(b.0, Fx::HALF));
                }
            }
        }
        tries
            .into_iter()
            .map(|c| (covered(&worth, c, reach), c))
            .filter(|(v, _)| *v >= shield_need(bp))
            .max_by_key(|(v, c)| (*v, -c.x, -c.y))
            .map(|(_, c)| c)
    }

    /// The free lot near `spot` (from `shield_spot`) that covers the most:
    /// the spot itself is often the middle of a factory.
    pub(super) fn shield_site(
        &self,
        player: u8,
        bp: &UnitBlueprint,
        spot: FxVec2,
        start: FxVec2,
        claimed: &[Claim],
        home: Option<&staging::HomeGround>,
    ) -> Option<FxVec2> {
        let reach = bp.shield.as_ref()?.radius;
        let worth = self.unshielded(player, start, claimed);
        let cell = mc_map::BUILD_CELL_M;
        let origin = snap_to_build_grid(bp, spot);
        let n = SHIELD_SLACK / cell;
        let ore = self.ore_centres();
        let mut best: Option<(Fx, i32, FxVec2)> = None;
        for dy in -n..=n {
            for dx in -n..=n {
                let d2 = dx * dx + dy * dy;
                if d2 > n * n {
                    continue;
                }
                let site = origin + FxVec2::from_ints(dx * cell, dy * cell);
                if !self.terrain.in_bounds(site) {
                    continue;
                }
                let v = covered(&worth, site, reach);
                if best.is_some_and(|(bv, bd, _)| (v, -d2) <= (bv, -bd)) {
                    continue;
                }
                if self.lot_free(bp, site, claimed, &ore, home) {
                    best = Some((v, d2, site));
                }
            }
        }
        best.filter(|(v, _, _)| *v >= shield_need(bp))
            .map(|(_, _, p)| p)
    }

    /// The base's factories, power and storage no shield covers yet, with
    /// what they cost. The rest is cheap, spread out, or out in front.
    fn unshielded(&self, player: u8, start: FxVec2, claimed: &[Claim]) -> Vec<(FxVec2, Fx)> {
        let units = &self.state.units;
        let mut shields: Vec<(FxVec2, Fx)> = claimed
            .iter()
            .filter(|c| c.cover > Fx::ZERO)
            .map(|c| (c.pos, c.cover))
            .collect();
        let mut worth: Vec<(FxVec2, Fx)> = Vec::new();
        for row in units.slots.iter() {
            if units.owner[row] != player {
                continue;
            }
            let other = self.bp(row);
            if !other.is_structure() {
                continue;
            }
            let pos = units.pos[row];
            if let Some(s) = &other.shield {
                shields.push((pos, s.radius));
                continue;
            }
            if !(other.has(cat::FACTORY) || other.has(cat::POWER) || other.has(cat::STORAGE))
                || other.has(cat::EXTRACTOR)
                || pos.distance(start) > Fx::from_int(SHIELDED_BASE)
            {
                continue;
            }
            worth.push((pos, other.cost_mass));
        }
        worth.retain(|(p, _)| !shields.iter().any(|(c, r)| inside(*p, *c, *r)));
        worth
    }
}

/// A building counts as covered with its middle under the dome. A shield
/// cannot stand much closer to a factory: half the factory, a lane and half
/// the shield are 90 m.
fn inside(p: FxVec2, centre: FxVec2, radius: Fx) -> bool {
    p.distance(centre) < radius
}

fn covered(worth: &[(FxVec2, Fx)], centre: FxVec2, radius: Fx) -> Fx {
    worth
        .iter()
        .filter(|(p, _)| inside(*p, centre, radius))
        .fold(Fx::ZERO, |s, (_, v)| s + *v)
}

fn shield_need(bp: &UnitBlueprint) -> Fx {
    bp.cost_mass * Fx::ratio(SHIELD_WORTH, 100)
}
