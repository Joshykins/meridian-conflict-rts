//! The orders phase's loop (`World::run_orders`): every unit's front order advanced,
//! row by row, out of `orders.rs`, whose private helpers it shares.

use super::*;

impl World {
    pub(crate) fn run_orders(&mut self) -> Result<(), SimError> {
        {
            let _span = mc_core::perf_span!("orders.rollouts");
            self.run_rollouts()?;
            self.run_batches()?;
        }
        let fights = {
            let _span = mc_core::perf_span!("orders.fight_lines");
            self.fight_lines()
        };
        self.list_chasers();
        // Units spawned while orders run start acting next tick.
        let rows = self.state.units.slots.rows();
        self.speculate_air_picks();
        for row in 0..rows {
            let before = self.chase_state(row);
            self.run_orders_row(row, &fights)?;
            self.note_chase_change(before, self.chase_state(row));
        }
        Ok(())
    }

    /// Advances `row`'s front order (`run_orders`).
    fn run_orders_row(
        &mut self,
        row: usize,
        fights: &crate::fight_line::FightLines,
    ) -> Result<(), SimError> {
        if !self.state.units.slots.is_alive(row) || !self.state.units.is_active(row) {
            return Ok(());
        }
        if self.state.units.drone_parent[row] != Handle::NONE {
            return Ok(());
        }
        // Stunned: its orders wait for it (`warp.rs`).
        if self.stunned(row) {
            return Ok(());
        }
        let Some(o) = self.state.orders.front(&self.state.units, row).copied() else {
            // Looked at a few times a second, spread over the rows: a sky full of fighters seeing an army at once
            // would otherwise all search on one tick.
            if self.is_air(row)
                && !self.bp(row).weapons.is_empty()
                && !self.state.units.has_flag(row, flag::PASSIVE)
                && (self.state.tick as usize + row).is_multiple_of(4)
            {
                // An idle fighter scrambles a little before the enemy is in its own
                // sight, once radar or a friend has it: parked, it would otherwise
                // still be getting up to speed when an attack reaches it.
                let bp = self.bp(row);
                let scramble = if bp.has(cat::ANTI_AIR) {
                    bp.vision * Fx::ratio(3, 2)
                } else {
                    bp.vision
                };
                let span = mc_core::perf_span!("orders.scramble");
                let found = self.air_engage_target_within(row, scramble);
                drop(span);
                if let Some(target) = found {
                    // Idle armed aircraft launch a flight before using their weapons.
                    let mut home = order(
                        OrderKind::AttackMove,
                        self.state.units.pos[row],
                        Handle::NONE,
                    );
                    home.heading = self.state.units.heading[row];
                    self.give(row, home, false)?;
                    self.air_fight(row, target)?;
                    return Ok(());
                }
            }
            // An aircraft that orbits, left with nothing to do, circles where it is.
            if let Some(radius) = self
                .bp(row)
                .orbit
                .filter(|_| self.state.units.hangar[row] == Handle::NONE)
            {
                let mut o = order(OrderKind::Guard, self.state.units.pos[row], Handle::NONE);
                o.radius = radius;
                self.give(row, o, false)?;
                return Ok(());
            }
            // An engineering station left with nothing to do works its whole reach:
            // it guards its own spot, so raising, mending and reclaiming round it
            // are taken up as they turn up (`area_work.rs`).
            if self.bp(row).is_station() {
                let mut o = order(OrderKind::Guard, self.state.units.pos[row], Handle::NONE);
                o.radius = self.work_range(row);
                self.give(row, o, false)?;
                return Ok(());
            }
            if self.state.units.has_flag(row, flag::HAS_FIELD) {
                self.stop_moving(row);
            }
            let _span = mc_core::perf_span!("orders.idle");
            if self.idle_air_land(row)? {
                return Ok(());
            }
            if !self.idle_repair(row)? {
                self.idle_reclaim(row)?;
            }
            return Ok(());
        };
        match o.kind {
            OrderKind::Move | OrderKind::AttackMove => {
                let _span = mc_core::perf_span!("orders.move");
                self.run_move(row, &o, fights)?
            }
            OrderKind::Attack => {
                let _span = mc_core::perf_span!("orders.attack");
                self.run_attack(row, &o)?
            }
            OrderKind::Build => self.run_build(row, &o)?,
            OrderKind::Assist => self.run_assist(row, &o)?,
            OrderKind::Reclaim => self.run_reclaim(row, &o)?,
            OrderKind::ReclaimUnit => self.run_reclaim_unit(row, &o)?,
            // An elevator's queue is not built: its platform brings it down (`elevator.rs`).
            OrderKind::Produce if self.bp(row).elevator.is_some() => {}
            OrderKind::Produce => self.run_produce(row, &o)?,
            OrderKind::Upgrade => self.run_upgrade(row, &o)?,
            OrderKind::AttackGround | OrderKind::Strike | OrderKind::Bombard => {
                self.run_attack_ground(row, &o)?
            }
            OrderKind::Patrol => self.run_patrol(row, &o)?,
            OrderKind::Guard => self.run_guard(row, &o)?,
            OrderKind::Board => self.run_board(row, &o)?,
            OrderKind::Land | OrderKind::Unload => self.run_land(row, &o)?,
            OrderKind::Warp => self.run_warp_order(row, &o)?,
            OrderKind::Dash => self.run_dash_order(row, &o)?,
            OrderKind::ReclaimArea => self.run_reclaim_area(row, &o)?,
            OrderKind::Stage => self.run_stage(row, &o)?,
        }
        // A salvage vehicle, boat or aircraft clears what it passes as it goes.
        if matches!(
            o.kind,
            OrderKind::Move | OrderKind::AttackMove | OrderKind::Patrol | OrderKind::Guard
        ) && self.bp(row).reclaimer.is_some_and(|r| r.mobile)
            && self.state.units.slots.is_alive(row)
        {
            self.idle_reclaim(row)?;
        }
        Ok(())
    }
}
