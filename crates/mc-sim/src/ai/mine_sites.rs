//! Where the AI's next core mine goes: ore first, then the open ground or sea
//! that keeps enough of a mine's reach to itself.
use super::*;

impl World {
    /// Nearest ore field with room for another mine, within `range` of `from`,
    /// staying off the enemy's doorstep until the army can contest it; with
    /// `bare`, else the open ground or sea where a mine gets at least that share
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
        let mine = self
            .blueprints
            .units
            .iter()
            .find(|b| b.mine.is_some() && b.tech == 1)?;
        let m = mine.mine?;
        // Mines may stand close, but split the ground between them: keep them
        // a reach apart, where each still has about 80% of its circle. A mine
        // at sea shares only with mines at sea and reaches farther, so it is
        // kept a sea reach from those and nowhere near the land's: held a land
        // reach off every mine, an island's own mines left it no sea to mine.
        let at_sea = |p: FxVec2| self.ore.at_sea(p);
        let units = &self.state.units;
        let crowded = |d: FxVec2| {
            let sea = at_sea(d);
            let spacing = m.reach_on(sea);
            units.slots.iter().any(|row| {
                self.bp(row).mine.is_some()
                    && units.pos[row].distance_sq(d) < spacing * spacing
                    && at_sea(units.pos[row]) == sea
            })
        };
        let open = |d: &FxVec2| {
            d.distance(from) <= range
                && !crowded(*d)
                // A planned mine counts like a built one. Its site can stand well off
                // the deposit (the middle may be steep), so a check near the site
                // alone sent every idle builder back to the same deposit, one
                // think after another, and piled mines up around it.
                && !claimed.iter().any(|c| {
                    let same = c.mine && at_sea(c.pos) == at_sea(*d);
                    c.pos.distance(*d) < if same { m.reach_on(at_sea(*d)) } else { Fx::from_int(32) }
                })
                && intel.enemy_start.is_none_or(|e| {
                    d.distance(e) > Fx::from_int(480) || d.distance(from) < d.distance(e)
                })
                && !intel.danger.hot(*d)
        };
        let ore = self
            .ore_centres()
            .into_iter()
            .filter(|d| open(d))
            .min_by_key(|d| (d.distance_sq(from), d.x, d.y));
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
            // The nearest few, the one that pays most for the walk: a mine out
            // at sea a little farther off can make three times a land one's gap.
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
}
