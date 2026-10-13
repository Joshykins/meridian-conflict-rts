//! Construction as the presentation sees it: the beams from builders and factories to
//! the work they raise, the nanite streams and swarms of a faction that builds with
//! nanites, and the great beam a yard casts up to a hull it raises aloft
//! (`sky_yard.rs`).

use super::*;
use crate::reclaim::{BeamInstance, BEAM_SKY_LIFT};

/// How wide the great beam a yard casts up to its hull is drawn, metres round its axis.
const SKY_LIFT_RADIUS: f32 = 22.0;
/// The size a helper's stream into a yard's intake is drawn for.
const INTAKE_SIZE: f32 = 6.0;

impl World {
    /// This tick's construction beams and build sounds into `frame`, for `viewer`.
    pub(super) fn write_build_beams(&self, viewer: Option<u8>, frame: &mut RenderFrame) {
        let s = &self.state;
        // Construction beams: from each mobile builder at work to the weld it prints from,
        // including engineers helping an upgrade, or a factory printing in its bay.
        use crate::tables::flag;
        frame.build_sources.clear();
        let mut nanite_beams: Vec<(u32, BeamInstance)> = Vec::new();
        // The Regency sites those streams feed, by unit id.
        let mut nanite_sites: Vec<u32> = Vec::new();
        // The hulls yards are raising aloft, and their yards: work on them pours into the
        // yard's intakes, not up under the hull.
        let aloft: Vec<(UnitId, usize)> = s
            .units
            .slots
            .iter()
            .filter(|&r| self.builds_aloft(r))
            .map(|r| (s.units.build_target[r], r))
            .filter(|&(t, _)| s.units.row(t).is_some())
            .collect();
        for row in s.units.slots.iter() {
            let flags = s.units.flags[row];
            if flags & flag::BUILDING == 0
                || flags & flag::REPAIRING != 0
                || !self.beams_to_work(row)
            {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            // A refit of its own is assembled on it, not beamed onto it by its own arm
            // (it is still heard, below).
            let refitting = s.orders.front(&s.units, row).is_some_and(|o| {
                o.kind == crate::tables::OrderKind::Upgrade && self.upgrades_in_place(row)
            });
            if refitting {
                continue;
            }
            let Some(t) = self.print_target(row) else {
                continue;
            };
            let from = self.builder_emitter(row);
            // A hull going up aloft is fed through one of its yard's intakes.
            let intake = aloft
                .iter()
                .find(|&&(hull, _)| hull == s.units.id(t))
                .map(|&(_, yard)| self.yard_intake(yard, s.units.id(row).0));
            let to = intake.unwrap_or_else(|| self.weld_on(t, from.to_f32()).0);
            let size = intake.map_or(self.bp(t).radius.to_f32(), |_| INTAKE_SIZE);
            frame.build_sources.push(self.build_source(row, to));
            if self.uses_nanites(row) {
                // A Regency site is fed by one nanite stream from the builder's emitter.
                if intake.is_none() {
                    nanite_sites.push(s.units.id(t).0);
                }
                nanite_beams.push((
                    s.units.id(row).0,
                    BeamInstance {
                        from: from.to_f32(),
                        kind: BEAM_NANITE,
                        to_prev: to,
                        radius: size,
                        to,
                        height: 0.0,
                    },
                ));
                continue;
            }
            for from in std::iter::once(from).chain(self.builder_extra_emitters(row)) {
                frame.projectiles.push(ProjectileInstance {
                    prev_pos: from.to_f32(),
                    color: COLOR_BUILD | PROJECTILE_BEAM,
                    pos: intake.unwrap_or_else(|| self.weld_on(t, from.to_f32()).0),
                    size,
                    wake: 0.0,
                    plasma: 0.0,
                    _pad: [0.0; 2],
                    aim: [0.0; 4],
                    prev_aim: [0.0; 4],
                });
            }
        }
        // Factory print guns: one amber thread per nozzle, never during a refit.
        for row in s.units.slots.iter() {
            if s.units.flags[row] & flag::BUILDING == 0 || !self.bp(row).has(mc_data::cat::FACTORY)
            {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            let Some(t) = s.units.row(s.units.build_target[row]) else {
                continue;
            };
            if !s.units.has_flag(t, flag::UNDER_CONSTRUCTION) || s.units.has_flag(t, flag::UPGRADE)
            {
                continue;
            }
            if self.builds_aloft(row) {
                // A yard casts one great beam up from its emitter to the hull it raises.
                let (from, to) = self.sky_lift(row, t);
                frame.build_sources.push(self.build_source(row, from));
                nanite_beams.push((
                    s.units.id(row).0,
                    BeamInstance {
                        from,
                        kind: BEAM_SKY_LIFT,
                        to_prev: to,
                        radius: SKY_LIFT_RADIUS,
                        to,
                        height: 0.0,
                    },
                ));
                continue;
            }
            let heads = self.factory_print_heads(row);
            if let Some(first) = heads.first() {
                let to = self.weld_on(t, first.to_f32()).0;
                frame.build_sources.push(self.build_source(row, to));
            } else if self.uses_nanites(row) {
                // A Regency factory without fabricator heads is heard from the hull it builds.
                let at = s.units.pos[t].extend(s.units.z[t]).to_f32();
                frame.build_sources.push(self.build_source(row, at));
            }
            if self.uses_nanites(row) {
                // A Regency factory pours a nanite stream from each fabricator head. Each needs
                // a source of its own (the renderer times a beam by its source).
                if !heads.is_empty() {
                    nanite_sites.push(s.units.id(t).0);
                }
                for (k, head) in heads.iter().enumerate() {
                    let to = self.weld_on(t, head.to_f32()).0;
                    nanite_beams.push((
                        s.units.id(row).0 ^ ((k as u32) << 27),
                        BeamInstance {
                            from: head.to_f32(),
                            kind: BEAM_NANITE,
                            to_prev: to,
                            radius: self.bp(t).radius.to_f32(),
                            to,
                            height: 0.0,
                        },
                    ));
                }
                continue;
            }
            for head in heads {
                let to = self.weld_on(t, head.to_f32()).0;
                frame.projectiles.push(ProjectileInstance {
                    prev_pos: head.to_f32(),
                    color: COLOR_BUILD | PROJECTILE_BEAM,
                    pos: to,
                    size: self.bp(t).radius.to_f32(),
                    wake: 0.0,
                    plasma: 0.0,
                    _pad: [0.0; 2],
                    aim: [0.0; 4],
                    prev_aim: [0.0; 4],
                });
            }
        }

        // Upgrades a unit or structure works on itself: no beam, but the same
        // construction hum, heard from the hull being refitted.
        for row in s.units.slots.iter() {
            let flags = s.units.flags[row];
            if flags & flag::BUILDING == 0 || flags & flag::REPAIRING != 0 {
                continue;
            }
            if s.orders.front(&s.units, row).map(|o| o.kind)
                != Some(crate::tables::OrderKind::Upgrade)
            {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            let at = s.units.pos[row].extend(s.units.z[row]).to_f32();
            let h = self.bp(row).height.to_f32();
            frame
                .build_sources
                .push(self.build_source(row, [at[0], at[1], at[2] + h * 0.5]));
        }

        self.write_reclaim_beams(viewer, &mut frame.beams, &mut frame.beam_sources);
        self.write_repair_beams(viewer, &mut frame.beams, &mut frame.beam_sources);
        self.write_survival_beams(viewer, &mut frame.beams, &mut frame.beam_sources);
        for (source, beam) in nanite_beams {
            frame.beam_sources.push(source);
            frame.beams.push(beam);
        }
        // Sites of a faction that builds with nanites condense out of a swarm, and its
        // refits go up the same way.
        for u in frame.units.iter_mut() {
            let working = u.owner_flags & ((crate::tables::flag::UNDER_CONSTRUCTION as u32) << 8)
                != 0
                || (u.upgrade > 0.0 && u.upgrade < 1.0);
            if u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
                && working
                && self
                    .blueprints
                    .units
                    .get(u.blueprint as usize)
                    .is_some_and(|b| self.faction_uses_nanites(b.faction))
            {
                u.status[1] |= UNIT_NANITE;
            }
        }
        // Round each Regency site at work, its swarm and rings: one record a site,
        // timed by the site's id with bits 27..31 set (no stream's source has them all). A
        // Regency refit gets the same, fed or not: it goes up the way a new hull does.
        nanite_sites.sort_unstable();
        nanite_sites.dedup();
        for u in frame.units.iter() {
            let printing = (crate::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
            let fed =
                u.owner_flags & printing != 0 && nanite_sites.binary_search(&u.unit_id).is_ok();
            let refit = u.upgrade > 0.0 && u.upgrade < 1.0 && u.status[1] & UNIT_NANITE != 0;
            if !fed && !refit {
                continue;
            }
            let Some(bp) = self.blueprints.units.get(u.blueprint as usize) else {
                continue;
            };
            let height = bp.height.to_f32();
            // Where the hull has condensed to (`nanite_grow` in entity.wgsl); a refit's
            // work climbs it as the refit goes.
            let done = if fed { u.build / 0.8 } else { u.upgrade };
            let front = u.pos[2] + height * done.clamp(0.0, 1.0);
            let to = [u.pos[0], u.pos[1], front];
            frame.beam_sources.push(u.unit_id | 0xF800_0000);
            frame.beams.push(BeamInstance {
                from: u.pos,
                kind: BEAM_NANITE_SITE,
                to_prev: to,
                radius: bp.radius.to_f32(),
                to,
                height,
            });
        }
    }

    /// Where a yard's great beam leaves (its first emitter) and where it meets the hull
    /// going up over it: the hull's belly over its middle.
    fn sky_lift(&self, yard: usize, hull: usize) -> ([f32; 3], [f32; 3]) {
        let s = &self.state;
        let from = self.yard_emitter(yard, 0);
        let h = self.bp(hull).height.to_f32();
        let p = s.units.pos[hull].to_f32();
        (from, [p[0], p[1], s.units.z[hull].to_f32() + h * 0.06])
    }

    /// The intake of a yard a helper's stream pours into: one of its emitters after the
    /// first, by the helper's id, so a crowd of helpers spreads over them.
    fn yard_intake(&self, yard: usize, helper: u32) -> [f32; 3] {
        let n = self
            .bp(yard)
            .builder
            .as_ref()
            .map_or(1, |b| b.emitters.len().max(2));
        self.yard_emitter(yard, 1 + helper as usize % (n - 1))
    }

    /// A yard's `k`-th emitter in the world (`Builder::emitters`, turned with the yard).
    fn yard_emitter(&self, yard: usize, k: usize) -> [f32; 3] {
        let s = &self.state;
        let e = self
            .bp(yard)
            .builder
            .as_ref()
            .and_then(|b| b.emitters.get(k).copied())
            .unwrap_or(FxVec3::new(Fx::ZERO, Fx::ZERO, self.bp(yard).height));
        let at = s.units.pos[yard] + mc_core::FxVec2::new(e.x, e.y).rotate(s.units.heading[yard]);
        at.extend(s.units.z[yard] + e.z).to_f32()
    }
}
