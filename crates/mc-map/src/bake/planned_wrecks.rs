//! Wreckage laid with a map's plan, for the layouts that have no symmetry for
//! `wreckage::stamp` to copy by (the survival maps): fields along the roads
//! the old assaults took, each filled with wrecks up to its salvage weight.

use super::machine::dist;
use super::Terrain;
use crate::format::MapWreck;
use crate::noise::{hash2, unit};
use std::f64::consts::TAU;

/// A wreck kind: key, how far it reaches (m) and its salvage weight (T1 1,
/// T2 4, T3 16, as `wreckage.rs` counts it).
type Kind = (&'static str, f64, u32);

pub(super) const KINDS: [Kind; 12] = [
    ("aster_t1_tank", 5.0, 1),
    ("aster_t1_bot", 3.0, 1),
    ("aster_t1_artillery", 5.0, 1),
    ("aster_t2_tank", 7.0, 4),
    ("aster_t2_hover", 6.0, 4),
    ("aster_t2_missile", 6.0, 4),
    ("regency_t1_tank", 5.0, 1),
    ("regency_t1_raider", 4.0, 1),
    ("regency_t2_tank", 7.0, 4),
    ("regency_t2_bombard", 7.0, 4),
    ("aster_t3_assault_bot", 14.0, 16),
    ("aster_t3_artillery", 11.0, 16),
];

/// A field of wreckage: its centre and radius (map metres) and its salvage
/// weight.
pub(super) type Field = ((f64, f64), f64, u32);

impl Terrain {
    /// Wrecks scattered over `fields`, on open, gentle, dry ground clear of the
    /// starts and the ore; `salt` keeps one map's scatter its own.
    pub(super) fn planned_wrecks(&self, fields: &[Field], salt: u64) -> Vec<MapWreck> {
        let fx = |v: f64| mc_core::Fx((v * 65536.0).round() as i64);
        let mut out = Vec::new();
        let mut laid: Vec<((f64, f64), f64)> = Vec::new();
        for (i, &(centre, radius, budget)) in fields.iter().enumerate() {
            let mut weight = 0;
            let mut tries = 0_i64;
            while weight < budget && tries < 4_000 {
                tries += 1;
                let h = hash2(self.seed ^ salt, i as i64, tries);
                let (a, r) = (unit(h, 8) * TAU, radius * unit(h, 32).sqrt());
                let p = (centre.0 + r * a.cos(), centre.1 + r * a.sin());
                let (key, reach, w) = KINDS[(unit(h, 20) * KINDS.len() as f64) as usize];
                // A heavy wreck only in a field heavy enough, once it has light ones.
                if w + weight > budget || (w >= 16 && (budget < 60 || weight < budget / 3)) {
                    continue;
                }
                let clear = self.height(p.0, p.1) > 2.0
                    && self.slope(p.0, p.1) < 0.3
                    && self
                        .starts
                        .iter()
                        .all(|&s| dist(p, s) > self.start_outer * 0.6)
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
                    materials_milli: 550 + (unit(h, 52) * 400.0) as u16,
                });
            }
        }
        out
    }
}
