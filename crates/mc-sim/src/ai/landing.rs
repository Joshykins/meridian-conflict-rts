//! Landings: land units carried by lift ship and jumped through warp beside an
//! enemy's weak spot (`Gambit::Landing`).
//!
//! The AI never built a lift ship, and on an island map its land army stood at
//! home all game. Now a side holding the plan builds one (`lift_job`), and runs
//! one landing at a time (`Landing`):
//!
//! 1. **Board**: idle land units at the staging point walk up the ramp, as
//!    many as the hold takes, once there are enough to be worth a trip.
//! 2. **Jump**: with them aboard (or a minute gone), the ship jumps to a mark
//!    short of the target and out of any remembered dampener's field, sets down
//!    on the nearest ground that takes it and lets them out.
//! 3. **Ashore**: once the hold is empty, the cargo attack-moves on the target
//!    and the ship lifts off and jumps home.
//!
//! A small ship (a Courier) flanks: it goes for the mine or plant on the
//! enemy's outskirts least covered by guns and anti-air. A big one (a Bastion)
//! is an assault and goes for a factory. The plan's own score
//! (`strategy.rs`) is high on an island map, where nothing else gets the land
//! army to a fight.
use super::strategy::Gambit;
use super::*;
use crate::tables::WarpPhase;

/// Ticks units may take to board before the ship goes with what it has.
const BOARD_TICKS: u32 = 600;
/// Ticks with nothing aboard before a landing is called off.
const BOARD_GIVE_UP: u32 = 900;
/// Ticks a ship may take from leaving to an empty hold before the landing is
/// given up on (the cargo, if any, is let out where it is).
const JUMP_TICKS: u32 = 1800;
/// A hold at least this big makes an assault, not a flank.
const ASSAULT_ROOM: u16 = 24;
/// The ship comes out this far short of its target.
const LANDING_STANDOFF: Fx = Fx::from_int(380);
/// Guns within this of a landing target count against it.
const GUN_REACH: Fx = Fx::from_int(600);
/// Land units a side with no land route keeps at home through a landing.
const KEEP_HOME: usize = 4;
/// A lift ship needs this share of its health to go.
const SHIP_HEALTH: Fx = Fx::ratio(7, 10);
/// Seconds of the side's mass income, three fifths of it, a lift ship may cost.
const LIFT_SECONDS: i32 = 300;
/// Most lift ships a side keeps.
const MOST_LIFTS: usize = 2;

/// One landing under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Landing {
    pub ship: UnitId,
    /// 0 boarding, 1 jumping and setting down.
    pub phase: u8,
    pub target: FxVec2,
    pub since: u32,
}

impl Landing {
    pub(super) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.ship.0 as u64 | (self.phase as u64) << 32);
        h.write_i64(self.target.x.0);
        h.write_i64(self.target.y.0);
        h.write_u64(self.since as u64);
    }
}

impl World {
    /// A lift ship to build while the side holds `Gambit::Landing`: the biggest
    /// with a warp drive the builder makes and the income carries, while the
    /// side has none that big. A Courier early on is followed by a Bastion once
    /// it can be paid for: by then the army has outgrown the Courier's hold.
    pub(super) fn lift_job(
        &self,
        row: usize,
        census: &Census,
        start: FxVec2,
        facing: Angle,
    ) -> Option<Job> {
        let player = self.state.units.owner[row];
        // After the second factory: one built first cost a factory's worth of
        // production for a ship with nothing yet to carry.
        if !self.holds(player, Gambit::Landing)
            || census.lifts.len() >= MOST_LIFTS
            || census.factories.len() < 2
        {
            return None;
        }
        let lift = |bp: &UnitBlueprint| bp.transport.is_some() && bp.warp.is_some();
        if self
            .planned_sites(player)
            .any(|(_, o)| lift(self.blueprints.unit(o.blueprint)))
            || census.sites.iter().any(|&s| lift(self.bp(s)))
        {
            return None;
        }
        let pl = &self.state.players[player as usize];
        let budget = pl.mass_income * Fx::from_int(LIFT_SECONDS) * Fx::ratio(3, 5);
        let room = |id: BlueprintId| self.blueprints.unit(id).transport.map_or(0, |t| t.capacity);
        let blueprint = self
            .bp(row)
            .builder
            .as_ref()?
            .builds
            .iter()
            .copied()
            .filter(|&id| {
                let bp = self.blueprints.unit(id);
                lift(bp) && bp.cost_mass <= budget.max(Fx::from_int(200))
            })
            .max_by_key(|&id| (room(id), std::cmp::Reverse(id.0)))?;
        let held = census
            .lifts
            .iter()
            .map(|&r| room(self.state.units.blueprint[r]))
            .max()
            .unwrap_or(0);
        if held >= room(blueprint) {
            return None;
        }
        Some(Job {
            blueprint,
            near: start + FxVec2::from_angle(facing + Angle::HALF_TURN) * Fx::from_int(260),
            heading: self.blueprints.unit(blueprint).build_heading(),
            min_r: Fx::from_int(24),
            keep_off_deposits: true,
            place: Place::Around,
        })
    }

    /// Land units of the home guard a side with no land route keeps back, plus
    /// what its lift ships could carry, so landings have cargo (`theatre.rs`).
    pub(super) fn land_guard_cap(&self, player: u8, census: &Census) -> usize {
        let commander_lands = self.state.ai[player as usize]
            .commander
            .plan(super::commander::state::PlanKind::Landing)
            > super::commander::state::Stake::Off;
        if !self.holds(player, Gambit::Landing) && !commander_lands {
            return theatre::HOME_GUARD;
        }
        let room: usize = census
            .lifts
            .iter()
            .filter_map(|&r| self.bp(r).transport)
            .map(|t| t.capacity as usize / 2)
            .sum();
        theatre::HOME_GUARD + room.max(4)
    }

    /// Runs `player`'s landing: starts one with an idle lift ship and enough
    /// idle land units at `staging`, taken out of `army_idle`, and carries the
    /// one under way on.
    pub(super) fn direct_landing(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        start: FxVec2,
        staging: FxVec2,
        wave: usize,
        army_idle: &mut Vec<usize>,
        out: &mut Vec<Command>,
    ) {
        let op = self.state.ai[player as usize].landing;
        let op = match op {
            Some(op) => self.carry_landing(player, op, start, staging, out),
            None => None,
        };
        self.state.ai[player as usize].landing = op;
        if op.is_some() || !self.holds(player, Gambit::Landing) {
            return;
        }
        let units = &self.state.units;
        // An idle ship out in the field comes home for the next.
        for &r in &census.lifts {
            if units.order_head[r] == NO_ORDER && units.pos[r].distance(start) > FAR_FROM_HOME {
                out.push(Command::Move {
                    units: vec![units.id(r)],
                    target: staging,
                    queue: false,
                });
            }
        }
        let mut ships: Vec<usize> = census
            .lifts
            .iter()
            .copied()
            .filter(|&r| {
                units.order_head[r] == NO_ORDER
                    && units.warp[r].phase == WarpPhase::Idle
                    && units.health[r] >= self.bp(r).health * SHIP_HEALTH
                    && units.pos[r].distance(start) <= FAR_FROM_HOME
            })
            .collect();
        // The biggest hold first: a Courier takes none of a later army's tanks.
        ships.sort_by_key(|&r| {
            let room = self.bp(r).transport.map_or(0, |t| t.capacity);
            (std::cmp::Reverse(room), r)
        });
        let Some((ship, t, taken)) = ships.into_iter().find_map(|ship| {
            let t = self.bp(ship).transport?;
            let taken = self.cargo_for(ship, census, army_idle, start, staging);
            let filled: u16 = taken
                .iter()
                .map(|&r| self.bp(r).cargo_room().unwrap_or(0))
                .sum::<u16>()
                + self.cargo_used(ship);
            // Worth a trip: most of a small hold, or a wave's worth for a big one.
            let enough = (t.capacity * 2 / 3).min(wave as u16 * 2).max(2);
            (taken.len() >= 2 && filled >= enough).then_some((ship, t, taken))
        }) else {
            return;
        };
        let Some(target) = self.landing_target(player, intel, t.capacity >= ASSAULT_ROOM) else {
            return;
        };
        let carrier = units.id(ship);
        out.push(Command::Board {
            units: self.ids_of(&taken),
            carrier,
            queue: false,
        });
        army_idle.retain(|r| !taken.contains(r));
        self.state.ai[player as usize].landings += 1;
        self.state.ai[player as usize].landing = Some(Landing {
            ship: carrier,
            phase: 0,
            target,
            since: self.state.tick,
        });
    }

    /// One think of the landing `op`; `None` once it is over.
    fn carry_landing(
        &self,
        player: u8,
        op: Landing,
        start: FxVec2,
        staging: FxVec2,
        out: &mut Vec<Command>,
    ) -> Option<Landing> {
        let units = &self.state.units;
        // In warp it is out of the world (`IN_FACTORY`), not gone.
        let ship = units.row(op.ship)?;
        let tick = self.state.tick;
        let aboard = self.cargo_used(ship);
        let boarders: Vec<usize> = units
            .slots
            .iter()
            .filter(|&r| {
                units.owner[r] == player
                    && units.hangar[r] == Handle::NONE
                    && self
                        .state
                        .orders
                        .front(units, r)
                        .is_some_and(|o| o.kind == OrderKind::Board && o.target == op.ship)
            })
            .collect();
        let age = tick.saturating_sub(op.since);
        if op.phase == 0 {
            if aboard == 0 && age > BOARD_GIVE_UP {
                if !boarders.is_empty() {
                    out.push(Command::Move {
                        units: self.ids_of(&boarders),
                        target: staging,
                        queue: false,
                    });
                }
                return None;
            }
            let all_in = boarders.is_empty() && aboard > 0;
            if !(all_in || (age > BOARD_TICKS && aboard > 0)) {
                return Some(op);
            }
            if !boarders.is_empty() {
                out.push(Command::Move {
                    units: self.ids_of(&boarders),
                    target: staging,
                    queue: false,
                });
            }
            let from = units.pos[ship];
            let mark = self.safe_mark(
                player,
                offset_toward(op.target, from, LANDING_STANDOFF),
                from,
            );
            let id = vec![op.ship];
            // The drive not ready a minute on: it flies there instead.
            if self.can_jump(ship) {
                out.push(Command::Warp {
                    units: id.clone(),
                    pos: mark,
                    queue: false,
                });
            } else if age <= BOARD_TICKS * 2 {
                return Some(op);
            }
            out.push(Command::Land {
                units: id,
                pos: mark,
                unload: true,
                queue: self.can_jump(ship),
            });
            return Some(Landing {
                phase: 1,
                since: tick,
                ..op
            });
        }
        // Climbing to jump, in warp, or on its way down to let them out.
        let busy = units.warp[ship].phase != WarpPhase::Idle
            || self.state.orders.front(units, ship).is_some_and(|o| {
                matches!(
                    o.kind,
                    OrderKind::Warp | OrderKind::Unload | OrderKind::Land
                )
            });
        if aboard > 0 && busy && age <= JUMP_TICKS {
            return Some(op);
        }
        if aboard > 0 {
            // Stuck with its hold full: let them out where it is.
            out.push(Command::Unload {
                units: units
                    .slots
                    .iter()
                    .filter(|&r| units.hangar[r] == op.ship)
                    .map(|r| units.id(r))
                    .collect(),
            });
            return None;
        }
        // Ashore: those let out go for the target, the ship home.
        let ashore: Vec<usize> = units
            .slots
            .iter()
            .filter(|&r| {
                // Idle, or still walking off the ramp.
                units.owner[r] == player
                    && units.hangar[r] == Handle::NONE
                    && self
                        .state
                        .orders
                        .front(units, r)
                        .is_none_or(|o| o.kind == OrderKind::Move)
                    && units.pos[r].distance(units.pos[ship]) <= Fx::from_int(500)
                    && self.bp(r).cargo_room().is_some()
                    && !self.bp(r).weapons.is_empty()
            })
            .collect();
        if !ashore.is_empty() {
            out.push(Command::AttackMove {
                units: self.ids_of(&ashore),
                target: op.target,
                queue: false,
            });
        }
        out.push(Command::TakeOff {
            units: vec![op.ship],
        });
        let home = offset_toward(start, op.target, Fx::from_int(300));
        out.push(if self.can_jump(ship) {
            Command::Warp {
                units: vec![op.ship],
                pos: home,
                queue: true,
            }
        } else {
            Command::Move {
                units: vec![op.ship],
                target: home,
                queue: true,
            }
        });
        None
    }

    /// Idle land units at `staging` or home that fit `ship`, nearest it first,
    /// as many as its hold takes; a side with no land route keeps a few home.
    fn cargo_for(
        &self,
        ship: usize,
        census: &Census,
        army_idle: &[usize],
        start: FxVec2,
        staging: FxVec2,
    ) -> Vec<usize> {
        let units = &self.state.units;
        let Some(t) = self.bp(ship).transport else {
            return Vec::new();
        };
        let ship_pos = units.pos[ship];
        let mut keep = if census.land_route { 0 } else { KEEP_HOME };
        let mut cands: Vec<usize> = army_idle
            .iter()
            .copied()
            .filter(|&r| {
                self.cargo_fits(r, ship)
                    && (units.pos[r].distance(staging) <= Fx::from_int(600)
                        || units.pos[r].distance(start) <= HOME_RADIUS)
            })
            .collect();
        cands.sort_by_key(|&r| (units.pos[r].distance_sq(ship_pos), r));
        let mut room = t.capacity.saturating_sub(self.cargo_used(ship));
        let mut taken = Vec::new();
        for r in cands {
            let need = self.bp(r).cargo_room().unwrap_or(u16::MAX);
            if keep > 0 && census.home_guard.contains(&r) {
                keep -= 1;
                continue;
            }
            if need <= room {
                room -= need;
                taken.push(r);
            }
        }
        taken
    }

    /// Where a landing goes: for a flank, the enemy mine or plant least covered
    /// by guns, far from its start; for an assault, the factory least covered.
    /// With nothing seen, the enemy start.
    fn landing_target(&self, player: u8, intel: &Intel, assault: bool) -> Option<FxVec2> {
        let contacts = &self.state.ai[player as usize].contacts;
        let guns = |at: FxVec2| -> i64 {
            contacts
                .iter()
                .filter(|c| c.pos.distance(at) <= GUN_REACH)
                .map(|c| self.blueprints.unit(c.blueprint))
                .filter(|bp| !bp.weapons.is_empty())
                .map(adaptive::strength)
                .sum()
        };
        let best = contacts
            .iter()
            .filter(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                if assault {
                    bp.has(cat::FACTORY)
                } else {
                    bp.has(cat::EXTRACTOR) || bp.has(cat::POWER)
                }
            })
            .map(|c| {
                let home = self
                    .enemy_start_near(player, c.pos)
                    .map_or(0, |s| s.distance(c.pos).floor_int() as i64);
                let aa = self.anti_air_near(player, c.pos, GUN_REACH);
                let cost = guns(c.pos) * 2 + aa * 4 - if assault { home / 8 } else { home / 2 };
                (cost, c.pos)
            })
            .min_by_key(|&(cost, p)| (cost, p.x, p.y))
            .map(|(_, p)| p);
        best.or(intel.enemy_start)
    }
}
