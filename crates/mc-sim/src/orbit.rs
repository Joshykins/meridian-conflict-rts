//! Orbit orders given to a group: the aircraft fly a V that keeps its shape
//! round the circle, breaking off to fight and forming back up after.
use crate::formations::Group;
use crate::movement::FormationMotion;
use crate::orders::order;
use crate::tables::{Handle, Order, OrderKind};
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use std::collections::BTreeMap;

const DT: i32 = TICKS_PER_SECOND as i32;
/// `Angle` units in a radian.
const PER_RADIAN: i64 = 10430;
const QUARTER: Angle = Angle::from_degrees(90);

/// A point on a circle round `centre`, travelling counter-clockwise with
/// `heading` its direction of travel, and a formation slot offset from it.
fn slot_at(centre: FxVec2, radius: Fx, heading: Angle, o: &Order) -> FxVec2 {
    centre + FxVec2::from_angle(heading - QUARTER) * radius + o.offset.rotate(heading - o.heading)
}

/// How far out to put a point `lead` ahead on the circle so that an aircraft
/// chasing it cuts the chord and flies the circle itself, not one inside it.
pub(crate) fn chase_radius(radius: Fx, lead: Angle) -> Fx {
    radius / FxVec2::from_angle(Angle(lead.0 / 2)).x.max(Fx::HALF)
}

/// The angle a distance `arc` sweeps on a circle of `radius`.
fn sweep(arc: Fx, radius: Fx) -> i32 {
    (arc / radius.max(Fx::ONE))
        .mul_div(PER_RADIAN, 1)
        .floor_int()
}

impl World {
    /// One aircraft circles on its own; two or more fly it as a formation. `rows`
    /// are aircraft that can orbit; `radius` zero lets them pick their own.
    pub(crate) fn order_orbit(
        &mut self,
        mut rows: Vec<usize>,
        pos: FxVec2,
        anchor: Handle,
        radius: Fx,
        queue: bool,
    ) -> Result<(), SimError> {
        rows.sort_unstable();
        rows.dedup();
        if rows.len() < 2 {
            for row in rows {
                let mut o = order(OrderKind::Orbit, pos, anchor);
                o.radius = radius;
                self.give(row, o, queue)?;
            }
            return Ok(());
        }
        let n = rows.len();
        let mut spacing = Fx::ZERO;
        let mut own = Fx::ZERO;
        let mut mean = FxVec2::ZERO;
        for &row in &rows {
            spacing = spacing.max(self.bp(row).radius * 2 + Fx::from_int(6));
            own = own.max(self.bp(row).orbit_radius);
            mean += self.state.units.pos[row];
        }
        let mean = FxVec2::new(mean.x / n as i32, mean.y / n as i32);
        let offsets = crate::formations::slots(n, spacing * Fx::ratio(5, 4), true);
        let extent = offsets.iter().map(|p| p.length()).fold(Fx::ZERO, Fx::max);
        // The circle asked for, but never so tight the V cannot turn round it.
        let radius = if radius > Fx::ZERO { radius } else { own }.max(extent * Fx::ratio(3, 2));
        let centre = self
            .state
            .units
            .row(anchor)
            .map_or(pos, |t| self.state.units.pos[t]);
        let heading = if mean.distance(centre) < Fx::ONE {
            self.state.units.heading[rows[0]]
        } else {
            (mean - centre).angle() + QUARTER
        };
        // Whoever is furthest along the circle leads the V.
        let along = FxVec2::from_angle(heading);
        rows.sort_by_key(|&row| (-(self.state.units.pos[row] - mean).dot(along).0, row));
        let mut slots: Vec<usize> = (0..n).collect();
        slots.sort_by_key(|&i| (-offsets[i].x.0, -offsets[i].y.0, i));
        self.state.formation_serial += 1;
        let id = self.state.formation_serial;
        self.state.formations.insert(
            id,
            Group {
                anchor: centre,
                heading,
                phase: 0,
                speed: Fx::ZERO,
            },
        );
        for (row, slot) in rows.into_iter().zip(slots) {
            let mut o = order(OrderKind::Orbit, pos, anchor);
            o.formation = id;
            o.offset = offsets[slot];
            o.heading = Angle::ZERO;
            o.radius = radius;
            self.give(row, o, queue)?;
        }
        Ok(())
    }

    /// Where a member of a circling group should be now, once the group is flying.
    pub(crate) fn orbit_slot(&self, o: &Order) -> Option<FxVec2> {
        let g = self
            .state
            .formations
            .get(&o.formation)
            .filter(|g| g.phase != 0)?;
        Some(g.anchor + o.offset.rotate(g.heading - o.heading))
    }

    /// Carries each circling group's anchor round its circle at a pace its
    /// slowest member can hold, and steers every member along the circle onto its slot.
    pub(crate) fn orbit_formation_motion(
        &mut self,
        groups: BTreeMap<u64, Vec<usize>>,
        out: &mut [Option<FormationMotion>],
    ) {
        for (id, rows) in groups {
            let Some(mut group) = self.state.formations.get(&id).cloned() else {
                continue;
            };
            let units = &self.state.units;
            let first = *self.state.orders.front(units, rows[0]).unwrap();
            let (centre, radius) = (first.pos, first.radius);
            let mut pace = Fx::MAX;
            let mut accel = Fx::MAX;
            let mut size = Fx::ZERO;
            let mut mean = FxVec2::ZERO;
            for &row in &rows {
                let m = self.bp(row).motion.unwrap();
                pace = pace.min(m.speed);
                accel = accel.min(m.accel);
                size = size.max(self.bp(row).radius);
                mean += units.pos[row];
            }
            let mean = FxVec2::new(mean.x / rows.len() as i32, mean.y / rows.len() as i32);
            if group.phase == 0 {
                // Join the circle where the group already is.
                if mean.distance(centre) >= Fx::ONE {
                    group.heading = (mean - centre).angle() + QUARTER;
                }
                group.phase = 2;
                group.speed = Fx::ZERO;
            }
            let worst = rows
                .iter()
                .map(|&row| {
                    let o = self.state.orders.front(units, row).unwrap();
                    units.pos[row].distance(slot_at(centre, radius, group.heading, o))
                })
                .fold(Fx::ZERO, Fx::max);
            // Ease off while the V is strewn about so stragglers can catch it.
            let share = if worst < size * 2 + Fx::from_int(14) {
                Fx::ratio(7, 10)
            } else {
                Fx::ratio(2, 5)
            };
            group.speed = group.speed.approach(pace * share, accel / DT);
            group.heading = Angle(
                group
                    .heading
                    .0
                    .wrapping_add(sweep(group.speed / DT, radius) as u16),
            );
            group.anchor = centre + FxVec2::from_angle(group.heading - QUARTER) * radius;
            let route = FxVec2::from_angle(group.heading);
            for &row in &rows {
                let o = self.state.orders.front(units, row).unwrap();
                let slot = slot_at(centre, radius, group.heading, o);
                let lag = (slot - units.pos[row]).dot(route);
                let mo = self.bp(row).motion.unwrap();
                let look = (pace / 2).max(Fx::from_int(12));
                // As in a formation move, a wing chases a point two turn radii
                // out, or it weaves; here that point stays on the circle.
                let forward = if mo.hover {
                    (look + lag).clamp(look / 4, look * 2)
                } else {
                    let turn_radius = mo
                        .speed
                        .mul_div(10430, (mo.turn_rate as i64 * DT as i64).max(1));
                    (turn_radius * 2).max(look * 2)
                };
                let lead = Angle(sweep(forward, radius).clamp(0, 0x2000) as u16);
                let ahead = group.heading + lead;
                out[row] = Some(FormationMotion {
                    goal: slot_at(centre, chase_radius(radius, lead), ahead, o),
                    speed: (group.speed + lag * 2).clamp(pace / 5, mo.speed),
                    facing: group.heading,
                    cruise: false,
                    settling: false,
                });
            }
            self.state.formations.insert(id, group);
        }
    }
}
