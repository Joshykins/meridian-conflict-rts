//! Shots in flight: each tick's sweep for what a shot runs into, and what its
//! impact does (damage, splash, fires, scorch), with the blasts that work the same
//! way (`death_blast`, `blast_blocker`). Moved out of `combat.rs`, which keeps
//! targeting, the weapons and death.
//!
//! The sweeps are read-only and run in parallel over fixed-size chunks of the
//! projectile table; impacts are applied after, in hit order.

use crate::city::{Blow, KINETIC_SHARE};
use crate::combat::{CHUNK, GRAVITY};
use crate::mirror::SimEvent;
use crate::shields::{in_dome, ray_dome};
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2, FxVec3};
use mc_data::Trajectory;

struct Hit {
    projectile: usize,
    point: FxVec3,
    unit: Option<usize>,
    /// A dome that stopped the shot. Mutually exclusive with `unit`.
    shield: Option<usize>,
    /// The city structure (`city.rs` row) the shot ran into. Mutually exclusive
    /// with `unit` and `shield`.
    structure: Option<usize>,
    /// Share of this tick's step flown before the hit.
    after: Fx,
    /// Where the hit shows: `point` can lie deep inside a unit (the sweep finds
    /// the closest pass to its centre), so this is backed out to about its skin.
    /// Presentation only; damage and blasts use `point`.
    seen: FxVec3,
}

impl World {
    pub(crate) fn run_projectiles(&mut self) -> Result<(), SimError> {
        let span = mc_core::perf_span!("shots.guide");
        self.guide_and_intercept_missiles();
        self.steer_torpedoes();
        self.steer_interceptors();
        self.steer_curving_shots();
        self.split_cluster_shots()?;
        drop(span);
        let span = mc_core::perf_span!("shots.sweep");
        let count = self.state.projectiles.len();
        // The domes up this tick, found once: a battle's shells each look only at them.
        let domes: Vec<usize> = self
            .scratch
            .shielded
            .iter()
            .copied()
            .filter(|&row| {
                self.state.units.slots.is_alive(row)
                    && self.shield_blocking(row)
                    && self.bp(row).shield.is_some_and(|s| !s.is_hull())
            })
            .collect();
        let this = &*self;
        let hits: Vec<Vec<Hit>> = self.pool.parallel_map_chunks(count, CHUNK, |_, range| {
            let mut out = Vec::new();
            for i in range {
                if let Some(hit) = this.sweep_projectile(i, &domes) {
                    out.push(hit);
                }
            }
            out
        });
        let hits: Vec<Hit> = hits.into_iter().flatten().collect();
        drop(span);
        let _span = mc_core::perf_span!("shots.impacts");

        let p = &mut self.state.projectiles;
        for i in 0..count {
            if self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize].trajectory
                == Trajectory::Ballistic
            {
                p.vel[i].z -= GRAVITY;
            }
            p.age[i] = p.age[i].saturating_add(1);
            p.pos[i] += p.vel[i];
            p.ticks_left[i] = p.ticks_left[i].saturating_sub(1);
        }

        let mut remove: Vec<usize> = Vec::with_capacity(hits.len());
        for hit in &hits {
            let i = hit.projectile;
            let p = &self.state.projectiles;
            let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            #[expect(
                clippy::float_arithmetic,
                clippy::disallowed_types,
                clippy::disallowed_methods,
                reason = "presentation: spent shots and stream tails are drawn by the render mirror only"
            )]
            {
                let travel = self.unit_travel(p.source[i]);
                let back = travel.map(|t| -t);
                self.spent.push(crate::mirror::SpentShot {
                    cold: weapon.motor_out(p.age[i]),
                    sub: p.sub[i] > 0,
                    from: p.prev_pos[i],
                    to: hit.seen,
                    after: hit.after,
                    lead: crate::mirror::launch_shift(back, p.age[i] as f32 - 1.0),
                    blueprint: p.blueprint[i],
                    weapon: p.weapon[i],
                    owner: p.owner[i],
                });
                if weapon.rounds > 1 {
                    let lands = p.age[i] as f32 - 1.0 + hit.after.to_f32();
                    self.streams.push(crate::mirror::StreamTail::of(
                        p, i, weapon, lands, true, travel,
                    ));
                }
            }
            self.state.projectiles.pos[i] = hit.point;
            self.apply_impact(i, hit)?;
            remove.push(i);
        }
        let mut struck = vec![false; count];
        for &i in &remove {
            struck[i] = true;
        }
        // A bore's tracer that met nothing still carries the charge: it strikes where it ends.
        let spent: Vec<usize> = (0..count)
            .filter(|&i| {
                let p = &self.state.projectiles;
                p.ticks_left[i] == 0
                    && !struck[i]
                    && self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize]
                        .bore
                        .is_some()
            })
            .collect();
        for i in spent {
            let to = self.state.projectiles.pos[i];
            self.bore_discharge(i, to, None, Fx::ONE)?;
        }
        let p = &self.state.projectiles;
        for i in (0..count).filter(|&i| p.ticks_left[i] == 0 && !struck[i]) {
            let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            #[expect(
                clippy::disallowed_types,
                reason = "presentation: stream tails are drawn by the render mirror only"
            )]
            if weapon.rounds > 1 {
                let lands = p.age[i] as f32;
                let travel = self.unit_travel(p.source[i]);
                self.streams.push(crate::mirror::StreamTail::of(
                    p, i, weapon, lands, false, travel,
                ));
            }
        }
        remove.extend((0..count).filter(|&i| p.ticks_left[i] == 0));
        remove.sort_unstable();
        remove.dedup();
        for &i in remove.iter().rev() {
            self.state.projectiles.swap_remove(i);
        }
        Ok(())
    }

    /// Finds what projectile `i` runs into during this tick's step, if anything.
    fn sweep_projectile(&self, i: usize, domes: &[usize]) -> Option<Hit> {
        let p = &self.state.projectiles;
        let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
        let mut vel = p.vel[i];
        if weapon.trajectory == Trajectory::Ballistic {
            vel.z -= GRAVITY;
        }
        let from = p.pos[i];
        let to = from + vel;
        let mut best_t = Fx::MAX;
        let mut best = None;
        // A torpedo with nothing left to run at bursts where it is (`naval.rs`).
        if let Some(t) = self.torpedo_burst(i, weapon) {
            let point = from + vel * t;
            best_t = t;
            best = Some(Hit {
                projectile: i,
                point,
                unit: None,
                shield: None,
                structure: None,
                after: t,
                seen: point,
            });
        }
        // A flak shell's timed fuse: it bursts as it passes the point it was laid on.
        if let Some(t) = crate::flak::fuse(weapon, p.mark[i], from, vel).filter(|&t| t < best_t) {
            let point = from + vel * t;
            best_t = t;
            best = Some(Hit {
                projectile: i,
                point,
                unit: None,
                shield: None,
                structure: None,
                after: t,
                seen: point,
            });
        }
        // A fast slug can cover more than one raycast's reach in a tick.
        let ground = self.terrain.raycast_split(from, to).map(|point| {
            let t = (point.xy() - from.xy()).length() / vel.xy().length().max(Fx::EPSILON);
            (point, t)
        });
        if let Some((point, t)) = ground.filter(|g| g.1 < best_t) {
            best_t = t;
            best = Some(Hit {
                projectile: i,
                point,
                unit: None,
                shield: None,
                structure: None,
                after: best_t.clamp(Fx::ZERO, Fx::ONE),
                seen: point,
            });
        }
        // Shots stop on the water; nothing carries on down to the bed. An air-dropped
        // torpedo goes in and runs on (`naval.rs`).
        let water = self.terrain.water_level();
        if from.z > water && to.z <= water && !weapon.torpedo {
            let t = (from.z - water) / (from.z - to.z);
            if t < best_t {
                best_t = t;
                let point = from + vel * t;
                best = Some(Hit {
                    projectile: i,
                    point,
                    unit: None,
                    shield: None,
                    structure: None,
                    after: t.clamp(Fx::ZERO, Fx::ONE),
                    seen: point,
                });
            }
        }
        // Lobbed shells pass over things on the way up.
        if weapon.trajectory == Trajectory::Direct || vel.z < Fx::ZERO {
            let seg = vel.xy();
            let mid = from.xy() + seg * Fx::HALF;
            let reach = seg.length() / 2 + weapon.proximity;
            self.index
                .query_foes(mid, reach, kind::UNIT, self.team_mask(p.owner[i]), |e| {
                    let row = e.row as usize;
                    let units = &self.state.units;
                    // An interceptor runs at torpedoes, never at hulls.
                    if !self.unit_entry_is_current(e)
                        || weapon.intercepts
                        || !self.are_enemies(p.owner[i], units.owner[row])
                        || !self.weapon_reaches(row, weapon)
                        || crate::bore::passes_through(weapon, p.target[i], units.id(row))
                    {
                        return true;
                    }
                    let height = self.bp(row).height;
                    // A torpedo meets a hull below its waterline, down to the keel.
                    let keel = if weapon.torpedo {
                        height * crate::naval::KEEL
                    } else {
                        Fx::ZERO
                    };
                    let low = units.z[row] - keel - Fx::ONE - weapon.proximity;
                    let high = units.z[row] + height + Fx::ONE + weapon.proximity;
                    // Restrict the closest horizontal pass to the part of the
                    // segment inside the hull's vertical slab. A steep AA shot
                    // can cross the hull before its closest horizontal pass.
                    let (enter, leave) = if vel.z.abs() > Fx::EPSILON {
                        let a = (low - from.z) / vel.z;
                        let b = (high - from.z) / vel.z;
                        (a.min(b).max(Fx::ZERO), a.max(b).min(Fx::ONE))
                    } else {
                        if from.z < low || from.z > high {
                            return true;
                        }
                        (Fx::ZERO, Fx::ONE)
                    };
                    if enter > leave {
                        return true;
                    }
                    // A long hull is met along its spine, not as a disc (`body.rs`).
                    let (a, b, girth) = crate::body::spine(self.bp(row), e.pos, units.heading[row]);
                    let (t, off) = crate::body::closest_pass(from.xy(), seg, enter, leave, a, b);
                    let point = from + vel * t;
                    let inside = off <= (girth + weapon.proximity) * (girth + weapon.proximity);
                    if inside && t < best_t {
                        best_t = t;
                        let depth = (girth * girth - off).max(Fx::ZERO).sqrt() * Fx::ratio(8, 10);
                        let back = (depth / vel.length().max(Fx::EPSILON)).min(t);
                        // A hull field is the unit: shots that would land on the
                        // armour land on the wrap instead. A torpedo runs under it.
                        let on_hull = !weapon.torpedo
                            && self.shield_blocking(row)
                            && self.bp(row).shield.is_some_and(|s| s.is_hull());
                        best = Some(Hit {
                            projectile: i,
                            point,
                            unit: if on_hull { None } else { Some(row) },
                            shield: if on_hull { Some(row) } else { None },
                            structure: None,
                            after: t - back,
                            seen: from + vel * (t - back),
                        });
                    }
                    true
                });
        }
        // A city block in the way: a miss, a stray round or a shell over the
        // rooftops lands on it. A torpedo runs in the water, under the quays.
        if !weapon.torpedo {
            if let Some((t, row)) = self
                .city_shapes
                .first_hit(&self.state.city, from, to)
                .filter(|&(t, _)| t < best_t)
            {
                best_t = t;
                let point = from + vel * t;
                best = Some(Hit {
                    projectile: i,
                    point,
                    unit: None,
                    shield: None,
                    structure: Some(row),
                    after: t,
                    seen: point,
                });
            }
        }
        self.sweep_shields(i, domes, from, vel, &mut best_t, &mut best);
        best
    }

    /// Incoming fire hits the first enemy dome along the step. Shots that
    /// already started inside a bubble pass through it. `domes` are the rows
    /// whose dome (not hull field) is up.
    fn sweep_shields(
        &self,
        projectile: usize,
        domes: &[usize],
        from: FxVec3,
        vel: FxVec3,
        best_t: &mut Fx,
        best: &mut Option<Hit>,
    ) {
        let owner = self.state.projectiles.owner[projectile];
        // A torpedo in the water runs under the skirt of every dome. One still
        // falling from an aircraft breaks on it like anything else.
        let p = &self.state.projectiles;
        if self.blueprints.unit(p.blueprint[projectile]).weapons[p.weapon[projectile] as usize]
            .torpedo
            && from.z <= self.terrain.water_level()
        {
            return;
        }
        // Hull fields are the unit they wrap; the unit sweep already charges
        // them. A dome is the only thing that stops a shot early.
        for &row in domes {
            if !self.are_enemies(owner, self.state.units.owner[row]) {
                continue;
            }
            let radius = self.dome_radius(row);
            let center = self.state.units.pos[row].extend(self.state.units.z[row]);
            let floor = self.dome_floor();
            if in_dome(from, center, radius, floor) {
                continue;
            }
            if let Some(t) = ray_dome(from, vel, center, radius, floor) {
                if t < *best_t {
                    *best_t = t;
                    let point = from + vel * t;
                    *best = Some(Hit {
                        projectile,
                        point,
                        unit: None,
                        shield: Some(row),
                        structure: None,
                        after: t,
                        seen: point,
                    });
                }
            }
        }
    }

    fn apply_impact(&mut self, projectile: usize, hit: &Hit) -> Result<(), SimError> {
        let p = &self.state.projectiles;
        let (owner, source) = (p.owner[projectile], p.source[projectile]);
        let blueprints = self.blueprints.clone();
        let whole =
            &blueprints.unit(p.blueprint[projectile]).weapons[p.weapon[projectile] as usize];
        // A cluster's sub-shot lands with its share of the damage and its own splash.
        let sub_shot = (p.sub[projectile] > 0).then(|| whole.sub_shot()).flatten();
        let weapon = sub_shot.as_ref().unwrap_or(whole);
        let target_motion = hit.unit.map_or(FxVec3::ZERO, |row| {
            let u = &self.state.units;
            (u.pos[row] - u.prev_pos[row]).extend(u.z[row] - u.prev_z[row])
        });
        self.events.push(SimEvent::Impact {
            pos: hit.seen,
            target_motion,
            splash: weapon.splash,
            color: weapon.color,
            after: hit.after,
            on_unit: hit.unit.is_some(),
            on_shield: hit.shield.is_some(),
            on_structure: hit.structure.map(|row| self.state.city.prop[row]),
            blueprint: p.blueprint[projectile],
            weapon: p.weapon[projectile],
        });
        if let Some(bore) = weapon.bore {
            self.bore_discharge(projectile, hit.point, hit.unit.or(hit.shield), hit.after)?;
            if let Some(storm) = bore.storm {
                // What is left of the charge spreads out from the hit (`titan.rs`).
                self.state.storms.push(crate::titan::DischargeStorm::struck(
                    hit.point,
                    storm,
                    owner,
                    source,
                    weapon.target_mask,
                ));
            }
        }
        if weapon.discharge > 0.0 {
            // A charged shell: its charge strikes back up the last of its flight.
            #[expect(
                clippy::disallowed_methods,
                reason = "presentation: the discharge arc only goes into a SimEvent the game draws"
            )]
            let back =
                self.state.projectiles.vel[projectile].normalize() * Fx::from_f32(weapon.discharge);
            self.events.push(SimEvent::ShellDischarge {
                from: hit.seen - back,
                to: hit.seen,
                after: hit.after,
                blueprint: self.state.projectiles.blueprint[projectile],
                weapon: self.state.projectiles.weapon[projectile],
            });
        }
        if let Some(row) = hit.shield {
            self.damage_shield(row, weapon.damage);
            return Ok(());
        }
        // City blocks: a blast reaches every one round it (the one struck at no
        // distance), a slug only the one it struck (`city.rs` for the shares).
        // A torpedo bursts under the water, clear of them.
        let ignite = weapon.burn_ticks > 0;
        if weapon.splash > Fx::ZERO && !weapon.torpedo {
            let dealt = weapon.damage;
            let blow = Blow {
                ignite,
                ..Blow::BLAST
            };
            self.blast_structures(hit.point, weapon.splash, blow, |_| dealt);
        } else if let Some(row) = hit.structure {
            self.damage_structure(row, weapon.damage * KINETIC_SHARE, ignite);
        }
        if weapon.splash > Fx::ZERO {
            let victims: Vec<_> = self
                .state
                .units
                .slots
                .iter()
                .filter(|&r| {
                    // A half-built site stands in the open and takes the blast too;
                    // only what is still inside a factory is out of reach.
                    !self.state.units.has_flag(r, flag::IN_FACTORY)
                        && self.are_enemies(owner, self.state.units.owner[r])
                        && (self.bp(r).target_categories() & weapon.target_mask != 0
                            || (weapon.torpedo && self.on_seabed(r)))
                        && (hit.unit == Some(r) || self.blast_reaches(hit.point, r, weapon))
                })
                .collect();
            // Snapshot all protection before charging any field. A blast that
            // exhausts a shield is still absorbed for every victim of that blast.
            let victims: Vec<_> = victims
                .into_iter()
                .map(|r| {
                    let target = self.state.units.pos[r]
                        .extend(self.state.units.z[r] + self.bp(r).height / 2);
                    // A torpedo bursts under the skirt: no shield takes it.
                    let blocker = if weapon.torpedo {
                        None
                    } else {
                        self.blast_blocker(hit.point, target, Some(r))
                    };
                    (r, blocker)
                })
                .collect();
            let mut felled = Vec::new();
            // A torpedo bursts under water: it fells no trees and scorches nothing.
            let on_land = weapon.target_mask & mc_data::cat::AIR == 0 && !weapon.torpedo;
            if on_land {
                self.prop_index
                    .query(hit.point.xy(), weapon.splash, kind::PROP, |e| {
                        let prop = e.row as usize;
                        let xy = self.map.props[prop].pos;
                        if self
                            .blast_blocker(
                                hit.point,
                                xy.extend(self.terrain.height_at(xy) + Fx::ONE),
                                None,
                            )
                            .is_none()
                        {
                            felled.push(prop);
                        }
                        true
                    });
            }
            let mut charged = Vec::new();
            for (r, blocker) in victims {
                if let Some(shield) = blocker {
                    if !charged.contains(&shield) {
                        self.damage_shield(shield, weapon.damage);
                        charged.push(shield);
                    }
                    continue;
                }
                if !weapon.torpedo
                    && self.shield_blocking(r)
                    && self.bp(r).shield.is_some_and(|s| s.is_hull())
                {
                    self.damage_shield(r, weapon.damage);
                } else {
                    self.damage_unit(r, weapon.damage, owner, source);
                }
            }
            // The blast wears down the wrecks it reaches too (`wreck_damage.rs`).
            let dealt = weapon.damage;
            self.wear_wrecks(hit.point, weapon.splash, |_| dealt);
            if weapon.burn_ticks > 0 {
                // The patch stays where the bomb landed. Units burn by standing in it.
                let radius =
                    (Fx::from_int(4) + weapon.splash * Fx::ratio(11, 20)).min(Fx::from_int(32));
                let xy = hit.point.xy();
                // A stream gun lands ten rounds a second in one place: a round that falls
                // within half a patch of one the same gun lit rekindles it rather than
                // lighting another, so a strafe leaves a burning line, not a stack of
                // patches ten deep (and the fires table stays small).
                let fires = &mut self.state.fires;
                let rekindle = (weapon.rounds > 1)
                    .then(|| {
                        (0..fires.len()).find(|&i| {
                            fires.source[i] == source
                                && fires.ticks[i] > 0
                                && fires.pos[i].distance(xy) <= fires.radius[i] / 2
                        })
                    })
                    .flatten();
                if let Some(i) = rekindle {
                    fires.ticks[i] = weapon.burn_ticks;
                    fires.span[i] = weapon.burn_ticks;
                } else {
                    // A round that struck a hull still sets the ground under it alight.
                    let z = if weapon.rounds > 1 {
                        self.terrain.height_at(xy).max(self.terrain.water_level())
                    } else {
                        hit.point.z
                    };
                    fires.light(
                        xy,
                        z,
                        radius,
                        weapon.burn_ticks,
                        weapon.burn_dps,
                        owner,
                        source,
                        weapon.target_mask,
                    );
                }
            }
            if on_land {
                for prop in felled {
                    if self.map.props[prop].kind.is_tree() {
                        self.state.props_dead[prop / 64] |= 1 << (prop % 64);
                    }
                }
                self.add_stain(hit.point.xy(), weapon.splash, 96);
            }
        } else {
            if let Some(row) = hit.unit {
                self.damage_unit(row, weapon.damage, owner, source);
            } else if hit.structure.is_none() {
                self.add_stain(
                    hit.point.xy(),
                    Fx::from_int(2) + weapon.damage.sqrt() / 4,
                    40,
                );
            }
        }
        Ok(())
    }

    /// First live membrane crossed by blast propagation, regardless of team.
    /// The source and victim can share a dome: no membrane lies between them.
    pub(crate) fn blast_blocker(
        &self,
        from: FxVec3,
        to: FxVec3,
        victim: Option<usize>,
    ) -> Option<usize> {
        let mut best = None;
        let mut first = Fx::MAX;
        for &row in &self.scratch.shielded {
            if !self.state.units.slots.is_alive(row) || !self.shield_blocking(row) {
                continue;
            }
            let spec = self.bp(row).shield.unwrap();
            if spec.is_hull() {
                if victim == Some(row) && best.is_none() {
                    best = Some(row);
                }
                continue;
            }
            let center = self.state.units.pos[row].extend(self.state.units.z[row]);
            let a = from - center;
            let radius = self.dome_radius(row);
            let floor = self.dome_floor();
            if in_dome(from, center, radius, floor) && in_dome(to, center, radius, floor) {
                continue;
            }
            if let Some(t) = ray_dome(from, to - from, center, radius, floor) {
                // On-membrane impacts may throw sparks back away from the dome.
                if t <= Fx::EPSILON && (to - from).dot(a) >= Fx::ZERO {
                    continue;
                }
                if t < first {
                    first = t;
                    best = Some(row);
                }
            }
        }
        best
    }

    /// A volatile unit's blast: every unit in reach, its owner's included, takes
    /// the blast's damage, less toward the edge. Shields between take it instead.
    /// `afloat`: it went up on the water (a ship, or a structure built on the sea),
    /// so the blast starts at the surface, not on the seabed under it.
    pub(crate) fn death_blast(
        &mut self,
        center: FxVec2,
        db: mc_data::DeathBlast,
        owner: u8,
        afloat: bool,
    ) {
        let mut victims = Vec::new();
        self.index.query(center, db.radius, kind::UNIT, |e| {
            let row = e.row as usize;
            if self.unit_entry_is_current(e) && !self.state.units.has_flag(row, flag::IN_FACTORY) {
                victims.push(row);
            }
            true
        });
        let ground = self.terrain.height_at(center);
        let base = if afloat {
            ground.max(self.terrain.water_level())
        } else {
            ground
        };
        let origin = center.extend(base + Fx::ONE);
        let mut charged = Vec::new();
        for row in victims {
            let bp = self.bp(row);
            let (radius, height) = (bp.radius, bp.height);
            let reach = (self.state.units.pos[row].distance(center) - radius).max(Fx::ZERO);
            if reach > db.radius {
                continue;
            }
            let damage = db.damage * db.falloff(reach);
            let target = self.state.units.pos[row].extend(self.state.units.z[row] + height / 2);
            if let Some(shield) = self.blast_blocker(origin, target, Some(row)) {
                if !charged.contains(&shield) {
                    charged.push(shield);
                    self.damage_shield(shield, damage);
                }
                continue;
            }
            // Kills among the owner's own go uncredited.
            let by = if self.are_enemies(owner, self.state.units.owner[row]) {
                owner
            } else {
                u8::MAX
            };
            self.damage_unit(row, damage, by, Handle::NONE);
        }
        self.wear_wrecks(origin, db.radius, |reach| db.damage * db.falloff(reach));
        self.blast_structures(origin, db.radius, Blow::BLAST, |reach| {
            db.damage * db.falloff(reach)
        });
        let mut felled = Vec::new();
        self.prop_index
            .query(center, db.radius * Fx::ratio(3, 4), kind::PROP, |e| {
                felled.push(e.row as usize);
                true
            });
        for prop in felled {
            if self.map.props[prop].kind.is_tree() {
                self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            }
        }
        self.add_stain(center, db.radius * Fx::ratio(3, 4), 110)
    }
}
