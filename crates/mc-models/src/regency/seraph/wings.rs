//! The wings: from a hinge on each shoulder blade, a plated binder carrying long feathers
//! of dark armour, each rimmed in red (`pattern::EMBER_RIM`), a narrower plate laid down
//! its middle. The design variants lay the feathers differently (`Wings`).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

use super::super::commander::form::{ball, ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::{armour, swept, Frame};
use super::Wings;

/// The left wing's hinge on the shoulder blade, outside the launch deck.
const ROOT: Vec3 = Vec3::new(-2.9, 2.75, 19.6);

/// A direction in the wing's plane: `deg` above level, out to the left, swept back by
/// `sweep` of a unit.
fn ray(deg: f32, sweep: f32) -> Vec3 {
    let a = deg.to_radians();
    v3(-sweep, a.cos(), a.sin()).normalize()
}

/// One feather from `root` along `dir`, `len` long and `half` either side, its broad face
/// turned back and a little up: the rimmed plate, and down its middle a narrower plate
/// standing off it (full detail only).
fn feather(b: &mut MeshBuilder, root: Vec3, dir: Vec3, len: f32, half: f32) {
    let face = -Vec3::X + Vec3::Z * 0.15;
    let f = Frame::new(root, dir, face);
    b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
    armour(b, &f, &swept(len, half, 0.0, 0.78), 0.2);
    if b.fine() && len > 3.0 {
        dark_plate(b);
        let g = Frame::new(f.at(len * 0.08, 0.0, 0.2), dir, f.n);
        armour(b, &g, &swept(len * 0.66, half * 0.42, 0.0, 0.7), 0.1);
        armour(
            b,
            &Frame::new(f.at(len * 0.08, 0.0, 0.0), dir, -f.n),
            &swept(len * 0.66, half * 0.42, 0.0, 0.7),
            0.1,
        );
    }
}

/// A short inner blade in graphite: the machinery the feathers spread from.
fn vane(b: &mut MeshBuilder, root: Vec3, dir: Vec3, len: f32, half: f32) {
    metal(b);
    armour(
        b,
        &Frame::new(root, dir, -Vec3::X),
        &swept(len, half, 0.0, 0.6),
        0.16,
    );
}

/// A plated binder from `a` to `c`, keeled along its back.
fn binder(b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32) {
    let out = -Vec3::X + Vec3::Z * 0.3;
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(a, out, w * 0.9, w),
            ring(a.lerp(c, 0.4), out, w, w * 1.05),
            ring(c, out, w * 0.55, w * 0.6),
        ],
        &KEEL,
    );
}

/// The hinge on the shoulder blade every wing turns out of.
fn hinge(b: &mut MeshBuilder) {
    metal(b);
    ball(b, ROOT, 0.55);
    b.cylinder_between(ROOT - v3(0.0, 0.85, 0.0), ROOT, 0.5, 0.6, b.sides(8));
}

/// The left wing (the right is its mirror).
pub(super) fn wing(b: &mut MeshBuilder, wings: Wings) {
    hinge(b);
    match wings {
        Wings::Freedom => freedom(b),
        Wings::Seraph => seraph(b),
        Wings::Halo => halo(b),
    }
}

/// A: the binder rises out to an elbow, the great upper feather runs on from it, five
/// more fan down off the elbow, and short graphite vanes fill the fan's root.
fn freedom(b: &mut MeshBuilder) {
    let elbow = v3(-3.9, 5.9, 23.6);
    binder(b, ROOT, elbow, 0.62);
    metal(b);
    ball(b, elbow, 0.38);
    for (deg, len, half) in [
        (40.0f32, 12.2, 0.72),
        (20.0, 12.6, 0.66),
        (2.0, 12.2, 0.62),
        (-16.0, 11.2, 0.6),
        (-33.0, 9.8, 0.56),
        (-50.0, 8.2, 0.52),
    ] {
        feather(b, elbow, ray(deg, 0.16), len, half);
    }
    for (deg, len) in [
        (55.0f32, 4.6),
        (28.0, 5.6),
        (5.0, 6.0),
        (-20.0, 5.6),
        (-42.0, 4.8),
    ] {
        vane(b, ROOT + v3(-0.4, 0.4, 0.2), ray(deg, 0.1), len, 0.34);
    }
}

/// B: three wings a side, upper, middle and lower, each a binder with feathers stepped
/// along it, longer toward its tip, laid like a bird's primaries.
fn seraph(b: &mut MeshBuilder) {
    // (binder's tip, feathers' direction at the root and at the tip, how many, lengths)
    let arms: [(Vec3, f32, f32, usize, f32, f32); 3] = [
        (v3(-4.2, 8.2, 26.6), 80.0, 40.0, 5, 5.0, 9.5),
        (v3(-4.4, 9.6, 20.4), 10.0, -8.0, 5, 4.2, 8.6),
        (v3(-3.8, 6.4, 15.6), -50.0, -70.0, 4, 3.6, 6.4),
    ];
    for (tip, d0, d1, count, l0, l1) in arms {
        binder(b, ROOT, tip, 0.48);
        for i in 0..count {
            let t = (i as f32 + 1.0) / count as f32;
            let root = ROOT.lerp(tip, 0.15 + 0.85 * t);
            let len = l0 + (l1 - l0) * t;
            feather(b, root, ray(d0 + (d1 - d0) * t, 0.2), len, 0.48);
        }
    }
}

/// C: seven long blades raying from a hub on the shoulder blade like a sunburst, longest
/// toward the top, each stepped out from the hub on a graphite spoke.
fn halo(b: &mut MeshBuilder) {
    let hub = ROOT + v3(-0.5, 0.4, 0.6);
    metal(b);
    sleeve(
        b,
        &[
            ring(hub - v3(0.0, 0.3, 0.0), -Vec3::X, 1.0, 1.0),
            ring(hub + v3(-0.5, 0.0, 0.0), -Vec3::X, 0.6, 0.6),
        ],
        &OCT,
    );
    for (deg, len) in [
        (96.0f32, 12.6),
        (74.0, 14.2),
        (52.0, 14.6),
        (31.0, 14.2),
        (11.0, 13.0),
        (-9.0, 11.4),
        (-29.0, 9.4),
    ] {
        let d = ray(deg, 0.12);
        let start = hub + d * 1.4;
        metal(b);
        b.cylinder_between(hub, start, 0.22, 0.16, b.sides(6));
        feather(b, start, d, len, 0.5);
    }
}

/// From far off: each wing two broad blades, seen from both sides.
pub(super) fn coarse(b: &mut MeshBuilder, wings: Wings) {
    dark_plate(b);
    let (top, out, low) = match wings {
        Wings::Freedom => (
            v3(-5.5, 13.0, 31.0),
            v3(-6.0, 18.0, 23.0),
            v3(-5.2, 10.5, 14.0),
        ),
        Wings::Seraph => (
            v3(-5.5, 11.0, 31.0),
            v3(-6.0, 17.5, 20.0),
            v3(-5.0, 8.5, 9.6),
        ),
        Wings::Halo => (
            v3(-5.0, 4.5, 33.0),
            v3(-5.5, 17.0, 23.0),
            v3(-5.0, 11.0, 14.0),
        ),
    };
    for (a, c) in [(top, out), (out, low)] {
        b.face(&[ROOT, a, c]);
        b.face(&[ROOT, c, a]);
    }
}
