//! The war before the match: wrecks where the assaults went in. Fields of
//! them lie before every gate and breach on the glacis, and behind each in
//! the streets the defenders held, the same weight of salvage on each side of
//! the wall (`tests/halcyon.rs`). Laid with the plan, since a siege has no
//! symmetry for `wreckage::stamp` to copy by.

use super::*;
use crate::format::MapWreck;
use crate::noise::hash2;

/// A wreck kind: key, how far it reaches (m) and its salvage weight (T1 1,
/// T2 4, T3 16, as `wreckage.rs` counts it).
type Kind = (&'static str, f64, u32);

const ASSAULT: [Kind; 8] = [
    ("aster_t1_tank", 5.0, 1),
    ("aster_t1_bot", 3.0, 1),
    ("aster_t1_artillery", 5.0, 1),
    ("aster_t2_tank", 7.0, 4),
    ("aster_t2_hover", 6.0, 4),
    ("regency_t1_tank", 5.0, 1),
    ("regency_t1_raider", 4.0, 1),
    ("aster_t3_assault_bot", 14.0, 16),
];

/// Salvage weight laid on each side of the wall.
const WEIGHT_PER_SIDE: u32 = 210;

impl Terrain {
    pub(in crate::bake) fn siege_wrecks(&self) -> Vec<MapWreck> {
        let fx = |v: f64| mc_core::Fx((v * 65536.0).round() as i64);
        // Before and behind every gate and breach, the outside's on the glacis,
        // the inside's in the streets behind the ring road.
        let mut fields: Vec<(P, f64, bool)> = Vec::new();
        for &x in GATES_X.iter().chain(&BREACHES_X) {
            let wall = y_at(WALL, x);
            fields.push(((x, wall - 230.0), 170.0, false));
            fields.push(((x + 40.0, wall + 300.0), 150.0, true));
        }
        let mut out = Vec::new();
        for city in [true, false] {
            let mut weight = 0;
            let mut laid: Vec<(P, f64)> = Vec::new();
            let mut tries = 0_i64;
            while weight < WEIGHT_PER_SIDE && tries < 40_000 {
                tries += 1;
                let h = hash2(self.seed ^ 0x7772_6563 ^ city as u64, tries, 0);
                let side: Vec<&(P, f64, bool)> = fields.iter().filter(|f| f.2 == city).collect();
                let &(centre, radius, _) = side[(h % side.len() as u64) as usize];
                let (a, r) = (
                    unit(h, 8) * std::f64::consts::TAU,
                    radius * unit(h, 32).sqrt(),
                );
                let p = (centre.0 + r * a.cos(), centre.1 + r * a.sin());
                let (key, reach, w) = ASSAULT[(unit(h, 20) * ASSAULT.len() as f64) as usize];
                // A heavy wreck only once the field has some light ones.
                if w + weight > WEIGHT_PER_SIDE || (w >= 16 && weight < 60) {
                    continue;
                }
                let clear = Self::in_city(p) == city
                    && !self.siege_lot_near(p, reach + 3.0)
                    && self.slope(p.0, p.1) < 0.3
                    && !self.ore.iter().any(|f| f.covers(p.0, p.1, reach + 6.0))
                    && laid.iter().all(|&(q, s)| dist(p, q) > reach + s + 2.0);
                if !clear {
                    continue;
                }
                laid.push((p, reach));
                weight += w;
                out.push(MapWreck {
                    blueprint: key.into(),
                    pos: mc_core::FxVec2::new(fx(p.0), fx(p.1)),
                    heading: mc_core::Angle((h >> 40) as u16),
                    bank: ((unit(h, 44) - 0.5) * 1_200.0) as i16,
                    mass_milli: 550 + (unit(h, 52) * 400.0) as u16,
                });
            }
        }
        out
    }
}
