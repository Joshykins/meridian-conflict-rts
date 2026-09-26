//! The Eye, the Naga radar, on its 2 x 2 lot (24 m square, cut to an octagon): a sensor
//! mast crowned with rings held floating by gravity. It upgrades in place (tech 2 and 3),
//! each tier adding a ring and heavier machinery.
//!
//! - Four plated legs on the diagonals brace a ribbed bronze mast, each leg's plates
//!   lapped down into a spike, a foot block where it meets the lot.
//! - The mast climbs in three armoured sleeves, each a ring of plates lapped down over
//!   the bronze, the bronze showing in the gaps between them, to a bearing ring.
//! - Over the bearing a bronze spindle carries the sensor rings. Each ring floats clear
//!   of it with a gap all round, held by red-tipped pinch studs on a collar at its
//!   height: a toothed bronze hoop, plated carriages on it whose plates sweep back into
//!   spikes, a red sensor slot on each. The rings turn together (`part::SPINNER`), each
//!   leaning a little a different way, so they wobble out of step as they go round;
//!   from above they widen upward into a scope of rings. The spindle's cap carries red
//!   optics and the owner's colour.
//! - Tech 2 adds the second ring and braces the legs: a second course of plates on
//!   each and a ram from the mast out to it, cables up the mast. Tech 3 adds the third
//!   ring, clamp rams working at each foot (`part::PUMP`) and an armoured collar round
//!   the mast's foot.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::part;

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::*;
use super::tier::tier;

/// Where the rings turn about (the top of the bearing).
const SPIN: Vec3 = Vec3::new(0.0, 0.0, 18.4);
/// The mast: its foot and top, and its radius.
const MAST: (f32, f32, f32) = (1.0, 18.0, 0.8);
/// The legs' bearings and how far out their feet stand.
const LEGS: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const FOOT: f32 = 8.4;
const THICK: f32 = 0.3;
/// The spindle's radius, and how far its pinch studs reach out past it.
const SPINDLE: f32 = 0.55;
const STUD: f32 = 1.4;
/// Each tier's ring: its height, its radius, and the bearing (degrees) it leans toward;
/// and the top of the spindle's cap at that tier.
const RINGS: [(f32, f32, f32); 3] = [(20.6, 4.2, 0.0), (24.4, 5.0, 120.0), (28.2, 5.8, 240.0)];
const TOPS: [f32; 3] = [23.6, 27.4, 31.2];
/// How far each ring leans off level, degrees.
const LEAN: f32 = 7.0;
/// The cap's height.
const CAP: f32 = 1.4;

pub(super) fn eye(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    b.set_spinner_pivot(SPIN);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    mast(b);
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), leg);
    }
    for t in 1..=3u8 {
        // The top stage wears the cap; under a later tier's it is a plain sleeve.
        let top = t >= tech;
        tier(b, tech, t, 0.3, |b| stage(b, t, top));
    }
    tier(b, tech, 2, 0.1, |b| {
        for deg in LEGS {
            b.yawed(Vec3::ZERO, deg.to_radians(), brace);
        }
    });
    tier(b, tech, 3, 0.1, |b| {
        for deg in LEGS {
            b.yawed(Vec3::ZERO, deg.to_radians(), clamp);
        }
        foot_collar(b);
    });
}

/// Far off: the legs as plates, the mast and spindle, each ring as a flat band, the
/// owner's colour on the cap.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            dark_plate(b);
            b.face(&[
                v3(0.8, -1.0, 7.0),
                v3(FOOT + 1.0, 0.0, 0.3),
                v3(0.8, 1.0, 7.0),
            ]);
            b.face(&[
                v3(0.8, 1.0, 7.0),
                v3(FOOT + 1.0, 0.0, 0.3),
                v3(0.8, -1.0, 7.0),
            ]);
        });
    }
    let top = TOPS[tech as usize - 1];
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 2.0, 0.9, top);
    b.paint(TEAM);
    b.face(&[
        v3(-0.9, 0.0, top + 0.02),
        v3(0.0, -0.9, top + 0.02),
        v3(0.9, 0.0, top + 0.02),
        v3(0.0, 0.9, top + 0.02),
    ]);
    b.with_part(part::SPINNER, |b| {
        metal(b);
        for &(z, r, _) in &RINGS[..tech as usize] {
            let at = |k: usize, rr: f32| {
                let a = (45.0 + 90.0 * k as f32).to_radians();
                v3(a.cos() * rr, a.sin() * rr, z)
            };
            for k in 0..4 {
                b.face(&[
                    at(k, r + 0.5),
                    at(k + 1, r + 0.5),
                    at(k + 1, r - 0.5),
                    at(k, r - 0.5),
                ]);
            }
        }
    });
}

/// The mast: its footing, the ribbed bronze column, three sleeves of plates lapped down
/// over it, the bearing ring at the top.
fn mast(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (z0, z1, r) = MAST;
    seam(b);
    b.prism(Vec3::ZERO, 8, 2.6, 2.2, z0 + 1.0);
    ribbed(b, Vec3::Z * z0, Vec3::Z * z1, r, if fine { 8 } else { 0 });
    // The sleeves: four plates round the mast at each, lapped down.
    for (k, &z) in [7.0f32, 11.6, 16.0].iter().enumerate() {
        let spread = 1.25 - 0.12 * k as f32;
        for q in 0..4 {
            let a = (45.0 + 90.0 * q as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            let f = Frame::new(
                d * (r * spread) + Vec3::Z * z,
                d * 0.28 - Vec3::Z,
                d + Vec3::Z * 0.2,
            );
            dark_plate(b);
            Course {
                count: if fine { 2 } else { 1 },
                step: 1.2,
                len: 2.2,
                half: 0.75,
                tip: 0.0,
                thick: THICK,
                tail: 0.8,
            }
            .lay(b, &f);
        }
    }
    metal(b);
    hoop(
        b,
        Vec3::Z * (z1 + 0.1),
        r + 0.5,
        1.0,
        0.7,
        if fine { 16 } else { 8 },
    );
}

/// One leg, standing out along +x (turned into place by the caller): a ribbed bronze
/// strut from the mast down to a foot block, plates lapped down it into a spike.
fn leg(b: &mut MeshBuilder) {
    let fine = b.fine();
    let root = v3(MAST.2 + 0.1, 0.0, 7.6);
    let foot = v3(FOOT - 0.8, 0.0, 1.0);
    ribbed(b, root, foot, 0.42, if fine { 3 } else { 0 });
    let down = (foot - root).normalize();
    let out = v3(-down.z, 0.0, down.x);
    let f = Frame::new(root + out * 0.55, down, out);
    dark_plate(b);
    Course {
        count: 3,
        step: 2.4,
        len: 3.6,
        half: 0.95,
        tip: 0.0,
        thick: THICK,
        tail: 1.0,
    }
    .lay(b, &f);
    dark_plate(b);
    b.block(v3(FOOT - 2.0, -1.1, 0.0), v3(FOOT, 1.1, 1.3));
    if fine {
        red_slot(b, v3(FOOT + 0.02, 0.0, 0.8), Vec3::X, Vec3::Y, 1.2, 0.16);
    }
}

/// Tier `t`'s stage of the spindle: the bronze spindle up from the stage below, the
/// collar with its pinch studs, the ring floating round it, and the cap on top when this
/// is the highest stage (`top`), else a plated sleeve under the next.
fn stage(b: &mut MeshBuilder, t: u8, top: bool) {
    let fine = b.fine();
    let i = t as usize - 1;
    let (z, _, _) = RINGS[i];
    let from = if t == 1 {
        SPIN.z - 0.4
    } else {
        TOPS[i - 1] - CAP
    };
    let to = TOPS[i] - CAP;
    ribbed(
        b,
        Vec3::Z * from,
        Vec3::Z * to,
        SPINDLE,
        if fine { 2 } else { 0 },
    );
    // The collar and its studs: bronze arms out from the spindle, their tips lit red,
    // stopping well short of the ring.
    collar(b, Vec3::Z * z, Vec3::Z, SPINDLE + 0.35, 0.8);
    let studs = if fine { 3 } else { 2 };
    for k in 0..studs {
        let a = (60.0 * t as f32 + 360.0 / studs as f32 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let (root, tip) = (d * (SPINDLE + 0.6), d * (SPINDLE + STUD - 0.2));
        metal(b);
        b.cylinder_between(root + Vec3::Z * z, tip + Vec3::Z * z, 0.22, 0.16, 5);
        b.paint(GLOW_LASER);
        b.cylinder_between(
            tip + Vec3::Z * z,
            tip + d * 0.2 + Vec3::Z * z,
            0.16,
            0.08,
            6,
        );
    }
    b.with_part(part::SPINNER, |b| ring(b, t));
    if top {
        cap(b, TOPS[i]);
    } else {
        dark_plate(b);
        let sides = b.sides(8);
        b.prism(
            Vec3::Z * (TOPS[i] - CAP),
            sides,
            SPINDLE + 0.5,
            SPINDLE + 0.3,
            CAP,
        );
    }
}

/// Tier `t`'s ring (it turns): a toothed bronze hoop leaning off level, plated
/// carriages on it whose plates sweep back against the way it turns, a red sensor slot
/// on each carriage's face.
fn ring(b: &mut MeshBuilder, t: u8) {
    let (z, r, toward) = RINGS[t as usize - 1];
    let fine = b.fine();
    let carriages = 4;
    b.yawed(Vec3::Z * z, toward.to_radians(), |b| {
        b.pitched(Vec3::ZERO, LEAN.to_radians(), |b| {
            let segs = if fine { 24 } else { 12 };
            metal(b);
            hoop(b, Vec3::ZERO, r, 0.8, 0.6, segs);
            if fine {
                seam(b);
                teeth(
                    b,
                    Vec3::ZERO,
                    r - 0.4,
                    18 + 4 * t as usize,
                    v3(-0.35, 0.3, 0.45),
                );
            }
            for k in 0..carriages {
                let a = (360.0 / carriages as f32 * k as f32).to_radians();
                let d = v3(a.cos(), a.sin(), 0.0);
                // The ring turns anticlockwise from above: the plates trail clockwise.
                let back = v3(a.sin(), -a.cos(), 0.0);
                let at = d * r;
                dark_plate(b);
                b.beam(
                    at - back * 0.7 + Vec3::Z * 0.1,
                    at + back * 0.7 + Vec3::Z * 0.1,
                    Vec2::new(1.3, 0.8),
                    Vec2::new(1.1, 0.7),
                );
                let f = Frame::new(at - back * 0.5 + Vec3::Z * 0.55, back, Vec3::Z + d * 0.25);
                dark_plate(b);
                Course {
                    count: 1,
                    step: 0.0,
                    len: 1.6,
                    half: 0.55,
                    tip: -0.6,
                    thick: 0.22,
                    tail: 0.7,
                }
                .lay(b, &f);
                red_slot(b, at + d * 0.66, d, back, 0.9, 0.18);
            }
        });
    });
}

/// The spindle's cap at `top`: a plated head with red optics round it and the owner's
/// colour on top.
fn cap(b: &mut MeshBuilder, top: f32) {
    let fine = b.fine();
    let foot = top - CAP;
    dark_plate(b);
    b.frustum(
        Vec3::Z * foot,
        Vec2::splat(SPINDLE * 2.0 + 1.0),
        Vec2::splat(SPINDLE * 2.0 + 0.2),
        CAP,
        Vec2::ZERO,
    );
    b.paint(TEAM);
    let w = SPINDLE + 0.1;
    b.face(&[
        v3(w, 0.0, top + 0.03),
        v3(0.0, w, top + 0.03),
        v3(-w, 0.0, top + 0.03),
        v3(0.0, -w, top + 0.03),
    ]);
    for k in 0..if fine { 4 } else { 2 } {
        let a = (90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        red_slot(
            b,
            d * (SPINDLE + 0.62) + Vec3::Z * (foot + CAP * 0.45),
            d + Vec3::Z * 0.25,
            v3(-d.y, d.x, 0.0),
            0.7,
            0.2,
        );
    }
}

/// Tech 2's brace on one leg (along +x): a second course of plates lapped over the
/// strut, a ram from the mast out to its middle, a cable up the mast.
fn brace(b: &mut MeshBuilder) {
    let root = v3(MAST.2 + 0.1, 0.0, 7.6);
    let foot = v3(FOOT - 0.8, 0.0, 1.0);
    let down = (foot - root).normalize();
    let out = v3(-down.z, 0.0, down.x);
    let mid = root.lerp(foot, 0.55);
    let f = Frame::new(root.lerp(foot, 0.3) + out * 0.9, down, out);
    dark_plate(b);
    Course {
        count: 2,
        step: 2.0,
        len: 3.0,
        half: 1.2,
        tip: 0.0,
        thick: THICK,
        tail: 0.8,
    }
    .lay(b, &f);
    piston(b, v3(MAST.2 + 0.2, 0.0, 12.5), mid + out * 0.3, 0.28, false);
    metal(b);
    cable(
        b,
        &[
            v3(MAST.2 + 0.25, 0.0, 3.0),
            v3(MAST.2 + 0.3, 0.3, 10.0),
            v3(MAST.2 + 0.3, 0.3, 17.4),
        ],
        0.12,
    );
}

/// Tech 3's clamp at one foot (along +x): a pair of rams working down either side of the
/// foot block, a red slot between them.
fn clamp(b: &mut MeshBuilder) {
    dark_plate(b);
    b.block(v3(FOOT - 2.6, -1.7, 0.0), v3(FOOT - 1.0, 1.7, 2.0));
    for y in [-1.35f32, 1.35] {
        piston(b, v3(FOOT - 1.4, y, 4.0), v3(FOOT - 1.4, y, 1.8), 0.3, true);
    }
    red_slot(b, v3(FOOT - 0.98, 0.0, 1.4), Vec3::X, Vec3::Y, 0.9, 0.16);
}

/// Tech 3's armoured collar round the mast's foot: plates lapped up the footing.
fn foot_collar(b: &mut MeshBuilder) {
    for q in 0..8 {
        let a = (22.5 + 45.0 * q as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let f = Frame::new(
            d * 3.1 + Vec3::Z * 0.2,
            Vec3::Z * 1.0 - d * 0.45,
            d + Vec3::Z * 0.3,
        );
        dark_plate(b);
        Course {
            count: 1,
            step: 0.0,
            len: 2.4,
            half: 0.9,
            tip: 0.0,
            thick: THICK,
            tail: 0.9,
        }
        .lay(b, &f);
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_scaled, part, rig};

    /// The unit files' heights, by tier.
    const HEIGHTS: [f32; 3] = [24.0, 28.0, 32.0];

    #[test]
    fn eye_fits_at_every_tier() {
        for tech in 1..=3u8 {
            let h = HEIGHTS[tech as usize - 1];
            super::super::check_tier("naga_eye", 7.0, h, tech, Some(2), &[]);
            let model = build_model_scaled("naga_eye", 7.0, h, tech).unwrap();
            assert!(!model.spinner_scans, "the rings turn round and round");
            // A ring a tier, turning, and the next tier's ring going up in the refit.
            let rings = model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.part == part::SPINNER && v.rig & rig::UPGRADE == 0)
                .count();
            let refit = model.lods[0]
                .vertices
                .iter()
                .any(|v| v.part == part::SPINNER && v.rig & rig::UPGRADE != 0);
            assert!(rings > 0, "tech {tech}: no rings");
            assert_eq!(refit, tech < 3, "tech {tech}: the next tier's ring");
        }
    }

    /// Each ring floats: nothing that turns comes near the spindle.
    #[test]
    fn the_rings_float_clear_of_the_spindle() {
        let model = build_model_scaled("naga_eye", 7.0, 32.0, 3).unwrap();
        let near = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.part == part::SPINNER)
            .map(|v| v.pos[0].hypot(v.pos[1]))
            .fold(f32::MAX, f32::min);
        let studs = super::SPINDLE + 1.9;
        assert!(
            near > studs + 0.8,
            "a ring comes within {near} m of the axis"
        );
    }
}
