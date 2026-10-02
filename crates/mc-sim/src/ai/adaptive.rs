//! Capability-based decisions shared by current and future combat rosters.
use super::*;
use mc_data::MoveLayer;
use std::collections::BTreeMap;

const MAX_CONTACTS: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Contact {
    pub id: UnitId,
    pub blueprint: BlueprintId,
    pub pos: FxVec2,
    pub seen: u32,
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
        h.write_u64(self.contacts.len() as u64);
        for c in &self.contacts {
            h.write_u64(c.id.0 as u64 | (c.blueprint.0 as u64) << 32);
            h.write_i64(c.pos.x.0);
            h.write_i64(c.pos.y.0);
            h.write_u64(c.seen as u64);
        }
    }

    /// The Commander's operations, a line each, for headless reports.
    pub fn op_report(&self) -> Vec<String> {
        self.commander.op_lines()
    }

    /// A compact diagnostic for headless match reports.
    pub fn summary(&self) -> String {
        format!(
            "commander contacts={} {}",
            self.contacts.len(),
            self.commander.summary()
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
        // An upgrade's successor is not one more unit: counted, the factories
        // took the Masons upgrading in the field for new ones and made none.
        for row in self.state.units.slots.iter() {
            if self.state.units.owner[row] == player
                && !self.state.units.has_flag(row, flag::UPGRADE)
            {
                *counts.entry(self.state.units.blueprint[row]).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Distance plus three metres for each unit of defensive strength scouted
    /// around an objective: raiders change targets when an expansion is fortified.
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
        from.distance(target).floor_int() as i64 + risk * 3
    }

    /// How much the side wants a factory of `bp`'s domain: the forces its plans
    /// want, less for each factory of that domain it holds.
    pub(super) fn factory_domain_score(&self, player: u8, bp: &UnitBlueprint) -> i64 {
        let d = domain(bp);
        let shares = self.force_shares(player);
        let want = match d {
            0 => shares[0],
            1 => shares[1],
            _ => shares[2] + shares[3],
        };
        let held = self
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
        want * 100 / (1 + held)
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

    /// Split movement by hull and project destinations onto valid terrain. A future ship
    /// never inherits the land army's staging point or an inland artillery position.
    pub(super) fn route_ai_commands(&self, commands: Vec<Command>) -> Vec<Command> {
        let mut result = Vec::new();
        for command in commands {
            let (ids, target, attack, queue) = match command {
                Command::Move {
                    units,
                    target,
                    queue,
                } => (units, target, false, queue),
                Command::AttackMove {
                    units,
                    target,
                    queue,
                } => (units, target, true, queue),
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
                            queue,
                        }
                    } else {
                        Command::Move {
                            units: group.to_vec(),
                            target,
                            queue,
                        }
                    });
                }
            }
        }
        result
    }
}
