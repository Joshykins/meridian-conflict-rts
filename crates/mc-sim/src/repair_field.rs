//! Nanite repair fields (`UnitBlueprint::repair_field`): a unit that carries one mends
//! every finished friendly unit within its radius, ground or air, structure or mobile,
//! a share of that unit's full health a second. It costs nothing to run and takes no
//! orders; two fields over the same unit both mend it. The unit carrying it is left to
//! its own regen.

use crate::spatial::kind;
use crate::World;
use mc_core::{Fx, TICKS_PER_SECOND};

impl World {
    pub(crate) fn run_repair_fields(&mut self) {
        let units = &self.state.units;
        let mut mend: Vec<(usize, Fx)> = Vec::new();
        for row in units.slots.iter() {
            let Some(field) = self.bp(row).repair_field else {
                continue;
            };
            if !units.is_active(row) || units.health[row] <= Fx::ZERO {
                continue;
            }
            let (pos, owner) = (units.pos[row], units.owner[row]);
            self.index
                .query(pos, field.radius, kind::UNIT | kind::AIRCRAFT, |e| {
                    let t = e.row as usize;
                    if t == row
                        || !self.unit_entry_is_current(e)
                        || !units.is_active(t)
                        || units.health[t] <= Fx::ZERO
                        || e.pos.distance(pos) > field.radius
                        || self.are_enemies(owner, units.owner[t])
                    {
                        return true;
                    }
                    let max = self.unit_max_health(t);
                    if units.health[t] < max {
                        mend.push((t, max * field.rate / TICKS_PER_SECOND as i32));
                    }
                    true
                });
        }
        for (t, amount) in mend {
            let max = self.unit_max_health(t);
            let health = &mut self.state.units.health[t];
            *health = (*health + amount).min(max);
        }
    }
}
