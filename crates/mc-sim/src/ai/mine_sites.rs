//! Where the AI's next core mine goes: ore first, then the open ground that
//! keeps enough of a mine's reach to itself.
use super::*;

impl World {
    /// Nearest ore field with room for another mine, within `range` of `from`,
    /// staying off the enemy's doorstep until the army can contest it; with
    /// `bare`, else the open ground where a mine gets at least that share
    /// of what a whole circle of land would give it, the most for the walk
    /// there. Its centre; the builder's site search finds the lot.
    pub(super) fn free_deposit(
        &self,
        from: FxVec2,
        claimed: &[Claim],
        range: Fx,
        intel: &Intel,
        bare: Option<Fx>,
    ) -> Option<FxVec2> {
        let mine = self.first_mine()?;
        let m = mine.mine?;
        let open = |d: &FxVec2| self.deposit_open(&m, *d, from, range, claimed, intel);
        let ore = self
            .free_ores(from, claimed, range, intel)
            .into_iter()
            .next();
        // No ore left in range: a bare mine still pays, but only where it keeps
        // enough ground. Filling the gaps between mines only takes ground from
        // them: the side gains little more than the new shaft's base.
        ore.or_else(|| {
            let least = bare?;
            let step = m.reach / 2;
            let mut spots: Vec<FxVec2> = (1..=(range / step).floor_int().max(1))
                .flat_map(|ring| {
                    let r = step * ring;
                    let n = 6 * ring;
                    (0..n)
                        .map(move |k| from + FxVec2::from_angle(Angle((k * 65536 / n) as u16)) * r)
                })
                .filter(|d| self.terrain.in_bounds(*d) && open(d))
                .collect();
            spots.sort_by_key(|d| (d.distance_sq(from), d.x, d.y));
            // Against a whole circle of land, so the sea and the map's edge count as lost ground.
            let hectares = m.reach * m.reach * Fx::ratio(355, 113) / 10000;
            let whole = crate::mines::land_rate(&m, hectares, Fx::ZERO);
            // The nearest few, the one that pays most for the walk.
            let walk = m.reach * 3 / 2;
            spots
                .into_iter()
                .take(BARE_MINE_PROBES)
                .filter_map(|d| {
                    let share = self.mine_share_at(mine, d);
                    let rate = crate::mines::land_rate(&m, share.ground, share.ore);
                    (rate >= whole * least).then_some((rate * walk / (d.distance(from) + walk), d))
                })
                .max_by_key(|&(worth, d)| (worth, std::cmp::Reverse((d.x, d.y))))
                .map(|(_, d)| d)
        })
    }

    /// The tech 1 mine: the one a new deposit is claimed with.
    fn first_mine(&self) -> Option<&mc_data::UnitBlueprint> {
        self.blueprints
            .units
            .iter()
            .find(|b| b.mine.is_some() && b.tech == 1)
    }

    /// Ore fields with room for another mine within `range` of `from`, nearest
    /// first (`free_deposit`'s rule for ore).
    pub(super) fn free_ores(
        &self,
        from: FxVec2,
        claimed: &[Claim],
        range: Fx,
        intel: &Intel,
    ) -> Vec<FxVec2> {
        let Some(m) = self.first_mine().and_then(|b| b.mine) else {
            return Vec::new();
        };
        let mut ore: Vec<FxVec2> = self
            .ore_centres()
            .into_iter()
            .filter(|d| self.deposit_open(&m, *d, from, range, claimed, intel))
            .collect();
        ore.sort_by_key(|d| (d.distance_sq(from), d.x, d.y));
        ore
    }

    /// Whether a mine may go at `d`: in range, not crowding another mine (built or
    /// planned), off the enemy's doorstep, and out of danger.
    fn deposit_open(
        &self,
        m: &mc_data::Mine,
        d: FxVec2,
        from: FxVec2,
        range: Fx,
        claimed: &[Claim],
        intel: &Intel,
    ) -> bool {
        // Mines keep out of each other's reach (`can_place`), where each still
        // has about 80% of its circle. They stand on land only.
        d.distance(from) <= range
            && self.terrain.height_at(d) > self.terrain.water_level()
            && self.mine_in_the_way(d, m.reach, None).is_none()
            // A planned mine counts like a built one. Its site can stand well off
            // the deposit (the middle may be steep), so a check near the site
            // alone sent every idle builder back to the same deposit, one
            // think after another, and piled mines up around it.
            && !claimed.iter().any(|c| {
                c.pos.distance(d) < if c.mine { m.reach } else { Fx::from_int(32) }
            })
            && intel.enemy_start.is_none_or(|e| {
                d.distance(e) > Fx::from_int(480) || d.distance(from) < d.distance(e)
            })
            && !intel.danger.hot(d)
    }
}
