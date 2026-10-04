//! The Breacher's body and arms: a hunched torso thrown forward over the hips, a dark core
//! under lapped faceted plates with thin seams of light between some of them, a heavy brow
//! over the head, the hump the launchers sit in; and each arm, carried in its gun's house,
//! a layered pauldron over the shoulder drum and an upper arm down to the elbow on the
//! gun's roof (the gun is the forearm).

use glam::Vec3;

use super::super::limbs::*;
use super::super::parts::*;
use super::{gun, GUN};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

const Y: Vec3 = Vec3::Y;

/// The shoulder joint, either side of the torso, at the pivot's height.
const SHOULDER: Vec3 = Vec3::new(2.0, 15.0, 40.0);

/// A faceted slab: the quad `q` (listed round its edge) lofted `t` out along `out`.
fn slab(b: &mut MeshBuilder, q: [Vec3; 4], out: Vec3, t: f32) {
    let out = out.normalize() * t;
    b.loft(
        &[q.to_vec(), q.iter().map(|&p| p + out).collect()],
        true,
        true,
    );
}

/// A thin seam of light from `a` to `c`, up close only. Leaves the brush on plating.
fn seam(b: &mut MeshBuilder, a: Vec3, c: Vec3) {
    if b.fine() {
        b.paint(GLOW).pattern(pattern::PLAIN);
        b.beam(a, c, v2(0.14, 0.14), v2(0.14, 0.14));
    }
    b.paint(PLATING);
}

/// The pelvis, and over the waist the torso, its plates, the brow and the hump.
pub(super) fn body(b: &mut MeshBuilder) {
    b.with_part(part::HULL, |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.chamfered_box(v3(-3.0, 0.0, 22.0), v3(10.0, 15.0, 7.0), 2.4);
    });
    b.with_part(part::TURRET, torso);
}

fn torso(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(-1.0, 0.0, 24.0), b.sides(14), 7.2, 6.8, 2.0);
    // The core: narrow over the hips, swelling forward and up into the shoulders.
    b.loft_z(
        &chamfered_rect(v2(8.6, 10.6), 3.4),
        &[
            Section::scaled(25.5, 0.68, 0.7).shifted(-2.0, 0.0),
            Section::scaled(31.0, 0.95, 0.95),
            Section::scaled(38.0, 1.12, 1.26).shifted(3.0, 0.0),
            Section::scaled(44.0, 1.02, 1.22).shifted(2.0, 0.0),
            Section::scaled(48.0, 0.7, 0.78).shifted(-2.0, 0.0),
        ],
    );
    // The brow: a heavy faceted plate jutting forward over the head, its crown ridged.
    b.paint(PLATING);
    b.extrude_y_chamfered(
        &[
            [5.0, 38.0],
            [15.6, 37.4],
            [16.6, 39.2],
            [12.4, 44.4],
            [3.0, 45.6],
        ],
        9.4,
        2.0,
    );
    b.extrude_y(
        &[[3.0, 45.5], [12.0, 44.4], [10.0, 46.0], [2.0, 46.8]],
        -2.2,
        2.2,
    );
    seam(b, v3(15.9, -7.4, 37.25), v3(15.9, 7.4, 37.25));
    b.mirror_y(|b| {
        // A pectoral slab either side of the head, raked back under the brow.
        b.paint(PLATING);
        slab(
            b,
            [
                v3(9.4, 4.6, 30.6),
                v3(8.8, 11.4, 31.2),
                v3(12.6, 13.0, 36.8),
                v3(13.8, 4.6, 36.8),
            ],
            v3(1.0, 0.15, -0.55),
            1.1,
        );
        seam(b, v3(13.0, 4.5, 37.3), v3(12.2, 12.6, 37.3));
        // A flank plate down the side of the shoulders, lapped over a lower one.
        slab(
            b,
            [
                v3(-5.0, 13.6, 37.0),
                v3(8.0, 14.6, 36.6),
                v3(9.0, 13.6, 44.6),
                v3(-6.0, 12.8, 45.0),
            ],
            Y,
            1.0,
        );
        slab(
            b,
            [
                v3(-6.0, 10.2, 29.0),
                v3(6.0, 10.6, 28.6),
                v3(7.4, 13.4, 35.6),
                v3(-5.0, 12.6, 36.2),
            ],
            v3(0.0, 1.0, -0.3),
            0.9,
        );
        seam(b, v3(-4.6, 13.5, 36.6), v3(7.6, 14.4, 36.2));
        // The hump's side plates, stepped, round the launcher wells.
        slab(
            b,
            [
                v3(-15.4, 10.6, 37.0),
                v3(-5.0, 11.0, 37.0),
                v3(-3.6, 10.6, 45.6),
                v3(-12.6, 10.2, 43.6),
            ],
            Y,
            1.0,
        );
        seam(b, v3(-14.8, 11.7, 37.4), v3(-5.4, 12.1, 37.4));
    });
    // The belly plate under the chest.
    b.paint(PLATING);
    b.extrude_y_chamfered(
        &[
            [4.0, 26.0],
            [9.4, 27.0],
            [11.0, 30.0],
            [8.0, 30.6],
            [4.0, 30.2],
        ],
        6.4,
        1.4,
    );
    // The hump: an armoured back the launchers sit in, sloping down to the tail.
    b.extrude_y_chamfered(
        &[
            [-6.0, 38.0],
            [-4.0, 46.0],
            [-7.0, 49.0],
            [-15.0, 44.0],
            [-16.4, 38.0],
            [-12.0, 33.0],
        ],
        10.4,
        2.0,
    );
    team_panel(b, v3(0.5, 0.0, 46.8), v2(3.0, 3.6));
    if fine {
        b.paint(PLATING_DARK);
        for k in 0..4 {
            let z = 35.0 + 1.8 * k as f32;
            b.beam(
                v3(-16.6, -7.0, z),
                v3(-16.6, 7.0, z),
                v2(0.5, 0.35),
                v2(0.5, 0.35),
            );
        }
    }
}

/// An arm, carried in its gun house: the shoulder drum, a layered pauldron over it, the
/// upper arm down and forward to the elbow on the gun's roof.
pub(super) fn arm(b: &mut MeshBuilder, side: f32) {
    let fine = b.fine();
    let s = SHOULDER * v3(1.0, side, 1.0);
    let g = GUN * v3(1.0, side, 1.0);
    let elbow = g + v3(-2.0, 0.0, 6.4) * gun::SCALE;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(
        s - Y * (side * 4.0),
        s + Y * (side * 3.0),
        4.2,
        4.2,
        b.sides(14),
    );
    // The pauldron: a faceted hood over the shoulder, a second plate lapped over its
    // crown, a seam of light between them.
    b.paint(PLATING);
    b.at(s + v3(1.0, side * 1.0, 1.2), |b| {
        b.loft(
            &[
                x_ring(-6.6, 0.0, 0.0, 4.4, 4.4, 2.0),
                x_ring(-4.8, 0.0, 0.4, 5.6, 5.4, 2.6),
                x_ring(4.6, 0.0, 0.4, 5.6, 5.4, 2.6),
                x_ring(7.2, 0.0, -0.8, 4.2, 4.0, 2.0),
            ],
            true,
            true,
        );
        slab(
            b,
            [
                v3(-4.0, -4.4, 5.8),
                v3(5.0, -4.4, 5.8),
                v3(4.0, 4.4, 5.2),
                v3(-3.0, 4.4, 5.2),
            ],
            Vec3::Z,
            0.9,
        );
        seam(b, v3(-3.6, side * 5.65, 3.0), v3(4.6, side * 5.65, 3.0));
    });
    // The upper arm.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    limb(
        b,
        s + v3(0.0, side * 1.5, -2.0),
        elbow,
        &[(0.0, 3.0, 2.8, 2.8, 1.0), (1.0, 2.4, 2.2, 2.2, 0.9)],
    );
    b.paint(PLATING);
    armour(
        b,
        s + v3(1.0, side * 4.0, -4.0),
        elbow + v3(0.0, side * 2.6, 0.4),
        Y * side,
        (3.0, 1.2),
        (2.4, 1.0),
    );
    if fine {
        ram(
            b,
            s + v3(-3.0, 0.0, -3.0),
            elbow + v3(-3.4, 0.0, -0.4),
            0.7,
            0.4,
        );
    }
    joint(b, elbow, 2.4, 1.8);
}
