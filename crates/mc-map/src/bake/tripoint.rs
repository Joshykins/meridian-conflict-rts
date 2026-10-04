//! "Tripoint": three players, each for themselves, in three climates, the
//! Precursors' climate walls running between them to the installation they
//! meet at in the middle.
//!
//! [`Layout::Tripoint`], 12 km. The map is three thirds of a turn about its
//! middle: Alaska to the north, the desert to the south-west, the jungle to
//! the south-east, each with one base. The walls ([`TRIPOINT_WALLS`]) run out
//! from the middle north-east, north-west and south, between the thirds.
//!
//! Each base stands on its third's upland, ringed behind by mountains. Down
//! each wall runs a vale both of its neighbours look down into: the wall's
//! line is the vale's floor, so the two meet there, the climates changing
//! underfoot. Each side has two ramps down into each of its vales, and a
//! ridge stands on the wall halfway along the vale, so a vale is crossed in
//! two places: near the middle, under the installation's booms, and out by
//! the lake at its far end, which the wall runs through. In the middle the
//! installation stands on a plateau with a ramp up from each upland: the
//! Axis, a needle 1.4 km tall, and three bastions round it on the walls'
//! lines, their booms reaching out over the vales and spans running back
//! into the needle. Out along the walls the line is kept by towers built
//! into the ground; units pass between them.
//!
//! Fairness is in the play, not the look, as on Frostline: what decides
//! where a unit can go is designed once, for the north third, and laid on
//! the other two turned ([`turn`]): the base and its ore, the uplands, the
//! vales and their ramps, the plateau, the ridges' feet and the first 70 m of
//! every mountain wall, the lakes, the outline of the land the mountains
//! ring. A slow warp that is not the same turned bends those lines out of
//! true away from the bases, and everything else is each climate's own: the
//! small relief (drumlins and kettles in Alaska; dunes, slickrock and spires
//! in the desert; rounded hills and limestone knolls in the jungle), and what
//! stands above the mountains' shared wall: peaks, arêtes and ice; mesas cut
//! in Vermilion Gorge's beds; steep green peaks. `tests/tripoint.rs` holds
//! the baked map to the same walks, ground and timber for all three.
//!
//! "Design" points are metres from the map's middle, in the north third's
//! frame; map points are metres from the map's corner, y north.

use super::bays::{dist, inside, segment};
use super::canyon::smooth_closed;
use super::machine::Machine;
use super::{OreField, Pad, Terrain};
use crate::format::PropKind;
use crate::landmark::{
    tripoint_region, TRIPOINT_ALASKA as ALASKA, TRIPOINT_DESERT as DESERT,
    TRIPOINT_JUNGLE as JUNGLE,
};
use crate::noise::{smoothstep, unit};
use crate::BUILD_CELL_M;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_3, PI, TAU};

mod shape;
#[cfg(test)]
mod tests;

/// The map's edge, metres: the design is for exactly this.
pub(super) const SIZE: f64 = 12_288.0;
const MID: f64 = SIZE / 2.0;
/// The sine of a third of a turn.
const SIN_THIRD: f64 = 0.866_025_403_784_438_6;

/// The benches, metres over the water: the vales, the uplands, the plateau
/// the installation stands on.
const LOW: f64 = 20.0;
const UPLAND: f64 = 60.0;
const HIGH: f64 = 104.0;

/// The land the mountains ring, the north third's share: (degrees
/// counter-clockwise from east, metres from the middle), from the north-east
/// wall round toward the north-west one, which it meets at the first
/// point's distance (the next third's first point).
const ARENA: &[(f64, f64)] = &[
    (30.0, 5_000.0),
    (36.0, 5_300.0),
    (42.0, 4_850.0),
    (50.0, 5_450.0),
    (57.0, 5_900.0),
    (63.0, 5_300.0),
    // A spur reaching in on the base's right flank.
    (68.0, 4_450.0),
    (73.0, 5_250.0),
    (82.0, 5_700.0),
    (90.0, 5_800.0),
    (98.0, 5_650.0),
    (106.0, 5_150.0),
    // And one on its left.
    (111.0, 4_400.0),
    (116.0, 5_100.0),
    (124.0, 5_800.0),
    (132.0, 5_500.0),
    (139.0, 4_900.0),
    (145.0, 5_150.0),
];

/// The plateau in the middle, the north third's share, as [`ARENA`]: low
/// where the walls leave it, so the bastions stand out over the vales.
const PLATEAU: &[(f64, f64)] = &[
    (30.0, 860.0),
    (45.0, 1_000.0),
    (60.0, 1_120.0),
    (75.0, 1_040.0),
    (90.0, 1_160.0),
    (105.0, 1_080.0),
    (120.0, 1_190.0),
    (135.0, 990.0),
];

/// The vale down the north-east wall, as (metres out along the wall, metres
/// to its left): the north third's side, then the jungle's coming back. The
/// other two vales are this turned, so every base has this vale's left side
/// on its right hand and its right side on its left.
const VALE: &[(f64, f64)] = &[
    (600.0, 300.0),
    (1_500.0, 650.0),
    (2_200.0, 820.0),
    (2_900.0, 700.0),
    (3_600.0, 950.0),
    (4_300.0, 880.0),
    (5_000.0, 720.0),
    (5_600.0, 700.0),
    (5_600.0, -680.0),
    (5_000.0, -660.0),
    (4_500.0, -900.0),
    (3_800.0, -760.0),
    (3_100.0, -880.0),
    (2_400.0, -620.0),
    (1_600.0, -720.0),
    (600.0, -320.0),
];

/// A line from one point to another and everything within a distance of it.
type Capsule = ((f64, f64), (f64, f64), f64);

/// Ramps: where an escarpment is laid back into a slope, as the line it is
/// laid back along and how far either side of it. Along the vale (as
/// [`VALE`]): two down its left side, two down its right. Design points: the
/// way up onto the plateau.
const VALE_RAMPS: &[Capsule] = &[
    ((1_500.0, 635.0), (1_660.0, 685.0), 90.0),
    ((3_580.0, 940.0), (3_740.0, 945.0), 90.0),
    ((1_950.0, -655.0), (2_110.0, -642.0), 90.0),
    ((3_960.0, -805.0), (4_110.0, -855.0), 90.0),
];
const PLATEAU_RAMP: Capsule = ((-100.0, 1_150.0), (100.0, 1_150.0), 110.0);

/// The lake at the vale's far end, the wall through it: (along, left, radius).
const LAKE: (f64, f64, f64) = (4_250.0, 60.0, 330.0);
/// Its floor: open water, too shallow for a ship.
const LAKE_FLOOR: f64 = -4.2;

/// The base, design.
const START: (f64, f64) = (0.0, 3_850.0);

struct Ridge {
    /// Crest line, design.
    line: &'static [(f64, f64)],
    /// Half width of the mountain's foot.
    half: f64,
    /// Height of a crest over the land round it (a desert mesa stands as high
    /// as its width lets the beds stack).
    tall: f64,
}

/// The mountains inside the ring: laid in every third as listed, turned.
const RIDGES: &[Ridge] = &[
    // On the north-east wall halfway along the vale (2350 to 3150 m out):
    // half one climate, half the other.
    Ridge {
        line: &[(2_035.0, 1_175.0), (2_728.0, 1_575.0)],
        half: 230.0,
        tall: 380.0,
    },
    // A butte across the way from the base to the plateau; the ways run
    // either side of it.
    Ridge {
        line: &[(-450.0, 2_480.0), (380.0, 2_330.0)],
        half: 210.0,
        tall: 300.0,
    },
    // Knolls standing alone on the upland.
    Ridge {
        line: &[(880.0, 3_260.0), (960.0, 3_330.0)],
        half: 130.0,
        tall: 170.0,
    },
    Ridge {
        line: &[(-1_300.0, 4_250.0), (-1_220.0, 4_330.0)],
        half: 140.0,
        tall: 190.0,
    },
    Ridge {
        line: &[(1_500.0, 4_650.0), (1_580.0, 4_700.0)],
        half: 150.0,
        tall: 200.0,
    },
    Ridge {
        line: &[(-720.0, 1_900.0), (-650.0, 1_960.0)],
        half: 120.0,
        tall: 150.0,
    },
];

/// Ore away from the base, design: centre, radius. Each third has these
/// turned; the two on the wall's line are shared by its neighbours.
const ORE: &[(f64, f64, f64)] = &[
    // On the plateau, by the ramp's head.
    (0.0, 640.0, 75.0),
    // At the ramp's foot.
    (0.0, 1_760.0, 70.0),
    // The upland's flanks either side of the base.
    (-1_150.0, 3_560.0, 70.0),
    (1_250.0, 3_700.0, 70.0),
    // Behind the base, under the mountains.
    (-820.0, 5_000.0, 65.0),
    (900.0, 5_060.0, 65.0),
    // On the wall in the vale: the inner crossing (1650 m out), and the
    // outer one past the ridge (3600 m out).
    (1_428.9, 825.0, 75.0),
    (3_117.7, 1_800.0, 75.0),
    // Each side's own, down in the vale: the left (2600 m out, 520 m left)
    // and the right (3000 m out, 560 m right).
    (1_991.7, 1_750.3, 65.0),
    (2_878.1, 1_015.0, 65.0),
];

/// The installation: the Axis in the middle at this scale, the bastions this
/// far out along the walls at this scale, their booms at this one.
const AXIS_SCALE: f64 = 0.3;
const BASTION_OUT: f64 = 640.0;
const BASTION_SCALE: f64 = 0.5;
const BOOM_SCALE: f64 = 0.75;
/// The towers along each wall, metres out from the middle: in the vale, on
/// the ridge, in the lake, then in the mountains to the map's edge.
const TOWERS: &[f64] = &[2_000.0, 2_750.0, 4_250.0, 5_350.0, 6_350.0, 7_350.0];
const TOWER_SCALE: f64 = 0.85;

/// A design vector turned `k` thirds of a turn counter-clockwise.
pub(super) fn turn((x, y): (f64, f64), k: usize) -> (f64, f64) {
    match k % 3 {
        0 => (x, y),
        1 => (-0.5 * x - SIN_THIRD * y, SIN_THIRD * x - 0.5 * y),
        _ => (-0.5 * x + SIN_THIRD * y, -SIN_THIRD * x - 0.5 * y),
    }
}

/// Which third a design vector lies in: 0 the north (Alaska), 1 the
/// south-west (the desert), 2 the south-east (the jungle).
pub(super) fn third((x, y): (f64, f64)) -> usize {
    let a = (y.atan2(x) - FRAC_PI_3 / 2.0).rem_euclid(TAU);
    ((a / (TAU / 3.0)) as usize).min(2)
}

/// A design point on the map, and a map point in the design.
fn on_map((x, y): (f64, f64)) -> (f64, f64) {
    (MID + x, MID + y)
}
fn design((x, y): (f64, f64)) -> (f64, f64) {
    (x - MID, y - MID)
}

/// A point of the vale's frame ([`VALE`]) in the design.
fn vale_point((along, left): (f64, f64)) -> (f64, f64) {
    (
        along * SIN_THIRD - left * 0.5,
        along * 0.5 + left * SIN_THIRD,
    )
}

/// A closed outline round the middle from the north third's share, laid in
/// all three thirds.
fn round_the_middle(share: &[(f64, f64)]) -> Vec<(f64, f64)> {
    (0..3)
        .flat_map(|k| {
            share.iter().map(move |&(deg, r)| {
                let (s, c) = deg.to_radians().sin_cos();
                turn((r * c, r * s), k)
            })
        })
        .collect()
}

/// What the layout works out once, at set-up.
#[derive(Default)]
pub(super) struct Tripoint {
    /// The land the mountains ring and the plateau, round the middle,
    /// rounded; the north-east vale, rounded; its ramps and the plateau's,
    /// design.
    arena: Vec<(f64, f64)>,
    plateau: Vec<(f64, f64)>,
    vale: Vec<(f64, f64)>,
    ramps: Vec<Capsule>,
    /// What water did to the open country (`erode_tripoint`).
    gullies: super::alpine::Erosion,
    /// Pools cut off in the mountains, filled.
    pools: super::alpine::Erosion,
}

impl Terrain {
    /// Glacier ice and lying snow, in Alaska only: snow on the upland and
    /// above and in drifts across the vales, ice on the high shoulders and
    /// in the frozen kettles and streams.
    pub(super) fn tripoint_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        // Up to the wall itself: the renderer cuts the climates on the line.
        let alaska = smoothstep(-48.0, -16.0, shape::depths(design((x, y)))[ALASKA]);
        if alaska <= 0.0 || h < 1.0 {
            return (0.0, 0.0);
        }
        let slope = (gx * gx + gy * gy).sqrt();
        // Slopes that face north keep their snow lower.
        let north = (-gy / slope.max(1e-3)) * smoothstep(0.05, 0.3, slope);
        let line =
            UPLAND + 18.0 + 12.0 * self.mtn_mask.get(x / 1_400.0, y / 1_400.0) - 20.0 * north;
        let high = smoothstep(line - 24.0, line + 36.0, h);
        let drift = self.tilt.fbm(x / 620.0 + 77.0, y / 620.0 - 31.0, 3, 0.5)
            + 0.4 * self.detail.fbm(x / 160.0 - 12.0, y / 160.0 + 40.0, 2, 0.5);
        let reach = 0.1 - 0.2 * smoothstep(LOW + 4.0, UPLAND - 4.0, h);
        let low = 0.7 * smoothstep(reach, reach + 0.2, drift) * smoothstep(3.0, 8.0, h);
        let snow = high.max(low) * (1.0 - smoothstep(0.75, 1.5, slope));
        // An ice cap on the highest gentle ground; not on the faces.
        let cap = smoothstep(
            300.0,
            400.0,
            h + 70.0 * self.mtn_mask.fbm(x / 700.0 + 5.0, y / 700.0, 2, 0.5),
        ) * (1.0 - smoothstep(0.25, 0.5, slope));
        let floor = self.tp_frozen(x, y) * (1.0 - smoothstep(0.12, 0.3, slope));
        (alaska * cap.max(floor), alaska * snow * (1.0 - floor))
    }

    /// How thickly trees grow: each region's own woods, thick jungle, open
    /// pinyon and juniper, spruce in stands. (Trees carry no mass and slow no
    /// one, so the regions need not have the same.) None on the plateau.
    pub(super) fn tripoint_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        let region = tripoint_region((x, y));
        let broad = self.forest.fbm(x / l, y / l, 3, 0.5);
        let copse = self
            .forest
            .fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5);
        let clearing = self
            .forest
            .fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5);
        let clearing = smoothstep(0.30, 0.55, clearing);
        let mut habitable = smoothstep(1.5, 4.0, height);
        let density = match region {
            ALASKA => {
                // Spruce in the vales and over the upland, thinning out up
                // the mountains; none on the ice.
                habitable *= (1.0 - smoothstep(0.30, 0.5, slope))
                    * (1.0 - smoothstep(UPLAND + 30.0, UPLAND + 70.0, height));
                if habitable > 0.0 {
                    habitable *= 1.0 - self.tp_frozen(x, y);
                }
                let forest = smoothstep(self.forest_edge - 0.06, self.forest_edge + 0.16, broad);
                (forest * (1.0 - 0.85 * clearing)).max(smoothstep(0.42, 0.62, copse) * 0.75)
            }
            DESERT => {
                // Pinyon and juniper in open stands on the upland, thinner
                // down in the vales.
                habitable *=
                    (1.0 - smoothstep(0.30, 0.5, slope)) * (1.0 - smoothstep(150.0, 185.0, height));
                let forest = smoothstep(self.forest_edge + 0.05, self.forest_edge + 0.27, broad);
                (forest * (1.0 - 0.85 * clearing)).max(smoothstep(0.5, 0.7, copse) * 0.7)
                    * (0.45 + 0.55 * smoothstep(LOW + 6.0, UPLAND - 6.0, height))
            }
            _ => {
                // Jungle over nearly everything, broken by clearings, and up
                // the mountains' sides to the crests, thinner there.
                habitable *= (1.0 - smoothstep(0.7, 0.95, slope))
                    * (1.0 - smoothstep(480.0, 560.0, height))
                    * (1.0 - 0.6 * smoothstep(UPLAND + 50.0, UPLAND + 160.0, height));
                let forest = smoothstep(self.forest_edge - 0.4, self.forest_edge - 0.12, broad);
                // (Half the trees a wood could hold: every one is drawn, and
                // half is canopy enough.)
                0.5 * (forest * (1.0 - 0.6 * clearing)).max(smoothstep(0.3, 0.5, copse) * 0.85)
            }
        };
        // The installation's plateau is bare.
        if habitable > 0.0 && (x - MID).hypot(y - MID) < 1_500.0 {
            habitable *= smoothstep(-40.0, 30.0, -inside(design((x, y)), &self.tp.plateau));
        }
        // A glade round every ore field.
        for f in &self.ore {
            let d = dist((x, y), (f.x, f.y)) - f.radius;
            if d < 200.0 {
                habitable *= smoothstep(70.0, 200.0, d);
            }
        }
        (density * habitable, height)
    }

    /// Each climate's trees (`forest_density` hands the height over in the
    /// conifer slot): spruce and pine with birch low down in Alaska, juniper
    /// and pinyon in the desert, jungle in the jungle with palms by the water.
    pub(super) fn tripoint_tree(&self, x: f64, y: f64, height: f64, hash: u64) -> PropKind {
        let (dead, pick) = (unit(hash, 40), unit(hash, 48));
        match tripoint_region((x, y)) {
            ALASKA => {
                let pines = self.forest_kind.get(x / 420.0 + 11.0, y / 420.0 - 5.0) > 0.1;
                match () {
                    _ if dead < 0.03 => PropKind::TreeDead,
                    _ if pick < 0.25 * (1.0 - smoothstep(LOW + 2.0, LOW + 12.0, height)) => {
                        PropKind::TreeBroadleaf
                    }
                    _ if pines => PropKind::TreePine,
                    _ => PropKind::TreeConifer,
                }
            }
            DESERT => match () {
                _ if dead < 0.05 => PropKind::TreeDead,
                _ if height < LOW - 2.0 => PropKind::TreeCottonwood,
                _ if pick < smoothstep(40.0, 100.0, height) * 0.6 => PropKind::TreePinyon,
                _ => PropKind::TreeJuniper,
            },
            _ => {
                let palms = 0.08 + 0.8 * (1.0 - smoothstep(LOW - 4.0, LOW + 4.0, height));
                match () {
                    _ if dead < 0.006 => PropKind::TreeDead,
                    _ if pick < palms => PropKind::TreePalm,
                    _ if pick > 0.86 => PropKind::TreeBroadleaf,
                    _ => PropKind::TreeJungle,
                }
            }
        }
    }

    /// The installation: the Axis on the plateau in the middle; on each
    /// wall's line a bastion facing out along it, its booms reaching out over
    /// the vale and a span back into the needle; out along the wall, towers
    /// built into the ground (no bench: their feet go down into it), all the
    /// way to the map's edge. Map metres.
    pub(super) fn machine_tripoint(&self) -> Machine<'_> {
        let mut m = Machine::new(self, 44.0);
        let axis = m.axis((MID, MID), 0.0, AXIS_SCALE, HIGH);
        for k in 0..3 {
            let heading = FRAC_PI_3 / 2.0 + k as f64 * TAU / 3.0;
            let (s, c) = heading.sin_cos();
            let out = |r: f64| (MID + r * c, MID + r * s);
            let bastion = m.bastion(out(BASTION_OUT), heading, BASTION_SCALE, HIGH);
            m.booms(bastion, heading, BOOM_SCALE, BASTION_SCALE);
            m.link(bastion, axis);
            for &r in TOWERS {
                let at = out(r);
                if at.0 > 0.0 && at.1 > 0.0 && at.0 < self.size_x && at.1 < self.size_y {
                    m.put(PropKind::PrecursorTower, at, heading, TOWER_SCALE);
                }
            }
        }
        m
    }

    pub(super) fn setup_tripoint(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        self.forest_edge = 0.0;
        let mut ramps: Vec<Capsule> = VALE_RAMPS
            .iter()
            .map(|&(a, b, r)| (vale_point(a), vale_point(b), r))
            .collect();
        ramps.push(PLATEAU_RAMP);
        let vale: Vec<(f64, f64)> = VALE.iter().map(|&p| vale_point(p)).collect();
        self.tp = Tripoint {
            arena: smooth_closed(&round_the_middle(ARENA), 4),
            plateau: smooth_closed(&round_the_middle(PLATEAU), 5),
            vale: smooth_closed(&vale, 4),
            ramps,
            gullies: super::alpine::Erosion::default(),
            pools: super::alpine::Erosion::default(),
        };
        self.erode_tripoint();

        // The north third's base, and the same turned; all three at its height.
        let first = self.snap(on_map(START));
        let height = self.natural(first.0, first.1).max(12.0);
        self.starts = (0..3).map(|k| on_map(turn(design(first), k))).collect();
        self.pads = self
            .starts
            .iter()
            .map(|&(x, y)| Pad {
                x,
                y,
                core: start_core,
                outer: 2.0 * start_core,
                height,
            })
            .collect();

        // Three small fields round the base, inside its pad: one behind (away
        // from the middle), two on the forward flanks. Then the rest. Each
        // laid in the north third and turned: outline and all.
        let back = FRAC_PI_2;
        let mut sites: Vec<((f64, f64), f64)> = [0.0, PI - 1.15, PI + 1.15]
            .iter()
            .map(|turn_by| {
                let (s, c) = (back + turn_by).sin_cos();
                let d = 0.8 * start_core;
                (
                    (first.0 + c * d, first.1 + s * d),
                    (0.2 * start_core).max(55.0),
                )
            })
            .collect();
        sites.extend(ORE.iter().map(|&(x, y, r)| (on_map((x, y)), r)));
        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = snap(p);
            let f = self.ore_field(p.0, p.1, r);
            let turned: Vec<OreField> = (1..3)
                .map(|k| {
                    let moved = |q: (f64, f64)| on_map(turn(design(q), k));
                    let centre = moved((f.x, f.y));
                    OreField {
                        x: centre.0,
                        y: centre.1,
                        radius: f.radius,
                        corners: f.corners.iter().map(|&q| moved(q)).collect(),
                    }
                })
                .collect();
            fields.push(f);
            fields.extend(turned);
        }
        self.ore = fields;
        self.lay_machine();
        self.fit_forests();
    }
}
