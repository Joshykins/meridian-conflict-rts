//! The Fathom, a tech 3 torpedo-launcher installation on the seabed
//! (`aster_t3_torpedo_defense`): torpedo batteries round a hardened body, and a
//! spire from it to the surface (`gpu_consts::spire`).
//!
//! The origin is on the sea floor. The body stands below `spire::BASE`; what is authored
//! from there to `spire::TOP` is stretched up to the surface however deep the water is
//! (two to six times and more), so the spire is long plain members and thin bands only,
//! and every horizontal detail sits below `BASE` or above `TOP` (the small float that
//! rides the surface). Four batteries of two torpedo tubes, one on each face, fire
//! heavy torpedoes and interceptors up and out at 45 degrees from the muzzles in
//! `naval.ron` ((6, ±3, 7) turned by quarter turns). Nothing is lit but the float's
//! obstruction lamp.

use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4};

use glam::Vec2;

use super::*;
use crate::gpu_consts::spire::{BASE, TOP};

/// The +x battery's two muzzles (the others are it turned by quarter turns).
const MUZZLES: [Vec3; 2] = [Vec3::new(6.0, -3.0, 7.0), Vec3::new(6.0, 3.0, 7.0)];
/// Up and out: the way every torpedo leaves its tube, on the +x battery.
const FIRE: Vec3 = Vec3::new(FRAC_1_SQRT_2, 0.0, FRAC_1_SQRT_2);

/// Runs `f` once per battery, in a frame turned so that battery faces +x.
fn batteries(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    b.radial(4, f);
}

/// Runs `f` once per diagonal, in a frame turned so the diagonal lies along +x.
fn diagonals(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    b.radial(4, |b| b.yawed(Vec3::ZERO, FRAC_PI_4, |b| f(b)));
}

/// A round torpedo tube along `FIRE` ending at `muzzle`: a gunmetal barrel, a
/// heavy collar at the mouth and the dark bore.
fn round_tube(b: &mut MeshBuilder, muzzle: Vec3, length: f32, r: f32) {
    let sides = b.sides(10);
    b.paint(METAL);
    b.cylinder_between(muzzle - FIRE * length, muzzle - FIRE * 0.3, r, r, sides);
    b.paint(PLATING);
    b.cylinder_between(muzzle - FIRE * 0.9, muzzle, r * 1.3, r * 1.22, sides);
    b.paint(ACCENT);
    b.cylinder_between(
        muzzle - FIRE * 0.12,
        muzzle + FIRE * 0.02,
        r * 0.86,
        r * 0.86,
        sides,
    );
    if b.fine() {
        b.paint(ACCENT);
        for t in [1.05, 1.6] {
            b.cylinder_between(
                muzzle - FIRE * t,
                muzzle - FIRE * (t + 0.12),
                r * 1.14,
                r * 1.14,
                sides,
            );
        }
    }
}

// ---- A: the bastion ------------------------------------------------------------

/// The bastion: a faceted armoured drum pinned to the rock by four raked pile
/// legs, a raked twin-tube casemate on each face, and one riser mast held by four guys.
pub(in crate::aster) fn build(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        a_coarse(b);
        return;
    }
    diagonals(b, a_leg);
    a_citadel(b);
    batteries(b, a_casemate);
    a_spire(b);
    a_float(b);
}

/// The drum's plan: an octagon, a flat face toward each battery.
fn a_plan() -> Vec<[f32; 2]> {
    ngon(8, 7.8)
}

/// Far off: the drum, the casemates as a cross, the mast, the pile pads, the owner's colour.
fn a_coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.frustum_open(Vec3::ZERO, v2(14.4, 14.4), v2(8.0, 8.0), 8.6, Vec2::ZERO);
    b.cuboid_open(v3(0.0, 0.0, 4.5), v3(15.2, 7.6, 7.0));
    b.cuboid_open(v3(0.0, 0.0, 4.5), v3(7.6, 15.2, 7.0));
    b.frustum_open(
        v3(0.0, 0.0, 8.6),
        v2(2.4, 2.4),
        v2(1.6, 1.6),
        TOP + 1.0 - 8.6,
        Vec2::ZERO,
    );
    b.paint(ACCENT);
    for s in [1.0, -1.0] {
        let (a, e) = (v3(-11.5, -11.5 * s, 0.4), v3(11.5, 11.5 * s, 0.4));
        let side = Vec3::Z.cross(e - a).normalize() * 1.8;
        b.face(&[a - side, e - side, e + side, a + side]);
    }
    b.paint(TEAM);
    b.decal(v3(0.0, 0.0, 8.65), v2(3.6, 3.6));
}

/// One pile leg on a diagonal (+x here): a raked strut from the drum to a pile cap on a
/// mud mat, the piles' heads round it, the guy's padeye on top, the owner's disc.
fn a_leg(b: &mut MeshBuilder) {
    let foot = 16.0;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.plate(v3(foot, 0.0, 0.0), v2(4.6, 4.6), 0.35, 0.12);
    b.prism(v3(foot, 0.0, 0.3), b.sides(8), 2.1, 1.8, 1.3);
    b.paint(PLATING);
    b.prism(v3(foot, 0.0, 1.6), b.sides(8), 1.6, 1.35, 0.35);
    team_panel(b, v3(foot + 0.4, 0.0, 1.95), v2(1.2, 1.5));
    b.paint(PLATING);
    b.beam(
        v3(5.6, 0.0, 6.2),
        v3(foot - 1.2, 0.0, 1.5),
        v2(1.5, 1.3),
        v2(1.1, 0.9),
    );
    b.paint(METAL);
    b.cuboid(v3(foot - 0.9, 0.0, 2.2), v3(0.5, 0.35, 0.6));
    if b.fine() {
        // The piles' heads, driven through the mat, and a stiffener under the strut.
        b.paint(METAL);
        for k in 0..4 {
            let a = FRAC_PI_4 + FRAC_PI_2 * k as f32;
            let at = v3(foot + 1.75 * a.cos(), 1.75 * a.sin(), 0.3);
            b.cylinder_between(at, at + Vec3::Z * 0.55, 0.28, 0.24, 6);
        }
        b.paint(ACCENT);
        b.beam(
            v3(7.2, 0.0, 1.0),
            v3(foot - 2.0, 0.0, 0.9),
            v2(0.5, 0.6),
            v2(0.5, 0.6),
        );
        b.beam(
            v3(10.5, 0.0, 0.95),
            v3(10.5, 0.0, 3.6),
            v2(0.45, 0.4),
            v2(0.45, 0.4),
        );
    }
}

/// The drum: a dark foot, sheer armoured sides, sloped shoulders and a crown under the
/// mast; flush hydrophone arrays on the four diagonal faces between the batteries.
fn a_citadel(b: &mut MeshBuilder) {
    let plan = a_plan();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::new(0.0, 1.02), Section::new(1.1, 1.0)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(1.0, 0.97),
            Section::new(5.6, 0.93),
            Section::new(7.6, 0.62),
            Section::new(8.6, 0.42),
        ],
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(0.0, 0.0, 8.5), b.sides(12), 2.6, 2.2, BASE - 8.5);
    // The arrays: dark acoustic panels, ribbed at full detail.
    diagonals(b, |b| {
        b.paint(PLATING_DARK);
        b.block(v3(6.2, -2.0, 1.6), v3(6.95, 2.0, 4.6));
        if b.fine() {
            b.paint(ACCENT);
            for y in [-1.3, 0.0, 1.3] {
                b.block(v3(6.9, y - 0.08, 1.5), v3(7.05, y + 0.08, 4.7));
            }
        }
    });
    if b.fine() {
        // A trim at the shoulder, and a hatch on each diagonal shoulder.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(&plan, &[Section::new(5.45, 0.935), Section::new(5.7, 0.93)]);
        diagonals(b, |b| {
            b.paint(METAL);
            b.pitched(v3(4.9, 0.0, 7.1), -0.98, |b| {
                b.cylinder_between(Vec3::ZERO, v3(0.0, 0.0, 0.25), 0.8, 0.7, 10);
            });
        });
    }
}

/// One battery (+x here): a raked casemate on the drum's face, its sloped front square
/// to the tubes, the two tube mouths through it, a hatch and the owner's strip on its roof.
fn a_casemate(b: &mut MeshBuilder) {
    // The sloped front stands back from the muzzles, so the tubes' collars stand proud.
    let profile = [[3.8, 1.0], [7.6, 1.0], [7.6, 4.2], [4.2, 7.6], [3.8, 7.6]];
    b.paint(PLATING);
    b.extrude_y(&profile, -3.9, 3.9);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(7.5, -3.95, 1.0), v3(7.7, 3.95, 2.0));
    for m in MUZZLES {
        round_tube(b, m, 3.4, 0.55);
    }
    if b.fine() {
        // A reload hatch in the casemate's front: a dark door in a frame.
        b.paint(ACCENT);
        b.block(v3(7.58, -2.2, 1.9), v3(7.72, 2.2, 3.9));
        b.paint(PLATING_DARK);
        b.block(v3(7.7, -1.9, 2.1), v3(7.8, 1.9, 3.7));
    }
}

/// The riser mast: a tapered tube to the surface, three conduits stood off it, thin
/// clamp bands, and four guys from the pile caps to a collar two thirds up.
fn a_spire(b: &mut MeshBuilder) {
    let sides = b.sides(10);
    let (r0, r1) = (1.3, 0.8);
    let r_at = |z: f32| r0 + (r1 - r0) * (z - BASE) / (TOP - BASE);
    b.paint(PLATING);
    b.loft_z(
        &ngon(sides, 1.0),
        &[Section::new(BASE, r0), Section::new(TOP, r1)],
    );
    // The conduits.
    b.paint(METAL);
    for k in 0..3 {
        let a = FRAC_PI_2 / 3.0 + (std::f32::consts::TAU / 3.0) * k as f32;
        let dir = v3(a.cos(), a.sin(), 0.0);
        b.cylinder_between(
            dir * (r0 + 0.3) + Vec3::Z * (BASE - 0.4),
            dir * (r1 + 0.28) + Vec3::Z * TOP,
            0.16,
            0.14,
            b.sides(6),
        );
    }
    // Clamp bands, thin enough to stay bands when stretched.
    let guy_z = BASE + (TOP - BASE) * 0.62;
    b.paint(ACCENT);
    let bands: &[f32] = if b.fine() {
        &[0.25, 0.45, 0.62, 0.8]
    } else {
        &[0.45, 0.62]
    };
    for t in bands {
        let z = BASE + (TOP - BASE) * t;
        let r = r_at(z) + 0.52;
        b.prism(v3(0.0, 0.0, z - 0.12), sides, r, r, 0.24);
    }
    // The guys, from each pile cap's padeye to the collar.
    b.paint(ACCENT);
    diagonals(b, |b| {
        b.beam(
            v3(15.1, 0.0, 2.4),
            v3(r_at(guy_z) + 0.45, 0.0, guy_z),
            v2(0.14, 0.14),
            v2(0.14, 0.14),
        );
    });
}

/// The float at the top: a short drum riding the surface, a dark rim, the lamp.
fn a_float(b: &mut MeshBuilder) {
    let sides = b.sides(10);
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, TOP), sides, 1.25, 1.1, 0.7);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, TOP + 0.3), sides, 1.32, 1.32, 0.16);
    beacon(b, v3(0.0, 0.0, TOP + 0.7));
}
