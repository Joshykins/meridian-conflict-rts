//! The islands layout ([`super::Layout::Islands`], Twin Shoals): a designed
//! two-player sea map. One main island carries both starts, with a lake in its
//! middle, so the land either side of the lake is a causeway, the only two
//! ways across; a town island lies across a deep channel on either flank. The
//! main island is two lobes with a waist round the lake, its coast broken into
//! coves, points and bays.
//!
//! The ground between each start and the lake is open lowland, with no plateau
//! or hill in front of a base. The relief is behind and beside each start.
//! Everything designed is an even function of the folded coordinates, so it
//! is smooth across the fold axes too.

use super::Terrain;
use crate::noise::{bump, smoothstep};

/// How far each start sits from the middle, as a share of the map's size.
pub(super) const START_R: f64 = 0.25;

/// The main island's half length and half width, as shares of the map's size.
const OUTLINE: (f64, f64) = (0.37, 0.22);
/// How much narrower the main island is at its waist, round the lake, than
/// across its lobes: what makes the land either side of the lake a causeway.
const WAIST: f64 = 0.45;

impl Terrain {
    /// A few sea stacks per quadrant, strung along the main island's outline a
    /// little way out to sea, clear of the town channels.
    pub(super) fn lay_islets(&mut self) {
        let size = self.size;
        let (a, b) = OUTLINE;
        let mut rng = mc_core::Rng::new(self.seed ^ 0x6973_6C65);
        for at in [0.20, 0.58, 0.95] {
            let angle: f64 = at + 0.24 * (rng.unit().to_f64() - 0.5);
            let out = 1.24 + 0.10 * rng.unit().to_f64();
            let radius = (0.008 + 0.014 * rng.unit().to_f64().powi(2)) * size;
            self.islets.push((
                a * size * out * angle.cos(),
                b * size * out * angle.sin(),
                radius,
            ));
        }
    }

    /// Folded position for sampling noise in the islands layout. A field
    /// sampled at the plain folded position is creased along the fold axes,
    /// which shows wherever a coast crosses one; this fold is rounded (zero
    /// gradient on the axis), so fields stay smooth across it.
    fn soft_fold(&self, px: f64, py: f64) -> (f64, f64) {
        let e = 0.02 * self.size;
        ((px * px + e * e).sqrt(), (py * py + e * e).sqrt())
    }

    /// How far inside the central lake's shoreline a folded position is, in
    /// metres (roughly; negative on land). Its shore is pushed about by noise
    /// on the scale of the lake itself.
    pub(super) fn lake_inside(&self, px: f64, py: f64) -> f64 {
        let s = self.size;
        let (sx, sy) = self.soft_fold(px, py);
        let e = ((px / (0.075 * s)).powi(2) + (py / (0.055 * s)).powi(2)).sqrt();
        0.06 * s
            * (1.0 - e
                + 0.30
                    * self
                        .lake_shore
                        .fbm(sx / (0.05 * s), sy / (0.05 * s), 3, 0.55))
    }

    /// How far inland of the main island's coast a folded position is, in
    /// metres: an ellipse along the start axis with a waist round the lake,
    /// its coast broken into coves, points and bays, which keep still round
    /// the starts and nearly so across the waist.
    fn main_inland(&self, px: f64, py: f64, d_start: f64) -> f64 {
        let s = self.size;
        let so = self.start_outer;
        let (sx, sy) = self.soft_fold(px, py);
        let (a, b) = OUTLINE;
        let waist = 1.0 - smoothstep(0.05 * s, 0.15 * s, px);
        let (a, b) = (a * s, b * s * (1.0 - WAIST * waist));
        let e = ((px / a).powi(2) + (py / b).powi(2)).sqrt();
        let g = ((px / (a * a)).powi(2) + (py / (b * b)).powi(2)).sqrt();
        let outline = (1.0 - e) * if g > 1e-12 { e / g } else { b };
        let still = 0.4 + 0.6 * smoothstep(so, 4.0 * so, d_start);
        let bays = 0.06 * s * self.coast.fbm(sx / (0.11 * s), sy / (0.11 * s), 4, 0.55)
            + 0.06 * s * self.coast_warp.get(sx / (0.28 * s), sy / (0.28 * s));
        // The coves only along the coast, not out at sea, where they raised shoals.
        let coves = 0.07
            * s
            * self
                .coast
                .fbm(sx / (0.06 * s) + 9.0, sy / (0.06 * s), 4, 0.6)
            * (1.0 - smoothstep(0.02 * s, 0.06 * s, -outline));
        outline + (bays + coves) * still * (1.0 - 0.85 * waist)
    }

    pub(super) fn natural_islands(&self, x: f64, y: f64) -> f64 {
        let s = self.size;
        // Lengths given in metres (beaches, shore shelves) are for the 10 km
        // map and follow the size only so far: a beach can be neither a cliff
        // nor a plain.
        let k = (s / 10_240.0).clamp(0.8, 1.25);
        let (px, py, _) = self.fold(x, y);
        let (sx, sy) = self.soft_fold(px, py);
        let d_start = ((px - self.start_r).powi(2) + py * py).sqrt();
        let d_town = ((px - self.town.0).powi(2) + (py - self.town.1).powi(2)).sqrt();
        let so = self.start_outer;

        // The main island keeps clear of the town islands, so the channel
        // between them stays wide and deep (a smooth minimum with the distance
        // to a circle round the town island).
        let d_main = self.main_inland(px, py, d_start);
        let d_clear = d_town - 0.20 * s;
        let d_main =
            0.5 * (d_main + d_clear - ((d_main - d_clear).powi(2) + (100.0 * k).powi(2)).sqrt());
        // The town islands: rough ovals lying along the channel.
        let e_sec = ((px / (0.125 * s)).powi(2) + ((py - self.town.1) / (0.10 * s)).powi(2)).sqrt();
        let d_sec = 0.11
            * s
            * (1.0 - e_sec
                + 0.40
                    * self
                        .coast
                        .fbm(sx / (0.08 * s) + 40.0, sy / (0.08 * s), 3, 0.55)
                + 0.2 * self.coast_warp.get(sx / (0.15 * s) + 40.0, sy / (0.15 * s)));
        let inland_m = d_main.max(d_sec);

        // Interior relief is the basin's terrace field, but only behind and
        // beside the starts: everything from a start to the lake, the
        // causeways and the town islands is lowland, and so is a coastal strip
        // all the way round (the field is capped by the distance inland),
        // which is what guarantees a ground route from each start to both
        // causeways.
        let lake_in = self.lake_inside(px, py);
        let wf = 1.0 / (0.6 * self.l_cont);
        let wx = self.warp_x.get(sx * wf, sy * wf) * 0.15 * self.l_cont;
        let wy = self.warp_y.get(sx * wf, sy * wf) * 0.15 * self.l_cont;
        let mut c = 0.42
            + 1.0
                * self
                    .cont
                    .fbm((sx + wx) / self.l_cont, (sy + wy) / self.l_cont, 4, 0.5);
        let rough = 1.0 + 0.3 * self.detail.get(sx / (1.5 * so), sy / (1.5 * so));
        c += (0.15 - c) * bump(d_start * rough / (3.0 * so));
        c += (0.15 - c) * bump(-lake_in.min(0.0) / (0.10 * s));
        // In front of the base: from a little behind the start to the lake.
        let front = 1.0 - smoothstep(self.start_r - 0.02 * s, self.start_r + 0.05 * s, px);
        c += (0.15 - c) * front;
        let u = (inland_m / (1500.0 * k)).min(c.max(0.12));
        // 0 round the starts, in front of them and by the lake; 1 out behind and beside.
        let lobes = smoothstep(so, 2.5 * so, d_start)
            * (1.0 - front)
            * smoothstep(0.03 * s, 0.07 * s, -lake_in);

        // Same shore and terrace profile as the basin, on a wider beach.
        let ramp = self
            .ramp
            .get(sx / (0.35 * self.l_cont), sy / (0.35 * self.l_cont));
        let half = 0.006 + 0.10 * smoothstep(0.15, 0.55, ramp);
        let mut h = -8.0 + (super::TERRACES[0] + 8.0) * smoothstep(-0.08, 0.08, u);
        h -= 40.0 * smoothstep(0.08, 0.45, -u);
        h += (super::TERRACES[1] - super::TERRACES[0]) * smoothstep(0.40 - half, 0.40 + half, u);
        h += (super::TERRACES[2] - super::TERRACES[1]) * smoothstep(0.80 - half, 0.80 + half, u);
        let land = smoothstep(0.0, 0.1, u);
        h += 2.5 * land * self.tilt.get(sx / 1800.0, sy / 1800.0);

        // Noise-driven ranges and ponds as in the basin, behind and beside the starts only.
        let mut m = 0.0;
        let inland = smoothstep(300.0 * k, 520.0 * k, inland_m) * lobes;
        if inland > 0.0 {
            let mask = smoothstep(
                -0.25,
                0.05,
                self.mtn_mask
                    .get(sx / (2.4 * self.l_mtn), sy / (2.4 * self.l_mtn)),
            );
            let (mx, my) = ((sx + 0.5 * wx) / self.l_mtn, (sy + 0.5 * wy) / self.l_mtn);
            let line = smoothstep(0.88, 0.985, 1.0 - self.mtn.get(mx, my).abs());
            let gap = smoothstep(
                -0.35,
                -0.05,
                self.mtn_gap
                    .get(sx / (0.7 * self.l_mtn), sy / (0.7 * self.l_mtn)),
            );
            m = line * mask * gap * inland;
        }
        let crag = self.crag.ridged(sx / 700.0, sy / 700.0, 4, 0.5);
        if m > 0.0 {
            let tall = 250.0
                + 70.0
                    * self
                        .mtn_height
                        .get(sx / (1.3 * self.l_mtn), sy / (1.3 * self.l_mtn));
            h += self.mtn_scale * m * (tall + 120.0 * (crag - 0.4));
        }
        let lowland = land
            * (1.0 - smoothstep(0.30, 0.38, u))
            * lobes
            * smoothstep(300.0 * k, 500.0 * k, inland_m);
        if lowland > 0.0 {
            let pond = smoothstep(0.45, 0.7, self.lake.get(sx / self.l_lake, sy / self.l_lake));
            h += (-7.0 - h) * pond * lowland;
        }

        // A rocky knoll on the seaward side of each town island.
        let d_knoll = (px * px + (py - self.town.1 - 0.05 * s).powi(2)).sqrt();
        h += (22.0 + 40.0 * crag) * bump(d_knoll / (0.04 * s)) * smoothstep(0.0, 300.0 * k, d_sec);

        // The lake: deep enough in the middle that land units go around.
        let lake = smoothstep(-100.0 * k, 200.0 * k, lake_in);
        h += (-9.0 - 9.0 * smoothstep(0.0, 0.05 * s, lake_in) - h) * lake;

        h += (1.0 + 5.0 * m) * 1.4 * self.detail.fbm(sx / 350.0, sy / 350.0, 3, 0.45);

        // Sea stacks off the main island's coast.
        for &(ix, iy, radius) in &self.islets {
            let d = ((px - ix).powi(2) + (py - iy).powi(2)).sqrt();
            if d < radius {
                h += (24.0 + 36.0 * crag - h) * smoothstep(0.1, 0.7, bump(d / radius));
            }
        }
        // Open sea all the way round, whatever the noise did.
        let edge = x.min(y).min(self.size_x - x).min(self.size_y - y);
        h + (-48.0 - h) * (1.0 - smoothstep(0.015 * s, 0.04 * s, edge))
    }
}
