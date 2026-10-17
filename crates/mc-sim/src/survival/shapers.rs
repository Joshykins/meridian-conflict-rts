//! The Shapers (replication nodes): the facility's ray raises one on each of
//! the map's Shaper sites, each of one domain (land, air or naval). Online, a
//! Shaper prints formations of its products and sends each down its site's
//! path at the defenders. It grows the longer it stands, with no ceiling:
//! every level its formations are bigger, every other level it takes on
//! another kind of unit (of the tier the engine prints at by then), from level
//! 8 heavies join its formations, and it builds up taller. Killing one early
//! is the answer: its site stands empty until the ray reseeds it, though a
//! reseeded Shaper quickly evolves to a level that fits the round.

use super::print::Route;
use super::rules::{
    aux_first_round, fleet_gap_ticks, launch_gap_ticks, reseed_round, shaper_batch,
    shaper_gains_heavy, shaper_gains_product, shaper_height, volley_ticks, CATCH_UP_TICKS,
    CLIMB_PER_TICK,
};
use super::{Hold, Node, NodeSite, Spot, Survival, HZ, RAISE_TICKS};
use crate::mirror::SimEvent;
use crate::SimError;
use mc_core::{Angle, Fx};
use mc_data::survival::{Domain, SiteKind};
use mc_data::BlueprintId;

impl crate::World {
    /// Picks a free site and starts the ray on it: one Shaper to a site, and a
    /// site whose Shaper was destroyed stands empty until `reseed_from`.
    pub(super) fn survival_start_node(&mut self, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let rules = s.config.rules;
        if s.nodes.len() >= rules.node_limit() {
            return Ok(());
        }
        let tier = rules.tier_at(s.round);
        let free: Vec<(usize, SiteKind)> = s
            .config
            .node_sites
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                !s.nodes.iter().any(|n| n.site as usize == *i)
                    && s.reseed_from.get(*i).is_none_or(|r| *r <= s.round)
            })
            .filter_map(|(i, site)| Some((i, self.raise_kind(s, site, tier)?)))
            .collect();
        if free.is_empty() {
            return Ok(());
        }
        let mut rng = s.rng;
        let (site_i, kind) = free[rng.below(free.len() as u32) as usize];
        let site = s.config.node_sites[site_i].clone();
        // A land site raises land Shapers, a sea site naval ones; either may raise air.
        // A Seedcaster's squads are land units; an Unmaker prints nothing, and a
        // Gatewright's fleets are its own.
        let mut kinds = Vec::new();
        match kind {
            SiteKind::Printer => {
                if rules.has(site.domain) && site.domain != Domain::Air {
                    kinds.push(site.domain);
                }
                if rules.has(Domain::Air) {
                    kinds.push(Domain::Air);
                }
            }
            SiteKind::Fleet => kinds.push(Domain::Air),
            SiteKind::Artillery | SiteKind::Nuke | SiteKind::Auxiliary => kinds.push(Domain::Land),
        }
        if kinds.is_empty() {
            return Ok(());
        }
        let side = s.config.engine_player;
        let domain = kinds[rng.below(kinds.len() as u32) as usize];
        let products = match kind {
            SiteKind::Nuke | SiteKind::Auxiliary => Vec::new(),
            SiteKind::Fleet => self.fleet_classes_of(1, tier),
            SiteKind::Printer | SiteKind::Artillery => {
                let Some(p) = newest_new(&self.roster_ids(domain, tier), &[], &mut rng) else {
                    return Ok(());
                };
                vec![p]
            }
        };
        let node_bp = s.kit.shaper(kind);
        self.state.survival.as_mut().unwrap().rng = rng;

        let nbp = self.blueprints.unit(node_bp);
        let heading = self.site_heading(&site);
        let pos = crate::world::snap_to_build_grid(nbp, site.at);
        // Something of the defenders' stands on the site: the ray waits for it to clear.
        if !self.can_place(nbp, pos) {
            return Ok(());
        }
        let row = self.spawn_unit(node_bp, side, pos, heading, false)?;
        let unit = self.state.units.id(row);
        let s = self.state.survival.as_mut().unwrap();
        s.nodes.push(Node {
            unit,
            site: site_i as u8,
            kind,
            pos,
            domain,
            products: products.clone(),
            level: 1,
            born: s.round,
            started: now,
            raised: false,
            next_print: 0,
            printed: 0,
            height: shaper_height(1),
            next_level_at: 0,
            pieces: Vec::new(),
            next_piece_at: 0,
            next_fire: 0,
            hold: Hold::None,
            foot: Fx::ZERO,
        });
        self.events.push(SimEvent::NodeRaising {
            site: site_i as u8,
            pos,
            product: products.first().copied(),
            kind,
            shaper: node_bp,
        });
        Ok(())
    }

    /// The first round a Shaper of auxiliary `kind` may rise in this match, and the
    /// tech it waits for (a Gatewright's cheapest warship's, else 0); None when the
    /// match never reaches it (no warship at its tier cap).
    pub(super) fn wake_round(&self, s: &Survival, kind: SiteKind) -> Option<(u16, u8)> {
        let first = aux_first_round(kind);
        if kind != SiteKind::Fleet {
            return Some((first, 0));
        }
        let bps = self.blueprints.as_ref();
        let tech = self
            .fleet_roster(mc_data::MAX_TECH)
            .iter()
            .map(|b| bps.unit(*b).tech)
            .min()?;
        let round = s.config.rules.first_round_of(tech)?;
        Some((round.max(first), tech))
    }

    /// The kind of Shaper the ray would raise on `site` now (the round's tier `tier`),
    /// if any. A printing site raises a printer while it has a front to print for; a
    /// site naming an auxiliary kind raises that once its first round has come; an
    /// `Auxiliary` site raises the auxiliary kind fewest of the Shapers standing (or
    /// rising) are, of those whose first round has come and that have something to
    /// do: a tie goes to the first in `SiteKind::AUXILIARY`.
    fn raise_kind(&self, s: &Survival, site: &NodeSite, tier: u8) -> Option<SiteKind> {
        let rules = s.config.rules;
        let can = |kind: SiteKind| {
            s.round >= aux_first_round(kind)
                && match kind {
                    SiteKind::Printer if site.domain == Domain::Naval => {
                        rules.has(Domain::Naval) || rules.has(Domain::Air)
                    }
                    SiteKind::Printer => rules.has(Domain::Land) || rules.has(Domain::Air),
                    // A Seedcaster's squads are land units; an Unmaker needs no front.
                    SiteKind::Artillery => rules.has(Domain::Land),
                    SiteKind::Nuke => true,
                    // A Gatewright needs warships at the round's tier.
                    SiteKind::Fleet => !self.fleet_roster(tier).is_empty(),
                    SiteKind::Auxiliary => false,
                }
        };
        if site.kind != SiteKind::Auxiliary {
            return can(site.kind).then_some(site.kind);
        }
        SiteKind::AUXILIARY
            .iter()
            .enumerate()
            .filter(|(_, k)| can(**k))
            .min_by_key(|(order, k)| (s.nodes.iter().filter(|n| n.kind == **k).count(), *order))
            .map(|(_, k)| *k)
    }

    /// The way site `site`'s base faces: its own facing, else toward the first
    /// point of its path, else toward the defenders.
    pub(super) fn site_heading(&self, site: &NodeSite) -> Angle {
        site.facing.unwrap_or_else(|| {
            let toward = site
                .path
                .first()
                .copied()
                .unwrap_or_else(|| self.survival_target());
            (toward - site.at).angle()
        })
    }

    /// The rays build up the nodes they are on.
    pub(super) fn survival_raise(&mut self, now: u32) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_ref() else {
            return Ok(());
        };
        let rising: Vec<usize> = (0..s.nodes.len()).filter(|i| !s.nodes[*i].raised).collect();
        for i in rising {
            let s = self.state.survival.as_ref().unwrap();
            let (unit, started, site, product, kind) = {
                let n = &s.nodes[i];
                (
                    n.unit,
                    n.started,
                    n.site,
                    n.products.first().copied(),
                    n.kind,
                )
            };
            let Some(row) = self.state.units.row(unit) else {
                continue;
            };
            let t = Fx::ratio((now - started) as i64, RAISE_TICKS as i64).min(Fx::ONE);
            let bp = self.blueprints.unit(self.state.units.blueprint[row]);
            self.state.units.build_progress[row] = bp.build_time * t;
            let full = self.unit_max_health(row);
            let floor = full / 10;
            let want = floor + (full - floor) * t;
            // Damage taken while it goes up stays taken.
            if self.state.units.health[row] < want && now.is_multiple_of(HZ) {
                self.state.units.health[row] = (self.state.units.health[row]
                    + (full - floor) * HZ as i32 / RAISE_TICKS as i32)
                    .min(want);
            }
            if t >= Fx::ONE {
                self.complete_unit(row)?;
                let pos = self.state.units.pos[row];
                let s = self.state.survival.as_mut().unwrap();
                s.nodes[i].raised = true;
                s.nodes[i].born = s.round;
                s.nodes[i].next_print = now + 3 * HZ;
                s.nodes[i].next_level_at = now + CATCH_UP_TICKS;
                // An auxiliary Shaper's first volley, launch or fleet comes after half a beat.
                s.nodes[i].next_fire = now
                    + match kind {
                        SiteKind::Artillery => volley_ticks(1) / 2,
                        SiteKind::Nuke => launch_gap_ticks(1),
                        SiteKind::Fleet => fleet_gap_ticks(1) / 2,
                        SiteKind::Printer | SiteKind::Auxiliary => 0,
                    };
                let shaper = self.state.units.blueprint[row];
                self.events.push(SimEvent::NodeOnline {
                    site,
                    pos,
                    product,
                    kind,
                    shaper,
                });
            }
        }
        Ok(())
    }

    /// As a round begins, the Shapers that have stood long enough gain a level,
    /// and with some levels another kind of unit.
    pub(super) fn survival_level_shapers(&mut self, round: u16) {
        let s = self.state.survival.as_ref().unwrap();
        let rules = s.config.rules;
        let due: Vec<usize> = (0..s.nodes.len())
            .filter(|i| s.nodes[*i].raised && rules.shaper_levels_up(round, s.nodes[*i].born))
            .collect();
        for i in due {
            self.survival_gain_level(i, rules.tier_at(round));
        }
    }

    /// Shapers grow: each climbs toward its level's height, and one raised
    /// late evolves a level every `CATCH_UP_TICKS` until it has caught up.
    pub(super) fn survival_grow(&mut self, now: u32) {
        let Some(s) = self.state.survival.as_mut() else {
            return;
        };
        for n in s.nodes.iter_mut().filter(|n| n.raised) {
            let top = shaper_height(n.level);
            n.height = (n.height + CLIMB_PER_TICK).min(top.max(n.height));
        }
        let tier = s.config.rules.tier_at(s.round.max(1));
        let catch_up = s.config.rules.catch_up_level(s.round.max(1));
        let due: Vec<usize> = (0..s.nodes.len())
            .filter(|i| {
                let n = &s.nodes[*i];
                n.raised && n.level < catch_up && now >= n.next_level_at
            })
            .collect();
        for i in due {
            self.state.survival.as_mut().unwrap().nodes[i].next_level_at = now + CATCH_UP_TICKS;
            self.survival_gain_level(i, tier);
        }
    }

    /// Shaper `i` gains a level: every other level from 3 another kind of its
    /// domain (or its oldest kind gives way to a newer), from 8 heavies.
    fn survival_gain_level(&mut self, i: usize, tier: u8) {
        let s = self.state.survival.as_ref().unwrap();
        let mut rng = s.rng;
        let mut n = s.nodes[i].clone();
        // An Unmaker prints nothing: its levels grow its defences and its silos. A
        // Gatewright's levels open bigger classes of warship to its fleets.
        let prints = matches!(n.kind, SiteKind::Printer | SiteKind::Artillery);
        n.level = n.level.saturating_add(1);
        let mut added = None;
        if n.kind == SiteKind::Fleet {
            let classes = self.fleet_classes_of(n.level, tier);
            added = classes.iter().copied().find(|c| !n.products.contains(c));
            n.products = classes;
        }
        if prints && shaper_gains_product(n.level) {
            let light: Vec<(BlueprintId, u8)> = self.roster_ids(n.domain, tier);
            added = newest_new(&light, &n.products, &mut rng);
            // Nothing new of its domain left: its oldest kind gives way to one of the newest tier.
            if added.is_none() {
                let top = light.iter().map(|u| u.1).max().unwrap_or(1);
                let oldest = n
                    .products
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        let t = self.blueprints.unit(**p).tech;
                        t < top && t < 4
                    })
                    .min_by_key(|(i, p)| (self.blueprints.unit(**p).tech, *i))
                    .map(|(i, _)| i);
                let newest: Vec<BlueprintId> =
                    light.iter().filter(|u| u.1 == top).map(|u| u.0).collect();
                if let (Some(i), false) = (oldest, newest.is_empty()) {
                    let p = newest[rng.below(newest.len() as u32) as usize];
                    n.products[i] = p;
                    added = Some(p);
                }
            }
            if let Some(p) = added.filter(|p| !n.products.contains(p)) {
                n.products.push(p);
            }
        }
        if prints && shaper_gains_heavy(n.level) {
            let heavy = self.heavy_ids(n.domain, tier);
            let fresh: Vec<BlueprintId> = heavy
                .iter()
                .copied()
                .filter(|h| !n.products.contains(h))
                .collect();
            if !fresh.is_empty() {
                let p = fresh[rng.below(fresh.len() as u32) as usize];
                n.products.push(p);
                added = Some(p);
            }
        }
        self.events.push(SimEvent::ShaperLevelled {
            site: n.site,
            pos: n.pos,
            level: n.level,
            added,
        });
        // What a level is worth in hit points, shield, Lance and interceptors is its
        // blueprint's growth block.
        if let Some(row) = self.state.units.row(n.unit) {
            self.set_unit_level(row, n.level);
        }
        // Its silos grow with it (pieces without a `growth` block are left alone).
        for p in n.pieces.iter().filter(|p| p.raised) {
            if let Some(row) = self.state.units.row(p.unit) {
                self.set_unit_level(row, n.level);
            }
        }
        let s = self.state.survival.as_mut().unwrap();
        s.nodes[i] = n;
        s.rng = rng;
    }

    /// Online Shapers print a formation of their products at a time on one of
    /// their free platforms (the smallest the biggest unit fits, else the
    /// biggest), on a beat set by their tech and level. Without a free platform
    /// it waits for one.
    pub(super) fn survival_node_prints(&mut self, now: u32) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_ref() else {
            return Ok(());
        };
        let ready: Vec<usize> = (0..s.nodes.len())
            .filter(|i| {
                let n = &s.nodes[*i];
                n.raised && now >= n.next_print && n.kind == SiteKind::Printer
            })
            .collect();
        for i in ready {
            let s = self.state.survival.as_ref().unwrap();
            let n = s.nodes[i].clone();
            if self.state.units.row(n.unit).is_none()
                || s.printing.iter().any(|p| p.source == n.unit && !p.round)
            {
                continue;
            }
            let bps = self.blueprints.as_ref();
            let (light, heavy): (Vec<BlueprintId>, Vec<BlueprintId>) =
                n.products.iter().partition(|p| bps.unit(**p).tech < 4);
            let light_tech = light.iter().map(|p| bps.unit(*p).tech).max().unwrap_or(1);
            let (nl, nh) = shaper_batch(n.level, light_tech, !heavy.is_empty());
            let mut batch: Vec<BlueprintId> = (0..nh).map(|k| heavy[k % heavy.len()]).collect();
            if !light.is_empty() {
                batch.extend((0..nl).map(|k| light[k % light.len()]));
            }
            if batch.is_empty() {
                continue;
            }
            let radius = batch
                .iter()
                .map(|b| bps.unit(*b).radius)
                .max()
                .unwrap_or(Fx::ZERO);
            let free: Vec<(crate::UnitId, Fx)> = self
                .shaper_pads(&n)
                .into_iter()
                .filter(|(pad, _)| !self.survival_spot_taken(Spot::Pad(*pad)))
                .collect();
            let fit = free
                .iter()
                .filter(|(_, room)| radius <= *room)
                .min_by_key(|(pad, room)| (*room, *pad))
                .or_else(|| {
                    free.iter()
                        .max_by_key(|(pad, room)| (*room, std::cmp::Reverse(*pad)))
                });
            let Some(&(pad, _)) = fit else {
                self.state.survival.as_mut().unwrap().nodes[i].next_print = now + HZ;
                continue;
            };
            let tech = batch.iter().map(|p| bps.unit(*p).tech).max().unwrap_or(1);
            let route = Route::Site(n.site as usize);
            self.survival_print_formation(&batch, Spot::Pad(pad), n.unit, route, false, now)?;
            let s = self.state.survival.as_mut().unwrap();
            s.nodes[i].printed += batch.len() as u32;
            // Slower for bigger units; faster as the rounds climb and as it grows.
            // The more Shapers stand, the slower each prints: the facility grows
            // busier, not runaway.
            let online = s.nodes.iter().filter(|n| n.raised).count() as u32;
            let beat = (14 + 10 * tech as u32) * HZ * 10 / (10 + s.round.min(20) as u32 / 2);
            let beat = beat * (6 + online) / 7;
            let beat = beat * 20 / (20 + n.level.min(200) as u32);
            s.nodes[i].next_print = now + beat;
        }
        Ok(())
    }

    /// Nodes that died since last tick: their bases unravel, and the defenders
    /// are told what their wreck holds.
    pub(super) fn survival_nodes_lost(&mut self) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_mut() else {
            return Ok(());
        };
        let units = &self.state.units;
        let mut lost = Vec::new();
        s.nodes.retain(|n| {
            if units.row(n.unit).is_some() {
                true
            } else {
                lost.push(n.clone());
                false
            }
        });
        for n in lost {
            // Its base unravels with it, and its foot opens.
            self.survival_unravel(&n)?;
            self.survival_release_foot(n.pos, n.foot);
            let s = self.state.survival.as_ref().unwrap();
            let kind = n.kind;
            let bp = self.blueprints.unit(s.kit.shaper(kind));
            let wreck = if n.raised {
                (bp.cost_materials * bp.wreck_fraction).round_int().max(0) as u32
            } else {
                0
            };
            let s = self.state.survival.as_mut().unwrap();
            s.nodes_destroyed += 1;
            // Its site stands empty until the ray reseeds it.
            let again = reseed_round(s.round);
            if let Some(r) = s.reseed_from.get_mut(n.site as usize) {
                *r = again;
            }
            self.events.push(SimEvent::NodeDestroyed {
                site: n.site,
                pos: n.pos,
                product: n.products.first().copied(),
                kind,
                wreck,
                raised: n.raised,
            });
        }
        Ok(())
    }
}

/// A unit of the newest tier in `roster` that is not in `have`, if there is one;
/// else of any tier.
fn newest_new(
    roster: &[(BlueprintId, u8)],
    have: &[BlueprintId],
    rng: &mut mc_core::Rng,
) -> Option<BlueprintId> {
    let fresh: Vec<&(BlueprintId, u8)> = roster.iter().filter(|u| !have.contains(&u.0)).collect();
    let top = fresh.iter().map(|u| u.1).max()?;
    let pick: Vec<BlueprintId> = fresh.iter().filter(|u| u.1 == top).map(|u| u.0).collect();
    Some(pick[rng.below(pick.len() as u32) as usize])
}
