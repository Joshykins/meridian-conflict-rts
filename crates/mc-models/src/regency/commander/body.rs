//! The body: a pelvis under thick tassets; a narrow waist of dark bands over a bronze core
//! and spine; the chest (`chest`); the back; high pauldrons swept back into blades; the
//! upper arms.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::plating::plate;
use super::form::ram;
use super::form::{ball, blade, ring, sleeve, ARCH, KEEL, OCT};
use super::{ELBOW, HIP, SHOULDER, WAIST};

/// A plan widest at the middle, pointed front and back: the pelvis and the waist's bands.
const HEX: [[f32; 2]; 6] = [
    [1.5, 0.0],
    [1.0, 1.0],
    [-1.0, 1.0],
    [-1.4, 0.0],
    [-1.0, -1.0],
    [1.0, -1.0],
];

/// The pelvis (hull): a block narrowing down between the thighs, bronze balls the thighs
/// hang from, the turntable, a plate pointed down the front, tassets over the hips.
pub(super) fn pelvis(b: &mut MeshBuilder) {
    seam(b);
    b.loft_z(
        &HEX,
        &[
            Section::scaled(10.4, 0.45, 0.5),
            Section::scaled(11.3, 0.9, 1.55),
            Section::scaled(12.3, 1.0, 1.8),
            Section::scaled(WAIST - 0.2, 0.9, 1.6),
        ],
    );
    metal(b);
    b.mirror_y(|b| ball(b, HIP, 0.95));
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, WAIST - 0.35), sides, 1.45, 1.4, 0.35);
    dark_plate(b);
    blade(
        b,
        v3(1.55, 0.0, 12.5),
        Vec3::new(0.2, 0.0, -1.0),
        Vec3::X,
        2.5,
        0.8,
        0.0,
        0.36,
    );
    b.mirror_y(|b| {
        let n = if b.fine() { 2 } else { 1 };
        for k in 0..n {
            let k = k as f32;
            blade(
                b,
                v3(0.95 - 1.25 * k, HIP.y + 0.35 + 0.12 * k, 12.75 - 0.1 * k),
                Vec3::new(-0.35, 0.32, -1.0),
                Vec3::new(0.0, 1.0, 0.3),
                2.9 - 0.3 * k,
                0.9,
                -0.6,
                0.34,
            );
        }
        blade(
            b,
            v3(-1.35, 0.75, 12.4),
            Vec3::new(-0.6, 0.15, -0.8),
            Vec3::new(-1.0, 0.0, 0.3),
            2.2,
            0.7,
            0.2,
            0.3,
        );
    });
}

/// Everything on the turret but the head and the forearms.
pub(super) fn torso(b: &mut MeshBuilder) {
    waist(b);
    super::chest::chest(b);
    back(b);
    b.mirror_y(shoulder);
}

/// The waist: a bronze core with a segmented spine behind it, three dark bands round it
/// widening upward with the core showing between them, and a ram either side.
fn waist(b: &mut MeshBuilder) {
    metal(b);
    b.loft_z(
        &HEX,
        &[
            Section::scaled(WAIST, 0.75, 0.95),
            Section::scaled(15.0, 0.85, 1.1),
        ],
    );
    let sides = b.sides(10);
    for k in 0..4 {
        let z = WAIST + 0.1 + 0.55 * k as f32;
        b.cylinder_between(
            v3(-1.35, 0.0, z),
            v3(-1.35, 0.0, z + 0.42),
            0.55,
            0.5,
            sides,
        );
    }
    dark_plate(b);
    for k in 0..3 {
        let z = WAIST + 0.25 + 0.62 * k as f32;
        let s = 1.0 + 0.12 * k as f32;
        let band = [
            Section::scaled(z, 0.95 * s, 1.1 * s),
            Section::scaled(z + 0.28, 1.0 * s, 1.18 * s),
            Section::scaled(z + 0.44, 0.95 * s, 1.12 * s),
        ];
        if b.fine() {
            b.loft_z(&HEX, &band);
        } else {
            b.loft_z(&HEX, &[band[0], band[2]]);
        }
    }
    if b.fine() {
        b.mirror_y(|b| ram(b, v3(-0.3, 1.25, WAIST + 0.1), v3(-0.1, 1.9, 15.4), 0.22));
    }
}

/// The back: plates lapped down it over the spine, and a heavy blade either side swept up
/// and back behind the shoulders.
fn back(b: &mut MeshBuilder) {
    dark_plate(b);
    b.mirror_y(|b| {
        let n = if b.fine() { 3 } else { 1 };
        for k in 0..n {
            let z = 20.0 - 1.45 * k as f32;
            blade(
                b,
                v3(-2.0 - 0.12 * k as f32, 0.2, z),
                Vec3::new(-0.35, 0.35, -1.0),
                Vec3::new(-1.0, 0.25, 0.0),
                2.1,
                1.0,
                0.5,
                0.32,
            );
        }
        blade(
            b,
            v3(-1.9, 1.9, 19.6),
            Vec3::new(-0.8, 0.3, 1.0),
            Vec3::new(-0.7, 0.7, 0.0),
            4.2,
            0.8,
            -0.5,
            0.34,
        );
    });
}

/// The left shoulder: a bronze ball, a pauldron arched over it and swept back into a
/// point, two blades rising up and back off its crown, the team colour on its top, and
/// the upper arm hung from the ball down to the elbow.
fn shoulder(b: &mut MeshBuilder) {
    let s = SHOULDER;
    metal(b);
    b.cylinder_between(v3(s.x, 2.4, s.z), s, 0.8, 0.8, b.sides(10));
    ball(b, s, 1.05);
    dark_plate(b);
    let c = |x: f32, dz: f32| v3(x, s.y + 0.2, s.z + dz);
    sleeve(
        b,
        &[
            ring(c(2.0, 0.1), Vec3::Z, 1.55, 1.55),
            ring(c(0.4, 0.2), Vec3::Z, 2.05, 2.0),
            ring(c(-1.6, 0.45), Vec3::Z, 1.8, 1.8),
            ring(c(-3.3, 1.2), Vec3::Z, 0.45, 0.55),
        ],
        &ARCH,
    );
    // The front cap, closing the pauldron's mouth, pointed down over the arm.
    blade(
        b,
        c(2.0, 1.55) + Vec3::Y * 0.1,
        Vec3::new(0.2, 0.45, -1.0),
        Vec3::new(1.0, 0.4, 0.0),
        3.0,
        1.05,
        0.75,
        0.34,
    );
    // Blades rising up and back off the crown, fanned outward.
    let fan: &[(f32, f32, f32, f32)] = if b.fine() {
        &[
            (1.0, 3.8, 0.4, 0.2),
            (-0.35, 3.3, 0.6, 0.45),
            (-1.6, 2.5, 0.85, 0.7),
        ]
    } else {
        &[(1.0, 3.8, 0.4, 0.2), (-0.35, 3.3, 0.6, 0.45)]
    };
    for &(x, len, lean, splay) in fan {
        // A solid spike, keeled outward, so it has breadth from any side.
        let root = v3(x, s.y + 0.45 + 0.3 * splay, s.z + 1.7);
        let dir = Vec3::new(-lean, splay, 1.0).normalize();
        let out = Vec3::new(0.35, 1.0, 0.0);
        sleeve(
            b,
            &[
                ring(root, out, 0.62, 0.42),
                ring(root + dir * (len * 0.5), out, 0.42, 0.3),
                ring(root + dir * len, out, 0.03, 0.03),
            ],
            &KEEL,
        );
    }
    b.paint(TEAM);
    plate(
        b,
        &[
            v3(1.9, s.y - 0.55, s.z + 1.62),
            v3(1.9, s.y + 0.2, s.z + 1.75),
            v3(0.9, s.y + 0.2, s.z + 2.2),
            v3(0.9, s.y - 0.55, s.z + 2.05),
        ],
        Vec3::new(0.3, 0.0, 1.0) * 0.1,
    );
    upper_arm(b);
}

/// The upper arm, from the shoulder ball down to the elbow: a faceted sleeve, a plate
/// down its outside swept back at the elbow into the elbow's blade, the elbow's drum.
fn upper_arm(b: &mut MeshBuilder) {
    let (s, e) = (SHOULDER, ELBOW);
    let at = |t: f32| s.lerp(e, t);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(at(0.18), Vec3::X, 0.85, 0.85),
            ring(at(0.5), Vec3::X, 0.98, 0.95),
            ring(at(0.86), Vec3::X, 0.78, 0.8),
        ],
        &OCT,
    );
    let up = (s - e).normalize();
    blade(
        b,
        e + Vec3::Y * 0.9 + Vec3::Z * 0.4 + Vec3::X * 0.5,
        up - Vec3::X * 0.12,
        Vec3::Y,
        3.4,
        0.85,
        0.4,
        0.28,
    );
    blade(
        b,
        e + Vec3::Y * 0.85 + Vec3::Z * 0.75,
        Vec3::new(-1.0, 0.0, -0.3),
        Vec3::Y,
        2.1,
        0.5,
        0.4,
        0.28,
    );
    metal(b);
    b.cylinder_between(e - Vec3::Y * 0.6, e + Vec3::Y * 0.6, 0.7, 0.7, b.sides(10));
}
