//! A Shaper's base: the progenitor structures it raises round itself as it
//! grows. Print platforms stand in an arc in front of it (slips in the water at
//! a naval site), more of them and great ones as it levels so bigger units fit;
//! pylons stand at its sides and behind it and feed its shield. Each piece is
//! printed up by the Shaper's beam, one at a time; one destroyed is raised
//! again after a while; all of them unravel when their Shaper dies.
//!
//! How many of each a level raises and what a pylon feeds are `rules`; where
//! they stand is here (`slot_at`), inside the site's `room`.

use super::rules::{
    pylon_feed, shaper_platforms, shaper_pylons, shaper_silos, PIECE_GAP_TICKS, PIECE_LOST_TICKS,
    PIECE_RAISE_TICKS,
};
use super::{Node, Piece, PieceKind};
use crate::SimError;
use mc_core::{Angle, Fx, FxVec2};
use mc_data::survival::{Domain, SiteKind};
use mc_data::BlueprintId;

/// Most slots of one kind round a Shaper: past this the arc has no room. A bound on
/// the geometry, not on growth: the site's `room` runs out first.
const MOST_SLOTS: u8 = 12;
/// Metres between neighbouring pieces in an arc.
const GAP: i32 = 24;
/// Angle units per radian (65536 / 2 pi).
const PER_RADIAN: i64 = 10_430;

/// The blueprints the Shapers and their bases are made of (`Survival::kit`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kit {
    /// The Shaper of each kind, in `SiteKind`'s numbers' order: the printer
    /// (`replication_node`), the Seedcaster (warp-ins), the Unmaker (nukes) and the
    /// Gatewright (fleets).
    pub shapers: [BlueprintId; 4],
    /// Print platforms on land: ordinary, great.
    pub platform: [BlueprintId; 2],
    /// Print platforms in the water (slips): ordinary, great.
    pub slip: [BlueprintId; 2],
    pub pylon: BlueprintId,
    /// An Unmaker's silo.
    pub silo: BlueprintId,
}

impl Kit {
    /// Keys of the blueprints, in `Kit` order.
    const KEYS: [&'static str; 10] = [
        "replication_node",
        "progenitor_seedcaster",
        "progenitor_unmaker",
        "progenitor_gatewright",
        "progenitor_platform",
        "progenitor_platform_great",
        "progenitor_slip",
        "progenitor_slip_great",
        "progenitor_pylon",
        "progenitor_silo",
    ];

    pub(super) fn load(bps: &mc_data::Blueprints) -> Result<Kit, SimError> {
        let id = |k: &str| {
            bps.id_of(k)
                .ok_or_else(|| SimError::Setup(format!("survival needs blueprint {k}")))
        };
        let [a, b, c, j, d, e, f, g, h, i] = Kit::KEYS;
        Ok(Kit {
            shapers: [id(a)?, id(b)?, id(c)?, id(j)?],
            platform: [id(d)?, id(e)?],
            slip: [id(f)?, id(g)?],
            pylon: id(h)?,
            silo: id(i)?,
        })
    }

    /// The Shaper of `kind` (an `Auxiliary` site's kind is chosen before it is raised:
    /// never asked for, the Seedcaster).
    pub(super) fn shaper(&self, kind: SiteKind) -> BlueprintId {
        match kind {
            SiteKind::Printer => self.shapers[0],
            SiteKind::Artillery | SiteKind::Auxiliary => self.shapers[1],
            SiteKind::Nuke => self.shapers[2],
            SiteKind::Fleet => self.shapers[3],
        }
    }

    /// Every blueprint in it.
    pub(super) fn blueprints(&self) -> impl Iterator<Item = BlueprintId> {
        let mut all = self.shapers.to_vec();
        all.extend(self.platform);
        all.extend(self.slip);
        all.extend([self.pylon, self.silo]);
        all.into_iter()
    }

    /// The blueprint a piece of `kind` is at a site of `domain`.
    fn blueprint(&self, kind: PieceKind, domain: Domain) -> BlueprintId {
        let set = if domain == Domain::Naval {
            self.slip
        } else {
            self.platform
        };
        match kind {
            PieceKind::Platform => set[0],
            PieceKind::GreatPlatform => set[1],
            PieceKind::Pylon => self.pylon,
            PieceKind::Silo => self.silo,
        }
    }
}

/// The biggest unit (collision radius) a platform of `bp` prints: what fits on it
/// with a margin round it.
pub(super) fn platform_room(bp: &mc_data::UnitBlueprint) -> Fx {
    let side = bp.footprint.0.min(bp.footprint.1) as i32 * mc_map::BUILD_CELL_M;
    Fx::from_int((side / 2 - 8).max(8))
}

/// Where slot `k` of a piece `side` metres across stands round a Shaper at `at`
/// facing `heading` whose foot reaches `foot` (`rules::shaper_foot` at its level's
/// height), inside `room`: platforms on arcs in front (great ones on the outer arc),
/// pylons on a ring at the sides and behind, silos further out behind. The middle
/// slot first, then out to either side. None past the arc's ends, or when the room
/// has no ring clear of the foot left for it.
fn slot_at(
    kind: PieceKind,
    k: u8,
    at: FxVec2,
    heading: Angle,
    (foot, room): (Fx, Fx),
    side: i32,
) -> Option<FxVec2> {
    let half = side / 2;
    let (foot, room) = (foot.round_int(), room.round_int());
    // Each ring a way out past the foot, further as the room allows; never past the
    // room's edge, and never on the foot. The rooms (450 m, 350 m) hold every ring
    // round the widest feet (200 m, 180 m) but the great platforms', which the room
    // pulls in at the highest levels.
    let ring = |near: i32, far: i32| (room - half).min(far).max(near).min(room - half);
    let (ring, centre, spread) = match kind {
        PieceKind::Platform => (ring(foot + 50 + half / 2, foot + 90), 0u16, 0x3555u16),
        PieceKind::GreatPlatform => (ring(foot + 160, foot + 240), 0, 0x3555),
        PieceKind::Pylon => (ring(foot + 30 + half / 2, foot + 50), 0x8000, 0x4800),
        PieceKind::Silo => (ring(foot + 40 + half / 2, foot + 80), 0x8000, 0x3000),
    };
    if ring - half <= foot {
        return None;
    }
    let step = ((side + GAP) as i64 * PER_RADIAN / ring.max(1) as i64).min(0x4000) as i32;
    let out = (k as i32 + 1) / 2 * if k % 2 == 1 { 1 } else { -1 };
    let off = out * step;
    if off.unsigned_abs() > spread as u32 {
        return None;
    }
    let angle = Angle(heading.0.wrapping_add(centre).wrapping_add(off as u16));
    Some(at + FxVec2::from_angle(angle) * Fx::from_int(ring))
}

impl crate::World {
    /// Every Shaper's base a tick: pieces lost are noted, those being printed
    /// build up, the next is started when one is due, and pylons feed the dome.
    pub(super) fn survival_base(&mut self, now: u32) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_ref() else {
            return Ok(());
        };
        for i in 0..s.nodes.len() {
            let s = self.state.survival.as_ref().unwrap();
            let n = &s.nodes[i];
            if !n.raised || self.state.units.row(n.unit).is_none() {
                continue;
            }
            self.survival_pieces_lost(i, now);
            self.survival_pieces_rise(i, now)?;
            self.survival_next_piece(i, now)?;
            self.survival_pylons_feed(i);
        }
        Ok(())
    }

    fn survival_pieces_lost(&mut self, i: usize, now: u32) {
        let units = &self.state.units;
        let n = &mut self.state.survival.as_mut().unwrap().nodes[i];
        let before = n.pieces.len();
        n.pieces.retain(|p| units.row(p.unit).is_some());
        if n.pieces.len() < before {
            n.next_piece_at = n.next_piece_at.max(now + PIECE_LOST_TICKS);
        }
    }

    /// Pieces being printed build up under the Shaper's beam.
    fn survival_pieces_rise(&mut self, i: usize, now: u32) -> Result<(), SimError> {
        let pieces = self.state.survival.as_ref().unwrap().nodes[i]
            .pieces
            .clone();
        for (k, p) in pieces.iter().enumerate().filter(|(_, p)| !p.raised) {
            let Some(row) = self.state.units.row(p.unit) else {
                continue;
            };
            let t = Fx::ratio((now - p.started) as i64, PIECE_RAISE_TICKS as i64).min(Fx::ONE);
            let bp = self.bp(row);
            self.state.units.build_progress[row] = bp.build_time * t;
            let full = self.unit_max_health(row);
            let want = full / 10 + (full - full / 10) * t;
            if self.state.units.health[row] < want {
                self.state.units.health[row] = want.min(full);
            }
            if t >= Fx::ONE {
                self.complete_unit(row)?;
                let n = &mut self.state.survival.as_mut().unwrap().nodes[i];
                n.pieces[k].raised = true;
                // A silo grows with its Unmaker (its blueprint's `growth`).
                let level = n.level;
                self.set_unit_level(row, level);
            }
        }
        Ok(())
    }

    /// When one is due and none is being printed, the Shaper starts the next
    /// piece its level asks for: platforms first, then great ones, then pylons.
    fn survival_next_piece(&mut self, i: usize, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let n = &s.nodes[i];
        if now < n.next_piece_at || n.pieces.iter().any(|p| !p.raised) {
            return Ok(());
        }
        let Some(site) = s.config.node_sites.get(n.site as usize).cloned() else {
            return Ok(());
        };
        // A printer raises platforms; an auxiliary Shaper prints nothing on the ground, and
        // an Unmaker raises its silos instead.
        let (small, great) = match n.kind {
            SiteKind::Printer => shaper_platforms(n.level),
            _ => (0, 0),
        };
        let silos = match n.kind {
            SiteKind::Nuke => shaper_silos(n.level),
            _ => 0,
        };
        let want = [
            (PieceKind::Platform, small),
            (PieceKind::GreatPlatform, great),
            (PieceKind::Silo, silos),
            (PieceKind::Pylon, shaper_pylons(n.level)),
        ];
        let count = |kind| n.pieces.iter().filter(|p| p.kind == kind).count();
        let Some(kind) = want.iter().find(|(k, w)| count(*k) < *w).map(|(k, _)| *k) else {
            return Ok(());
        };
        let (side, unit_at, kit, shaper) = (s.config.engine_player, n.pos, s.kit, n.unit);
        let taken: Vec<u8> = n
            .pieces
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| p.slot)
            .collect();
        let Some(row) = self.state.units.row(shaper) else {
            return Ok(());
        };
        let heading = self.state.units.heading[row];
        // Clear of the foot the Shaper's level will spread it to.
        let foot = super::rules::shaper_foot(n.kind, super::rules::shaper_height(n.level));
        let bp_id = kit.blueprint(kind, site.domain);
        let bp = self.blueprints.unit(bp_id);
        let across = bp.footprint.0.max(bp.footprint.1) as i32 * mc_map::BUILD_CELL_M;
        // The first free slot it fits in; a slot the ground or something standing
        // there refuses is passed over.
        let spot = (0..MOST_SLOTS)
            .filter(|k| !taken.contains(k))
            .find_map(|k| {
                let at = slot_at(kind, k, unit_at, heading, (foot, site.room), across)?;
                let at = crate::world::snap_to_build_grid(bp, at);
                self.can_place(bp, at).then_some((k, at))
            });
        let n = &mut self.state.survival.as_mut().unwrap().nodes[i];
        n.next_piece_at = now + PIECE_GAP_TICKS;
        let Some((slot, at)) = spot else {
            return Ok(());
        };
        let row = self.spawn_unit(bp_id, side, at, heading, false)?;
        let unit = self.state.units.id(row);
        self.state.survival.as_mut().unwrap().nodes[i]
            .pieces
            .push(Piece {
                unit,
                kind,
                slot,
                started: now,
                raised: false,
            });
        Ok(())
    }

    /// Standing pylons feed their Shaper's dome while it is up.
    fn survival_pylons_feed(&mut self, i: usize) {
        let n = &self.state.survival.as_ref().unwrap().nodes[i];
        let pylons = n
            .pieces
            .iter()
            .filter(|p| p.kind == PieceKind::Pylon && p.raised)
            .count();
        let Some(row) = self.state.units.row(n.unit) else {
            return;
        };
        let Some(spec) = self.shield_spec(row) else {
            return;
        };
        if pylons == 0 || !self.shield_blocking(row) {
            return;
        }
        let hp = &mut self.state.units.shield_hp[row];
        *hp = (*hp + pylon_feed(spec.regen, pylons) / super::HZ as i32).min(spec.health);
    }

    /// A Shaper that died: its base unravels with it, leaving nothing.
    pub(super) fn survival_unravel(&mut self, node: &Node) -> Result<(), SimError> {
        for p in &node.pieces {
            if let Some(row) = self.state.units.row(p.unit) {
                self.despawn_unit(row, false)?;
            }
        }
        Ok(())
    }

    /// The raised platforms of Shaper `node`: (platform unit, the biggest unit it takes).
    pub(super) fn shaper_pads(&self, node: &Node) -> Vec<(crate::UnitId, Fx)> {
        node.pieces
            .iter()
            .filter(|p| {
                p.raised && matches!(p.kind, PieceKind::Platform | PieceKind::GreatPlatform)
            })
            .filter_map(|p| {
                let row = self.state.units.row(p.unit)?;
                Some((p.unit, platform_room(self.bp(row))))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_fan_out_in_front_and_pylons_stand_behind() {
        let at = FxVec2::from_ints(1000, 1000);
        let room = (Fx::from_int(30), Fx::from_int(450));
        let east = Angle::ZERO;
        let first = slot_at(PieceKind::Platform, 0, at, east, room, 96).unwrap();
        // Its near edge a way out past the foot (30 m).
        assert!(
            first.x - Fx::from_int(48) > at.x + Fx::from_int(60),
            "{first:?}"
        );
        let (a, b) = (
            slot_at(PieceKind::Platform, 1, at, east, room, 96).unwrap(),
            slot_at(PieceKind::Platform, 2, at, east, room, 96).unwrap(),
        );
        assert!(a.y > at.y && b.y < at.y, "either side: {a:?} {b:?}");
        assert!(a.distance(first) > Fx::from_int(100));
        let great = slot_at(PieceKind::GreatPlatform, 0, at, east, room, 192).unwrap();
        assert!(great.x > first.x + Fx::from_int(100));
        assert!(great.distance(at) + Fx::from_int(96) <= room.1 + Fx::ONE);
        let pylon = slot_at(PieceKind::Pylon, 0, at, east, room, 36).unwrap();
        assert!(pylon.x < at.x, "behind: {pylon:?}");
        // The arc ends: there is no tenth platform this close in.
        assert!(slot_at(PieceKind::Platform, 9, at, east, room, 96).is_none());
    }

    #[test]
    fn pieces_stand_past_the_widest_foot_inside_the_room() {
        let at = FxVec2::from_ints(1000, 1000);
        let east = Angle::ZERO;
        for (kind, room) in [(SiteKind::Printer, 450), (SiteKind::Nuke, 350)] {
            let foot = super::super::rules::shaper_foot(kind, Fx::from_int(1160));
            let room = Fx::from_int(room);
            for (piece, side) in [
                (PieceKind::Platform, 96),
                (PieceKind::GreatPlatform, 192),
                (PieceKind::Pylon, 36),
                (PieceKind::Silo, 32),
            ] {
                if kind == SiteKind::Nuke && piece != PieceKind::Pylon && piece != PieceKind::Silo {
                    continue;
                }
                let spot = slot_at(piece, 0, at, east, (foot, room), side)
                    .unwrap_or_else(|| panic!("{kind:?} {piece:?}: no room"));
                let d = spot.distance(at);
                let half = Fx::from_int(side / 2);
                assert!(
                    d - half > foot,
                    "{kind:?} {piece:?} at {d} on the foot {foot}"
                );
                assert!(
                    d + half <= room + Fx::ONE,
                    "{kind:?} {piece:?} at {d} past the room"
                );
            }
        }
    }
}
