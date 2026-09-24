//! Re-forming orders already given: when the player changes the formation
//! settings, the selection's moves, patrols and orbits are laid out again,
//! together or free and at the new spacing, with the same destinations,
//! posts and circles.
use crate::formations::{slots, Group};
use crate::tables::{Order, OrderKind};
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, MoveLayer};
use std::collections::BTreeMap;

const QUARTER: Angle = Angle::from_degrees(90);

/// How far apart a group stands at each spacing setting, in hull widths plus a margin.
pub(crate) fn spacing_scale(level: u8) -> Fx {
    match level.min(2) {
        0 => Fx::ONE,
        1 => Fx::ratio(5, 4),
        _ => Fx::ratio(7, 4),
    }
}

impl World {
    pub(crate) fn reform(
        &mut self,
        player: u8,
        ids: &[crate::tables::UnitId],
        together: bool,
        spacing_level: u8,
    ) -> Result<(), SimError> {
        let mut queues: BTreeMap<usize, Vec<Order>> = self
            .owned(player, ids, cat::MOBILE)
            .into_iter()
            .map(|row| {
                let q = self
                    .state
                    .orders
                    .iter(&self.state.units, row)
                    .copied()
                    .collect();
                (row, q)
            })
            .collect();
        // Orders given as one: a shared command group, or, for units moving free,
        // the same kind of order to the same place. Air and ground never share one.
        let mut groups = BTreeMap::<_, BTreeMap<usize, Vec<usize>>>::new();
        for (&row, queue) in &queues {
            let m = self.bp(row).motion.expect("mobile");
            let layer = match m.layer {
                MoveLayer::Air => (2, m.altitude.0),
                MoveLayer::Naval => (1, 0),
                _ => (0, 0),
            };
            for (i, o) in queue.iter().enumerate() {
                if !matches!(
                    o.kind,
                    OrderKind::Move | OrderKind::AttackMove | OrderKind::Patrol | OrderKind::Orbit
                ) {
                    continue;
                }
                let key = if o.formation != 0 {
                    (o.formation, 0, 0, 0, crate::Handle::NONE, layer)
                } else {
                    (0, o.kind as u8, o.pos.x.0, o.pos.y.0, o.target, layer)
                };
                groups
                    .entry(key)
                    .or_default()
                    .entry(row)
                    .or_default()
                    .push(i);
            }
        }

        // A patrol is one group per leg, all of the same units: each keeps one
        // slot round the whole loop.
        let mut ranks = BTreeMap::<Vec<usize>, Vec<usize>>::new();
        for members in groups.into_values() {
            let rows: Vec<usize> = members.keys().copied().collect();
            let n = rows.len();
            let (&row0, idx0) = members.iter().next().expect("a member");
            let first = queues[&row0][idx0[0]];
            let orbit = first.kind == OrderKind::Orbit;
            let air = self
                .bp(row0)
                .motion
                .is_some_and(|m| m.layer == MoveLayer::Air);
            let mut spacing = Fx::ZERO;
            let mut mean = FxVec2::ZERO;
            for &row in &rows {
                spacing = spacing.max(self.bp(row).radius * 2 + Fx::from_int(6));
                mean += self.state.units.pos[row];
            }
            let spacing = spacing * spacing_scale(spacing_level);
            let mean = FxVec2::new(mean.x / n as i32, mean.y / n as i32);
            // The way the group faces: along its order, or, circling, along the circle.
            let centre = self
                .state
                .units
                .row(first.target)
                .map_or(first.pos, |t| self.state.units.pos[t]);
            let facing = if !orbit {
                first.heading
            } else if mean.distance(centre) < Fx::ONE {
                self.state.units.heading[row0]
            } else {
                (mean - centre).angle() + QUARTER
            };
            // Where each stood in the old layout, local X forward and Y left.
            let local = |o: &Order| {
                if orbit {
                    o.offset
                } else {
                    o.offset.rotate(-o.heading)
                }
            };
            let laid_out = rows
                .iter()
                .any(|r| local(&queues[r][members[r][0]]) != FxVec2::ZERO);

            // A single unit stands on the point; a group keeps its ranks, front
            // to back and left to right, or takes them from where it flies now.
            let mut offsets = vec![FxVec2::ZERO; n];
            let mut ranked = rows.clone();
            if n > 1 && (together || !orbit) {
                let slots_at = slots(n, spacing, air);
                // Rounded, so a slot turned to a leg's heading and back still ranks the same.
                let grain = (spacing.0 / 4).max(1);
                let round = |v: Fx| (v.0 + grain / 2).div_euclid(grain);
                ranked = ranks
                    .entry(rows.clone())
                    .or_insert_with(|| {
                        let mut ranked = rows.clone();
                        ranked.sort_by_key(|&row| {
                            let p = if laid_out {
                                local(&queues[&row][members[&row][0]])
                            } else {
                                (self.state.units.pos[row] - mean).rotate(-facing)
                            };
                            (-round(p.x), -round(p.y), row)
                        });
                        ranked
                    })
                    .clone();
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by_key(|&i| (-slots_at[i].x.0, -slots_at[i].y.0, i));
                offsets = order.into_iter().map(|i| slots_at[i]).collect();
            }
            let extent = offsets.iter().map(|p| p.length()).fold(Fx::ZERO, Fx::max);
            let formation = if together && n > 1 {
                self.state.formation_serial += 1;
                let id = self.state.formation_serial;
                let anchor = if orbit { centre } else { mean };
                self.state.formations.insert(
                    id,
                    Group {
                        anchor,
                        heading: facing,
                        phase: 0,
                        speed: Fx::ZERO,
                    },
                );
                id
            } else {
                0
            };
            for (row, slot) in ranked.into_iter().zip(offsets) {
                for &i in &members[&row] {
                    let o = &mut queues.get_mut(&row).expect("queued")[i];
                    o.formation = formation;
                    if orbit {
                        o.offset = slot;
                        o.heading = Angle::ZERO;
                        // Never so tight the V cannot turn round it.
                        o.radius = o.radius.max(extent * Fx::ratio(3, 2));
                    } else {
                        o.offset = slot.rotate(o.heading);
                    }
                }
            }
        }

        // The front order stays in front: whatever it is doing carries on.
        for (row, queue) in queues {
            self.state.orders.clear(&mut self.state.units, row);
            for o in queue {
                self.state.orders.push_back(&mut self.state.units, row, o)?;
            }
        }
        Ok(())
    }
}
