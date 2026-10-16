//! The blades under the arms: the Regent's keel spikes brought to the Scourge, one under
//! each arm. Each is a faceted armoured housing let into the arm's underside, plates swept
//! back down its flanks and a lit slot in its brow; out of it a thick tapering prism raked
//! hard aft and canted out, ribbed down its faces, two collars round it, its tip cut off on
//! a slant like a chisel. Raked so its tip stays above the ship's keel line (the hull is
//! built on its lot, nothing under z 0).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::Frame;
use super::super::style::{shard, Finish};
use super::wings::Wing;

/// A blade under the port arm: how it hangs.
#[derive(Clone, Copy)]
pub(super) struct Blade {
    /// Its housing's middle out along the arm, and how far back along the arm's chord
    /// (0 its leading edge, 1 its trailing).
    pub(super) y: f32,
    pub(super) chord: f32,
    /// How far it rakes aft of straight down, and cants out, radians.
    pub(super) rake: f32,
    pub(super) cant: f32,
    /// The housing's size against the Regent's (a 300 m housing), and the prism's length
    /// from it to its tip, metres.
    pub(super) size: f32,
    pub(super) len: f32,
    /// The prism's girth against the housing's.
    pub(super) girth: f32,
}

/// The prism's sections along it from the housing to the tip: how far along (0 to 1), its
/// chord fore and aft, its width, at the Regent's size.
const PRISM: [(f32, f32, f32); 5] = [
    (0.0, 190.0, 90.0),
    (0.2, 170.0, 80.0),
    (0.55, 130.0, 62.0),
    (0.85, 95.0, 46.0),
    (1.0, 80.0, 38.0),
];
/// How far the chisel tip's cut rises from its trailing edge to its leading edge, as a
/// share of the tip's chord.
const CHISEL: f32 = 0.42;

/// The port blade under `wing`, both sides drawn by the caller's mirror.
pub(super) fn blade(b: &mut MeshBuilder, wing: &Wing, s: Blade) {
    if b.coarse() {
        return;
    }
    let rib = wing.rib_at(s.y);
    let x = rib.le + (rib.te - rib.le) * s.chord;
    // The housing's top sinks into the arm's underside.
    let under = rib.underside() + 4.0 * s.size;
    b.at(v3(x, s.y, under), |b| {
        housing(b, s.size);
        prism(b, s);
    });
}

/// The housing about the arm's underside at its middle: the Regent's, at `k` its size.
fn housing(b: &mut MeshBuilder, k: f32) {
    let plan: Vec<[f32; 2]> = [
        [150.0, 0.0],
        [100.0, 70.0],
        [-110.0, 66.0],
        [-150.0, 30.0],
        [-150.0, -30.0],
        [-110.0, -66.0],
        [100.0, -70.0],
    ]
    .iter()
    .map(|&[x, y]| [x * k, y * k])
    .collect();
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(-120.0 * k, 0.72),
                Section::new(-60.0 * k, 0.95),
                Section::new(20.0 * k, 1.0),
            ],
        )
    });
    for side in [1.0f32, -1.0] {
        let f = Frame::new(
            v3(110.0 * k, side * 64.0 * k, -30.0 * k),
            v3(-1.0, side * 0.08, -0.25),
            v3(0.0, side, -0.3),
        );
        shard(b, &f, (260.0 * k, 34.0 * k, 0.4), 7.0 * k, Finish::Dark);
    }
    let f = Frame::new(v3(140.0 * k, 0.0, -50.0 * k), v3(-0.4, 0.0, -1.0), Vec3::X);
    shard(b, &f, (60.0 * k, 22.0 * k, 0.0), 3.0 * k, Finish::Lit);
}

/// The prism out of the housing, perpendicular sections down its axis.
fn prism(b: &mut MeshBuilder, s: Blade) {
    let k = s.size * s.girth;
    let origin = v3(-20.0 * s.size, 0.0, -100.0 * s.size);
    let axis = v3(-s.rake.sin(), s.cant.sin(), -s.rake.cos()).normalize();
    let chord = (Vec3::X - axis * Vec3::X.dot(axis)).normalize();
    let across = axis.cross(chord);
    let ring = |t: f32, c: f32, w: f32, grow: f32| -> Vec<Vec3> {
        let mid = origin + axis * (t * s.len);
        let (c, w) = (c * k * grow, w * k * grow);
        [
            (0.5, 0.0),
            (0.25, 0.5),
            (-0.35, 0.5),
            (-0.5, 0.0),
            (-0.35, -0.5),
            (0.25, -0.5),
        ]
        .iter()
        .map(|&(u, v)| {
            let p = mid + chord * (u * c) + across * (v * w);
            if t >= 1.0 {
                // The chisel: the tip's cut rises toward its leading edge.
                p - axis * (CHISEL * c * (u + 0.5))
            } else {
                p
            }
        })
        .collect()
    };
    let rings: Vec<Vec<Vec3>> = PRISM.iter().map(|&(t, c, w)| ring(t, c, w, 1.0)).collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    // Collars round it.
    for &(t, c, w) in &PRISM[1..3] {
        let band = [ring(t, c, w, 1.08), ring(t - 0.05, c, w, 1.08)];
        dark_plate(b);
        b.with_facets(|b| b.loft(&band, true, true));
    }
    // Ridges down its faces, root to tip.
    if b.fine() {
        metal(b);
        let r = 4.0 * k;
        for corner in [1usize, 2, 4, 5] {
            for w in rings.windows(2) {
                b.beam(
                    w[0][corner],
                    w[1][corner],
                    Vec2::splat(r),
                    Vec2::splat(r * 0.75),
                );
            }
        }
    }
}

/// The blade: out under the arm's corner, long and lean, raked hard aft.
pub(super) const BLADE: Blade = Blade {
    y: 185.0,
    chord: 0.35,
    rake: 1.35,
    cant: 0.3,
    size: 0.16,
    len: 105.0,
    girth: 0.8,
};
