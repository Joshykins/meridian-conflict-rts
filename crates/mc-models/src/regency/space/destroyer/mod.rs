//! The Regency's tech 4 capital ship (`regency_t4_space_destroyer`, mesh
//! `regency_space_destroyer`): the Scourge's hull dressed as the Regent was. One long
//! faceted arrowhead of hard chines, widest aft of the middle and run forward to a chisel
//! bow, under the Regent's feathered carapace and red-lit seams; out of its chines amidships
//! the Regent's two great arms, curving out to their corners and bending aft into claws
//! that reach the stern (`wings.rs`); the Regent's terraced command tower and its horns
//! aft. Through the ship's middle stands the drum of the energy core the main lance is laid
//! from (`core.rs`); six Pinch-plasma Batteries turn in ball mounts seated in pockets
//! pressed into the chines (`battery.rs`), four Twin AA Repeaters stand on pedestals on the
//! arms and the after deck and four Rondels guard the deck (`guns.rs`).
//! +X is forward, +Y port, the ground at z 0 (it is built on its lot, the core lowest).
//! Authored in metres as built.
//!
//! Contracts (tests.rs): the core's lens is `space.ron` weapon 0's `muzzle`, the guns'
//! pivots and muzzles are weapons 1 and 2's mounts, and the Rondels' lenses its
//! `anti_missile_mounts`.

pub(super) mod battery;
mod core;
mod guns;
#[cfg(test)]
mod tests;
mod wings;

use std::sync::LazyLock;

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::pattern;

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::Frame;
use super::body::{self, feather, st, Body, Station, BILGE, CHINE, DECK_EDGE, KEEL_EDGE, SPINE};
use super::dress::{course, drives, lifts, mark, pale_plate};
use super::style::{shard, Finish};
use battery::BALL_R;
use core::{core, Core};
use guns::{Kind, Mount, AA, RONDEL};
use wings::{arm, Wing};

/// The hull's stations, stern first, before the guns' pockets are pressed into them.
const STATIONS: [Station; 8] = [
    st(-310.0, 58.0, 20.0, 50.0, 68.0, 30.0, 96.0, 100.0),
    st(-262.0, 100.0, 26.0, 40.0, 64.0, 40.0, 108.0, 116.0),
    st(-160.0, 114.0, 28.0, 36.0, 62.0, 44.0, 112.0, 122.0),
    st(-40.0, 98.0, 24.0, 34.0, 62.0, 38.0, 106.0, 114.0),
    st(80.0, 78.0, 20.0, 38.0, 62.0, 30.0, 98.0, 104.0),
    st(200.0, 54.0, 16.0, 44.0, 63.0, 22.0, 88.0, 92.0),
    st(300.0, 30.0, 10.0, 52.0, 64.0, 12.0, 78.0, 80.0),
    st(362.0, 8.0, 3.0, 60.0, 65.0, 3.0, 70.0, 70.0),
];

/// One design of the ship: how big its arms are and where they are rooted, and where the
/// batteries' sockets stand down each chine, fore to aft, all ahead of the arms.
pub(crate) struct Design {
    wing: Wing,
    mounts: [f32; 3],
}

/// The arms at three tenths of the Regent's, their claws reaching the stern.
pub(crate) const A: Design = Design {
    wing: Wing {
        k: 0.30,
        root: Vec3::new(-175.0, 60.0, 62.0),
    },
    mounts: [250.0, 170.0, 90.0],
};

/// Larger arms, their claws a little past the stern.
pub(crate) const B: Design = Design {
    wing: Wing {
        k: 0.38,
        root: Vec3::new(-170.0, 60.0, 62.0),
    },
    mounts: [270.0, 200.0, 130.0],
};

/// The Regent's proportions: arms near half its size, rooted further forward, their claws
/// well past the stern.
pub(crate) const C: Design = Design {
    wing: Wing {
        k: 0.46,
        root: Vec3::new(-160.0, 60.0, 62.0),
    },
    mounts: [300.0, 245.0, 190.0],
};

/// The hull of `d`: `STATIONS`, the chine pressed in round each battery's ball into a
/// faceted pocket the hull's shoulder and bilge fold down and up into.
fn hull(d: &Design) -> Body {
    let plain = Body {
        stations: STATIONS.to_vec(),
    };
    let mut stations = STATIONS.to_vec();
    for &m in &d.mounts {
        for (dx, notch) in [
            (POCKET.0, 0.0),
            (POCKET.1, NOTCH),
            (-POCKET.1, NOTCH),
            (-POCKET.0, 0.0),
        ] {
            stations.push(Station {
                notch,
                ..plain.at(m + dx)
            });
        }
    }
    stations.sort_by(|a, c| a.x.total_cmp(&c.x));
    Body { stations }
}

static HULL: LazyLock<Body> = LazyLock::new(|| hull(&A));

/// The core stands at the ship's middle, so the beam leaves the same point whichever
/// way it is laid.
const CORE: Core = Core {
    x: 0.0,
    low: 20.0,
    high: 118.0,
};

/// The drives' mouths and radii: a great drive on the centreline over two lesser.
const DRIVES: [([f32; 3], f32); 3] = [
    ([-310.0, 0.0, 80.0], 14.0),
    ([-310.0, -32.0, 64.0], 8.0),
    ([-310.0, 32.0, 64.0], 8.0),
];

/// The stern drives' mouths, for the drive effects.
pub(crate) const NOZZLES: [[f32; 3]; 3] = [DRIVES[0].0, DRIVES[1].0, DRIVES[2].0];

/// The drives' size as the drive effects take it (1: a 12 m mouth): the great drive's.
pub(crate) const DRIVE_SIZE: f32 = DRIVES[0].1 / 12.0;

/// Each ball's pocket: how far the chine is pressed in at its bottom, and its half
/// lengths at the hull's line and at its flat bottom.
const NOTCH: f32 = 0.75 * BALL_R;
const POCKET: (f32, f32) = (1.7 * BALL_R, 0.45 * BALL_R);
/// How far the ball's middle stands out of the pocket's bottom: just inside the hull's
/// line, so its back half sits in the pocket.
const BALL_OUT: f32 = NOTCH - 0.1 * BALL_R;

/// The command tower on the spine aft: its middle, length and half-width, and its tiers
/// `(tall, scale)`.
const TOWER: (f32, f32, f32) = (-196.0, 118.0, 36.0);
const TOWER_TIERS: [(f32, f32); 3] = [(15.0, 1.0), (13.0, 0.8), (10.0, 0.6)];

/// The Rondels down each deck edge, fore to aft, and the AA repeaters on the after deck.
const RONDELS: [f32; 2] = [270.0, -110.0];
const DECK_AA: f32 = -262.0;

/// Every gun on a house, port side, and the Rondels' posts, port side.
fn mounts(d: &Design, body: &Body) -> (Vec<Mount>, Vec<Vec3>) {
    let mut port: Vec<Mount> = d
        .mounts
        .iter()
        .map(|&x| Mount::port(Kind::Battery, body.out(x, CHINE, BALL_OUT)))
        .collect();
    let corner = d.wing.root.y + 300.0 * d.wing.k;
    port.push(Mount::port(
        Kind::Aa,
        d.wing.rib_at(corner).top(0.8) + Vec3::Z * (AA * 0.5),
    ));
    port.push(Mount::port(
        Kind::Aa,
        body.out(DECK_AA, DECK_EDGE - 0.4, AA * 0.5),
    ));
    let rondels = RONDELS
        .iter()
        .map(|&x| (body.out(x, DECK_EDGE - 0.4, RONDEL * 0.5) * 4.0).round() / 4.0)
        .collect();
    (port, rondels)
}

pub(super) fn destroyer(b: &mut MeshBuilder, _tech: u8) {
    build(b, &A, &HULL);
}

pub(super) fn destroyer_b(b: &mut MeshBuilder, _tech: u8) {
    build(b, &B, &hull(&B));
}

pub(super) fn destroyer_c(b: &mut MeshBuilder, _tech: u8) {
    build(b, &C, &hull(&C));
}

fn build(b: &mut MeshBuilder, d: &Design, body: &Body) {
    if b.coarse() {
        coarse(b, d);
        return;
    }
    let (port, rondels) = mounts(d, body);
    body.hull(b);
    b.mirror_y(|b| {
        plates(b, body);
        carapace(b, body);
        arm(b, &d.wing);
        seams(b, body, 330.0, -280.0);
        guns::rondels(b, &rondels);
    });
    trench(b, body, (330.0, -296.0), 2.6);
    let crown = terrace(b, body, TOWER, &TOWER_TIERS);
    horns(b, TOWER, crown);
    drives(b, &DRIVES, 22.0);
    lifts(
        b,
        body,
        &[
            (140.0, 34.0),
            (140.0, -34.0),
            (-160.0, 50.0),
            (-160.0, -50.0),
        ],
        9.5,
    );
    core(b, body, CORE);
    guns::arm(b, &guns::guns(&port));
    mark(b, v3(120.0, 0.0, 104.5), 46.0);
}

/// Far off: the hull's plan as one slab, each arm a slab of its outline, and the owner's
/// mark.
fn coarse(b: &mut MeshBuilder, d: &Design) {
    let half = [[362.0, 0.0], [200.0, 54.0], [-160.0, 114.0], [-310.0, 58.0]];
    let mut outline: Vec<[f32; 2]> = half.to_vec();
    outline.extend(half[1..].iter().rev().map(|&[x, y]| [x, -y]));
    dark_plate(b);
    b.extrude_z(&outline, 40.0, 112.0);
    b.mirror_y(|b| d.wing.coarse(b));
    mark(b, v3(120.0, 0.0, 112.1), 50.0);
}

/// The plate courses, port side: charcoal everywhere but a pale stripe course down the
/// shoulder from bow to stern.
fn plates(b: &mut MeshBuilder, body: &Body) {
    dark_plate(b);
    if b.fine() {
        // The keel and the bilge, which the mid level leaves to the hull.
        course(
            b,
            body,
            (320.0, -290.0),
            5,
            (0.15, KEEL_EDGE + 0.35, KEEL_EDGE),
            1.8,
        );
        course(
            b,
            body,
            (300.0, -300.0),
            6,
            (BILGE - 0.3, CHINE + 0.25, CHINE + 0.1),
            1.6,
        );
    }
    course(
        b,
        body,
        (350.0, -296.0),
        8,
        (KEEL_EDGE + 0.2, CHINE - 0.1, CHINE - 0.4),
        2.2,
    );
    // The deck: charcoal plates either side of the trench.
    course(
        b,
        body,
        (356.0, -300.0),
        7,
        (DECK_EDGE - 0.2, SPINE - 0.15, DECK_EDGE + 0.3),
        2.4,
    );
    // The pale stripe down the shoulder.
    pale_plate(b);
    course(
        b,
        body,
        (358.0, -306.0),
        9,
        (CHINE + 0.08, DECK_EDGE + 0.1, CHINE + 0.3),
        2.8,
    );
}

/// The Regent's carapace, port side: feathered plates over the shoulder and deck from the
/// bow back to the arms' roots and one more aft of them, each one's trailing edge lifted
/// clear over the head of the next, so the hull reads as layered shell.
fn carapace(b: &mut MeshBuilder, body: &Body) {
    for (from, to, count, lift) in [(350.0, -20.0, 6, 9.0), (-40.0, -150.0, 1, 6.0)] {
        let step = (from - to) / count as f32;
        for k in 0..count {
            let head = from - step * k as f32;
            dark_plate(b);
            feather(
                b,
                body,
                (head, head - step * 1.3),
                (CHINE + 0.45, SPINE - 0.42, DECK_EDGE - 0.2),
                3.0,
                lift,
                0.45,
            );
        }
    }
}

/// The red lines, port side: lit slots let into the shoulder near the bow, largest
/// forward, and a lit seam run from them down the chine to the stern.
fn seams(b: &mut MeshBuilder, body: &Body, bow: f32, seam_to: f32) {
    if b.coarse() {
        return;
    }
    for (k, len) in [(0.0, 14.0), (20.0, 9.0), (35.0, 6.0)] {
        let x = bow - k;
        let f = super::style::on(body, x, CHINE + 0.45, 0.4, 0.04);
        shard(b, &f, (len * 1.6, 3.2, 0.2), 1.2, Finish::Lit);
    }
    dark_plate(b);
    b.pattern(pattern::EMBER_SLOT);
    body::skin(b, body, (bow - 45.0, seam_to), |_| {
        (CHINE + 0.3, CHINE + 0.42, -0.2, 0.6)
    });
}

/// The spine's trench: the conduit from the head to the drives in a dark channel down
/// the deck, coupled every so often through a sleeve with a lit rim.
fn trench(b: &mut MeshBuilder, body: &Body, (from, to): (f32, f32), r: f32) {
    let n = if b.fine() { 12 } else { 6 };
    let path: Vec<Vec3> = (0..=n)
        .map(|i| body.out(from + (to - from) * i as f32 / n as f32, SPINE, r * 0.6))
        .collect();
    metal(b);
    let sides = b.sides(8);
    for w in path.windows(2) {
        b.cylinder_between(w[0], w[1], r, r, sides);
    }
    let couplings = if b.fine() { 7 } else { 3 };
    for i in 0..couplings {
        let x = from + (to - from) * (i as f32 + 0.5) / couplings as f32;
        let c = body.out(x, SPINE, r * 0.6);
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(c + Vec3::X * 6.0, c - Vec3::X * 6.0, r * 1.7, r * 1.7, 6)
        });
        dark_plate(b);
        b.pattern(pattern::EMBER_RIM);
        b.cylinder_between(c + Vec3::X * 0.8, c - Vec3::X * 0.8, r * 1.78, r * 1.78, 6);
    }
}

/// The Regent's command tower on the spine centred at `x`: each tier `(tall, scale)` a
/// pointed hexagon in plan `len` by `half` at scale 1, set back and narrower than the one
/// under it, a lit slot round its brow. Returns its top.
fn terrace(
    b: &mut MeshBuilder,
    body: &Body,
    (x, len, half): (f32, f32, f32),
    tiers: &[(f32, f32)],
) -> f32 {
    let plan = |l: f32, h: f32| -> Vec<[f32; 2]> {
        vec![
            [l * 0.5, 0.0],
            [l * 0.24, h],
            [-l * 0.42, h * 0.94],
            [-l * 0.5, h * 0.55],
            [-l * 0.5, -h * 0.55],
            [-l * 0.42, -h * 0.94],
            [l * 0.24, -h],
        ]
    };
    let mut z = body.at(x).spine - 3.0;
    for (k, &(tall, f)) in tiers.iter().enumerate() {
        let (l, h) = (len * f, half * f);
        let back = -len * 0.06 * k as f32;
        let p = plan(l, h);
        b.at(v3(x + back, 0.0, 0.0), |b| {
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &p,
                    &[
                        Section::new(z, 1.0),
                        Section::new(z + tall * 0.72, 1.0),
                        Section::new(z + tall, 0.92),
                    ],
                )
            });
            if !b.coarse() {
                dark_plate(b);
                b.pattern(pattern::EMBER_SLOT);
                b.loft_z(
                    &p,
                    &[
                        Section::new(z + tall * 0.42, 1.004),
                        Section::new(z + tall * 0.55, 1.004),
                    ],
                );
            }
        });
        z += tall;
    }
    z
}

/// The tower's horns, its crown at `top`: two plates swept up and back off the crown, a
/// lit one swept out off each flank. Both sides.
fn horns(b: &mut MeshBuilder, (x, len, half): (f32, f32, f32), top: f32) {
    if b.coarse() {
        return;
    }
    let s = len / 280.0;
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(x + 40.0 * s, half * 0.25, top - 8.0 * s),
            v3(-1.0, 0.3, 0.2),
            v3(0.1, 0.35, 1.0),
        );
        shard(b, &f, (240.0 * s, 18.0 * s, 0.7), 5.0 * s, Finish::Dark);
        let g = Frame::new(
            v3(x + 10.0 * s, half * 0.6, top - 40.0 * s),
            v3(-1.0, 0.4, 0.05),
            v3(0.0, 1.0, 0.5),
        );
        shard(b, &g, (200.0 * s, 16.0 * s, 0.5), 4.0 * s, Finish::Lit);
    });
}
