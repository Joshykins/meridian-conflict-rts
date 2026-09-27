//! The works round the dam: below each powerhouse a switchyard, its power
//! line striding off down the dry valley to the map's edge, and an
//! operations town on the valley floor (the models are mc-render's
//! `dam_works.rs`, built to `landmark::GORGE_YARD`, `GORGE_LINE` and
//! `GORGE_TOWN`). Each side's works are the other's mirror image, so they
//! take the same ground from the valley both sides.
//!
//! Every piece stands on a level lot the bake cuts and fills for it, each at
//! the ground's own height there; the yard at its first tower's, since the
//! yard's model strings that span level. Each tower's span is a prop of its
//! own (`PropKind::DamSpan`), which the renderer pitches to meet the next.

use super::*;
use crate::landmark::{GORGE_LINE, GORGE_TOWN, GORGE_YARD};

/// The west switchyard's middle (u, v), below the west powerhouse.
const YARD: (f64, f64) = (424.0, 1990.0);
/// The west line's way (u, v), a unit vector: out across the valley and
/// down it, to the map's south edge past the valley's west fork.
const LINE: (f64, f64) = (0.654, -0.757);
/// The west town's middle (u, v), on the valley floor between its line and
/// the middle of the valley, clear of the valley's ore.
const TOWN: (f64, f64) = (544.0, 1450.0);

/// Metres of level ground round a lot's edge before it eases into the ground.
const LOT_MARGIN: f64 = 12.0;
/// A tower's level footing, radius.
const FOOTING: f64 = 14.0;

/// A level lot: a rectangle `half` either side of `at` along `heading` and
/// across it, at `height`, easing out over `blend`.
pub(super) struct Lot {
    at: (f64, f64),
    heading: f64,
    half: (f64, f64),
    height: f64,
    blend: f64,
}

impl Lot {
    /// Metres outside the lot (0 inside).
    fn outside(&self, x: f64, y: f64) -> f64 {
        let (s, c) = self.heading.sin_cos();
        let (dx, dy) = (x - self.at.0, y - self.at.1);
        let (a, b) = (dx * c + dy * s, -dx * s + dy * c);
        let ox = (a.abs() - self.half.0).max(0.0);
        let oy = (b.abs() - self.half.1).max(0.0);
        ox.hypot(oy)
    }
}

impl Terrain {
    /// Lays the works out, both sides: their lots and their props.
    pub(super) fn lay_works(&mut self) {
        let [yard_w, yard_e] = self.mirror_sides(YARD);
        let [town_w, town_e] = self.mirror_sides(TOWN);
        let ways = [(-LINE.0, LINE.1), (LINE.0, LINE.1)];
        for (side, (yard, town)) in [(yard_w, town_w), (yard_e, town_e)].into_iter().enumerate() {
            let way = ways[side];
            let heading = way.1.atan2(way.0);
            let first = GORGE_YARD.first;
            let height = self.natural_canyon(yard.0 + way.0 * first, yard.1 + way.1 * first);
            self.canyon.works.push(Lot {
                at: yard,
                heading,
                half: (
                    GORGE_YARD.half.0 + LOT_MARGIN,
                    GORGE_YARD.half.1 + LOT_MARGIN,
                ),
                height,
                blend: 70.0,
            });
            self.precursor.push(PrecursorSite {
                kind: PropKind::DamSwitchyard,
                x: yard.0,
                y: yard.1,
                heading,
                scale: 1.0,
            });
            // Towers every span from the first to the map's edge; the last
            // one's span runs on off it.
            let mut along = first;
            loop {
                let p = (yard.0 + way.0 * along, yard.1 + way.1 * along);
                if p.0 < 0.0 || p.1 < 0.0 || p.0 > self.size_x || p.1 > self.size_y {
                    break;
                }
                let height = self.natural_canyon(p.0, p.1);
                self.canyon.works.push(Lot {
                    at: p,
                    heading,
                    half: (FOOTING, FOOTING),
                    height,
                    blend: 40.0,
                });
                for kind in [PropKind::DamPylon, PropKind::DamSpan] {
                    self.precursor.push(PrecursorSite {
                        kind,
                        x: p.0,
                        y: p.1,
                        heading,
                        scale: 1.0,
                    });
                }
                along += GORGE_LINE.span;
            }
            // The town faces the dam: its office north over its square.
            let height = self.natural_canyon(town.0, town.1);
            self.canyon.works.push(Lot {
                at: town,
                heading: 0.0,
                half: (
                    GORGE_TOWN.half.0 + LOT_MARGIN,
                    GORGE_TOWN.half.1 + LOT_MARGIN,
                ),
                height,
                blend: 90.0,
            });
            self.precursor.push(PrecursorSite {
                kind: PropKind::DamTown,
                x: town.0,
                y: town.1,
                heading: 0.0,
                scale: 1.0,
            });
        }
    }

    /// The ground as the works' lots cut and fill it.
    pub(super) fn works_ground(&self, x: f64, y: f64, h: f64) -> f64 {
        let mut h = h;
        for lot in &self.canyon.works {
            let reach = lot.half.0.max(lot.half.1) * 1.5 + lot.blend;
            if (x - lot.at.0).abs() > reach || (y - lot.at.1).abs() > reach {
                continue;
            }
            let w = 1.0 - smoothstep(0.0, lot.blend, lot.outside(x, y));
            h += (lot.height - h) * w;
        }
        h
    }

    /// Whether a lot of the works covers this point: nothing grows or lies
    /// there.
    pub(super) fn on_works(&self, x: f64, y: f64) -> bool {
        self.canyon
            .works
            .iter()
            .any(|lot| lot.outside(x, y) < 0.5 * lot.blend)
    }
}
