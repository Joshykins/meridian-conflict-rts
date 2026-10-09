//! The wings: a plated housing on each shoulder blade, and fanned out of it seven
//! feathers of dark armour, each rimmed in red (`pattern::EMBER_RIM`). They change as the
//! fan turns down: the crown, the highest, is a broad compound blade with a spar along
//! its leading edge, a vent at its root and a forked tip; under it a notched blade, a
//! hooked one, a straight sword, a sickle, a guarded dagger and last a bare spike
//! pointing at the ground. Each lies a little behind the one above, so they overlap like
//! the feathers of a folded fan. Short graphite vanes fill the fan's root.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::flight;
use crate::material::*;
use crate::pattern;

use super::super::commander::form::{ring, sleeve, ARCH};
use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::{armour, swept, Frame};

/// The middle of the left fan's root, at the housing's outer face.
pub(super) const HUB: Vec3 = Vec3::new(-3.3, 3.55, 19.9);

/// A feather's cut, from the crown down.
#[derive(Clone, Copy)]
enum Cut {
    Crown,
    Notched,
    Hooked,
    Sword,
    Sickle,
    Dagger,
    Spike,
}

/// The left fan from the top: each feather's angle above level, its length and cut.
const FAN: [(f32, f32, Cut); 7] = [
    (46.0, 14.0, Cut::Crown),
    (26.0, 13.0, Cut::Notched),
    (8.0, 12.3, Cut::Hooked),
    (-10.0, 11.2, Cut::Sword),
    (-28.0, 9.8, Cut::Sickle),
    (-46.0, 8.4, Cut::Dagger),
    (-63.0, 6.8, Cut::Spike),
];

/// A direction in the wing's plane: `deg` above level, out to the left, swept back by
/// `sweep` of a unit.
fn ray(deg: f32, sweep: f32) -> Vec3 {
    let a = deg.to_radians();
    v3(-sweep, a.cos(), a.sin()).normalize()
}

/// A feather's outline (`s` along it from the root, `t` across, positive toward its
/// trailing edge, the side nearer the ground), `len` long.
fn outline(cut: Cut, len: f32) -> Vec<[f32; 2]> {
    let pts: &[[f32; 2]] = match cut {
        // Broad, stepped in a third of the way out on both edges, its tip swept up.
        Cut::Crown => &[
            [0.0, -0.25],
            [0.2, -1.05],
            [0.5, -1.1],
            [0.56, -0.78],
            [0.86, -0.66],
            [1.0, -0.2],
            [0.93, 0.22],
            [0.7, 0.56],
            [0.63, 0.92],
            [0.3, 1.15],
            [0.08, 1.0],
            [0.0, 0.6],
        ],
        // A step cut out of its leading edge near the tip.
        Cut::Notched => &[
            [0.0, -0.5],
            [0.6, -0.66],
            [0.65, -0.36],
            [0.94, -0.32],
            [1.0, 0.0],
            [0.82, 0.36],
            [0.2, 0.62],
            [0.0, 0.5],
        ],
        // Its point turned down toward the trailing edge, a hook behind it.
        Cut::Hooked => &[
            [0.0, -0.46],
            [0.76, -0.56],
            [1.0, -0.06],
            [0.96, 0.4],
            [0.86, 0.2],
            [0.22, 0.52],
            [0.0, 0.46],
        ],
        // Straight and even to a centred point.
        Cut::Sword => &[
            [0.0, -0.46],
            [0.8, -0.46],
            [1.0, 0.0],
            [0.8, 0.46],
            [0.0, 0.46],
        ],
        // Bowed down along its length.
        Cut::Sickle => &[
            [0.0, -0.4],
            [0.5, -0.46],
            [0.84, -0.16],
            [1.0, 0.36],
            [0.8, 0.18],
            [0.45, 0.36],
            [0.0, 0.4],
        ],
        // A guard near the root, then a long narrow blade.
        Cut::Dagger => &[
            [0.0, -0.3],
            [0.22, -0.3],
            [0.25, -0.46],
            [0.32, -0.28],
            [1.0, 0.0],
            [0.32, 0.28],
            [0.25, 0.46],
            [0.22, 0.3],
            [0.0, 0.3],
        ],
        Cut::Spike => &[[0.0, -0.26], [1.0, 0.0], [0.0, 0.26]],
    };
    // Broad blades, as the crown, narrowing down the fan to the spike.
    let wide = match cut {
        Cut::Crown => 1.55,
        Cut::Notched | Cut::Hooked => 1.4,
        Cut::Sword | Cut::Sickle => 1.3,
        Cut::Dagger => 1.25,
        Cut::Spike => 1.2,
    };
    pts.iter().map(|&[s, t]| [s * len, t * wide]).collect()
}

/// One feather from `root` along `dir`: its rimmed plate turned to face back and a little
/// up, and down its middle a narrower plate standing off each face (full detail only).
fn feather(b: &mut MeshBuilder, root: Vec3, dir: Vec3, len: f32, cut: Cut) {
    let f = Frame::new(root, dir, -Vec3::X + Vec3::Z * 0.15);
    b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
    armour(b, &f, &outline(cut, len), 0.2);
    if matches!(cut, Cut::Crown) {
        crown(b, &f, len);
    }
    if !b.fine() || matches!(cut, Cut::Spike) {
        return;
    }
    dark_plate(b);
    let half = match cut {
        Cut::Crown => 0.62,
        Cut::Notched | Cut::Hooked => 0.34,
        _ => 0.24,
    };
    for n in [f.n, -f.n] {
        let lift = if n == f.n { 0.2 } else { 0.0 };
        let g = Frame::new(f.at(len * 0.06, 0.0, lift), f.u, n);
        armour(b, &g, &swept(len * 0.6, half, 0.0, 0.72), 0.08);
    }
}

/// The crown's own gear: a spar laid along its leading edge from the root, a vent let
/// into its root, and a second, short blade forked up off its tip.
fn crown(b: &mut MeshBuilder, f: &Frame, len: f32) {
    dark_plate(b);
    let spar = Frame::new(f.at(len * 0.2, -1.52, 0.18), f.u, f.n);
    armour(b, &spar, &swept(len * 0.64, 0.26, -0.6, 0.85), 0.24);
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    armour(
        b,
        &Frame::new(f.at(len * 0.12, 0.2, 0.2), f.u, f.n),
        &[
            [0.0, -0.32],
            [len * 0.22, -0.32],
            [len * 0.2, 0.32],
            [0.0, 0.32],
        ],
        0.1,
    );
    // The fork: off the leading edge three quarters out, raked up past the tip.
    let up = (f.u - f.v * 0.32).normalize();
    b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
    armour(
        b,
        &Frame::new(f.at(len * 0.66, -1.08, -0.02), up, f.n),
        &swept(len * 0.42, 0.36, -0.4, 0.6),
        0.16,
    );
}

/// A short graphite vane: the machinery the feathers spread from.
fn vane(b: &mut MeshBuilder, root: Vec3, dir: Vec3, len: f32) {
    metal(b);
    armour(
        b,
        &Frame::new(root, dir, -Vec3::X),
        &swept(len, 0.32, 0.0, 0.6),
        0.14,
    );
}

/// The housing on the shoulder blade: an arched plate sleeve from the back out to the
/// fan's root, a graphite drum under it where the fan turns.
fn housing(b: &mut MeshBuilder) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-1.9, 1.7, 19.0), -Vec3::X + Vec3::Z * 0.4, 0.95, 0.8),
            ring(v3(-2.7, 2.7, 19.6), -Vec3::X + Vec3::Z * 0.4, 1.15, 0.95),
            ring(
                HUB + v3(-0.1, 0.1, 0.0),
                -Vec3::X + Vec3::Z * 0.4,
                0.9,
                0.75,
            ),
        ],
        &ARCH,
    );
    metal(b);
    b.cylinder_between(
        HUB + v3(0.45, 0.0, 0.0),
        HUB - v3(0.6, 0.0, 0.0),
        0.72,
        0.72,
        b.sides(10),
    );
}

/// The left wing (the right is its mirror).
pub(super) fn wing(b: &mut MeshBuilder) {
    housing(b);
    for (i, &(deg, len, cut)) in FAN.iter().enumerate() {
        let dir = ray(deg, 0.14 + 0.02 * i as f32);
        // Each lies behind the one above it, its root out on the drum's rim.
        let root = HUB + dir * 0.55 - Vec3::X * (0.12 + 0.34 * i as f32);
        b.with_flight(flight::FEATHER + i as u32, |b| {
            feather(b, root, dir, len, cut)
        });
    }
    b.with_flight(flight::VANE, |b| {
        for (deg, len) in VANES {
            vane(b, HUB + v3(0.3, 0.0, 0.0), ray(deg, 0.08), len);
        }
    });
}

/// The vanes at the fan's root: each one's angle above level and its length.
const VANES: [(f32, f32); 5] = [
    (36.0, 4.8),
    (16.0, 5.6),
    (-2.0, 5.8),
    (-20.0, 5.2),
    (-38.0, 4.4),
];

/// From far off: the wing two broad blades, seen from both sides, moving as its middle
/// feathers do.
pub(super) fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.with_flight(flight::VANE, coarse_blades);
}

fn coarse_blades(b: &mut MeshBuilder) {
    let (top, out, low) = (
        v3(-5.5, 12.8, 30.0),
        v3(-6.2, 18.0, 21.5),
        v3(-5.2, 7.0, 13.0),
    );
    for (a, c) in [(top, out), (out, low)] {
        b.face(&[HUB, a, c]);
        b.face(&[HUB, c, a]);
    }
}
