//! Vigil: the tech 1 sensor ship, a slim armoured spacecraft carrying one great search
//! radar. +X is forward, +Y left, the ground at z 0 when landed (it sets down on skids).
//!
//! Contracts: drive mouths are `NOZZLES`, belly lift jets `LIFT_JETS`, the lamp fittings
//! modelled on the hull are `LAMPS`, and the radar turns as `part::SPINNER` about its
//! pivot (it stops when the ship's power fails, `entity.wgsl`). Every fit shares the
//! hull; only the radar differs (`Sensor`).
use std::f32::consts::TAU;

use super::capital::{self, CapitalRig};
use super::*;

/// The two stern drives' mouths (model space): the exhaust trails and drive effects start here.
pub(crate) const NOZZLES: [[f32; 3]; 2] = [[-56.0, -17.0, 12.0], [-56.0, 17.0, 12.0]];
/// Downward lift jets, mouth centres: under the nacelles and either side of the forward keel.
pub(crate) const LIFT_JETS: [[f32; 3]; 4] = [
    [-30.0, -17.0, 3.6],
    [-30.0, 17.0, 3.6],
    [32.0, -5.5, 3.6],
    [32.0, 5.5, 3.6],
];
/// Lamp fittings on the hull (model space): landing floods under the chin and the
/// nacelles, nav lights on the chines' widest points, strobes at the nose and nacelle
/// tails, one amber beacon on the spine aft.
pub(crate) const LAMPS: crate::CapitalLamps = crate::CapitalLamps {
    floods: &[
        [42.0, 3.0, 5.6],
        [42.0, -3.0, 5.6],
        [-40.0, 17.0, 5.4],
        [-40.0, -17.0, 5.4],
    ],
    nav_port: [2.0, 14.9, 9.0],
    nav_starboard: [2.0, -14.9, 9.0],
    strobes: &[[54.6, 0.0, 12.6], [-45.0, 17.0, 19.0], [-45.0, -17.0, 19.0]],
    beacons: &[[-36.0, 0.0, 24.6]],
    hold: None,
};

/// Size of the stern drives against the Bastion's 12 m bells.
const DRIVE_SCALE: f32 = 0.45;
/// What `entity.wgsl` animates (`models::capital_rig`): the drives' glow and vanes and the
/// lift jets' glow. No legs (skids) and no ramp or doors: it carries nothing.
pub(crate) const RIG: CapitalRig = CapitalRig {
    legs: None,
    door_hinge: 0.0,
    drives: Some((
        [NOZZLES[1][0], NOZZLES[1][2], NOZZLES[1][1], NOZZLES[1][1]],
        DRIVE_SCALE,
    )),
    lift_jets: Some((
        [
            LIFT_JETS[3][0],
            LIFT_JETS[3][1],
            LIFT_JETS[1][0],
            LIFT_JETS[1][1],
        ],
        LIFT_JETS[3][2],
    )),
    ramp: None,
};

/// Which radar the hull carries: the design variants shown side by side.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Sensor {
    /// A rotodome on two raked pylons over the spine, turning.
    Dome,
    /// A fixed phased-array blade standing on the spine, a top-hat plank over it, cheek
    /// arrays on the bow, and a small interrogator bar turning aft.
    Array,
    /// A ring of array panels on spokes over the spine, turning about a mast.
    Halo,
}

/// Keel height: the hull's underside, skids below it.
const KEEL: f32 = 4.0;
/// Hull stations: x, keel half width, chine half width and height, deck half width and height.
const STATIONS: [[f32; 6]; 6] = [
    [54.0, 0.8, 2.5, 11.0, 1.5, 15.0],
    [46.0, 3.0, 8.0, 9.5, 5.0, 20.0],
    [30.0, 5.0, 12.5, 9.0, 8.5, 24.0],
    [4.0, 5.5, 14.0, 9.0, 9.5, 25.0],
    [-26.0, 5.5, 13.5, 9.0, 9.0, 24.5],
    [-44.0, 4.5, 10.5, 10.0, 7.0, 22.0],
];

fn ring(s: &[f32; 6]) -> Vec<Vec3> {
    let [x, keel, chine, cz, deck, dz] = *s;
    vec![
        v3(x, keel, KEEL),
        v3(x, chine, cz),
        v3(x, deck, dz),
        v3(x, -deck, dz),
        v3(x, -chine, cz),
        v3(x, -keel, KEEL),
    ]
}

/// A point on the +y upper flank of station `s`, `t` of the way from chine to deck edge,
/// stood `lift` off the plating.
fn flank(s: &[f32; 6], t: f32, lift: f32) -> Vec3 {
    let [x, _, chine, cz, deck, dz] = *s;
    let (a, c) = (v2(chine, cz), v2(deck, dz));
    let n = v2(c.y - a.y, -(c.x - a.x)).normalize();
    let q = a + (c - a) * t + n * lift;
    v3(x, q.x, q.y)
}

/// The deck's top (z) at `x`, from the stations.
fn deck_at(x: f32) -> f32 {
    for p in STATIONS.windows(2) {
        if x >= p[1][0] {
            let t = ((p[0][0] - x) / (p[0][0] - p[1][0])).clamp(0.0, 1.0);
            return p[0][5] + (p[1][5] - p[0][5]) * t;
        }
    }
    STATIONS[5][5]
}

pub(crate) fn build(b: &mut MeshBuilder, sensor: Sensor) {
    if b.coarse() {
        coarse(b, sensor);
        return;
    }
    hull(b);
    nacelles(b);
    skids(b);
    lamps(b);
    match sensor {
        Sensor::Dome => dome(b),
        Sensor::Array => array(b),
        Sensor::Halo => halo(b),
    }
}

fn coarse(b: &mut MeshBuilder, sensor: Sensor) {
    b.paint(PLATING_DARK);
    b.frustum(
        v3(0.0, 0.0, KEEL),
        v2(100.0, 12.0),
        v2(84.0, 17.0),
        20.0,
        v2(0.0, 0.0),
    );
    b.cuboid(v3(-34.0, 0.0, 12.0), v3(20.0, 46.0, 12.0));
    b.paint(PLATING);
    match sensor {
        Sensor::Dome => b.prism(v3(-12.0, 0.0, 34.0), 6, 20.0, 17.0, 6.0),
        Sensor::Array => b.cuboid(v3(-13.0, 0.0, 31.0), v3(38.0, 3.0, 14.0)),
        Sensor::Halo => b.prism(v3(-8.0, 0.0, 31.5), 6, 28.5, 27.0, 5.0),
    }
}

/// The spine: a slab-sided dark hull, pale armour plates on the upper flanks with the
/// owner's stripe, a pale deck, and the bridge glazing at the bow.
fn hull(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    let rings = STATIONS.iter().map(ring).collect::<Vec<_>>();
    b.loft(&rings, true, true);
    // Pale deck plate down the spine.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    let deck = |s: &[f32; 6]| {
        let [x, _, _, _, w, z] = *s;
        vec![
            v3(x, w - 1.0, z - 0.3),
            v3(x, w - 2.2, z + 0.9),
            v3(x, -(w - 2.2), z + 0.9),
            v3(x, -(w - 1.0), z - 0.3),
        ]
    };
    b.loft(
        &STATIONS[1..6].iter().map(deck).collect::<Vec<_>>(),
        true,
        true,
    );
    b.mirror_y(|b| {
        // Armour over the upper flank, broken at the bridge.
        let plate = |s: &[f32; 6], t0: f32, t1: f32, lift: f32| {
            vec![
                flank(s, t0, -0.2),
                flank(s, t1, -0.2),
                flank(s, t1, lift),
                flank(s, t0, lift),
            ]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &STATIONS[2..6]
                .iter()
                .map(|s| plate(s, 0.12, 0.88, 0.7))
                .collect::<Vec<_>>(),
            true,
            true,
        );
        // The bridge: a glazed slit round the bow's upper flank.
        b.paint(GLASS);
        b.loft(
            &STATIONS[1..3]
                .iter()
                .map(|s| plate(s, 0.5, 0.78, 0.35))
                .collect::<Vec<_>>(),
            true,
            true,
        );
        if b.fine() {
            b.paint(TEAM);
            b.loft(
                &STATIONS[2..5]
                    .iter()
                    .map(|s| plate(s, 0.2, 0.3, 0.85))
                    .collect::<Vec<_>>(),
                true,
                true,
            );
        }
        if b.fine() {
            // A lit service rail along the chine, and hatch lids on the armour.
            b.paint(GLOW_AMBER);
            b.beam(
                flank(&STATIONS[2], 0.02, 0.1),
                flank(&STATIONS[4], 0.02, 0.1),
                v2(0.3, 0.3),
                v2(0.3, 0.3),
            );
            b.paint(ACCENT);
            for x in [22.0, 10.0, -2.0, -14.0] {
                let s = station_at(x);
                b.beam(
                    flank(&s, 0.5, 0.7),
                    flank(&s, 0.75, 0.7),
                    v2(3.2, 0.25),
                    v2(3.2, 0.25),
                );
            }
        }
    });
    team_panel(b, v3(26.0, 0.0, deck_at(26.0) + 0.6), v2(6.0, 5.0));
}

/// Station `x` interpolated between the hull's stations.
fn station_at(x: f32) -> [f32; 6] {
    for p in STATIONS.windows(2) {
        if x >= p[1][0] {
            let t = ((p[0][0] - x) / (p[0][0] - p[1][0])).clamp(0.0, 1.0);
            let mut s = [0.0; 6];
            for i in 0..6 {
                s[i] = p[0][i] + (p[1][i] - p[0][i]) * t;
            }
            return s;
        }
    }
    STATIONS[5]
}

/// An octagonal ring about the x axis through (y, z): half size `r`, corners cut by `cut`.
fn octagon(x: f32, y: f32, z: f32, r: f32, cut: f32) -> Vec<Vec3> {
    let k = r - cut;
    [
        (-k, -r),
        (k, -r),
        (r, -k),
        (r, k),
        (k, r),
        (-k, r),
        (-r, k),
        (-r, -k),
    ]
    .iter()
    .map(|&(dy, dz)| v3(x, y + dy, z + dz))
    .collect()
}

/// Two drive nacelles fused to the aft flanks, pale saddles over them, the drives and the
/// aft lift jets under them.
fn nacelles(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let [nx, y, z] = NOZZLES[1];
        let aft = nx + 27.6 * DRIVE_SCALE + 0.4;
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                octagon(-8.0, y - 2.0, z, 3.0, 1.2),
                octagon(-20.0, y, z, 6.0, 2.2),
                octagon(aft + 1.5, y, z, 6.2, 2.2),
                octagon(aft, y, z, 5.7, 2.0),
            ],
            true,
            true,
        );
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        const SADDLE: [[f32; 2]; 6] = [
            [-4.0, 6.6],
            [4.0, 6.6],
            [6.6, 4.0],
            [6.2, 3.8],
            [3.8, 6.2],
            [-3.8, 6.2],
        ];
        let saddle = |x: f32| {
            SADDLE
                .iter()
                .map(|&[dy, dz]| v3(x, y + dy, z + dz))
                .collect::<Vec<_>>()
        };
        b.loft(&[saddle(-19.0), saddle(aft + 2.0)], true, true);
        if b.fine() {
            b.paint(TEAM);
            b.beam(
                v3(-24.0, y - 3.4, z + 6.7),
                v3(-24.0, y + 3.4, z + 6.7),
                v2(1.2, 0.2),
                v2(1.2, 0.2),
            );
        }
        b.paint(ACCENT);
        b.loft(
            &[
                octagon(aft + 0.2, y, z, 5.9, 2.1),
                octagon(aft - 1.2, y, z, 5.4, 1.9),
            ],
            false,
            true,
        );
        capital::drive(b, Vec3::from(NOZZLES[1]), DRIVE_SCALE);
        // Aft lift jet in a fairing under the nacelle.
        let jet = Vec3::from(LIFT_JETS[1]);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(
            v3(jet.x, jet.y, jet.z + 1.2),
            v2(9.0, 7.0),
            v2(12.0, 8.0),
            z - 5.0 - jet.z,
            v2(0.0, 0.0),
        );
        capital::lift_jet(b, jet, 0.6);
        if b.fine() {
            b.paint(GLOW_RED);
            b.cuboid(v3(aft - 0.2, y + 5.0, z + 3.4), v3(0.6, 0.8, 0.8));
            b.paint(METAL);
            b.cylinder_between(
                v3(-20.0, y + 5.4, z - 2.0),
                v3(aft + 1.0, y + 5.4, z - 2.0),
                0.5,
                0.5,
                6,
            );
        }
    });
    // Forward lift jets either side of the keel.
    b.mirror_y(|b| capital::lift_jet(b, Vec3::from(LIFT_JETS[3]), 0.5));
}

/// Two skids under the keel on raked struts: the hull rests on them on the ground.
fn skids(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::AIRFRAME);
        b.frustum(
            v3(-2.0, 9.0, 0.0),
            v2(60.0, 3.2),
            v2(56.0, 2.4),
            1.4,
            v2(0.0, 0.0),
        );
        b.paint(METAL);
        for x in [-24.0, -4.0, 16.0] {
            b.beam(
                v3(x, 9.0, 1.2),
                v3(x + 2.0, 5.0, KEEL + 0.6),
                v2(1.6, 1.2),
                v2(1.4, 1.0),
            );
        }
    });
}

/// Fittings for the lamps the renderer lights (`LAMPS`): flood housings, nav pods, strobes.
fn lamps(b: &mut MeshBuilder) {
    for at in LAMPS.floods {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.cuboid(c + v3(0.0, 0.0, 0.7), v3(2.2, 1.8, 1.0));
        b.paint(GLOW);
        b.cuboid(c + v3(0.0, 0.0, 0.15), v3(1.4, 1.0, 0.12));
    }
    for (at, glow) in [(LAMPS.nav_port, GLOW_RED), (LAMPS.nav_starboard, GLOW)] {
        let c = Vec3::from(at);
        let out = c.y.signum();
        b.paint(PLATING_DARK);
        b.beam(
            c - v3(0.0, out * 1.6, 0.0),
            c - v3(0.0, out * 0.3, 0.0),
            v2(2.2, 1.4),
            v2(1.8, 1.0),
        );
        b.paint(glow);
        b.cuboid(c, v3(1.0, 0.7, 0.7));
    }
    for at in LAMPS.strobes {
        b.paint(GLOW);
        b.cuboid(Vec3::from(*at), v3(0.6, 0.6, 0.6));
    }
    let c = Vec3::from(LAMPS.beacons[0]);
    b.paint(ACCENT);
    b.prism(c - v3(0.0, 0.0, 1.2), b.sides(8), 0.9, 0.8, 0.9);
    b.paint(GLOW_AMBER);
    b.prism(c - v3(0.0, 0.0, 0.3), b.sides(8), 0.6, 0.5, 0.7);
}

/// Where the dome turns, and its radius.
const DOME: Vec3 = Vec3::new(-12.0, 0.0, 34.0);
const DOME_R: f32 = 20.4;

/// A: a rotodome, pale above and dark below with the owner's band round its rim, on two
/// raked pylons. Two dark blisters on the rim and a stripe over the top show it turning.
fn dome(b: &mut MeshBuilder) {
    let n = if b.fine() { 28 } else { 14 };
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    for (foot, head) in [(-2.0, -7.0), (-24.0, -17.0)] {
        b.beam(
            v3(foot, 0.0, deck_at(foot) + 0.5),
            v3(head, 0.0, DOME.z - 0.4),
            v2(7.0, 2.4),
            v2(4.0, 1.6),
        );
    }
    b.paint(METAL);
    b.prism(DOME - v3(0.0, 0.0, 1.6), b.sides(10), 3.4, 2.8, 2.0);
    b.set_spinner_pivot(DOME);
    b.with_part(part::SPINNER, |b| {
        let top = DOME + v3(0.0, 0.0, 6.0);
        b.paint(PLATING).pattern(pattern::PLAIN);
        capital::lathe(
            b,
            top,
            -Vec3::Z,
            &[
                [0.0, 1.2],
                [0.5, 9.0],
                [1.4, 16.0],
                [2.4, 19.6],
                [3.0, DOME_R],
                [3.0, 1.2],
            ],
            n,
        );
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        capital::lathe(
            b,
            top,
            -Vec3::Z,
            &[
                [3.0, 1.2],
                [3.0, DOME_R],
                [3.6, 19.6],
                [4.6, 16.0],
                [5.5, 9.0],
                [6.0, 1.2],
            ],
            n,
        );
        if b.fine() {
            b.paint(TEAM);
            capital::lathe(
                b,
                top,
                -Vec3::Z,
                &[
                    [2.6, DOME_R - 0.3],
                    [2.6, DOME_R + 0.3],
                    [3.4, DOME_R + 0.3],
                    [3.4, DOME_R - 0.3],
                ],
                n,
            );
            b.paint(ACCENT);
            b.mirror_y(|b| {
                b.spheroid(DOME + v3(0.0, 17.5, 3.4), v3(3.4, 3.0, 1.6), b.sides(10), 3);
                b.beam(
                    top + v3(0.0, 0.0, 0.05),
                    top + v3(0.0, 9.0, -0.45),
                    v2(2.4, 0.2),
                    v2(2.4, 0.2),
                );
                b.beam(
                    top + v3(0.0, 9.0, -0.45),
                    top + v3(0.0, 16.0, -1.35),
                    v2(2.4, 0.2),
                    v2(2.4, 0.2),
                );
            });
        }
    });
}

/// B: a fixed array. A tall blade on the spine with an array face on each side and a
/// pale top-hat plank over it; cheek arrays on the bow; a small interrogator bar turning
/// on a mast aft (the only moving part).
fn array(b: &mut MeshBuilder) {
    let base = deck_at(-13.0) - 0.3;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(-13.0, 0.0, base),
        v2(44.0, 3.4),
        v2(36.0, 2.6),
        36.0 - base,
        v2(0.0, 0.0),
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(-13.0, 0.0, 36.0),
        v2(40.0, 7.0),
        v2(36.0, 5.6),
        2.2,
        v2(0.0, 0.0),
    );
    b.paint(TEAM);
    b.cuboid(v3(-13.0, 0.0, 38.25), v3(8.0, 4.0, 0.12));
    // Rounded fairings at the blade's ends.
    b.paint(PLATING).pattern(pattern::PLAIN);
    for x in [-32.0, 6.0] {
        b.spheroid(v3(x, 0.0, 32.0), v3(3.0, 2.2, 5.5), b.sides(10), 4);
    }
    b.mirror_y(|b| {
        // The array face: a dark panel standing proud of the blade, its tiles lit faintly.
        b.paint(ACCENT);
        b.cuboid(v3(-13.0, 1.5, 31.0), v3(32.0, 0.6, 8.0));
        if b.fine() {
            b.paint(METAL);
            for k in 0..7 {
                let x = -26.0 + k as f32 * 4.33;
                b.cuboid(v3(x, 1.85, 31.0), v3(0.25, 0.2, 8.0));
            }
            b.cuboid(v3(-13.0, 1.85, 31.0), v3(32.0, 0.2, 0.25));
        }
        // Cheek arrays: dark panels over the bow's upper flank, behind the bridge.
        let s0 = station_at(40.0);
        let s1 = station_at(24.0);
        b.paint(ACCENT);
        b.loft(
            &[
                vec![
                    flank(&s0, 0.1, -0.2),
                    flank(&s0, 0.45, -0.2),
                    flank(&s0, 0.45, 0.9),
                    flank(&s0, 0.1, 0.9),
                ],
                vec![
                    flank(&s1, 0.1, -0.2),
                    flank(&s1, 0.45, -0.2),
                    flank(&s1, 0.45, 0.9),
                    flank(&s1, 0.1, 0.9),
                ],
            ],
            true,
            true,
        );
    });
    // Interrogator mast aft.
    let foot = v3(-40.0, 0.0, deck_at(-40.0) + 0.5);
    let pivot = foot + v3(0.0, 0.0, 7.0);
    b.paint(METAL);
    b.beam(foot, pivot, v2(1.4, 1.4), v2(0.8, 0.8));
    b.set_spinner_pivot(pivot);
    b.with_part(part::SPINNER, |b| {
        b.paint(ACCENT);
        b.cuboid(pivot + v3(0.0, 0.0, 0.6), v3(1.4, 10.0, 1.2));
        b.paint(METAL);
        b.cuboid(pivot + v3(0.0, 0.0, 0.1), v3(1.0, 1.0, 0.6));
    });
}

/// Where the halo turns.
const HALO: Vec3 = Vec3::new(-8.0, 0.0, 34.0);

/// C: a halo. A ring of eight array panels, canted outward, carried on four spokes from a
/// hub turning on a mast over the spine. Two panels carry the owner's colour so the
/// ring's turning shows.
fn halo(b: &mut MeshBuilder) {
    let foot = deck_at(HALO.x) - 0.3;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(
        v3(HALO.x, 0.0, foot),
        b.sides(10),
        3.6,
        2.4,
        HALO.z - 2.0 - foot,
    );
    b.set_spinner_pivot(HALO);
    b.with_part(part::SPINNER, |b| {
        b.paint(METAL);
        b.prism(HALO - v3(0.0, 0.0, 2.0), b.sides(10), 4.2, 3.6, 3.4);
        b.paint(PLATING_DARK);
        for k in 0..4 {
            let a = (k as f32 + 0.5) * TAU / 4.0;
            let d = v3(a.cos(), a.sin(), 0.0);
            b.beam(HALO + d * 3.8, HALO + d * 25.8, v2(1.6, 1.8), v2(1.2, 1.4));
        }
        let steps = if b.fine() { 3 } else { 1 };
        for k in 0..8 {
            let (a0, a1) = (
                k as f32 * TAU / 8.0 + 0.05,
                (k + 1) as f32 * TAU / 8.0 - 0.05,
            );
            let section = |a: f32, inner: f32, outer: f32, lo: f32, hi: f32| {
                let d = v3(a.cos(), a.sin(), 0.0);
                vec![
                    HALO + d * inner + v3(0.0, 0.0, lo),
                    HALO + d * outer + v3(0.0, 0.0, lo + 0.8),
                    HALO + d * (outer - 0.8) + v3(0.0, 0.0, hi),
                    HALO + d * (inner - 0.4) + v3(0.0, 0.0, hi - 0.6),
                ]
            };
            let arc = |inner: f32, outer: f32, lo: f32, hi: f32| {
                (0..=steps)
                    .map(|i| {
                        let a = a0 + (a1 - a0) * i as f32 / steps as f32;
                        section(a, inner, outer, lo, hi)
                    })
                    .collect::<Vec<_>>()
            };
            b.paint(if k % 4 == 0 { TEAM } else { PLATING })
                .pattern(pattern::AIRFRAME);
            b.loft(&arc(25.2, 28.2, -2.6, 2.6), true, true);
            if b.fine() {
                // The array face on the outer side.
                b.paint(ACCENT);
                b.loft(&arc(28.0, 28.6, -1.5, 1.7), true, true);
            }
        }
        if b.fine() {
            b.paint(GLOW_RED);
            b.cuboid(HALO + v3(0.0, 0.0, 1.7), v3(0.6, 0.6, 0.6));
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::{build_model_fitted, part};

    #[test]
    fn every_vigil_fit_turns_its_radar_sits_on_its_skids_and_keeps_lods_bounded() {
        for key in ["sensor_ship", "sensor_ship~array", "sensor_ship~halo"] {
            let model = build_model_fitted(key, 56.0, 40.0, 1, &[]).unwrap();
            let tris = model.lods.each_ref().map(|lod| lod.indices.len() / 3);
            // Budgets and the reduced share are `tests::lods_reduce_and_respect_budgets`.
            println!("{key} triangles: {tris:?}");
            let lod = &model.lods[0];
            assert!(
                lod.vertices.iter().any(|v| v.part == part::SPINNER),
                "{key}"
            );
            for v in &lod.vertices {
                assert!(
                    [part::HULL, part::SPINNER, part::DRIVE].contains(&v.part),
                    "{key}: part {}",
                    v.part
                );
                // Nothing hangs under the ground it lands on.
                assert!(v.pos[2] >= -0.61, "{key}: {:?}", v.pos);
            }
        }
    }
}
