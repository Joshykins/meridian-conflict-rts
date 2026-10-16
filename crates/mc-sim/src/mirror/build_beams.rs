//! Construction as the presentation sees it: the beams from builders and factories to
//! the work they raise, the nanite streams and swarms of a faction that builds with
//! nanites.

use super::*;
use crate::reclaim::BeamInstance;

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
            let to = self.weld_on(t, from.to_f32()).0;
            let size = self.bp(t).radius.to_f32();
            frame.build_sources.push(self.build_source(row, to));
            if self.uses_nanites(row) {
                // A Regency site is fed by one nanite stream from the builder's emitter.
                nanite_sites.push(s.units.id(t).0);
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
                    pos: self.weld_on(t, from.to_f32()).0,
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
}
