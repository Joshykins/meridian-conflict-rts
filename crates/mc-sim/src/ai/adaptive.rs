//! Capability-based decisions shared by current and future combat rosters.
use super::*;
use mc_data::MoveLayer;
use std::collections::BTreeMap;

const MAX_CONTACTS: usize = 256;

/// Where hurt or outmatched units at `pos` fall back to: short of the start on
/// their own side. The start itself stands among the base's buildings; units
/// sent there crowded short of it with a move they never finished, so they were
/// never idle again and never rejoined the army.
fn fall_back_to(start: FxVec2, pos: FxVec2) -> FxVec2 {
    offset_toward(start, pos, Fx::from_int(220))
}

/// Units this close to their start have fallen back already.
const FALL_BACK_NEAR: Fx = Fx::from_int(260);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Contact {
    pub id: UnitId,
    pub blueprint: BlueprintId,
    pub pos: FxVec2,
    pub seen: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Recovery {
    pub id: UnitId,
    pub until: u32,
}

/// Physical movement domain for units; production domain for structures.
pub(super) fn domain(bp: &UnitBlueprint) -> usize {
    match bp.motion.map(|m| m.layer) {
        Some(MoveLayer::Air) => 1,
        Some(MoveLayer::Naval) => 2,
        Some(_) => 0,
        None if bp.has(cat::AIR) => 1,
        None if bp.has(cat::NAVAL) || bp.water_build => 2,
        _ => 0,
    }
}

pub(super) fn strength(bp: &UnitBlueprint) -> i64 {
    // Commander replacement cost is an economic sentinel, not its combat strength.
    let cost = if bp.has(cat::COMMANDER) {
        400
    } else {
        bp.cost_mass.floor_int() as i64
    };
    (cost + bp.health.floor_int() as i64 / 10).max(1)
}

impl AiState {
    pub(super) fn hash_adaptation(&self, h: &mut StateHasher) {
        self.config.hash(h);
        h.write_u64(self.next_tactical_tick as u64);
        h.write_u64(self.contacts.len() as u64);
        for c in &self.contacts {
            h.write_u64(c.id.0 as u64 | (c.blueprint.0 as u64) << 32);
            h.write_i64(c.pos.x.0);
            h.write_i64(c.pos.y.0);
            h.write_u64(c.seen as u64);
        }
        h.write_u64(self.recovering.len() as u64);
        for r in &self.recovering {
            h.write_u64(r.id.0 as u64 | (r.until as u64) << 32);
        }
    }

    /// A compact diagnostic for headless match reports.
    pub fn summary(&self) -> String {
        format!(
            "stance={} waves={} raids={} contacts={} recovering={} production={}",
            self.stance,
            self.waves,
            self.raids,
            self.contacts.len(),
            self.recovering.len(),
            self.production_counter
        )
    }
}

impl World {
    pub(super) fn remember_enemies(&mut self, player: u8) {
        let tick = self.state.tick;
        let mask = self.team_mask(player);
        let config = self.state.ai[player as usize].config;
        let mut seen = Vec::new();
        for row in self.state.units.slots.iter() {
            if !self.are_enemies(player, self.state.units.owner[row])
                || self.state.units.has_flag(row, flag::IN_FACTORY)
                || !self.detects(player, row)
            {
                continue;
            }
            let id = self.state.units.id(row);
            // A radar blip that has never been identified is not a unit blueprint.
            if self.state.fog_enabled && !self.fog.is_identified(row, id.generation(), mask) {
                continue;
            }
            seen.push(Contact {
                id,
                blueprint: self.state.units.blueprint[row],
                pos: self.state.units.pos[row],
                seen: tick,
            });
        }
        let mut contacts = std::mem::take(&mut self.state.ai[player as usize].contacts);
        contacts.retain(|c| {
            tick.saturating_sub(c.seen) <= config.memory_ticks()
                && (seen.iter().any(|s| s.id == c.id)
                    || (self.state.fog_enabled && !self.fog.is_visible(c.pos, mask)))
        });
        for c in seen {
            if let Some(old) = contacts.iter_mut().find(|old| old.id == c.id) {
                *old = c;
            } else {
                contacts.push(c);
            }
        }
        // Keep fresh information, with stable ties. Memory and CPU cost remain bounded.
        contacts.sort_by_key(|c| (std::cmp::Reverse(c.seen), c.id));
        contacts.truncate(MAX_CONTACTS);
        self.state.ai[player as usize].contacts = contacts;
        let recovered: Vec<_> = self.state.ai[player as usize]
            .recovering
            .iter()
            .filter(|r| {
                self.state.units.row(r.id).is_some_and(|row| {
                    tick < r.until
                        && self.state.units.health[row] < self.bp(row).health * Fx::ratio(7, 10)
                })
            })
            .cloned()
            .collect();
        self.state.ai[player as usize].recovering = recovered;
    }

    pub(super) fn ai_personality(&self, player: u8, census: &Census, intel: &Intel) -> Personality {
        match self.state.ai[player as usize].config.doctrine {
            Doctrine::Aggressive => Personality::Aggressive,
            Doctrine::Economic => Personality::Expander,
            Doctrine::Defensive => Personality::Turtle,
            Doctrine::Adaptive => {
                if !intel.threats.is_empty() && intel.enemy_army > census.army {
                    Personality::Turtle
                } else if census.army >= intel.enemy_army + 6 {
                    Personality::Aggressive
                } else {
                    Personality::Expander
                }
            }
        }
    }

    pub(super) fn ai_composition(&self, player: u8) -> BTreeMap<BlueprintId, usize> {
        let mut counts = BTreeMap::new();
        for row in self.state.units.slots.iter() {
            if self.state.units.owner[row] == player {
                *counts.entry(self.state.units.blueprint[row]).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Rank every legal combat blueprint; no roster keys, tank/ship lists, or tier cycles.
    pub(super) fn choose_combat_unit(
        &self,
        player: u8,
        candidates: &[BlueprintId],
        counts: &BTreeMap<BlueprintId, usize>,
        stance: Stance,
        serial: u32,
    ) -> Option<BlueprintId> {
        let ai = &self.state.ai[player as usize];
        let mut targets = BTreeMap::from([(cat::LAND, 10i64), (cat::LAND | cat::STRUCTURE, 4)]);
        let mut fortifications = 0;
        for c in &ai.contacts {
            let bp = self.blueprints.unit(c.blueprint);
            let age = self.state.tick.saturating_sub(c.seen);
            let confidence = (ai.config.memory_ticks().saturating_sub(age) * 100
                / ai.config.memory_ticks())
            .max(10) as i64;
            let weight = strength(bp).min(2000) * confidence / 100;
            *targets.entry(bp.target_categories()).or_insert(0) += weight;
            if bp.has(cat::DEFENSE) {
                fortifications += 1;
            }
        }
        let total: i64 = targets.values().sum();
        candidates
            .iter()
            .copied()
            .filter_map(|id| {
                let bp = self.blueprints.unit(id);
                let bias = ai.config.domain_weights[domain(bp)] as i64;
                if bias == 0 || bp.motion.is_none() || bp.has(cat::SCOUT) {
                    return None;
                }
                if bp.weapons.is_empty() {
                    let armed: usize = counts
                        .iter()
                        .filter(|(id, _)| {
                            !self.blueprints.unit(**id).weapons.is_empty()
                                && self.blueprints.unit(**id).is_mobile()
                        })
                        .map(|(_, n)| *n)
                        .sum();
                    let support: usize = counts
                        .iter()
                        .filter(|(id, _)| {
                            let u = self.blueprints.unit(**id);
                            u.is_mobile()
                                && u.weapons.is_empty()
                                && !u.has(cat::ENGINEER)
                                && !u.has(cat::SCOUT)
                        })
                        .map(|(_, n)| *n)
                        .sum();
                    if armed < 8 || support >= (armed / 10).max(1) {
                        return None;
                    }
                    return Some((id, 180 * bias / 100));
                }
                let mut effectiveness = 0i64;
                for weapon in &bp.weapons {
                    let covered: i64 = targets
                        .iter()
                        .filter(|(mask, _)| weapon.target_mask & **mask != 0)
                        .map(|(_, weight)| *weight)
                        .sum();
                    let dps = weapon.damage.floor_int() as i64
                        * weapon.salvo.max(1) as i64
                        * weapon.salvo_batch.max(1) as i64
                        * 10
                        / weapon.reload_ticks.max(1) as i64;
                    effectiveness += dps * covered * 100 / total.max(1);
                }
                // A torpedo bomber is no use until there are hulls on the water to hunt.
                if effectiveness == 0
                    && domain(bp) == 1
                    && bp.weapons.iter().all(|w| w.target_mask & !cat::NAVAL == 0)
                {
                    return None;
                }
                let adaptation = ai.config.adaptation as i64;
                let cost = bp.cost_mass.floor_int().max(20) as i64;
                let mut score = 100 + (effectiveness * 100 / cost).min(800) * adaptation / 100;
                // Preserve a useful combined-arms force; duplicate types have diminishing utility.
                let existing = counts.get(&id).copied().unwrap_or(0) as i64;
                score = score * 6 / (6 + existing);
                if bp.has(cat::ARTILLERY) && matches!(stance, Stance::Firebase | Stance::Push) {
                    score += 55;
                }
                if bp.has(cat::ARTILLERY) {
                    let range = bp
                        .weapons
                        .iter()
                        .map(|w| w.range_max.floor_int())
                        .max()
                        .unwrap_or(0) as i64;
                    score += fortifications.min(6) * range.min(800) * adaptation / 1000;
                }
                if bp.has(cat::SCOUT) {
                    score /= 3;
                }
                if stance == Stance::Raid {
                    score += bp.motion.map_or(0, |m| m.speed.floor_int() as i64).min(100);
                }
                let pl = &self.state.players[player as usize];
                if bp.cost_energy > (pl.energy + pl.energy_income * 20).max(Fx::from_int(200)) {
                    score /= 3;
                }
                if bp.cost_mass > (pl.mass + pl.mass_income * 20).max(Fx::from_int(100)) {
                    score /= 3;
                }
                score = score * bias / 100;
                // Small stable variation, not random noise that can override a needed counter.
                let tie = (id.0 as u32)
                    .wrapping_mul(1664525)
                    .wrapping_add(serial.wrapping_mul(1013904223))
                    .wrapping_add(player as u32 * 97)
                    % 17;
                Some((id, score + tie as i64))
            })
            .max_by_key(|(id, score)| (*score, std::cmp::Reverse(id.0)))
            .map(|(id, _)| id)
    }

    /// Distance plus the defensive investment actually scouted around an objective.
    /// This gives raiders a reason to change targets when an expansion is fortified.
    pub(super) fn ai_objective_cost(&self, player: u8, target: FxVec2, from: FxVec2) -> i64 {
        let ai = &self.state.ai[player as usize];
        let risk: i64 = ai
            .contacts
            .iter()
            .filter(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                !bp.weapons.is_empty() && c.pos.distance(target) < Fx::from_int(420)
            })
            .map(|c| strength(self.blueprints.unit(c.blueprint)))
            .sum();
        from.distance(target).floor_int() as i64 + risk * ai.config.adaptation as i64 / 25
    }

    pub(super) fn factory_domain_score(&self, player: u8, bp: &UnitBlueprint) -> i64 {
        let d = domain(bp);
        let ai = &self.state.ai[player as usize];
        let count = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| {
                self.state.units.owner[r] == player
                    && self.bp(r).has(cat::FACTORY)
                    && domain(self.bp(r)) == d
            })
            .count() as i64;
        let demand = ai
            .contacts
            .iter()
            .filter(|c| domain(self.blueprints.unit(c.blueprint)) == d)
            .count() as i64;
        (ai.config.domain_weights[d] as i64 + demand.min(20) * ai.config.adaptation as i64 / 10)
            / (1 + count)
    }

    pub(super) fn visible_air_target(&self, player: u8, from: FxVec2) -> Option<FxVec2> {
        self.state
            .units
            .slots
            .iter()
            .filter(|&r| {
                self.are_enemies(player, self.state.units.owner[r])
                    && self.detects(player, r)
                    && self.bp(r).motion.is_some_and(|m| m.layer == MoveLayer::Air)
            })
            .map(|r| self.state.units.pos[r])
            .min_by_key(|p| (p.distance_sq(from), p.x, p.y))
    }

    pub(super) fn react_tactically(
        &mut self,
        player: u8,
        census: &mut Census,
        intel: &Intel,
        out: &mut Vec<Command>,
    ) {
        let ai = &self.state.ai[player as usize];
        if self.state.tick < ai.next_tactical_tick {
            return;
        }
        let config = ai.config;
        let start = self.state.players[player as usize].start;
        let contacts = ai.contacts.clone();
        if let Some(row) = self
            .state
            .units
            .row(self.state.players[player as usize].commander)
        {
            let bp = self.bp(row);
            let pos = self.state.units.pos[row];
            let danger = contacts.iter().any(|c| {
                c.seen == self.state.tick
                    && c.pos.distance(pos) < Fx::from_int(450)
                    && !self.blueprints.unit(c.blueprint).weapons.is_empty()
            });
            if danger
                && self.state.units.health[row] * 100
                    < bp.health * config.retreat_health.max(40) as i32
            {
                let repair = census
                    .builders_idle
                    .iter()
                    .copied()
                    .filter(|&r| r != row)
                    .map(|r| self.state.units.pos[r])
                    .min_by_key(|p| p.distance_sq(start))
                    .unwrap_or(start);
                out.push(Command::Move {
                    units: vec![self.state.units.id(row)],
                    target: repair,
                    queue: false,
                });
            }
        }
        let mut withdrawn = Vec::new();
        let mut outmatched_rows = Vec::new();
        let mut defenders = Vec::new();
        // Bounded reaction cadence avoids cancelling movement/volleys on every think.
        let offset = (self.state.tick as usize / config.think_period().max(40) as usize * 128)
            % census.combat_rows.len().max(1);
        for &row in census
            .combat_rows
            .iter()
            .cycle()
            .skip(offset)
            .take(census.combat_rows.len().min(128))
        {
            let bp = self.bp(row);
            let pos = self.state.units.pos[row];
            if self.state.ai[player as usize]
                .recovering
                .iter()
                .any(|r| r.id == self.state.units.id(row))
            {
                continue;
            }
            let hostile: Vec<_> = contacts
                .iter()
                .filter(|c| {
                    c.seen == self.state.tick
                        && c.pos.distance(pos) < Fx::from_int(440)
                        && self
                            .blueprints
                            .unit(c.blueprint)
                            .weapons
                            .iter()
                            .any(|w| w.target_mask & bp.target_categories() != 0)
                })
                .collect();
            let hurt =
                self.state.units.health[row] * 100 < bp.health * config.retreat_health as i32;
            let enemy_power: i64 = hostile
                .iter()
                .map(|c| strength(self.blueprints.unit(c.blueprint)))
                .sum();
            let friendly_power: i64 = if hostile.is_empty() {
                0
            } else {
                census
                    .combat_rows
                    .iter()
                    .copied()
                    .filter(|&r| self.state.units.pos[r].distance(pos) < Fx::from_int(440))
                    .map(|r| strength(self.bp(r)))
                    .sum()
            };
            let outmatched = config.difficulty != Difficulty::Easy
                && enemy_power > friendly_power * 2
                && pos.distance(start) > HOME_RADIUS;
            if !hostile.is_empty() && outmatched && !hurt {
                // Fall back with the others, below, not one by one.
                outmatched_rows.push(row);
                withdrawn.push(row);
            } else if !hostile.is_empty() && hurt && pos.distance(start) > FALL_BACK_NEAR {
                // Already home, it stays and fights: there is nowhere to fall back to.
                let target = self.nav.nearest_passable(
                    bp.motion.unwrap().layer,
                    bp.motion.unwrap().size_class,
                    fall_back_to(start, pos),
                );
                if let Some(target) = target {
                    out.push(Command::Move {
                        units: vec![self.state.units.id(row)],
                        target,
                        queue: false,
                    });
                    withdrawn.push(row);
                }
            } else if let Some(&(_, enemy)) = intel.threats.first() {
                // Recall nearby fighting units as well as idle ones; air-only weapons stay on air defense.
                if pos.distance(start) < Fx::from_int(1100)
                    && self.can_engage_at(row, player, enemy)
                {
                    defenders.push(row);
                }
            }
        }
        // An outmatched group falls back as a group: units that break off one
        // at a time are caught and shot in the back.
        let mut groups: Vec<(FxVec2, Vec<usize>)> = Vec::new();
        for &row in &outmatched_rows {
            let pos = self.state.units.pos[row];
            match groups
                .iter_mut()
                .find(|(seed, _)| seed.distance(pos) < Fx::from_int(600))
            {
                Some((_, rows)) => rows.push(row),
                None => groups.push((pos, vec![row])),
            }
        }
        for (seed, rows) in groups {
            let bp = self.bp(rows[0]);
            let Some(target) = self.nav.nearest_passable(
                bp.motion.unwrap().layer,
                bp.motion.unwrap().size_class,
                fall_back_to(start, seed),
            ) else {
                continue;
            };
            for chunk in rows.chunks(MAX_COMMAND_UNITS) {
                out.push(Command::Move {
                    units: chunk.iter().map(|&r| self.state.units.id(r)).collect(),
                    target,
                    queue: false,
                });
            }
        }
        for &r in &withdrawn {
            self.state.ai[player as usize].recovering.push(Recovery {
                id: self.state.units.id(r),
                until: self.state.tick + 450,
            });
        }
        if let Some(&(_, enemy)) = intel.threats.first() {
            defenders.sort_by_key(|&r| (self.state.units.pos[r].distance_sq(enemy), r));
            let reserve = if enemy.distance(start) < Fx::from_int(420) {
                defenders.len()
            } else {
                (census.combat_rows.len() / 3).clamp(3, 12)
            };
            defenders.truncate(reserve);
            // Units already fighting near the incursion retain their orders and volleys.
            defenders.retain(|&r| {
                self.state.units.order_head[r] == NO_ORDER
                    || self.state.units.pos[r].distance(enemy) > Fx::from_int(300)
            });
            for group in defenders.chunks(MAX_COMMAND_UNITS) {
                out.push(Command::AttackMove {
                    units: group.iter().map(|&r| self.state.units.id(r)).collect(),
                    target: enemy,
                    queue: false,
                });
            }
        }
        let available = |r: &usize| !withdrawn.contains(r) && !defenders.contains(r);
        census.army_idle.retain(available);
        census.artillery_idle.retain(available);
        census.bombers_idle.retain(available);
        census.interceptors_idle.retain(available);
        census.naval_idle.retain(available);
        self.state.ai[player as usize].next_tactical_tick =
            self.state.tick + config.think_period().max(40);
    }

    fn can_engage_at(&self, row: usize, player: u8, pos: FxVec2) -> bool {
        self.state.units.slots.iter().any(|enemy| {
            self.state.units.pos[enemy].distance(pos) < Fx::from_int(80)
                && self.are_enemies(player, self.state.units.owner[enemy])
                && self.detects(player, enemy)
                && self
                    .bp(row)
                    .weapons
                    .iter()
                    .any(|w| w.target_mask & self.bp(enemy).target_categories() != 0)
        })
    }

    /// Split movement by hull and project destinations onto valid terrain. A future ship
    /// never inherits the land army's staging point or an inland artillery position.
    pub(super) fn route_ai_commands(&self, commands: Vec<Command>) -> Vec<Command> {
        let mut result = Vec::new();
        for command in commands {
            let (ids, target, attack) = match command {
                Command::Move { units, target, .. } => (units, target, false),
                Command::AttackMove { units, target, .. } => (units, target, true),
                c => {
                    result.push(c);
                    continue;
                }
            };
            let mut groups: BTreeMap<(i64, i64), Vec<UnitId>> = BTreeMap::new();
            for id in ids {
                let Some(row) = self.state.units.row(id) else {
                    continue;
                };
                let Some(m) = self.bp(row).motion else {
                    continue;
                };
                if let Some(p) = self.nav.nearest_passable(m.layer, m.size_class, target) {
                    groups.entry((p.x.0, p.y.0)).or_default().push(id);
                }
            }
            for ((x, y), ids) in groups {
                for group in ids.chunks(MAX_COMMAND_UNITS) {
                    let target = FxVec2::new(Fx(x), Fx(y));
                    result.push(if attack {
                        Command::AttackMove {
                            units: group.to_vec(),
                            target,
                            queue: false,
                        }
                    } else {
                        Command::Move {
                            units: group.to_vec(),
                            target,
                            queue: false,
                        }
                    });
                }
            }
        }
        result
    }
}
