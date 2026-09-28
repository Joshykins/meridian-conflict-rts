//! [`Layout::Threshold`]: "The Threshold", the survival map. One road runs the
//! length of a coast, west to east, between a mountain wall on the north and the
//! sea on the south. The west half is the defenders' country: a broad coastal
//! lowland of woods, meadow and beaches, pinched in the middle to a single pass
//! where a mountain spur comes down to a headland. The east half is the
//! Precursor facility, and it is the enemy: nothing of it stands apart as "the
//! engine", every part of the machine is built into one installation.
//!
//! * The Rampart: a 90 m stepped face of Precursor casing across the whole band,
//!   from the sea cliffs to the mountains, and one road up it: a causeway and a
//!   cut ramp under the Gate. Behind it the plateau, the facility's floor.
//! * The Forges: two print halls flanking the processional inside the Gate,
//!   four bays each; survival prints its rounds in them.
//! * The Heart: the central spire, 2.8 km, its Lens hovering 720 m up, where the
//!   ray that raises Shapers leaves. Halos float round it far over the clouds.
//! * Cradles: berths across the plateau and out on the water where Shapers form
//!   in rows of three, more of them as the rounds climb.
//! * The Sea Gate: a harbour cut into the plateau's sea cliff; ships are
//!   printed on its slips.
//! * In the mountains, the megastructure (`machine.rs`): bastions cut into the
//!   slopes with their booms reaching out over the plateau, towers into the
//!   sky, spans between them and off the map. Out over the sea, platforms on
//!   legs; everywhere over it, monoliths hovering above the cloud deck and
//!   needles standing through it.
//!
//! Laid out in metres for the 16.384 km map (8 x 8 tiles); another size scales
//! it (mostly for tests). The sidecar `maps/threshold.ron` carries the same
//! coordinates for the survival rules (`survival_block` prints them).

use super::machine::{dist, Machine, PrecursorSite};
use super::{Layout, Pad, Terrain};
use crate::format::PropKind;
use crate::noise::{bump, smoothstep};
use crate::BUILD_CELL_M;
use std::f64::consts::{FRAC_PI_2, PI};

/// The size the design coordinates below are written for.
const DESIGN_M: f64 = 16_384.0;

/// Defender starts: the landing (middle), the shore, the high fold.
pub(crate) const SPAWNS: [(f64, f64); 3] =
    [(2_304.0, 7_704.0), (3_708.0, 6_300.0), (1_908.0, 9_300.0)];
/// The Heart; the facility side's start position.
pub(crate) const HEART: (f64, f64) = (13_404.0, 8_304.0);

/// The coast as y at x (sea to the south), and the mountains' foot (mountains
/// to the north). Between them the one road.
const COAST: [(f64, f64); 18] = [
    (-600.0, 5_050.0),
    (900.0, 5_350.0),
    (1_900.0, 4_850.0),
    (2_900.0, 5_100.0),
    (3_900.0, 5_600.0),
    (4_800.0, 5_150.0),
    (5_700.0, 5_450.0),
    (6_500.0, 6_250.0),
    (7_200.0, 7_150.0),
    (7_800.0, 7_480.0),
    (8_300.0, 7_300.0),
    (8_800.0, 6_700.0),
    (9_300.0, 6_300.0),
    (9_900.0, SEA_WALL_Y),
    (11_500.0, SEA_WALL_Y),
    (13_500.0, SEA_WALL_Y),
    (15_200.0, SEA_WALL_Y),
    (17_000.0, SEA_WALL_Y),
];
/// The plateau's sea wall: dead straight along y, cased with the Rampart's
/// courses at this scale (three of 42 m, each 19.6 m behind the one below).
const SEA_WALL_Y: f64 = 6_160.0;
const SEA_WALL_SCALE: f64 = 1.4;
const FOOT: [(f64, f64); 15] = [
    (-600.0, 10_350.0),
    (1_200.0, 10_150.0),
    (2_600.0, 10_450.0),
    (4_000.0, 10_250.0),
    (5_200.0, 10_050.0),
    (6_100.0, 9_600.0),
    (6_900.0, 9_050.0),
    (7_600.0, 8_780.0),
    (8_300.0, 8_760.0),
    (8_900.0, 9_150.0),
    (9_500.0, 9_900.0),
    (10_200.0, 10_450.0),
    (12_000.0, 10_750.0),
    (14_500.0, 10_700.0),
    (17_000.0, 10_900.0),
];
/// How far the mountains take to rise from their foot: a wall, not a slope.
const WALL: f64 = 300.0;
/// The shelf the machine stands on: a terrace cut level into the mountainside
/// over the plateau, from its west end to the map's edge (design y, and level).
const SHELF: (f64, f64, f64) = (10_900.0, 11_520.0, 380.0);
const SHELF_WEST: f64 = 9_450.0;

/// The forecourt below the Rampart, and the plateau behind it.
const FORECOURT_H: f64 = 32.0;
const PLATEAU_H: f64 = FORECOURT_H + 90.0;
/// The Rampart's foot, along y; its three 30 m courses step back 14 m each.
pub(crate) const RAMPART_X: f64 = 9_900.0;
const COURSE_RUN: f64 = 14.0;
/// The road up: its middle line, half its width, where the causeway leaves the
/// forecourt and where the cut reaches the plateau. The Gate stands past its top.
pub(crate) const ROAD_Y: f64 = 8_304.0;
const RAMP_HALF: f64 = 150.0;
const RAMP_X0: f64 = 9_150.0;
const RAMP_X1: f64 = 10_450.0;
/// Where the Rampart's casing stops either side of the road.
const RAMP_GAP: f64 = 190.0;
pub(crate) const GATE: (f64, f64) = (10_560.0, ROAD_Y);

/// The two print halls, facing west down the processional (heading pi), at
/// half as big again as the model.
pub(crate) const FORGES: [(f64, f64); 2] = [(11_484.0, 9_204.0), (11_484.0, 7_404.0)];
const FORGE_SCALE: f64 = 1.5;
/// Their bays across the hall (model y), the printed unit's stand and the
/// projector over it (model x, height): `precursor_forge.rs`.
#[cfg(test)]
const FORGE_BAYS: [f64; 4] = [-255.0, -85.0, 85.0, 255.0];
#[cfg(test)]
const FORGE_STAND: f64 = -40.0;
#[cfg(test)]
const FORGE_EMITTER: (f64, f64) = (-115.0, 110.0);

/// The Sea Gate: ships leave it southward (heading -pi/2) out of a basin cut
/// into the plateau's sea cliff. Slips across the channel; `precursor_forge.rs`.
pub(crate) const SEA_GATE: (f64, f64) = (12_600.0, 5_988.0);
const SEA_GATE_HEADING: f64 = -FRAC_PI_2;
#[cfg(test)]
const SLIPS: [f64; 3] = [-110.0, 0.0, 110.0];
#[cfg(test)]
const SLIP_X: f64 = -150.0;
#[cfg(test)]
const SLIP_EMITTER: (f64, f64) = (-230.0, 140.0);
/// The basin in the sea gate's frame: from the back wall to past the mouth,
/// and half its width (the moles stand inside it, in water).
const BASIN: (f64, f64, f64) = (-430.0, 400.0, 300.0);

/// Cradles where Shapers form: name, where, the way they face (degrees), and
/// whether they stand on the water.
pub(crate) const CRADLES: [(&str, (f64, f64), f64, bool); 13] = [
    ("Gatewatch North", (10_806.0, 9_798.0), 180.0, false),
    ("Gatewatch South", (10_806.0, 6_810.0), 180.0, false),
    ("The Nave", (12_306.0, 8_304.0), 180.0, false),
    ("Lantern Berth", (12_426.0, 9_978.0), 180.0, false),
    ("Tidewall Berth", (12_306.0, 6_654.0), 180.0, false),
    ("The Choir", (14_406.0, 9_738.0), 180.0, false),
    ("Undercroft", (14_406.0, 6_858.0), 180.0, false),
    ("The Reliquary", (15_306.0, 8_304.0), 180.0, false),
    ("Far Berth North", (15_906.0, 9_858.0), 180.0, false),
    ("Far Berth South", (15_906.0, 6_750.0), 180.0, false),
    ("Drowned Cradle", (11_406.0, 5_206.0), 180.0, true),
    ("The Sounding", (13_806.0, 5_206.0), 0.0, true),
    ("Deep Berth", (12_606.0, 3_906.0), 270.0, true),
];

/// Floating and towering pieces: Halo scale and heading, Monolith, Needle, Platform.
const HALOS: [((f64, f64), f64, f64); 2] = [(HEART, 1.25, 0.0), (HEART, 0.95, FRAC_PI_2)];
/// A matched pair hovering on the processional's line, either side of the Heart.
const MONOLITHS: [((f64, f64), f64, f64); 2] = [
    ((11_404.0, ROAD_Y), 1.8, 0.0),
    ((15_404.0, ROAD_Y), 1.8, PI),
];
/// Needles in pairs, mirrored across the processional.
const NEEDLES: [((f64, f64), f64); 4] = [
    ((10_350.0, 10_050.0), 1.0),
    ((10_350.0, 6_558.0), 1.0),
    ((13_950.0, 10_200.0), 1.2),
    ((13_950.0, 6_408.0), 1.2),
];
/// Sea platforms either side of the Sea Gate's lane.
const PLATFORMS: [((f64, f64), f64); 2] = [
    ((12_000.0, 5_300.0), FRAC_PI_2),
    ((13_200.0, 5_300.0), FRAC_PI_2),
];

/// The Great Forge: the same hall at three times the size, at the head of the
/// processional; T4s and spacecraft are printed in its bays.
pub(crate) const GREAT_FORGE: (f64, f64) = (16_200.0, ROAD_Y);
const GREAT_SCALE: f64 = 3.0;
/// The biggest unit a bay takes (collision radius): forge bays, the Great Forge's,
/// the aeries', the side slips' and the great centre slip's.
#[cfg(test)]
const FORGE_FITS: f64 = 45.0;
#[cfg(test)]
const GREAT_FITS: f64 = 180.0;
#[cfg(test)]
const AERIE_FITS: f64 = 40.0;
#[cfg(test)]
const SLIP_FITS: f64 = 38.0;
#[cfg(test)]
const GREAT_SLIP_FITS: f64 = 180.0;
/// The tower ring round the Heart; each has an aerie at its foot on its outer
/// side, where aircraft are printed by a projector 220 m up the tower.
const RING: [(f64, f64); 4] = [
    (12_704.0, 7_604.0),
    (14_104.0, 7_604.0),
    (14_104.0, 9_004.0),
    (12_704.0, 9_004.0),
];
const AERIE_OUT: f64 = 230.0;
const AERIE_EMITTER: (f64, f64) = (60.0, 220.0);

/// An aerie's pad (x, y) and its projector head (x, y, height).
type Aerie = ((f64, f64), (f64, f64, f64));

/// The viaducts tying it all together: from one building's port to another's,
/// design metres. Ports sit inside the buildings (a tower's shaft, a hall's pier,
/// a cradle's end tower), so the decks run into them.
const LINKS: [((f64, f64), (f64, f64)); 25] = [
    // The Heart's spokes.
    (HEART, RING[0]),
    (HEART, RING[1]),
    (HEART, RING[2]),
    (HEART, RING[3]),
    // The Forges' backs to the ring, their fronts to the Gate's legs.
    ((11_850.0, 9_204.0), RING[3]),
    ((11_850.0, 7_404.0), RING[0]),
    ((10_560.0, 8_604.0), (11_424.0, 8_694.0)),
    ((10_560.0, 8_004.0), (11_424.0, 7_914.0)),
    // Bastions to the halls and the ring.
    ((11_350.0, 10_060.0), (11_350.0, 9_700.0)),
    ((13_300.0, 10_130.0), RING[2]),
    ((13_300.0, 6_870.0), RING[1]),
    // Cradles, by an end tower, to the nearest hall or tower.
    ((10_806.0, 9_648.0), (11_424.0, 9_714.0)),
    ((10_806.0, 6_960.0), (11_424.0, 6_894.0)),
    ((12_426.0, 9_828.0), (11_880.0, 9_700.0)),
    ((12_306.0, 6_804.0), (11_880.0, 6_908.0)),
    ((14_406.0, 9_588.0), RING[2]),
    ((14_406.0, 7_008.0), RING[1]),
    ((12_402.0, ROAD_Y), HEART),
    ((15_402.0, ROAD_Y), (16_080.0, ROAD_Y)),
    ((15_906.0, 9_708.0), (16_080.0, 9_324.0)),
    ((15_906.0, 6_900.0), (16_080.0, 7_284.0)),
    // The ring to the Great Forge's piers.
    (RING[1], (16_080.0, 7_794.0)),
    (RING[2], (16_080.0, 8_814.0)),
    // The Sea Gate's moles to the Tidewall cradle and the south bastion.
    ((12_370.0, 6_388.0), (12_306.0, 6_504.0)),
    ((12_830.0, 6_388.0), (13_140.0, 6_620.0)),
];

/// Ore: small fields at each start, a few more out along the road.
const CONTESTED_ORE: [(f64, f64); 3] = [(4_900.0, 7_700.0), (6_400.0, 8_300.0), (8_750.0, 8_000.0)];

impl Terrain {
    fn tv(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let f = self.size / DESIGN_M;
        (x * f, y * f)
    }

    fn tl(&self, l: f64) -> f64 {
        l * self.size / DESIGN_M
    }

    /// A y-at-x table, in map metres.
    fn th_table(&self, table: &[(f64, f64)], x: f64) -> f64 {
        let f = self.size / DESIGN_M;
        let xd = x / f;
        let i = table
            .windows(2)
            .position(|w| xd <= w[1].0)
            .unwrap_or(table.len() - 2);
        let (a, b) = (table[i], table[i + 1]);
        let t = ((xd - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        // Eased between the corners, so the line bends instead of kinking.
        let t = t * t * (3.0 - 2.0 * t);
        (a.1 + (b.1 - a.1) * t) * f
    }

    /// Starts, pads, ore, the woods and the facility.
    pub(super) fn setup_threshold(&mut self) {
        debug_assert_eq!(self.layout, Layout::Threshold);
        self.forest_edge = 0.12;
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        let g = BUILD_CELL_M as f64;
        let snap = |t: &Terrain, p: (f64, f64)| {
            let p = t.tv(p);
            ((p.0 / g).round() * g, (p.1 / g).round() * g)
        };
        let mut pads = Vec::new();
        for &s in &SPAWNS {
            let at = snap(self, s);
            pads.push(Pad {
                x: at.0,
                y: at.1,
                core: start_core,
                outer: 2.0 * start_core,
                height: self.natural(at.0, at.1).max(10.0),
            });
            self.starts.push(at);
        }
        self.starts.push(snap(self, HEART));

        let small = (0.18 * start_core).max(55.0);
        let mut sites = Vec::new();
        for &(sx, sy) in &self.starts[..SPAWNS.len()] {
            // Behind the start, away from the road east.
            for turn in [PI - 0.95, PI + 0.95] {
                let (s, c) = turn.sin_cos();
                let d = 0.8 * start_core;
                sites.push((sx + c * d, sy + s * d, small));
            }
        }
        for &p in &CONTESTED_ORE {
            let at = snap(self, p);
            sites.push((at.0, at.1, small * 1.1));
        }
        self.pads = pads;
        self.ore = sites
            .into_iter()
            .map(|(x, y, r)| self.ore_field((x / g).round() * g, (y / g).round() * g, r))
            .collect();

        // Water erosion over the mountains, from the landscape as laid so far.
        self.erode_alpine();
        self.precursor = self.lay_facility();
        self.lay_machine();
        self.fit_forests();
    }

    // -- the ground -------------------------------------------------------------

    /// Signed distance to the sea (positive on land), roughly: the coast table
    /// wobbled into bays and headlands on the defenders' side, straight under the
    /// plateau, and the Sea Gate's basin cut into it.
    pub(super) fn th_shore(&self, x: f64, y: f64) -> f64 {
        let coast = self.th_table(&COAST, x);
        let l = self.tl(700.0);
        // Bays and points on the lowland coast; the plateau's cliff runs true.
        let rough = 1.0 - smoothstep(self.tl(RAMPART_X - 900.0), self.tl(RAMPART_X), x);
        let mut sd = y - coast
            + rough
                * (self.tl(150.0) * self.coast.fbm(x / l, y / l, 3, 0.55)
                    + self.tl(70.0) * self.coast_warp.get(x / (0.35 * l), y / (0.35 * l)));
        // The Sea Gate's basin.
        let (lx, ly) = self.th_gate_frame(x, y);
        let (b0, b1, bh) = BASIN;
        if lx > self.tl(b0) - 40.0 && lx < self.tl(b1) + 400.0 {
            let inside = (self.tl(b0) - lx).max(ly.abs() - self.tl(bh));
            sd = sd.min(inside);
        }
        sd
    }

    /// A point in the Sea Gate's frame (x out to sea along its heading, y to its left).
    fn th_gate_frame(&self, x: f64, y: f64) -> (f64, f64) {
        let o = self.tv(SEA_GATE);
        let (s, c) = SEA_GATE_HEADING.sin_cos();
        let (dx, dy) = (x - o.0, y - o.1);
        (dx * c + dy * s, dy * c - dx * s)
    }

    /// Metres north of the mountains' foot (negative out on the road), wobbled.
    fn th_foot(&self, x: f64, y: f64) -> f64 {
        let l = self.tl(800.0);
        let wobble = self.tl(90.0) * self.ridge.fbm(x / l, y / l, 3, 0.5)
            + self.tl(35.0) * self.ridge.get(x / 170.0, y / 170.0);
        y - self.th_table(&FOOT, x) - wobble
    }

    /// The road's own floor: lowland to the pass, the forecourt, the plateau.
    fn th_floor(&self, x: f64, y: f64, shore: f64) -> f64 {
        let xd = x * DESIGN_M / self.size;
        // Lowland: rising inland off the beaches, rolling, drumlins and knolls.
        let inland = smoothstep(0.0, self.tl(2_200.0), shore);
        let roll = self.lake.fbm(x / 1_500.0, y / 1_500.0, 3, 0.5);
        let knolls = self.crag.ridged(x / 900.0 + 3.7, y / 900.0 - 1.3, 2, 0.5);
        let mut calm: f64 = 1.0;
        for &s in SPAWNS.iter().chain(CONTESTED_ORE.iter()) {
            calm = calm.min(smoothstep(
                self.tl(260.0),
                self.tl(800.0),
                dist((x, y), self.tv(s)),
            ));
        }
        let mut h = 5.0
            + 34.0 * inland
            + calm * (14.0 * roll + 16.0 * smoothstep(0.55, 0.9, knolls) * inland);
        // The pass: a saddle between the spur and the headland, a rocky knoll
        // over the sea that crowds the road against the mountains.
        let pass = bump(((xd - 7_750.0) / 1_300.0).abs());
        h += pass * 20.0;
        let knoll = dist((x, y), self.tv((7_850.0, 7_250.0))) / self.tl(760.0);
        let crags = 0.7 + 0.3 * self.crag.ridged(x / 260.0 + 1.7, y / 260.0 + 4.1, 2, 0.5);
        h += 150.0 * bump(knoll.min(1.0)).powf(0.8) * crags;
        // Down to the forecourt, level under the Rampart.
        let fc = smoothstep(8_500.0, 9_100.0, xd);
        h += (FORECOURT_H + 1.2 * roll - h) * fc;
        // The plateau behind the Rampart: three courses up.
        let mut step = 0.0;
        for k in 0..3 {
            let xk = self.tl(RAMPART_X + COURSE_RUN * k as f64 + 3.0);
            step += smoothstep(xk - 3.0, xk + 3.0, x) / 3.0;
        }
        // Dead level: the facility's paving lies on it.
        h += (PLATEAU_H - h) * step;
        // The road up: a causeway across the forecourt, then a cut through the casing.
        let (dy, ramp) = ((y - self.tl(ROAD_Y)).abs(), self.th_ramp(x));
        if let Some(r) = ramp {
            if x < self.tl(RAMPART_X) {
                // Causeway: its flanks fall away steeply to the forecourt.
                let w = 1.0
                    - smoothstep(
                        self.tl(RAMP_HALF),
                        self.tl(RAMP_HALF) + 1.6 * (r - h).max(4.0),
                        dy,
                    );
                h += (r - h) * w;
            } else {
                // Cut: sheer faces, cased (`lay_facility`).
                let w = 1.0 - smoothstep(self.tl(RAMP_HALF), self.tl(RAMP_HALF) + 8.0, dy);
                h += (r - h) * w;
            }
        }
        h
    }

    /// The ramp's height at x, where it runs.
    fn th_ramp(&self, x: f64) -> Option<f64> {
        let (x0, x1) = (self.tl(RAMP_X0), self.tl(RAMP_X1));
        if x < x0 - 60.0 || x > x1 + 60.0 {
            return None;
        }
        let t = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        Some(FORECOURT_H + (PLATEAU_H - FORECOURT_H) * t)
    }

    /// The mountains over a point `into` metres past their foot.
    fn th_mountains(&self, x: f64, y: f64, into: f64) -> f64 {
        let l = 2_400.0;
        let (wx, wy) = (
            600.0 * self.warp_x.get(x / l, y / l),
            600.0 * self.warp_y.get(x / l, y / l),
        );
        let big = self
            .crag
            .ridged((x + wx) / 2_100.0, (y + wy) / 2_100.0, 5, 0.5);
        let small = self.crag.ridged(x / 560.0 + 3.1, y / 560.0 - 7.7, 3, 0.5);
        let massif = smoothstep(0.0, self.tl(2_600.0), into);
        150.0
            + 420.0 * massif
            + (380.0 + 800.0 * massif) * big.max(0.0).powf(1.4)
            + 80.0 * small * big.max(0.0)
            + 60.0 * self.mtn_height.get(x / 2_900.0, y / 2_900.0)
    }

    /// The landscape before pads, metres above the water.
    pub(super) fn natural_threshold(&self, x: f64, y: f64) -> f64 {
        let shore = self.th_shore(x, y);
        let detail = self.detail.fbm(x / 300.0, y / 300.0, 3, 0.45);
        let xd = x * DESIGN_M / self.size;
        let plateau = smoothstep(RAMPART_X - 40.0, RAMPART_X + 40.0, xd);
        if shore <= 0.0 {
            // The basin: square cut, a level floor well under a keel.
            let (lx, ly) = self.th_gate_frame(x, y);
            let (b0, b1, bh) = BASIN;
            if lx > self.tl(b0) - 20.0 && lx < self.tl(b1) && ly.abs() < self.tl(bh) + 20.0 {
                return -24.0 + 0.3 * detail;
            }
            let deep = smoothstep(0.0, self.tl(900.0), -shore);
            return -4.0 - 46.0 * deep - 30.0 * deep * deep + 1.5 * detail;
        }
        let into = self.th_foot(x, y);
        let floor = self.th_floor(x, y, shore);
        let m = smoothstep(-40.0, self.tl(WALL), into);
        let mut h = floor;
        if m > 0.0 {
            let tall = self.th_mountains(x, y, into.max(0.0));
            h += m * (tall + floor.max(0.0) * 0.3 - floor * 0.3).max(0.0);
            h += self.erosion.at(x, y) * smoothstep(self.tl(60.0), self.tl(300.0), into);
            // The machine's shelf, cut (and built up) level into the mountainside.
            let yd = y * DESIGN_M / self.size;
            let band = smoothstep(SHELF.0 - 70.0, SHELF.0, yd)
                * (1.0 - smoothstep(SHELF.1, SHELF.1 + 50.0, yd));
            let shelf = band * smoothstep(SHELF_WEST - 60.0, SHELF_WEST, xd);
            h += (SHELF.2 + 0.6 * detail - h) * shelf * m;
        }
        h += (1.0 - plateau) * (1.0 - 0.6 * m) * 1.2 * detail;
        // Down to the water: beaches on the lowland, a headland of cliffs at the
        // pass, the plateau's sheer sea wall.
        let sea = -4.0;
        let headland = bump(((xd - 7_800.0) / 900.0).abs());
        let cut = self.tl(320.0) * (1.0 - 0.8 * headland);
        let cut = cut + (self.tl(60.0) - cut) * smoothstep(RAMPART_X - 700.0, RAMPART_X, xd);
        let natural = sea + (h - sea) * smoothstep(0.0, cut, shore);
        if plateau <= 0.0 {
            return natural;
        }
        // The sea wall: three courses, each just behind its casing's face.
        let run = self.tl(COURSE_RUN * SEA_WALL_SCALE);
        let mut up = 0.0;
        for k in 0..3 {
            let at = run * k as f64 + self.tl(3.0);
            up += smoothstep(at - 2.0, at + 2.0, shore) / 3.0;
        }
        natural + (sea + (h - sea) * up - natural) * plateau
    }

    /// Snow on the high peaks; none on the road.
    pub(super) fn threshold_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        let slope = (gx * gx + gy * gy).sqrt();
        let north = (-gy / slope.max(1e-3)) * smoothstep(0.05, 0.3, slope);
        let l = 1_400.0;
        let line = 720.0 + 90.0 * self.mtn_mask.get(x / l, y / l) - 140.0 * north;
        let snow = smoothstep(line - 60.0, line + 120.0, h) * (1.0 - smoothstep(1.3, 2.2, slope));
        (0.0, snow)
    }

    /// Woods on the lowland and up the mountains' lower slopes; none on the
    /// facility's ground or the beaches.
    pub(super) fn threshold_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let xd = x * DESIGN_M / self.size;
        if xd > RAMPART_X - 300.0 && self.th_foot(x, y) < self.tl(200.0) {
            return (0.0, 0.0);
        }
        let l = self.l_forest;
        let broad = self.forest.fbm(x / l, y / l, 3, 0.5);
        let forest = smoothstep(self.forest_edge, self.forest_edge + 0.22, broad);
        let copse = smoothstep(
            0.42,
            0.62,
            self.forest
                .fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5),
        );
        let clearing = smoothstep(
            0.30,
            0.55,
            self.forest
                .fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5),
        );
        let tree_line = 430.0 + 60.0 * self.forest_kind.get(x / 900.0, y / 900.0);
        let habitable = smoothstep(6.0, 14.0, height)
            * (1.0 - smoothstep(0.55, 0.95, slope))
            * (1.0 - smoothstep(tree_line - 80.0, tree_line, height));
        // Keep the road through the pass open; the starts' glades are cut in
        // `forest_density`.
        let road = (y - self.tl(ROAD_Y)).abs();
        let open = 0.35 + 0.65 * smoothstep(self.tl(120.0), self.tl(320.0), road);
        let density = (forest * (1.0 - 0.85 * clearing)).max(copse * 0.75) * habitable * open;
        let cold = smoothstep(80.0, 260.0, height)
            + 0.8 * self.forest_kind.fbm(x / 1_500.0, y / 1_500.0, 2, 0.5);
        (density, smoothstep(-0.1, 0.5, cold))
    }

    /// Trees and rocks stay off the facility's floor and clear of its pieces.
    pub(super) fn threshold_clear(&self, x: f64, y: f64) -> bool {
        let xd = x * DESIGN_M / self.size;
        if xd > RAMPART_X - 120.0 && self.th_foot(x, y) < self.tl(150.0) {
            return false;
        }
        !self.precursor.iter().any(|s| {
            let r = s.reach() + 12.0;
            (x - s.x).abs() < r && (y - s.y).abs() < r && dist((x, y), (s.x, s.y)) < r
        })
    }

    // -- the facility -------------------------------------------------------------

    /// Every piece of the facility on the road and the water; the machine in the
    /// mountains is `machine_threshold`.
    fn lay_facility(&self) -> Vec<PrecursorSite> {
        let f = self.size / DESIGN_M;
        let mut out = Vec::new();
        let mut put = |kind: PropKind, at: (f64, f64), heading: f64, scale: f64| {
            out.push(PrecursorSite {
                kind,
                x: at.0 * f,
                y: at.1 * f,
                heading,
                scale: scale * f,
            });
        };
        // The Rampart: 240 m lengths from the sea cliff to the mountains, facing
        // west, stopping either side of the road.
        let north = 10_500.0;
        let mut y = ROAD_Y + RAMP_GAP + 120.0;
        while y < north {
            put(PropKind::PrecursorRampart, (RAMPART_X, y), PI, 1.0);
            y += 240.0;
        }
        let mut y = SEA_WALL_Y + 120.0;
        while y + 120.0 <= ROAD_Y - RAMP_GAP + 1.0 {
            put(PropKind::PrecursorRampart, (RAMPART_X, y), PI, 1.0);
            y += 240.0;
        }
        // The sea wall, facing south, but where the Sea Gate's basin opens.
        let long = 240.0 * SEA_WALL_SCALE;
        let mut x = RAMPART_X + long / 2.0;
        while x - long / 2.0 < DESIGN_M + 200.0 {
            if (x - SEA_GATE.0).abs() > BASIN.2 + long / 2.0 + 10.0 {
                put(
                    PropKind::PrecursorRampart,
                    (x, SEA_WALL_Y),
                    -FRAC_PI_2,
                    SEA_WALL_SCALE,
                );
            }
            x += long;
        }
        // The cut's faces, cased, looking into the road; spires at its lips.
        for side in [-1.0, 1.0] {
            let face = ROAD_Y + side * (RAMP_HALF + 4.0);
            let mut x = RAMPART_X + 90.0;
            while x < RAMP_X1 - 60.0 {
                put(PropKind::PrecursorWall, (x, face), -side * FRAC_PI_2, 2.2);
                x += 176.0;
            }
            put(
                PropKind::PrecursorSpire,
                (RAMPART_X + 30.0, ROAD_Y + side * (RAMP_GAP + 20.0)),
                0.25 * PI,
                2.2,
            );
        }
        // Beacons up the causeway, conduits down the processional to the Heart.
        let mut x = RAMP_X0 + 60.0;
        while x < RAMPART_X - 40.0 {
            for side in [-1.0, 1.0] {
                put(
                    PropKind::PrecursorBeacon,
                    (x, ROAD_Y + side * (RAMP_HALF - 18.0)),
                    0.0,
                    1.0,
                );
            }
            x += 150.0;
        }
        put(PropKind::PrecursorGate, GATE, 0.0, 1.0);
        let mut x = GATE.0 + 260.0;
        while x < HEART.0 - 320.0 {
            for side in [-1.0, 1.0] {
                put(
                    PropKind::PrecursorConduit,
                    (x, ROAD_Y + side * 160.0),
                    0.0,
                    2.0,
                );
                if (x - GATE.0) as i64 % 480 < 160 {
                    put(
                        PropKind::PrecursorPylon,
                        (x, ROAD_Y + side * 420.0),
                        FRAC_PI_2,
                        2.4,
                    );
                }
            }
            x += 160.0;
        }
        // The Forges, the Heart and its halos, the Sea Gate.
        for &at in &FORGES {
            put(PropKind::PrecursorForge, at, PI, FORGE_SCALE);
        }
        put(PropKind::PrecursorForge, GREAT_FORGE, PI, GREAT_SCALE);
        // The aeries: a lit pad marked by beacons either side, under the tower.
        for (pad, _) in self.th_aeries() {
            for side in [-1.0, 1.0] {
                put(
                    PropKind::PrecursorBeacon,
                    (pad.0 + side * 75.0, pad.1),
                    0.0,
                    1.3,
                );
            }
            put(PropKind::PrecursorConduit, pad, 0.0, 1.4);
            put(PropKind::PrecursorConduit, pad, FRAC_PI_2, 1.4);
        }
        put(PropKind::PrecursorHeart, HEART, 0.25 * PI, 1.0);
        put(PropKind::PrecursorSeaGate, SEA_GATE, SEA_GATE_HEADING, 1.0);
        for &(_, at, facing, _) in &CRADLES {
            put(PropKind::PrecursorCradle, at, facing.to_radians(), 1.0);
        }
        for &(at, scale, heading) in &HALOS {
            put(PropKind::PrecursorHalo, at, heading, scale);
        }
        for &(at, scale, heading) in &MONOLITHS {
            put(PropKind::PrecursorMonolith, at, heading, scale);
        }
        for &(at, scale) in &NEEDLES {
            put(PropKind::PrecursorNeedle, at, 0.25 * PI, scale);
        }
        for &(at, heading) in &PLATFORMS {
            put(PropKind::PrecursorPlatform, at, heading, 1.0);
        }
        // The viaducts, and a pier under every joint that stands clear of
        // everything: buildings, bays, aeries, cradles, the roads.
        let keep_clear = self.th_keep_clear();
        for &(a, b) in &LINKS {
            let len = dist(a, b);
            let n = (len / 200.0).round().max(1.0);
            let scale = len / (n * 200.0);
            let heading = (b.1 - a.1).atan2(b.0 - a.0);
            let (s, c) = heading.sin_cos();
            for k in 0..n as usize {
                let d = k as f64 * 200.0 * scale;
                let at = (a.0 + c * d, a.1 + s * d);
                put(PropKind::PrecursorViaduct, at, heading, scale);
                if k > 0 && keep_clear.iter().all(|&(p, r)| dist(p, at) > r) {
                    put(PropKind::PrecursorPier, at, heading, scale);
                }
            }
        }
        // The paving: 400 m squares edge to edge wherever the plateau lies level
        // under the whole square, from the top of the road's cut east; a column of
        // wider ones between the Rampart's top and there, either side of the cut.
        let level = |x: f64, y: f64, half: f64| {
            [
                (-1.0, -1.0),
                (1.0, -1.0),
                (-1.0, 1.0),
                (1.0, 1.0),
                (0.0, 0.0),
            ]
            .iter()
            .all(|&(i, j)| {
                let (px, py) = ((x + i * (half - 1.0)).min(DESIGN_M), y + j * (half - 1.0));
                (self.natural_threshold(px * f, py * f) - PLATEAU_H).abs() < 0.05
            })
        };
        let rows = |step: f64| {
            let first = SEA_WALL_Y + COURSE_RUN * 3.0 * SEA_WALL_SCALE + step / 2.0 + 10.0;
            (0..)
                .map(move |k| first + step * k as f64)
                .take_while(|y| *y < 11_000.0)
        };
        for y in rows(400.0) {
            let mut x = RAMP_X1 + 200.0;
            while x < DESIGN_M + 200.0 {
                if level(x, y, 200.0) {
                    put(PropKind::PrecursorFloor, (x, y), 0.0, 1.0);
                }
                x += 400.0;
            }
        }
        let wide = RAMP_X1 - (RAMPART_X + COURSE_RUN * 3.0 + 60.0);
        let x = RAMP_X1 - wide / 2.0;
        for y in rows(wide) {
            if (y - ROAD_Y).abs() > RAMP_HALF + 8.0 + wide / 2.0 && level(x, y, wide / 2.0) {
                put(PropKind::PrecursorFloor, (x, y), 0.0, wide / 400.0);
            }
        }
        out
    }

    /// The aeries: pad and projector head (x, y, height) at each ring tower's outer foot.
    fn th_aeries(&self) -> Vec<Aerie> {
        RING.iter()
            .map(|&(x, y)| {
                let out = if y < ROAD_Y { -1.0 } else { 1.0 };
                (
                    (x, y + out * AERIE_OUT),
                    (x, y + out * AERIE_EMITTER.0, AERIE_EMITTER.1),
                )
            })
            .collect()
    }

    /// Points a viaduct's pier keeps its distance from, and how far (design metres).
    fn th_keep_clear(&self) -> Vec<((f64, f64), f64)> {
        let world = |o: (f64, f64), heading: f64, (lx, ly): (f64, f64)| {
            let (s, c) = heading.sin_cos();
            (o.0 + lx * c - ly * s, o.1 + lx * s + ly * c)
        };
        let mut out = Vec::new();
        // Inside a hall or a building the deck just runs in.
        for &o in &FORGES {
            out.push((world(o, PI, (-140.0 * FORGE_SCALE, 0.0)), 560.0));
        }
        out.push((world(GREAT_FORGE, PI, (-140.0 * GREAT_SCALE, 0.0)), 1_150.0));
        out.push((HEART, 330.0));
        for &r in &RING {
            out.push((r, 140.0));
        }
        for (pad, _) in self.th_aeries() {
            out.push((pad, 110.0));
        }
        for &(_, at, _, _) in &CRADLES {
            out.push((at, 200.0));
        }
        out.push((GATE, 420.0));
        out.push((SEA_GATE, 460.0));
        // The muster and the road down from it.
        for x in [10_300.0, 10_600.0, 10_900.0, 11_200.0] {
            out.push(((x, ROAD_Y), 160.0));
        }
        out
    }

    /// The mountains over the plateau: two bastions cut into the slopes facing
    /// south, their booms reaching out over the facility, towers between them
    /// and at the ends, spans linking them and running off the east edge.
    pub(super) fn machine_threshold(&self) -> Machine<'_> {
        let at = |x: f64, y: f64| self.tv((x, y));
        let f = self.size / DESIGN_M;
        let mut m = Machine::new(self, 60.0 * f);
        let south = -FRAC_PI_2;
        let level = SHELF.2;
        let west = m.bastion(at(11_900.0, 11_200.0), south, 1.6 * f, level);
        m.booms(west, south, 2.2 * f, 1.6 * f);
        let east = m.bastion(at(14_900.0, 11_250.0), south, 1.7 * f, level);
        m.booms(east, south, 2.3 * f, 1.7 * f);
        let pass = m.tower(at(9_800.0, 11_150.0), 0.0, 1.5 * f, level);
        let mid = m.tower(at(13_400.0, 11_300.0), 0.0, 1.8 * f, level);
        let edge = m.tower(at(16_300.0, 11_250.0), 0.0, 1.5 * f, level);
        m.link(west, pass);
        m.link(west, mid);
        m.link(east, mid);
        m.link(east, edge);
        m.span(edge, 0.0, 1.4 * f);
        // On the plateau: four towers round the Heart, spans between them 60 m up,
        // and bastions along the mountains' foot and the sea wall, their booms
        // reaching out over the cradles toward the processional.
        let h = PLATEAU_H;
        let ring: Vec<_> = [
            (12_704.0, 7_604.0),
            (14_104.0, 7_604.0),
            (14_104.0, 9_004.0),
            (12_704.0, 9_004.0),
        ]
        .iter()
        .map(|&(x, y)| m.tower(at(x, y), 0.25 * PI, 1.0 * f, h))
        .collect();
        for i in 0..4 {
            m.link(ring[i], ring[(i + 1) % 4]);
        }
        for &(x, y, heading) in &[
            (11_350.0, 10_250.0, south),
            (13_300.0, 10_350.0, south),
            (11_350.0, 6_600.0, FRAC_PI_2),
            (13_300.0, 6_650.0, FRAC_PI_2),
        ] {
            let b = m.bastion(at(x, y), heading, 0.9 * f, h);
            m.booms(b, heading, 0.9 * f, 0.9 * f);
        }
        m
    }

    /// The sidecar's facility block for the survival rules, in map metres.
    #[cfg(test)]
    fn survival_block(&self) -> String {
        let f = self.size / DESIGN_M;
        let world = |o: (f64, f64), heading: f64, (lx, ly): (f64, f64)| {
            let (s, c) = heading.sin_cos();
            ((o.0 + lx * c - ly * s) * f, (o.1 + lx * s + ly * c) * f)
        };
        let mut out = String::new();
        out += &format!("engine: ({:.0}, {:.0}),\n", HEART.0 * f, HEART.1 * f);
        out += &format!("harbor: ({:.0}, {:.0}),\n", SEA_GATE.0 * f, SEA_GATE.1 * f);
        out += "bays: [\n";
        let hall = |o: (f64, f64), k: f64, fits: f64, out: &mut String| {
            for &by in &FORGE_BAYS {
                let stand = world(o, PI, (FORGE_STAND * k, by * k));
                let head = world(o, PI, (FORGE_EMITTER.0 * k, by * k));
                *out += &format!(
                    "    (at: ({:.0}, {:.0}), facing: 180, emitter: ({:.0}, {:.0}, {:.0}), max_radius: Some({fits:.0})),\n",
                    stand.0, stand.1, head.0, head.1, FORGE_EMITTER.1 * k * f
                );
            }
        };
        for &o in &FORGES {
            hall(o, FORGE_SCALE, FORGE_FITS, &mut out);
        }
        hall(GREAT_FORGE, GREAT_SCALE, GREAT_FITS, &mut out);
        for (pad, head) in self.th_aeries() {
            out += &format!(
                "    (at: ({:.0}, {:.0}), facing: 180, emitter: ({:.0}, {:.0}, {:.0}), domain: Air, max_radius: Some({AERIE_FITS:.0})),\n",
                pad.0 * f, pad.1 * f, head.0 * f, head.1 * f, head.2 * f
            );
        }
        for &sy in &SLIPS {
            let stand = world(SEA_GATE, SEA_GATE_HEADING, (SLIP_X, sy));
            let head = world(SEA_GATE, SEA_GATE_HEADING, (SLIP_EMITTER.0, sy));
            let fits = if sy == 0.0 {
                GREAT_SLIP_FITS
            } else {
                SLIP_FITS
            };
            out += &format!(
                "    (at: ({:.0}, {:.0}), facing: 270, emitter: ({:.0}, {:.0}, {:.0}), domain: Naval, max_radius: Some({fits:.0})),\n",
                stand.0, stand.1, head.0, head.1, SLIP_EMITTER.1
            );
        }
        out += "],\nnode_sites: [\n";
        for &(name, at, facing, sea) in &CRADLES {
            out += &format!(
                "    (name: \"{name}\", at: ({:.0}, {:.0}), domain: {}, facing: Some({facing:.0})),\n",
                at.0 * f,
                at.1 * f,
                if sea { "Naval" } else { "Land" }
            );
        }
        out += "],\n";
        out
    }
}

#[cfg(test)]
mod tests {

    /// Heights of the mountains over the plateau, before the machine's benches.
    #[test]
    #[ignore]
    fn threshold_profile() {
        let t = &*crate::bake::test_maps::THRESHOLD;
        for y in (10_600..=13_000).step_by(200) {
            let row: Vec<String> = (9_000..=16_400)
                .step_by(400)
                .map(|x| format!("{:4.0}", t.natural_threshold(x as f64, y as f64)))
                .collect();
            println!("{y:6} {}", row.join(" "));
        }
    }

    /// Prints the sidecar's facility block: `cargo test -p mc-map --lib threshold_block -- --nocapture`.
    #[test]
    fn threshold_block() {
        let t = &*crate::bake::test_maps::THRESHOLD;
        println!("{}", t.survival_block());
    }
}
