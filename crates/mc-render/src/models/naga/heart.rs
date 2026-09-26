//! The Heart, the Naga power plant, on its 2 x 2 lot (24 m square): a star core.
//!
//! Gravity pinches plasma into a small star, and the plant is the cage that holds it:
//!
//! - The star in the middle: a white-hot core wrapped in red flares (`GLOW_LAMP`,
//!   `GLOW_LASER`).
//! - The gravity cage round it: two fixed bronze rings crossed about it, and a third,
//!   level, turning (`part::SPINNER`) with its weights.
//! - Four armoured pylons on the diagonals, each on its own foot with plates lapped out
//!   toward the lot's corners, aim bronze pinch emitters at the star, their tips lit red,
//!   and carry a plated frame over it, open in the middle so the star shows from above.
//!   The owner's colour is on the pylons' heads.
//! - Coolant rams work in turn between the pylons (`part::PUMP`).
//!
//! Nothing on it can go off: a breached cage lets the star fall in on itself and go out
//! (the Heart has no death blast).

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::part;

use super::kit::{hide, metal, under_hide, v3};
use super::machine::*;

/// The star's middle (and the spinner's pivot) and its core's radius.
const STAR: Vec3 = Vec3::new(0.0, 0.0, 4.0);
const CORE_R: f32 = 1.35;
/// The pylons' bearings and how far out they stand, and their tops.
const PYLONS: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const PYLON_R: f32 = 5.2;
const TOP: f32 = 7.0;
const THICK: f32 = 0.4;

pub(super) fn heart(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(STAR);
    if b.coarse() {
        coarse(b);
        return;
    }
    star(b);
    cage(b);
    for deg in PYLONS {
        b.yawed(Vec3::ZERO, deg.to_radians(), pylon);
    }
    frame(b);
    rams(b);
}

/// Far off: the pylons as posts with the owner's colour on their heads, a plate out from
/// each foot, the star.
fn coarse(b: &mut MeshBuilder) {
    for deg in PYLONS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            hide(b);
            b.cylinder_between(
                v3(PYLON_R - 0.3, 0.0, 0.0),
                v3(PYLON_R - 0.3, 0.0, TOP),
                1.5,
                1.0,
                3,
            );
            b.face(&[
                v3(PYLON_R + 0.6, -1.4, 0.6),
                v3(PYLON_R + 5.0, 0.0, 0.2),
                v3(PYLON_R + 0.6, 1.4, 0.6),
            ]);
            b.paint(TEAM);
            b.face(&[
                v3(PYLON_R - 1.4, -0.8, TOP + 0.02),
                v3(PYLON_R + 0.2, -0.8, TOP + 0.02),
                v3(PYLON_R + 0.2, 0.8, TOP + 0.02),
                v3(PYLON_R - 1.4, 0.8, TOP + 0.02),
            ]);
        });
    }
    b.paint(GLOW_LAMP);
    b.cylinder_between(
        STAR - Vec3::Z * CORE_R,
        STAR + Vec3::Z * CORE_R,
        CORE_R,
        CORE_R * 0.6,
        3,
    );
}

/// The star: a white-hot core wrapped in red flares.
fn star(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(GLOW_LAMP);
    let sides = b.sides(12);
    b.spheroid(STAR, Vec3::splat(CORE_R), sides, if fine { 8 } else { 5 });
    b.paint(GLOW_LASER);
    let flares: &[Vec3] = if fine {
        &[
            v3(1.0, 0.0, 0.35),
            v3(-0.5, 0.87, 0.35),
            v3(-0.5, -0.87, 0.35),
        ]
    } else {
        &[v3(1.0, 0.0, 0.35)]
    };
    for &axis in flares {
        hoop_on(
            b,
            STAR,
            axis,
            CORE_R * 1.3,
            0.16,
            0.34,
            if fine { 16 } else { 8 },
        );
    }
}

/// The gravity cage: two fixed bronze rings crossed about the star, and a third, level,
/// turning with its weights.
fn cage(b: &mut MeshBuilder) {
    let fine = b.fine();
    let segs = if fine { 24 } else { 10 };
    metal(b);
    for tilt in [0.8f32, -0.8] {
        hoop_on(b, STAR, v3(0.0, tilt, 1.0), 2.8, 0.4, 0.5, segs);
    }
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(b, STAR, 3.5, 0.55, 0.55, segs);
        hide(b);
        for k in 0..4 {
            let a = (90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            b.cuboid(STAR + d * 3.5, v3(1.0, 1.0, 1.0));
        }
    });
}

/// One pylon, standing out along +x (turned into place by the caller): its foot with
/// plates lapped out toward the lot's corner, an armoured post, a bronze pinch emitter
/// aimed at the star with its tip lit red, the owner's colour on its head.
fn pylon(b: &mut MeshBuilder) {
    let fine = b.fine();
    let x = PYLON_R;
    under_hide(b);
    b.frustum_open(
        v3(x + 0.4, 0.0, 0.0),
        Vec2::new(3.6, 3.0),
        Vec2::new(3.0, 2.6),
        0.8,
        Vec2::ZERO,
    );
    let f = Frame::new(v3(x + 1.4, 0.0, 0.9), v3(1.0, 0.0, -0.18), Vec3::Z);
    hide(b);
    Course {
        count: if fine { 2 } else { 1 },
        step: 1.6,
        len: 2.6,
        half: 1.5,
        tip: 0.0,
        thick: THICK,
        tail: 0.5,
    }
    .lay(b, &f);
    hide(b);
    b.frustum_open(
        v3(x, 0.0, 0.8),
        Vec2::new(2.2, 2.0),
        Vec2::new(1.5, 1.5),
        TOP - 0.8,
        Vec2::new(-0.5, 0.0),
    );
    let f = Frame::new(
        v3(x + 0.35, 0.0, TOP - 0.3),
        v3(0.3, 0.0, -1.0),
        v3(1.0, 0.0, 0.3),
    );
    hide(b);
    Course {
        count: 2,
        step: 2.0,
        len: 3.0,
        half: 1.3,
        tip: 0.0,
        thick: THICK,
        tail: 0.8,
    }
    .lay(b, &f);
    b.paint(TEAM);
    b.face(&[
        v3(x - 1.1, -0.6, TOP + 0.02),
        v3(x + 0.1, -0.6, TOP + 0.02),
        v3(x + 0.1, 0.6, TOP + 0.02),
        v3(x - 1.1, 0.6, TOP + 0.02),
    ]);
    // The pinch emitter: a bronze barrel from the post toward the star, its tip red.
    let from = v3(x - 0.8, 0.0, STAR.z);
    let to = v3(CORE_R + 1.0, 0.0, STAR.z);
    collar(b, from, Vec3::X, 0.6, 0.8);
    metal(b);
    b.cylinder_between(from, to, 0.3, 0.22, if fine { 8 } else { 6 });
    b.paint(GLOW_LASER);
    b.cylinder_between(to, to - Vec3::X * 0.25, 0.24, 0.1, 6);
    if fine {
        red_slot(b, v3(x + 0.62, 0.0, 2.4), Vec3::X, Vec3::Z, 1.4, 0.18);
    }
}

/// The plated frame over the cage: a beam from each pylon's head to the next, open in
/// the middle so the star shows from above.
fn frame(b: &mut MeshBuilder) {
    hide(b);
    let at = |a: f32| v3(a.cos(), a.sin(), 0.0) * (PYLON_R - 0.6) + Vec3::Z * (TOP - 0.5);
    for k in 0..4 {
        let a0 = (45.0 + 90.0 * k as f32).to_radians();
        let a1 = (135.0 + 90.0 * k as f32).to_radians();
        b.beam(at(a0), at(a1), Vec2::new(0.9, 0.8), Vec2::new(0.9, 0.8));
    }
}

/// The coolant rams between the pylons, each on a block, working in turn.
fn rams(b: &mut MeshBuilder) {
    for k in 0..4 {
        let a = (90.0 * k as f32).to_radians();
        let at = v3(a.cos(), a.sin(), 0.0) * 6.4;
        hide(b);
        b.block(at - v3(0.9, 0.9, 0.0), at + v3(0.9, 0.9, 0.8));
        piston(b, at + Vec3::Z * 0.8, at + Vec3::Z * 4.0, 0.4, true);
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_scaled, part};

    #[test]
    fn heart() {
        super::super::check("naga_heart", 6.9, 7.5, Some(2), &[]);
        let model = build_model_scaled("naga_heart", 6.9, 7.5, 1).unwrap();
        for kind in [part::PUMP, part::SPINNER] {
            assert!(
                model.lods[0].vertices.iter().any(|v| v.part == kind),
                "no part {kind}"
            );
        }
    }
}
