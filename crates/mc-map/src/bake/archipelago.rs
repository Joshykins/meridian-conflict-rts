//! "The Axis": four against four on a tropical archipelago, round a jungle
//! island whose middle opens on the Meridian.
//!
//! [`Layout::Archipelago`], 20 km. Every player starts on an island of their
//! own; each side's four lie in an arc round its half of the map, the forward
//! pair facing the other side's across the north and south straits. Between
//! them lie cays worth holding: ore behind the lines, gun rocks (flat-topped
//! limestone stacks for artillery and coastal guns) off the big island's ends,
//! and an islet in each strait that both sides can reach. Shallow sand banks
//! (too shoal for ships, turquoise over white sand) fringe the islands; the
//! channels between them are deep blue water.
//!
//! The big island in the middle is jungle and nothing else: no ore. Down its
//! length runs the Precursor machine: a bastion cut into the north end, its booms
//! reaching south over a level avenue toward the Axis, a needle standing up out of
//! the jungle into the clouds, and a span from the Axis to a tower at the south end.
//! Everything that stands on the ground stands on the island's long axis, which the
//! half turn lays on itself, so neither side is nearer the machine.
//!
//! Fairness without a mirror look: everything that matters to play (where the
//! islands and starts are, how big the islands are, the ore, the banks' reach
//! into the channels) is laid for one side and turned half round the centre
//! for the other. The islands' outlines, coves, hills, reefs and woods are not
//! turned: each island's lobes take their own phases and the coast noise is
//! sampled where it stands, so the twins are the same size and shape of play
//! but no two coasts look alike.
//!
//! Coordinates are design metres from the map centre, x east, y north, for a
//! 20 km map; a smaller bake scales them.

use super::machine::{in_solid, Machine};
use super::{OreField, Pad, Terrain};
use crate::format::PropKind;
use crate::noise::{hash2, smoothstep, unit};
use crate::BUILD_CELL_M;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// Edge of the map the layout is drawn for, in metres.
const DESIGN_M: f64 = 20_480.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// A player's island: start, harbour, ore.
    Home,
    /// The jungle island in the middle, with the pit.
    Centre,
    /// A low sandy cay.
    Cay,
    /// A limestone stack with a flat top and cliffs all round.
    Gun,
}

/// One island as designed: centre, mean radius (design m), kind.
struct Design {
    at: (f64, f64),
    r: f64,
    kind: Kind,
    /// Home islands: the bearing the harbour opens toward (deep water close in).
    harbour: f64,
}

const fn isle(x: f64, y: f64, r: f64, kind: Kind) -> Design {
    Design {
        at: (x, y),
        r,
        kind,
        harbour: 0.0,
    }
}

/// What stands on a cay: the west side's cays by their index in `WEST`.
#[derive(Clone, Copy)]
enum Outpost {
    /// A beacon and pylons on a gun rock's lip, clear of its top.
    Lamp,
    /// A tower of the scale given.
    Tower(f64),
    /// A needle into the clouds.
    Needle(f64),
    /// A small bastion, its booms reaching toward the big island.
    Bastion(f64),
    /// The Axis, on the strait islets both sides reach.
    Axis(f64),
}

const OUTPOSTS: &[(usize, Outpost)] = &[
    (4, Outpost::Tower(0.3)),
    (5, Outpost::Needle(0.22)),
    (6, Outpost::Needle(0.22)),
    (7, Outpost::Tower(0.34)),
    (8, Outpost::Tower(0.34)),
    (9, Outpost::Bastion(0.3)),
    (10, Outpost::Lamp),
    (11, Outpost::Lamp),
    (12, Outpost::Axis(0.13)),
];

/// A node of the network: the big island's machine or a cay's outpost.
#[derive(Clone, Copy)]
enum At {
    Bastion,
    Citadel,
    Tower,
    /// A west cay by its index in `WEST`.
    Cay(usize),
    /// A west cay turned half round: the east side's twin.
    Twin(usize),
}

/// The seaways, laid for the west and turned for the east.
const SEAWAYS: &[(At, At)] = &[
    (At::Citadel, At::Cay(9)),
    (At::Cay(9), At::Cay(4)),
    (At::Bastion, At::Cay(10)),
    (At::Tower, At::Cay(11)),
    (At::Cay(10), At::Cay(7)),
    (At::Cay(11), At::Cay(8)),
    (At::Cay(7), At::Cay(5)),
    (At::Cay(8), At::Cay(6)),
    (At::Cay(10), At::Cay(12)),
    (At::Cay(11), At::Twin(12)),
];

const fn home(x: f64, y: f64, harbour: f64) -> Design {
    Design {
        at: (x, y),
        r: HOME_R,
        kind: Kind::Home,
        harbour,
    }
}

const HOME_R: f64 = 1_250.0;

/// The west side's islands; the east side's are these turned half round the centre.
/// The first four are the homes, in start order.
const WEST: &[Design] = &[
    // Homes: two back, facing the big island across the middle; two forward, on the straits.
    home(-6_900.0, 3_300.0, -0.55),
    home(-6_900.0, -3_300.0, 0.55),
    home(-3_400.0, 7_600.0, -2.25),
    home(-3_400.0, -7_600.0, 2.25),
    // Behind the lines: an islet between the back homes, the two corner islands.
    isle(-8_700.0, 0.0, 460.0, Kind::Cay),
    isle(-8_350.0, 8_300.0, 600.0, Kind::Cay),
    isle(-8_350.0, -8_300.0, 600.0, Kind::Cay),
    // Forward: between each back home and its forward neighbour, and off the big island's flank.
    isle(-5_150.0, 5_450.0, 400.0, Kind::Cay),
    isle(-5_150.0, -5_450.0, 400.0, Kind::Cay),
    isle(-4_350.0, 0.0, 400.0, Kind::Cay),
    // The gun rocks off the big island's north and south ends.
    isle(-1_750.0, 4_750.0, 300.0, Kind::Gun),
    isle(-1_750.0, -4_750.0, 300.0, Kind::Gun),
    // The north strait's islet; the south strait's is its twin.
    isle(0.0, 8_350.0, 540.0, Kind::Cay),
];

/// The big island: mean radius, stretch along its long axis (north-south).
const CENTRE_R: f64 = 2_450.0;
const CENTRE_STRETCH: f64 = 1.28;

/// Ore: west side's centre (design m) and radius (m); each also turned.
/// Home fields are laid round each start separately.
const ORE: &[((f64, f64), f64)] = &[
    // The islet between the back homes.
    ((-8_780.0, 160.0), 78.0),
    ((-8_620.0, -170.0), 70.0),
    // The corner islands.
    ((-8_480.0, 8_440.0), 82.0),
    ((-8_200.0, 8_150.0), 74.0),
    ((-8_480.0, -8_440.0), 82.0),
    ((-8_200.0, -8_150.0), 74.0),
    // The forward cays.
    ((-5_150.0, 5_450.0), 84.0),
    ((-5_150.0, -5_450.0), 84.0),
    ((-4_350.0, 0.0), 76.0),
    // The north strait's islet.
    ((-170.0, 8_400.0), 78.0),
    ((190.0, 8_300.0), 78.0),
];

/// Shoals out in open water: centre (design m) and reach; turned for the east.
const BANKS: &[((f64, f64), f64)] = &[
    ((-9_450.0, -5_400.0), 950.0),
    ((-5_600.0, 9_550.0), 800.0),
    ((-9_500.0, 5_000.0), 700.0),
];

/// How far the plane's warp moves a coast (design m).
const WARP: f64 = 650.0;

/// Home pads: level core and blend radius (m).
const PAD_CORE: f64 = 340.0;
const PAD_OUTER: f64 = 620.0;
const PAD_HEIGHT: f64 = 16.0;
/// Home islands keep this much land round the start whatever their lobes do.
const SAFE: f64 = 860.0;
/// The harbour: how far out the coast lies on the harbour bearing, and the
/// half width of the harbour's sector (radians).
const HARBOUR_COAST: f64 = 980.0;
const HARBOUR_HALF: f64 = 0.42;

// The machine down the big island (design m along its long axis from the centre).
/// The long axis's bearing, north a little west.
const AXIS_HEADING: f64 = FRAC_PI_2 + 0.18;
/// Every bench of the machine and its avenue is cut to this level.
const MACHINE_LEVEL: f64 = 26.0;
/// Where the bastion, the citadel and the tower stand along the long axis.
const BASTION_AT: f64 = 1_300.0;
const CITADEL_AT: f64 = -380.0;
const CITADEL_SCALE: f64 = 1.25;
const TOWER_AT: f64 = -1_500.0;
/// The seaways: water (m) a full-size causeway wants over it, and its length and
/// height at scale 1 (`mc-render` precursor_citadel.rs).
const SEAWAY_DEPTH: f64 = 16.0;
const SEAWAY_LEN: f64 = 196.0;
const SEAWAY_TOP: f64 = 14.0;
/// The start clearings: how far the jungle's edge stands from a start, as a share
/// of the pad's core, and how deep the wood's thinning edge is (m).
const CLEARING: (f64, f64) = (0.62, 0.55);
const CLEARING_EDGE: f64 = 170.0;

/// Distance from `p` to `q`.
fn dist(p: (f64, f64), q: (f64, f64)) -> f64 {
    (p.0 - q.0).hypot(p.1 - q.1)
}

fn turn(q: (f64, f64)) -> (f64, f64) {
    (-q.0, -q.1)
}

/// An island as laid: in design metres, with its own outline.
pub(super) struct Isle {
    at: (f64, f64),
    r: f64,
    kind: Kind,
    harbour: Option<f64>,
    /// Lobes: (order, amplitude, phase). Twins share amplitudes, not phases.
    lobes: [(f64, f64, f64); 4],
    /// Long axis and stretch (1: round).
    axis: f64,
    stretch: f64,
    /// Each island's own offset into the noise, so its hills are its own.
    noise_at: (f64, f64),
}

impl Isle {
    /// How far out the coast lies on bearing `a`, before the coast noise.
    fn reach(&self, a: f64) -> f64 {
        let lobes: f64 = self
            .lobes
            .iter()
            .map(|&(n, amp, ph)| amp * (n * a + ph).sin())
            .sum();
        // An ellipse of the same area as the circle, stretched along its axis.
        let (ea, eb) = (self.r * self.stretch.sqrt(), self.r / self.stretch.sqrt());
        let (s, c) = (a - self.axis).sin_cos();
        let e = 1.0 / ((c / ea).powi(2) + (s / eb).powi(2)).sqrt();
        let mut r = e * (1.0 + lobes);
        if let Some(h) = self.harbour {
            let off = (a - h + PI).rem_euclid(TAU) - PI;
            let k = 1.0 - smoothstep(0.6 * HARBOUR_HALF, HARBOUR_HALF, off.abs());
            r += (HARBOUR_COAST - r) * k;
            r = r.max(SAFE);
        }
        r
    }

    /// Metres in from this island's coast, before the coast noise (negative: out at sea).
    fn inside(&self, q: (f64, f64)) -> f64 {
        let d = dist(q, self.at);
        if d > 2.2 * self.r + 3_000.0 {
            return self.r * 1.4 - d;
        }
        let a = (q.1 - self.at.1).atan2(q.0 - self.at.0);
        self.reach(a) - d
    }

    /// Within the harbour's sector (0..1), for a design point.
    fn harbour_at(&self, q: (f64, f64)) -> f64 {
        match self.harbour {
            Some(h) => {
                let a = (q.1 - self.at.1).atan2(q.0 - self.at.0);
                let off = (a - h + PI).rem_euclid(TAU) - PI;
                1.0 - smoothstep(HARBOUR_HALF, 1.6 * HARBOUR_HALF, off.abs())
            }
            None => 0.0,
        }
    }
}

/// The layout as laid for one bake: the islands and the land kept round starts and ore.
#[derive(Default)]
pub(super) struct Archipelago {
    isles: Vec<Isle>,
    /// Design point and radius of guaranteed land (starts, ore).
    anchors: Vec<((f64, f64), f64)>,
}

/// What the land field knows about a point: the nearest island and how far in
/// from its coast, and how far out from the next.
struct Near<'a> {
    isle: &'a Isle,
    s: f64,
    /// The next island's `s` (negative: its coast is this far away).
    next: f64,
}

impl Terrain {
    /// Design scale: metres per design metre.
    fn af(&self) -> f64 {
        self.size / DESIGN_M
    }

    /// Design point to world.
    fn aw(&self, q: (f64, f64)) -> (f64, f64) {
        let f = self.af();
        (self.size_x / 2.0 + q.0 * f, self.size_y / 2.0 + q.1 * f)
    }

    /// World point to design.
    fn ad(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let f = self.af();
        ((x - self.size_x / 2.0) / f, (y - self.size_y / 2.0) / f)
    }

    /// The islands, both sides' and the big one, with their outlines.
    pub(super) fn lay_isles(seed: u64) -> Vec<Isle> {
        let mut out = Vec::new();
        let mut index = 0i64;
        let mut push = |d: &Design, at: (f64, f64), harbour: Option<f64>, out: &mut Vec<Isle>| {
            index += 1;
            let h = |n: i64| hash2(seed ^ 0x6172_6368, index, n);
            // Amplitudes by the island's slot, so both twins get the same; phases
            // by the island itself.
            let slot = |n: i64| {
                hash2(
                    seed ^ 0x6C6F_6265,
                    (d.at.0 as i64).abs() + (d.at.1 as i64).abs(),
                    n,
                )
            };
            let wild = match d.kind {
                Kind::Home => 1.2,
                Kind::Centre => 1.1,
                Kind::Cay => 1.5,
                Kind::Gun => 0.3,
            };
            let lobes: [(f64, f64, f64); 4] = std::array::from_fn(|i| {
                let amp =
                    [0.10, 0.08, 0.05, 0.03][i] * wild * (0.6 + 0.8 * unit(slot(i as i64), 0));
                ([2.0, 3.0, 4.0, 6.0][i], amp, unit(h(i as i64), 8) * TAU)
            });
            let (axis, stretch) = match d.kind {
                Kind::Centre => (FRAC_PI_2 + 0.18, CENTRE_STRETCH),
                Kind::Gun => (0.0, 1.0),
                _ => (unit(h(9), 16) * PI, 1.0 + 0.55 * unit(slot(9), 16)),
            };
            out.push(Isle {
                at,
                r: d.r,
                kind: d.kind,
                harbour,
                lobes,
                axis,
                stretch,
                noise_at: (
                    unit(h(11), 0) * 4_000.0 - 2_000.0,
                    unit(h(12), 0) * 4_000.0 - 2_000.0,
                ),
            });
        };
        push(
            &isle(0.0, 0.0, CENTRE_R, Kind::Centre),
            (0.0, 0.0),
            None,
            &mut out,
        );
        for d in WEST {
            let harbour = (d.kind == Kind::Home).then_some(d.harbour);
            push(d, d.at, harbour, &mut out);
            push(d, turn(d.at), harbour.map(|h| h + PI), &mut out);
        }
        out
    }

    /// Guaranteed land round every home start and ore field, both sides' (design m, radius).
    fn lay_anchors() -> Vec<((f64, f64), f64)> {
        let homes = WEST
            .iter()
            .filter(|d| d.kind == Kind::Home)
            .flat_map(|d| [(d.at, SAFE), (turn(d.at), SAFE)]);
        let ore = ORE
            .iter()
            .flat_map(|&(at, r)| [(at, r + 230.0), (turn(at), r + 230.0)]);
        let home_ore = home_ore().into_iter().map(|(at, r)| (at, r + 200.0));
        homes.chain(ore).chain(home_ore).collect()
    }

    /// The coast noise at a design point (metres the coast is pushed out).
    fn coast_noise(&self, q: (f64, f64), calm: f64) -> f64 {
        let big = self.coast.fbm(q.0 / 900.0, q.1 / 900.0, 4, 0.55) * 230.0;
        let fine = self.coast_warp.fbm(q.0 / 230.0, q.1 / 230.0, 3, 0.5) * 60.0;
        (big + fine) * (0.3 + 0.7 * calm)
    }

    /// 1 in open country, falling to 0 round the starts and the ore.
    fn arch_keep(&self, q: (f64, f64)) -> f64 {
        self.arch.anchors.iter().fold(1.0, |k: f64, &(at, r)| {
            if (q.0 - at.0).abs() > r + 250.0 || (q.1 - at.1).abs() > r + 250.0 {
                return k;
            }
            k.min(smoothstep(r - 120.0, r + 250.0, dist(q, at)))
        })
    }

    fn near(&self, q: (f64, f64)) -> Near<'_> {
        let calm = self.arch_keep(q);
        let wobble = self.coast_noise(q, calm);
        // The outlines bend: every island is read through one slow warp of the
        // plane, so coasts run in arcs and crescents rather than round the centre.
        let warp = (
            self.warp_x.fbm(q.0 / 1_700.0 + 17.0, q.1 / 1_700.0, 3, 0.5) * WARP,
            self.warp_y.fbm(q.0 / 1_700.0, q.1 / 1_700.0 - 23.0, 3, 0.5) * WARP,
        );
        let (mut best, mut next): (Option<(&Isle, f64)>, f64) = (None, f64::NEG_INFINITY);
        for isle in &self.arch.isles {
            let bend = match isle.kind {
                Kind::Gun => 0.1,
                Kind::Centre => 0.8,
                _ => (isle.r / HOME_R).min(1.0),
            } * (1.0 - 0.85 * isle.harbour_at(q));
            let mut s = isle.inside((q.0 + warp.0 * bend, q.1 + warp.1 * bend));
            if s > -3_500.0 {
                // The harbour's coast keeps still; guns keep their round tops.
                let k = if isle.kind == Kind::Gun {
                    0.2
                } else {
                    1.0 - 0.8 * isle.harbour_at(q)
                };
                s += wobble * k;
            }
            match best {
                Some((_, b)) if s <= b => next = next.max(s),
                _ => {
                    if let Some((_, b)) = best {
                        next = next.max(b);
                    }
                    best = Some((isle, s));
                }
            }
        }
        let (isle, mut s) = best.expect("the map has islands");
        // Land kept round starts and ore, blended in so it reads as the island's own.
        for &(at, r) in &self.arch.anchors {
            if (q.0 - at.0).abs() > r + 400.0 || (q.1 - at.1).abs() > r + 400.0 {
                continue;
            }
            let d = dist(q, at);
            if d < r + 400.0 {
                let a = r - d;
                s = 0.5 * (s + a + ((s - a).powi(2) + 90.0f64.powi(2)).sqrt());
            }
        }
        Near { isle, s, next }
    }

    /// The landscape before pads.
    pub(super) fn natural_archipelago(&self, x: f64, y: f64) -> f64 {
        let q = self.ad((x, y));
        let n = self.near(q);
        let s = n.s;
        let isle = n.isle;
        let keep = self.arch_keep(q);
        let nq = (q.0 + isle.noise_at.0, q.1 + isle.noise_at.1);

        // -- the shore: long white beaches, and here and there a low ironshore bluff --
        let bluff = smoothstep(
            0.25,
            0.5,
            self.crag.fbm(q.0 / 700.0 + 13.0, q.1 / 700.0 - 7.0, 2, 0.5),
        ) * keep
            * (1.0 - isle.harbour_at(q))
            * if isle.kind == Kind::Gun { 0.0 } else { 1.0 };
        let run = 150.0 - 110.0 * bluff;
        let beach =
            3.2 * smoothstep(0.0, run, s).powf(0.8) + 5.0 * bluff * smoothstep(run * 0.3, run, s);

        let h = if s >= 0.0 {
            match isle.kind {
                Kind::Gun => {
                    // A flat top on cliffs, a sand apron at their foot.
                    let top = 26.0 + 2.0 * self.detail.fbm(q.0 / 90.0, q.1 / 90.0, 2, 0.5);
                    let foot = 0.2 * isle.r;
                    let cliff = smoothstep(foot, foot + 45.0, s);
                    beach + (top - beach) * cliff
                }
                _ => {
                    // Sand (the tropical shading's, up to about 12 m) gives way to grass
                    // and jungle a few hundred metres in.
                    let inland = smoothstep(run * 0.8, run + 380.0, s);
                    let upland = smoothstep(
                        -0.3,
                        0.4,
                        self.mtn_mask.fbm(nq.0 / 1_600.0, nq.1 / 1_600.0, 3, 0.5),
                    );
                    let size: f64 = match isle.kind {
                        Kind::Home => 1.0,
                        Kind::Centre => 1.25,
                        _ => 0.45,
                    };
                    let mut land = beach + inland * (10.5 + 8.0 * upland) * size.min(1.0);
                    // Low limestone hills, rounded, jungle-clad; the big island has the biggest.
                    let hills = {
                        let (wx, wy) = (
                            self.warp_x.get(nq.0 / 1_800.0, nq.1 / 1_800.0) * 500.0,
                            self.warp_y.get(nq.0 / 1_800.0, nq.1 / 1_800.0) * 500.0,
                        );
                        self.mtn
                            .fbm((nq.0 + wx) / 1_100.0, (nq.1 + wy) / 1_100.0, 4, 0.5)
                    };
                    land += 34.0
                        * size
                        * smoothstep(0.0, 0.5, hills).powf(1.3)
                        * smoothstep(run, run + 700.0, s)
                        * keep;
                    // Hummocks and swales, what the eye reads at play zoom.
                    land += 4.5
                        * self.mtn_height.fbm(nq.0 / 420.0, nq.1 / 420.0, 3, 0.5)
                        * inland
                        * (0.4 + 0.6 * keep);
                    land += 1.2 * self.detail.fbm(q.0 / 90.0, q.1 / 90.0, 3, 0.5) * inland;
                    // Knolls of bare rock among the trees.
                    let knoll = self
                        .crag
                        .fbm(nq.0 / 380.0 + 5.1, nq.1 / 380.0 - 2.7, 2, 0.45);
                    land += 7.0 * smoothstep(0.25, 0.45, knoll) * inland * keep;
                    land.max(beach.min(2.0))
                }
            }
        } else {
            self.sea(q, &n)
        };
        // Deep water off every edge.
        let e = x.min(y).min(self.size_x - x).min(self.size_y - y);
        h + (-60.0 - h) * (1.0 - smoothstep(150.0, 900.0, e))
    }

    /// The sea floor: sand shelving off the beach, a bank of white sand out to
    /// where the reef drops away, then deep water. Banks keep clear of the
    /// middle of every channel so ships always have a way through.
    fn sea(&self, q: (f64, f64), n: &Near<'_>) -> f64 {
        let d = -n.s;
        let isle = n.isle;
        // Across the channel to the next coast: its middle stays deep.
        let gap = d - n.next;
        let wide = smoothstep(
            -0.2,
            0.45,
            self.lake
                .fbm(q.0 / 1_300.0 + 3.0, q.1 / 1_300.0 - 8.0, 3, 0.5),
        );
        let mut bank = (90.0 + 650.0 * wide)
            .min(0.3 * gap)
            .min(0.5 * gap - 420.0)
            .max(40.0)
            * (1.0 - 0.9 * isle.harbour_at(q));
        if isle.kind == Kind::Gun {
            bank = bank.min(140.0);
        }
        // The shoals out in open water, fair both ways: reach by the design, outline by the noise.
        let fray = self.lake_shore.fbm(q.0 / 520.0, q.1 / 520.0, 3, 0.5) * 260.0;
        let shoal = BANKS
            .iter()
            .flat_map(|&(at, r)| [(at, r), (turn(at), r)])
            .map(|(at, r)| r + fray - dist(q, at))
            .fold(f64::MIN, f64::max);

        let ripple = 0.6 * self.detail.fbm(q.0 / 60.0, q.1 / 60.0, 2, 0.5);
        let flat = -2.6 - 1.4 * smoothstep(0.0, 1.0, (d / bank.max(1.0)).min(1.0)) + ripple;
        let shelf = -2.4 * smoothstep(0.0, 70.0, d).powf(0.7);
        let near_floor = shelf.min(0.0).max(flat);
        let deep = -(42.0 + 22.0 * self.tilt.fbm(q.0 / 2_400.0, q.1 / 2_400.0, 3, 0.5).abs() * 1.4);
        let mut h = near_floor + (deep - near_floor) * smoothstep(bank, bank + 320.0, d);
        if shoal > -600.0 {
            let k = smoothstep(-450.0, 0.0, shoal);
            h += (-3.2 + ripple - h) * k;
        }
        let h = h.min(-0.4);
        // Little sand cays out on the banks, where no ship goes anyway.
        let on_bank = (smoothstep(160.0, 260.0, d)
            * (1.0 - smoothstep(bank - 160.0, bank - 60.0, d)))
        .max(smoothstep(-250.0, -80.0, shoal));
        if on_bank > 0.0 && isle.kind != Kind::Gun {
            let cay = self
                .crag
                .fbm(q.0 / 240.0 - 31.0, q.1 / 240.0 + 12.0, 3, 0.5);
            let rise = smoothstep(0.34, 0.5, cay) * on_bank * (1.0 - isle.harbour_at(q));
            if rise > 0.0 {
                return h + (2.6 + 1.5 * smoothstep(0.5, 0.65, cay) - h) * rise;
            }
        }
        h
    }

    /// How thickly trees grow, and how much of the stand is palm. Woods are
    /// not income here worth fairness: they are not turned.
    pub(super) fn archipelago_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        let broad = self.forest.fbm(x / l, y / l, 3, 0.5);
        let jungle = smoothstep(self.forest_edge, self.forest_edge + 0.2, broad);
        let clearing = smoothstep(
            0.35,
            0.6,
            self.forest
                .fbm(x / (0.1 * l) - 33.7, y / (0.1 * l) + 57.2, 2, 0.5),
        );
        let copse = smoothstep(
            0.35,
            0.6,
            self.forest
                .fbm(x / (0.2 * l) + 71.3, y / (0.2 * l) - 19.1, 2, 0.5),
        );
        // The strand: palms along the top of the beach, open sand below them. The
        // jungle's edge wanders up and down the slope and thins out over a deep
        // band, palms running on into it, so the wood does not stop on a contour.
        let wander = 4.5
            * self
                .forest_kind
                .fbm(x / 260.0 + 7.3, y / 260.0 - 3.1, 3, 0.5)
            + 1.5 * self.detail.fbm(x / 45.0, y / 45.0, 2, 0.5);
        let h = height + wander;
        let strand = smoothstep(1.7, 2.6, height) * (1.0 - smoothstep(10.0, 17.0, h));
        let inland = smoothstep(6.0, 16.0, h).powf(1.4);
        let mut density = strand * (0.32 + 0.25 * copse)
            + inland * ((0.45 + 0.55 * jungle) * (1.0 - 0.75 * clearing)).max(copse * 0.8);
        density *= (1.0 - smoothstep(0.35, 0.6, slope)) * self.start_clearing(x, y);
        // A glade round every ore field.
        for f in &self.ore {
            let d = ((x - f.x).powi(2) + (y - f.y).powi(2)).sqrt() - f.radius;
            if d < 200.0 {
                density *= smoothstep(60.0, 200.0, d);
            }
        }
        let palms = (1.0 - inland)
            + inland
                * (0.12
                    + 0.4
                        * smoothstep(0.2, 0.6, self.forest_kind.fbm(x / 600.0, y / 600.0, 2, 0.5)));
        (density, palms.clamp(0.0, 1.0))
    }

    /// Species: palms by the share given, jungle hardwoods with a few broadleaves among them.
    pub(super) fn archipelago_tree(&self, conifer: f64, hash: u64) -> PropKind {
        match (unit(hash, 40), unit(hash, 48)) {
            (dead, _) if dead < 0.006 => PropKind::TreeDead,
            (_, pick) if pick < conifer => PropKind::TreePalm,
            (_, pick) if pick > 0.86 => PropKind::TreeBroadleaf,
            _ => PropKind::TreeJungle,
        }
    }

    /// 0 in the clearing round a start, rising to 1 in the jungle. The clearing's
    /// edge wanders in and out with the bearing and thins out over a wood's edge,
    /// so a start sits in a glade, not in a drawn circle.
    pub(super) fn start_clearing(&self, x: f64, y: f64) -> f64 {
        let f = self.af();
        let mut k = 1.0f64;
        for &(sx, sy) in &self.starts {
            let (dx, dy) = (x - sx, y - sy);
            let d = dx.hypot(dy);
            if d > (PAD_CORE + CLEARING_EDGE + 120.0) * f {
                continue;
            }
            let (s, c) = dy.atan2(dx).sin_cos();
            let wander = self
                .forest_kind
                .fbm(sx / 311.0 + c * 1.7, sy / 311.0 + s * 1.7, 3, 0.5);
            let edge = (CLEARING.0 + CLEARING.1 * wander).clamp(0.35, 0.95) * PAD_CORE * f;
            let tatter = 45.0 * f * self.detail.fbm(x / 70.0 + 4.1, y / 70.0 - 9.3, 2, 0.5);
            k = k.min(smoothstep(edge, edge + CLEARING_EDGE * f, d + tatter));
        }
        k
    }
}

/// Home ore, both sides': three fields inside each pad, and two out on the island.
/// Each home's five, then its twin's five.
fn home_ore() -> Vec<((f64, f64), f64)> {
    let mut out = Vec::new();
    for d in WEST.iter().filter(|d| d.kind == Kind::Home) {
        for (at, harbour) in [(d.at, d.harbour), (turn(d.at), d.harbour + PI)] {
            let back = harbour + PI;
            for (turn_by, reach, r) in [
                (0.0, 0.78 * PAD_CORE, 66.0),
                (1.15, 0.78 * PAD_CORE, 66.0),
                (-1.15, 0.78 * PAD_CORE, 66.0),
                (0.95, 820.0, 78.0),
                (-0.95, 820.0, 78.0),
            ] {
                let (s, c) = (back + turn_by).sin_cos();
                out.push(((at.0 + c * reach, at.1 + s * reach), r));
            }
        }
    }
    out
}

impl Terrain {
    /// Land (design m², sampled) that island `i` holds.
    fn isle_area(&self, i: usize) -> f64 {
        let isle = &self.arch.isles[i];
        let step = (isle.r / 40.0).clamp(10.0, 40.0);
        let reach = isle.r * 2.2 + 400.0;
        let mut land = 0.0;
        let mut y = -reach;
        while y < reach {
            let mut x = -reach;
            while x < reach {
                let q = (isle.at.0 + x, isle.at.1 + y);
                let n = self.near(q);
                if n.s > 0.0 && std::ptr::eq(n.isle, isle) {
                    land += step * step;
                }
                x += step;
            }
            y += step;
        }
        land
    }

    /// Twins' outlines differ, so their sizes are evened out: each is scaled
    /// until both hold as much land as they did on average.
    fn even_out_twins(&mut self) {
        for _ in 0..4 {
            for i in (1..self.arch.isles.len()).step_by(2) {
                let (a, b) = (self.isle_area(i), self.isle_area(i + 1));
                let mean = 0.5 * (a + b);
                self.arch.isles[i].r *= (mean / a).sqrt();
                self.arch.isles[i + 1].r *= (mean / b).sqrt();
            }
        }
    }

    /// Starts, pads, ore and the pit's artifacts.
    pub(super) fn setup_archipelago(&mut self) {
        self.arch = Archipelago {
            isles: Terrain::lay_isles(self.seed),
            anchors: Terrain::lay_anchors(),
        };
        self.even_out_twins();
        let f = self.af();
        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        self.start_outer = PAD_OUTER * f;

        // Starts: each west home followed by its twin, so skirmish's "Two
        // Sides" (teams by alternate slot) puts one side on each half.
        let mut pads = Vec::new();
        for d in WEST.iter().filter(|d| d.kind == Kind::Home) {
            let at = snap(self.aw(d.at));
            for at in [at, (self.size_x - at.0, self.size_y - at.1)] {
                pads.push(Pad {
                    x: at.0,
                    y: at.1,
                    core: PAD_CORE * f,
                    outer: PAD_OUTER * f,
                    height: PAD_HEIGHT,
                });
                self.starts.push(at);
            }
        }
        self.pads = pads;

        // Ore: every west field laid and its twin copied turned, corner for corner.
        let mut sites: Vec<((f64, f64), f64)> = ORE.to_vec();
        // `home_ore` lists each home's fields, then its twin's: keep the west ones to copy.
        sites.extend(
            home_ore()
                .into_iter()
                .enumerate()
                .filter(|(i, _)| (i / 5) % 2 == 0)
                .map(|(_, s)| s),
        );
        let mut fields = Vec::new();
        for (q, r) in sites {
            let p = snap(self.aw(q));
            let field = self.ore_field(p.0, p.1, r * f);
            let t = (self.size_x - p.0, self.size_y - p.1);
            let corners = field
                .corners
                .iter()
                .map(|&c| (self.size_x - c.0, self.size_y - c.1))
                .collect();
            fields.push(OreField {
                x: t.0,
                y: t.1,
                radius: field.radius,
                corners,
            });
            fields.push(field);
        }
        self.ore = fields;

        self.lay_machine();
        self.fit_forests();
    }

    /// Where a cay's outpost stands: in from the coast facing the big island,
    /// clear of the ore in its middle (design m).
    fn outpost_at(d: &Design) -> (f64, f64) {
        let (x, y) = d.at;
        let k = (0.55 * d.r).min(d.r - 120.0) / x.hypot(y);
        (x - x * k, y - y * k)
    }

    /// The machine: the citadel at the heart of the big island, a bastion at its
    /// north end with booms reaching toward it, a tower at its south end with a
    /// span running into it; an outpost on every cay and gun rock; and seaways
    /// lying on the seabed from node to node across the map, lines of light where
    /// they come ashore. Everything that blocks is laid for the west and turned for
    /// the east, and on the big island stands on its long axis.
    pub(super) fn machine_axis(&self) -> Machine<'_> {
        let f = self.af();
        let (s, c) = AXIS_HEADING.sin_cos();
        let along = |d: f64| self.aw((c * d, s * d));
        let mut m = Machine::new(self, 46.0 * f);
        let level = MACHINE_LEVEL;
        let south = AXIS_HEADING + PI;
        let bastion = m.bastion(along(BASTION_AT), south, 1.25 * f, level);
        m.booms(bastion, south, 1.3 * f, 1.25 * f);
        let citadel = m.citadel(along(CITADEL_AT), AXIS_HEADING, CITADEL_SCALE * f, level);
        let tower = m.tower(along(TOWER_AT), AXIS_HEADING, 1.15 * f, level);
        m.link(tower, citadel);

        // The outposts, west then turned.
        for &(i, outpost) in OUTPOSTS {
            let d = &WEST[i];
            for twin in [false, true] {
                let q = Terrain::outpost_at(d);
                let q = if twin { turn(q) } else { q };
                let p = self.aw(q);
                // Facing the big island.
                let heading = (-q.1).atan2(-q.0);
                let level = self.natural(p.0, p.1).max(4.0);
                match outpost {
                    Outpost::Lamp => {
                        m.put(PropKind::PrecursorBeacon, p, heading, 1.4 * f);
                        for side in [-1.0, 1.0] {
                            let (sn, cs) = (heading + side * FRAC_PI_2).sin_cos();
                            m.put(
                                PropKind::PrecursorPylon,
                                (p.0 + cs * 34.0 * f, p.1 + sn * 34.0 * f),
                                heading,
                                1.2 * f,
                            );
                        }
                    }
                    Outpost::Tower(scale) => {
                        m.tower(p, heading, scale * f, level);
                    }
                    Outpost::Needle(scale) => {
                        m.put(PropKind::PrecursorNeedle, p, heading, scale * f);
                    }
                    Outpost::Bastion(scale) => {
                        let node = m.bastion(p, heading, scale * f, level);
                        m.booms(node, heading, scale * 1.2 * f, scale * f);
                    }
                    Outpost::Axis(scale) => {
                        m.axis(p, heading + FRAC_PI_4 * 0.5, scale * f, level);
                    }
                }
            }
        }

        // The seaways.
        let node = |a: At| -> (f64, f64) {
            match a {
                At::Bastion => (bastion.x, bastion.y),
                At::Citadel => (citadel.x, citadel.y),
                At::Tower => (tower.x, tower.y),
                At::Cay(i) => self.aw(Terrain::outpost_at(&WEST[i])),
                At::Twin(i) => self.aw(turn(Terrain::outpost_at(&WEST[i]))),
            }
        };
        let turned = |p: (f64, f64)| (self.size_x - p.0, self.size_y - p.1);
        for &(a, b) in SEAWAYS {
            let (pa, pb) = (node(a), node(b));
            self.seaway(&mut m, pa, pb);
            self.seaway(&mut m, turned(pa), turned(pb));
        }
        m
    }

    /// One seaway from `a` to `b` (world m): causeways end to end along the seabed,
    /// each scaled to the water over it, and flush lines of light wherever the line
    /// runs over land or water too shallow to cover a causeway.
    fn seaway(&self, m: &mut Machine<'_>, a: (f64, f64), b: (f64, f64)) {
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let dir = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let heading = dir.1.atan2(dir.0);
        let at = |t: f64| (a.0 + dir.0 * t, a.1 + dir.1 * t);
        // Halfway, where the water is deep all round: a platform standing astride
        // the causeway on its four legs, ships sailing between them.
        let f = self.af();
        let mid = at(len * 0.5);
        let (s, c) = heading.sin_cos();
        let legs = [
            (170.0, 100.0),
            (170.0, -100.0),
            (-170.0, 100.0),
            (-170.0, -100.0),
            (0.0, 0.0),
            (300.0, 0.0),
            (-300.0, 0.0),
        ];
        let deep = legs.iter().all(|&(lx, ly)| {
            let p = (mid.0 + (c * lx - s * ly) * f, mid.1 + (s * lx + c * ly) * f);
            self.natural(p.0, p.1) < -24.0
        });
        if deep && len > 2_400.0 * f {
            m.put(PropKind::PrecursorPlatform, mid, heading, f);
        }
        // Keep clear of the nodes' own footprints at both ends.
        let mut t = 0.0;
        while t < len {
            let p = at(t);
            let depth = -self.natural(p.0, p.1);
            let fits = |s: f64| {
                // The shallowest water under the whole causeway must still cover it.
                let q = at(t + SEAWAY_LEN * s);
                let shallow = depth.min(-self.natural(q.0, q.1)).min(-self.natural(
                    at(t + SEAWAY_LEN * s * 0.5).0,
                    at(t + SEAWAY_LEN * s * 0.5).1,
                ));
                shallow > SEAWAY_TOP * s + 0.6
            };
            let mut scale = (depth / SEAWAY_DEPTH).clamp(0.15, 1.8);
            while scale > 0.15 && !fits(scale) {
                scale *= 0.8;
            }
            if depth > 2.0 && fits(scale) && t + SEAWAY_LEN * scale < len {
                let mid = at(t + SEAWAY_LEN * scale * 0.5);
                m.put(PropKind::PrecursorSeaway, mid, heading, scale);
                t += SEAWAY_LEN * scale;
            } else {
                // Over land or the shallows: a flush line of light, where the ground
                // is even and nothing stands.
                let flat = [(24.0, 0.0), (-24.0, 0.0), (0.0, 24.0), (0.0, -24.0)]
                    .iter()
                    .all(|&(dx, dy)| (self.natural(p.0 + dx, p.1 + dy) + depth).abs() < 3.0);
                if flat && !in_solid(&m.sites, p.0, p.1, 20.0) {
                    m.conduit(p, heading);
                }
                t += 61.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BakeParams;

    fn map() -> Terrain {
        Terrain::new(&BakeParams::archipelago("t", 10, 23))
    }

    /// The layout's pieces are where they must be: every start on its island
    /// with its harbour, islands clear of each other, the twins turned.
    #[test]
    fn islands_stand_apart_and_keep_channels() {
        let t = map();
        let isles = &t.arch.isles;
        // The farthest each coast reaches, with room for the coast noise.
        let reach = |isle: &Isle| {
            (0..360)
                .map(|n| isle.reach(n as f64 / 360.0 * TAU))
                .fold(0.0, f64::max)
                + 150.0
        };
        for (i, a) in isles.iter().enumerate() {
            for b in &isles[i + 1..] {
                let gap = dist(a.at, b.at) - reach(a) - reach(b);
                assert!(
                    gap > 200.0,
                    "islands at {:?} and {:?} are {gap:.0} m apart",
                    a.at,
                    b.at
                );
            }
        }
    }

    #[test]
    fn starts_are_level_on_their_islands_with_deep_water_near() {
        let t = map();
        assert_eq!(t.starts.len(), 8);
        for (k, &s) in t.starts.iter().enumerate() {
            assert!((t.height(s.0, s.1) - PAD_HEIGHT).abs() < 0.01, "start {k}");
            // Land all round the pad, then the harbour's deep water within reach.
            for n in 0..32 {
                let a = n as f64 / 32.0 * TAU;
                let p = (
                    s.0 + a.cos() * PAD_OUTER * t.af(),
                    s.1 + a.sin() * PAD_OUTER * t.af(),
                );
                assert!(
                    t.height(p.0, p.1) > 1.0,
                    "start {k}: sea at the pad's edge, bearing {a:.2}"
                );
            }
            let deep = (0..64).any(|n| {
                let a = n as f64 / 64.0 * TAU;
                let p = (
                    s.0 + a.cos() * 1_400.0 * t.af(),
                    s.1 + a.sin() * 1_400.0 * t.af(),
                );
                t.height(p.0, p.1) < -8.0
            });
            assert!(deep, "start {k}: no deep water within 1400 m");
        }
    }

    #[test]
    fn twins_share_starts_and_ore() {
        let t = map();
        for pair in t.starts.chunks(2) {
            assert!(
                (pair[0].0 + pair[1].0 - t.size_x).abs() < 1e-6
                    && (pair[0].1 + pair[1].1 - t.size_y).abs() < 1e-6
            );
        }
        for pair in t.ore.chunks(2) {
            assert!(
                (pair[0].x + pair[1].x - t.size_x).abs() < 1e-6
                    && (pair[0].radius - pair[1].radius).abs() < 1e-9
            );
        }
        // Nothing on the big island.
        for o in &t.ore {
            let q = t.ad((o.x, o.y));
            assert!(q.0.hypot(q.1) > 3_800.0, "ore at {q:?} on the big island");
        }
    }

    /// Twin islands are the same size to play on, though not the same shape.
    #[test]
    fn twin_islands_hold_the_same_ground() {
        let t = map();
        let f = t.af();
        for pair in t.arch.isles[1..].chunks(2) {
            let area = |isle: &Isle| {
                let (mut land, mut flat, mut same) = (0, 0, 0);
                let step = if isle.r < 700.0 { 16.0 } else { 40.0 };
                let r = isle.r * 1.6;
                let mut y = -r;
                while y < r {
                    let mut x = -r;
                    while x < r {
                        let q = (isle.at.0 + x, isle.at.1 + y);
                        let w = t.aw(q);
                        let h = t.height(w.0, w.1);
                        if h > 0.5 {
                            land += 1;
                            if t.slope(w.0, w.1) < 0.2 {
                                flat += 1;
                            }
                        }
                        // Does the twin's ground at the turned point look the same?
                        let tw = t.aw(turn(q));
                        same += ((h > 0.5) == (t.height(tw.0, tw.1) > 0.5)) as i32;
                        x += step / f;
                    }
                    y += step / f;
                }
                (land as f64, flat as f64, same)
            };
            let (a, b) = (area(&pair[0]), area(&pair[1]));
            let off = (a.0 - b.0).abs() / a.0.max(b.0);
            assert!(
                off < 0.08,
                "islands at {:?}: land {} vs {}",
                pair[0].at,
                a.0,
                b.0
            );
            let off = (a.1 - b.1).abs() / a.1.max(b.1);
            assert!(
                off < 0.12,
                "islands at {:?}: flat ground {} vs {}",
                pair[0].at,
                a.1,
                b.1
            );
        }
    }

    /// The big island is dry ground all through: no pit, no water in its middle.
    #[test]
    fn the_big_island_is_dry_through_its_middle() {
        let t = map();
        let (s, c) = AXIS_HEADING.sin_cos();
        for n in -20..=20 {
            let along = n as f64 * 90.0;
            for across in [-400.0, 0.0, 400.0] {
                let w = t.aw((c * along - s * across, s * along + c * across));
                assert!(
                    t.height(w.0, w.1) > 8.0,
                    "low ground at {along}, {across}: {:.1}",
                    t.height(w.0, w.1)
                );
            }
        }
    }

    /// The start glades are not circles: the jungle's edge stands at different
    /// distances on different bearings.
    #[test]
    fn start_glades_are_ragged() {
        let t = map();
        for &(sx, sy) in &t.starts {
            let edges: Vec<f64> = (0..36)
                .map(|n| {
                    let a = n as f64 / 36.0 * TAU;
                    let mut d = 0.0;
                    while t.start_clearing(sx + a.cos() * d, sy + a.sin() * d) < 0.5 && d < 2_000.0
                    {
                        d += 5.0;
                    }
                    d
                })
                .collect();
            let (lo, hi) = edges
                .iter()
                .fold((f64::MAX, 0.0f64), |(lo, hi), &e| (lo.min(e), hi.max(e)));
            assert!(
                lo > 100.0 * t.af(),
                "start at {:?}: the wood comes within {lo:.0} m",
                (sx, sy)
            );
            assert!(
                hi - lo > 60.0 * t.af(),
                "start at {:?}: the glade is a circle ({lo:.0}..{hi:.0} m)",
                (sx, sy)
            );
        }
    }

    #[test]
    fn beaches_are_walkable_ashore() {
        let t = map();
        // From each start, walking out on any bearing, the ground stays climbable
        // until the water.
        for &s in &t.starts {
            for n in 0..24 {
                let a = n as f64 / 24.0 * TAU;
                let mut d = PAD_OUTER * t.af();
                while d < 1_800.0 {
                    let p = (s.0 + a.cos() * d, s.1 + a.sin() * d);
                    if t.height(p.0, p.1) < 0.5 {
                        break;
                    }
                    assert!(t.slope(p.0, p.1) < 1.2, "cliff at {p:?}");
                    d += 20.0;
                }
            }
        }
    }
}
