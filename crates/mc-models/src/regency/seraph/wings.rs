//! The wings: a plated housing on each shoulder blade, and fanned out of it seven
//! feathers of dark armour, each faced in overlapping plates (machined and plain in
//! turn, graphite ribs between), a spar down its leading edge and one red line down its
//! trailing edge, the light in the plate (`pattern::EMBER_SLOT`). Each feather's lower
//! half is in the deep shade (`pattern::EMBER_DEEP`). They change as the fan turns down: the crown, the highest, is a
//! broad compound blade with a spar along its leading edge and a vent at its root; under
//! it a notched blade, a
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
use super::super::machine::{armour, collar, swept, Frame};

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
            [0.0, -0.6],
            [0.12, -1.05],
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
    pts.iter().map(|&[s, t]| [s * len, t]).collect()
}

/// Where the outline crosses `s` along the feather: its nearest and farthest `t`.
fn span(outline: &[[f32; 2]], s: f32) -> (f32, f32) {
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for k in 0..outline.len() {
        let [a, c] = [outline[k], outline[(k + 1) % outline.len()]];
        if (a[0] - s) * (c[0] - s) <= 0.0 && (a[0] - c[0]).abs() > 1e-5 {
            let t = a[1] + (c[1] - a[1]) * (s - a[0]) / (c[0] - a[0]);
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    if lo > hi {
        (0.0, 0.0)
    } else {
        (lo, hi)
    }
}

/// The plates a feather is laid in along its face, from the root: where each starts and
/// ends, as shares of its length, a gap where they lap.
const COURSES: [(f32, f32); 4] = [(0.03, 0.3), (0.32, 0.56), (0.58, 0.8), (0.82, 0.97)];

/// One feather from `root` along `dir`: a thick dark plate turned to face back and a
/// little up, a knuckle where it leaves the hub, its faces laid in overlapping plates and
/// one unbroken lit line down its trailing edge.
fn feather(b: &mut MeshBuilder, root: Vec3, dir: Vec3, len: f32, cut: Cut) {
    let f = Frame::new(root, dir, -Vec3::X + Vec3::Z * 0.15);
    let shape = outline(cut, len);
    if b.fine() {
        dark_plate(b);
    } else {
        // Far off the lit edge is the plate's rim.
        b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
    }
    armour(b, &f, &shape, 0.28);
    if matches!(cut, Cut::Crown) {
        crown(b, &f, len);
    }
    if !b.fine() {
        return;
    }
    // The knuckle the feather turns on, out of the hub's drum.
    metal(b);
    collar(b, f.at(len * 0.02, 0.0, 0.14), f.u, 0.42, 0.5);
    let inset = |lo: f32, hi: f32, k: f32| {
        let m = (lo + hi) * 0.5;
        (m + (lo - m) * k, m + (hi - m) * k)
    };
    for n in [f.n, -f.n] {
        let lift = if n == f.n { 0.28 } else { 0.0 };
        let g = Frame::new(f.at(0.0, 0.0, lift), f.u, n);
        // Seen from the back the frame is mirrored: its `t` runs the other way.
        let side = if n == f.n { 1.0 } else { -1.0 };
        let flip = |pts: Vec<[f32; 2]>| -> Vec<[f32; 2]> {
            pts.iter().map(|&[s, t]| [s, t * side]).collect()
        };
        for (k, &(a, c)) in COURSES.iter().enumerate() {
            let last = k + 1 == COURSES.len();
            let (a, c) = (a * len, c * len);
            let (a0, a1) = inset(span(&shape, a).0, span(&shape, a).1, 0.86);
            let (c0, c1) = inset(span(&shape, c).0, span(&shape, c).1, 0.86);
            if a1 - a0 < 0.2 {
                continue;
            }
            // Split down its length: the upper half in plate, machined and plain in turn,
            // the lower half, where the light runs, in the deep shade.
            let finish = if k % 2 == 0 {
                pattern::EMBER_MACHINED
            } else {
                pattern::EMBER
            };
            let (am, cm) = ((a0 + a1) * 0.5, (c0 + c1) * 0.5);
            let (upper, lower) = if last {
                (
                    vec![[a, a0], [c, cm], [a, am]],
                    vec![[a, am], [c, cm], [a, a1]],
                )
            } else {
                (
                    vec![[a, a0], [c, c0], [c, cm], [a, am]],
                    vec![[a, am], [c, cm], [c, c1], [a, a1]],
                )
            };
            b.paint(PLATING_DARK).pattern(finish);
            armour(b, &g, &flip(upper), 0.1);
            b.paint(PLATING_DARK).pattern(pattern::EMBER_DEEP);
            armour(b, &g, &flip(lower), 0.1);
            if last {
                continue;
            }
            // A graphite rib across the gap to the next plate.
            metal(b);
            let (r0, r1) = inset(span(&shape, c).0, span(&shape, c).1, 0.8);
            armour(
                b,
                &Frame::new(g.at(c - 0.05, 0.0, 0.0), g.u, n),
                &flip(vec![[0.0, r0], [0.3, r0], [0.3, r1], [0.0, r1]]),
                0.14,
            );
        }
        // The spar: a raised edge down the leading side, over the plates.
        if !matches!(cut, Cut::Spike | Cut::Crown) {
            let (s0, s1) = (len * 0.05, len * 0.78);
            let t0 = inset(span(&shape, s0).0, span(&shape, s0).1, 0.86).0;
            let t1 = inset(span(&shape, s1).0, span(&shape, s1).1, 0.86).0;
            let p = g.at(s0, t0 * side, 0.1);
            let q = g.at(s1, t1 * side, 0.1);
            dark_plate(b);
            armour(
                b,
                &Frame::new(p, q - p, n),
                &swept(p.distance(q), 0.14, 0.0, 0.85),
                0.12,
            );
        }
        // The lit line down its trailing edge, over the plates.
        let (s0, s1) = (len * 0.04, len * 0.9);
        let t0 = inset(span(&shape, s0).0, span(&shape, s0).1, 0.86).1;
        let t1 = inset(span(&shape, s1).0, span(&shape, s1).1, 0.86).1;
        lit_line(b, &g, side, [s0, t0 - 0.2], [s1, t1 - 0.2]);
    }
}

/// A thin lit strip on a feather's face from `a` to `c` (`[s, t]` in `g`, `t` mirrored by
/// `side`): its light in the plate (`pattern::EMBER_SLOT`).
fn lit_line(b: &mut MeshBuilder, g: &Frame, side: f32, a: [f32; 2], c: [f32; 2]) {
    let p = g.at(a[0], a[1] * side, 0.1);
    let q = g.at(c[0], c[1] * side, 0.1);
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    armour(
        b,
        &Frame::new(p, q - p, g.n),
        &[
            [0.0, -0.1],
            [p.distance(q), -0.1],
            [p.distance(q), 0.1],
            [0.0, 0.1],
        ],
        0.04,
    );
}

/// The crown's own gear: a spar laid along its leading edge from the root and a vent let
/// into its root.
fn crown(b: &mut MeshBuilder, f: &Frame, len: f32) {
    dark_plate(b);
    let spar = Frame::new(f.at(0.0, -0.98, 0.18), f.u, f.n);
    armour(b, &spar, &swept(len * 0.84, 0.2, -0.6, 0.85), 0.24);
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    armour(
        b,
        &Frame::new(f.at(len * 0.12, 0.1, 0.2), f.u, f.n),
        &[
            [0.0, -0.32],
            [len * 0.22, -0.32],
            [len * 0.2, 0.32],
            [0.0, 0.32],
        ],
        0.1,
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
/// Where the `i`th feather from the top leaves the hub, and which way it runs.
fn feather_line(i: usize, deg: f32) -> (Vec3, Vec3) {
    let dir = ray(deg, 0.14 + 0.02 * i as f32);
    // Each lies behind the one above it, its root out on the drum's rim.
    (HUB + dir * 0.55 - Vec3::X * (0.12 + 0.2 * i as f32), dir)
}

/// The left wing's feather tips at rest, from the top (`Flight::tips`).
pub(super) fn tips() -> [[f32; 3]; flight::FEATHERS as usize] {
    std::array::from_fn(|i| {
        let (deg, len, _) = FAN[i];
        let (root, dir) = feather_line(i, deg);
        (root + dir * len).to_array()
    })
}

pub(super) fn wing(b: &mut MeshBuilder) {
    housing(b);
    for (i, &(deg, len, cut)) in FAN.iter().enumerate() {
        let (root, dir) = feather_line(i, deg);
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
