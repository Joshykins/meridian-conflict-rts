//! The survival maps' Shaper sites as the bake cuts them (The Last Harvest,
//! Rimehold): each site's level room, a naval site's pad and slips, and the
//! way from it toward the defenders, its floor no steeper than [`GRADE`], cut
//! exactly after the erosion so it stays level and drivable whatever the noise
//! did. Each layout lists its own sites and says how its ground is read; the
//! cutting is the same.

use super::bays::{dist, segment};
use super::canyon::smooth_open;
use super::Terrain;
use crate::noise::smoothstep;

/// What a site raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// Prints units onto platforms in front of it.
    Printer,
    /// Raises an auxiliary Shaper (whichever kind the sim picks): hidden behind
    /// the printers.
    Auxiliary,
    /// Kept for the Gatewright, whose fleets come by air: always raises one once its
    /// tier comes.
    Fleet,
}

/// Where a site's Shaper stands: on dry ground, or on an islet or shore pad
/// with its slips in deep water in front of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ground {
    Land,
    Naval,
}

/// A Shaper site.
pub(super) struct Site {
    pub(super) name: &'static str,
    pub(super) at: (f64, f64),
    pub(super) kind: Kind,
    pub(super) ground: Ground,
    /// The room's level (land), or the Shaper's pad's (naval), metres over the water.
    pub(super) level: f64,
    /// A printer's way toward the defenders, after `at`; an auxiliary site's
    /// narrow way in, from `at` out to ground the rest of the map reaches.
    pub(super) way: &'static [(f64, f64)],
    /// How the way's walls stand where it cuts into higher ground: metres out
    /// per metre up (sheer canyon walls in the desert, valley sides in the
    /// mountains).
    pub(super) cut: f64,
}

/// The room a printing site's base grows into, and an auxiliary one's, metres.
pub(super) const PRINTER_ROOM: f64 = 450.0;
const AUX_ROOM: f64 = 350.0;
/// Half a printing site's way's width, and an auxiliary site's narrow way's;
/// half a sea lane's.
const WAY_HALF: f64 = 130.0;
const NARROW_HALF: f64 = 36.0;
const LANE_HALF: f64 = 160.0;
/// How deep a sea lane and a naval site's slips are kept, metres.
pub(super) const LANE_DEEP: f64 = 18.0;
/// The steepest a way climbs, rise over run.
const GRADE: f64 = 0.09;

impl Site {
    pub(super) fn room(&self) -> f64 {
        match self.kind {
            Kind::Printer => PRINTER_ROOM,
            _ => AUX_ROOM,
        }
    }

    pub(super) fn half(&self) -> f64 {
        match (self.kind, self.ground) {
            (Kind::Printer, Ground::Land) => WAY_HALF,
            (Kind::Printer, Ground::Naval) => LANE_HALF,
            _ => NARROW_HALF,
        }
    }
}

/// The most a way's or a room's walls reach out from its floor, metres.
pub(super) const SHOULDER: f64 = 900.0;
/// The most a room's level ground runs out past it in lobes.
pub(super) const ROOM_LOBES: f64 = 280.0;
/// Metres out per metre up an embankment, and up the ground a way climbs
/// through in the lowland.
pub(super) const FILL: f64 = 4.0;
/// The most an embankment spreads.
pub(super) const FILL_MOST: f64 = 260.0;
pub(super) const LOW_CUT: f64 = 6.0;
/// Metres between a way's profile's samples.
const STEP: f64 = 16.0;
/// The ways' index: cells this big across the map.
const CELL: f64 = 512.0;
const CELLS: usize = 48;

/// One way as cut: its line, its floor along it, its walls.
pub(super) struct Way {
    line: Vec<(f64, f64)>,
    /// Metres along the line at each of its points.
    run: Vec<f64>,
    /// The floor's height every [`STEP`] metres along; empty for a sea lane.
    pub(super) profile: Vec<f64>,
    pub(super) half: f64,
    pub(super) cut: f64,
    /// A sea lane, kept deep, not a floor cut level.
    pub(super) sea: bool,
    /// A printing site's way through the mountains: a valley is shaped
    /// round it before the erosion runs.
    valley: Option<(f64, f64)>,
    /// Metres from the line past which the way changes nothing.
    reach: f64,
    /// For each index cell, the segments that may lie within `reach` of it.
    cells: Vec<Vec<u16>>,
}

impl Way {
    pub(super) fn new(site: &Site) -> Way {
        let (line, half, cut) = (site_line_points(site), site.half(), site.cut);
        let mut run = vec![0.0];
        for w in line.windows(2) {
            run.push(run.last().unwrap() + dist(w[0], w[1]));
        }
        let reach = half + SHOULDER + 20.0;
        let mut cells = vec![Vec::new(); CELLS * CELLS];
        for (k, w) in line.windows(2).enumerate() {
            let lo = (w[0].0.min(w[1].0) - reach, w[0].1.min(w[1].1) - reach);
            let hi = (w[0].0.max(w[1].0) + reach, w[0].1.max(w[1].1) + reach);
            let span = |a: f64, b: f64| {
                let i0 = ((a / CELL).floor().max(0.0) as usize).min(CELLS - 1);
                let i1 = ((b / CELL).floor().max(0.0) as usize).min(CELLS - 1);
                i0..=i1
            };
            for j in span(lo.1, hi.1) {
                for i in span(lo.0, hi.0) {
                    cells[j * CELLS + i].push(k as u16);
                }
            }
        }
        Way {
            line,
            run,
            profile: Vec::new(),
            half,
            cut,
            sea: site.ground == Ground::Naval,
            valley: (site.kind == Kind::Printer && site.ground == Ground::Land && cut >= 0.8)
                .then_some((site.level, site.room())),
            reach,
            cells,
        }
    }

    /// Metres from the way's line and along it, within its reach.
    pub(super) fn near(&self, p: (f64, f64)) -> Option<(f64, f64)> {
        let cell = |v: f64| ((v / CELL).floor().max(0.0) as usize).min(CELLS - 1);
        let mut best = (f64::INFINITY, 0.0);
        for &k in &self.cells[cell(p.1) * CELLS + cell(p.0)] {
            let k = k as usize;
            let (d, t) = segment(p, self.line[k], self.line[k + 1]);
            if d < best.0 {
                best = (d, self.run[k] + t * (self.run[k + 1] - self.run[k]));
            }
        }
        (best.0 <= self.reach).then_some(best)
    }

    /// The floor at `along` metres.
    pub(super) fn floor(&self, along: f64) -> f64 {
        let f = (along / STEP).clamp(0.0, (self.profile.len() - 1) as f64);
        let i = (f as usize).min(self.profile.len().saturating_sub(2));
        let t = f - i as f64;
        self.profile[i] * (1.0 - t) + self.profile[(i + 1).min(self.profile.len() - 1)] * t
    }

    /// Points every `every` metres along the line, the last one included.
    #[cfg(test)]
    pub(super) fn points(&self, every: f64) -> Vec<(f64, f64)> {
        self.points_from(0.0, every)
    }

    /// Points from `start` metres along to the end, about `every` metres
    /// apart, the last one included.
    #[cfg(test)]
    pub(super) fn points_from(&self, start: f64, every: f64) -> Vec<(f64, f64)> {
        let total = *self.run.last().unwrap();
        let n = ((total - start) / every).ceil().max(1.0) as usize;
        (0..=n)
            .map(|i| self.at(start + (total - start) * i as f64 / n as f64))
            .collect()
    }

    fn at(&self, along: f64) -> (f64, f64) {
        let k = self
            .run
            .windows(2)
            .position(|w| along <= w[1])
            .unwrap_or(self.run.len() - 2);
        let t = ((along - self.run[k]) / (self.run[k + 1] - self.run[k]).max(1e-9)).clamp(0.0, 1.0);
        let (a, b) = (self.line[k], self.line[k + 1]);
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }
}

/// A site's way's line, the site first, rounded.
fn site_line_points(s: &Site) -> Vec<(f64, f64)> {
    let mut pts = vec![s.at];
    pts.extend_from_slice(s.way);
    smooth_open(&pts, 8)
}

/// Metres from the nearest of `ways`' lines, less its half width.
pub(super) fn off_ways(ways: &[Way], p: (f64, f64)) -> f64 {
    ways.iter()
        .filter_map(|w| w.near(p).map(|(d, _)| d - w.half))
        .fold(f64::INFINITY, f64::min)
}

impl Terrain {
    /// A site's room cut level; a naval site's pad and, in front of it, its
    /// slips' water.
    pub(super) fn site_room(&self, s: &Site, p: (f64, f64), h: f64) -> f64 {
        let d = dist(p, s.at);
        let reach = s.room() + 40.0;
        if d > reach + ROOM_LOBES + SHOULDER {
            return h;
        }
        if s.ground == Ground::Naval {
            return self.site_slips(s, p, d, h);
        }
        // The level ground runs out past the room in lobes, and its shoulders
        // wander: a floor the ground has, not a disc cut into it. A printer's
        // room in the hills or the mountains is a valley's floor, the slopes
        // coming down to it long and concave; the desert's rooms keep the
        // canyon's sheer walls, and the hidden sites the steep sides of their
        // rings, which must not open.
        let lobes = ROOM_LOBES
            * self
                .mtn_gap
                .fbm(p.0 / 380.0 + 1.7, p.1 / 380.0, 3, 0.5)
                .max(0.0)
            + 60.0 * self.ridge.fbm(p.0 / 140.0, p.1 / 140.0 - 3.0, 2, 0.5);
        let core = reach + lobes.max(0.0);
        let valley = s.kind == Kind::Printer && s.cut >= 0.5;
        let run = match () {
            _ if h <= s.level => FILL,
            _ if valley => s.cut.max(3.4),
            _ => s.cut.max(1.0),
        };
        let edge = core
            + (run
                * (h - s.level).abs()
                * (0.75 + 0.6 * self.detail.get(p.0 / 260.0, p.1 / 260.0))
                + 30.0)
                .min(SHOULDER);
        let t = smoothstep(core, edge, d);
        // (A valley's foot: the slope eases out onto the floor.)
        let t = if valley { t.powf(1.5) } else { t };
        h + (s.level - h) * (1.0 - t)
    }

    /// A naval site: the Shaper's pad level on an islet or a shore, the land
    /// rising behind it into a hill, its coast wandering; in front, a quay and
    /// the slips' water, deep.
    fn site_slips(&self, s: &Site, p: (f64, f64), d: f64, mut h: f64) -> f64 {
        let first = s.way[0];
        let f = dist(first, s.at);
        let (fx, fy) = ((first.0 - s.at.0) / f, (first.1 - s.at.1) / f);
        let u = (p.0 - s.at.0) * fx + (p.1 - s.at.1) * fy;
        let wander = |k: f64, l: f64| self.coast.fbm(p.0 / l + k, p.1 / l - k, 3, 0.5);
        let pad = 200.0 + 50.0 * wander(3.0, 240.0);
        let coast = 400.0 + 170.0 * wander(-5.0, 330.0);
        // Behind the pad the ground climbs; at its sides it falls to a beach.
        let behind = smoothstep(40.0, -260.0, u);
        let hill = 55.0
            * behind
            * smoothstep(pad, coast, d)
            * (0.6 + 0.4 * self.crag.ridged(p.0 / 260.0, p.1 / 260.0, 2, 0.5));
        let shore = 1.0 - smoothstep(coast - 30.0, coast + 90.0 * (1.0 - behind) + 30.0, d);
        let ground = s.level + hill - (s.level + 3.0) * smoothstep(coast - 90.0, coast + 30.0, d);
        let quay = 95.0 + 35.0 * wander(9.0, 180.0);
        let land = shore * (1.0 - smoothstep(quay, quay + 25.0, u));
        if land > 0.0 && (h < ground || d < coast) {
            h += (ground - h) * land;
        }
        // The pad itself dead level.
        h += (s.level - h)
            * (1.0 - smoothstep(pad, pad + 30.0, d))
            * (1.0 - smoothstep(quay, quay + 25.0, u));
        let reach = s.room() + 40.0;
        let wet = (1.0 - smoothstep(reach + 20.0, reach + 140.0, d))
            * smoothstep(quay + 15.0, quay + 45.0, u);
        let deep = -LANE_DEEP - 4.0;
        if h > deep {
            h += (deep - h) * wet;
        }
        h
    }

    /// A printing site's valley down out of the mountains: the ground round
    /// its way's line eased down toward a floor falling from the site's level,
    /// walls wandering in and out, before the erosion furrows them.
    pub(super) fn site_valleys(&self, ways: &[Way], p: (f64, f64), mut h: f64, low: f64) -> f64 {
        for way in ways {
            let Some((level, room)) = way.valley else {
                continue;
            };
            let Some((d, along)) = way.near(p) else {
                continue;
            };
            let floor = (level - 0.085 * (along - room).max(0.0)).max(low.min(level));
            if h <= floor {
                continue;
            }
            let wide = 520.0 + 380.0 * self.mtn_gap.fbm(along / 1_400.0, 3.7, 2, 0.5);
            let wander = 160.0 * self.ridge.fbm(p.0 / 700.0 + 1.3, p.1 / 700.0, 3, 0.5);
            let shape = smoothstep(0.5 * way.half, way.half + wide, d + wander).powf(0.8);
            h = floor + (h - floor) * shape;
        }
        h
    }

    /// A way's floor: the ground along it (`ground`, before the cuts),
    /// smoothed, held level through the rooms of `sites` it leaves and enters,
    /// never under 2.5 m over the water (fords and sandbars), and no steeper
    /// than [`GRADE`] anywhere.
    pub(super) fn way_profile(
        way: &Way,
        sites: &[Site],
        ground: impl Fn((f64, f64)) -> f64,
    ) -> Vec<f64> {
        let total = *way.run.last().unwrap();
        let n = (total / STEP).ceil() as usize + 1;
        let pts: Vec<(f64, f64)> = (0..n).map(|i| way.at(i as f64 * STEP)).collect();
        let raw: Vec<f64> = pts.iter().map(|&q| ground(q)).collect();
        // The rooms the way runs through hold it at their level.
        let lock: Vec<Option<f64>> = pts
            .iter()
            .map(|&q| {
                sites
                    .iter()
                    .find(|s| dist(q, s.at) < s.room() + 60.0)
                    .map(|s| s.level)
            })
            .collect();
        // The lowest floor no steeper than the grade anywhere (cutting through
        // what stands in the way), then held within the grade's cones from
        // every room it touches: each room's level exactly, and from there
        // down (or up) to the ground no steeper than the grade.
        let rise = GRADE * STEP;
        let mut low: Vec<f64> = raw.iter().map(|v| v.max(2.5)).collect();
        for i in 1..n {
            low[i] = low[i].min(low[i - 1] + rise);
        }
        for i in (0..n - 1).rev() {
            low[i] = low[i].min(low[i + 1] + rise);
        }
        let locks: Vec<(usize, f64)> = lock
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.map(|v| (i, v)))
            .collect();
        let z = low
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let cone = |sign: f64| {
                    locks
                        .iter()
                        .map(move |&(k, l)| l + sign * rise * (i as f64 - k as f64).abs())
                };
                let under = cone(-1.0).fold(f64::NEG_INFINITY, f64::max);
                let over = cone(1.0).fold(f64::INFINITY, f64::min);
                v.clamp(under.min(over), over)
            })
            .collect();
        z
    }
}

/// A site's sidecar line: its room, its kind and its way (`path`), map metres.
/// An auxiliary site faces `toward` (the defenders' middle).
#[cfg(test)]
pub(super) fn site_line(s: &Site, path: &[(f64, f64)], toward: (f64, f64)) -> String {
    let kind = match s.kind {
        Kind::Printer => "Printer",
        Kind::Auxiliary => "Auxiliary",
        Kind::Fleet => "Fleet",
    };
    let domain = match s.ground {
        Ground::Land => "Land",
        Ground::Naval => "Naval",
    };
    let path = path
        .iter()
        .map(|p| format!("({:.0}, {:.0})", p.0, p.1))
        .collect::<Vec<_>>()
        .join(", ");
    let facing = if s.kind == Kind::Printer {
        "None".to_string()
    } else {
        let deg = (toward.1 - s.at.1)
            .atan2(toward.0 - s.at.0)
            .to_degrees()
            .rem_euclid(360.0);
        format!("Some({deg:.0})")
    };
    format!(
        "    (name: \"{}\", at: ({:.0}, {:.0}), domain: {domain}, facing: {facing}, kind: {kind}, room: {:.0}, path: [{path}]),\n",
        s.name,
        s.at.0,
        s.at.1,
        s.room()
    )
}
