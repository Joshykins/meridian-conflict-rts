//! Nuclear blasts running out (`docs/NUKES.md`): a warhead's or a commander's, announced
//! (`detonate`), its front run out over the ticks after.

use mc_core::{Fx, FxVec3};
use mc_data::strategic::{NuclearBlast, StrategicLook};

use super::Detonation;
use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::{flag, UnitId};
use crate::{SimError, World};

/// Trees die out to this share of a blast's damage radius (the thermal pulse carries
/// further than the pressure that breaks armour).
const TREE_REACH: Fx = Fx::ratio(13, 10);

impl World {
    /// A nuclear blast at `pos`: announced now, its front run out over the next ticks.
    pub(crate) fn detonate(
        &mut self,
        pos: FxVec3,
        blast: NuclearBlast,
        owner: u8,
        source: UnitId,
        commander: bool,
        look: StrategicLook,
    ) -> Result<(), SimError> {
        self.events.push(SimEvent::NuclearDetonation {
            pos,
            radius: blast.radius,
            owner,
            commander,
            look,
        });
        self.state.strategic.detonations.push(Detonation {
            pos,
            owner,
            source,
            radius: blast.radius,
            core: blast.core,
            damage: blast.damage,
            edge: blast.edge,
            front_ticks: blast.front_ticks,
            age: 0,
            reached: Fx::ZERO,
            hit: Vec::new(),
            screens: Vec::new(),
        });
        // The first ring lands the tick it forms.
        self.run_detonation(self.state.strategic.detonations.len() - 1);
        Ok(())
    }

    pub(super) fn run_detonations(&mut self) -> Result<(), SimError> {
        let mut i = 0;
        while i < self.state.strategic.detonations.len() {
            let d = &mut self.state.strategic.detonations[i];
            d.age += 1;
            if d.age > d.front_ticks {
                self.state.strategic.detonations.swap_remove(i);
                continue;
            }
            self.run_detonation(i);
            i += 1;
        }
        Ok(())
    }

    /// Everything the front of detonation `i` reaches this tick.
    fn run_detonation(&mut self, i: usize) {
        let d = &self.state.strategic.detonations[i];
        let blast = d.blast();
        let front = blast.front_at(d.age);
        // Trees go at the same pace, out to further than armour breaks.
        let tree_front = front * TREE_REACH;
        let (center, from, owner, source) = (d.pos.xy(), d.reached, d.owner, d.source);
        let origin = d.pos;
        let mut victims = Vec::new();
        self.index.query(center, front, kind::UNIT, |e| {
            let row = e.row as usize;
            if self.unit_entry_is_current(e) && !self.state.units.has_flag(row, flag::IN_FACTORY) {
                victims.push(row);
            }
            true
        });
        victims.sort_unstable();
        victims.dedup();
        let mut charged = Vec::new();
        for row in victims {
            let id = self.state.units.id(row);
            if self.state.strategic.detonations[i].hit.contains(&id) {
                continue;
            }
            let radius = self.hull_radius(row);
            let height = self.bp(row).height;
            let reach = (self.state.units.pos[row].distance(center) - radius).max(Fx::ZERO);
            if reach > front {
                continue;
            }
            self.state.strategic.detonations[i].hit.push(id);
            let damage = blast.damage_at(reach);
            let target = self.state.units.pos[row].extend(self.state.units.z[row] + height / 2);
            // A dome takes what it can hold and the rest goes through: nothing that
            // stands against a warhead's middle survives it, dome or no dome.
            let screen = self.state.strategic.detonations[i]
                .screens
                .iter()
                .find(|&&(c, r, _)| {
                    use crate::shields::in_dome;
                    let floor = self.dome_floor();
                    in_dome(target, c, r, floor) && !in_dome(origin, c, r, floor)
                });
            let damage = if let Some(&(_, _, held)) = screen {
                damage - held
            } else if let Some(shield) = self.blast_blocker(origin, target, Some(row), None) {
                let held = if self.state.units.has_flag(shield, flag::INVULNERABLE) {
                    Fx::MAX
                } else {
                    self.state.units.shield_hp[shield]
                };
                // A dome takes the blast once (a hull field once per tick it is reached, as
                // it covers only its own hull); later ticks of the front find its screen.
                if self.bp(shield).shield.is_some_and(|s| !s.is_hull()) {
                    let c = self.state.units.pos[shield].extend(self.state.units.z[shield]);
                    let r = self.dome_radius(shield);
                    self.state.strategic.detonations[i]
                        .screens
                        .push((c, r, held));
                }
                if !charged.contains(&shield) {
                    charged.push(shield);
                    self.damage_shield(shield, damage);
                }
                damage - held.min(damage)
            } else {
                damage
            };
            if damage <= Fx::ZERO {
                continue;
            }
            let by = if self.are_enemies(owner, self.state.units.owner[row]) {
                owner
            } else {
                u8::MAX
            };
            self.damage_unit(row, damage, by, source);
        }
        // Wrecks go under the front once, as it passes them.
        self.wear_wrecks(origin, front, |reach| {
            if reach > from {
                blast.damage_at(reach)
            } else {
                Fx::ZERO
            }
        });
        // City blocks go under the front as it passes them, dome or no dome.
        let blow = crate::city::Blow {
            share: crate::city::NUCLEAR_SHARE,
            ignite: false,
            domes: false,
        };
        self.blast_structures(origin, front, blow, |reach| {
            if reach >= from {
                blast.damage_at(reach)
            } else {
                Fx::ZERO
            }
        });
        let tree_from = from * TREE_REACH;
        let mut felled = Vec::new();
        self.prop_index.query(center, tree_front, kind::PROP, |e| {
            let prop = e.row as usize;
            let dist = self.map.props[prop].pos.distance(center);
            if dist >= tree_from && dist <= tree_front {
                felled.push(prop);
            }
            true
        });
        for prop in felled {
            if self.is_scenery_tree(prop) {
                self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            }
        }
        let d = &mut self.state.strategic.detonations[i];
        d.reached = front;
    }
}
