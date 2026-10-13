//! "Vela": the campaign's first mission, The Contract, on the ocean colony.
//!
//! [`Layout::Vela`], 8 km. The sea lies along the north and the west. The
//! Company lands on a beach in the north-west and drives east along the coast
//! road, past the junction where the planetary defence force meets it, over a
//! ridge and down to the bridge across the river, with guns dug into the rise
//! beyond; past the miners' camp, a fenced yard off the road; into Port Calder,
//! a prefab mining town on the coast round its square and its harbour, the
//! warehouses on the quay. The harbour road climbs out of the town to the
//! spaceport on its plateau behind, the rebels' main base round the apron where a
//! Bastion stands landed; beside it the quarry dug down to the Precursor
//! installation, the dig's hall on its floor.
//!
//! The rest is the colony's country: farms and their fields along the south
//! road and the lanes off it, woods on the hills, fishing hamlets on the
//! islands. Nothing is mirrored: a mission's map, its route the script's
//! (josh-os "Mission 1: The Contract"). Its two starts are the beach and the
//! spaceport, the Company's first.

use super::siege::grid::{dist, Buckets, Rect, Seg, P};
use super::{Pad, Terrain};
use crate::format::{Ground, PropKind, StreetSample};
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;

mod plan;
mod shape;
#[cfg(test)]
mod tests;

use shape::{PLATEAU, QUARRY};

/// The map's side, metres.
pub(super) const SIZE: f64 = 8_192.0;

/// Where the coast road crosses the river, on the bridge.
pub(super) const ROAD_Y: f64 = 5_600.0;
pub(super) const BRIDGE_X: f64 = 3_380.0;
pub(super) const BRIDGE_HALF_SPAN: f64 = crate::city::COLONY_BRIDGE_M as f64 / 2.0;
/// The bridge's road over the river's zero.
pub(super) const DECK_TOP: f64 = 9.0;

/// The starts: the beach landing zone, then the spaceport's apron.
pub(super) const BEACH_START: P = (1_320.0, 5_940.0);
pub(super) const PORT_START: P = (6_930.0, 4_560.0);
const START_CORE: f64 = 60.0;

/// What the Vela layout works out at set-up: its plan, as Halcyon's.
#[derive(Default)]
pub(super) struct Vela {
    pub(super) roads: Vec<Seg>,
    pub(super) road_index: Option<Buckets>,
    pub(super) lots: Vec<Lot>,
    pub(super) lot_index: Option<Buckets>,
    pub(super) areas: Vec<Area>,
    pub(super) area_index: Option<Buckets>,
    /// Props that neither block the plan nor are levelled for: street lights,
    /// barricades, turbines, masts, the bridge.
    pub(super) dressing: Vec<Dressing>,
}

/// A structure laid on the plan, and the level its ground is cut to.
#[derive(Clone, Copy, Debug)]
pub(super) struct Lot {
    pub(super) kind: PropKind,
    pub(super) at: P,
    pub(super) heading: f64,
    pub(super) rect: Rect,
    pub(super) level: f64,
}

/// Made ground: an apron, a yard, a field, a square.
#[derive(Clone, Copy, Debug)]
pub(super) struct Area {
    pub(super) rect: Rect,
    pub(super) ground: Ground,
}

/// A prop the plan lays as it is.
#[derive(Clone, Copy, Debug)]
pub(super) struct Dressing {
    pub(super) kind: PropKind,
    pub(super) at: P,
    pub(super) heading: f64,
    pub(super) scale: f64,
}

/// A lot is level this far out past its footprint, and eases into the land by
/// this far.
const LOT_FLAT: f64 = 3.0;
const LOT_EASE: f64 = 18.0;

impl Terrain {
    pub(super) fn setup_vela(&mut self) {
        self.start_outer = 2.0 * START_CORE;
        let g = BUILD_CELL_M as f64;
        let snap = |p: P| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        let starts = [snap(BEACH_START), snap(PORT_START)];
        self.pads = starts
            .iter()
            .map(|&(x, y)| Pad {
                x,
                y,
                core: START_CORE,
                outer: 2.0 * START_CORE,
                height: self.vela_land(x, y),
            })
            .collect();
        self.starts = starts.to_vec();
        self.lay_vela_plan();
        self.forest_edge = 0.0;
        self.fit_forests();
    }

    /// The ground with the plan cut in: each lot's ground levelled to it.
    pub(super) fn natural_vela(&self, x: f64, y: f64) -> f64 {
        let h = self.vela_land(x, y);
        let h = if shape::in_harbour((x, y)) {
            h.min(-6.0)
        } else {
            h
        };
        self.vela_cut((x, y), h)
    }

    fn vela_cut(&self, p: P, h: f64) -> f64 {
        let Some(index) = self.vela.lot_index.as_ref() else {
            return h;
        };
        let (mut pull, mut sum, mut weight) = (0.0_f64, 0.0, 0.0);
        let mut seen: Vec<u32> = Vec::new();
        for i in index.near(p, LOT_EASE + 8.0) {
            if seen.contains(&i) {
                continue;
            }
            seen.push(i);
            let lot = &self.vela.lots[i as usize];
            let outside = lot.rect.outside(p);
            let k = 1.0 - smoothstep(LOT_FLAT, LOT_EASE, outside);
            if k <= 0.0 {
                continue;
            }
            let w = k / (outside.max(0.0) + 0.5).powi(2);
            pull = pull.max(k);
            sum += w * lot.level;
            weight += w;
        }
        if weight > 0.0 {
            h + (sum / weight - h) * pull
        } else {
            h
        }
    }

    /// How thickly trees grow, and how much of it is palm: palms along the
    /// top of the beaches, jungle inland and on the hills, mangrove-thick
    /// along the creek; never on the plan's ground or in the town.
    pub(super) fn vela_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let p = (x, y);
        if !self.vela_clear(x, y) || self.vela_area(p).is_some() || self.vl_farmed(p) {
            return (0.0, 0.0);
        }
        // The town: palms here and there between the plots.
        let ((tx, ty), rx, ry) = shape::TOWN;
        if ((x - tx) / (rx + 120.0)).hypot((y - ty) / (ry + 120.0)) < 1.0 {
            return (0.12, 1.0);
        }
        if shape::beachness(p) > 0.2 && self.vl_inland(p) < 380.0 {
            return (0.0, 0.0);
        }
        let ((px, py), hx, hy) = PLATEAU;
        if (x - px).abs() < hx + 100.0 && (y - py).abs() < hy + 100.0 {
            return (0.0, 0.0);
        }
        if dist(p, QUARRY.0) < QUARRY.1 + 60.0 {
            return (0.0, 0.0);
        }
        // Cleared ground round the miners' camp, and the guns' field of fire
        // down the rise to the bridge.
        let ((cx, cy), chx, chy) = plan::CAMP;
        let camp = ((x - cx).abs() - chx - 90.0).max((y - cy).abs() - chy - 90.0);
        let rise = dist(p, shape::RISE) - 260.0;
        let open = smoothstep(0.0, 60.0, camp.min(rise));
        let wander = 4.0
            * self
                .forest_kind
                .fbm(x / 260.0 + 7.3, y / 260.0 - 3.1, 3, 0.5);
        let h = height + wander;
        let strand = smoothstep(1.6, 2.6, height) * (1.0 - smoothstep(8.0, 14.0, h));
        let inland = smoothstep(5.0, 13.0, h);
        let jungle = smoothstep(-0.15, 0.25, self.forest.fbm(x / 900.0, y / 900.0, 3, 0.5));
        let clearing = smoothstep(
            0.35,
            0.6,
            self.forest.fbm(x / 90.0 - 33.7, y / 90.0 + 57.2, 2, 0.5),
        );
        let creek = 1.0 - smoothstep(40.0, 160.0, shape::river_distance(p));
        let density = (strand * 0.45 + inland * (0.4 + 0.6 * jungle) * (1.0 - 0.7 * clearing))
            .max(0.8 * creek)
            * (1.0 - smoothstep(0.35, 0.6, slope));
        let palms = (1.0 - inland) + inland * 0.2;
        (density * open, palms.clamp(0.0, 1.0))
    }

    /// Whether the plan keeps trees and rocks off this point.
    pub(super) fn vela_clear(&self, x: f64, y: f64) -> bool {
        let p = (x, y);
        if let Some(index) = self.vela.road_index.as_ref() {
            if index.near(p, 30.0).any(|i| {
                self.vela.roads[i as usize].distance(p) < self.vela.roads[i as usize].reach() + 5.0
            }) {
                return false;
            }
        }
        if let Some(index) = self.vela.lot_index.as_ref() {
            if index
                .near(p, 12.0)
                .any(|i| self.vela.lots[i as usize].rect.outside(p) < 6.0)
            {
                return false;
            }
        }
        // The bridge's deck and the river under it.
        !((p.0 - BRIDGE_X).abs() < BRIDGE_HALF_SPAN + 20.0 && (p.1 - ROAD_Y).abs() < 30.0)
    }

    /// The made ground at `p`, the last laid winning.
    pub(super) fn vela_area(&self, p: P) -> Option<Ground> {
        let index = self.vela.area_index.as_ref()?;
        index
            .near(p, 0.0)
            .filter(|&i| self.vela.areas[i as usize].rect.outside(p) <= 0.0)
            .max()
            .map(|i| self.vela.areas[i as usize].ground)
    }

    /// The nearest road to `p` within 64 m.
    fn vela_road(&self, p: P) -> Option<(f64, Seg)> {
        let index = self.vela.road_index.as_ref()?;
        index
            .near(p, 64.0)
            .map(|i| self.vela.roads[i as usize])
            .map(|s| (s.distance(p), s))
            .filter(|(d, s)| *d < s.reach() + 40.0)
            .min_by(|a, b| (a.0 - a.1.half).total_cmp(&(b.0 - b.1.half)))
    }

    /// The streets layer over one tile.
    pub(super) fn vela_streets(&self, x0: f64, y0: f64) -> Vec<u8> {
        let n = crate::TILE_SAMPLES as usize;
        let cell = crate::CELL_SIZE_M as f64;
        let mut out = Vec::with_capacity(n * n * 4);
        for j in 0..n {
            for i in 0..n {
                let p = (x0 + i as f64 * cell, y0 + j as f64 * cell);
                out.extend(self.vela_street(p).bytes());
            }
        }
        out
    }

    fn vela_street(&self, p: P) -> StreetSample {
        let quarters = |m: f64| (m * 4.0).round().clamp(0.0, 255.0) as u8;
        let road = self.vela_road(p);
        let mut ground = self.vela_area(p).unwrap_or(Ground::Natural);
        if let Some((d, s)) = road {
            if s.walk > 0.0 && d < s.half + s.walk {
                ground = Ground::Paving;
            }
        }
        if ground == Ground::Natural && self.vl_farmed(p) {
            let (field, edge, _) = super::siege::fields::field_at(p);
            if edge > 2.5 && super::siege::fields::tilled(field) {
                ground = Ground::Field;
            }
        }
        match road {
            Some((d, s)) => StreetSample {
                offset: (128.0 + d.copysign(s.side(p)) * 4.0)
                    .round()
                    .clamp(0.0, 255.0) as u8,
                half: quarters(s.half),
                road: s.road,
                junction: self.vela_junction(p, &s),
                ground,
                battered: 0,
            },
            None => StreetSample {
                ground,
                ..StreetSample::NATURAL
            },
        }
    }

    /// Whether `p`, on road `s`, is where it meets another.
    fn vela_junction(&self, p: P, s: &Seg) -> bool {
        let Some(index) = self.vela.road_index.as_ref() else {
            return false;
        };
        let along = |r: &Seg| {
            let (dx, dy) = (r.b.0 - r.a.0, r.b.1 - r.a.1);
            let len = dx.hypot(dy).max(1e-6);
            (dx / len, dy / len)
        };
        let (ux, uy) = along(s);
        s.distance(p) < s.half + 5.0
            && index.near(p, 64.0).any(|i| {
                let r = &self.vela.roads[i as usize];
                let (vx, vy) = along(r);
                (ux * vy - uy * vx).abs() > 0.3 && r.distance(p) < r.half + 5.0
            })
    }

    /// The plan's structures and dressing, as props.
    pub(super) fn vela_props(&self, out: &mut Vec<crate::format::Prop>) {
        use mc_core::{Angle, Fx, FxVec2};
        let fx = |v: f64| Fx((v * 65536.0).round() as i64);
        let angle =
            |a: f64| Angle((a / std::f64::consts::TAU * 65536.0).rem_euclid(65536.0) as u16);
        let prop = |kind, at: P, heading, scale: f64| crate::format::Prop {
            kind,
            pos: FxVec2::new(fx(at.0), fx(at.1)),
            heading: angle(heading),
            scale_milli: (scale * 1000.0).round() as u16,
            wear_milli: 0,
        };
        for lot in &self.vela.lots {
            out.push(prop(lot.kind, lot.at, lot.heading, 1.0));
        }
        for d in &self.vela.dressing {
            out.push(prop(d.kind, d.at, d.heading, d.scale));
        }
    }

    /// The bridge's deck over the river.
    pub(in crate::bake) fn vela_decks(&self) -> Vec<crate::deck::Deck> {
        use mc_core::{Fx, FxVec2};
        let fx = |v: f64| Fx((v * 65_536.0).round() as i64);
        vec![crate::deck::Deck {
            a: FxVec2::new(fx(BRIDGE_X - BRIDGE_HALF_SPAN), fx(ROAD_Y)),
            b: FxVec2::new(fx(BRIDGE_X + BRIDGE_HALF_SPAN), fx(ROAD_Y)),
            half: fx(crate::city::COLONY_BRIDGE_HALF_M as f64),
            top: fx(DECK_TOP),
        }]
    }
}
